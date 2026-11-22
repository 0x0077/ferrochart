//! Core data model: basic types, columnar (`SoA`) input formats and column storage.
//!

mod change_log;
mod deq_column;
pub(crate) mod lod;
pub(crate) mod ohlc;
mod time_index;
pub(crate) mod value;

pub use change_log::{ChangeLog, DataDelta};
pub use deq_column::DeqColumn;
pub use lod::{ANCHOR, MIN_LOD_LEVEL};
pub use ohlc::{BarUpdate, Column, DataChange, OhlcLevelView, ValidationMode};
pub use time_index::{SeriesMapping, TimeIndex};

/// UTC timestamp in milliseconds. The engine does not handle time zones internally;
/// time zones only affect label formatting.
pub type Timestamp = i64;

/// Logical index: position on the global time index. May be negative or beyond the
/// data (empty space). Fractional values address positions between bars.
pub type LogicalIndex = f64;

/// Identifier of a series in a chart.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SeriesId(pub u32);

/// Identifier of an indicator in a chart.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct IndicatorId(pub u32);

/// Identifier of a price line in a chart.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PriceLineId(pub u32);

/// Identifier of a band (fill between two series) in a chart.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BandId(pub u32);

/// Identifier of a layer plugin in a chart.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LayerPluginId(pub u32);

/// Identifier of a pane in a chart.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PaneId(pub u32);

/// Identifier of a drawing.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DrawingId(pub u64);

/// Single-value series in columnar layout (line, area, histogram).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValueColumns {
    /// Point times (UTC milliseconds), strictly increasing.
    pub time: Vec<Timestamp>,
    /// Values; NaN means a gap (no line is drawn through it).
    pub value: Vec<f64>,
    /// Optional per-point color as `0xRRGGBBAA`; `0` (transparent) uses the
    /// series color. Only histogram series draw these, and only at full
    /// detail (LOD level 0); other kinds keep them but use the series color.
    pub color: Option<Vec<u32>>,
}

/// Half-open span of logical indices, `start..end`.
///
/// Used instead of `Range<i64>` because `Range` is not `Copy`, and everything that
/// ends up in a `Frame` must be.
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::IdxSpan;
///
/// let span = IdxSpan { start: -2, end: 3 };
/// assert_eq!(span.len(), 5);
/// assert!(!span.is_empty());
/// assert!(IdxSpan { start: 4, end: 4 }.is_empty());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct IdxSpan {
    /// First logical index in the span.
    pub start: i64,
    /// One past the last logical index in the span.
    pub end: i64,
}

impl IdxSpan {
    /// Whether the span contains no index (`end <= start`).
    pub fn is_empty(self) -> bool {
        self.end <= self.start
    }

    /// Number of indices in the span; 0 if empty.
    pub fn len(self) -> u64 {
        self.end.abs_diff(self.start) * u64::from(self.end > self.start)
    }

    /// Whether `self` and `other` overlap or are directly adjacent.
    pub(crate) fn touches(self, other: Self) -> bool {
        other.start <= self.end && self.start <= other.end
    }

    /// Smallest span containing both.
    pub(crate) fn hull(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// OHLC(V) series in columnar layout — the input format of `set_data` and friends.
///
/// Constraints (validated at the API boundary): `time` is strictly increasing,
/// all columns have the same length, and `low <= min(open, close)`,
/// `high >= max(open, close)` for every bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OhlcColumns {
    /// Bar open times (UTC milliseconds), strictly increasing.
    pub time: Vec<Timestamp>,
    /// Open prices.
    pub open: Vec<f64>,
    /// High prices.
    pub high: Vec<f64>,
    /// Low prices.
    pub low: Vec<f64>,
    /// Close prices.
    pub close: Vec<f64>,
    /// Optional volumes.
    pub volume: Option<Vec<f64>>,
}

impl OhlcColumns {
    /// Number of bars (length of the `time` column).
    pub fn len(&self) -> usize {
        self.time.len()
    }

    /// Whether there are no bars.
    pub fn is_empty(&self) -> bool {
        self.time.is_empty()
    }
}
