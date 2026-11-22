//! The browser chart: `FerroChart` and
//! `Series`. The canvas, renderer, frame loop and input are in `host`, shared
//! with `FerroPlot`; `ChartScene` is the chart's side of it.
//!
//! Doc comments here become the JS API docs: they use JSDoc tags and describe
//! errors as rejected promises / thrown exceptions, hence the allowed lints.
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]

use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use ferrochart_core::chart::{
    ChartDataView, ChartEvent, ClickTarget, CrosshairPoint, SeriesData, WarningKind,
};
use ferrochart_core::data::{BarUpdate, OhlcColumns, SeriesId, ValueColumns};
use ferrochart_core::frame::{Frame, Invalidation, Reveal, SeriesKind};
use ferrochart_core::geom::{Css, PointPx, SizeCss};
use ferrochart_core::interaction::{CrosshairMode, InputEvent, InputResult};
use ferrochart_core::render::Renderer;
use ferrochart_core::sync::DrawingSyncOp;
use ferrochart_core::sync::drawings::MAX_OPS;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_wgpu::WgpuRenderer;
use js_sys::{Array, BigInt64Array, Float64Array, Function, Object, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::convert;
use crate::host::{self, Host, Scene, State};
pub(crate) use crate::host::{js_err, schedule, set};
use crate::plot_api::PlotScene;

/// A chart's host.
pub(crate) type Shared = Host<ChartScene>;
/// Charts of a sync group in this page.
pub(crate) type Peers = host::Peers<ChartScene>;

/// The chart and what the binding keeps next to it.
pub(crate) struct ChartScene {
    pub(crate) chart: Chart,
    /// Drawing sync operations taken from the chart after each frame, for
    /// `takeDrawingSyncOps` (at most `MAX_OPS`).
    pub(crate) sync_ops: VecDeque<DrawingSyncOp>,
    /// `drawingSync` was dispatched and `sync_ops` not taken since.
    pub(crate) sync_ops_reported: bool,
    /// Plots attached with `attachSidePlot`, drawn right after the chart.
    pub(crate) side_plots: Vec<Weak<Host<PlotScene>>>,
}

impl Scene for ChartScene {
    const NAME: &'static str = "FerroChart";
    const EVENTS: &'static [&'static str] = &[
        "click",
        "crosshairMove",
        "visibleRangeChanged",
        "needMoreHistory",
        "drawingCreated",
        "drawingModified",
        "drawingRemoved",
        "drawingSelected",
        "sync",
        "drawingSync",
        "warning",
    ];

    fn resize(&mut self, size: SizeCss, dpr: f32) {
        self.chart.resize(size, dpr);
    }

    fn handle_input(&mut self, event: InputEvent) -> InputResult {
        self.chart.handle_input(event)
    }

    fn needs_redraw(&self) -> Invalidation {
        self.chart.needs_redraw()
    }

    fn build_frame(&mut self, now_ms: f64, renderer: &WgpuRenderer) {
        self.chart.build_frame(now_ms, &renderer.measurer());
    }

    fn frame(&self) -> &Frame {
        self.chart.frame()
    }

    fn data_view(&self) -> ChartDataView<'_> {
        self.chart.data_view()
    }

    fn set_reveal(&mut self, reveal: Option<Reveal>) {
        self.chart.set_reveal(reveal);
    }

    fn logo_anchor(&self) -> PointPx {
        // The lowest pane ends at the time axis.
        let l = self.chart.layout();
        PointPx {
            x: l.plot.x,
            y: l.time_axis.y,
        }
    }

    fn prepare(state: &mut State<Self>) {
        // Native custom series of the renderer; no-op when unchanged.
        let kinds = state.renderer.native_kinds();
        state.scene.chart.set_native_kinds(kinds);
    }

    fn take_events(
        host: &Rc<Shared>,
        state: &mut State<Self>,
        out: &mut Vec<(&'static str, JsValue)>,
    ) -> bool {
        let scene = &mut state.scene;
        let mut events: Vec<ChartEvent> = scene
            .chart
            .drain_events()
            .filter(|e| !matches!(e, ChartEvent::DrawingSync))
            .collect();
        // Take the chart's drawing sync operations every frame: peers are
        // woken for each one, whether or not the page takes them.
        let mut ops = Vec::new();
        scene.chart.take_drawing_sync_ops(&mut ops);
        let posted = !ops.is_empty();
        for op in ops {
            if scene.sync_ops.len() >= MAX_OPS {
                scene.sync_ops.pop_front();
            }
            scene.sync_ops.push_back(op);
        }
        if posted && !scene.sync_ops_reported {
            scene.sync_ops_reported = true;
            events.push(ChartEvent::DrawingSync);
        }
        let pointer = state.pointer;
        out.extend(
            events
                .iter()
                .map(|e| (event_name(e), event_js(host, e, pointer))),
        );
        posted || events.iter().any(|e| matches!(e, ChartEvent::Sync(_)))
    }

    fn after_frame(host: &Rc<Shared>, now: f64) {
        // Side plots follow the price range just drawn, in this frame.
        let plots = match host.state.try_borrow_mut() {
            Ok(mut s) => {
                s.scene.side_plots.retain(|p| p.strong_count() > 0);
                s.scene.side_plots.clone()
            }
            Err(_) => return,
        };
        for p in plots.iter().filter_map(Weak::upgrade) {
            host::frame_now(&p, now);
        }
    }
}

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
/** Options of `FerroChart.create`. */
export interface ChartCreateOptions {
  /** Color theme. Default `'light'`. */
  theme?: 'light' | 'dark';
  /** GPU API. `'auto'` (default) tries WebGPU, then WebGL2. */
  renderer?: 'auto' | 'webgpu' | 'webgl2';
  /** Largest bar spacing when zooming in, CSS px. Default 50; footprint
   *  numbers need more than 60, TPO letters several hundred. */
  maxBarSpacing?: number;
  /** A small FerroChart logo linking to ferrochart.com at the bottom-left of
   *  the lowest pane (a DOM element next to the canvas, not drawn into it).
   *  Default `true`; turning it off is fine (courtesy, not a license term). */
  attributionLogo?: boolean;
}

/** Options of `playIntro`. */
export interface IntroOptions {
  /** Time to draw the data from left to right, ms. Default 900. */
  duration?: number;
  /** Time the empty chart (grid and axes) is shown first, ms. Default 0. */
  delay?: number;
  /** CSS color of a glowing line at the drawing edge; `null` (default) for none. */
  pen?: string | null;
}

/** Columns of `Series.setData`. All columns have the same length. */
export interface OhlcData {
  /** Bar open times, UTC milliseconds, strictly increasing. */
  time: BigInt64Array | Float64Array | number[];
  open: Float64Array | number[];
  high: Float64Array | number[];
  low: Float64Array | number[];
  close: Float64Array | number[];
  volume?: Float64Array | number[];
}

/** One bar of a `Series.setData` row array (columns are faster for large data). */
export interface OhlcRow {
  /** UTC milliseconds or a `Date`. */
  time: number | bigint | Date;
  open: number;
  high: number;
  low: number;
  close: number;
  volume?: number;
}

/** One point of a `Series.setData` row array for single-value series. */
export interface ValueRow {
  /** UTC milliseconds or a `Date`. */
  time: number | bigint | Date;
  /** Missing or `NaN`: a gap. */
  value?: number;
  /** CSS color of this point (histograms). */
  color?: string;
}

/** One bar for `Series.update`. */
export interface Bar {
  /** UTC milliseconds or a `Date`. */
  time: number | bigint | Date;
  open: number;
  high: number;
  low: number;
  close: number;
  volume?: number;
}

/** Values of the first OHLC series that has a bar under the crosshair. */
export interface BarValues {
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number | null;
}

export interface CrosshairPoint {
  logicalIndex: number;
  /** UTC ms, `null` beyond the data. */
  time: number | null;
  /** Id of the pane under the pointer (the main pane for a synced crosshair). */
  pane: number;
  /** Price at the crosshair; `NaN` for a crosshair received from a synced chart. */
  price: number;
  bar: BarValues | null;
  /** Pointer position in CSS px relative to the canvas; absent for a synced crosshair. */
  x?: number;
  y?: number;
}

/** What a click hit: a drawing, a marker (its index in `series.markers()`) or a series. */
export type ClickTarget =
  | { type: 'drawing'; id: number }
  | { type: 'marker'; series: Series; index: number }
  | { type: 'series'; series: Series };

/**
 * A press and release in a plot that moved at most 3 CSS px (not sent while a
 * drawing is being created).
 */
export interface ClickEvent {
  /** Position in CSS px relative to the canvas. */
  x: number;
  y: number;
  pane: number;
  logicalIndex: number;
  /** UTC ms of the nearest bar, `null` beyond the data. */
  time: number | null;
  price: number;
  /** `null` for empty space. */
  target: ClickTarget | null;
}

export interface ChartEventMap {
  click: ClickEvent;
  crosshairMove: { point: CrosshairPoint | null };
  visibleRangeChanged: { from: number; to: number };
  needMoreHistory: { barsBeforeStart: number };
  drawingCreated: { id: number };
  drawingModified: { id: number };
  drawingRemoved: { id: number };
  drawingSelected: { id: number | null };
  /** A message this chart posted to its sync group. */
  sync: { message: SyncMessage };
  /** The chart sent drawing sync operations; take them with `takeDrawingSyncOps()`. Once until taken. */
  drawingSync: {};
  warning: ChartWarning;
}

/**
 * A non-fatal problem. `timeAxisMerged`: times of `series` fell between bars
 * of the time axis, so `count` bars were inserted and the logical indices
 * after them moved. `dataDropped`: the data of `series` could not be moved
 * onto a rebuilt time axis (out of memory) and was cleared. `ohlcRepaired`
 * (not sent from JS, where invalid bars are rejected): `count` bars were
 * repaired. `other`: a kind added in a later version.
 */
export type ChartWarning =
  | { kind: 'timeAxisMerged'; series: Series; count: number }
  | { kind: 'dataDropped'; series: Series }
  | { kind: 'ohlcRepaired'; series: Series; count: number }
  | { kind: 'other' };

export interface FerroChart {
  /**
   * Subscribes to an event of `ChartEventMap` (crosshair, visible range,
   * history requests, drawings, sync messages, warnings). Events are
   * delivered after the frame that produced them. An exception thrown by the
   * callback is reported (`reportError`, shown in the console) and does not
   * stop the other callbacks.
   */
  on<K extends keyof ChartEventMap>(event: K, callback: (event: ChartEventMap[K]) => void): void;
  /** Removes a callback added with `on`. */
  off<K extends keyof ChartEventMap>(event: K, callback: (event: ChartEventMap[K]) => void): void;
}
"#;

fn event_name(e: &ChartEvent) -> &'static str {
    match e {
        ChartEvent::Click { .. } => "click",
        ChartEvent::CrosshairMove { .. } => "crosshairMove",
        ChartEvent::VisibleRangeChanged { .. } => "visibleRangeChanged",
        ChartEvent::NeedMoreHistory { .. } => "needMoreHistory",
        ChartEvent::DrawingCreated(_) => "drawingCreated",
        ChartEvent::DrawingModified(_) => "drawingModified",
        ChartEvent::DrawingRemoved(_) => "drawingRemoved",
        ChartEvent::DrawingSelected(_) => "drawingSelected",
        ChartEvent::Sync(_) => "sync",
        ChartEvent::DrawingSync => "drawingSync",
        _ => "warning",
    }
}

fn point_js(p: &CrosshairPoint, pointer: Option<(f64, f64)>) -> JsValue {
    let o = Object::new();
    set(
        &o,
        "logicalIndex",
        &JsValue::from_f64(p.logical_index as f64),
    );
    set(
        &o,
        "time",
        &p.time
            .map_or(JsValue::NULL, |t| JsValue::from_f64(t as f64)),
    );
    set(&o, "pane", &JsValue::from_f64(f64::from(p.pane.0)));
    set(&o, "price", &JsValue::from_f64(p.price));
    if let Some((x, y)) = pointer.filter(|_| !p.price.is_nan()) {
        set(&o, "x", &x.into());
        set(&o, "y", &y.into());
    }
    let bar = p.bar.map_or(JsValue::NULL, |b| {
        let v = Object::new();
        set(&v, "open", &b.open.into());
        set(&v, "high", &b.high.into());
        set(&v, "low", &b.low.into());
        set(&v, "close", &b.close.into());
        set(
            &v,
            "volume",
            &b.volume.map_or(JsValue::NULL, JsValue::from_f64),
        );
        v.into()
    });
    set(&o, "bar", &bar);
    o.into()
}

fn click_target_js(shared: &Rc<Shared>, t: ClickTarget) -> JsValue {
    let o = Object::new();
    let series = |s| JsValue::from(crate::api::series(shared, s));
    match t {
        ClickTarget::Drawing(d) => {
            set(&o, "type", &"drawing".into());
            set(&o, "id", &JsValue::from_f64(d.0 as f64));
        }
        ClickTarget::Marker { series: s, index } => {
            set(&o, "type", &"marker".into());
            set(&o, "series", &series(s));
            set(&o, "index", &JsValue::from_f64(index as f64));
        }
        ClickTarget::Series(s) => {
            set(&o, "type", &"series".into());
            set(&o, "series", &series(s));
        }
        _ => return JsValue::NULL,
    }
    o.into()
}

fn event_js(shared: &Rc<Shared>, e: &ChartEvent, pointer: Option<(f64, f64)>) -> JsValue {
    let o = Object::new();
    let id = |d: ferrochart_core::data::DrawingId| JsValue::from_f64(d.0 as f64);
    match e {
        ChartEvent::Click {
            pos,
            pane,
            logical_index,
            time,
            price,
            target,
        } => {
            set(&o, "x", &JsValue::from_f64(f64::from(pos.x)));
            set(&o, "y", &JsValue::from_f64(f64::from(pos.y)));
            set(&o, "pane", &JsValue::from_f64(f64::from(pane.0)));
            set(&o, "logicalIndex", &JsValue::from_f64(*logical_index));
            set(
                &o,
                "time",
                &time.map_or(JsValue::NULL, |t| JsValue::from_f64(t as f64)),
            );
            set(&o, "price", &JsValue::from_f64(*price));
            let t = target.map_or(JsValue::NULL, |t| click_target_js(shared, t));
            set(&o, "target", &t);
        }
        ChartEvent::CrosshairMove { point } => {
            let p = point
                .as_ref()
                .map_or(JsValue::NULL, |p| point_js(p, pointer));
            set(&o, "point", &p);
        }
        ChartEvent::DrawingCreated(d)
        | ChartEvent::DrawingModified(d)
        | ChartEvent::DrawingRemoved(d) => set(&o, "id", &id(*d)),
        ChartEvent::DrawingSelected(d) => set(&o, "id", &d.map_or(JsValue::NULL, id)),
        ChartEvent::Sync(m) => set(&o, "message", &crate::js::sync_message_js(*m)),
        ChartEvent::DrawingSync => {}
        ChartEvent::VisibleRangeChanged { from, to } => {
            set(&o, "from", &JsValue::from_f64(*from));
            set(&o, "to", &JsValue::from_f64(*to));
        }
        ChartEvent::NeedMoreHistory { bars_before_start } => {
            set(
                &o,
                "barsBeforeStart",
                &JsValue::from_f64(*bars_before_start as f64),
            );
        }
        ChartEvent::Warning(w) => {
            let series = |s| JsValue::from(crate::api::series(shared, s));
            let num = |n: usize| JsValue::from_f64(n as f64);
            match *w {
                WarningKind::OhlcRepaired { series: s, bars } => {
                    set(&o, "kind", &"ohlcRepaired".into());
                    set(&o, "series", &series(s));
                    set(&o, "count", &num(bars));
                }
                WarningKind::TimeAxisMerged { series: s, slots } => {
                    set(&o, "kind", &"timeAxisMerged".into());
                    set(&o, "series", &series(s));
                    set(&o, "count", &num(slots));
                }
                WarningKind::DataDropped { series: s } => {
                    set(&o, "kind", &"dataDropped".into());
                    set(&o, "series", &series(s));
                }
                _ => set(&o, "kind", &"other".into()),
            }
        }
        _ => set(&o, "kind", &"other".into()),
    }
    o.into()
}

/// A chart on a `<canvas>`.
#[wasm_bindgen]
pub struct FerroChart {
    pub(crate) shared: Rc<Shared>,
}

#[wasm_bindgen]
impl FerroChart {
    /// Creates a chart drawing into `canvas`, which is sized by CSS (the
    /// backing store follows its CSS size × `devicePixelRatio`).
    ///
    /// @param {HTMLCanvasElement} canvas
    /// @param {ChartCreateOptions} [options]
    /// @returns {Promise<FerroChart>}
    pub async fn create(
        canvas: HtmlCanvasElement,
        #[wasm_bindgen(unchecked_optional_param_type = "ChartCreateOptions")] options: Option<
            JsValue,
        >,
    ) -> Result<FerroChart, JsError> {
        let options = options.unwrap_or(JsValue::UNDEFINED);
        let max_bar_spacing = match crate::js::opt_f64(&options, "maxBarSpacing")? {
            None => None,
            Some(v) if v.is_finite() && v >= 1.0 => Some(Css(v as f32)),
            Some(_) => return Err(js_err("'maxBarSpacing' must be a number >= 1")),
        };
        let mut surface = host::open(&canvas, &options).await?;
        // Native order flow series the device supports (the heatmap's
        // compute path on WebGPU); otherwise their portable paths draw.
        ferrochart_orderflow::register_gpu(&surface.ctx, &mut surface.renderer);
        let defaults = ChartOptions::default();
        let chart = Chart::new(ChartOptions {
            theme: surface.theme,
            max_bar_spacing: max_bar_spacing.unwrap_or(defaults.max_bar_spacing),
            ..defaults
        });
        let scene = ChartScene {
            chart,
            sync_ops: VecDeque::new(),
            sync_ops_reported: false,
            side_plots: Vec::new(),
        };
        let shared = Host::start(surface, &canvas, scene)?;
        Ok(FerroChart { shared })
    }

    /// The GPU API in use: `'webgpu'` or `'webgl2'`.
    #[wasm_bindgen(getter, unchecked_return_type = "'webgpu' | 'webgl2'")]
    pub fn renderer(&self) -> String {
        self.shared.backend.to_owned()
    }

    /// The theme the chart was created with.
    pub(crate) fn theme(&self) -> Result<Theme, JsError> {
        self.shared
            .state
            .try_borrow()
            .map(|s| s.theme)
            .map_err(|_| js_err("chart is busy (re-entrant call)"))
    }

    pub(crate) fn with_chart<T>(&self, f: impl FnOnce(&mut Chart) -> T) -> Result<T, JsError> {
        self.shared.with_scene(|s| f(&mut s.chart))
    }

    /// Shows all data.
    #[wasm_bindgen(js_name = fitContent)]
    pub fn fit_content(&self) -> Result<(), JsError> {
        self.with_chart(Chart::fit_content)
    }

    /// Scrolls to the latest bar.
    #[wasm_bindgen(js_name = scrollToRealtime)]
    pub fn scroll_to_realtime(&self) -> Result<(), JsError> {
        self.with_chart(Chart::scroll_to_realtime)
    }

    /// Sets the bar spacing in CSS pixels.
    #[wasm_bindgen(js_name = setBarSpacing)]
    pub fn set_bar_spacing(
        &self,
        #[wasm_bindgen(js_name = cssPx)] css_px: f64,
    ) -> Result<(), JsError> {
        let dpr = self.shared.state.try_borrow().map_or(1.0, |s| s.dpr);
        self.with_chart(|c| c.time_scale().set_bar_spacing(css_px * dpr))
    }

    /// Crosshair mode: `'normal'`, `'magnet'` (snaps to the nearest of the
    /// bar's open, high, low and close) or
    /// `'hidden'`.
    #[wasm_bindgen(js_name = setCrosshairMode)]
    pub fn set_crosshair_mode(
        &self,
        #[wasm_bindgen(unchecked_param_type = "'normal' | 'magnet' | 'hidden'")] mode: &str,
    ) -> Result<(), JsError> {
        let mode = match mode {
            "normal" => CrosshairMode::Normal,
            "magnet" => CrosshairMode::Magnet,
            "hidden" => CrosshairMode::Hidden,
            other => return Err(js_err(format!("unknown crosshair mode '{other}'"))),
        };
        self.with_chart(|c| c.set_crosshair_mode(mode))
    }

    /// Subscribes to an event (typed in the `FerroChart` interface above).
    #[wasm_bindgen(skip_typescript)]
    pub fn on(
        &self,
        #[wasm_bindgen(unchecked_param_type = "keyof ChartEventMap")] event: &str,
        #[wasm_bindgen(unchecked_param_type = "(event: any) => void")] callback: Function,
    ) -> Result<(), JsError> {
        self.shared.listen(event, callback)
    }

    /// Removes a callback added with `on` (typed in the interface above).
    #[wasm_bindgen(skip_typescript)]
    pub fn off(
        &self,
        #[wasm_bindgen(unchecked_param_type = "keyof ChartEventMap")] event: &str,
        #[wasm_bindgen(unchecked_param_type = "(event: any) => void")] callback: &Function,
    ) -> Result<(), JsError> {
        self.shared.unlisten(event, callback)
    }

    /// Intro animation: the chart is shown empty at once (grid and
    /// axes), then after `delay` ms its data is drawn from left to right over
    /// `duration` ms. Price lines, value labels and the crosshair appear at
    /// the end. A press, wheel, pinch or key ends it early. The promise
    /// resolves when it ends (also through `finishIntro`).
    ///
    /// @param {IntroOptions} [options]
    /// @returns {Promise<void>}
    #[wasm_bindgen(js_name = playIntro, unchecked_return_type = "Promise<void>")]
    pub fn play_intro(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "IntroOptions")] options: Option<JsValue>,
    ) -> Result<js_sys::Promise, JsError> {
        crate::intro::play(&self.shared, &options.unwrap_or(JsValue::UNDEFINED))
    }

    /// Ends the intro animation now, showing everything.
    #[wasm_bindgen(js_name = finishIntro)]
    pub fn finish_intro(&self) {
        crate::intro::finish(&self.shared);
    }

    /// Shows or removes the attribution logo (see `attributionLogo`).
    #[wasm_bindgen(js_name = setAttributionLogo)]
    pub fn set_attribution_logo(&self, on: bool) {
        self.shared.set_logo(on);
    }

    /// Removes all listeners and releases the GPU resources. The object can
    /// no longer be used.
    pub fn destroy(self) {
        crate::api::leave_group(&self.shared);
        self.shared.shut_down();
    }
}

/// A series of a [`FerroChart`].
#[wasm_bindgen]
pub struct Series {
    pub(crate) shared: Weak<Shared>,
    pub(crate) id: SeriesId,
}

pub(crate) fn f64_column(
    data: &JsValue,
    key: &str,
    required: bool,
) -> Result<Option<Vec<f64>>, JsError> {
    let v = Reflect::get(data, &JsValue::from_str(key)).map_err(|_| js_err("invalid data"))?;
    if v.is_undefined() || v.is_null() {
        return if required {
            Err(js_err(format!("missing column '{key}'")))
        } else {
            Ok(None)
        };
    }
    if let Some(a) = v.dyn_ref::<Float64Array>() {
        return Ok(Some(a.to_vec()));
    }
    if Array::is_array(&v) {
        return Ok(Some(Float64Array::new(&v).to_vec()));
    }
    Err(js_err(format!(
        "column '{key}' must be a Float64Array or an array of numbers"
    )))
}

/// Runs `f` on the chart of `shared` and schedules a frame.
pub(crate) fn with_shared<T>(
    shared: &Weak<Shared>,
    f: impl FnOnce(&mut Chart) -> T,
) -> Result<T, JsError> {
    host::with_weak(shared, |s| f(&mut s.chart))
}

pub(crate) fn time_column(data: &JsValue) -> Result<Vec<i64>, JsError> {
    let v = Reflect::get(data, &JsValue::from_str("time")).map_err(|_| js_err("invalid data"))?;
    if let Some(a) = v.dyn_ref::<BigInt64Array>() {
        return Ok(a.to_vec());
    }
    let floats = f64_column(data, "time", true)?.unwrap_or_default();
    convert::times_from_f64(&floats)
        .map_err(|i| js_err(format!("time[{i}] is not an integer millisecond timestamp")))
}

/// A row's `time`: integer UTC milliseconds (number or bigint) or a `Date`.
fn row_time(row: &JsValue, i: usize) -> Result<i64, JsError> {
    let t = Reflect::get(row, &JsValue::from_str("time")).map_err(|_| js_err("invalid data"))?;
    if t.is_bigint() {
        return i64::try_from(t).map_err(|_| js_err(format!("data[{i}].time is out of range")));
    }
    let f = if let Some(d) = t.dyn_ref::<js_sys::Date>() {
        d.get_time()
    } else {
        t.as_f64()
            .ok_or_else(|| js_err(format!("data[{i}].time must be a number, bigint or Date")))?
    };
    convert::times_from_f64(&[f])
        .ok()
        .and_then(|v| v.first().copied())
        .ok_or_else(|| {
            js_err(format!(
                "data[{i}].time is not an integer millisecond timestamp"
            ))
        })
}

/// `setData` with an array of rows (`[{ time, open, high, low, close,
/// volume? }]` or `[{ time, value, color? }]`): turned into columns here, one
/// pass over the rows. Columns are faster for large data.
fn rows_to_columns(rows: &Array, value: bool) -> Result<SeriesData, JsError> {
    let n = rows.length() as usize;
    let mut time = Vec::with_capacity(n);
    let field = |row: &JsValue, i: usize, k: &str| {
        number(row, k)?.ok_or_else(|| js_err(format!("data[{i}] has no '{k}'")))
    };
    if value {
        let mut v = Vec::with_capacity(n);
        let mut color: Option<Vec<u32>> = None;
        for (i, row) in rows.iter().enumerate() {
            time.push(row_time(&row, i)?);
            v.push(number(&row, "value")?.unwrap_or(f64::NAN)); // missing: a gap
            let c = Reflect::get(&row, &JsValue::from_str("color")).unwrap_or(JsValue::UNDEFINED);
            if let Some(css) = c.as_string() {
                let c = crate::js::parse_color(&css, "color")?;
                color.get_or_insert_with(|| vec![0; n])[i] =
                    u32::from_be_bytes([c.r, c.g, c.b, c.a]);
            }
        }
        return Ok(SeriesData::Value(ValueColumns {
            time,
            value: v,
            color,
        }));
    }
    let (mut open, mut high, mut low, mut close) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    let mut volume: Option<Vec<f64>> = None;
    for (i, row) in rows.iter().enumerate() {
        time.push(row_time(&row, i)?);
        open.push(field(&row, i, "open")?);
        high.push(field(&row, i, "high")?);
        low.push(field(&row, i, "low")?);
        close.push(field(&row, i, "close")?);
        if let Some(v) = number(&row, "volume")? {
            volume.get_or_insert_with(|| vec![0.0; n])[i] = v;
        }
    }
    Ok(SeriesData::Ohlc(OhlcColumns {
        time,
        open,
        high,
        low,
        close,
        volume,
    }))
}

pub(crate) fn number(bar: &JsValue, key: &str) -> Result<Option<f64>, JsError> {
    let v = Reflect::get(bar, &JsValue::from_str(key)).map_err(|_| js_err("invalid bar"))?;
    if v.is_undefined() || v.is_null() {
        return Ok(None);
    }
    v.as_f64()
        .map(Some)
        .ok_or_else(|| js_err(format!("'{key}' must be a number")))
}

#[wasm_bindgen]
impl Series {
    pub(crate) fn with_chart<T>(&self, f: impl FnOnce(&mut Chart) -> T) -> Result<T, JsError> {
        with_shared(&self.shared, f)
    }

    fn is_value(&self) -> Result<bool, JsError> {
        Ok(self
            .with_chart(|c| c.series_kind(self.id))?
            .is_some_and(|k| {
                matches!(
                    k,
                    SeriesKind::Line
                        | SeriesKind::Area
                        | SeriesKind::Baseline
                        | SeriesKind::Histogram
                        | SeriesKind::Points
                )
            }))
    }

    /// The data argument of `setData`, `append` and `prepend` as columns.
    fn series_data(&self, data: &JsValue, method: &str) -> Result<SeriesData, JsError> {
        if let Some(rows) = data.dyn_ref::<Array>() {
            return rows_to_columns(rows, self.is_value()?);
        }
        if !data.is_object() {
            return Err(js_err(format!(
                "{method} expects {{ time, open, high, low, close, volume? }} or {{ time, value, color? }}"
            )));
        }
        if self.is_value()? {
            return Ok(SeriesData::Value(ValueColumns {
                time: time_column(data)?,
                value: f64_column(data, "value", true)?.unwrap_or_default(),
                color: crate::js::color_column(data)?,
            }));
        }
        let req = |k| f64_column(data, k, true).map(Option::unwrap_or_default);
        Ok(SeriesData::Ohlc(OhlcColumns {
            time: time_column(data)?,
            open: req("open")?,
            high: req("high")?,
            low: req("low")?,
            close: req("close")?,
            volume: f64_column(data, "volume", false)?,
        }))
    }

    /// Replaces all data: OHLC columns for candlestick, OHLC and Heikin-Ashi
    /// series, `{ time, value, color? }` for line, area, baseline, histogram
    /// and points series. Typed arrays are copied in one `memcpy` per column;
    /// `Float64Array` times are converted to integers. An array of rows
    /// (`[{ time, open, high, low, close, volume? }]` or `[{ time, value,
    /// color? }]`, `time` in UTC ms or a `Date`) is also accepted; it is
    /// converted to columns first, so columns stay faster for large data.
    ///
    /// @param {OhlcData | ValueData | OhlcRow[] | ValueRow[]} data
    #[wasm_bindgen(js_name = setData)]
    pub fn set_data(
        &self,
        #[wasm_bindgen(unchecked_param_type = "OhlcData | ValueData | OhlcRow[] | ValueRow[]")]
        data: &JsValue,
    ) -> Result<(), JsError> {
        let d = self.series_data(data, "setData")?;
        self.with_chart(|c| c.set_data(self.id, d))?.map_err(js_err)
    }

    /// Adds bars or points after the last one (a batch from a feed), in any
    /// form `setData` takes. Times must come after the last point, and the
    /// volume column must be present exactly when the series has one. Throws,
    /// leaving the data unchanged, otherwise. For a single live bar, `update`.
    ///
    /// @param {OhlcData | ValueData | OhlcRow[] | ValueRow[]} data
    pub fn append(
        &self,
        #[wasm_bindgen(unchecked_param_type = "OhlcData | ValueData | OhlcRow[] | ValueRow[]")]
        data: &JsValue,
    ) -> Result<(), JsError> {
        let d = self.series_data(data, "append")?;
        self.with_chart(|c| c.append(self.id, d))?.map_err(js_err)
    }

    /// Inserts older bars or points before the first one (loading history,
    /// typically on `needMoreHistory`), in any form `setData` takes. Existing
    /// bars keep their logical indices (the new ones get negative indices), so
    /// the view does not move. Times must come before the first point; volume
    /// as for `append`. Throws, leaving the data unchanged, otherwise.
    ///
    /// @param {OhlcData | ValueData | OhlcRow[] | ValueRow[]} data
    pub fn prepend(
        &self,
        #[wasm_bindgen(unchecked_param_type = "OhlcData | ValueData | OhlcRow[] | ValueRow[]")]
        data: &JsValue,
    ) -> Result<(), JsError> {
        let d = self.series_data(data, "prepend")?;
        self.with_chart(|c| c.prepend(self.id, d))?.map_err(js_err)
    }

    /// Updates the last bar or point (same time) or appends one (later time):
    /// `{ time, open, high, low, close, volume? }` or `{ time, value }`.
    ///
    /// @param {Bar | ValuePoint} bar
    pub fn update(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Bar | ValuePoint")] bar: &JsValue,
    ) -> Result<(), JsError> {
        let t = Reflect::get(bar, &JsValue::from_str("time")).map_err(|_| js_err("invalid bar"))?;
        let time = crate::js::time_ms(&t, "time")?;
        let req = |k| number(bar, k)?.ok_or_else(|| js_err(format!("missing '{k}'")));
        if self.is_value()? {
            // `update_last` takes the value of single-value series from `close`.
            let v = req("value")?;
            let update = BarUpdate {
                time,
                open: v,
                high: v,
                low: v,
                close: v,
                volume: None,
            };
            return self
                .with_chart(|c| c.update_last(self.id, update))?
                .map(|_| ())
                .map_err(js_err);
        }
        let update = BarUpdate {
            time,
            open: req("open")?,
            high: req("high")?,
            low: req("low")?,
            close: req("close")?,
            volume: number(bar, "volume")?,
        };
        self.with_chart(|c| c.update_last(self.id, update))?
            .map(|_| ())
            .map_err(js_err)
    }
}

/// Deterministic random-walk OHLCV bars, bit-identical to Rust's
/// `ferrochart_core::testing::random_walk` (for demos, benchmarks and tests).
///
/// @param {number} seed
/// @param {number} n number of bars
/// @param {number} startMs time of the first bar, UTC ms
/// @param {number} intervalMs time between bars
/// @returns {OhlcData}
#[wasm_bindgen(js_name = randomWalk, unchecked_return_type = "OhlcData")]
pub fn random_walk(
    seed: f64,
    n: u32,
    #[wasm_bindgen(js_name = startMs)] start_ms: f64,
    #[wasm_bindgen(js_name = intervalMs)] interval_ms: f64,
) -> Result<JsValue, JsError> {
    let whole = |v: f64, what: &str| {
        convert::times_from_f64(&[v])
            .ok()
            .and_then(|t| t.first().copied())
            .ok_or_else(|| js_err(format!("{what} must be an integer")))
    };
    let seed = u64::try_from(whole(seed, "seed")?).map_err(|_| js_err("seed must be >= 0"))?;
    let bars = ferrochart_core::testing::random_walk(
        seed,
        n as usize,
        whole(start_ms, "startMs")?,
        whole(interval_ms, "intervalMs")?,
    )
    .map_err(js_err)?;
    let o = Object::new();
    set(&o, "time", &BigInt64Array::from(&bars.time[..]).into());
    set(&o, "open", &Float64Array::from(&bars.open[..]).into());
    set(&o, "high", &Float64Array::from(&bars.high[..]).into());
    set(&o, "low", &Float64Array::from(&bars.low[..]).into());
    set(&o, "close", &Float64Array::from(&bars.close[..]).into());
    if let Some(v) = &bars.volume {
        set(&o, "volume", &Float64Array::from(&v[..]).into());
    }
    Ok(o.into())
}
