//! Deterministic data and golden scenes for tests and benchmarks
//! (feature `testing`).

use ferrochart_core::Chart;
use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::{OhlcColumns, SeriesId, Timestamp};
use ferrochart_core::error::{Error, Result};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::theme::Theme;

use crate::{
    DepthSnapshot, Footprint, FootprintMode, FootprintOptions, Heatmap, HeatmapOptions,
    HeatmapSource, Side, TickColumns, Tpo, TpoOptions, append_bars, append_depth, append_trades,
};

/// xorshift64.
struct Rng(u64);

impl Rng {
    /// Uniform in [0, 1).
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1_u64 << 53) as f64
    }
}

/// `n` trades `interval_ms` apart from `start`, on a random walk of
/// `tick` steps from 100 (each trade moves one tick up or down with
/// probability 0.3 each). Sizes are whole numbers 1–9, occasionally 20–60;
/// the aggressor follows the move (up: buy, down: sell) and is unknown when
/// the price stays.
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a non-positive `interval_ms` or `tick`.
///
/// # Examples
///
/// ```
/// let t = ferrochart_orderflow::testing::random_trades(1, 1_000, 0, 100, 0.25)?;
/// assert_eq!(t.len(), 1_000);
/// assert!(t.price.iter().all(|p| (p / 0.25).fract() == 0.0));
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn random_trades(
    seed: u64,
    n: usize,
    start: Timestamp,
    interval_ms: i64,
    tick: f64,
) -> Result<TickColumns> {
    if interval_ms <= 0 || !(tick.is_finite() && tick > 0.0) {
        return Err(Error::InvalidArgument(
            "interval_ms and tick must be positive",
        ));
    }
    let mut rng = Rng(seed.max(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut t = TickColumns::default();
    for v in [&mut t.price, &mut t.size] {
        v.try_reserve_exact(n).map_err(|_| Error::OutOfMemory)?;
    }
    t.time
        .try_reserve_exact(n)
        .map_err(|_| Error::OutOfMemory)?;
    t.side
        .try_reserve_exact(n)
        .map_err(|_| Error::OutOfMemory)?;
    let base = (100.0 / tick).round() as i64;
    let mut ticks = 0_i64;
    let mut time = start;
    for _ in 0..n {
        let r = rng.next();
        let side = if r < 0.3 {
            ticks += 1;
            Side::Buy
        } else if r < 0.6 {
            ticks -= 1;
            Side::Sell
        } else {
            Side::Unknown
        };
        let big = rng.next() < 0.03;
        let size = if big {
            20.0 + (rng.next() * 40.0).floor()
        } else {
            1.0 + (rng.next() * 9.0).floor()
        };
        t.time.push(time);
        t.price.push((base + ticks) as f64 * tick);
        t.size.push(size);
        t.side.push(side);
        time = time.saturating_add(interval_ms);
    }
    Ok(t)
}

/// Golden scene of a footprint: 40 one-minute columns from 12 000
/// trades (0.25 ticks, `row_size` ticks per row), in `mode`, at `spacing` px
/// per bar (≥ 60: numbers; 12–60: shaded cells), scrolled to the end. The
/// chart's `max_bar_spacing` must allow `spacing` (the default is 50 CSS px).
///
/// # Errors
///
/// The chart's errors.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::{Css, SizeCss};
/// use ferrochart_core::theme::Theme;
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_orderflow::FootprintMode;
/// use ferrochart_orderflow::testing::footprint_scene;
///
/// let mut chart = Chart::new(ChartOptions { max_bar_spacing: Css(120.0), ..ChartOptions::default() });
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// footprint_scene(&mut chart, &Theme::light(), 0, FootprintMode::BidAsk, 90.0, 4)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn footprint_scene(
    chart: &mut Chart,
    theme: &Theme,
    start: Timestamp,
    mode: FootprintMode,
    spacing: f64,
    row_size: u32,
) -> Result<SeriesId> {
    let mut opts = FootprintOptions::new(0.25, 60_000, theme);
    opts.mode = mode;
    opts.row_size = row_size;
    let id = chart.add_custom_series(
        chart.main_pane(),
        Box::new(Footprint::new(opts)?),
        SeriesOptions::default(),
    )?;
    append_trades(chart, id, &random_trades(5, 12_000, start, 200, 0.25)?)?;
    chart.time_scale().set_bar_spacing(spacing);
    chart.scroll_to_realtime();
    Ok(id)
}

/// Candles of `period` ms built from `trades` (open, high, low and close of
/// the trades in each period; volume is the traded size).
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a non-positive `period`.
///
/// # Examples
///
/// ```
/// use ferrochart_orderflow::testing::{candles_of, random_trades};
///
/// let t = random_trades(1, 600, 0, 1_000, 0.25)?;
/// assert_eq!(candles_of(&t, 60_000)?.time, (0..10).map(|i| i * 60_000).collect::<Vec<_>>());
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn candles_of(trades: &TickColumns, period: i64) -> Result<OhlcColumns> {
    if period <= 0 {
        return Err(Error::InvalidArgument("period must be positive"));
    }
    let mut c = OhlcColumns::default();
    let mut volume = Vec::new();
    for ((&t, &p), &s) in trades.time.iter().zip(&trades.price).zip(&trades.size) {
        let start = crate::column_start(t, period);
        if c.time.last() == Some(&start) {
            let k = c.time.len() - 1;
            c.high[k] = c.high[k].max(p);
            c.low[k] = c.low[k].min(p);
            c.close[k] = p;
            volume[k] += s;
        } else {
            c.time.push(start);
            c.open.push(p);
            c.high.push(p);
            c.low.push(p);
            c.close.push(p);
            volume.push(s);
        }
    }
    c.volume = Some(volume);
    Ok(c)
}

/// Order book snapshots for tests: one per `interval_ms` over the span of
/// `trades`, `depth` levels of `tick` on each side of the last trade price.
/// Sizes grow away from the price with some noise, and some prices hold
/// large resting orders ("walls") that come and go.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DepthBook {
    /// Snapshot times.
    pub time: Vec<Timestamp>,
    /// Prices of each snapshot.
    pub prices: Vec<Vec<f64>>,
    /// Sizes of each snapshot.
    pub sizes: Vec<Vec<f64>>,
}

impl DepthBook {
    /// Snapshot `i`.
    pub fn snapshot(&self, i: usize) -> Option<DepthSnapshot<'_>> {
        Some(DepthSnapshot {
            time: *self.time.get(i)?,
            prices: self.prices.get(i)?,
            sizes: self.sizes.get(i)?,
        })
    }
}

/// See [`DepthBook`].
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a non-positive `interval_ms` or `tick`.
///
/// # Examples
///
/// ```
/// use ferrochart_orderflow::testing::{random_depth, random_trades};
///
/// let t = random_trades(1, 100, 0, 100, 0.25)?;
/// let book = random_depth(1, &t, 1_000, 0.25, 20)?;
/// assert_eq!(book.time, (0..10).map(|i| i * 1_000).collect::<Vec<_>>());
/// assert_eq!(book.snapshot(0).unwrap().prices.len(), 40);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn random_depth(
    seed: u64,
    trades: &TickColumns,
    interval_ms: i64,
    tick: f64,
    depth: usize,
) -> Result<DepthBook> {
    if interval_ms <= 0 || !(tick.is_finite() && tick > 0.0) {
        return Err(Error::InvalidArgument(
            "interval_ms and tick must be positive",
        ));
    }
    let mut book = DepthBook::default();
    let (Some(&first), Some(&last)) = (trades.time.first(), trades.time.last()) else {
        return Ok(book);
    };
    let mut rng = Rng(seed.max(1).wrapping_mul(0xD1B5_4A32_D192_ED03) | 1);
    let mut k = 0;
    let mut t = first;
    while t <= last {
        while k + 1 < trades.len() && trades.time[k + 1] <= t {
            k += 1;
        }
        let mid = (trades.price[k] / tick).round() as i64;
        let (mut prices, mut sizes) =
            (Vec::with_capacity(2 * depth), Vec::with_capacity(2 * depth));
        // Bids from the best down, then asks from the best up (book order
        // per side; levels never repeat).
        for level in (mid - depth as i64..mid)
            .rev()
            .chain(mid + 1..=mid + depth as i64)
        {
            let d = (level - mid).unsigned_abs() as f64;
            let mut size = (1.0 + d * 0.4) * (0.5 + rng.next());
            // A wall every 12 ticks, present in 3 periods of 4 (by level and
            // time), shifted per level so they come and go at different times.
            let phase = (t / interval_ms / 45 + level.rem_euclid(5)) % 4;
            if level.rem_euclid(12) == 0 && phase != 0 {
                size += 150.0 + 100.0 * rng.next();
            }
            prices.push(level as f64 * tick);
            sizes.push(size.round());
        }
        book.time.push(t);
        book.prices.push(prices);
        book.sizes.push(sizes);
        t = t.saturating_add(interval_ms);
    }
    Ok(book)
}

/// Golden scene of a heatmap under 1 min candles, from the trades of
/// [`footprint_scene`] (12 000 trades 200 ms apart, 0.25 ticks): 1 s columns
/// of depth snapshots (40 levels a side) or of traded volume, log colors
/// from 0 to the visible p99, at `spacing` px per candle, scrolled to the
/// end. Returns the heatmap and the candles. The chart's
/// `max_bar_spacing` must allow `spacing`.
///
/// # Errors
///
/// The chart's errors.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::{Css, SizeCss};
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_orderflow::HeatmapSource;
/// use ferrochart_orderflow::testing::heatmap_scene;
///
/// let mut chart = Chart::new(ChartOptions { max_bar_spacing: Css(120.0), ..ChartOptions::default() });
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// heatmap_scene(&mut chart, 0, HeatmapSource::Depth, 120.0)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn heatmap_scene(
    chart: &mut Chart,
    start: Timestamp,
    source: HeatmapSource,
    spacing: f64,
) -> Result<(SeriesId, SeriesId)> {
    let trades = random_trades(5, 12_000, start, 200, 0.25)?;
    let pane = chart.main_pane();
    let heatmap = chart.add_custom_series(
        pane,
        Box::new(Heatmap::new(HeatmapOptions::new(0.25, 1_000, source))?),
        SeriesOptions::default(),
    )?;
    match source {
        HeatmapSource::Depth => {
            let book = random_depth(3, &trades, 1_000, 0.25, 40)?;
            for i in 0..book.time.len() {
                if let Some(s) = book.snapshot(i) {
                    append_depth(chart, heatmap, &s)?;
                }
            }
        }
        HeatmapSource::Trades => append_trades(chart, heatmap, &trades)?,
    }
    let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
    chart.set_data(candles, SeriesData::Ohlc(candles_of(&trades, 60_000)?))?;
    chart.time_scale().set_bar_spacing(spacing);
    chart.scroll_to_realtime();
    Ok((heatmap, candles))
}

/// `n` bars `interval_ms` apart from `start` on a walk of `tick` steps
/// from 100 that reverts toward a level that moves once a UTC day, so each
/// day's profile is roughly bell shaped. Each bar moves up to 3 ticks and
/// has wicks of up to 2 ticks; volume is unset.
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a non-positive `interval_ms` or `tick`.
///
/// # Examples
///
/// ```
/// let bars = ferrochart_orderflow::testing::random_bars(1, 1_440, 0, 60_000, 0.25)?;
/// assert_eq!(bars.time.len(), 1_440);
/// assert!(bars.low.iter().zip(&bars.high).all(|(l, h)| l <= h));
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn random_bars(
    seed: u64,
    n: usize,
    start: Timestamp,
    interval_ms: i64,
    tick: f64,
) -> Result<OhlcColumns> {
    if interval_ms <= 0 || !(tick.is_finite() && tick > 0.0) {
        return Err(Error::InvalidArgument(
            "interval_ms and tick must be positive",
        ));
    }
    let mut rng = Rng(seed.max(1).wrapping_mul(0xA24B_AED4_963E_E407) | 1);
    let mut c = OhlcColumns::default();
    for v in [&mut c.open, &mut c.high, &mut c.low, &mut c.close] {
        v.try_reserve_exact(n).map_err(|_| Error::OutOfMemory)?;
    }
    c.time
        .try_reserve_exact(n)
        .map_err(|_| Error::OutOfMemory)?;
    let base = (100.0 / tick).round() as i64;
    let (mut p, mut anchor) = (base, base);
    let mut day = None;
    let mut time = start;
    for _ in 0..n {
        let d = time.div_euclid(86_400_000);
        if day != Some(d) {
            if day.is_some() {
                anchor += ((rng.next() - 0.5) * 60.0).round() as i64;
            }
            day = Some(d);
        }
        let mut step = ((rng.next() - 0.5) * 7.0).round() as i64;
        if p > anchor + 12 {
            step -= 1;
        } else if p < anchor - 12 {
            step += 1;
        }
        let close = p + step;
        let high = p.max(close) + (rng.next() * 3.0).floor() as i64;
        let low = p.min(close) - (rng.next() * 3.0).floor() as i64;
        c.time.push(time);
        c.open.push(p as f64 * tick);
        c.high.push(high as f64 * tick);
        c.low.push(low as f64 * tick);
        c.close.push(close as f64 * tick);
        p = close;
        time = time.saturating_add(interval_ms);
    }
    Ok(c)
}

/// Golden scene of a TPO profile: `days` UTC days of 1 min bars
/// ([`random_bars`], 0.25 ticks) in 9:30–16:00 New York sessions (14:30
/// UTC, 6.5 h: periods A–M), 30 min periods, rows of 4 ticks, at `spacing`
/// px per session, the last session half a slot from the right edge. The
/// chart's `max_bar_spacing` must allow `spacing`.
///
/// # Errors
///
/// The chart's errors.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::{Css, SizeCss};
/// use ferrochart_core::theme::Theme;
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_orderflow::testing::tpo_scene;
///
/// let mut chart = Chart::new(ChartOptions { max_bar_spacing: Css(240.0), ..ChartOptions::default() });
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// tpo_scene(&mut chart, &Theme::light(), 0, 3, 230.0)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn tpo_scene(
    chart: &mut Chart,
    theme: &Theme,
    start: Timestamp,
    days: usize,
    spacing: f64,
) -> Result<SeriesId> {
    let mut opts = TpoOptions::new(0.25, theme);
    opts.row_size = 4;
    opts.session_offset_ms = 14 * 3_600_000 + 1_800_000;
    opts.session_ms = 6 * 3_600_000 + 1_800_000;
    let id = chart.add_custom_series(
        chart.main_pane(),
        Box::new(Tpo::new(opts)?),
        SeriesOptions::default(),
    )?;
    append_bars(
        chart,
        id,
        &random_bars(9, days * 1_440, start, 60_000, 0.25)?,
    )?;
    chart.time_scale().set_bar_spacing(spacing);
    chart.scroll_to_realtime();
    chart.time_scale().set_right_offset(0.5);
    Ok(id)
}
