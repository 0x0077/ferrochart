//! Python option dicts (`snake_case` keys) ↔ chart types. Unknown keys are errors; a
//! `None` color uses the theme.

use std::cell::RefCell;

use ferrochart_core::chart::{
    Marker, MarkerPosition, MarkerShape, PriceLineOptions, SeriesOptions,
};
use ferrochart_core::geom::{Css, Rgba};
use ferrochart_core::icon::IconId;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList};

use crate::params::{self, Value};
use crate::{camel, err, snake_message};

/// Fails if `d` has a key not in `known`.
pub(crate) fn check_keys(d: &Bound<'_, PyDict>, known: &[&str], what: &str) -> PyResult<()> {
    for k in d.keys() {
        let k: String = k.extract()?;
        if !known.contains(&k.as_str()) {
            return Err(err(format!(
                "unknown {what} '{k}' (expected one of: {})",
                known.join(", ")
            )));
        }
    }
    Ok(())
}

/// The value of `k`, `None` if absent or `None`.
fn item<'py>(d: &Bound<'py, PyDict>, k: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(d.get_item(k)?.filter(|v| !v.is_none()))
}

pub(crate) fn opt_f64(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<f64>> {
    item(d, k)?
        .map(|v| {
            if v.is_instance_of::<PyBool>() {
                return Err(err(format!("'{k}' must be a number")));
            }
            v.extract::<f64>()
                .map_err(|_| err(format!("'{k}' must be a number")))
        })
        .transpose()
}

pub(crate) fn opt_bool(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<bool>> {
    item(d, k)?
        .map(|v| {
            v.cast::<PyBool>()
                .map(PyBoolMethods::is_true)
                .map_err(|_| err(format!("'{k}' must be True or False")))
        })
        .transpose()
}

pub(crate) fn opt_str(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<String>> {
    item(d, k)?
        .map(|v| {
            v.extract::<String>()
                .map_err(|_| err(format!("'{k}' must be a string")))
        })
        .transpose()
}

/// A CSS color (see `params::color`).
pub(crate) fn color(s: &str, k: &str) -> PyResult<Rgba> {
    params::color(s).ok_or_else(|| err(format!("'{k}': '{s}' is not a CSS color")))
}

/// `Some(None)` for an explicit `None` (theme color).
#[allow(clippy::option_option)]
pub(crate) fn opt_color(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<Option<Rgba>>> {
    match d.get_item(k)? {
        None => Ok(None),
        Some(v) if v.is_none() => Ok(Some(None)),
        Some(v) => {
            let s: String = v
                .extract()
                .map_err(|_| err(format!("'{k}' must be a CSS color string or None")))?;
            Ok(Some(Some(color(&s, k)?)))
        }
    }
}

pub(crate) fn opt_width(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<Css>> {
    match opt_f64(d, k)? {
        Some(w) if !(w.is_finite() && w > 0.0) => Err(err(format!("'{k}' must be positive"))),
        w => Ok(w.map(|w| Css(w as f32))),
    }
}

pub(crate) fn opt_dash(
    d: &Bound<'_, PyDict>,
    k: &str,
) -> PyResult<Option<ferrochart_core::frame::LineDash>> {
    opt_str(d, k)?
        .map(|s| {
            params::line_dash(&s)
                .ok_or_else(|| err(format!("'{k}' must be 'solid', 'dashed' or 'dotted'")))
        })
        .transpose()
}

fn color_py(py: Python<'_>, c: Option<Rgba>) -> Py<PyAny> {
    match c {
        Some(c) => params::color_css(c)
            .into_pyobject(py)
            .map_or_else(|_| py.None(), |s| s.into_any().unbind()),
        None => py.None(),
    }
}

const SERIES_KEYS: [&str; 13] = [
    "color",
    "down_color",
    "top_fill",
    "bottom_fill",
    "line_width",
    "line_style",
    "step",
    "base_value",
    "last_value_visible",
    "price_line_visible",
    "shift",
    "price_precision",
    "marker_stack_limit",
];

/// `base` with the keys of `d` applied.
pub(crate) fn series_options(
    base: SeriesOptions,
    d: &Bound<'_, PyDict>,
) -> PyResult<SeriesOptions> {
    check_keys(d, &SERIES_KEYS, "series option")?;
    let mut o = base;
    if let Some(c) = opt_color(d, "color")? {
        o.color = c;
    }
    if let Some(c) = opt_color(d, "down_color")? {
        o.down_color = c;
    }
    if let Some(c) = opt_color(d, "top_fill")? {
        o.top_fill = c;
    }
    if let Some(c) = opt_color(d, "bottom_fill")? {
        o.bottom_fill = c;
    }
    if let Some(w) = opt_width(d, "line_width")? {
        o.line_width = w;
    }
    if let Some(s) = opt_dash(d, "line_style")? {
        o.line_style = s;
    }
    if let Some(b) = opt_bool(d, "step")? {
        o.step = b;
    }
    if let Some(v) = opt_f64(d, "base_value")? {
        if !v.is_finite() {
            return Err(err("'base_value' must be finite"));
        }
        o.base_value = v;
    }
    if let Some(b) = opt_bool(d, "last_value_visible")? {
        o.last_value_visible = b;
    }
    if let Some(b) = opt_bool(d, "price_line_visible")? {
        o.price_line_visible = b;
    }
    if let Some(v) = opt_f64(d, "shift")? {
        if v.fract() != 0.0 || v.abs() > 1e6 {
            return Err(err("'shift' must be an integer"));
        }
        o.shift = v as i32;
    }
    if d.get_item("price_precision")?.is_some_and(|v| v.is_none()) {
        o.price_precision = None; // automatic
    } else if let Some(v) = opt_f64(d, "price_precision")? {
        if !(0.0..=12.0).contains(&v) || v.fract() != 0.0 {
            return Err(err(
                "'price_precision' must be an integer in 0..=12, or None for automatic",
            ));
        }
        o.price_precision = Some(v as u8);
    }
    if d.get_item("marker_stack_limit")?
        .is_some_and(|v| v.is_none())
    {
        o.marker_stack_limit = None; // no folding
    } else if let Some(v) = opt_f64(d, "marker_stack_limit")? {
        if !(1.0..=f64::from(u16::MAX)).contains(&v) || v.fract() != 0.0 {
            return Err(err(
                "'marker_stack_limit' must be an integer >= 1, or None for no limit",
            ));
        }
        o.marker_stack_limit = Some(v as u16);
    }
    Ok(o)
}

pub(crate) fn series_options_py<'py>(
    py: Python<'py>,
    o: &SeriesOptions,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("color", color_py(py, o.color))?;
    d.set_item("down_color", color_py(py, o.down_color))?;
    d.set_item("top_fill", color_py(py, o.top_fill))?;
    d.set_item("bottom_fill", color_py(py, o.bottom_fill))?;
    d.set_item("line_width", f64::from(o.line_width.0))?;
    d.set_item("line_style", params::line_dash_name(o.line_style))?;
    d.set_item("step", o.step)?;
    d.set_item("base_value", o.base_value)?;
    d.set_item("last_value_visible", o.last_value_visible)?;
    d.set_item("price_line_visible", o.price_line_visible)?;
    d.set_item("shift", o.shift)?;
    d.set_item("price_precision", o.price_precision)?;
    d.set_item("marker_stack_limit", o.marker_stack_limit)?;
    Ok(d)
}

const PRICE_LINE_KEYS: [&str; 6] = [
    "price",
    "color",
    "line_width",
    "line_style",
    "axis_label_visible",
    "title",
];

pub(crate) fn price_line_options(
    base: PriceLineOptions,
    d: &Bound<'_, PyDict>,
) -> PyResult<PriceLineOptions> {
    check_keys(d, &PRICE_LINE_KEYS, "price line option")?;
    let mut o = base;
    if let Some(p) = opt_f64(d, "price")? {
        if !p.is_finite() {
            return Err(err("'price' must be finite"));
        }
        o.price = p;
    }
    if let Some(c) = opt_color(d, "color")? {
        o.color = c;
    }
    if let Some(w) = opt_width(d, "line_width")? {
        o.line_width = w;
    }
    if let Some(s) = opt_dash(d, "line_style")? {
        o.line_style = s;
    }
    if let Some(b) = opt_bool(d, "axis_label_visible")? {
        o.axis_label_visible = b;
    }
    if let Some(t) = opt_str(d, "title")? {
        o.title = t;
    }
    Ok(o)
}

pub(crate) fn price_line_options_py<'py>(
    py: Python<'py>,
    o: &PriceLineOptions,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("price", o.price)?;
    d.set_item("color", color_py(py, o.color))?;
    d.set_item("line_width", f64::from(o.line_width.0))?;
    d.set_item("line_style", params::line_dash_name(o.line_style))?;
    d.set_item("axis_label_visible", o.axis_label_visible)?;
    d.set_item("title", &o.title)?;
    Ok(d)
}

/// Markers from dicts `{time (int ms), position?, shape?, color?, text?,
/// size?, icon? (an add_icon id), label?, border?, border_width?}`.
pub(crate) fn markers(list: &Bound<'_, PyList>) -> PyResult<Vec<Marker>> {
    let mut out = Vec::with_capacity(list.len());
    for m in list.iter() {
        let d = m
            .cast::<PyDict>()
            .map_err(|_| err("each marker must be a dict"))?;
        check_keys(
            d,
            &[
                "time",
                "position",
                "shape",
                "color",
                "text",
                "size",
                "icon",
                "label",
                "border",
                "border_width",
            ],
            "marker key",
        )?;
        let time: i64 = item(d, "time")?
            .ok_or_else(|| err("a marker needs 'time'"))?
            .extract()
            .map_err(|_| err("marker 'time' must be an integer (ms)"))?;
        let mut marker = Marker {
            time,
            color: opt_color(d, "color")?.flatten(),
            text: opt_str(d, "text")?.unwrap_or_default(),
            label: opt_str(d, "label")?.unwrap_or_default(),
            border: opt_color(d, "border")?.flatten(),
            ..Marker::default()
        };
        if let Some(s) = opt_f64(d, "size")? {
            marker.size = Some(Css(s as f32));
        }
        if let Some(w) = opt_f64(d, "border_width")? {
            marker.border_width = Css(w as f32);
        }
        if let Some(icon) = item(d, "icon")?.filter(|v| !v.is_none()) {
            let id: u32 = icon
                .extract()
                .map_err(|_| err("marker 'icon' must be an id from Chart.add_icon"))?;
            marker.icon = Some(IconId(id));
        }
        if let Some(p) = opt_str(d, "position")? {
            marker.position = params::marker_position(&camel(&p))
                .ok_or_else(|| err("'position' must be 'above_bar' or 'below_bar'"))?;
        }
        if let Some(s) = opt_str(d, "shape")? {
            marker.shape = params::marker_shape(&camel(&s)).ok_or_else(|| {
                err("'shape' must be 'circle', 'square', 'arrow_up' or 'arrow_down'")
            })?;
        }
        out.push(marker);
    }
    Ok(out)
}

pub(crate) fn markers_py<'py>(py: Python<'py>, markers: &[Marker]) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::empty(py);
    for m in markers {
        let d = PyDict::new(py);
        d.set_item("time", m.time)?;
        let position = match m.position {
            MarkerPosition::AboveBar => "above_bar",
            MarkerPosition::BelowBar => "below_bar",
        };
        let shape = match m.shape {
            MarkerShape::Circle => "circle",
            MarkerShape::Square => "square",
            MarkerShape::ArrowUp => "arrow_up",
            MarkerShape::ArrowDown => "arrow_down",
        };
        d.set_item("position", position)?;
        d.set_item("shape", shape)?;
        d.set_item("color", color_py(py, m.color))?;
        d.set_item("text", &m.text)?;
        d.set_item("size", m.size.map(|s| f64::from(s.0)))?;
        d.set_item("icon", m.icon.map(|i| i.0))?;
        d.set_item("label", &m.label)?;
        d.set_item("border", color_py(py, m.border))?;
        d.set_item("border_width", f64::from(m.border_width.0))?;
        list.append(d)?;
    }
    Ok(list)
}

fn param_value(dict: &Bound<'_, PyDict>, key: &str) -> Value {
    match dict.get_item(key) {
        Ok(Some(v)) if !v.is_none() => {
            if v.is_instance_of::<PyBool>() {
                Value::Other
            } else if let Ok(s) = v.extract::<String>() {
                Value::Text(camel(&s))
            } else if let Ok(f) = v.extract::<f64>() {
                Value::Number(f)
            } else {
                Value::Other
            }
        }
        _ => Value::Missing,
    }
}

/// Builds an indicator or transform with `build` (a builder of `params.rs`,
/// which asks parameters by their `camelCase` names) from a `snake_case`
/// dict. Keys the builder never asks for are errors, reported before the
/// builder's own error. The builder is first run with every parameter
/// missing, so that the names it asks after a parameter that fails are
/// known too.
pub(crate) fn build_params<T>(
    dict: &Bound<'_, PyDict>,
    what: &str,
    build: impl Fn(&dyn Fn(&str) -> Value) -> Result<T, String>,
) -> PyResult<T> {
    let asked = RefCell::new(Vec::<String>::new());
    let ask = |k: &str| {
        let key = crate::snake(k);
        let mut a = asked.borrow_mut();
        if !a.contains(&key) {
            a.push(key);
        }
    };
    let _ = build(&|k| {
        ask(k);
        Value::Missing
    });
    let built = build(&|k| {
        ask(k);
        param_value(dict, &crate::snake(k))
    });
    let asked = asked.into_inner();
    for k in dict.keys() {
        let k: String = k.extract()?;
        if !asked.contains(&k) {
            return Err(err(format!(
                "unknown parameter '{k}' for {what} (expected: {})",
                asked.join(", ")
            )));
        }
    }
    built.map_err(|m| err(snake_message(&m)))
}
