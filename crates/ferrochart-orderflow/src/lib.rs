//! Order flow series for FerroChart: footprint,
//! heatmap and TPO, as [`CustomSeries`] plugins. The exact rules (price
//! levels, aggressor classification, imbalances, color scales, value area)
//! are in `docs/book/src/orderflow.md`.
//!
//! Prices are stored as level numbers of a fixed grid ([`PriceLevels`]).
//! The footprint and the heatmap share a sparse "time column × level" grid
//! ([`SparseGrid`]); only the last column changes, so real-time trades cost
//! O(levels of the last column).
//!
//! With the default feature `gpu`, `register_gpu` adds the heatmap's
//! native wgpu path (a compute shader) to a renderer whose device supports
//! it; elsewhere (WebGL2, the CPU backend) the heatmap draws through its
//! portable path with the same output.
//!
//! ```
//! use ferrochart_core::chart::SeriesOptions;
//! use ferrochart_core::testing::MonoMeasurer;
//! use ferrochart_core::theme::Theme;
//! use ferrochart_core::{Chart, ChartOptions};
//! use ferrochart_orderflow::{Footprint, FootprintOptions, Side, TickColumns, append_trades};
//!
//! let mut chart = Chart::new(ChartOptions::default());
//! let fp = Footprint::new(FootprintOptions::new(0.5, 60_000, &Theme::light()))?;
//! let id = chart.add_custom_series(chart.main_pane(), Box::new(fp), SeriesOptions::default())?;
//! let trades = TickColumns {
//!     time: (0..600).map(|i| i * 1_000).collect(),
//!     price: (0..600).map(|i| 100.0 + f64::from(i % 7) * 0.5).collect(),
//!     size: vec![1.0; 600],
//!     side: vec![Side::Unknown; 600],
//! };
//! append_trades(&mut chart, id, &trades)?;
//! chart.build_frame(0.0, &MonoMeasurer);
//! # Ok::<(), ferrochart_core::Error>(())
//! ```
//!
//! [`CustomSeries`]: ferrochart_core::plugin::CustomSeries
#![forbid(unsafe_code)]

mod footprint;
#[cfg(feature = "gpu")]
mod gpu;
mod grid;
mod heatmap;
mod levels;
#[cfg(feature = "testing")]
pub mod testing;
mod tpo;
mod trades;

use core::ops::Range;

use ferrochart_core::Chart;
use ferrochart_core::data::{DataChange, OhlcColumns, SeriesId, Timestamp};
use ferrochart_core::error::{Error, Result};
use ferrochart_core::plugin::CustomSeries;

pub use footprint::{
    CELLS_MIN_SPACING, FOOTPRINT_KIND, Footprint, FootprintColors, FootprintMode, FootprintOptions,
    NUMBERS_MIN_SPACING,
};
#[cfg(feature = "gpu")]
pub use gpu::{HeatmapGpu, register_gpu, supports_native_heatmap};
pub use grid::{Column, SparseGrid};
pub use heatmap::{
    ColorScale, DepthSnapshot, HEATMAP_KIND, Heatmap, HeatmapColors, HeatmapOptions, HeatmapRange,
    HeatmapSource, MAX_LEVELS_PER_ROW,
};
pub use levels::PriceLevels;
pub use tpo::{
    SessionProfile, TPO_KIND, TPO_MIN_FONT, TPO_MIN_SPACING, Tpo, TpoColors, TpoOptions, tpo_letter,
};
pub use trades::{Side, TickColumns, TickRule};

/// Start of the time unit of `period` ms that contains `time` (UTC
/// multiples of `period`).
pub(crate) fn column_start(time: Timestamp, period: i64) -> Timestamp {
    time.div_euclid(period).checked_mul(period).unwrap_or(time)
}

/// A series built from trades in time columns.
pub(crate) trait TradeSink: CustomSeries {
    /// Length of a column, ms.
    fn timeframe(&self) -> i64;
    /// Start of the last column.
    fn last_column(&self) -> Option<Timestamp>;
    /// Adds `trades[range]` (already checked).
    fn apply(&mut self, trades: &TickColumns, range: Range<usize>) -> Result<()>;
}

/// Adds trades to a footprint (or a trade heatmap) in `chart`.
///
/// Trades in the last column update it; later ones open new columns. Times
/// must be non-decreasing, and no trade may fall in a column before the
/// last one.
///
/// # Errors
///
/// [`Error::UnknownId`]; [`Error::InvalidArgument`] if `id` is not such a
/// series or for bad trade columns (see [`TickColumns`]);
/// [`Error::TimeNotIncreasing`]; [`Error::OutOfMemory`]. Nothing changes
/// when the trades are rejected up front; an allocation failure part way
/// keeps the trades added so far.
pub fn append_trades(chart: &mut Chart, id: SeriesId, trades: &TickColumns) -> Result<()> {
    trades.check(None)?;
    if chart.custom_series::<Footprint>(id).is_some() {
        return append::<Footprint>(chart, id, trades);
    }
    if let Some(h) = chart.custom_series::<Heatmap>(id) {
        if h.options().source != HeatmapSource::Trades {
            return Err(Error::InvalidArgument("the heatmap takes depth snapshots"));
        }
        return append::<Heatmap>(chart, id, trades);
    }
    chart.series_kind(id).ok_or(Error::UnknownId)?;
    Err(Error::InvalidArgument("the series takes no trades"))
}

/// Adds an order book snapshot to a depth heatmap in `chart`.
///
/// A snapshot in the last column updates it (each level keeps its largest
/// size); a later one opens a new column.
///
/// # Errors
///
/// [`Error::UnknownId`]; [`Error::InvalidArgument`] if `id` is not a depth
/// heatmap or for a bad snapshot (see [`DepthSnapshot`]);
/// [`Error::TimeNotIncreasing`] if the snapshot falls in a column before the
/// last one; [`Error::OutOfMemory`]. Nothing changes when the snapshot is
/// rejected up front; an allocation failure part way keeps the levels
/// added so far.
pub fn append_depth(chart: &mut Chart, id: SeriesId, snapshot: &DepthSnapshot<'_>) -> Result<()> {
    if chart.custom_series::<Heatmap>(id).is_none() {
        chart.series_kind(id).ok_or(Error::UnknownId)?;
        return Err(Error::InvalidArgument(
            "the series takes no depth snapshots",
        ));
    }
    let mut failed = None;
    chart.update_custom(id, |h: &mut Heatmap| {
        let (before, version) = (h.grid().columns(), h.version());
        let result = h.push_depth(snapshot);
        let change = match h.grid().columns() - before {
            _ if h.version() == version => DataChange::Append { count: 0 },
            0 => DataChange::UpdateLast,
            count if result.is_ok() => DataChange::Append { count },
            _ => DataChange::Replace,
        };
        failed = result.err();
        change
    })?;
    failed.map_or(Ok(()), Err)
}

fn append<T: TradeSink>(chart: &mut Chart, id: SeriesId, trades: &TickColumns) -> Result<()> {
    let s = chart.custom_series::<T>(id).ok_or(Error::UnknownId)?;
    let (period, last) = (s.timeframe(), s.last_column());
    let n = trades.len();
    // Trades in the last column, then trades opening new ones.
    let split = match (last, trades.time.first()) {
        (Some(last), Some(&t0)) if column_start(t0, period) < last => {
            return Err(Error::TimeNotIncreasing { index: 0 });
        }
        (Some(last), _) => trades
            .time
            .partition_point(|&t| column_start(t, period) <= last),
        (None, _) => 0,
    };
    let mut failed = None;
    if split > 0 {
        chart.update_custom(id, |s: &mut T| match s.apply(trades, 0..split) {
            Ok(()) => DataChange::UpdateLast,
            Err(e) => {
                failed = Some(e);
                DataChange::Replace
            }
        })?;
    }
    if failed.is_none() && split < n {
        chart.update_custom(id, |s: &mut T| {
            let before = s.times().len();
            match s.apply(trades, split..n) {
                Ok(()) => DataChange::Append {
                    count: s.times().len() - before,
                },
                Err(e) => {
                    failed = Some(e);
                    DataChange::Replace
                }
            }
        })?;
    }
    failed.map_or(Ok(()), Err)
}

/// Adds bars (time, high and low are used) to a TPO series in `chart`.
///
/// Each bar marks the levels from its low to its high in the period that
/// contains its time; bars must be no longer than a period. Bars in the last
/// session update it (a bar sent again, e.g. a live candle, widens its
/// period); later ones open new sessions. Bars outside the sessions are
/// skipped.
///
/// # Errors
///
/// [`Error::UnknownId`]; [`Error::InvalidArgument`] if `id` is not a TPO
/// series or for bad bars (columns of different lengths, non-finite prices,
/// `high < low`); [`Error::TimeNotIncreasing`] for times out of order or a
/// session before the last one; [`Error::OutOfMemory`]. Nothing changes
/// when the bars are rejected up front; an allocation failure part way
/// keeps the bars added so far.
pub fn append_bars(chart: &mut Chart, id: SeriesId, bars: &OhlcColumns) -> Result<()> {
    let Some(t) = chart.custom_series::<Tpo>(id) else {
        chart.series_kind(id).ok_or(Error::UnknownId)?;
        return Err(Error::InvalidArgument("the series takes no bars"));
    };
    tpo::check_bars(bars, t.price_levels())?;
    let n = bars.time.len();
    let split = match (t.last_session(), bars.time.first()) {
        (Some(last), Some(&t0)) if t.session_start(t0) < last => {
            return Err(Error::TimeNotIncreasing { index: 0 });
        }
        (Some(last), _) => bars.time.partition_point(|&x| t.session_start(x) <= last),
        (None, _) => 0,
    };
    let apply = |t: &mut Tpo, range: Range<usize>| -> Result<()> {
        let result = range.into_iter().try_for_each(|i| {
            t.push_raw(bars.time[i], bars.high[i], bars.low[i])
                .map(drop)
        });
        t.finish_last();
        result
    };
    let mut failed = None;
    if split > 0 {
        chart.update_custom(id, |t: &mut Tpo| match apply(t, 0..split) {
            Ok(()) => DataChange::UpdateLast,
            Err(e) => {
                failed = Some(e);
                DataChange::Replace
            }
        })?;
    }
    if failed.is_none() && split < n {
        chart.update_custom(id, |t: &mut Tpo| {
            let before = t.sessions().len();
            match apply(t, split..n) {
                Ok(()) => DataChange::Append {
                    count: t.sessions().len() - before,
                },
                Err(e) => {
                    failed = Some(e);
                    DataChange::Replace
                }
            }
        })?;
    }
    failed.map_or(Ok(()), Err)
}
