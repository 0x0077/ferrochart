//! JavaScript plugins: layer plugins
//! (`chart.addLayer`) and drawing tools (`chart.drawings.register`).
//!
//! `JsValue` is not `Send`, so JS objects never go into the `Chart`: they
//! stay in a thread-local registry and the chart holds proxies that only
//! know a registry id (wasm32 is single-threaded). A proxy calls its object
//! while the frame is built. The `ctx` it passes has conversion functions
//! that borrow the frame's coordinates and stop working when the call
//! returns. Commands and `ctx` use CSS pixels from the canvas' top-left
//! corner.
//!
//! A plugin that throws or returns an invalid command draws nothing for
//! that call (valid commands before the bad one are kept); the first error
//! of each plugin is logged with `console.warn`.

use std::cell::RefCell;
use std::fmt::Write as _;

use ferrochart_core::drawing::{
    Anchor, AnchorCount, DrawingCtx, DrawingData, DrawingStyle, DrawingTool, FrameBuilder, HitPart,
    Label, Param,
};
use ferrochart_core::error::{Error, Result as CoreResult};
use ferrochart_core::frame::{FontId, FontStyleRef, IconInst, LabelBg, StrokeStyle, TextAlign};
use ferrochart_core::geom::{PointPx, Px, RectPx, Rgba};
use ferrochart_core::plugin::{LayerCtx, LayerPlugin, LayerPosition};
use ferrochart_core::transform_helpers::{
    ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_CIRCLE, ICON_SQUARE,
};
use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::ScopedClosure;
use wasm_bindgen::prelude::*;

use crate::js::{drawing_style_js, get, param_js};
use crate::params;
use crate::web::set;

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
/** A drawing command of a JS plugin. Coordinates and sizes are CSS px from the canvas' top-left corner; colors are CSS colors. */
export type PluginCommand =
  | { type: 'rect'; x: number; y: number; width: number; height: number; color: string }
  | { type: 'line'; x1: number; y1: number; x2: number; y2: number; color: string; width?: number; dash?: LineStyle }
  | { type: 'polyline'; points: ({ x: number; y: number } | [number, number])[]; color: string; width?: number; dash?: LineStyle }
  | { type: 'polygon'; points: ({ x: number; y: number } | [number, number])[]; color: string }
  | { type: 'text'; x: number; y: number; text: string; color: string; size?: number; align?: 'left' | 'center' | 'right'; background?: string; padding?: number }
  /**
   * `image`: a name registered with `FerroChart.registerIcon`, drawn inside
   * the shape (circle or square); `border`/`borderWidth` (default 2): a ring
   * inside the edge; `label`: up to 8 characters inside the shape (not with
   * `image`).
   */
  | { type: 'icon'; x: number; y: number; shape?: 'circle' | 'square' | 'arrowUp' | 'arrowDown'; size?: number; color: string; image?: string; border?: string; borderWidth?: number; label?: string };

/** What a JS plugin gets while the frame is built. The functions only work during the call. */
export interface PluginCtx {
  /** The area drawn in (clipped to): the pane's plot, or all panes for a chart-wide layer. */
  rect: { x: number; y: number; width: number; height: number };
  dpr: number;
  /** x of a time (UTC ms), interpolated and extrapolated; `null` for a chart-wide layer or a pane without data. */
  timeToX(time: number): number | null;
  priceToY(price: number): number | null;
  xToTime(x: number): number | null;
  yToPrice(y: number): number | null;
  /** Width of `text` at font size `size` (CSS px, default 12). */
  textWidth(text: string, size?: number): number;
  /** `price` formatted like the pane's axis labels. */
  formatPrice(price: number): string;
}

/** A layer plugin for `FerroChart.addLayer`. */
export interface LayerPluginJs {
  /** Default `'aboveSeries'`. */
  position?: 'background' | 'belowSeries' | 'aboveSeries' | 'overlay';
  /** Pane id; absent: the whole chart. */
  pane?: number;
  build(ctx: PluginCtx): PluginCommand[];
}

/** What a JS drawing tool gets in addition to `PluginCtx`. */
export interface DrawingToolCtx extends PluginCtx {
  /** Anchors with their position (`null` where they have none). */
  anchors: { time: number; price: number; x: number | null; y: number | null }[];
  style: DrawingStyle;
  params: Record<string, DrawingParam>;
  /** Hit-test distance in CSS px. */
  hitTolerance: number;
}

/** A drawing tool for `Drawings.register`. */
export interface DrawingToolJs {
  /**
   * How it is created: a number (1–64) of clicks; `{ open: min }`, one click
   * per anchor until a double click or Enter, at least `min` (1–64) anchors;
   * `'freehand'`, press, drag and release (an anchor every 3 CSS px, no
   * handles).
   */
  anchorCount: number | { open: number } | 'freehand';
  build(ctx: DrawingToolCtx): PluginCommand[];
  /** `true`: the drawing is hit; a number: that handle (anchor); `false`: not hit. Default: hit inside the drawn bounds. */
  hitTest?(ctx: DrawingToolCtx, point: { x: number; y: number }): boolean | number;
}
"#;

/// A registered JS object.
struct Entry {
    object: JsValue,
    /// Whether an error of it was logged already.
    warned: bool,
}

/// A drawing kind implemented in JS.
#[derive(Clone, Copy)]
struct ToolKind {
    kind: &'static str,
    object: u32,
    anchors: AnchorCount,
}

thread_local! {
    static OBJECTS: RefCell<Vec<Option<Entry>>> = const { RefCell::new(Vec::new()) };
    static TOOLS: RefCell<Vec<ToolKind>> = const { RefCell::new(Vec::new()) };
}

fn store(object: JsValue) -> u32 {
    OBJECTS.with(|o| {
        let mut o = o.borrow_mut();
        let e = Some(Entry {
            object,
            warned: false,
        });
        let i = if let Some(i) = o.iter().position(Option::is_none) {
            o[i] = e;
            i
        } else {
            o.push(e);
            o.len() - 1
        };
        u32::try_from(i).unwrap_or(u32::MAX)
    })
}

fn release(id: u32) {
    OBJECTS.with(|o| {
        if let Ok(mut o) = o.try_borrow_mut()
            && let Some(slot) = o.get_mut(id as usize)
        {
            *slot = None;
        }
    });
}

fn object(id: u32) -> Option<JsValue> {
    OBJECTS.with(|o| {
        let o = o.try_borrow().ok()?;
        o.get(id as usize)?.as_ref().map(|e| e.object.clone())
    })
}

/// Logs the first error of plugin `id`.
fn warn(id: u32, what: &str, e: &JsValue) {
    let first = OBJECTS.with(|o| {
        let Ok(mut o) = o.try_borrow_mut() else {
            return false;
        };
        match o.get_mut(id as usize).and_then(Option::as_mut) {
            Some(entry) if !entry.warned => {
                entry.warned = true;
                true
            }
            _ => false,
        }
    });
    if first {
        web_sys::console::warn_2(&JsValue::from_str(&format!("ferrochart plugin: {what}")), e);
    }
}

/// `obj[name](args…)`; `Ok(None)` if it has no such method.
fn call(obj: &JsValue, name: &str, args: &[&JsValue]) -> Result<Option<JsValue>, JsValue> {
    let f = Reflect::get(obj, &JsValue::from_str(name))?;
    let Some(f) = f.dyn_ref::<Function>() else {
        return Ok(None);
    };
    match args {
        [a] => f.call1(obj, a),
        [a, b] => f.call2(obj, a, b),
        _ => f.call0(obj),
    }
    .map(Some)
}

fn font(size_css: f64, dpr: f64) -> FontStyleRef {
    FontStyleRef {
        font: FontId(0),
        size: (size_css * dpr) as f32,
    }
}

fn rect_js(r: RectPx, dpr: f64) -> JsValue {
    let o = Object::new();
    set(&o, "x", &(f64::from(r.x) / dpr).into());
    set(&o, "y", &(f64::from(r.y) / dpr).into());
    set(&o, "width", &(f64::from(r.w) / dpr).into());
    set(&o, "height", &(f64::from(r.h) / dpr).into());
    o.into()
}

/// What the `ctx` of a call is built from.
struct Frame<'a> {
    coords: Option<&'a DrawingCtx<'a>>,
    rect: RectPx,
    dpr: f32,
    measure: &'a dyn Fn(&str, FontStyleRef) -> f32,
}

/// Builds `ctx` (with `fill` adding the caller's keys) and runs `f` with it.
/// The conversion functions are invalidated when `f` returns.
fn with_ctx<R>(c: &Frame<'_>, fill: impl FnOnce(&Object), f: impl FnOnce(&JsValue) -> R) -> R {
    let dpr = f64::from(c.dpr);
    let coords = c.coords;
    let measure = c.measure;
    // `None` is `null` in JS (not `undefined`).
    let js = |v: Option<f64>| v.map_or(JsValue::NULL, JsValue::from_f64);
    let time_to_x = move |t: f64| -> JsValue {
        let x = (t.is_finite() && t.abs() < 9.0e15)
            .then(|| coords?.x_of(t.round() as i64))
            .flatten();
        js(x.map(|x| f64::from(x) / dpr))
    };
    let price_to_y =
        move |p: f64| -> JsValue { js(coords.and_then(|c| c.y_of(p)).map(|y| f64::from(y) / dpr)) };
    let x_to_time = move |x: f64| -> JsValue {
        js(coords
            .and_then(|c| c.time_at((x * dpr) as f32))
            .map(|t| t as f64))
    };
    let y_to_price = move |y: f64| -> JsValue {
        let p = coords.map(|c| c.price_at((y * dpr) as f32));
        js(p.filter(|p| p.is_finite()))
    };
    let text_width = move |s: String, size: Option<f64>| -> f64 {
        f64::from(measure(&s, font(size.unwrap_or(12.0), dpr))) / dpr
    };
    let format_price = move |p: f64| -> String {
        let mut s = String::new();
        match coords {
            Some(c) => c.write_price(&mut s, p),
            None => {
                let _ = write!(s, "{p}");
            }
        }
        s
    };
    let time_to_x = ScopedClosure::<dyn Fn(f64) -> JsValue>::borrow_aborting(&time_to_x);
    let price_to_y = ScopedClosure::<dyn Fn(f64) -> JsValue>::borrow_aborting(&price_to_y);
    let x_to_time = ScopedClosure::<dyn Fn(f64) -> JsValue>::borrow_aborting(&x_to_time);
    let y_to_price = ScopedClosure::<dyn Fn(f64) -> JsValue>::borrow_aborting(&y_to_price);
    let text_width =
        ScopedClosure::<dyn Fn(String, Option<f64>) -> f64>::borrow_aborting(&text_width);
    let format_price = ScopedClosure::<dyn Fn(f64) -> String>::borrow_aborting(&format_price);
    let o = Object::new();
    set(&o, "rect", &rect_js(c.rect, dpr));
    set(&o, "dpr", &dpr.into());
    set(&o, "timeToX", time_to_x.as_js_value());
    set(&o, "priceToY", price_to_y.as_js_value());
    set(&o, "xToTime", x_to_time.as_js_value());
    set(&o, "yToPrice", y_to_price.as_js_value());
    set(&o, "textWidth", text_width.as_js_value());
    set(&o, "formatPrice", format_price.as_js_value());
    fill(&o);
    f(&o.into())
}

fn num(o: &JsValue, k: &str) -> Result<f64, String> {
    get(o, k)
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("'{k}' must be a finite number"))
}

fn opt_num(o: &JsValue, k: &str) -> Result<Option<f64>, String> {
    let v = get(o, k);
    if v.is_undefined() || v.is_null() {
        return Ok(None);
    }
    num(o, k).map(Some)
}

fn color(o: &JsValue, k: &str) -> Result<Rgba, String> {
    get(o, k)
        .as_string()
        .and_then(|s| params::color(&s))
        .ok_or_else(|| format!("'{k}' must be a CSS color"))
}

/// Points of `o.points`: `{x, y}` objects or `[x, y]` pairs.
fn points(o: &JsValue, dpr: f32) -> Result<Vec<PointPx>, String> {
    let v = get(o, "points");
    let a = v.dyn_ref::<Array>().ok_or("'points' must be an array")?;
    a.iter()
        .map(|p| {
            let (x, y) = if Array::is_array(&p) {
                let p: &Array = p.unchecked_ref();
                (p.get(0).as_f64(), p.get(1).as_f64())
            } else {
                (get(&p, "x").as_f64(), get(&p, "y").as_f64())
            };
            match (x, y) {
                (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok(PointPx {
                    x: x as f32 * dpr,
                    y: y as f32 * dpr,
                }),
                _ => Err("a point must be {x, y} or [x, y] with finite numbers".to_owned()),
            }
        })
        .collect()
}

fn stroke(o: &JsValue, dpr: f32) -> Result<StrokeStyle, String> {
    let width = opt_num(o, "width")?.unwrap_or(1.0) as f32;
    let dash = match get(o, "dash").as_string() {
        Some(s) => params::line_dash(&s).ok_or("'dash' must be 'solid', 'dashed' or 'dotted'")?,
        None => ferrochart_core::frame::LineDash::Solid,
    };
    Ok(StrokeStyle {
        width: Px((width * dpr).round().max(1.0)),
        color: color(o, "color")?,
        dash,
    })
}

/// Writes the commands `cmds` returned by a plugin.
fn emit(cmds: &JsValue, dpr: f32, out: &mut FrameBuilder<'_>) -> Result<(), String> {
    if cmds.is_undefined() || cmds.is_null() {
        return Ok(());
    }
    let a = cmds
        .dyn_ref::<Array>()
        .ok_or("build must return an array of commands")?;
    let p = |o: &JsValue, kx: &str, ky: &str| -> Result<PointPx, String> {
        Ok(PointPx {
            x: num(o, kx)? as f32 * dpr,
            y: num(o, ky)? as f32 * dpr,
        })
    };
    for c in a.iter() {
        let kind = get(&c, "type").as_string().unwrap_or_default();
        match kind.as_str() {
            "rect" => {
                let at = p(&c, "x", "y")?;
                let rect = RectPx {
                    x: at.x,
                    y: at.y,
                    w: num(&c, "width")? as f32 * dpr,
                    h: num(&c, "height")? as f32 * dpr,
                };
                out.rect(rect, color(&c, "color")?);
            }
            "line" => {
                let (a, b) = (p(&c, "x1", "y1")?, p(&c, "x2", "y2")?);
                out.line(a, b, stroke(&c, dpr)?);
            }
            "polyline" => out.polyline(points(&c, dpr)?, stroke(&c, dpr)?),
            "polygon" => out.polygon(points(&c, dpr)?, color(&c, "color")?),
            "text" => {
                let text = get(&c, "text")
                    .as_string()
                    .ok_or("'text' must be a string")?;
                let align = match get(&c, "align").as_string().as_deref() {
                    None | Some("left") => TextAlign::Left,
                    Some("center") => TextAlign::Center,
                    Some("right") => TextAlign::Right,
                    Some(_) => return Err("'align' must be 'left', 'center' or 'right'".into()),
                };
                let bg = match get(&c, "background").as_string() {
                    Some(s) => Some(LabelBg {
                        color: params::color(&s).ok_or("'background' must be a CSS color")?,
                        padding: Px((opt_num(&c, "padding")?.unwrap_or(2.0) as f32 * dpr).round()),
                    }),
                    None => None,
                };
                let label = Label {
                    font: font(opt_num(&c, "size")?.unwrap_or(12.0), f64::from(dpr)),
                    color: color(&c, "color")?,
                    align,
                    bg,
                };
                out.text(p(&c, "x", "y")?, &text, label);
            }
            "icon" => {
                let icon = match get(&c, "shape").as_string().as_deref() {
                    None | Some("circle") => ICON_CIRCLE,
                    Some("square") => ICON_SQUARE,
                    Some("arrowUp") => ICON_ARROW_UP,
                    Some("arrowDown") => ICON_ARROW_DOWN,
                    Some(_) => {
                        return Err(
                            "'shape' must be 'circle', 'square', 'arrowUp' or 'arrowDown'".into(),
                        );
                    }
                };
                let arrow = icon == ICON_ARROW_UP || icon == ICON_ARROW_DOWN;
                let image = match get(&c, "image").as_string() {
                    Some(name) => Some(
                        crate::icons::lookup_current(&name)
                            .ok_or_else(|| format!("unknown icon image '{name}'"))?,
                    ),
                    None => None,
                };
                let label = get(&c, "label").as_string().unwrap_or_default();
                if arrow && (image.is_some() || !label.is_empty()) {
                    return Err("'image' and 'label' need a circle or square shape".into());
                }
                if image.is_some() && !label.is_empty() {
                    return Err("an icon has an 'image' or a 'label', not both".into());
                }
                if label.chars().count() > 8 {
                    return Err("'label' is at most 8 characters".into());
                }
                let border = match get(&c, "border").as_string() {
                    Some(b) => params::color(&b).ok_or("'border' must be a CSS color")?,
                    None => Rgba::default(),
                };
                let border_width = opt_num(&c, "borderWidth")?.unwrap_or(2.0);
                if !(border_width.is_finite() && border_width >= 0.0) {
                    return Err("'borderWidth' must be finite and >= 0".into());
                }
                let size_css = opt_num(&c, "size")?.unwrap_or(8.0);
                let inst = IconInst {
                    pos: p(&c, "x", "y")?,
                    size: Px(size_css as f32 * dpr),
                    icon,
                    color: color(&c, "color")?,
                    image,
                    border,
                    border_width: Px(if border.a == 0 {
                        0.0
                    } else {
                        border_width as f32 * dpr
                    }),
                };
                out.icon(inst);
                if !label.is_empty() {
                    // As marker labels: Latin at 0.45 × size, CJK at
                    // 0.6, black or white by the fill's luminance.
                    let ratio = if label.is_ascii() { 0.45 } else { 0.6 };
                    let c8 = inst.color;
                    let lum =
                        0.299 * f64::from(c8.r) + 0.587 * f64::from(c8.g) + 0.114 * f64::from(c8.b);
                    let ink = if lum > 150.0 {
                        Rgba::hex(0x0013_1722)
                    } else {
                        Rgba::hex(0x00FF_FFFF)
                    };
                    let l = Label {
                        font: font((size_css * ratio).round().max(7.0), f64::from(dpr)),
                        color: ink,
                        align: TextAlign::Center,
                        bg: None,
                    };
                    out.text(inst.pos, &label, l);
                }
            }
            other => {
                return Err(format!(
                    "unknown command type '{other}' (rect, line, polyline, polygon, text, icon)"
                ));
            }
        }
    }
    Ok(())
}

/// Emits the result of `build`, logging the first error of plugin `id`.
fn finish(id: u32, result: Result<Option<JsValue>, JsValue>, dpr: f32, out: &mut FrameBuilder<'_>) {
    match result {
        Ok(Some(cmds)) => {
            if let Err(e) = emit(&cmds, dpr, out) {
                warn(id, "invalid command", &JsValue::from_str(&e));
            }
        }
        Ok(None) => warn(id, "no build() method", &JsValue::UNDEFINED),
        Err(e) => warn(id, "build() threw", &e),
    }
}

/// Parses a layer position name.
pub(crate) fn layer_position(s: &str) -> Option<LayerPosition> {
    Some(match s {
        "background" => LayerPosition::Background,
        "belowSeries" => LayerPosition::BelowSeries,
        "aboveSeries" => LayerPosition::AboveSeries,
        "overlay" => LayerPosition::Overlay,
        _ => return None,
    })
}

/// A layer plugin implemented in JS. Dropping it (with its layer,
/// pane or chart) releases the JS object.
pub(crate) struct JsLayer {
    id: u32,
    position: LayerPosition,
}

impl JsLayer {
    pub(crate) fn new(object: JsValue, position: LayerPosition) -> Self {
        Self {
            id: store(object),
            position,
        }
    }
}

impl Drop for JsLayer {
    fn drop(&mut self) {
        release(self.id);
    }
}

impl LayerPlugin for JsLayer {
    fn position(&self) -> LayerPosition {
        self.position
    }

    fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
        let Some(obj) = object(self.id) else {
            return;
        };
        let measure = |s: &str, f: FontStyleRef| ctx.text_width(s, f);
        let frame = Frame {
            coords: ctx.coords(),
            rect: ctx.rect(),
            dpr: ctx.dpr(),
            measure: &measure,
        };
        let result = with_ctx(&frame, |_| {}, |o| call(&obj, "build", &[o]));
        finish(self.id, result, ctx.dpr(), out);
    }
}

/// Registers drawing kind `kind` implemented by the JS object `tool` with
/// `anchors` anchors, replacing an earlier JS registration of the kind;
/// returns the kind for `DrawingManager::register`.
pub(crate) fn register_tool(kind: &str, tool: JsValue, anchors: AnchorCount) -> &'static str {
    TOOLS.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(k) = t.iter_mut().find(|k| k.kind == kind) {
            release(k.object);
            k.object = store(tool);
            k.anchors = anchors;
            k.kind
        } else {
            // Kinds are `&'static str` in core; one small string per
            // registered kind, kept for the page's lifetime.
            let kind: &'static str = Box::leak(kind.to_owned().into_boxed_str());
            t.push(ToolKind {
                kind,
                object: store(tool),
                anchors,
            });
            kind
        }
    })
}

/// The `DrawingFactory` of every JS drawing kind.
pub(crate) fn js_tool(data: &DrawingData) -> CoreResult<Box<dyn DrawingTool>> {
    let k = TOOLS
        .with(|t| t.borrow().iter().find(|k| k.kind == data.kind).copied())
        .ok_or(Error::InvalidArgument("unknown drawing kind"))?;
    Ok(Box::new(JsTool {
        kind: k.kind,
        object: k.object,
        count: k.anchors,
        anchors: data.anchors.clone(),
        style: data.style,
        params: data.params.clone(),
    }))
}

/// A drawing whose kind is implemented in JS.
struct JsTool {
    kind: &'static str,
    object: u32,
    count: AnchorCount,
    anchors: Vec<Anchor>,
    style: DrawingStyle,
    params: Vec<(String, Param)>,
}

impl JsTool {
    /// The tool's keys of `ctx`: anchors (with their position), style,
    /// params and the hit tolerance.
    fn fill(&self, o: &Object, ctx: &DrawingCtx<'_>) {
        let dpr = f64::from(ctx.dpr());
        let anchors: Array = self
            .anchors
            .iter()
            .map(|a| {
                let p = ctx.point(*a);
                let x = Object::new();
                set(&x, "time", &JsValue::from_f64(a.time as f64));
                set(&x, "price", &a.price.into());
                set(
                    &x,
                    "x",
                    &p.map_or(JsValue::NULL, |p| (f64::from(p.x) / dpr).into()),
                );
                set(
                    &x,
                    "y",
                    &p.map_or(JsValue::NULL, |p| (f64::from(p.y) / dpr).into()),
                );
                JsValue::from(x)
            })
            .collect();
        set(o, "anchors", &anchors);
        set(o, "style", &drawing_style_js(&self.style));
        let params = Object::new();
        for (k, v) in &self.params {
            set(&params, k, &param_js(v));
        }
        set(o, "params", &params);
        set(
            o,
            "hitTolerance",
            &(f64::from(ctx.hit_tolerance()) / dpr).into(),
        );
    }

    fn frame<'a>(
        ctx: &'a DrawingCtx<'a>,
        measure: &'a dyn Fn(&str, FontStyleRef) -> f32,
    ) -> Frame<'a> {
        Frame {
            coords: Some(ctx),
            rect: ctx.plot(),
            dpr: ctx.dpr(),
            measure,
        }
    }
}

impl DrawingTool for JsTool {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    fn anchors_mut(&mut self) -> &mut Vec<Anchor> {
        &mut self.anchors
    }

    fn anchor_count(&self) -> AnchorCount {
        self.count
    }

    fn build(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>) {
        let Some(obj) = object(self.object) else {
            return;
        };
        let measure = |s: &str, f: FontStyleRef| ctx.text_width(s, f);
        let result = with_ctx(
            &Self::frame(ctx, &measure),
            |o| self.fill(o, ctx),
            |o| call(&obj, "build", &[o]),
        );
        finish(self.object, result, ctx.dpr(), out);
    }

    fn hit_test(&self, ctx: &DrawingCtx<'_>, p: PointPx) -> Option<HitPart> {
        let obj = object(self.object)?;
        let dpr = f64::from(ctx.dpr());
        let point = Object::new();
        set(&point, "x", &(f64::from(p.x) / dpr).into());
        set(&point, "y", &(f64::from(p.y) / dpr).into());
        let measure = |s: &str, f: FontStyleRef| ctx.text_width(s, f);
        let result = with_ctx(
            &Self::frame(ctx, &measure),
            |o| self.fill(o, ctx),
            |o| call(&obj, "hitTest", &[o, &point]),
        );
        match result {
            // Without `hitTest`, anything within the drawn bounds hits.
            Ok(None) => Some(HitPart::Body),
            Ok(Some(v)) => {
                if v.as_bool() == Some(true) || v.as_string().as_deref() == Some("body") {
                    Some(HitPart::Body)
                } else {
                    v.as_f64()
                        .filter(|n| *n >= 0.0 && n.fract() == 0.0)
                        .map(|n| HitPart::Handle(n as usize))
                }
            }
            Err(e) => {
                warn(self.object, "hitTest() threw", &e);
                None
            }
        }
    }

    fn handles(&self, ctx: &DrawingCtx<'_>, out: &mut Vec<PointPx>) {
        // Like the built-in brush, freehand drawings have no handles.
        if self.count == AnchorCount::Freehand {
            return;
        }
        out.extend(self.anchors.iter().filter_map(|a| ctx.point(*a)));
    }

    fn style(&self) -> &DrawingStyle {
        &self.style
    }

    fn save(&self) -> DrawingData {
        DrawingData {
            kind: self.kind.to_owned(),
            anchors: self.anchors.clone(),
            style: self.style,
            params: self.params.clone(),
        }
    }
}
