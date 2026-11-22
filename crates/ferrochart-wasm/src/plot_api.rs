//! The browser plot: `FerroPlot` and its `Mark`,
//! `Axis`, `Rule` and `Label` handles, and `FerroChart.attachSidePlot`.
//! The canvas, renderer, frame loop and input are the chart's
//! (`host`); `PlotScene` is the plot's side of them.
//!
//! Options are plain JS objects: absent keys keep the current (or default)
//! value, `null` resets a color to the theme's, wrong types are errors.
//!
//! Doc comments here become the JS API docs: they use JSDoc tags and describe
//! errors as thrown exceptions, hence the allowed lints.
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]
// `Option<Option<_>>`: absent (keep) vs `null` (theme default) vs a color.
#![allow(clippy::option_option)]

use std::rc::{Rc, Weak};

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::data::PaneId;
use ferrochart_core::frame::{Frame, Invalidation};
use ferrochart_core::geom::{Css, Rgba, SizeCss};
use ferrochart_core::interaction::{InputEvent, InputResult};
use ferrochart_core::price_scale::{PriceScaleMode, ScaleSide};
use ferrochart_plot::{
    AxisFormat, AxisId, AxisOptions, AxisRange, ColorScale, Hit, LabelId, LabelOptions, MarkData,
    MarkId, MarkKind, MarkOptions, MarkPoint, Plot, PlotEvent, PlotOptions, RuleId, RuleOptions,
    ScaleDomain, ScaleKind,
};
use ferrochart_render_wgpu::WgpuRenderer;
use js_sys::{Array, Float32Array, Function, Object};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::host::{self, Host, Scene, State, js_err, set};
use crate::js::{
    absent, color_column, color_js, get, opt_bool, opt_color, opt_dash, opt_f64, opt_string,
    opt_width,
};
use crate::plot_params as names;
use crate::web::{FerroChart, Shared, f64_column};

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
/** Options of `FerroPlot.create`. */
export interface PlotCreateOptions {
  /** Color theme. Default `'light'`. */
  theme?: 'light' | 'dark';
  /** GPU API. `'auto'` (default) tries WebGPU, then WebGL2. */
  renderer?: 'auto' | 'webgpu' | 'webgl2';
  /** Crosshair lines through the hovered element. Default `false`. */
  crosshair?: boolean;
  /** Highlight the hovered element. Default `true`. */
  hoverHighlight?: boolean;
  /** The FerroChart logo at the bottom-left of the plot area, as for charts. Default `true`. */
  attributionLogo?: boolean;
}

export type MarkKind = 'line' | 'area' | 'scatter' | 'bars' | 'waterfall' | 'cells';
export type AxisName = 'x' | 'left' | 'right';
export type PlotIcon = 'circle' | 'square' | 'arrowUp' | 'arrowDown';

/** How labels and value texts are written. */
export type PlotFormat =
  | 'auto'
  | 'compact'
  | 'date'
  | { type: 'fixed' | 'percent'; decimals: number }
  | { type: 'date'; utcOffsetMinutes: number };

/** Options of an axis (`plot.axis(name).applyOptions`). */
export interface AxisOptions {
  type?: 'linear' | 'log' | 'category';
  /** `'auto'` follows the data (after a pan or zoom, until `fit()`); `[lo, hi]` is fixed. */
  range?: 'auto' | [number, number];
  /** Automatic range: keep 0 inside. */
  includeZero?: boolean;
  /** Automatic range: fraction added at both ends (default 0.05, category axes 0). */
  margin?: number;
  format?: PlotFormat;
  inverted?: boolean;
  visible?: boolean;
  grid?: boolean;
  /** `null` removes it. */
  title?: string | null;
  /** Panning and zooming leave the axis alone. */
  locked?: boolean;
}

/** A color scale: a preset, or stops (positions 0 to 1) with an optional diverging center and domain. */
export type ColorScaleOptions =
  | 'sequential'
  | 'diverging'
  | {
      stops?: [number, string][];
      /** Diverging around this value. */
      center?: number;
      /** Fixed value range; otherwise the `quantile` and `1 − quantile` quantiles of the values. */
      domain?: [number, number];
      quantile?: number;
    };

/** Options of a mark (`addMark`, `Mark.applyOptions`). */
export interface MarkOptions {
  yAxis?: 'left' | 'right';
  orientation?: 'vertical' | 'horizontal';
  /** CSS colors; `null`: the palette / theme default. */
  color?: string | null;
  downColor?: string | null;
  totalColor?: string | null;
  fillAlpha?: number;
  lineWidth?: number;
  lineStyle?: 'solid' | 'dashed' | 'dotted';
  stepLine?: boolean;
  pointSize?: number;
  icon?: PlotIcon;
  baseValue?: number;
  /** Bars of the same group share a slot side by side; `null` for none. */
  group?: number | null;
  /** Bars or areas of the same stack add up; `null` for none. */
  stack?: number | null;
  /** Fraction of the slot taken by bars and cells; `null` for the default (0.8 / 1). */
  band?: number | null;
  /** `null`: the default sequential scale. */
  colorScale?: ColorScaleOptions | null;
  showValues?: boolean;
  valueFormat?: PlotFormat;
  /** Cells on numeric axes: width and height in data units; `null` for automatic. */
  cellSize?: [number, number] | null;
  hoverable?: boolean;
  visible?: boolean;
}

type Column = Float64Array | number[];

/** Columns of `Mark.setData` / `append`; all have the same length. */
export interface MarkData {
  /** Numbers, UTC ms or category numbers (values for horizontal bars). */
  x: Column;
  /** Values, `NaN` for a gap (category numbers for horizontal bars). */
  y: Column;
  /** Lower edge of areas, start of bars. */
  y0?: Column;
  /** Values colored by the color scale (cells, bars, scatter). */
  value?: Column;
  /** Scatter diameters, CSS px. */
  size?: Float32Array | number[];
  /** Per-point CSS colors (`null`: the mark color) or `0xRRGGBBAA`. */
  color?: (string | null)[] | Uint32Array;
  step?: ('delta' | 'subtotal' | 'total')[];
  /** Per-point texts (scatter labels; replaces value texts). */
  label?: (string | null)[];
}

/** The new last point for `Mark.update`. */
export interface MarkPoint {
  x: number;
  y: number;
  y0?: number;
  value?: number;
  size?: number;
  color?: string | null;
}

/** Options of `addRule` / `Rule.applyOptions`. */
export interface RuleOptions {
  /** The axis of `value`; `'x'` gives a vertical rule. Default `'right'`. */
  axis?: AxisName;
  value?: number;
  /** Other edge of a band; `null` for a line. */
  to?: number | null;
  color?: string | null;
  lineWidth?: number;
  lineStyle?: 'solid' | 'dashed' | 'dotted';
  /** Text on the axis; `null` for none. */
  label?: string | null;
  labelBackground?: boolean;
}

/** Options of `addLabel`. */
export interface LabelOptions {
  x: number;
  y: number;
  yAxis?: 'left' | 'right';
  text?: string;
  icon?: PlotIcon | null;
  color?: string | null;
  align?: 'left' | 'center' | 'right';
  /** Offset of the text from the point, CSS px right and down. Default `[6, 0]`. */
  offset?: [number, number];
}

/** The element under the pointer. */
export interface PlotHit {
  mark: Mark;
  /** Index of the point in the mark's data. */
  index: number;
  x: number;
  /** For waterfall subtotals and totals, the running total. */
  y: number;
  /** Where it is drawn (a point, or the center of a bar or cell), CSS px relative to the canvas. */
  position: { x: number; y: number };
}

export interface PlotEventMap {
  /** The hovered element changed. */
  hover: { hit: PlotHit | null };
  /** A click, with the element and the axis values at the pointer (`y` on the right axis, the left if the right is hidden). */
  click: { hit: PlotHit | null; x: number; y: number };
  /** The visible range of an axis changed (pan, zoom, `fit`). */
  viewChange: {};
}

export interface FerroPlot {
  /**
   * Subscribes to an event of `PlotEventMap`. Events are delivered after the
   * frame that produced them. An exception thrown by the callback is
   * reported (`reportError`, shown in the console) and does not stop the
   * other callbacks.
   */
  on<K extends keyof PlotEventMap>(event: K, callback: (event: PlotEventMap[K]) => void): void;
  /** Removes a callback added with `on`. */
  off<K extends keyof PlotEventMap>(event: K, callback: (event: PlotEventMap[K]) => void): void;
}
"#;

/// The plot and what the binding keeps next to it.
pub(crate) struct PlotScene {
    pub(crate) plot: Plot,
    /// `attachSidePlot`: the chart and pane whose price range the right axis
    /// follows.
    link: Option<(Weak<Shared>, PaneId)>,
    /// Options of the rules, for `Rule.applyOptions` (the plot has no getter).
    rules: Vec<(RuleId, RuleOptions)>,
}

impl Scene for PlotScene {
    const NAME: &'static str = "FerroPlot";
    const EVENTS: &'static [&'static str] = &["hover", "click", "viewChange"];

    fn resize(&mut self, size: SizeCss, dpr: f32) {
        self.plot.resize(size, dpr);
    }

    fn handle_input(&mut self, event: InputEvent) -> InputResult {
        self.plot.handle_input(event)
    }

    fn needs_redraw(&self) -> Invalidation {
        self.plot.needs_redraw()
    }

    fn build_frame(&mut self, now_ms: f64, renderer: &WgpuRenderer) {
        self.plot.build_frame(now_ms, &renderer.measurer());
    }

    fn frame(&self) -> &Frame {
        self.plot.frame()
    }

    fn data_view(&self) -> ChartDataView<'_> {
        ChartDataView::empty()
    }

    fn set_reveal(&mut self, reveal: Option<ferrochart_core::frame::Reveal>) {
        self.plot.set_reveal(reveal);
    }

    fn logo_anchor(&self) -> ferrochart_core::geom::PointPx {
        let plot = self.plot.layout().plot;
        ferrochart_core::geom::PointPx {
            x: plot.x,
            y: plot.bottom(),
        }
    }

    fn prepare(state: &mut State<Self>) {
        follow_chart(state);
    }

    fn take_events(
        host: &Rc<Host<Self>>,
        state: &mut State<Self>,
        out: &mut Vec<(&'static str, JsValue)>,
    ) -> bool {
        let weak = Rc::downgrade(host);
        for e in state.scene.plot.drain_events() {
            let o = Object::new();
            let name = match e {
                PlotEvent::Hover(hit) => {
                    set(&o, "hit", &hit_js(&weak, hit));
                    "hover"
                }
                PlotEvent::Click { hit, x, y } => {
                    set(&o, "hit", &hit_js(&weak, hit));
                    set(&o, "x", &x.into());
                    set(&o, "y", &y.into());
                    "click"
                }
                PlotEvent::ViewChanged => "viewChange",
            };
            out.push((name, o.into()));
        }
        false
    }

    fn after_frame(host: &Rc<Host<Self>>, _now: f64) {
        // The first frame of a linked plot lays it out; the range follows.
        let moved = host
            .state
            .try_borrow_mut()
            .is_ok_and(|mut s| follow_chart(&mut s));
        if moved {
            host::schedule(host);
        }
    }
}

/// Copies the linked chart pane's price range onto the right axis, matched
/// to where the two canvases are on the page: a price is at the same height
/// in both. Returns whether the view changed.
fn follow_chart(state: &mut State<PlotScene>) -> bool {
    let Some((chart, pane)) = &state.scene.link else {
        return false;
    };
    let Some(chart) = chart.upgrade() else {
        state.scene.link = None;
        return false;
    };
    let Ok(mut cs) = chart.state.try_borrow_mut() else {
        return false;
    };
    let c = &mut cs.scene.chart;
    let (Some(range), Some(layout)) = (c.visible_price_range(*pane), c.pane_layout(*pane)) else {
        return false;
    };
    let Some(scale) = c.price_scale(*pane, ScaleSide::Right) else {
        return false;
    };
    let log = scale.mode() == PriceScaleMode::Logarithmic;
    let inverted = scale.inverted();
    let top =
        |el: &HtmlCanvasElement| el.get_bounding_client_rect().top() + f64::from(el.client_top());
    let cdpr = cs.dpr;
    let pane_top = top(&cs.canvas) + f64::from(layout.plot.y) / cdpr;
    let pane_h = f64::from(layout.plot.h) / cdpr;
    let plot = state.scene.plot.layout().plot;
    if pane_h <= 0.0 || plot.h <= 0.0 {
        return false;
    }
    let plot_top = top(&state.canvas) + f64::from(plot.y) / state.dpr;
    let plot_bottom = plot_top + f64::from(plot.h) / state.dpr;
    let at = |y: f64| names::price_at((y - pane_top) / pane_h, range, log, inverted);
    let (a, b) = (at(plot_top), at(plot_bottom));
    let (lo, hi) = (a.min(b), a.max(b));
    let p = &mut state.scene.plot;
    let (vlo, vhi) = p.view(AxisId::Right);
    let same = |u: f64, v: f64| (u - v).abs() <= (hi - lo).abs() * 1e-9;
    if same(lo, vlo) && same(hi, vhi) {
        return false;
    }
    p.set_view(AxisId::Right, lo, hi).is_ok()
}

fn hit_js(plot: &Weak<Host<PlotScene>>, hit: Option<Hit>) -> JsValue {
    let Some(h) = hit else {
        return JsValue::NULL;
    };
    let o = Object::new();
    let mark = Mark {
        plot: plot.clone(),
        id: h.mark,
    };
    set(&o, "mark", &mark.into());
    set(&o, "index", &JsValue::from_f64(h.index as f64));
    set(&o, "x", &h.x.into());
    set(&o, "y", &h.y.into());
    let pos = Object::new();
    set(&pos, "x", &f64::from(h.pos.x).into());
    set(&pos, "y", &f64::from(h.pos.y).into());
    set(&o, "position", &pos.into());
    o.into()
}

// ---- JS → plot types -------------------------------------------------

fn bad(k: &str, what: &str) -> JsError {
    js_err(format!("'{k}' must be {what}"))
}

/// A name looked up with `f`, or an error listing the accepted names.
fn opt_name<T>(
    o: &JsValue,
    k: &str,
    f: impl Fn(&str) -> Option<T>,
    accepted: &str,
) -> Result<Option<T>, JsError> {
    opt_string(o, k)?
        .map(|s| f(&s).ok_or_else(|| bad(k, accepted)))
        .transpose()
}

fn finite(o: &JsValue, k: &str) -> Result<Option<f64>, JsError> {
    match opt_f64(o, k)? {
        Some(v) if !v.is_finite() => Err(bad(k, "a finite number")),
        v => Ok(v),
    }
}

/// A `[a, b]` pair of finite numbers.
fn pair(v: &JsValue, k: &str) -> Result<(f64, f64), JsError> {
    let err = || bad(k, "an array of two finite numbers");
    if !Array::is_array(v) {
        return Err(err());
    }
    let a: &Array = v.unchecked_ref();
    let (x, y) = (a.get(0).as_f64(), a.get(1).as_f64());
    match (x, y) {
        (Some(x), Some(y)) if a.length() == 2 && x.is_finite() && y.is_finite() => Ok((x, y)),
        _ => Err(err()),
    }
}

fn format(o: &JsValue, k: &str) -> Result<Option<AxisFormat>, JsError> {
    let v = get(o, k);
    if absent(&v) {
        return Ok(None);
    }
    let err = || {
        bad(
            k,
            "'auto', 'compact', 'date', { type: 'fixed' | 'percent', decimals } or { type: 'date', utcOffsetMinutes }",
        )
    };
    if let Some(s) = v.as_string() {
        return names::axis_format(&s).map(Some).ok_or_else(err);
    }
    let t = opt_string(&v, "type")?.ok_or_else(err)?;
    if t == "date" {
        let m = opt_f64(&v, "utcOffsetMinutes")?.unwrap_or(0.0);
        if !(m.fract() == 0.0 && m.abs() <= 1440.0) {
            return Err(err());
        }
        return Ok(Some(AxisFormat::Date {
            utc_offset_min: m as i32,
        }));
    }
    let d = opt_f64(&v, "decimals")?.ok_or_else(err)?;
    names::axis_format_decimals(&t, d).map(Some).ok_or_else(err)
}

fn format_js(f: AxisFormat) -> JsValue {
    match names::axis_format_parts(f) {
        (t, None, None) => JsValue::from_str(t),
        (t, d, m) => {
            let o = Object::new();
            set(&o, "type", &t.into());
            if let Some(d) = d {
                set(&o, "decimals", &f64::from(d).into());
            }
            if let Some(m) = m {
                set(&o, "utcOffsetMinutes", &f64::from(m).into());
            }
            o.into()
        }
    }
}

fn y_axis(o: &JsValue, k: &str) -> Result<Option<AxisId>, JsError> {
    match opt_name(o, k, names::axis_id, "'left' or 'right'")? {
        Some(AxisId::X) => Err(bad(k, "'left' or 'right'")),
        v => Ok(v),
    }
}

/// `undefined` keeps, `null` clears, a number sets (a non-negative integer).
fn opt_id_number(o: &JsValue, k: &str) -> Result<Option<Option<u32>>, JsError> {
    let v = get(o, k);
    if v.is_undefined() {
        return Ok(None);
    }
    if v.is_null() {
        return Ok(Some(None));
    }
    match v.as_f64() {
        Some(n) if n.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&n) => {
            Ok(Some(Some(n as u32)))
        }
        _ => Err(bad(k, "a non-negative integer or null")),
    }
}

fn color_scale(v: &JsValue, theme: &ferrochart_core::theme::Theme) -> Result<ColorScale, JsError> {
    const K: &str = "colorScale";
    if let Some(s) = v.as_string() {
        return match s.as_str() {
            "sequential" => Ok(ColorScale::sequential()),
            "diverging" => Ok(ColorScale::diverging(0.0, theme)),
            _ => Err(bad(K, "'sequential', 'diverging' or an object")),
        };
    }
    if !v.is_object() {
        return Err(bad(K, "'sequential', 'diverging' or an object"));
    }
    let center = finite(v, "center")?;
    let mut s = match center {
        Some(c) => ColorScale::diverging(c, theme),
        None => ColorScale::sequential(),
    };
    let stops = get(v, "stops");
    if !absent(&stops) {
        let err = || bad("stops", "an array of [position 0..1, CSS color], ascending");
        if !Array::is_array(&stops) {
            return Err(err());
        }
        let mut out = Vec::new();
        for e in stops.unchecked_ref::<Array>().iter() {
            let pos = Array::is_array(&e)
                .then(|| e.unchecked_ref::<Array>().get(0).as_f64())
                .flatten()
                .filter(|p| (0.0..=1.0).contains(p))
                .ok_or_else(err)?;
            let c = e
                .unchecked_ref::<Array>()
                .get(1)
                .as_string()
                .and_then(|c| crate::params::color(&c))
                .ok_or_else(err)?;
            if out
                .last()
                .is_some_and(|l: &(f32, Rgba)| f64::from(l.0) > pos)
            {
                return Err(err());
            }
            out.push((pos as f32, c));
        }
        if out.is_empty() {
            return Err(err());
        }
        s.stops = out;
    }
    let domain = get(v, "domain");
    if !absent(&domain) {
        let (a, b) = pair(&domain, "domain")?;
        s.domain = ScaleDomain::Fixed(a, b);
    } else if let Some(q) = opt_f64(v, "quantile")? {
        if !(0.0..=0.5).contains(&q) {
            return Err(bad("quantile", "between 0 and 0.5"));
        }
        s.domain = ScaleDomain::Auto { quantile: q as f32 };
    }
    Ok(s)
}

fn color_scale_js(s: &ColorScale) -> JsValue {
    let o = Object::new();
    let stops: Array = s
        .stops
        .iter()
        .map(|(p, c)| {
            let e = Array::new();
            e.push(&f64::from(*p).into());
            e.push(&color_js(Some(*c)));
            JsValue::from(e)
        })
        .collect();
    set(&o, "stops", &stops.into());
    if let ScaleKind::Diverging { center } = s.kind {
        set(&o, "center", &center.into());
    }
    match s.domain {
        ScaleDomain::Fixed(a, b) => set(&o, "domain", &pair_js(a, b)),
        ScaleDomain::Auto { quantile } => set(&o, "quantile", &f64::from(quantile).into()),
    }
    o.into()
}

fn pair_js(a: f64, b: f64) -> JsValue {
    let arr = Array::new();
    arr.push(&a.into());
    arr.push(&b.into());
    arr.into()
}

fn set_color(dst: &mut Option<Rgba>, v: Option<Option<Rgba>>) {
    if let Some(c) = v {
        *dst = c;
    }
}

fn mark_options(
    base: MarkOptions,
    o: &JsValue,
    theme: &ferrochart_core::theme::Theme,
) -> Result<MarkOptions, JsError> {
    let mut m = base;
    if absent(o) {
        return Ok(m);
    }
    if !o.is_object() {
        return Err(js_err("mark options must be an object"));
    }
    if let Some(a) = y_axis(o, "yAxis")? {
        m.y_axis = a;
    }
    if let Some(v) = opt_name(
        o,
        "orientation",
        names::orientation,
        "'vertical' or 'horizontal'",
    )? {
        m.orientation = v;
    }
    set_color(&mut m.color, opt_color(o, "color")?);
    set_color(&mut m.down_color, opt_color(o, "downColor")?);
    set_color(&mut m.total_color, opt_color(o, "totalColor")?);
    if let Some(a) = opt_f64(o, "fillAlpha")? {
        if !(0.0..=1.0).contains(&a) {
            return Err(bad("fillAlpha", "between 0 and 1"));
        }
        m.fill_alpha = a as f32;
    }
    if let Some(w) = opt_width(o, "lineWidth")? {
        m.line_width = w;
    }
    if let Some(d) = opt_dash(o, "lineStyle")? {
        m.line_style = d;
    }
    if let Some(b) = opt_bool(o, "stepLine")? {
        m.step_line = b;
    }
    if let Some(s) = opt_f64(o, "pointSize")? {
        if !(s.is_finite() && s >= 0.0) {
            return Err(bad("pointSize", "a number >= 0"));
        }
        m.point_size = Css(s as f32);
    }
    if let Some(i) = opt_name(
        o,
        "icon",
        names::icon,
        "'circle', 'square', 'arrowUp' or 'arrowDown'",
    )? {
        m.icon = i;
    }
    if let Some(b) = finite(o, "baseValue")? {
        m.base_value = b;
    }
    if let Some(g) = opt_id_number(o, "group")? {
        m.group = g;
    }
    if let Some(s) = opt_id_number(o, "stack")? {
        m.stack = s;
    }
    let band = get(o, "band");
    if band.is_null() {
        m.band = f32::NAN;
    } else if let Some(b) = opt_f64(o, "band")? {
        if !(b > 0.0 && b <= 1.0) {
            return Err(bad("band", "between 0 (exclusive) and 1, or null"));
        }
        m.band = b as f32;
    }
    let cs = get(o, "colorScale");
    if cs.is_null() {
        m.color_scale = None;
    } else if !cs.is_undefined() {
        m.color_scale = Some(color_scale(&cs, theme)?);
    }
    if let Some(b) = opt_bool(o, "showValues")? {
        m.show_values = b;
    }
    if let Some(f) = format(o, "valueFormat")? {
        m.value_format = f;
    }
    let cell = get(o, "cellSize");
    if cell.is_null() {
        m.cell_size = None;
    } else if !cell.is_undefined() {
        let (w, h) = pair(&cell, "cellSize")?;
        if !(w > 0.0 && h > 0.0) {
            return Err(bad("cellSize", "two positive numbers"));
        }
        m.cell_size = Some((w, h));
    }
    if let Some(b) = opt_bool(o, "hoverable")? {
        m.hoverable = b;
    }
    if let Some(b) = opt_bool(o, "visible")? {
        m.visible = b;
    }
    Ok(m)
}

fn mark_options_js(m: &MarkOptions) -> JsValue {
    let o = Object::new();
    set(&o, "yAxis", &names::axis_id_name(m.y_axis).into());
    set(
        &o,
        "orientation",
        &names::orientation_name(m.orientation).into(),
    );
    set(&o, "color", &color_js(m.color));
    set(&o, "downColor", &color_js(m.down_color));
    set(&o, "totalColor", &color_js(m.total_color));
    set(&o, "fillAlpha", &f64::from(m.fill_alpha).into());
    set(&o, "lineWidth", &f64::from(m.line_width.0).into());
    set(
        &o,
        "lineStyle",
        &crate::params::line_dash_name(m.line_style).into(),
    );
    set(&o, "stepLine", &m.step_line.into());
    set(&o, "pointSize", &f64::from(m.point_size.0).into());
    set(&o, "icon", &names::icon_name(m.icon).into());
    set(&o, "baseValue", &m.base_value.into());
    let id = |v: Option<u32>| v.map_or(JsValue::NULL, |v| f64::from(v).into());
    set(&o, "group", &id(m.group));
    set(&o, "stack", &id(m.stack));
    let band = if m.band.is_nan() {
        JsValue::NULL
    } else {
        f64::from(m.band).into()
    };
    set(&o, "band", &band);
    set(
        &o,
        "colorScale",
        &m.color_scale.as_ref().map_or(JsValue::NULL, color_scale_js),
    );
    set(&o, "showValues", &m.show_values.into());
    set(&o, "valueFormat", &format_js(m.value_format));
    set(
        &o,
        "cellSize",
        &m.cell_size.map_or(JsValue::NULL, |(w, h)| pair_js(w, h)),
    );
    set(&o, "hoverable", &m.hoverable.into());
    set(&o, "visible", &m.visible.into());
    o.into()
}

fn axis_options(base: AxisOptions, o: &JsValue) -> Result<AxisOptions, JsError> {
    let mut a = base.clone();
    if !o.is_object() {
        return Err(js_err("axis options must be an object"));
    }
    if let Some(k) = opt_name(o, "type", names::axis_kind, "'linear', 'log' or 'category'")?
        && k != a.kind
    {
        // A new kind starts from its own defaults (grid, margins).
        a = match k {
            ferrochart_plot::AxisKind::Linear => AxisOptions::linear(),
            ferrochart_plot::AxisKind::Log => AxisOptions::log(),
            ferrochart_plot::AxisKind::Category => AxisOptions::category(),
        };
        a.visible = base.visible;
        a.title = base.title;
        a.format = base.format;
        a.inverted = base.inverted;
        a.locked = base.locked;
    }
    let range = get(o, "range");
    if range.as_string().as_deref() == Some("auto") {
        if matches!(a.range, AxisRange::Fixed(..)) {
            a.range = match a.kind {
                ferrochart_plot::AxisKind::Category => AxisOptions::category().range,
                _ => AxisRange::default(),
            };
        }
    } else if !absent(&range) {
        let (lo, hi) = pair(&range, "range")?;
        a.range = AxisRange::Fixed(lo, hi);
    }
    if let AxisRange::Auto {
        include_zero,
        margin,
    } = &mut a.range
    {
        if let Some(z) = opt_bool(o, "includeZero")? {
            *include_zero = z;
        }
        if let Some(m) = opt_f64(o, "margin")? {
            if !(0.0..=1.0).contains(&m) {
                return Err(bad("margin", "between 0 and 1"));
            }
            *margin = m as f32;
        }
    }
    if let Some(f) = format(o, "format")? {
        a.format = f;
    }
    if let Some(b) = opt_bool(o, "inverted")? {
        a.inverted = b;
    }
    if let Some(b) = opt_bool(o, "visible")? {
        a.visible = b;
    }
    if let Some(b) = opt_bool(o, "grid")? {
        a.grid = b;
    }
    let title = get(o, "title");
    if title.is_null() {
        a.title = None;
    } else if let Some(t) = opt_string(o, "title")? {
        a.title = Some(t);
    }
    if let Some(b) = opt_bool(o, "locked")? {
        a.locked = b;
    }
    Ok(a)
}

fn axis_options_js(a: &AxisOptions) -> JsValue {
    let o = Object::new();
    set(&o, "type", &names::axis_kind_name(a.kind).into());
    match a.range {
        AxisRange::Auto {
            include_zero,
            margin,
        } => {
            set(&o, "range", &"auto".into());
            set(&o, "includeZero", &include_zero.into());
            set(&o, "margin", &f64::from(margin).into());
        }
        AxisRange::Fixed(lo, hi) => set(&o, "range", &pair_js(lo, hi)),
    }
    set(&o, "format", &format_js(a.format));
    set(&o, "inverted", &a.inverted.into());
    set(&o, "visible", &a.visible.into());
    set(&o, "grid", &a.grid.into());
    set(
        &o,
        "title",
        &a.title.as_deref().map_or(JsValue::NULL, JsValue::from_str),
    );
    set(&o, "locked", &a.locked.into());
    o.into()
}

fn rule_options(base: RuleOptions, o: &JsValue) -> Result<RuleOptions, JsError> {
    let mut r = base;
    if !o.is_object() {
        return Err(js_err("rule options must be an object"));
    }
    if let Some(a) = opt_name(o, "axis", names::axis_id, "'x', 'left' or 'right'")? {
        r.axis = a;
    }
    if let Some(v) = finite(o, "value")? {
        r.value = v;
    }
    let to = get(o, "to");
    if to.is_null() {
        r.to = None;
    } else if let Some(t) = finite(o, "to")? {
        r.to = Some(t);
    }
    set_color(&mut r.color, opt_color(o, "color")?);
    if let Some(w) = opt_width(o, "lineWidth")? {
        r.line_width = w;
    }
    if let Some(d) = opt_dash(o, "lineStyle")? {
        r.line_style = d;
    }
    let label = get(o, "label");
    if label.is_null() {
        r.label = None;
    } else if let Some(t) = opt_string(o, "label")? {
        r.label = Some(t);
    }
    if let Some(b) = opt_bool(o, "labelBackground")? {
        r.label_bg = b;
    }
    Ok(r)
}

fn label_options(o: &JsValue) -> Result<LabelOptions, JsError> {
    if !o.is_object() {
        return Err(js_err("label options must be an object"));
    }
    let mut l = LabelOptions {
        x: finite(o, "x")?.ok_or_else(|| js_err("missing 'x'"))?,
        y: finite(o, "y")?.ok_or_else(|| js_err("missing 'y'"))?,
        ..LabelOptions::default()
    };
    if let Some(a) = y_axis(o, "yAxis")? {
        l.y_axis = a;
    }
    if let Some(t) = opt_string(o, "text")? {
        l.text = t;
    }
    l.icon = opt_name(
        o,
        "icon",
        names::icon,
        "'circle', 'square', 'arrowUp', 'arrowDown' or null",
    )?;
    set_color(&mut l.color, opt_color(o, "color")?);
    if let Some(a) = opt_name(o, "align", names::text_align, "'left', 'center' or 'right'")? {
        l.align = a;
    }
    let off = get(o, "offset");
    if !absent(&off) {
        let (dx, dy) = pair(&off, "offset")?;
        l.offset = (Css(dx as f32), Css(dy as f32));
    }
    Ok(l)
}

/// Columns of `setData` / `append`.
fn mark_data(d: &JsValue) -> Result<MarkData, JsError> {
    if !d.is_object() {
        return Err(js_err("data must be an object of columns"));
    }
    let col = |k: &str, required: bool| f64_column(d, k, required);
    let mut m = MarkData::xy(
        col("x", true)?.unwrap_or_default(),
        col("y", true)?.unwrap_or_default(),
    );
    m.y0 = col("y0", false)?;
    m.value = col("value", false)?;
    let size = get(d, "size");
    if let Some(a) = size.dyn_ref::<Float32Array>() {
        m.size = Some(a.to_vec());
    } else if !absent(&size) {
        m.size = col("size", false)?.map(|v| v.into_iter().map(|s| s as f32).collect());
    }
    m.color = color_column(d)?;
    let strings = |k: &str| -> Result<Option<Vec<JsValue>>, JsError> {
        let v = get(d, k);
        if absent(&v) {
            return Ok(None);
        }
        if !Array::is_array(&v) {
            return Err(js_err(format!("column '{k}' must be an array of strings")));
        }
        Ok(Some(v.unchecked_ref::<Array>().to_vec()))
    };
    if let Some(steps) = strings("step")? {
        m.step = Some(
            steps
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    s.as_string()
                        .and_then(|s| names::waterfall_step(&s))
                        .ok_or_else(|| {
                            js_err(format!("step[{i}] must be 'delta', 'subtotal' or 'total'"))
                        })
                })
                .collect::<Result<_, _>>()?,
        );
    }
    if let Some(labels) = strings("label")? {
        m.label = Some(
            labels
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    if absent(s) {
                        Ok(String::new())
                    } else {
                        s.as_string()
                            .ok_or_else(|| js_err(format!("label[{i}] must be a string or null")))
                    }
                })
                .collect::<Result<_, _>>()?,
        );
    }
    Ok(m)
}

fn mark_point(p: &JsValue) -> Result<MarkPoint, JsError> {
    if !p.is_object() {
        return Err(js_err("the point must be an object"));
    }
    let mut m = MarkPoint::xy(
        opt_f64(p, "x")?.ok_or_else(|| js_err("missing 'x'"))?,
        opt_f64(p, "y")?.ok_or_else(|| js_err("missing 'y'"))?,
    );
    if let Some(v) = opt_f64(p, "y0")? {
        m.y0 = v;
    }
    if let Some(v) = opt_f64(p, "value")? {
        m.value = v;
    }
    if let Some(v) = opt_f64(p, "size")? {
        m.size = v as f32;
    }
    if let Some(Some(c)) = opt_color(p, "color")? {
        m.color = u32::from_be_bytes([c.r, c.g, c.b, c.a]);
    }
    Ok(m)
}

fn core_err(e: ferrochart_core::Error) -> JsError {
    js_err(e)
}

// ---- JS classes ------------------------------------------------------

/// A plot with numeric, category or date axes on a `<canvas>`: lines,
/// areas, scatter, bars, waterfalls and heatmaps.
#[wasm_bindgen]
pub struct FerroPlot {
    pub(crate) host: Rc<Host<PlotScene>>,
}

#[wasm_bindgen]
impl FerroPlot {
    /// Creates a plot drawing into `canvas`, which is sized by CSS (the
    /// backing store follows its CSS size × `devicePixelRatio`).
    ///
    /// @param {HTMLCanvasElement} canvas
    /// @param {PlotCreateOptions} [options]
    /// @returns {Promise<FerroPlot>}
    pub async fn create(
        canvas: HtmlCanvasElement,
        #[wasm_bindgen(unchecked_optional_param_type = "PlotCreateOptions")] options: Option<
            JsValue,
        >,
    ) -> Result<FerroPlot, JsError> {
        let options = options.unwrap_or(JsValue::UNDEFINED);
        let defaults = PlotOptions::default();
        let crosshair = opt_bool(&options, "crosshair")?.unwrap_or(defaults.crosshair);
        let hover_highlight =
            opt_bool(&options, "hoverHighlight")?.unwrap_or(defaults.hover_highlight);
        let surface = host::open(&canvas, &options).await?;
        let plot = Plot::new(PlotOptions {
            theme: surface.theme,
            crosshair,
            hover_highlight,
            ..defaults
        });
        let host = Host::start(
            surface,
            &canvas,
            PlotScene {
                plot,
                link: None,
                rules: Vec::new(),
            },
        )?;
        Ok(FerroPlot { host })
    }

    /// The GPU API in use: `'webgpu'` or `'webgl2'`.
    #[wasm_bindgen(getter, unchecked_return_type = "'webgpu' | 'webgl2'")]
    pub fn renderer(&self) -> String {
        self.host.backend.to_owned()
    }

    fn with_plot<T>(&self, f: impl FnOnce(&mut Plot) -> T) -> Result<T, JsError> {
        self.host.with_scene(|s| f(&mut s.plot))
    }

    /// Adds a mark drawn above the earlier ones.
    ///
    /// @param {MarkKind} kind
    /// @param {MarkOptions} [options]
    /// @returns {Mark}
    #[wasm_bindgen(js_name = addMark)]
    pub fn add_mark(
        &self,
        #[wasm_bindgen(unchecked_param_type = "MarkKind")] kind: &str,
        #[wasm_bindgen(unchecked_optional_param_type = "MarkOptions")] options: Option<JsValue>,
    ) -> Result<Mark, JsError> {
        let kind: MarkKind = names::mark_kind(kind).ok_or_else(|| {
            js_err(format!(
                "unknown mark kind '{kind}' (expected 'line', 'area', 'scatter', 'bars', 'waterfall' or 'cells')"
            ))
        })?;
        let theme = self
            .host
            .state
            .try_borrow()
            .map(|s| s.theme)
            .map_err(|_| js_err("FerroPlot is busy"))?;
        let opts = mark_options(
            MarkOptions::default(),
            &options.unwrap_or(JsValue::UNDEFINED),
            &theme,
        )?;
        let id = self
            .with_plot(|p| p.add_mark(kind, opts))?
            .map_err(core_err)?;
        Ok(Mark {
            plot: Rc::downgrade(&self.host),
            id,
        })
    }

    /// An axis: `'x'`, `'left'` (hidden by default) or `'right'`.
    ///
    /// @param {AxisName} name
    /// @returns {Axis}
    pub fn axis(
        &self,
        #[wasm_bindgen(unchecked_param_type = "AxisName")] name: &str,
    ) -> Result<Axis, JsError> {
        let id = names::axis_id(name).ok_or_else(|| {
            js_err(format!(
                "unknown axis '{name}' (expected 'x', 'left' or 'right')"
            ))
        })?;
        Ok(Axis {
            plot: Rc::downgrade(&self.host),
            id,
        })
    }

    /// Adds a line across the plot at a value, or a shaded band.
    ///
    /// @param {RuleOptions} options
    /// @returns {Rule}
    #[wasm_bindgen(js_name = addRule)]
    pub fn add_rule(
        &self,
        #[wasm_bindgen(unchecked_param_type = "RuleOptions")] options: &JsValue,
    ) -> Result<Rule, JsError> {
        let opts = rule_options(RuleOptions::default(), options)?;
        let id = self.host.with_scene(|s| {
            let id = s.plot.add_rule(opts.clone()).map_err(core_err)?;
            s.rules.push((id, opts));
            Ok::<_, JsError>(id)
        })??;
        Ok(Rule {
            plot: Rc::downgrade(&self.host),
            id,
        })
    }

    /// Adds a text or symbol at data coordinates.
    ///
    /// @param {LabelOptions} options
    /// @returns {Label}
    #[wasm_bindgen(js_name = addLabel)]
    pub fn add_label(
        &self,
        #[wasm_bindgen(unchecked_param_type = "LabelOptions")] options: &JsValue,
    ) -> Result<Label, JsError> {
        let opts = label_options(options)?;
        let id = self.with_plot(|p| p.add_label(opts))?.map_err(core_err)?;
        Ok(Label {
            plot: Rc::downgrade(&self.host),
            id,
        })
    }

    /// Shows all data: automatic axes follow the data again.
    pub fn fit(&self) -> Result<(), JsError> {
        self.with_plot(Plot::fit)
    }

    /// Subscribes to an event (typed in the `FerroPlot` interface above).
    #[wasm_bindgen(skip_typescript)]
    pub fn on(
        &self,
        #[wasm_bindgen(unchecked_param_type = "keyof PlotEventMap")] event: &str,
        #[wasm_bindgen(unchecked_param_type = "(event: any) => void")] callback: Function,
    ) -> Result<(), JsError> {
        self.host.listen(event, callback)
    }

    /// Removes a callback added with `on` (typed in the interface above).
    #[wasm_bindgen(skip_typescript)]
    pub fn off(
        &self,
        #[wasm_bindgen(unchecked_param_type = "keyof PlotEventMap")] event: &str,
        #[wasm_bindgen(unchecked_param_type = "(event: any) => void")] callback: &Function,
    ) -> Result<(), JsError> {
        self.host.unlisten(event, callback)
    }

    /// Intro animation, as `FerroChart.playIntro`: empty plot area and axes
    /// at once, then the marks drawn from left to right; rule labels and the
    /// hover appear at the end.
    ///
    /// @param {IntroOptions} [options]
    /// @returns {Promise<void>}
    #[wasm_bindgen(js_name = playIntro, unchecked_return_type = "Promise<void>")]
    pub fn play_intro(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "IntroOptions")] options: Option<JsValue>,
    ) -> Result<js_sys::Promise, JsError> {
        crate::intro::play(&self.host, &options.unwrap_or(JsValue::UNDEFINED))
    }

    /// Ends the intro animation now, showing everything.
    #[wasm_bindgen(js_name = finishIntro)]
    pub fn finish_intro(&self) {
        crate::intro::finish(&self.host);
    }

    /// Shows or removes the attribution logo (see `attributionLogo`).
    #[wasm_bindgen(js_name = setAttributionLogo)]
    pub fn set_attribution_logo(&self, on: bool) {
        self.host.set_logo(on);
    }

    /// Removes all listeners and releases the GPU resources. The object can
    /// no longer be used. (The same as `FerroChart.destroy`.)
    pub fn destroy(self) {
        self.dispose();
    }

    /// The former name of `destroy`, kept for existing code.
    pub fn dispose(self) {
        if let Ok(mut s) = self.host.state.try_borrow_mut() {
            s.scene.link = None;
        }
        self.host.shut_down();
    }
}

macro_rules! handle {
    ($name:ident, $id:ty, $doc:literal) => {
        #[doc = $doc]
        #[wasm_bindgen]
        pub struct $name {
            plot: Weak<Host<PlotScene>>,
            id: $id,
        }

        impl $name {
            #[allow(dead_code)] // `Rule` goes through the scene's copy of its options
            fn with_plot<T>(&self, f: impl FnOnce(&mut Plot) -> T) -> Result<T, JsError> {
                host::with_weak(&self.plot, |s| f(&mut s.plot))
            }
        }
    };
}

handle!(Mark, MarkId, "A mark of a [`FerroPlot`].");
handle!(Axis, AxisId, "An axis of a [`FerroPlot`].");
handle!(Rule, RuleId, "A rule of a [`FerroPlot`].");
handle!(Label, LabelId, "A label of a [`FerroPlot`].");

#[wasm_bindgen]
impl Mark {
    /// The mark's id, unique in its plot (hits of the same mark have the
    /// same id).
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> u32 {
        self.id.0
    }

    /// The kind given to `addMark`.
    #[wasm_bindgen(getter, unchecked_return_type = "MarkKind")]
    pub fn kind(&self) -> Result<String, JsError> {
        self.with_plot(|p| p.mark_kind(self.id))?
            .map(|k| names::mark_kind_name(k).to_owned())
            .ok_or_else(|| js_err("the mark was removed"))
    }

    /// Replaces all data. Typed arrays are copied in one `memcpy` per column.
    /// Invalid data (columns of different lengths, non-finite `x`, category
    /// numbers that are not whole numbers) is rejected and the old data kept.
    ///
    /// @param {MarkData} data
    #[wasm_bindgen(js_name = setData)]
    pub fn set_data(
        &self,
        #[wasm_bindgen(unchecked_param_type = "MarkData")] data: &JsValue,
    ) -> Result<(), JsError> {
        let d = mark_data(data)?;
        self.with_plot(|p| p.set_data(self.id, d))?
            .map_err(core_err)
    }

    /// Appends points (the same columns as the existing data).
    ///
    /// @param {MarkData} data
    pub fn append(
        &self,
        #[wasm_bindgen(unchecked_param_type = "MarkData")] data: &JsValue,
    ) -> Result<(), JsError> {
        let d = mark_data(data)?;
        self.with_plot(|p| p.append(self.id, d))?.map_err(core_err)
    }

    /// Replaces the last point (a live value); does not allocate.
    ///
    /// @param {MarkPoint} point
    pub fn update(
        &self,
        #[wasm_bindgen(unchecked_param_type = "MarkPoint")] point: &JsValue,
    ) -> Result<(), JsError> {
        let pt = mark_point(point)?;
        self.with_plot(|p| p.update_last(self.id, pt))?
            .map_err(core_err)
    }

    /// Changes options; absent keys keep their value.
    ///
    /// @param {MarkOptions} options
    #[wasm_bindgen(js_name = applyOptions)]
    pub fn apply_options(
        &self,
        #[wasm_bindgen(unchecked_param_type = "MarkOptions")] options: &JsValue,
    ) -> Result<(), JsError> {
        let host = self
            .plot
            .upgrade()
            .ok_or_else(|| js_err("the FerroPlot was destroyed"))?;
        let theme = host
            .state
            .try_borrow()
            .map(|s| s.theme)
            .map_err(|_| js_err("FerroPlot is busy"))?;
        let base = self
            .with_plot(|p| p.mark_options(self.id).cloned())?
            .ok_or_else(|| js_err("the mark was removed"))?;
        let opts = mark_options(base, options, &theme)?;
        self.with_plot(|p| p.set_mark_options(self.id, opts))?
            .map_err(core_err)
    }

    /// The current options (every key).
    #[wasm_bindgen(unchecked_return_type = "Required<MarkOptions>")]
    pub fn options(&self) -> Result<JsValue, JsError> {
        self.with_plot(|p| p.mark_options(self.id).map(mark_options_js))?
            .ok_or_else(|| js_err("the mark was removed"))
    }

    /// Removes the mark from the plot.
    pub fn remove(&self) -> Result<(), JsError> {
        self.with_plot(|p| p.remove_mark(self.id))?
            .map_err(core_err)
    }
}

#[wasm_bindgen]
impl Axis {
    /// Changes options; absent keys keep their value. A new `type` starts
    /// from that type's defaults (grid, margins).
    ///
    /// @param {AxisOptions} options
    #[wasm_bindgen(js_name = applyOptions)]
    pub fn apply_options(
        &self,
        #[wasm_bindgen(unchecked_param_type = "AxisOptions")] options: &JsValue,
    ) -> Result<(), JsError> {
        let base = self.with_plot(|p| p.axis(self.id).clone())?;
        let opts = axis_options(base, options)?;
        self.with_plot(|p| p.set_axis(self.id, opts))?
            .map_err(core_err)
    }

    /// The current options (every key).
    #[wasm_bindgen(unchecked_return_type = "Required<AxisOptions>")]
    pub fn options(&self) -> Result<JsValue, JsError> {
        self.with_plot(|p| axis_options_js(p.axis(self.id)))
    }

    /// Labels of a category axis: category `k` is `labels[k]`.
    ///
    /// @param {string[]} labels
    #[wasm_bindgen(js_name = setCategories)]
    #[allow(clippy::needless_pass_by_value)] // wasm-bindgen passes arrays by value
    pub fn set_categories(&self, labels: Vec<String>) -> Result<(), JsError> {
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        self.with_plot(|p| p.set_categories(self.id, &refs))?
            .map_err(core_err)
    }

    /// The visible range `[lo, hi]` as of the last frame.
    #[wasm_bindgen(unchecked_return_type = "[number, number]")]
    pub fn view(&self) -> Result<JsValue, JsError> {
        self.with_plot(|p| {
            let (lo, hi) = p.view(self.id);
            pair_js(lo, hi)
        })
    }

    /// Shows `lo..hi` (the axis stops following the data until `fit()`).
    /// Sends no `viewChange` event.
    #[wasm_bindgen(js_name = setView)]
    pub fn set_view(&self, lo: f64, hi: f64) -> Result<(), JsError> {
        self.with_plot(|p| p.set_view(self.id, lo, hi))?
            .map_err(core_err)
    }
}

#[wasm_bindgen]
impl Rule {
    /// Changes options; absent keys keep their value.
    ///
    /// @param {RuleOptions} options
    #[wasm_bindgen(js_name = applyOptions)]
    pub fn apply_options(
        &self,
        #[wasm_bindgen(unchecked_param_type = "RuleOptions")] options: &JsValue,
    ) -> Result<(), JsError> {
        host::with_weak(&self.plot, |s| {
            let k = s
                .rules
                .iter()
                .position(|r| r.0 == self.id)
                .ok_or_else(|| js_err("the rule was removed"))?;
            let opts = rule_options(s.rules[k].1.clone(), options)?;
            s.plot
                .update_rule(self.id, opts.clone())
                .map_err(core_err)?;
            s.rules[k].1 = opts;
            Ok(())
        })?
    }

    /// Removes the rule.
    pub fn remove(&self) -> Result<(), JsError> {
        host::with_weak(&self.plot, |s| {
            s.rules.retain(|r| r.0 != self.id);
            s.plot.remove_rule(self.id).map_err(core_err)
        })?
    }
}

#[wasm_bindgen]
impl Label {
    /// Removes the label.
    pub fn remove(&self) -> Result<(), JsError> {
        self.with_plot(|p| p.remove_label(self.id))?
            .map_err(core_err)
    }
}

#[wasm_bindgen]
impl FerroChart {
    /// Lines `plot` up with a pane (default: the main one): every frame of
    /// the chart sets the plot's right axis to the prices at the plot area's
    /// top and bottom, as the pane maps them, so a price is at the same
    /// height in both (the canvases' page positions are read each frame).
    /// The chart is the only source: the right axis is locked, and set to a
    /// log axis when the pane's scale is logarithmic. Typical use: a volume
    /// profile or order book depth as horizontal bars beside the candles.
    /// The plot's attribution logo is removed (the chart's stands for both).
    ///
    /// @param {FerroPlot} plot
    /// @param {number} [pane]
    #[wasm_bindgen(js_name = attachSidePlot)]
    pub fn attach_side_plot(&self, plot: &FerroPlot, pane: Option<u32>) -> Result<(), JsError> {
        let (pane, log) = self
            .with_chart(|c| {
                let id = pane.map_or_else(|| c.main_pane(), PaneId);
                let log = c
                    .price_scale(id, ScaleSide::Right)
                    .map(|s| s.mode() == PriceScaleMode::Logarithmic);
                log.map(|l| (id, l))
            })?
            .ok_or_else(|| js_err("unknown pane"))?;
        plot.host.with_scene(|s| {
            let mut a = s.plot.axis(AxisId::Right).clone();
            a.locked = true;
            if log && a.kind != ferrochart_plot::AxisKind::Log {
                a.kind = ferrochart_plot::AxisKind::Log;
            } else if !log && a.kind == ferrochart_plot::AxisKind::Log {
                a.kind = ferrochart_plot::AxisKind::Linear;
            }
            s.link = Some((Rc::downgrade(&self.shared), pane));
            s.plot.set_axis(AxisId::Right, a).map_err(core_err)
        })??;
        // One logo for the pair: the chart's.
        plot.host.set_logo(false);
        let weak = Rc::downgrade(&plot.host);
        self.shared.with_scene(|s| {
            s.side_plots
                .retain(|p| p.strong_count() > 0 && !p.ptr_eq(&weak));
            s.side_plots.push(weak);
        })?;
        Ok(())
    }

    /// Stops lining `plot` up with this chart (its right axis stays locked).
    ///
    /// @param {FerroPlot} plot
    #[wasm_bindgen(js_name = detachSidePlot)]
    pub fn detach_side_plot(&self, plot: &FerroPlot) -> Result<(), JsError> {
        let weak = Rc::downgrade(&plot.host);
        self.shared
            .with_scene(|s| s.side_plots.retain(|p| !p.ptr_eq(&weak)))?;
        plot.host.with_scene(|s| s.link = None)
    }
}
