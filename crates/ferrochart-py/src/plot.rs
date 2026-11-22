//! The native `Plot` and `Mark` classes: a
//! `ferrochart_plot::Plot` behind the same shared lock as charts
//! (`shared.rs`), so `show`, `to_png` and events work the same way. The
//! Python package wraps them (`ferrochart.Plot`); axes, rules and labels are
//! addressed by name or id here and get their Python classes there.

use std::sync::Arc;

use ferrochart_core::geom::SizeCss;
use ferrochart_core::theme::Theme;
use ferrochart_plot::{
    AxisId, Hit, LabelId, MarkData, MarkId, MarkOptions, MarkPoint, Plot as CorePlot, PlotEvent,
    PlotOptions, RuleId, RuleOptions,
};
use numpy::PyReadonlyArray1;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use crate::chart::{color_column, column};
use crate::err;
use crate::plot_opts as o;
use crate::plot_params as names;
use crate::png::Renderers;
use crate::shared::{Parts, Scene, check_size};

/// State behind a plot's lock.
pub(crate) struct PlotState {
    plot: CorePlot,
    size: SizeCss,
    dpr: f32,
    renderers: Renderers,
    /// Options of the rules (the plot has no getter).
    rules: Vec<(RuleId, RuleOptions)>,
}

/// A plot shared by its Python objects (and a window showing it).
pub(crate) type PlotShared = crate::shared::Shared<PlotState>;

impl Scene for PlotState {
    type Core = CorePlot;
    type Event = PlotEvent;

    fn parts(&mut self) -> Parts<'_, CorePlot> {
        Parts {
            core: &mut self.plot,
            size: &mut self.size,
            dpr: &mut self.dpr,
            renderers: &mut self.renderers,
        }
    }

    fn take_events(&mut self, out: &mut Vec<PlotEvent>) {
        out.extend(self.plot.drain_events());
    }

    fn deliver(py: Python<'_>, shared: &PlotShared, events: &[PlotEvent]) {
        let Some(handler) = shared.handler(py) else {
            return;
        };
        for e in events {
            let result = event_py(py, e).and_then(|(n, p)| handler.call1(py, (n, p)).map(drop));
            if let Err(e) = result {
                e.print(py);
            }
        }
    }
}

/// The Python name and payload of `e` (marks as ids; the Python `Plot`
/// turns them into `Mark`s).
fn event_py<'py>(py: Python<'py>, e: &PlotEvent) -> PyResult<(&'static str, Bound<'py, PyDict>)> {
    let d = PyDict::new(py);
    let name = match *e {
        PlotEvent::Hover(hit) => {
            d.set_item("hit", hit_py(py, hit)?)?;
            "hover"
        }
        PlotEvent::Click { hit, x, y } => {
            d.set_item("hit", hit_py(py, hit)?)?;
            d.set_item("x", x)?;
            d.set_item("y", y)?;
            "click"
        }
        PlotEvent::ViewChanged => "view_change",
    };
    Ok((name, d))
}

fn hit_py(py: Python<'_>, hit: Option<Hit>) -> PyResult<Option<Bound<'_, PyDict>>> {
    let Some(h) = hit else {
        return Ok(None);
    };
    let d = PyDict::new(py);
    d.set_item("mark", h.mark.0)?;
    d.set_item("index", h.index)?;
    d.set_item("x", h.x)?;
    d.set_item("y", h.y)?;
    let pos = PyDict::new(py);
    pos.set_item("x", f64::from(h.pos.x))?;
    pos.set_item("y", f64::from(h.pos.y))?;
    d.set_item("position", pos)?;
    Ok(Some(d))
}

fn core_err(e: ferrochart_core::Error) -> PyErr {
    err(e)
}

fn axis_of(name: &str) -> PyResult<AxisId> {
    names::axis_id(name).ok_or_else(|| {
        err(format!(
            "unknown axis '{name}' (expected 'x', 'left' or 'right')"
        ))
    })
}

/// A plot with numeric, category or date axes. Wrapped by
/// `ferrochart.Plot`.
#[pyclass(module = "ferrochart._native", frozen)]
pub(crate) struct Plot {
    shared: Arc<PlotShared>,
}

impl Plot {
    fn with<T>(&self, f: impl FnOnce(&mut CorePlot) -> T) -> T {
        f(&mut self.shared.py_lock().plot)
    }

    fn mark(&self, id: MarkId) -> Mark {
        Mark {
            shared: self.shared.clone(),
            id,
        }
    }
}

#[pymethods]
impl Plot {
    #[new]
    #[pyo3(signature = (theme = "light", width = 800.0, height = 500.0, crosshair = false, hover_highlight = true))]
    fn new(
        theme: &str,
        width: f64,
        height: f64,
        crosshair: bool,
        hover_highlight: bool,
    ) -> PyResult<Self> {
        let theme = match theme {
            "light" => Theme::light(),
            "dark" => Theme::dark(),
            other => {
                return Err(err(format!(
                    "unknown theme '{other}' (expected 'light' or 'dark')"
                )));
            }
        };
        let size = check_size(width, height)?;
        let mut plot = CorePlot::new(PlotOptions {
            theme,
            crosshair,
            hover_highlight,
            ..PlotOptions::default()
        });
        plot.resize(size, 1.0);
        Ok(Self {
            shared: crate::shared::Shared::new(PlotState {
                plot,
                size,
                dpr: 1.0,
                renderers: Renderers::default(),
                rules: Vec::new(),
            }),
        })
    }

    /// Opens a window showing the plot, like `Chart.show`.
    #[pyo3(signature = (block = true, title = "FerroChart", width = None, height = None, close_after = None))]
    fn show(
        &self,
        py: Python<'_>,
        block: bool,
        title: &str,
        width: Option<f64>,
        height: Option<f64>,
        close_after: Option<f64>,
    ) -> PyResult<()> {
        self.shared
            .show(py, block, title, width, height, close_after)
    }

    /// Whether the plot is in a window.
    #[getter]
    fn shown(&self) -> bool {
        self.shared.is_shown()
    }

    /// Sets `handler(name, payload)` for plot events (None: no handler).
    fn set_event_handler(&self, handler: Option<Py<PyAny>>) {
        self.shared.set_handler(handler);
    }

    /// Size in CSS px, `(width, height)`.
    #[getter]
    fn size(&self) -> (f64, f64) {
        self.shared.size()
    }

    fn resize(&self, width: f64, height: f64) -> PyResult<()> {
        self.shared.resize(width, height)
    }

    /// `(png, backend)`, like `Chart.render_png`.
    #[pyo3(signature = (width = None, height = None, scale = 1.0, backend = "auto"))]
    fn render_png<'py>(
        &self,
        py: Python<'py>,
        width: Option<f64>,
        height: Option<f64>,
        scale: f64,
        backend: &str,
    ) -> PyResult<(Bound<'py, PyBytes>, &'static str)> {
        self.shared.render_png(py, width, height, scale, backend)
    }

    /// Adds a mark drawn above the earlier ones.
    fn add_mark(&self, kind: &str, options: &Bound<'_, PyDict>) -> PyResult<Mark> {
        let k = names::mark_kind(kind).ok_or_else(|| {
            err(format!(
                "unknown mark kind '{kind}' (expected 'line', 'area', 'scatter', 'bars', \
                 'waterfall' or 'cells')"
            ))
        })?;
        let theme = self.with(|p| *p.theme());
        let opts = o::mark_options(MarkOptions::default(), options, &theme)?;
        let id = self.with(|p| p.add_mark(k, opts)).map_err(core_err)?;
        Ok(self.mark(id))
    }

    /// The mark with id `id`, if it exists.
    fn mark_by_id(&self, id: u32) -> Option<Mark> {
        let id = MarkId(id);
        self.with(|p| p.mark_kind(id).is_some())
            .then(|| self.mark(id))
    }

    /// Ids of the marks, bottom to top.
    fn mark_ids(&self) -> Vec<u32> {
        self.with(|p| p.marks().map(|m| m.0).collect())
    }

    fn axis_options<'py>(&self, py: Python<'py>, axis: &str) -> PyResult<Bound<'py, PyDict>> {
        let id = axis_of(axis)?;
        let a = self.with(|p| p.axis(id).clone());
        o::axis_options_py(py, &a)
    }

    fn apply_axis_options(&self, axis: &str, options: &Bound<'_, PyDict>) -> PyResult<()> {
        let id = axis_of(axis)?;
        let base = self.with(|p| p.axis(id).clone());
        let opts = o::axis_options(&base, options)?;
        self.with(|p| p.set_axis(id, opts)).map_err(core_err)
    }

    #[allow(clippy::needless_pass_by_value)] // pyo3 extracts lists by value
    fn set_categories(&self, axis: &str, labels: Vec<String>) -> PyResult<()> {
        let id = axis_of(axis)?;
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        self.with(|p| p.set_categories(id, &refs)).map_err(core_err)
    }

    fn categories(&self, axis: &str) -> PyResult<Vec<String>> {
        let id = axis_of(axis)?;
        Ok(self.with(|p| p.categories(id).to_vec()))
    }

    /// The visible range `(lo, hi)` as of the last frame.
    fn view(&self, axis: &str) -> PyResult<(f64, f64)> {
        let id = axis_of(axis)?;
        Ok(self.with(|p| p.view(id)))
    }

    fn set_view(&self, axis: &str, lo: f64, hi: f64) -> PyResult<()> {
        let id = axis_of(axis)?;
        self.with(|p| p.set_view(id, lo, hi)).map_err(core_err)
    }

    fn add_rule(&self, options: &Bound<'_, PyDict>) -> PyResult<u32> {
        let opts = o::rule_options(RuleOptions::default(), options)?;
        let mut s = self.shared.py_lock();
        let id = s.plot.add_rule(opts.clone()).map_err(core_err)?;
        s.rules.push((id, opts));
        Ok(id.0)
    }

    fn rule_options<'py>(&self, py: Python<'py>, id: u32) -> PyResult<Bound<'py, PyDict>> {
        let r = {
            let s = self.shared.py_lock();
            s.rules
                .iter()
                .find(|r| r.0 == RuleId(id))
                .map(|r| r.1.clone())
        };
        o::rule_options_py(py, &r.ok_or_else(|| err("the rule was removed"))?)
    }

    fn apply_rule_options(&self, id: u32, options: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut guard = self.shared.py_lock();
        let s = &mut *guard;
        let k = s
            .rules
            .iter()
            .position(|r| r.0 == RuleId(id))
            .ok_or_else(|| err("the rule was removed"))?;
        let opts = o::rule_options(s.rules[k].1.clone(), options)?;
        s.plot
            .update_rule(RuleId(id), opts.clone())
            .map_err(core_err)?;
        s.rules[k].1 = opts;
        Ok(())
    }

    fn remove_rule(&self, id: u32) -> PyResult<()> {
        let mut s = self.shared.py_lock();
        s.rules.retain(|r| r.0 != RuleId(id));
        s.plot.remove_rule(RuleId(id)).map_err(core_err)
    }

    fn add_label(&self, options: &Bound<'_, PyDict>) -> PyResult<u32> {
        let opts = o::label_options(options)?;
        self.with(|p| p.add_label(opts))
            .map(|id| id.0)
            .map_err(core_err)
    }

    fn remove_label(&self, id: u32) -> PyResult<()> {
        self.with(|p| p.remove_label(LabelId(id))).map_err(core_err)
    }

    /// Shows all data: automatic axes follow the data again.
    fn fit(&self) {
        self.with(CorePlot::fit);
    }
}

/// A mark of a plot. Wrapped by `ferrochart.Mark`.
#[pyclass(module = "ferrochart._native", frozen)]
pub(crate) struct Mark {
    shared: Arc<PlotShared>,
    id: MarkId,
}

impl Mark {
    fn with<T>(&self, f: impl FnOnce(&mut CorePlot) -> T) -> T {
        f(&mut self.shared.py_lock().plot)
    }
}

/// An optional float64 column.
type Col<'py> = Option<PyReadonlyArray1<'py, f64>>;

#[pymethods]
impl Mark {
    #[getter]
    fn id(&self) -> u32 {
        self.id.0
    }

    #[getter]
    fn kind(&self) -> PyResult<String> {
        self.with(|p| p.mark_kind(self.id))
            .map(|k| names::mark_kind_name(k).to_owned())
            .ok_or_else(|| err("the mark was removed"))
    }

    /// Replaces (or with `append`, extends) the data. Columns are
    /// contiguous float64 arrays of the same length (the Python `Mark`
    /// makes them); `color` is CSS strings (None: the mark color), `step`
    /// 'delta', 'subtotal' or 'total', `label` strings or None.
    #[pyo3(signature = (x, y, y0 = None, value = None, size = None, color = None, step = None, label = None, append = false))]
    // pyo3 extracts arguments by value.
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    fn set_data(
        &self,
        x: PyReadonlyArray1<'_, f64>,
        y: PyReadonlyArray1<'_, f64>,
        y0: Col<'_>,
        value: Col<'_>,
        size: Col<'_>,
        color: Option<Vec<Option<String>>>,
        step: Option<Vec<String>>,
        label: Option<Vec<Option<String>>>,
        append: bool,
    ) -> PyResult<()> {
        let opt = |c: &Col<'_>, n: &str| c.as_ref().map(|a| column(a, n)).transpose();
        let mut d = MarkData::xy(column(&x, "x")?, column(&y, "y")?);
        d.y0 = opt(&y0, "y0")?;
        d.value = opt(&value, "value")?;
        d.size = opt(&size, "size")?.map(|v| v.into_iter().map(|s| s as f32).collect());
        d.color = color_column(color)?;
        d.step = step
            .map(|steps| {
                steps
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        names::waterfall_step(s).ok_or_else(|| {
                            err(format!("step[{i}] must be 'delta', 'subtotal' or 'total'"))
                        })
                    })
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        d.label = label.map(|l| l.into_iter().map(Option::unwrap_or_default).collect());
        let id = self.id;
        self.with(|p| {
            if append {
                p.append(id, d)
            } else {
                p.set_data(id, d)
            }
        })
        .map_err(core_err)
    }

    /// Replaces the last point (a live value).
    #[pyo3(signature = (x, y, y0 = None, value = None, size = None, color = None))]
    fn update_last(
        &self,
        x: f64,
        y: f64,
        y0: Option<f64>,
        value: Option<f64>,
        size: Option<f64>,
        color: Option<String>,
    ) -> PyResult<()> {
        let mut pt = MarkPoint::xy(x, y);
        if let Some(v) = y0 {
            pt.y0 = v;
        }
        if let Some(v) = value {
            pt.value = v;
        }
        if let Some(v) = size {
            pt.size = v as f32;
        }
        if let Some(c) = color {
            let c = crate::opts::color(&c, "color")?;
            pt.color = u32::from_be_bytes([c.r, c.g, c.b, c.a]);
        }
        self.with(|p| p.update_last(self.id, pt)).map_err(core_err)
    }

    fn options<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let m = self
            .with(|p| p.mark_options(self.id).cloned())
            .ok_or_else(|| err("the mark was removed"))?;
        o::mark_options_py(py, &m)
    }

    fn apply_options(&self, options: &Bound<'_, PyDict>) -> PyResult<()> {
        let (base, theme) = self.with(|p| (p.mark_options(self.id).cloned(), *p.theme()));
        let base = base.ok_or_else(|| err("the mark was removed"))?;
        let opts = o::mark_options(base, options, &theme)?;
        self.with(|p| p.set_mark_options(self.id, opts))
            .map_err(core_err)
    }

    fn remove(&self) -> PyResult<()> {
        self.with(|p| p.remove_mark(self.id)).map_err(core_err)
    }

    fn __eq__(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared) && self.id == other.id
    }

    fn __hash__(&self) -> u64 {
        (Arc::as_ptr(&self.shared) as u64) ^ u64::from(self.id.0)
    }
}
