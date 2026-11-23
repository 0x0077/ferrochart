//! The native `Chart`, `Series`, `Indicator` and `PriceLine` classes.
//!
//! A chart lives in an `Arc<Mutex<…>>`: series and indicators hold the same
//! `Arc`, and a window showing the chart (`show`, another thread in the
//! non-blocking mode) locks it for each frame. Long work (rendering) runs
//! without the GIL.

use std::sync::Arc;

use ferrochart_core::chart::{ChartEvent, PriceLineOptions, SeriesData, SeriesOptions};
use ferrochart_core::data::{
    BarUpdate, IndicatorId, OhlcColumns, PaneId, PriceLineId, SeriesId, ValueColumns,
};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::SizeCss;
use ferrochart_core::pane::PaneOptions;
use ferrochart_core::plugin::{Indicator as _, Placement};
use ferrochart_core::price_scale::ScaleSide;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart as CoreChart, ChartOptions};
use numpy::{PyReadonlyArray1, PyReadonlyArrayDyn};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

use crate::opts;
use crate::png::Renderers;
use crate::shared::{Parts, Scene, check_size};
use crate::{camel, err, params, snake};

/// State behind a chart's lock.
pub(crate) struct State {
    pub(crate) chart: CoreChart,
    /// Size in CSS px and device pixel ratio the chart was last sized to.
    pub(crate) size: SizeCss,
    pub(crate) dpr: f32,
    renderers: Renderers,
}

impl Scene for State {
    type Core = CoreChart;
    type Event = ChartEvent;

    fn parts(&mut self) -> Parts<'_, CoreChart> {
        Parts {
            core: &mut self.chart,
            size: &mut self.size,
            dpr: &mut self.dpr,
            renderers: &mut self.renderers,
        }
    }

    fn take_events(&mut self, out: &mut Vec<ChartEvent>) {
        out.extend(
            self.chart
                .drain_events()
                .filter(|e| crate::events::name(e).is_some()),
        );
    }

    fn deliver(py: Python<'_>, shared: &Shared, events: &[ChartEvent]) {
        crate::events::deliver(py, shared, events);
    }
}

/// A chart shared by its Python objects (and a window showing it).
pub(crate) type Shared = crate::shared::Shared<State>;

pub(crate) fn scale_mode_name(m: ferrochart_core::price_scale::PriceScaleMode) -> &'static str {
    use ferrochart_core::price_scale::PriceScaleMode as M;
    match m {
        M::Normal => "normal",
        M::Logarithmic => "logarithmic",
        M::Percentage => "percentage",
        M::IndexedTo100 => "indexed_to_100",
    }
}

fn kind_name(k: SeriesKind) -> String {
    snake(params::series_kind_name(k))
}

/// A chart. Wrapped by `ferrochart.Chart`.
#[pyclass(module = "ferrochart._native", frozen)]
pub(crate) struct Chart {
    pub(crate) shared: Arc<Shared>,
}

impl Chart {
    fn with<T>(&self, f: impl FnOnce(&mut CoreChart) -> T) -> T {
        f(&mut self.shared.py_lock().chart)
    }

    fn series(&self, id: SeriesId) -> Series {
        Series {
            shared: self.shared.clone(),
            id,
        }
    }
}

fn pane_of(chart: &CoreChart, pane: Option<u32>) -> PaneId {
    pane.map_or_else(|| chart.main_pane(), PaneId)
}

#[pymethods]
impl Chart {
    #[new]
    #[pyo3(signature = (theme = "light", width = 1200.0, height = 720.0, fonts = Vec::new()))]
    fn new(theme: &str, width: f64, height: f64, fonts: Vec<Vec<u8>>) -> PyResult<Self> {
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
        let mut chart = CoreChart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(size, 1.0);
        let renderers = Renderers::with_fonts(fonts).map_err(err)?;
        Ok(Self {
            shared: Shared::new(State {
                chart,
                size,
                dpr: 1.0,
                renderers,
            }),
        })
    }

    /// Opens a window showing the chart (`window.rs`): blocks until it is
    /// closed, or returns at once with `block=False` (Linux and Windows).
    /// `close_after` (seconds) closes it by itself (for tests).
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

    /// Whether the chart is in a window.
    #[getter]
    fn shown(&self) -> bool {
        self.shared.is_shown()
    }

    /// Registers an image for markers: an RGBA (or RGB) `uint8` array of
    /// shape `(height, width, 4)` (or `3`), sides 1–256. Returns its id.
    #[allow(clippy::needless_pass_by_value)] // pyo3 extracts arguments by value
    fn add_icon(&self, image: PyReadonlyArrayDyn<'_, u8>) -> PyResult<u32> {
        let data = image.as_array();
        let [h, w, channels] = data.shape()[..] else {
            return Err(err(
                "the image must be an array of shape (height, width, 4) or (height, width, 3)",
            ));
        };
        if channels != 3 && channels != 4 {
            return Err(err("the image must have 3 (RGB) or 4 (RGBA) channels"));
        }
        // Before copying: the core rejects larger images anyway.
        let side = ferrochart_core::icon::MAX_IMAGE_SIDE as usize;
        if w == 0 || h == 0 || w > side || h > side {
            return Err(err("icon width and height must be 1..=256"));
        }
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                for c in 0..4 {
                    rgba.push(if c < channels { data[[y, x, c]] } else { 255 });
                }
            }
        }
        let (Ok(width), Ok(height)) = (u32::try_from(w), u32::try_from(h)) else {
            return Err(err("icon width and height must be 1..=256"));
        };
        self.with(|c| {
            c.add_icon(ferrochart_core::icon::IconImage {
                width,
                height,
                rgba: &rgba,
            })
        })
        .map(|id| id.0)
        .map_err(err)
    }

    /// Removes an image added with `add_icon`; markers using it draw their
    /// shape only.
    fn remove_icon(&self, id: u32) -> PyResult<()> {
        self.with(|c| c.remove_icon(ferrochart_core::icon::IconId(id)))
            .map_err(err)
    }

    /// Sets `handler(name, payload)` for chart events (None: no handler).
    fn set_event_handler(&self, handler: Option<Py<PyAny>>) {
        self.shared.set_handler(handler);
    }

    /// The series with id `id`, if it exists.
    fn series_by_id(&self, id: u32) -> Option<Series> {
        let id = SeriesId(id);
        self.with(|c| c.series_kind(id).is_some())
            .then(|| self.series(id))
    }

    /// Size in CSS px, `(width, height)`.
    #[getter]
    fn size(&self) -> (f64, f64) {
        self.shared.size()
    }

    /// Sizes the chart (CSS px) for `to_png` without arguments and the
    /// first window.
    fn resize(&self, width: f64, height: f64) -> PyResult<()> {
        self.shared.resize(width, height)
    }

    #[pyo3(signature = (kind, options, pane = None))]
    fn add_series(
        &self,
        kind: &str,
        options: &Bound<'_, PyDict>,
        pane: Option<u32>,
    ) -> PyResult<Series> {
        let k = params::series_kind(&camel(kind)).ok_or_else(|| {
            err(format!(
                "unknown series kind '{kind}' (expected candlestick, ohlc, heikin_ashi, line, \
                 area, baseline, histogram or points)"
            ))
        })?;
        let opts = opts::series_options(SeriesOptions::default(), options)?;
        let id = self
            .with(|c| {
                let pane = pane_of(c, pane);
                c.add_series(pane, k, opts)
            })
            .map_err(err)?;
        Ok(self.series(id))
    }

    #[pyo3(signature = (source, pane = None))]
    fn add_volume_series(&self, source: &Series, pane: Option<u32>) -> PyResult<Series> {
        let id = self
            .with(|c| {
                let pane = pane_of(c, pane);
                c.add_series(
                    pane,
                    SeriesKind::Volume,
                    SeriesOptions::volume_of(source.id),
                )
            })
            .map_err(err)?;
        Ok(self.series(id))
    }

    #[pyo3(signature = (name, params, options, pane = None))]
    fn add_transform_series(
        &self,
        name: &str,
        params: &Bound<'_, PyDict>,
        options: &Bound<'_, PyDict>,
        pane: Option<u32>,
    ) -> PyResult<Series> {
        let t = opts::build_params(params, &format!("transform '{name}'"), |get| {
            crate::params::transform(&camel(name), get)
        })?;
        let opts = opts::series_options(SeriesOptions::default(), options)?;
        let id = self
            .with(|c| {
                let pane = pane_of(c, pane);
                c.add_transform_series(pane, t, opts)
            })
            .map_err(err)?;
        Ok(self.series(id))
    }

    #[getter]
    fn main_pane(&self) -> u32 {
        self.with(|c| c.main_pane().0)
    }

    #[pyo3(signature = (stretch = 1.0))]
    fn add_pane(&self, stretch: f32) -> PyResult<u32> {
        if !(stretch.is_finite() && stretch > 0.0) {
            return Err(err("'stretch' must be positive"));
        }
        Ok(self.with(|c| {
            c.add_pane(PaneOptions {
                stretch_factor: stretch,
            })
            .0
        }))
    }

    fn remove_pane(&self, pane: u32) -> PyResult<()> {
        self.with(|c| c.remove_pane(PaneId(pane))).map_err(err)
    }

    /// Pane ids, top to bottom.
    #[getter]
    fn panes(&self) -> Vec<u32> {
        self.with(|c| c.panes().map(|p| p.0).collect())
    }

    #[pyo3(signature = (source, name, params, placement = None))]
    fn add_indicator(
        &self,
        source: &Series,
        name: &str,
        params: &Bound<'_, PyDict>,
        placement: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Indicator> {
        let ind = opts::build_params(params, &format!("indicator '{name}'"), |get| {
            crate::params::indicator(&camel(name), get)
        })?;
        let placement = match placement {
            None => None,
            Some(p) if p.is_none() => None,
            Some(p) => Some(if let Ok(s) = p.extract::<String>() {
                match s.as_str() {
                    "overlay" => Placement::Overlay,
                    "new_pane" => Placement::NewPane,
                    "volume" => Placement::Volume,
                    _ => {
                        return Err(err(
                            "placement must be 'overlay', 'new_pane', 'volume', a pane id or None",
                        ));
                    }
                }
            } else {
                Placement::Pane(PaneId(p.extract::<u32>().map_err(|_| {
                    err("placement must be 'overlay', 'new_pane', 'volume', a pane id or None")
                })?))
            }),
        };
        let placement = placement.unwrap_or_else(|| ind.default_placement());
        let id = self
            .with(|c| c.add_indicator(source.id, Box::new(ind), placement))
            .map_err(err)?;
        Ok(Indicator {
            shared: self.shared.clone(),
            id,
        })
    }

    /// Options of the right price scale of `pane` (default: main pane).
    #[pyo3(signature = (pane = None))]
    fn price_scale<'py>(&self, py: Python<'py>, pane: Option<u32>) -> PyResult<Bound<'py, PyDict>> {
        let (mode, auto, inverted, margins, range) = self
            .with(|c| {
                let pane = pane_of(c, pane);
                let s = c.price_scale(pane, ScaleSide::Right)?;
                Some((
                    s.mode(),
                    s.auto_scale(),
                    s.inverted(),
                    s.margins(),
                    s.range(),
                ))
            })
            .ok_or_else(|| err("unknown pane"))?;
        let d = PyDict::new(py);
        d.set_item("mode", scale_mode_name(mode))?;
        d.set_item("auto_scale", auto)?;
        d.set_item("inverted", inverted)?;
        d.set_item("margins", (f64::from(margins.0), f64::from(margins.1)))?;
        d.set_item("range", range)?;
        Ok(d)
    }

    /// Changes the right price scale of `pane`: `mode` ('normal',
    /// 'logarithmic', 'percentage', `'indexed_to_100'`), `auto_scale`,
    /// `inverted`, `margins` (top, bottom fractions), `range` (low, high).
    #[pyo3(signature = (options, pane = None))]
    fn apply_price_scale_options(
        &self,
        options: &Bound<'_, PyDict>,
        pane: Option<u32>,
    ) -> PyResult<()> {
        opts::check_keys(
            options,
            &["mode", "auto_scale", "inverted", "margins", "range"],
            "price scale option",
        )?;
        let mode = opts::opt_str(options, "mode")?
            .map(|m| {
                params::scale_mode(&camel(&m)).ok_or_else(|| {
                    err(format!(
                        "unknown mode '{m}' (expected normal, logarithmic, percentage or indexed_to_100)"
                    ))
                })
            })
            .transpose()?;
        let auto = opts::opt_bool(options, "auto_scale")?;
        let inverted = opts::opt_bool(options, "inverted")?;
        let pair = |k: &str| -> PyResult<Option<(f64, f64)>> {
            options
                .get_item(k)?
                .filter(|v| !v.is_none())
                .map(|v| {
                    v.extract::<(f64, f64)>()
                        .map_err(|_| err(format!("'{k}' must be a pair of numbers")))
                })
                .transpose()
        };
        let margins = pair("margins")?;
        let range = pair("range")?;
        self.with(|c| -> PyResult<()> {
            let pane = pane_of(c, pane);
            let s = c
                .price_scale(pane, ScaleSide::Right)
                .ok_or_else(|| err("unknown pane"))?;
            if let Some(m) = mode {
                s.set_mode(m).map_err(err)?;
            }
            if let Some((t, b)) = margins {
                s.set_margins(t as f32, b as f32).map_err(err)?;
            }
            if let Some(i) = inverted {
                s.set_inverted(i);
            }
            if let Some((lo, hi)) = range {
                s.set_range(lo, hi).map_err(err)?;
            }
            if let Some(a) = auto {
                s.set_auto_scale(a);
            }
            Ok(())
        })
    }

    fn fit_content(&self) {
        self.with(CoreChart::fit_content);
    }

    fn scroll_to_realtime(&self) {
        self.with(CoreChart::scroll_to_realtime);
    }

    /// Bar spacing in CSS px.
    #[getter]
    fn bar_spacing(&self) -> f64 {
        let mut s = self.shared.py_lock();
        let dpr = f64::from(s.dpr);
        s.chart.time_scale().bar_spacing() / dpr
    }

    fn set_bar_spacing(&self, spacing: f64) -> PyResult<()> {
        if !(spacing.is_finite() && spacing > 0.0) {
            return Err(err("the bar spacing must be positive"));
        }
        let mut s = self.shared.py_lock();
        let dpr = f64::from(s.dpr);
        s.chart.time_scale().set_bar_spacing(spacing * dpr);
        Ok(())
    }

    /// Empty space right of the last bar, in bars.
    #[getter]
    fn right_offset(&self) -> f64 {
        self.with(|c| c.time_scale().right_offset())
    }

    fn set_right_offset(&self, bars: f64) -> PyResult<()> {
        if !bars.is_finite() {
            return Err(err("the offset must be finite"));
        }
        self.with(|c| c.time_scale().set_right_offset(bars));
        Ok(())
    }

    /// Logical indices of the first and last bar, `None` without data.
    fn data_range(&self) -> Option<(i64, i64)> {
        self.with(|c| {
            let index = c.data_view().time_index();
            (!index.is_empty()).then(|| (index.first_index(), index.end_index() - 1))
        })
    }

    /// Visible logical range `(from, to)` (plot edges, fractional).
    fn visible_logical_range(&self) -> (f64, f64) {
        self.with(|c| c.time_scale().visible_range())
    }

    /// Shows logical indices `from_..to` across the plot (applied once the
    /// plot width is known).
    fn set_visible_logical_range(&self, from_: f64, to: f64) -> PyResult<()> {
        self.with(|c| c.time_scale().set_visible_range(from_, to))
            .map_err(err)
    }

    #[getter]
    fn crosshair_mode(&self) -> String {
        self.with(|c| params::crosshair_mode_name(c.crosshair_mode()).to_owned())
    }

    fn set_crosshair_mode(&self, mode: &str) -> PyResult<()> {
        let m = params::crosshair_mode(mode)
            .ok_or_else(|| err("crosshair mode must be 'normal', 'magnet' or 'hidden'"))?;
        self.with(|c| c.set_crosshair_mode(m));
        Ok(())
    }

    /// The drawings of the chart.
    #[getter]
    fn drawings(&self) -> crate::drawings::Drawings {
        crate::drawings::Drawings {
            shared: self.shared.clone(),
        }
    }

    /// The view, layout and drawings as a dict (`drawings.rs`); data, series
    /// and indicators are not included.
    fn save_state<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let s = self.with(|c| c.save_state());
        crate::drawings::state_py(py, &s)
    }

    /// Loads a state from `save_state` into a chart with the same panes; the
    /// chart is unchanged if it does not fit.
    fn load_state(&self, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let s = crate::drawings::state(state)?;
        self.with(|c| c.load_state(s)).map_err(err)
    }

    /// Renders an image of `width` × `height` CSS px (default: the chart's
    /// size) at `scale` device pixels per CSS px. Returns `(png, backend)`.
    /// The chart keeps its size.
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
}

/// A series. Wrapped by `ferrochart.Series`.
#[pyclass(module = "ferrochart._native", frozen)]
pub(crate) struct Series {
    shared: Arc<Shared>,
    pub(crate) id: SeriesId,
}

pub(crate) fn column<T: numpy::Element + Copy>(
    a: &PyReadonlyArray1<'_, T>,
    name: &str,
) -> PyResult<Vec<T>> {
    // One memcpy; the Python side made the array contiguous.
    a.as_slice()
        .map(<[T]>::to_vec)
        .map_err(|_| err(format!("column '{name}' is not contiguous")))
}

pub(crate) fn color_column(colors: Option<Vec<Option<String>>>) -> PyResult<Option<Vec<u32>>> {
    colors
        .map(|cs| {
            cs.iter()
                .map(|c| match c {
                    // 0: the series color.
                    None => Ok(0),
                    Some(s) => {
                        let c = opts::color(s, "color")?;
                        Ok(u32::from_be_bytes([c.r, c.g, c.b, c.a]))
                    }
                })
                .collect()
        })
        .transpose()
}

impl Series {
    fn with<T>(&self, f: impl FnOnce(&mut CoreChart) -> T) -> T {
        f(&mut self.shared.py_lock().chart)
    }
}

#[pymethods]
impl Series {
    #[getter]
    fn id(&self) -> u32 {
        self.id.0
    }

    #[getter]
    fn kind(&self) -> PyResult<String> {
        self.with(|c| c.series_kind(self.id))
            .map(kind_name)
            .ok_or_else(|| err("the series was removed"))
    }

    /// Whether the series takes `time`/`value` columns (else OHLC).
    #[getter]
    fn is_value(&self) -> bool {
        self.with(|c| c.series_kind(self.id)).is_some_and(|k| {
            matches!(
                k,
                SeriesKind::Line
                    | SeriesKind::Area
                    | SeriesKind::Baseline
                    | SeriesKind::Histogram
                    | SeriesKind::Points
            )
        })
    }

    #[getter]
    fn pane(&self) -> PyResult<u32> {
        self.with(|c| c.series_pane(self.id))
            .map(|p| p.0)
            .ok_or_else(|| err("the series was removed"))
    }

    /// Replaces (`append=False`) or extends the data with OHLC columns.
    #[pyo3(signature = (time, open, high, low, close, volume = None, append = false))]
    // pyo3 extracts arguments by value.
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    fn set_ohlc(
        &self,
        py: Python<'_>,
        time: PyReadonlyArray1<'_, i64>,
        open: PyReadonlyArray1<'_, f64>,
        high: PyReadonlyArray1<'_, f64>,
        low: PyReadonlyArray1<'_, f64>,
        close: PyReadonlyArray1<'_, f64>,
        volume: Option<PyReadonlyArray1<'_, f64>>,
        append: bool,
    ) -> PyResult<()> {
        let columns = OhlcColumns {
            time: column(&time, "time")?,
            open: column(&open, "open")?,
            high: column(&high, "high")?,
            low: column(&low, "low")?,
            close: column(&close, "close")?,
            volume: volume.map(|v| column(&v, "volume")).transpose()?,
        };
        let (shared, id) = (self.shared.clone(), self.id);
        py.detach(move || {
            let chart = &mut shared.py_lock().chart;
            if append {
                chart.append(id, SeriesData::Ohlc(columns))
            } else {
                chart.set_data(id, SeriesData::Ohlc(columns))
            }
        })
        .map_err(err)
    }

    /// Replaces or extends the data with `time`/`value` columns and
    /// optional per-point colors (`None`: the series color). Only histogram
    /// series draw per-point colors, and only at full detail; zoomed out far
    /// enough that bars are merged, and for other kinds, the series color
    /// is used.
    #[pyo3(signature = (time, value, color = None, append = false))]
    #[allow(clippy::needless_pass_by_value)]
    fn set_values(
        &self,
        py: Python<'_>,
        time: PyReadonlyArray1<'_, i64>,
        value: PyReadonlyArray1<'_, f64>,
        color: Option<Vec<Option<String>>>,
        append: bool,
    ) -> PyResult<()> {
        let columns = ValueColumns {
            time: column(&time, "time")?,
            value: column(&value, "value")?,
            color: color_column(color)?,
        };
        let (shared, id) = (self.shared.clone(), self.id);
        py.detach(move || {
            let chart = &mut shared.py_lock().chart;
            if append {
                chart.append(id, SeriesData::Value(columns))
            } else {
                chart.set_data(id, SeriesData::Value(columns))
            }
        })
        .map_err(err)
    }

    /// Updates the last bar (same time) or appends one (later time).
    /// Single-value series take the value from `close`.
    #[pyo3(signature = (time, open, high, low, close, volume = None))]
    fn update_last(
        &self,
        time: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: Option<f64>,
    ) -> PyResult<()> {
        let bar = BarUpdate {
            time,
            open,
            high,
            low,
            close,
            volume,
        };
        self.with(|c| c.update_last(self.id, bar))
            .map(|_| ())
            .map_err(err)
    }

    fn options<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let o = self
            .with(|c| c.series_options(self.id).copied())
            .ok_or_else(|| err("the series was removed"))?;
        opts::series_options_py(py, &o)
    }

    fn apply_options(&self, options: &Bound<'_, PyDict>) -> PyResult<()> {
        let base = self
            .with(|c| c.series_options(self.id).copied())
            .ok_or_else(|| err("the series was removed"))?;
        let o = opts::series_options(base, options)?;
        self.with(|c| c.set_series_options(self.id, o)).map_err(err)
    }

    fn create_price_line(&self, options: &Bound<'_, PyDict>) -> PyResult<PriceLine> {
        let o = opts::price_line_options(PriceLineOptions::default(), options)?;
        let id = self
            .with(|c| c.create_price_line(self.id, o))
            .map_err(err)?;
        Ok(PriceLine {
            shared: self.shared.clone(),
            id,
        })
    }

    /// Bar quality flags (r22): `times` (ms) and `flags` (bit 0
    /// filled, bit 1 clipped), the same length.
    fn set_bar_flags(&self, times: Vec<i64>, flags: Vec<u8>) -> PyResult<()> {
        use ferrochart_core::chart::BarFlags;
        if times.len() != flags.len() {
            return Err(err("times and flags differ in length"));
        }
        let f = times
            .into_iter()
            .zip(flags)
            .map(|(t, f)| (t, BarFlags(f)))
            .collect();
        self.with(|c| c.set_bar_flags(self.id, f).map_err(err))
    }

    fn set_markers(&self, markers: &Bound<'_, PyList>) -> PyResult<()> {
        let m = opts::markers(markers)?;
        self.with(|c| {
            let icons = c.data_view().icons();
            if let Some(id) = m
                .iter()
                .filter_map(|m| m.icon)
                .find(|&id| icons.get(id).is_none())
            {
                return Err(err(format!(
                    "unknown icon {} (add it with Chart.add_icon)",
                    id.0
                )));
            }
            c.set_markers(self.id, m).map_err(err)
        })
    }

    fn markers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let m = self
            .with(|c| c.markers(self.id).map(<[_]>::to_vec))
            .ok_or_else(|| err("the series was removed"))?;
        opts::markers_py(py, &m)
    }

    fn remove(&self) -> PyResult<()> {
        self.with(|c| c.remove_series(self.id)).map_err(err)
    }

    fn __eq__(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared) && self.id == other.id
    }

    fn __hash__(&self) -> u64 {
        u64::from(self.id.0)
    }
}

/// An indicator.
#[pyclass(module = "ferrochart._native", frozen)]
pub(crate) struct Indicator {
    shared: Arc<Shared>,
    id: IndicatorId,
}

#[pymethods]
impl Indicator {
    #[getter]
    fn id(&self) -> u32 {
        self.id.0
    }

    /// The series it draws (lines, histograms, points).
    #[getter]
    fn outputs(&self) -> PyResult<Vec<Series>> {
        let ids = self
            .shared
            .lock()
            .chart
            .indicator_series(self.id)
            .map(<[_]>::to_vec)
            .ok_or_else(|| err("the indicator was removed"))?;
        Ok(ids
            .into_iter()
            .map(|id| Series {
                shared: self.shared.clone(),
                id,
            })
            .collect())
    }

    fn remove(&self) -> PyResult<()> {
        self.shared
            .py_lock()
            .chart
            .remove_indicator(self.id)
            .map_err(err)
    }
}

/// A horizontal price line of a series.
#[pyclass(module = "ferrochart._native", frozen)]
pub(crate) struct PriceLine {
    shared: Arc<Shared>,
    id: PriceLineId,
}

#[pymethods]
impl PriceLine {
    fn options<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let o = self
            .shared
            .lock()
            .chart
            .price_line(self.id)
            .cloned()
            .ok_or_else(|| err("the price line was removed"))?;
        opts::price_line_options_py(py, &o)
    }

    fn apply_options(&self, options: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut s = self.shared.py_lock();
        let base = s
            .chart
            .price_line(self.id)
            .cloned()
            .ok_or_else(|| err("the price line was removed"))?;
        let o = opts::price_line_options(base, options)?;
        s.chart.update_price_line(self.id, o).map_err(err)
    }

    fn remove(&self) -> PyResult<()> {
        self.shared
            .py_lock()
            .chart
            .remove_price_line(self.id)
            .map_err(err)
    }
}
