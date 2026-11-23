//! JS objects ↔ chart types: series and price-line
//! options, markers, drawings, sync messages and saved states. Absent keys
//! keep the current (or default) value; wrong types are errors.
// `Option<Option<_>>`: absent (keep) vs `null` (theme default) vs a color.
#![allow(clippy::option_option)]

use ferrochart_core::chart::{BandOptions, Marker, PriceLineOptions, SeriesOptions};
use ferrochart_core::drawing::{Anchor, DrawingData, DrawingStyle, Param};
use ferrochart_core::frame::LineDash;
use ferrochart_core::geom::{Css, Rgba};
use ferrochart_core::state::{ChartState, PaneState, PriceScaleState, SavedDrawing};
use ferrochart_core::sync::{DrawingSyncOp, SyncKey, SyncMessage, SyncOptions};
use js_sys::{Array, Object, Reflect, Uint32Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::params;
use crate::web::{js_err, set};

/// `o[k]`, `undefined` when `o` is not an object.
pub(crate) fn get(o: &JsValue, k: &str) -> JsValue {
    if !o.is_object() {
        return JsValue::UNDEFINED;
    }
    Reflect::get(o, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
}

pub(crate) fn absent(v: &JsValue) -> bool {
    v.is_undefined() || v.is_null()
}

pub(crate) fn opt_f64(o: &JsValue, k: &str) -> Result<Option<f64>, JsError> {
    let v = get(o, k);
    if absent(&v) {
        return Ok(None);
    }
    v.as_f64()
        .map(Some)
        .ok_or_else(|| js_err(format!("'{k}' must be a number")))
}

pub(crate) fn opt_bool(o: &JsValue, k: &str) -> Result<Option<bool>, JsError> {
    let v = get(o, k);
    if absent(&v) {
        return Ok(None);
    }
    v.as_bool()
        .map(Some)
        .ok_or_else(|| js_err(format!("'{k}' must be a boolean")))
}

pub(crate) fn opt_string(o: &JsValue, k: &str) -> Result<Option<String>, JsError> {
    let v = get(o, k);
    if absent(&v) {
        return Ok(None);
    }
    v.as_string()
        .map(Some)
        .ok_or_else(|| js_err(format!("'{k}' must be a string")))
}

pub(crate) fn parse_color(s: &str, k: &str) -> Result<Rgba, JsError> {
    params::color(s).ok_or_else(|| js_err(format!("'{k}': invalid color '{s}'")))
}

/// `undefined` → keep (`None`); `null` → theme default (`Some(None)`); a CSS
/// color string → `Some(Some(c))`.
pub(crate) fn opt_color(o: &JsValue, k: &str) -> Result<Option<Option<Rgba>>, JsError> {
    let v = get(o, k);
    if v.is_undefined() {
        return Ok(None);
    }
    if v.is_null() {
        return Ok(Some(None));
    }
    let s = v
        .as_string()
        .ok_or_else(|| js_err(format!("'{k}' must be a CSS color string")))?;
    Ok(Some(Some(parse_color(&s, k)?)))
}

/// A color that cannot be reset to a default.
fn opt_color_required(o: &JsValue, k: &str) -> Result<Option<Rgba>, JsError> {
    match opt_color(o, k)? {
        Some(None) => Err(js_err(format!("'{k}' must be a CSS color string"))),
        Some(c) => Ok(c),
        None => Ok(None),
    }
}

pub(crate) fn color_js(c: Option<Rgba>) -> JsValue {
    c.map_or(JsValue::NULL, |c| JsValue::from_str(&params::color_css(c)))
}

pub(crate) fn opt_dash(o: &JsValue, k: &str) -> Result<Option<LineDash>, JsError> {
    opt_string(o, k)?
        .map(|s| {
            params::line_dash(&s)
                .ok_or_else(|| js_err(format!("'{k}' must be 'solid', 'dashed' or 'dotted'")))
        })
        .transpose()
}

pub(crate) fn opt_width(o: &JsValue, k: &str) -> Result<Option<Css>, JsError> {
    match opt_f64(o, k)? {
        Some(w) if w.is_finite() && w > 0.0 => Ok(Some(Css(w as f32))),
        Some(_) => Err(js_err(format!("'{k}' must be positive"))),
        None => Ok(None),
    }
}

/// A time in UTC ms: an integer number, a bigint or a `Date`.
pub(crate) fn time_ms(v: &JsValue, what: &str) -> Result<i64, JsError> {
    if v.is_bigint() {
        return i64::try_from(v.clone()).map_err(|_| js_err(format!("'{what}' is out of range")));
    }
    let f = match v.dyn_ref::<js_sys::Date>() {
        Some(d) => d.get_time(),
        None => v
            .as_f64()
            .ok_or_else(|| js_err(format!("'{what}' must be a number, bigint or Date")))?,
    };
    crate::convert::times_from_f64(&[f])
        .ok()
        .and_then(|t| t.first().copied())
        .ok_or_else(|| js_err(format!("'{what}' is not an integer millisecond timestamp")))
}

/// The optional `color` column of value data: CSS strings (`null` for the
/// series color) or a `Uint32Array` of `0xRRGGBBAA`. Only histograms draw
/// them, and only at full detail (LOD level 0).
pub(crate) fn color_column(data: &JsValue) -> Result<Option<Vec<u32>>, JsError> {
    let v = get(data, "color");
    if absent(&v) {
        return Ok(None);
    }
    if let Some(a) = v.dyn_ref::<Uint32Array>() {
        return Ok(Some(a.to_vec()));
    }
    if !Array::is_array(&v) {
        return Err(js_err(
            "column 'color' must be a Uint32Array or an array of CSS colors",
        ));
    }
    let arr: Array = v.unchecked_into();
    let mut out = Vec::with_capacity(arr.length() as usize);
    for (i, c) in arr.iter().enumerate() {
        if absent(&c) {
            out.push(0);
            continue;
        }
        let s = c
            .as_string()
            .ok_or_else(|| js_err(format!("color[{i}] must be a CSS color string")))?;
        let c = parse_color(&s, "color")?;
        out.push(u32::from_be_bytes([c.r, c.g, c.b, c.a]));
    }
    Ok(Some(out))
}

/// `base` with the keys of `o` applied.
pub(crate) fn series_options(base: SeriesOptions, o: &JsValue) -> Result<SeriesOptions, JsError> {
    let mut opts = base;
    if absent(o) {
        return Ok(opts);
    }
    if !o.is_object() {
        return Err(js_err("series options must be an object"));
    }
    if let Some(c) = opt_color(o, "color")? {
        opts.color = c;
    }
    if let Some(c) = opt_color(o, "downColor")? {
        opts.down_color = c;
    }
    if let Some(c) = opt_color(o, "topFill")? {
        opts.top_fill = c;
    }
    if let Some(c) = opt_color(o, "bottomFill")? {
        opts.bottom_fill = c;
    }
    if let Some(w) = opt_width(o, "lineWidth")? {
        opts.line_width = w;
    }
    if let Some(d) = opt_dash(o, "lineStyle")? {
        opts.line_style = d;
    }
    if let Some(b) = opt_bool(o, "step")? {
        opts.step = b;
    }
    if let Some(v) = opt_f64(o, "baseValue")? {
        if !v.is_finite() {
            return Err(js_err("'baseValue' must be finite"));
        }
        opts.base_value = v;
    }
    if let Some(b) = opt_bool(o, "lastValueVisible")? {
        opts.last_value_visible = b;
    }
    if let Some(b) = opt_bool(o, "priceLineVisible")? {
        opts.price_line_visible = b;
    }
    if let Some(v) = opt_f64(o, "shift")? {
        if v.fract() != 0.0 || v.abs() > 1e6 {
            return Err(js_err("'shift' must be an integer"));
        }
        opts.shift = v as i32;
    }
    if get(o, "pricePrecision").is_null() {
        opts.price_precision = None; // automatic
    } else if let Some(v) = opt_f64(o, "pricePrecision")? {
        if !(0.0..=12.0).contains(&v) || v.fract() != 0.0 {
            return Err(js_err(
                "'pricePrecision' must be an integer in 0..=12, or null for automatic",
            ));
        }
        opts.price_precision = Some(v as u8);
    }
    if get(o, "markerStackLimit").is_null() {
        opts.marker_stack_limit = None; // no folding
    } else if let Some(v) = opt_f64(o, "markerStackLimit")? {
        if !(1.0..=f64::from(u16::MAX)).contains(&v) || v.fract() != 0.0 {
            return Err(js_err(
                "'markerStackLimit' must be an integer >= 1, or null for no limit",
            ));
        }
        opts.marker_stack_limit = Some(v as u16);
    }
    if let Some(c) = opt_color(o, "filledBarColor")? {
        opts.filled_bar_color = c;
    }
    if get(o, "markerCluster").is_null() {
        opts.marker_cluster = None;
    } else if let Some(v) = opt_f64(o, "markerCluster")? {
        opts.marker_cluster = Some(Css(v as f32));
    }
    Ok(opts)
}

pub(crate) fn series_options_js(o: &SeriesOptions) -> JsValue {
    let js = Object::new();
    set(&js, "color", &color_js(o.color));
    set(&js, "downColor", &color_js(o.down_color));
    set(&js, "topFill", &color_js(o.top_fill));
    set(&js, "bottomFill", &color_js(o.bottom_fill));
    set(&js, "lineWidth", &f64::from(o.line_width.0).into());
    set(
        &js,
        "lineStyle",
        &JsValue::from_str(params::line_dash_name(o.line_style)),
    );
    set(&js, "step", &o.step.into());
    set(&js, "baseValue", &o.base_value.into());
    set(&js, "lastValueVisible", &o.last_value_visible.into());
    set(&js, "priceLineVisible", &o.price_line_visible.into());
    set(&js, "shift", &f64::from(o.shift).into());
    let precision = o
        .price_precision
        .map_or(JsValue::NULL, |p| f64::from(p).into());
    set(&js, "pricePrecision", &precision);
    let limit = o
        .marker_stack_limit
        .map_or(JsValue::NULL, |n| f64::from(n).into());
    set(&js, "markerStackLimit", &limit);
    let cluster = o
        .marker_cluster
        .map_or(JsValue::NULL, |d| f64::from(d.0).into());
    set(&js, "markerCluster", &cluster);
    set(&js, "filledBarColor", &color_js(o.filled_bar_color));
    js.into()
}

/// `{ fill?, fillCrossed? }` over `base`; `null` colors use the default.
pub(crate) fn band_options(base: BandOptions, o: &JsValue) -> Result<BandOptions, JsError> {
    if o.is_undefined() || o.is_null() {
        return Ok(base);
    }
    if !o.is_object() {
        return Err(js_err("band options must be an object"));
    }
    Ok(BandOptions {
        fill: opt_color(o, "fill")?.unwrap_or(base.fill),
        fill_crossed: opt_color(o, "fillCrossed")?.unwrap_or(base.fill_crossed),
    })
}

pub(crate) fn band_options_js(o: &BandOptions) -> JsValue {
    let obj = Object::new();
    set(&obj, "fill", &color_js(o.fill));
    set(&obj, "fillCrossed", &color_js(o.fill_crossed));
    obj.into()
}

pub(crate) fn price_line_options(
    base: PriceLineOptions,
    o: &JsValue,
) -> Result<PriceLineOptions, JsError> {
    let mut opts = base;
    if !o.is_object() {
        return Err(js_err("price line options must be an object"));
    }
    if let Some(p) = opt_f64(o, "price")? {
        if !p.is_finite() {
            return Err(js_err("'price' must be finite"));
        }
        opts.price = p;
    }
    if let Some(c) = opt_color(o, "color")? {
        opts.color = c;
    }
    if let Some(w) = opt_width(o, "lineWidth")? {
        opts.line_width = w;
    }
    if let Some(d) = opt_dash(o, "lineStyle")? {
        opts.line_style = d;
    }
    if let Some(b) = opt_bool(o, "axisLabelVisible")? {
        opts.axis_label_visible = b;
    }
    if let Some(t) = opt_string(o, "title")? {
        opts.title = t;
    }
    Ok(opts)
}

pub(crate) fn price_line_options_js(o: &PriceLineOptions) -> JsValue {
    let js = Object::new();
    set(&js, "price", &o.price.into());
    set(&js, "color", &color_js(o.color));
    set(&js, "lineWidth", &f64::from(o.line_width.0).into());
    set(
        &js,
        "lineStyle",
        &JsValue::from_str(params::line_dash_name(o.line_style)),
    );
    set(&js, "axisLabelVisible", &o.axis_label_visible.into());
    set(&js, "title", &JsValue::from_str(&o.title));
    js.into()
}

fn array(v: &JsValue, what: &str) -> Result<Array, JsError> {
    if !Array::is_array(v) {
        return Err(js_err(format!("{what} must be an array")));
    }
    Ok(v.clone().unchecked_into())
}

/// Markers from JS; `icon` names are looked up in chart `icon_key`'s table.
pub(crate) fn markers(v: &JsValue, icon_key: u32) -> Result<Vec<Marker>, JsError> {
    let arr = array(v, "markers")?;
    let mut out = Vec::with_capacity(arr.length() as usize);
    for m in arr.iter() {
        let mut marker = Marker {
            time: time_ms(&get(&m, "time"), "time")?,
            color: opt_color(&m, "color")?.flatten(),
            text: opt_string(&m, "text")?.unwrap_or_default(),
            label: opt_string(&m, "label")?.unwrap_or_default(),
            border: opt_color(&m, "border")?.flatten(),
            ..Marker::default()
        };
        if let Some(s) = opt_f64(&m, "size")? {
            marker.size = Some(Css(s as f32));
        }
        if let Some(w) = opt_f64(&m, "borderWidth")? {
            marker.border_width = Css(w as f32);
        }
        if let Some(name) = opt_string(&m, "icon")? {
            marker.icon = Some(crate::icons::lookup(icon_key, &name).ok_or_else(|| {
                js_err(format!(
                    "unknown icon '{name}' (register it with FerroChart.registerIcon)"
                ))
            })?);
        }
        if let Some(p) = opt_string(&m, "position")? {
            marker.position = params::marker_position(&p)
                .ok_or_else(|| js_err("'position' must be 'aboveBar', 'belowBar' or 'atPrice'"))?;
        }
        marker.price = opt_f64(&m, "price")?;
        if let Some(id) = opt_f64(&m, "id")? {
            if !(id >= 0.0 && id.fract() == 0.0 && id <= 9_007_199_254_740_991.0) {
                return Err(js_err("marker 'id' must be an integer from 0 to 2^53 - 1"));
            }
            marker.id = id as u64;
        }
        if let Some(s) = opt_string(&m, "shape")? {
            marker.shape = params::marker_shape(&s).ok_or_else(|| {
                js_err("'shape' must be 'circle', 'square', 'arrowUp' or 'arrowDown'")
            })?;
        }
        out.push(marker);
    }
    Ok(out)
}

/// Bar quality flags `[{ time, filled?, clipped? }]` (r22).
pub(crate) fn bar_flags(
    v: &JsValue,
) -> Result<Vec<(i64, ferrochart_core::chart::BarFlags)>, JsError> {
    use ferrochart_core::chart::BarFlags;
    let arr = array(v, "flags")?;
    let mut out = Vec::with_capacity(arr.length() as usize);
    for f in arr.iter() {
        let time = time_ms(&get(&f, "time"), "time")?;
        let mut flags = BarFlags::NONE;
        if opt_bool(&f, "filled")? == Some(true) {
            flags = flags | BarFlags::FILLED;
        }
        if opt_bool(&f, "clipped")? == Some(true) {
            flags = flags | BarFlags::CLIPPED;
        }
        out.push((time, flags));
    }
    Ok(out)
}

/// Markers for JS; icons by name (`null` for a removed image).
pub(crate) fn markers_js(markers: &[Marker], icon_key: u32) -> JsValue {
    use ferrochart_core::chart::{MarkerPosition, MarkerShape};
    markers
        .iter()
        .map(|m| {
            let o = Object::new();
            set(&o, "time", &JsValue::from_f64(m.time as f64));
            let position = match m.position {
                MarkerPosition::AboveBar => "aboveBar",
                MarkerPosition::BelowBar => "belowBar",
                MarkerPosition::AtPrice => "atPrice",
            };
            let shape = match m.shape {
                MarkerShape::Circle => "circle",
                MarkerShape::Square => "square",
                MarkerShape::ArrowUp => "arrowUp",
                MarkerShape::ArrowDown => "arrowDown",
            };
            set(&o, "position", &JsValue::from_str(position));
            set(&o, "shape", &JsValue::from_str(shape));
            set(&o, "color", &color_js(m.color));
            set(&o, "text", &JsValue::from_str(&m.text));
            set(
                &o,
                "size",
                &m.size.map_or(JsValue::NULL, |s| f64::from(s.0).into()),
            );
            let icon = m
                .icon
                .and_then(|id| crate::icons::name_of(icon_key, id))
                .map_or(JsValue::NULL, |n| JsValue::from_str(&n));
            set(&o, "icon", &icon);
            set(&o, "label", &JsValue::from_str(&m.label));
            set(&o, "border", &color_js(m.border));
            set(&o, "borderWidth", &f64::from(m.border_width.0).into());
            set(&o, "id", &JsValue::from_f64(m.id as f64));
            set(
                &o,
                "price",
                &m.price.map_or(JsValue::UNDEFINED, JsValue::from_f64),
            );
            JsValue::from(o)
        })
        .collect::<Array>()
        .into()
}

pub(crate) fn anchors(v: &JsValue) -> Result<Vec<Anchor>, JsError> {
    array(v, "anchors")?
        .iter()
        .map(|a| {
            let price = opt_f64(&a, "price")?.ok_or_else(|| js_err("anchor without 'price'"))?;
            Ok(Anchor {
                time: time_ms(&get(&a, "time"), "time")?,
                price,
            })
        })
        .collect()
}

fn anchors_js(anchors: &[Anchor]) -> JsValue {
    anchors
        .iter()
        .map(|a| {
            let o = Object::new();
            set(&o, "time", &JsValue::from_f64(a.time as f64));
            set(&o, "price", &a.price.into());
            JsValue::from(o)
        })
        .collect::<Array>()
        .into()
}

pub(crate) fn drawing_style(base: DrawingStyle, o: &JsValue) -> Result<DrawingStyle, JsError> {
    let mut s = base;
    if absent(o) {
        return Ok(s);
    }
    if !o.is_object() {
        return Err(js_err("drawing style must be an object"));
    }
    if let Some(c) = opt_color_required(o, "line")? {
        s.line = c;
    }
    if let Some(w) = opt_width(o, "lineWidth")? {
        s.line_width = w;
    }
    if let Some(d) = opt_dash(o, "lineDash")? {
        s.line_dash = d;
    }
    if let Some(c) = opt_color_required(o, "fill")? {
        s.fill = c;
    }
    if let Some(c) = opt_color_required(o, "text")? {
        s.text = c;
    }
    if let Some(w) = opt_width(o, "fontSize")? {
        s.font_size = w;
    }
    Ok(s)
}

pub(crate) fn drawing_style_js(s: &DrawingStyle) -> JsValue {
    let o = Object::new();
    set(&o, "line", &color_js(Some(s.line)));
    set(&o, "lineWidth", &f64::from(s.line_width.0).into());
    set(
        &o,
        "lineDash",
        &JsValue::from_str(params::line_dash_name(s.line_dash)),
    );
    set(&o, "fill", &color_js(Some(s.fill)));
    set(&o, "text", &color_js(Some(s.text)));
    set(&o, "fontSize", &f64::from(s.font_size.0).into());
    o.into()
}

pub(crate) fn param(v: &JsValue, key: &str) -> Result<Param, JsError> {
    if let Some(n) = v.as_f64() {
        return Ok(Param::Number(n));
    }
    if let Some(b) = v.as_bool() {
        return Ok(Param::Bool(b));
    }
    if let Some(s) = v.as_string() {
        return Ok(Param::Text(s));
    }
    if Array::is_array(v) {
        let arr: Array = v.clone().unchecked_into();
        return arr
            .iter()
            .map(|x| {
                x.as_f64()
                    .ok_or_else(|| js_err(format!("'{key}' must contain only numbers")))
            })
            .collect::<Result<_, _>>()
            .map(Param::Numbers);
    }
    Err(js_err(format!(
        "parameter '{key}' must be a number, boolean, string or array of numbers"
    )))
}

pub(crate) fn param_js(p: &Param) -> JsValue {
    match p {
        Param::Number(n) => (*n).into(),
        Param::Bool(b) => (*b).into(),
        Param::Text(s) => JsValue::from_str(s),
        Param::Numbers(v) => v
            .iter()
            .map(|&x| JsValue::from_f64(x))
            .collect::<Array>()
            .into(),
    }
}

pub(crate) fn drawing_data(o: &JsValue) -> Result<DrawingData, JsError> {
    if !o.is_object() {
        return Err(js_err(
            "drawing must be an object { kind, anchors, style?, params? }",
        ));
    }
    let kind = opt_string(o, "kind")?.ok_or_else(|| js_err("drawing without 'kind'"))?;
    let mut d = DrawingData::new(&kind, anchors(&get(o, "anchors"))?);
    d.style = drawing_style(d.style, &get(o, "style"))?;
    let ps = get(o, "params");
    if !absent(&ps) {
        if !ps.is_object() || Array::is_array(&ps) {
            return Err(js_err("'params' must be an object"));
        }
        let entries = Object::entries(ps.unchecked_ref());
        for e in entries.iter() {
            let pair: Array = e.unchecked_into();
            let key = pair.get(0).as_string().unwrap_or_default();
            let value = param(&pair.get(1), &key)?;
            d.set(&key, value);
        }
    }
    Ok(d)
}

pub(crate) fn drawing_data_js(d: &DrawingData) -> JsValue {
    let o = Object::new();
    set(&o, "kind", &JsValue::from_str(&d.kind));
    set(&o, "anchors", &anchors_js(&d.anchors));
    set(&o, "style", &drawing_style_js(&d.style));
    let ps = Object::new();
    for (k, v) in &d.params {
        set(&ps, k, &param_js(v));
    }
    set(&o, "params", &ps.into());
    o.into()
}

pub(crate) fn sync_options(o: &JsValue) -> Result<SyncOptions, JsError> {
    let mut opts = SyncOptions::default();
    if let Some(b) = opt_bool(o, "crosshair")? {
        opts.crosshair = b;
    }
    if let Some(b) = opt_bool(o, "visibleRange")? {
        opts.visible_range = b;
    }
    if let Some(b) = opt_bool(o, "drawings")? {
        opts.drawings = b;
    }
    Ok(opts)
}

pub(crate) fn sync_options_js(o: SyncOptions) -> JsValue {
    let js = Object::new();
    set(&js, "crosshair", &o.crosshair.into());
    set(&js, "visibleRange", &o.visible_range.into());
    set(&js, "drawings", &o.drawings.into());
    js.into()
}

/// Largest key a JS number holds exactly.
const MAX_SAFE: u64 = (1 << 53) - 1;

fn sync_key(o: &JsValue) -> Result<SyncKey, JsError> {
    let k = opt_f64(o, "key")?.ok_or_else(|| js_err("drawing sync op: missing 'key'"))?;
    if !(0.0..=MAX_SAFE as f64).contains(&k) || k.fract() != 0.0 {
        return Err(js_err("drawing sync op: invalid 'key'"));
    }
    Ok(SyncKey(k as u64))
}

fn req_bool(o: &JsValue, k: &str) -> Result<bool, JsError> {
    opt_bool(o, k)?.ok_or_else(|| js_err(format!("drawing sync op: missing '{k}'")))
}

pub(crate) fn drawing_sync_op(o: &JsValue) -> Result<DrawingSyncOp, JsError> {
    if !o.is_object() {
        return Err(js_err("a drawing sync op must be an object"));
    }
    let key = sync_key(o)?;
    let ty = opt_string(o, "type")?.unwrap_or_default();
    Ok(match ty.as_str() {
        "add" => DrawingSyncOp::Add {
            key,
            data: drawing_data(&get(o, "data"))?,
            visible: opt_bool(o, "visible")?.unwrap_or(true),
            locked: opt_bool(o, "locked")?.unwrap_or(false),
        },
        "update" => DrawingSyncOp::Update {
            key,
            data: drawing_data(&get(o, "data"))?,
        },
        "setVisible" => DrawingSyncOp::SetVisible {
            key,
            visible: req_bool(o, "visible")?,
        },
        "setLocked" => DrawingSyncOp::SetLocked {
            key,
            locked: req_bool(o, "locked")?,
        },
        "remove" => DrawingSyncOp::Remove { key },
        other => return Err(js_err(format!("unknown drawing sync op type '{other}'"))),
    })
}

pub(crate) fn drawing_sync_op_js(op: &DrawingSyncOp) -> JsValue {
    let o = Object::new();
    let ty = match op {
        DrawingSyncOp::Add {
            data,
            visible,
            locked,
            ..
        } => {
            set(&o, "data", &drawing_data_js(data));
            set(&o, "visible", &(*visible).into());
            set(&o, "locked", &(*locked).into());
            "add"
        }
        DrawingSyncOp::Update { data, .. } => {
            set(&o, "data", &drawing_data_js(data));
            "update"
        }
        DrawingSyncOp::SetVisible { visible, .. } => {
            set(&o, "visible", &(*visible).into());
            "setVisible"
        }
        DrawingSyncOp::SetLocked { locked, .. } => {
            set(&o, "locked", &(*locked).into());
            "setLocked"
        }
        DrawingSyncOp::Remove { .. } => "remove",
        _ => "unknown",
    };
    set(&o, "type", &ty.into());
    set(&o, "key", &JsValue::from_f64(op.key().0 as f64));
    o.into()
}

pub(crate) fn sync_message_js(m: SyncMessage) -> JsValue {
    let o = Object::new();
    match m {
        SyncMessage::Crosshair { time } => {
            set(&o, "type", &JsValue::from_str("crosshair"));
            set(
                &o,
                "time",
                &time.map_or(JsValue::NULL, |t| JsValue::from_f64(t as f64)),
            );
        }
        SyncMessage::VisibleRange { from, to } => {
            set(&o, "type", &JsValue::from_str("visibleRange"));
            set(&o, "from", &JsValue::from_f64(from as f64));
            set(&o, "to", &JsValue::from_f64(to as f64));
        }
        _ => set(&o, "type", &JsValue::from_str("unknown")),
    }
    o.into()
}

pub(crate) fn sync_message(o: &JsValue) -> Result<SyncMessage, JsError> {
    match opt_string(o, "type")?.as_deref() {
        Some("crosshair") => {
            let t = get(o, "time");
            let time = if absent(&t) {
                None
            } else {
                Some(time_ms(&t, "time")?)
            };
            Ok(SyncMessage::Crosshair { time })
        }
        Some("visibleRange") => Ok(SyncMessage::VisibleRange {
            from: time_ms(&get(o, "from"), "from")?,
            to: time_ms(&get(o, "to"), "to")?,
        }),
        _ => Err(js_err(
            "sync message 'type' must be 'crosshair' or 'visibleRange'",
        )),
    }
}

pub(crate) fn state_js(s: &ChartState) -> JsValue {
    let o = Object::new();
    set(&o, "version", &f64::from(s.version).into());
    set(&o, "barSpacing", &f64::from(s.bar_spacing.0).into());
    set(&o, "rightOffset", &s.right_offset.into());
    set(
        &o,
        "crosshairMode",
        &JsValue::from_str(params::crosshair_mode_name(s.crosshair_mode)),
    );
    let panes: Array = s
        .panes
        .iter()
        .map(|p| {
            let ps = &p.price_scale;
            let scale = Object::new();
            set(
                &scale,
                "mode",
                &JsValue::from_str(params::scale_mode_name(ps.mode)),
            );
            set(&scale, "inverted", &ps.inverted.into());
            let m = Object::new();
            set(&m, "top", &f64::from(ps.margins.0).into());
            set(&m, "bottom", &f64::from(ps.margins.1).into());
            set(&scale, "margins", &m.into());
            let range = ps.range.map_or(JsValue::NULL, |(lo, hi)| {
                let r = Object::new();
                set(&r, "low", &lo.into());
                set(&r, "high", &hi.into());
                r.into()
            });
            set(&scale, "range", &range);
            let pane = Object::new();
            set(&pane, "stretch", &f64::from(p.stretch).into());
            set(&pane, "priceScale", &scale.into());
            JsValue::from(pane)
        })
        .collect();
    set(&o, "panes", &panes.into());
    let drawings: Array = s
        .drawings
        .iter()
        .map(|d| {
            let js = Object::new();
            set(&js, "pane", &JsValue::from_f64(d.pane as f64));
            set(&js, "visible", &d.visible.into());
            set(&js, "locked", &d.locked.into());
            set(&js, "data", &drawing_data_js(&d.data));
            JsValue::from(js)
        })
        .collect();
    set(&o, "drawings", &drawings.into());
    o.into()
}

fn req_f64(o: &JsValue, k: &str) -> Result<f64, JsError> {
    opt_f64(o, k)?.ok_or_else(|| js_err(format!("state: missing '{k}'")))
}

pub(crate) fn state(o: &JsValue) -> Result<ChartState, JsError> {
    if !o.is_object() {
        return Err(js_err("state must be an object from saveState()"));
    }
    let version = req_f64(o, "version")?;
    if !(0.0..=f64::from(u32::MAX)).contains(&version) || version.fract() != 0.0 {
        return Err(js_err("state: invalid 'version'"));
    }
    let crosshair_mode = match opt_string(o, "crosshairMode")? {
        Some(m) => params::crosshair_mode(&m)
            .ok_or_else(|| js_err(format!("state: unknown crosshair mode '{m}'")))?,
        None => ferrochart_core::interaction::CrosshairMode::Normal,
    };
    let panes = array(&get(o, "panes"), "state.panes")?
        .iter()
        .map(|p| {
            let scale = get(&p, "priceScale");
            let mode = opt_string(&scale, "mode")?.unwrap_or_else(|| "normal".into());
            let mode = params::scale_mode(&mode)
                .ok_or_else(|| js_err(format!("state: unknown price scale mode '{mode}'")))?;
            let m = get(&scale, "margins");
            let r = get(&scale, "range");
            let range = if absent(&r) {
                None
            } else {
                Some((req_f64(&r, "low")?, req_f64(&r, "high")?))
            };
            Ok(PaneState {
                stretch: req_f64(&p, "stretch")? as f32,
                price_scale: PriceScaleState {
                    mode,
                    inverted: opt_bool(&scale, "inverted")?.unwrap_or(false),
                    margins: (req_f64(&m, "top")? as f32, req_f64(&m, "bottom")? as f32),
                    range,
                },
            })
        })
        .collect::<Result<Vec<_>, JsError>>()?;
    let drawings = array(&get(o, "drawings"), "state.drawings")?
        .iter()
        .map(|d| {
            let pane = req_f64(&d, "pane")?;
            if !(0.0..1e6).contains(&pane) || pane.fract() != 0.0 {
                return Err(js_err("state: invalid drawing pane"));
            }
            Ok(SavedDrawing {
                pane: pane as usize,
                data: drawing_data(&get(&d, "data"))?,
                visible: opt_bool(&d, "visible")?.unwrap_or(true),
                locked: opt_bool(&d, "locked")?.unwrap_or(false),
            })
        })
        .collect::<Result<Vec<_>, JsError>>()?;
    Ok(ChartState {
        version: version as u32,
        bar_spacing: Css(req_f64(o, "barSpacing")? as f32),
        right_offset: req_f64(o, "rightOffset")?,
        crosshair_mode,
        panes,
        drawings,
    })
}
