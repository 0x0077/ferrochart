//! Python option dicts (`snake_case` keys and names) ↔ plot types.
//! The same options as the JS binding
//! (`plot_api.rs` of `ferrochart-wasm`), spelled the Python way: `y_axis`,
//! `'arrow_up'`, `{'type': 'date', 'utc_offset_minutes': 60}`. Absent keys
//! keep the current value, `None` resets colors to the theme and clears
//! optional settings, unknown keys are errors.
// `Option<Option<_>>`: absent (keep) vs `None` (clear) vs a value.
#![allow(clippy::option_option)]

use ferrochart_core::geom::{Css, Rgba};
use ferrochart_core::theme::Theme;
use ferrochart_plot::{
    AxisFormat, AxisId, AxisKind, AxisOptions, AxisRange, ColorScale, LabelOptions, MarkOptions,
    RuleOptions, ScaleDomain, ScaleKind,
};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList, PyString};

use crate::opts::{check_keys, opt_bool, opt_color, opt_dash, opt_f64, opt_str, opt_width};
use crate::plot_params as names;
use crate::{camel, err, params, snake};

fn bad(k: &str, what: &str) -> PyErr {
    err(format!("'{k}' must be {what}"))
}

/// The value of `k`: `None` if absent, `Some(None)` if `None`.
fn entry<'py>(d: &Bound<'py, PyDict>, k: &str) -> PyResult<Option<Option<Bound<'py, PyAny>>>> {
    Ok(d.get_item(k)?.map(|v| (!v.is_none()).then_some(v)))
}

/// A name looked up with `f` (given in `camelCase`), or an error listing
/// the accepted names.
fn opt_name<T>(
    d: &Bound<'_, PyDict>,
    k: &str,
    f: impl Fn(&str) -> Option<T>,
    accepted: &str,
) -> PyResult<Option<T>> {
    opt_str(d, k)?
        .map(|s| f(&camel(&s)).ok_or_else(|| bad(k, accepted)))
        .transpose()
}

fn finite(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<f64>> {
    match opt_f64(d, k)? {
        Some(v) if !v.is_finite() => Err(bad(k, "a finite number")),
        v => Ok(v),
    }
}

fn number(v: &Bound<'_, PyAny>) -> Option<f64> {
    if v.is_instance_of::<PyBool>() {
        return None;
    }
    v.extract::<f64>().ok()
}

/// A pair of finite numbers (tuple, list or array of two).
fn pair(v: &Bound<'_, PyAny>, k: &str) -> PyResult<(f64, f64)> {
    let e = || bad(k, "a pair of finite numbers");
    if v.is_instance_of::<PyString>() {
        return Err(e());
    }
    let items: Vec<Bound<'_, PyAny>> = v
        .try_iter()
        .map_err(|_| e())?
        .collect::<PyResult<_>>()
        .map_err(|_| e())?;
    match items.as_slice() {
        [a, b] => match (number(a), number(b)) {
            (Some(a), Some(b)) if a.is_finite() && b.is_finite() => Ok((a, b)),
            _ => Err(e()),
        },
        _ => Err(e()),
    }
}

const FORMAT_HELP: &str = "'auto', 'compact', 'date', {'type': 'fixed' | 'percent', 'decimals': n} \
     or {'type': 'date', 'utc_offset_minutes': m}";

fn format(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<AxisFormat>> {
    let Some(Some(v)) = entry(d, k)? else {
        return Ok(None);
    };
    let e = || bad(k, FORMAT_HELP);
    if let Ok(s) = v.extract::<String>() {
        return names::axis_format(&s).map(Some).ok_or_else(e);
    }
    let o = v.cast::<PyDict>().map_err(|_| e())?;
    let t = opt_str(o, "type")?.ok_or_else(e)?;
    if t == "date" {
        check_keys(o, &["type", "utc_offset_minutes"], "format key")?;
        let m = opt_f64(o, "utc_offset_minutes")?.unwrap_or(0.0);
        if !(m.fract() == 0.0 && m.abs() <= 1440.0) {
            return Err(e());
        }
        return Ok(Some(AxisFormat::Date {
            utc_offset_min: m as i32,
        }));
    }
    check_keys(o, &["type", "decimals"], "format key")?;
    let n = opt_f64(o, "decimals")?.ok_or_else(e)?;
    names::axis_format_decimals(&t, n).map(Some).ok_or_else(e)
}

fn format_py(py: Python<'_>, f: AxisFormat) -> PyResult<Py<PyAny>> {
    Ok(match names::axis_format_parts(f) {
        (t, None, None) => PyString::new(py, t).into_any().unbind(),
        (t, n, m) => {
            let o = PyDict::new(py);
            o.set_item("type", t)?;
            if let Some(n) = n {
                o.set_item("decimals", n)?;
            }
            if let Some(m) = m {
                o.set_item("utc_offset_minutes", m)?;
            }
            o.into_any().unbind()
        }
    })
}

fn y_axis(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<AxisId>> {
    match opt_name(d, k, names::axis_id, "'left' or 'right'")? {
        Some(AxisId::X) => Err(bad(k, "'left' or 'right'")),
        v => Ok(v),
    }
}

/// `None` clears, a non-negative integer sets.
fn opt_id_number(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<Option<u32>>> {
    entry(d, k)?
        .map(|v| {
            v.map(|v| {
                if v.is_instance_of::<PyBool>() {
                    return Err(bad(k, "a non-negative integer or None"));
                }
                v.extract::<u32>()
                    .map_err(|_| bad(k, "a non-negative integer or None"))
            })
            .transpose()
        })
        .transpose()
}

const SCALE_HELP: &str = "'sequential', 'diverging' or a dict with stops, center, domain, quantile";

fn color_scale(v: &Bound<'_, PyAny>, theme: &Theme) -> PyResult<ColorScale> {
    const K: &str = "color_scale";
    if let Ok(s) = v.extract::<String>() {
        return match s.as_str() {
            "sequential" => Ok(ColorScale::sequential()),
            "diverging" => Ok(ColorScale::diverging(0.0, theme)),
            _ => Err(bad(K, SCALE_HELP)),
        };
    }
    let o = v.cast::<PyDict>().map_err(|_| bad(K, SCALE_HELP))?;
    check_keys(
        o,
        &["stops", "center", "domain", "quantile"],
        "color scale key",
    )?;
    let mut s = match finite(o, "center")? {
        Some(c) => ColorScale::diverging(c, theme),
        None => ColorScale::sequential(),
    };
    if let Some(Some(stops)) = entry(o, "stops")? {
        let e = || {
            bad(
                "stops",
                "a list of (position 0..1, CSS color) pairs, ascending",
            )
        };
        let mut out: Vec<(f32, Rgba)> = Vec::new();
        for item in stops.try_iter().map_err(|_| e())? {
            let item = item?;
            let (pos, c): (f64, String) = item.extract().map_err(|_| e())?;
            let c = params::color(&c).ok_or_else(e)?;
            if !(0.0..=1.0).contains(&pos) || out.last().is_some_and(|l| f64::from(l.0) > pos) {
                return Err(e());
            }
            out.push((pos as f32, c));
        }
        if out.is_empty() {
            return Err(e());
        }
        s.stops = out;
    }
    if let Some(Some(domain)) = entry(o, "domain")? {
        let (a, b) = pair(&domain, "domain")?;
        s.domain = ScaleDomain::Fixed(a, b);
    } else if let Some(q) = opt_f64(o, "quantile")? {
        if !(0.0..=0.5).contains(&q) {
            return Err(bad("quantile", "between 0 and 0.5"));
        }
        s.domain = ScaleDomain::Auto { quantile: q as f32 };
    }
    Ok(s)
}

fn color_py(c: Option<Rgba>) -> Option<String> {
    c.map(params::color_css)
}

fn color_scale_py<'py>(py: Python<'py>, s: &ColorScale) -> PyResult<Bound<'py, PyDict>> {
    let o = PyDict::new(py);
    let stops = PyList::empty(py);
    for (p, c) in &s.stops {
        stops.append((f64::from(*p), params::color_css(*c)))?;
    }
    o.set_item("stops", stops)?;
    if let ScaleKind::Diverging { center } = s.kind {
        o.set_item("center", center)?;
    }
    match s.domain {
        ScaleDomain::Fixed(a, b) => o.set_item("domain", (a, b))?,
        ScaleDomain::Auto { quantile } => o.set_item("quantile", f64::from(quantile))?,
    }
    Ok(o)
}

fn set_color(dst: &mut Option<Rgba>, v: Option<Option<Rgba>>) {
    if let Some(c) = v {
        *dst = c;
    }
}

const MARK_KEYS: [&str; 21] = [
    "y_axis",
    "orientation",
    "color",
    "down_color",
    "total_color",
    "fill_alpha",
    "line_width",
    "line_style",
    "step_line",
    "point_size",
    "icon",
    "base_value",
    "group",
    "stack",
    "band",
    "color_scale",
    "show_values",
    "value_format",
    "cell_size",
    "hoverable",
    "visible",
];

/// `base` with the keys of `d` applied.
pub(crate) fn mark_options(
    base: MarkOptions,
    d: &Bound<'_, PyDict>,
    theme: &Theme,
) -> PyResult<MarkOptions> {
    check_keys(d, &MARK_KEYS, "mark option")?;
    let mut m = base;
    if let Some(a) = y_axis(d, "y_axis")? {
        m.y_axis = a;
    }
    if let Some(v) = opt_name(
        d,
        "orientation",
        names::orientation,
        "'vertical' or 'horizontal'",
    )? {
        m.orientation = v;
    }
    set_color(&mut m.color, opt_color(d, "color")?);
    set_color(&mut m.down_color, opt_color(d, "down_color")?);
    set_color(&mut m.total_color, opt_color(d, "total_color")?);
    if let Some(a) = opt_f64(d, "fill_alpha")? {
        if !(0.0..=1.0).contains(&a) {
            return Err(bad("fill_alpha", "between 0 and 1"));
        }
        m.fill_alpha = a as f32;
    }
    if let Some(w) = opt_width(d, "line_width")? {
        m.line_width = w;
    }
    if let Some(s) = opt_dash(d, "line_style")? {
        m.line_style = s;
    }
    if let Some(b) = opt_bool(d, "step_line")? {
        m.step_line = b;
    }
    if let Some(s) = opt_f64(d, "point_size")? {
        if !(s.is_finite() && s >= 0.0) {
            return Err(bad("point_size", "a number >= 0"));
        }
        m.point_size = Css(s as f32);
    }
    if let Some(i) = opt_name(
        d,
        "icon",
        names::icon,
        "'circle', 'square', 'arrow_up' or 'arrow_down'",
    )? {
        m.icon = i;
    }
    if let Some(b) = finite(d, "base_value")? {
        m.base_value = b;
    }
    if let Some(g) = opt_id_number(d, "group")? {
        m.group = g;
    }
    if let Some(s) = opt_id_number(d, "stack")? {
        m.stack = s;
    }
    match entry(d, "band")? {
        Some(None) => m.band = f32::NAN,
        Some(Some(_)) => {
            let b = opt_f64(d, "band")?.unwrap_or(f64::NAN);
            if !(b > 0.0 && b <= 1.0) {
                return Err(bad("band", "between 0 (exclusive) and 1, or None"));
            }
            m.band = b as f32;
        }
        None => {}
    }
    match entry(d, "color_scale")? {
        Some(None) => m.color_scale = None,
        Some(Some(v)) => m.color_scale = Some(color_scale(&v, theme)?),
        None => {}
    }
    if let Some(b) = opt_bool(d, "show_values")? {
        m.show_values = b;
    }
    if let Some(f) = format(d, "value_format")? {
        m.value_format = f;
    }
    match entry(d, "cell_size")? {
        Some(None) => m.cell_size = None,
        Some(Some(v)) => {
            let (w, h) = pair(&v, "cell_size")?;
            if !(w > 0.0 && h > 0.0) {
                return Err(bad("cell_size", "two positive numbers"));
            }
            m.cell_size = Some((w, h));
        }
        None => {}
    }
    if let Some(b) = opt_bool(d, "hoverable")? {
        m.hoverable = b;
    }
    if let Some(b) = opt_bool(d, "visible")? {
        m.visible = b;
    }
    Ok(m)
}

pub(crate) fn mark_options_py<'py>(
    py: Python<'py>,
    m: &MarkOptions,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("y_axis", names::axis_id_name(m.y_axis))?;
    d.set_item("orientation", names::orientation_name(m.orientation))?;
    d.set_item("color", color_py(m.color))?;
    d.set_item("down_color", color_py(m.down_color))?;
    d.set_item("total_color", color_py(m.total_color))?;
    d.set_item("fill_alpha", f64::from(m.fill_alpha))?;
    d.set_item("line_width", f64::from(m.line_width.0))?;
    d.set_item("line_style", params::line_dash_name(m.line_style))?;
    d.set_item("step_line", m.step_line)?;
    d.set_item("point_size", f64::from(m.point_size.0))?;
    d.set_item("icon", snake(names::icon_name(m.icon)))?;
    d.set_item("base_value", m.base_value)?;
    d.set_item("group", m.group)?;
    d.set_item("stack", m.stack)?;
    d.set_item("band", (!m.band.is_nan()).then_some(f64::from(m.band)))?;
    match &m.color_scale {
        Some(s) => d.set_item("color_scale", color_scale_py(py, s)?)?,
        None => d.set_item("color_scale", py.None())?,
    }
    d.set_item("show_values", m.show_values)?;
    d.set_item("value_format", format_py(py, m.value_format)?)?;
    d.set_item("cell_size", m.cell_size)?;
    d.set_item("hoverable", m.hoverable)?;
    d.set_item("visible", m.visible)?;
    Ok(d)
}

const AXIS_KEYS: [&str; 10] = [
    "type",
    "range",
    "include_zero",
    "margin",
    "format",
    "inverted",
    "visible",
    "grid",
    "title",
    "locked",
];

/// `base` with the keys of `d` applied; a new `type` starts from that
/// type's defaults (grid, margins).
pub(crate) fn axis_options(base: &AxisOptions, d: &Bound<'_, PyDict>) -> PyResult<AxisOptions> {
    check_keys(d, &AXIS_KEYS, "axis option")?;
    let mut a = base.clone();
    if let Some(k) = opt_name(d, "type", names::axis_kind, "'linear', 'log' or 'category'")?
        && k != a.kind
    {
        a = match k {
            AxisKind::Linear => AxisOptions::linear(),
            AxisKind::Log => AxisOptions::log(),
            AxisKind::Category => AxisOptions::category(),
        };
        a.visible = base.visible;
        a.title.clone_from(&base.title);
        a.format = base.format;
        a.inverted = base.inverted;
        a.locked = base.locked;
    }
    if let Some(Some(range)) = entry(d, "range")? {
        if range.extract::<String>().ok().as_deref() == Some("auto") {
            if matches!(a.range, AxisRange::Fixed(..)) {
                a.range = match a.kind {
                    AxisKind::Category => AxisOptions::category().range,
                    _ => AxisRange::default(),
                };
            }
        } else {
            let (lo, hi) = pair(&range, "range")?;
            a.range = AxisRange::Fixed(lo, hi);
        }
    }
    if let AxisRange::Auto {
        include_zero,
        margin,
    } = &mut a.range
    {
        if let Some(z) = opt_bool(d, "include_zero")? {
            *include_zero = z;
        }
        if let Some(m) = opt_f64(d, "margin")? {
            if !(0.0..=1.0).contains(&m) {
                return Err(bad("margin", "between 0 and 1"));
            }
            *margin = m as f32;
        }
    }
    if let Some(f) = format(d, "format")? {
        a.format = f;
    }
    if let Some(b) = opt_bool(d, "inverted")? {
        a.inverted = b;
    }
    if let Some(b) = opt_bool(d, "visible")? {
        a.visible = b;
    }
    if let Some(b) = opt_bool(d, "grid")? {
        a.grid = b;
    }
    match entry(d, "title")? {
        Some(None) => a.title = None,
        Some(Some(_)) => a.title = opt_str(d, "title")?,
        None => {}
    }
    if let Some(b) = opt_bool(d, "locked")? {
        a.locked = b;
    }
    Ok(a)
}

pub(crate) fn axis_options_py<'py>(
    py: Python<'py>,
    a: &AxisOptions,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("type", names::axis_kind_name(a.kind))?;
    match a.range {
        AxisRange::Auto {
            include_zero,
            margin,
        } => {
            d.set_item("range", "auto")?;
            d.set_item("include_zero", include_zero)?;
            d.set_item("margin", f64::from(margin))?;
        }
        AxisRange::Fixed(lo, hi) => d.set_item("range", (lo, hi))?,
    }
    d.set_item("format", format_py(py, a.format)?)?;
    d.set_item("inverted", a.inverted)?;
    d.set_item("visible", a.visible)?;
    d.set_item("grid", a.grid)?;
    d.set_item("title", a.title.as_deref())?;
    d.set_item("locked", a.locked)?;
    Ok(d)
}

const RULE_KEYS: [&str; 8] = [
    "axis",
    "value",
    "to",
    "color",
    "line_width",
    "line_style",
    "label",
    "label_background",
];

pub(crate) fn rule_options(base: RuleOptions, d: &Bound<'_, PyDict>) -> PyResult<RuleOptions> {
    check_keys(d, &RULE_KEYS, "rule option")?;
    let mut r = base;
    if let Some(a) = opt_name(d, "axis", names::axis_id, "'x', 'left' or 'right'")? {
        r.axis = a;
    }
    if let Some(v) = finite(d, "value")? {
        r.value = v;
    }
    match entry(d, "to")? {
        Some(None) => r.to = None,
        Some(Some(_)) => r.to = finite(d, "to")?,
        None => {}
    }
    set_color(&mut r.color, opt_color(d, "color")?);
    if let Some(w) = opt_width(d, "line_width")? {
        r.line_width = w;
    }
    if let Some(s) = opt_dash(d, "line_style")? {
        r.line_style = s;
    }
    match entry(d, "label")? {
        Some(None) => r.label = None,
        Some(Some(_)) => r.label = opt_str(d, "label")?,
        None => {}
    }
    if let Some(b) = opt_bool(d, "label_background")? {
        r.label_bg = b;
    }
    Ok(r)
}

pub(crate) fn rule_options_py<'py>(
    py: Python<'py>,
    r: &RuleOptions,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("axis", names::axis_id_name(r.axis))?;
    d.set_item("value", r.value)?;
    d.set_item("to", r.to)?;
    d.set_item("color", color_py(r.color))?;
    d.set_item("line_width", f64::from(r.line_width.0))?;
    d.set_item("line_style", params::line_dash_name(r.line_style))?;
    d.set_item("label", r.label.as_deref())?;
    d.set_item("label_background", r.label_bg)?;
    Ok(d)
}

const LABEL_KEYS: [&str; 8] = [
    "x", "y", "y_axis", "text", "icon", "color", "align", "offset",
];

pub(crate) fn label_options(d: &Bound<'_, PyDict>) -> PyResult<LabelOptions> {
    check_keys(d, &LABEL_KEYS, "label option")?;
    let mut l = LabelOptions {
        x: finite(d, "x")?.ok_or_else(|| err("missing 'x'"))?,
        y: finite(d, "y")?.ok_or_else(|| err("missing 'y'"))?,
        ..LabelOptions::default()
    };
    if let Some(a) = y_axis(d, "y_axis")? {
        l.y_axis = a;
    }
    if let Some(t) = opt_str(d, "text")? {
        l.text = t;
    }
    l.icon = opt_name(
        d,
        "icon",
        names::icon,
        "'circle', 'square', 'arrow_up', 'arrow_down' or None",
    )?;
    set_color(&mut l.color, opt_color(d, "color")?);
    if let Some(a) = opt_name(d, "align", names::text_align, "'left', 'center' or 'right'")? {
        l.align = a;
    }
    if let Some(Some(off)) = entry(d, "offset")? {
        let (dx, dy) = pair(&off, "offset")?;
        l.offset = (Css(dx as f32), Css(dy as f32));
    }
    Ok(l)
}
