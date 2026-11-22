//! Drawings and saved state for Python:
//! the native `Drawings` class and the dict forms of drawings and of
//! `ChartState`.
//!
//! Dict forms (`snake_case` keys, JSON-compatible: lists, not tuples):
//! - drawing: `{kind, anchors: [{time (int ms), price}], style?, params?}`;
//!   style keys `line`, `line_width`, `line_dash`, `fill`, `text`,
//!   `font_size`; params are numbers, bools, strings or lists of numbers;
//! - state: `{version, bar_spacing, right_offset, crosshair_mode, panes:
//!   [{stretch, price_scale: {mode, inverted, margins: [top, bottom], range:
//!   [low, high] | None}}], drawings: [{pane, visible, locked, data}]}`.

use std::sync::Arc;

use ferrochart_core::data::{DrawingId, PaneId};
use ferrochart_core::drawing::{Anchor, DrawingData, DrawingManager, DrawingStyle, Param};
use ferrochart_core::geom::Css;
use ferrochart_core::interaction::CrosshairMode;
use ferrochart_core::state::{ChartState, PaneState, PriceScaleState, SavedDrawing};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList, PyString};

use crate::chart::Shared;
use crate::opts::{check_keys, opt_bool, opt_f64, opt_str};
use crate::{camel, err, params};

const STYLE_KEYS: [&str; 6] = [
    "line",
    "line_width",
    "line_dash",
    "fill",
    "text",
    "font_size",
];

fn dict<'a, 'py>(v: &'a Bound<'py, PyAny>, what: &str) -> PyResult<&'a Bound<'py, PyDict>> {
    v.cast::<PyDict>()
        .map_err(|_| err(format!("{what} must be a dict")))
}

fn list<'py>(v: &Bound<'py, PyAny>, what: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    if v.is_instance_of::<PyString>() || v.is_instance_of::<PyDict>() {
        return Err(err(format!("{what} must be a list")));
    }
    v.try_iter()
        .map_err(|_| err(format!("{what} must be a list")))?
        .collect()
}

fn req_f64(d: &Bound<'_, PyDict>, k: &str, what: &str) -> PyResult<f64> {
    opt_f64(d, k)?.ok_or_else(|| err(format!("{what} needs '{k}'")))
}

/// A pair `[a, b]` of numbers.
fn pair(v: &Bound<'_, PyAny>, what: &str) -> PyResult<(f64, f64)> {
    let items = list(v, what)?;
    let num = |x: &Bound<'_, PyAny>| -> PyResult<f64> {
        if x.is_instance_of::<PyBool>() {
            return Err(err(format!("{what} must hold two numbers")));
        }
        x.extract::<f64>()
            .map_err(|_| err(format!("{what} must hold two numbers")))
    };
    match items.as_slice() {
        [a, b] => Ok((num(a)?, num(b)?)),
        _ => Err(err(format!("{what} must hold two numbers"))),
    }
}

pub(crate) fn anchors(v: &Bound<'_, PyAny>) -> PyResult<Vec<Anchor>> {
    list(v, "anchors")?
        .iter()
        .map(|a| {
            let d = dict(a, "each anchor")?;
            check_keys(d, &["time", "price"], "anchor key")?;
            let time = d
                .get_item("time")?
                .filter(|t| !t.is_none() && !t.is_instance_of::<PyBool>())
                .ok_or_else(|| err("an anchor needs 'time'"))?
                .extract::<i64>()
                .map_err(|_| err("anchor 'time' must be an integer (ms)"))?;
            let price = req_f64(d, "price", "an anchor")?;
            if !price.is_finite() {
                return Err(err("anchor 'price' must be finite"));
            }
            Ok(Anchor { time, price })
        })
        .collect()
}

fn anchors_py<'py>(py: Python<'py>, anchors: &[Anchor]) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    for a in anchors {
        let d = PyDict::new(py);
        d.set_item("time", a.time)?;
        d.set_item("price", a.price)?;
        out.append(d)?;
    }
    Ok(out)
}

fn style_color(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<ferrochart_core::geom::Rgba>> {
    opt_str(d, k)?
        .map(|s| crate::opts::color(&s, k))
        .transpose()
}

fn style_width(d: &Bound<'_, PyDict>, k: &str) -> PyResult<Option<Css>> {
    match opt_f64(d, k)? {
        Some(w) if !(w.is_finite() && w > 0.0) => Err(err(format!("'{k}' must be positive"))),
        w => Ok(w.map(|w| Css(w as f32))),
    }
}

/// `base` with the keys of `d` applied.
pub(crate) fn style(base: DrawingStyle, d: &Bound<'_, PyDict>) -> PyResult<DrawingStyle> {
    check_keys(d, &STYLE_KEYS, "drawing style key")?;
    let mut s = base;
    if let Some(c) = style_color(d, "line")? {
        s.line = c;
    }
    if let Some(w) = style_width(d, "line_width")? {
        s.line_width = w;
    }
    if let Some(dash) = opt_str(d, "line_dash")? {
        s.line_dash = params::line_dash(&dash)
            .ok_or_else(|| err("'line_dash' must be 'solid', 'dashed' or 'dotted'"))?;
    }
    if let Some(c) = style_color(d, "fill")? {
        s.fill = c;
    }
    if let Some(c) = style_color(d, "text")? {
        s.text = c;
    }
    if let Some(w) = style_width(d, "font_size")? {
        s.font_size = w;
    }
    Ok(s)
}

fn style_py<'py>(py: Python<'py>, s: &DrawingStyle) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("line", params::color_css(s.line))?;
    d.set_item("line_width", f64::from(s.line_width.0))?;
    d.set_item("line_dash", params::line_dash_name(s.line_dash))?;
    d.set_item("fill", params::color_css(s.fill))?;
    d.set_item("text", params::color_css(s.text))?;
    d.set_item("font_size", f64::from(s.font_size.0))?;
    Ok(d)
}

/// A tool parameter: bool, number, string or list of numbers.
pub(crate) fn param(v: &Bound<'_, PyAny>, key: &str) -> PyResult<Param> {
    let bad = || {
        err(format!(
            "parameter '{key}' must be a number, bool, string or list of numbers"
        ))
    };
    if v.is_instance_of::<PyBool>() {
        return Ok(Param::Bool(v.extract()?));
    }
    if let Ok(s) = v.cast::<PyString>() {
        return Ok(Param::Text(s.to_str()?.to_owned()));
    }
    if let Ok(x) = v.extract::<f64>() {
        return Ok(Param::Number(x));
    }
    if v.is_none() || v.is_instance_of::<PyDict>() {
        return Err(bad());
    }
    let items = v.try_iter().map_err(|_| bad())?;
    items
        .map(|x| {
            let x = x?;
            if x.is_instance_of::<PyBool>() {
                return Err(err(format!("'{key}' must contain only numbers")));
            }
            x.extract::<f64>()
                .map_err(|_| err(format!("'{key}' must contain only numbers")))
        })
        .collect::<PyResult<_>>()
        .map(Param::Numbers)
}

fn param_py<'py>(py: Python<'py>, p: &Param) -> PyResult<Bound<'py, PyAny>> {
    Ok(match p {
        Param::Number(x) => x.into_pyobject(py)?.into_any(),
        Param::Bool(b) => PyBool::new(py, *b).to_owned().into_any(),
        Param::Text(s) => PyString::new(py, s).into_any(),
        Param::Numbers(v) => PyList::new(py, v)?.into_any(),
    })
}

pub(crate) fn drawing_data(v: &Bound<'_, PyAny>) -> PyResult<DrawingData> {
    let d = dict(v, "a drawing")?;
    check_keys(d, &["kind", "anchors", "style", "params"], "drawing key")?;
    let kind = opt_str(d, "kind")?.ok_or_else(|| err("a drawing needs 'kind'"))?;
    let a = d
        .get_item("anchors")?
        .ok_or_else(|| err("a drawing needs 'anchors'"))?;
    let mut data = DrawingData::new(&kind, anchors(&a)?);
    if let Some(s) = d.get_item("style")?.filter(|s| !s.is_none()) {
        data.style = style(data.style, dict(&s, "'style'")?)?;
    }
    if let Some(ps) = d.get_item("params")?.filter(|p| !p.is_none()) {
        for (k, v) in dict(&ps, "'params'")?.iter() {
            let k: String = k
                .extract()
                .map_err(|_| err("parameter names must be strings"))?;
            let p = param(&v, &k)?;
            data.set(&k, p);
        }
    }
    Ok(data)
}

pub(crate) fn drawing_data_py<'py>(
    py: Python<'py>,
    data: &DrawingData,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("kind", &data.kind)?;
    d.set_item("anchors", anchors_py(py, &data.anchors)?)?;
    d.set_item("style", style_py(py, &data.style)?)?;
    let ps = PyDict::new(py);
    for (k, v) in &data.params {
        ps.set_item(k, param_py(py, v)?)?;
    }
    d.set_item("params", ps)?;
    Ok(d)
}

pub(crate) fn state_py<'py>(py: Python<'py>, s: &ChartState) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("version", s.version)?;
    d.set_item("bar_spacing", f64::from(s.bar_spacing.0))?;
    d.set_item("right_offset", s.right_offset)?;
    d.set_item(
        "crosshair_mode",
        params::crosshair_mode_name(s.crosshair_mode),
    )?;
    let panes = PyList::empty(py);
    for p in &s.panes {
        let ps = &p.price_scale;
        let scale = PyDict::new(py);
        scale.set_item("mode", crate::chart::scale_mode_name(ps.mode))?;
        scale.set_item("inverted", ps.inverted)?;
        scale.set_item(
            "margins",
            PyList::new(py, [f64::from(ps.margins.0), f64::from(ps.margins.1)])?,
        )?;
        match ps.range {
            Some((lo, hi)) => scale.set_item("range", PyList::new(py, [lo, hi])?)?,
            None => scale.set_item("range", py.None())?,
        }
        let pane = PyDict::new(py);
        pane.set_item("stretch", f64::from(p.stretch))?;
        pane.set_item("price_scale", scale)?;
        panes.append(pane)?;
    }
    d.set_item("panes", panes)?;
    let drawings = PyList::empty(py);
    for sd in &s.drawings {
        let x = PyDict::new(py);
        x.set_item("pane", sd.pane)?;
        x.set_item("visible", sd.visible)?;
        x.set_item("locked", sd.locked)?;
        x.set_item("data", drawing_data_py(py, &sd.data)?)?;
        drawings.append(x)?;
    }
    d.set_item("drawings", drawings)?;
    Ok(d)
}

pub(crate) fn state(v: &Bound<'_, PyAny>) -> PyResult<ChartState> {
    let d = dict(v, "the state")?;
    check_keys(
        d,
        &[
            "version",
            "bar_spacing",
            "right_offset",
            "crosshair_mode",
            "panes",
            "drawings",
        ],
        "state key",
    )?;
    let version = req_f64(d, "version", "the state")?;
    if !(0.0..=f64::from(u32::MAX)).contains(&version) || version.fract() != 0.0 {
        return Err(err("state: invalid 'version'"));
    }
    let crosshair_mode = match opt_str(d, "crosshair_mode")? {
        Some(m) => params::crosshair_mode(&m)
            .ok_or_else(|| err(format!("state: unknown crosshair mode '{m}'")))?,
        None => CrosshairMode::Normal,
    };
    let item = |k: &str| -> PyResult<Bound<'_, PyAny>> {
        d.get_item(k)?
            .ok_or_else(|| err(format!("the state needs '{k}'")))
    };
    let panes = list(&item("panes")?, "state 'panes'")?
        .iter()
        .map(|p| {
            let p = dict(p, "each pane of the state")?;
            check_keys(p, &["stretch", "price_scale"], "pane state key")?;
            let scale = p
                .get_item("price_scale")?
                .ok_or_else(|| err("a pane state needs 'price_scale'"))?;
            let scale = dict(&scale, "'price_scale'")?;
            check_keys(
                scale,
                &["mode", "inverted", "margins", "range"],
                "price scale state key",
            )?;
            let mode = opt_str(scale, "mode")?.unwrap_or_else(|| "normal".into());
            let mode = params::scale_mode(&camel(&mode))
                .ok_or_else(|| err(format!("state: unknown price scale mode '{mode}'")))?;
            let margins = scale
                .get_item("margins")?
                .ok_or_else(|| err("a price scale state needs 'margins'"))?;
            let margins = pair(&margins, "'margins'")?;
            let range = scale
                .get_item("range")?
                .filter(|r| !r.is_none())
                .map(|r| pair(&r, "'range'"))
                .transpose()?;
            Ok(PaneState {
                stretch: req_f64(p, "stretch", "a pane state")? as f32,
                price_scale: PriceScaleState {
                    mode,
                    inverted: opt_bool(scale, "inverted")?.unwrap_or(false),
                    margins: (margins.0 as f32, margins.1 as f32),
                    range,
                },
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    let drawings = list(&item("drawings")?, "state 'drawings'")?
        .iter()
        .map(|x| {
            let x = dict(x, "each drawing of the state")?;
            check_keys(
                x,
                &["pane", "visible", "locked", "data"],
                "saved drawing key",
            )?;
            let pane = req_f64(x, "pane", "a saved drawing")?;
            if !(0.0..1e6).contains(&pane) || pane.fract() != 0.0 {
                return Err(err("state: invalid drawing pane"));
            }
            let data = x
                .get_item("data")?
                .ok_or_else(|| err("a saved drawing needs 'data'"))?;
            Ok(SavedDrawing {
                pane: pane as usize,
                data: drawing_data(&data)?,
                visible: opt_bool(x, "visible")?.unwrap_or(true),
                locked: opt_bool(x, "locked")?.unwrap_or(false),
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    Ok(ChartState {
        version: version as u32,
        bar_spacing: Css(req_f64(d, "bar_spacing", "the state")? as f32),
        right_offset: req_f64(d, "right_offset", "the state")?,
        crosshair_mode,
        panes,
        drawings,
    })
}

fn drawing_id(id: u64) -> DrawingId {
    DrawingId(id)
}

/// A core error about drawing `id`, naming it.
fn id_err(e: ferrochart_core::Error, id: u64) -> PyErr {
    match e {
        ferrochart_core::Error::UnknownId => err(format!("unknown drawing {id}")),
        e => err(e),
    }
}

/// A core error about a drawing of `kind`, naming an unknown kind.
fn kind_err(e: ferrochart_core::Error, kind: &str) -> PyErr {
    match e {
        ferrochart_core::Error::InvalidArgument(m) if m.contains("kind") => err(format!(
            "unknown drawing kind '{kind}' (see Drawings.kinds())"
        )),
        e => err(e),
    }
}

/// The drawings of a chart. Wrapped by `ferrochart.Drawings`.
#[pyclass(module = "ferrochart._native", frozen)]
pub(crate) struct Drawings {
    pub(crate) shared: Arc<Shared>,
}

impl Drawings {
    fn with<T>(&self, f: impl FnOnce(&mut DrawingManager) -> T) -> T {
        f(self.shared.py_lock().chart.drawings())
    }
}

#[pymethods]
impl Drawings {
    fn kinds(&self) -> Vec<String> {
        self.with(|d| d.kinds().map(str::to_owned).collect())
    }

    #[pyo3(signature = (data, pane = None))]
    fn add(&self, data: &Bound<'_, PyAny>, pane: Option<u32>) -> PyResult<u64> {
        let data = drawing_data(data)?;
        let mut s = self.shared.py_lock();
        let pane = pane.map_or_else(|| s.chart.main_pane(), PaneId);
        s.chart
            .drawings()
            .add(pane, &data)
            .map(|id| id.0)
            .map_err(|e| kind_err(e, &data.kind))
    }

    fn remove(&self, id: u64) -> PyResult<()> {
        self.with(|d| d.remove(drawing_id(id)))
            .map_err(|e| id_err(e, id))
    }

    fn ids(&self) -> Vec<u64> {
        self.with(|d| d.ids().map(|id| id.0).collect())
    }

    fn get<'py>(&self, py: Python<'py>, id: u64) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.with(|d| d.data(drawing_id(id)))
            .map(|data| drawing_data_py(py, &data))
            .transpose()
    }

    fn pane_of(&self, id: u64) -> Option<u32> {
        self.with(|d| d.pane_of(drawing_id(id)).map(|p| p.0))
    }

    fn set_anchors(&self, id: u64, anchors: &Bound<'_, PyAny>) -> PyResult<()> {
        let a = self::anchors(anchors)?;
        self.with(|d| d.set_anchors(drawing_id(id), &a))
            .map_err(|e| id_err(e, id))
    }

    fn set_style(&self, id: u64, style: &Bound<'_, PyDict>) -> PyResult<()> {
        self.with(|d| {
            let base = d
                .data(drawing_id(id))
                .ok_or_else(|| err(format!("unknown drawing {id}")))?
                .style;
            let s = self::style(base, style)?;
            d.set_style(drawing_id(id), s).map_err(|e| id_err(e, id))
        })
    }

    fn set_param(&self, id: u64, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let p = param(value, key)?;
        self.with(|d| d.set_param(drawing_id(id), key, p))
            .map_err(|e| id_err(e, id))
    }

    fn set_visible(&self, id: u64, visible: bool) -> PyResult<()> {
        self.with(|d| d.set_visible(drawing_id(id), visible))
            .map_err(|e| id_err(e, id))
    }

    fn is_visible(&self, id: u64) -> Option<bool> {
        self.with(|d| d.is_visible(drawing_id(id)))
    }

    fn set_locked(&self, id: u64, locked: bool) -> PyResult<()> {
        self.with(|d| d.set_locked(drawing_id(id), locked))
            .map_err(|e| id_err(e, id))
    }

    fn is_locked(&self, id: u64) -> Option<bool> {
        self.with(|d| d.is_locked(drawing_id(id)))
    }

    fn bring_to_front(&self, id: u64) -> PyResult<()> {
        self.with(|d| d.bring_to_front(drawing_id(id)))
            .map_err(|e| id_err(e, id))
    }

    fn send_to_back(&self, id: u64) -> PyResult<()> {
        self.with(|d| d.send_to_back(drawing_id(id)))
            .map_err(|e| id_err(e, id))
    }

    #[pyo3(signature = (id = None))]
    fn select(&self, id: Option<u64>) -> PyResult<()> {
        self.with(|d| d.select(id.map(drawing_id)))
            .map_err(|e| id_err(e, id.unwrap_or_default()))
    }

    #[getter]
    fn selected(&self) -> Option<u64> {
        self.with(|d| d.selected().map(|id| id.0))
    }

    fn start_tool(&self, kind: &str) -> PyResult<()> {
        self.with(|d| d.start_tool(kind))
            .map_err(|e| kind_err(e, kind))
    }

    fn cancel_tool(&self) {
        self.with(DrawingManager::cancel_tool);
    }

    #[getter]
    fn active_tool(&self) -> Option<String> {
        self.with(|d| d.active_tool().map(str::to_owned))
    }

    fn undo(&self) -> bool {
        self.with(DrawingManager::undo)
    }

    fn redo(&self) -> bool {
        self.with(DrawingManager::redo)
    }

    #[getter]
    fn can_undo(&self) -> bool {
        self.with(|d| d.can_undo())
    }

    #[getter]
    fn can_redo(&self) -> bool {
        self.with(|d| d.can_redo())
    }
}
