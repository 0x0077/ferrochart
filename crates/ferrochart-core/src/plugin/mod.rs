//! Plugin interfaces: indicators, custom
//! series, layer plugins and series transforms, and [`MinMaxIndex`] for
//! plugins' range queries.
//!
//! Core schedules indicator computation, so the [`Indicator`] trait lives
//! here; `ferrochart-indicators` implements the built-in indicators. Each
//! output of an indicator is an ordinary value series of the chart
//! ([`crate::Chart::indicator_series`]); [`Indicator::update`] writes its
//! values straight into those series' storage through [`IndicatorOutputs`].

use crate::chart::BandOptions;
use crate::data::{DataChange, PaneId, Timestamp};
use crate::geom::Rgba;
use crate::series::Source;

mod custom;
mod layer;
mod min_max;
mod transform;

pub use custom::{AxisMode, CustomSeries, SeriesCtx};
pub(crate) use custom::{CustomChange, CustomStore};
pub use layer::{LayerCtx, LayerPlugin, LayerPosition};
pub use min_max::MinMaxIndex;
pub use transform::{MAX_OUTPUT_PER_CALL, SeriesTransform, TransformBar, TransformOutput};

/// Where an indicator's outputs are shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// On the pane of the source series, sharing its price scale.
    Overlay,
    /// On a new pane below the others.
    NewPane,
    /// On an existing pane.
    Pane(PaneId),
    /// On the pane of the source's volume series, on the volume's scale (as
    /// high as the bars); a new pane when the source has no volume series.
    Volume,
}

/// How an indicator output is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKind {
    /// A line (a `Line` series).
    Line,
    /// Bars from 0 (a `Histogram` series).
    Histogram,
    /// Dots (a `Points` series), e.g. Parabolic SAR.
    Points,
}

/// One output of an indicator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputSpec {
    /// Short name, e.g. `"signal"`.
    pub name: &'static str,
    /// How it is drawn.
    pub kind: OutputKind,
    /// Default color; `None` picks one from the chart's palette.
    pub color: Option<Rgba>,
    /// Display shift in bars (`SeriesOptions::shift`): positive values are
    /// drawn later (Ichimoku's leading spans), negative ones earlier.
    pub shift: i32,
    /// Fill from this output to output `.0` (a band), e.g. between
    /// Bollinger's upper and lower lines; see
    /// [`Chart::indicator_bands`](crate::Chart::indicator_bands).
    pub fill_to: Option<(usize, BandOptions)>,
}

impl OutputSpec {
    /// A line output with no default color and no shift.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::plugin::{OutputKind, OutputSpec};
    ///
    /// let spec = OutputSpec::line("sma");
    /// assert_eq!((spec.kind, spec.shift), (OutputKind::Line, 0));
    /// ```
    pub const fn line(name: &'static str) -> Self {
        Self {
            name,
            kind: OutputKind::Line,
            color: None,
            shift: 0,
            fill_to: None,
        }
    }

    /// A histogram output with no default color and no shift.
    pub const fn histogram(name: &'static str) -> Self {
        Self {
            name,
            kind: OutputKind::Histogram,
            color: None,
            shift: 0,
            fill_to: None,
        }
    }
}

/// One bar of an indicator's source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputBar {
    /// Bar time (UTC milliseconds).
    pub time: Timestamp,
    /// Open (single-value sources: the value).
    pub open: f64,
    /// High.
    pub high: f64,
    /// Low.
    pub low: f64,
    /// Close.
    pub close: f64,
    /// Volume; NaN when the source has none.
    pub volume: f64,
}

/// An indicator's source series, slot by slot: slot `i` is logical index
/// `first + i` of the time axis, over the logical indices the source covers.
#[derive(Clone, Copy)]
pub struct IndicatorInput<'a> {
    store: Source<'a>,
    index: &'a crate::data::TimeIndex,
    first: i64,
    len: usize,
}

impl<'a> IndicatorInput<'a> {
    pub(crate) fn new(store: Source<'a>, index: &'a crate::data::TimeIndex) -> Self {
        let (first, end) = store.span().unwrap_or((0, 0));
        Self {
            store,
            index,
            first,
            len: usize::try_from(end - first).unwrap_or(0),
        }
    }

    /// Logical index of slot 0.
    pub(crate) fn first(&self) -> i64 {
        self.first
    }

    /// Number of slots.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no slots.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bar at `slot`; `None` for a gap bar or a slot out of range.
    pub fn bar(&self, slot: usize) -> Option<InputBar> {
        if slot >= self.len {
            return None;
        }
        let j = self.first + i64::try_from(slot).ok()?;
        let [open, high, low, close] = self.store.bar_at(j)?;
        Some(InputBar {
            time: self.index.time_at(j)?,
            open,
            high,
            low,
            close,
            volume: self.store.volume_at(j).unwrap_or(f64::NAN),
        })
    }

    /// Whether the source has volume.
    pub fn has_volume(&self) -> bool {
        self.store.has_volume()
    }
}

/// Where [`Indicator::update`] writes: for every output, the values of slots
/// `start()..len()` of the input, all NaN before the call.
#[derive(Clone, Debug, Default)]
pub struct IndicatorOutputs {
    start: usize,
    len: usize,
    /// Columns in use; `cols` keeps more (with their capacity) for
    /// indicators with more outputs, so alternating ones do not allocate.
    active: usize,
    cols: Vec<Vec<f64>>,
}

impl IndicatorOutputs {
    /// Prepares `outputs` columns for slots `start..len`, keeping capacity.
    pub(crate) fn reset(&mut self, outputs: usize, start: usize, len: usize) {
        self.start = start.min(len);
        self.len = len;
        if self.cols.len() < outputs {
            self.cols.resize_with(outputs, Vec::new);
        }
        self.active = outputs;
        for c in &mut self.cols[..outputs] {
            c.clear();
            c.resize(len - self.start, f64::NAN);
        }
    }

    /// Releases the memory of a full recomputation, keeping room for the
    /// real-time path (no allocation per tick).
    pub(crate) fn shrink(&mut self) {
        for c in &mut self.cols {
            c.clear();
            c.shrink_to(64);
        }
    }

    pub(crate) fn columns(&self) -> &[Vec<f64>] {
        &self.cols[..self.active]
    }

    /// First slot to write.
    pub fn start(&self) -> usize {
        self.start
    }

    /// Number of slots of the input.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no slots to write.
    pub fn is_empty(&self) -> bool {
        self.start >= self.len
    }

    /// Values of output `o` for slots `start()..len()`; empty for an output
    /// that does not exist.
    pub fn column(&mut self, o: usize) -> &mut [f64] {
        self.cols[..self.active]
            .get_mut(o)
            .map_or(&mut [], Vec::as_mut_slice)
    }
}

/// A technical indicator, computed incrementally by
/// the chart when its source series changes.
///
/// `update` gets the source and what changed:
///
/// - [`DataChange::Replace`] and [`DataChange::Prepend`]: recompute
///   everything (`out.start()` is 0);
/// - [`DataChange::Append`]: the last `count` slots are new
///   (`out.start() == len − count`);
/// - [`DataChange::UpdateLast`]: the last slot changed
///   (`out.start() == len − 1`).
///
/// `Append` and `UpdateLast` must be O(1) per slot and must not allocate
/// (`update_last` is a hot path). Gap bars (`input.bar(i) == None`)
/// should give NaN.
pub trait Indicator: Send {
    /// Display name, e.g. `"SMA 20"`.
    fn name(&self) -> &str;
    /// The outputs, in the order of `out.column(o)`.
    fn outputs(&self) -> &[OutputSpec];
    /// Where the outputs go by default.
    fn default_placement(&self) -> Placement;
    /// Number of leading valid bars whose outputs are NaN.
    fn warmup(&self) -> usize;
    /// Whether the indicator reads volume; `add_indicator` refuses sources
    /// without it.
    fn needs_volume(&self) -> bool {
        false
    }
    /// Updates the outputs for `change`; see the trait docs.
    fn update(
        &mut self,
        input: &IndicatorInput<'_>,
        change: DataChange,
        out: &mut IndicatorOutputs,
    );
}
