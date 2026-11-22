//! Heatmap (rules in `docs/book/src/orderflow.md`):
//! resting size from order book snapshots, or traded volume, per time column
//! and price level, colored by a log or linear scale.
//!
//! Both drawing paths share one pixel [`Layout`] computed on the core side:
//! the pixel columns each data column covers, the pixel rows of each level
//! and the color-scale range. The portable path aggregates on the CPU and
//! writes rectangles; the native path (`crate::gpu`, feature `gpu`) does the
//! same aggregation in a compute shader.

use core::cell::{Cell, Ref, RefCell};
use core::ops::Range;

use ferrochart_core::data::Timestamp;
use ferrochart_core::drawing::FrameBuilder;
use ferrochart_core::error::{Error, Result};
use ferrochart_core::geom::{RectPx, Rgba};
use ferrochart_core::plugin::{AxisMode, CustomSeries, SeriesCtx};

use crate::grid::{Combine, SparseGrid};
use crate::levels::PriceLevels;
use crate::trades::TickColumns;

/// The kind name of heatmap series.
pub const HEATMAP_KIND: &str = "orderflow.heatmap";

/// Nothing is drawn when the plot shows more than this many levels per
/// pixel row (levels are far below a pixel; zoom the price scale in).
pub const MAX_LEVELS_PER_ROW: u32 = 8;

/// What a heatmap aggregates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HeatmapSource {
    /// Order book snapshots ([`crate::append_depth`]): the largest resting
    /// size of each level within a column.
    #[default]
    Depth,
    /// Trades ([`crate::append_trades`]): the volume traded at each level
    /// within a column.
    Trades,
}

/// How values map to the color ramp.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorScale {
    /// Proportional to the value.
    Linear,
    /// Proportional to `ln(1 + value)`.
    #[default]
    Log,
}

/// The value range of the color ramp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HeatmapRange {
    /// From 0 to the given quantile (in (0, 1]) of the visible cells, rounded
    /// up to a quarter of its octave so it does not flicker while panning.
    Auto {
        /// E.g. 0.99: the top 1% of cells get the last color.
        percentile: f32,
    },
    /// Fixed.
    Fixed {
        /// Value of the first color (≥ 0).
        lo: f32,
        /// Value of the last color (> `lo`).
        hi: f32,
    },
}

impl Default for HeatmapRange {
    fn default() -> Self {
        Self::Auto { percentile: 0.99 }
    }
}

/// Colors of a heatmap: five evenly spaced stops, from the smallest to the
/// largest value, interpolated per channel (alpha included).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeatmapColors {
    /// The stops.
    pub stops: [Rgba; 5],
}

impl Default for HeatmapColors {
    /// Translucent blue through cyan and yellow to opaque orange-red.
    fn default() -> Self {
        Self {
            stops: [
                Rgba::hex(0x0D_47A1).with_alpha(80),
                Rgba::hex(0x1E_88E5).with_alpha(150),
                Rgba::hex(0x26_C6DA).with_alpha(200),
                Rgba::hex(0xFF_EE58).with_alpha(235),
                Rgba::hex(0xFF_5722),
            ],
        }
    }
}

/// Options of a [`Heatmap`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeatmapOptions {
    /// Minimum price increment.
    pub tick_size: f64,
    /// Ticks per row.
    pub row_size: u32,
    /// Column length, ms (e.g. 1 000; may be finer than the candles).
    /// Columns start at multiples of it (UTC).
    pub column_ms: i64,
    /// Depth snapshots or trades.
    pub source: HeatmapSource,
    /// Log or linear colors.
    pub scale: ColorScale,
    /// Range of the colors.
    pub range: HeatmapRange,
    /// Whether the heatmap's levels take part in auto scaling. Off by
    /// default: a book usually spans far more than the candles.
    pub auto_scale: bool,
    /// Colors.
    pub colors: HeatmapColors,
}

impl HeatmapOptions {
    /// One tick per row, log colors from 0 to the visible p99, no auto
    /// scaling, the default colors.
    pub fn new(tick_size: f64, column_ms: i64, source: HeatmapSource) -> Self {
        Self {
            tick_size,
            row_size: 1,
            column_ms,
            source,
            scale: ColorScale::Log,
            range: HeatmapRange::default(),
            auto_scale: false,
            colors: HeatmapColors::default(),
        }
    }
}

/// One order book snapshot: the resting size at each price (bids and asks
/// together). Prices are expected in book order (sorted either way):
/// adjacent prices in the same row are added up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthSnapshot<'a> {
    /// Time, UTC ms.
    pub time: Timestamp,
    /// Prices.
    pub prices: &'a [f64],
    /// Resting sizes (≥ 0; 0 is ignored).
    pub sizes: &'a [f64],
}

impl DepthSnapshot<'_> {
    fn check(&self, levels: PriceLevels) -> Result<()> {
        if self.prices.len() != self.sizes.len() {
            return Err(Error::InvalidArgument(
                "depth prices and sizes must have the same length",
            ));
        }
        if self.prices.iter().any(|&p| levels.level(p).is_none()) {
            return Err(Error::InvalidArgument(
                "depth prices must be finite and in range",
            ));
        }
        if self.sizes.iter().any(|s| !(s.is_finite() && *s >= 0.0)) {
            return Err(Error::InvalidArgument(
                "depth sizes must be finite and non-negative",
            ));
        }
        Ok(())
    }
}

/// The color-scale range of a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Scale {
    pub(crate) lo: f32,
    pub(crate) hi: f32,
    pub(crate) log: bool,
}

/// Where a frame's cells go, in whole pixels; shared by both paths.
#[derive(Clone, Debug, Default)]
pub(crate) struct Layout {
    /// Whether there is anything to draw.
    pub(crate) valid: bool,
    /// The plot: left, top, width, height.
    pub(crate) x0: i32,
    pub(crate) y0: i32,
    pub(crate) w: u32,
    pub(crate) h: u32,
    /// Column of `col_px[0]`.
    pub(crate) first: usize,
    /// Per column: pixel columns `[p0, p1)` from the plot's left (empty
    /// when off the plot).
    pub(crate) col_px: Vec<[u32; 2]>,
    /// Level of `level_band[0]`.
    pub(crate) level_lo: i32,
    /// Per level from `level_lo`: its band, or -1 when it has no pixel row.
    pub(crate) level_band: Vec<i32>,
    /// Per plot row: its band, or -1.
    pub(crate) row_band: Vec<i32>,
    /// Per band: plot rows `[r0, r1)`.
    pub(crate) band_rows: Vec<[u32; 2]>,
    pub(crate) scale: Scale,
}

type AutoKey = (u64, usize, usize, i32, i32, u32);

/// A heatmap series: add it with
/// [`Chart::add_custom_series`](ferrochart_core::Chart::add_custom_series)
/// and feed it with [`crate::append_depth`] or [`crate::append_trades`].
///
/// Columns are placed by time between the candles' slots
/// (`AxisMode::Overlay`), so 1 s columns on a 1 min chart add no slots.
/// Each pixel column shows the largest value of the data columns it
/// covers; each level fills the pixel rows between its rounded edges.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::SeriesOptions;
/// use ferrochart_core::testing::MonoMeasurer;
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_orderflow::{DepthSnapshot, Heatmap, HeatmapOptions, HeatmapSource, append_depth};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let opts = HeatmapOptions::new(0.5, 1_000, HeatmapSource::Depth);
/// let id = chart.add_custom_series(chart.main_pane(), Box::new(Heatmap::new(opts)?), SeriesOptions::default())?;
/// let (prices, sizes) = ([99.5, 100.0, 100.5], [12.0, 3.0, 40.0]);
/// append_depth(&mut chart, id, &DepthSnapshot { time: 0, prices: &prices, sizes: &sizes })?;
/// append_depth(&mut chart, id, &DepthSnapshot { time: 500, prices: &prices[..1], sizes: &[20.0] })?;
/// let h = chart.custom_series::<Heatmap>(id).unwrap();
/// // One column; the largest size of each level (100.0 is level 200).
/// assert_eq!(h.grid().columns(), 1);
/// assert_eq!(h.grid().column(0).unwrap().at(199), (20.0, 0.0));
/// chart.build_frame(0.0, &MonoMeasurer);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Debug)]
pub struct Heatmap {
    opts: HeatmapOptions,
    grid: SparseGrid,
    /// Bumped on every data change (keys the auto range).
    version: u64,
    layout: RefCell<Layout>,
    /// Portable path: band × pixel column maxima.
    scratch: RefCell<Vec<f32>>,
    auto: Cell<Option<(AutoKey, f32)>>,
}

fn check_display(range: HeatmapRange) -> Result<()> {
    let ok = match range {
        HeatmapRange::Auto { percentile } => percentile > 0.0 && percentile <= 1.0,
        HeatmapRange::Fixed { lo, hi } => lo.is_finite() && hi.is_finite() && lo >= 0.0 && hi > lo,
    };
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidArgument(
            "heatmap range: percentile in (0, 1], or 0 <= lo < hi",
        ))
    }
}

impl Heatmap {
    /// An empty heatmap.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for a bad price grid, a non-positive
    /// column length or a bad range.
    pub fn new(opts: HeatmapOptions) -> Result<Self> {
        let levels = PriceLevels::new(opts.tick_size, opts.row_size)?;
        if opts.column_ms <= 0 {
            return Err(Error::InvalidArgument("column_ms must be positive"));
        }
        check_display(opts.range)?;
        Ok(Self {
            opts,
            grid: SparseGrid::new(levels),
            version: 0,
            layout: RefCell::new(Layout::default()),
            scratch: RefCell::new(Vec::new()),
            auto: Cell::new(None),
        })
    }

    /// The options.
    pub fn options(&self) -> &HeatmapOptions {
        &self.opts
    }

    /// Changes the colors and their scale (the data stays).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for a bad range (nothing changes).
    pub fn set_display(
        &mut self,
        scale: ColorScale,
        range: HeatmapRange,
        colors: HeatmapColors,
    ) -> Result<()> {
        check_display(range)?;
        self.opts.scale = scale;
        self.opts.range = range;
        self.opts.colors = colors;
        self.auto.set(None);
        Ok(())
    }

    /// The cells: `a` is the size (`b` is unused).
    pub fn grid(&self) -> &SparseGrid {
        &self.grid
    }

    fn check_column(&self, time: Timestamp) -> Result<Timestamp> {
        let col = crate::column_start(time, self.opts.column_ms);
        if self.grid.times().last().is_some_and(|&t| col < t) {
            return Err(Error::TimeNotIncreasing {
                index: self.grid.columns(),
            });
        }
        Ok(col)
    }

    /// Adds one trade to a trade heatmap; returns whether it opened a column.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for a depth heatmap, a non-finite price
    /// or a negative size; [`Error::TimeNotIncreasing`] if its column is
    /// before the last one; [`Error::OutOfMemory`].
    pub fn push_trade(&mut self, time: Timestamp, price: f64, size: f64) -> Result<bool> {
        if self.opts.source != HeatmapSource::Trades {
            return Err(Error::InvalidArgument("the heatmap takes depth snapshots"));
        }
        if !(size.is_finite() && size >= 0.0) {
            return Err(Error::InvalidArgument(
                "trade sizes must be finite and non-negative",
            ));
        }
        let level = self
            .grid
            .price_levels()
            .level(price)
            .ok_or(Error::InvalidArgument("trade prices must be finite"))?;
        let col = self.check_column(time)?;
        if size == 0.0 {
            return Ok(false);
        }
        let opened = self.grid.add(col, level, size as f32, 0.0, Combine::Sum)?;
        self.version = self.version.wrapping_add(1);
        Ok(opened)
    }

    /// Adds an order book snapshot to a depth heatmap; returns whether it
    /// opened a column. Within a column each level keeps its largest size.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for a trade heatmap or a bad snapshot
    /// (lengths, non-finite prices, negative sizes; nothing changes);
    /// [`Error::TimeNotIncreasing`] if its column is before the last one;
    /// [`Error::OutOfMemory`] (the levels added so far stay).
    pub fn push_depth(&mut self, snapshot: &DepthSnapshot<'_>) -> Result<bool> {
        if self.opts.source != HeatmapSource::Depth {
            return Err(Error::InvalidArgument("the heatmap takes trades"));
        }
        let levels = self.grid.price_levels();
        snapshot.check(levels)?;
        let col = self.check_column(snapshot.time)?;
        let mut opened = false;
        let mut run: Option<(i32, f64)> = None;
        for (&price, &size) in snapshot.prices.iter().zip(snapshot.sizes) {
            if size == 0.0 {
                continue;
            }
            let Some(level) = levels.level(price) else {
                continue;
            };
            match run {
                Some((l, sum)) if l == level => run = Some((l, sum + size)),
                _ => {
                    if let Some((l, sum)) = run {
                        opened |= self.grid.add(col, l, sum as f32, 0.0, Combine::Max)?;
                        self.version = self.version.wrapping_add(1);
                    }
                    run = Some((level, size));
                }
            }
        }
        if let Some((l, sum)) = run {
            opened |= self.grid.add(col, l, sum as f32, 0.0, Combine::Max)?;
            self.version = self.version.wrapping_add(1);
        }
        Ok(opened)
    }

    /// Bumped on every data change.
    pub(crate) fn version(&self) -> u64 {
        self.version
    }

    /// The layout of the last frame (native path).
    #[cfg_attr(not(feature = "gpu"), allow(dead_code))]
    pub(crate) fn layout(&self) -> Option<Ref<'_, Layout>> {
        self.layout.try_borrow().ok()
    }

    /// Fills `l` for points `from..to`.
    fn compute_layout(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, l: &mut Layout) {
        l.valid = false;
        l.col_px.clear();
        l.level_band.clear();
        l.row_band.clear();
        l.band_rows.clear();
        let c = ctx.coords();
        let plot = c.plot();
        let (x0, x1) = (plot.x.round(), (plot.x + plot.w).round());
        let (y0, y1) = (plot.y.round(), (plot.y + plot.h).round());
        if !(x1 > x0 && y1 > y0) {
            return;
        }
        let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
        (l.x0, l.y0, l.w, l.h) = (x0 as i32, y0 as i32, w, h);

        // Levels: the plot's price range plus one on each side.
        let levels = self.grid.price_levels();
        let (pa, pb) = (c.price_at(plot.y), c.price_at(plot.y + plot.h));
        let (Some(lo), Some(hi)) = (levels.level(pa.min(pb)), levels.level(pa.max(pb))) else {
            return;
        };
        let (lo, hi) = (i64::from(lo) - 1, i64::from(hi) + 1);
        if hi - lo + 1 > i64::from(h) * i64::from(MAX_LEVELS_PER_ROW) {
            return;
        }
        let Ok(level_lo) = i32::try_from(lo) else {
            return;
        };
        l.level_lo = level_lo;
        l.row_band.resize(h as usize, -1);
        let step = levels.step();
        // Lower edge of level k; level k spans edge(k)..edge(k + 1), so
        // neighbours share an edge and their rows never overlap.
        let edge = |k: i64| (k as f64 - 0.5) * step;
        let fh = h as f32;
        for k in lo..=hi {
            let band = match (c.y_of(edge(k + 1)), c.y_of(edge(k))) {
                (Some(ya), Some(yb)) => {
                    let r0 = (ya.min(yb).round() - y0).clamp(0.0, fh) as u32;
                    let r1 = (ya.max(yb).round() - y0).clamp(0.0, fh) as u32;
                    if r1 > r0 {
                        let b = l.band_rows.len() as i32;
                        l.band_rows.push([r0, r1]);
                        l.row_band[r0 as usize..r1 as usize].fill(b);
                        b
                    } else {
                        -1
                    }
                }
                _ => -1,
            };
            l.level_band.push(band);
        }

        // Columns: from the column's start to its end, at least one pixel.
        // x of a time plus the series' shift (constant in pixels); a column
        // that starts where the previous one ended reuses its end.
        let times = self.grid.times();
        let period = self.opts.column_ms;
        l.first = from;
        let clip = |p: i64| p.clamp(0, i64::from(w)) as u32;
        let shift = match (times.get(from), ctx.x_of_point(from)) {
            (Some(&t), Some(x)) => c.x_of(t).map_or(0.0, |a| x - a),
            _ => 0.0,
        };
        let x_at = |t: Timestamp| c.x_of(t).map(|x| x + shift);
        let mut prev_end: Option<(Timestamp, Option<f32>)> = None;
        for i in from..to {
            let px = match times.get(i) {
                Some(&t) => {
                    let xl = match prev_end {
                        Some((end, x)) if end == t => x,
                        _ => x_at(t),
                    };
                    let end = t.saturating_add(period);
                    let xr = x_at(end);
                    prev_end = Some((end, xr));
                    match (xl, xr) {
                        (Some(xl), Some(xr)) => {
                            let p0 = (xl.round() - x0) as i64;
                            let p1 = ((xr.round() - x0) as i64).max(p0 + 1);
                            [clip(p0), clip(p1)]
                        }
                        _ => [0, 0],
                    }
                }
                None => [0, 0],
            };
            l.col_px.push(px);
        }

        l.scale = match self.opts.range {
            HeatmapRange::Fixed { lo, hi } => Scale {
                lo,
                hi,
                log: self.opts.scale == ColorScale::Log,
            },
            HeatmapRange::Auto { percentile } => Scale {
                lo: 0.0,
                hi: self.auto_hi(from, to, level_lo, hi as i32, percentile, w),
                log: self.opts.scale == ColorScale::Log,
            },
        };
        l.valid = true;
    }

    /// The `percentile` quantile of the non-zero cells of columns
    /// `from..to` and levels `lo..=hi`, rounded up to a quarter of its
    /// octave. With more than 4 columns per pixel column (`w` wide), only
    /// columns whose number is a multiple of `ceil(columns / 4w)` count, so
    /// the sample does not change while panning.
    fn auto_hi(&self, from: usize, to: usize, lo: i32, hi: i32, percentile: f32, w: u32) -> f32 {
        let key = (self.version, from, to, lo, hi, percentile.to_bits());
        if let Some((k, v)) = self.auto.get()
            && k == key
        {
            return v;
        }
        let mut bins = [0_u32; BINS];
        let mut n = 0_u64;
        let stride = (to - from).div_ceil(4 * w.max(1) as usize).max(1);
        for i in (from.next_multiple_of(stride)..to).step_by(stride) {
            let Some(col) = self.grid.column(i) else {
                continue;
            };
            let a = col.levels.partition_point(|&x| x < lo);
            let b = col.levels.partition_point(|&x| x <= hi);
            for &v in &col.a[a..b.max(a)] {
                if v > 0.0 {
                    bins[bin(v)] += 1;
                    n += 1;
                }
            }
        }
        let v = if n == 0 {
            1.0
        } else {
            let target = ((f64::from(percentile) * n as f64).ceil() as u64).clamp(1, n);
            let mut acc = 0_u64;
            let mut k = BINS - 1;
            for (j, &count) in bins.iter().enumerate() {
                acc += u64::from(count);
                if acc >= target {
                    k = j;
                    break;
                }
            }
            bin_upper(k)
        };
        self.auto.set(Some((key, v)));
        v
    }

    /// Portable path: per-pixel maxima, then one rectangle per run of equal
    /// color in each band.
    fn draw_cells(&self, l: &Layout, out: &mut FrameBuilder<'_>) {
        let Ok(mut buf) = self.scratch.try_borrow_mut() else {
            return;
        };
        let w = l.w as usize;
        buf.clear();
        buf.resize(w * l.band_rows.len(), 0.0);
        let lo = l.level_lo;
        let hi = i64::from(lo) + l.level_band.len() as i64 - 1;
        for (j, &[p0, p1]) in l.col_px.iter().enumerate() {
            if p0 >= p1 {
                continue;
            }
            let Some(col) = self.grid.column(l.first + j) else {
                continue;
            };
            let a = col.levels.partition_point(|&x| x < lo);
            let b = col.levels.partition_point(|&x| i64::from(x) <= hi);
            for k in a..b.max(a) {
                let rel = (i64::from(col.levels[k]) - i64::from(lo)) as usize;
                let (Some(&band), v) = (l.level_band.get(rel), col.a[k]) else {
                    continue;
                };
                if band < 0 || v <= 0.0 {
                    continue;
                }
                let start = band as usize * w;
                for c in &mut buf[start + p0 as usize..start + p1 as usize] {
                    if v > *c {
                        *c = v;
                    }
                }
            }
        }
        let stops = &self.opts.colors.stops;
        for (band, &[r0, r1]) in l.band_rows.iter().enumerate() {
            let row = &buf[band * w..(band + 1) * w];
            let mut run: Option<(usize, Rgba)> = None;
            let mut last = (0_u32, None::<Rgba>);
            for x in 0..=w {
                let color = match row.get(x) {
                    Some(&v) if v > 0.0 => {
                        if last.0 != v.to_bits() || last.1.is_none() {
                            last = (v.to_bits(), Some(ramp(scale_t(v, l.scale), stops)));
                        }
                        last.1
                    }
                    _ => None,
                };
                if run.map(|r| r.1) == color {
                    continue;
                }
                if let Some((s, c)) = run
                    && c.a > 0
                {
                    out.rect(
                        RectPx {
                            x: (l.x0 + s as i32) as f32,
                            y: (l.y0 + r0 as i32) as f32,
                            w: (x - s) as f32,
                            h: (r1 - r0) as f32,
                        },
                        c,
                    );
                }
                run = color.map(|c| (x, c));
            }
        }
    }
}

/// Octaves from 2^-32 to 2^32, each split into 4 equal parts.
const BINS: usize = 256;

/// The bin of a positive finite `v`, from its bits: the exponent and the two
/// top mantissa bits (no `log2` per cell).
fn bin(v: f32) -> usize {
    let bits = v.to_bits();
    let exp = ((bits >> 23) & 0xFF) as i32 - 127;
    let quarter = ((bits >> 21) & 3) as i32;
    ((exp + 32) * 4 + quarter).clamp(0, BINS as i32 - 1) as usize
}

/// The upper edge of bin `k`: `2^e × (1 + (q + 1)/4)`.
fn bin_upper(k: usize) -> f32 {
    let (e, q) = ((k / 4) as i32 - 32, (k % 4) as f32);
    (e as f32).exp2() * (1.0 + (q + 1.0) / 4.0)
}

/// Position of `v` on the ramp, 0–1. Shader: `scale_t()` in
/// `heatmap.wgsl`, same operations in the same order.
pub(crate) fn scale_t(v: f32, s: Scale) -> f32 {
    let (fv, flo, fhi) = if s.log {
        ((1.0 + v).ln(), (1.0 + s.lo).ln(), (1.0 + s.hi).ln())
    } else {
        (v, s.lo, s.hi)
    };
    let d = fhi - flo;
    if d.is_nan() || d <= 0.0 {
        return 1.0;
    }
    ((fv - flo) / d).clamp(0.0, 1.0)
}

/// The ramp's color at `t`. Shader: `ramp()` in `heatmap.wgsl`.
pub(crate) fn ramp(t: f32, stops: &[Rgba; 5]) -> Rgba {
    let s = t * 4.0;
    let k = s.floor().clamp(0.0, 3.0);
    let f = s - k;
    let (a, b) = (stops[k as usize], stops[k as usize + 1]);
    let mix = |x: u8, y: u8| {
        let (x, y) = (f32::from(x), f32::from(y));
        (x + (y - x) * f + 0.5).floor() as u8
    };
    Rgba {
        r: mix(a.r, b.r),
        g: mix(a.g, b.g),
        b: mix(a.b, b.b),
        a: mix(a.a, b.a),
    }
}

impl crate::TradeSink for Heatmap {
    fn timeframe(&self) -> i64 {
        self.opts.column_ms
    }

    fn last_column(&self) -> Option<Timestamp> {
        self.grid.times().last().copied()
    }

    fn apply(&mut self, trades: &TickColumns, range: Range<usize>) -> Result<()> {
        for i in range {
            let (t, p, s, _) = trades.get(i);
            self.push_trade(t, p, s)?;
        }
        Ok(())
    }
}

impl CustomSeries for Heatmap {
    fn kind(&self) -> &'static str {
        HEATMAP_KIND
    }

    fn axis_mode(&self) -> AxisMode {
        AxisMode::Overlay
    }

    fn times(&self) -> &[Timestamp] {
        self.grid.times()
    }

    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        if self.opts.auto_scale {
            self.grid.extremes(from, to)
        } else {
            None
        }
    }

    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
        let Ok(mut l) = self.layout.try_borrow_mut() else {
            return;
        };
        self.compute_layout(ctx, from, to, &mut l);
        if l.valid {
            self.draw_cells(&l, out);
        }
    }

    /// `[lo, hi, log (1/0), …]` of the color scale; also lays out the frame
    /// for the native path.
    fn native_params(&self, ctx: &SeriesCtx<'_>, params: &mut [f32; 8]) {
        let Ok(mut l) = self.layout.try_borrow_mut() else {
            return;
        };
        let (from, to) = ctx.visible();
        self.compute_layout(ctx, from, to, &mut l);
        params[0] = l.scale.lo;
        params[1] = l.scale.hi;
        params[2] = if l.scale.log { 1.0 } else { 0.0 };
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;

    fn depth() -> Heatmap {
        Heatmap::new(HeatmapOptions::new(1.0, 1_000, HeatmapSource::Depth)).unwrap()
    }

    #[test]
    fn snapshots_keep_the_largest_size() {
        let mut h = depth();
        let snap = |time, prices: &'static [f64], sizes: &'static [f64]| DepthSnapshot {
            time,
            prices,
            sizes,
        };
        assert!(
            h.push_depth(&snap(10, &[1.0, 2.0, 3.0], &[5.0, 0.0, 2.0]))
                .unwrap()
        );
        assert!(!h.push_depth(&snap(900, &[1.0, 3.0], &[4.0, 6.0])).unwrap());
        assert!(h.push_depth(&snap(1_000, &[2.0], &[1.0])).unwrap());
        let c = h.grid().column(0).unwrap();
        assert_eq!((c.levels, c.a), (&[1, 3][..], &[5.0, 6.0][..]));
        assert_eq!(h.grid().times(), &[0, 1_000]);
        // Adjacent prices in one row add up.
        let mut g = Heatmap::new(HeatmapOptions {
            row_size: 2,
            ..HeatmapOptions::new(1.0, 1_000, HeatmapSource::Depth)
        })
        .unwrap();
        g.push_depth(&snap(0, &[4.0, 4.4, 5.0], &[1.0, 2.0, 3.0]))
            .unwrap();
        assert_eq!(g.grid().column(0).unwrap().at(2), (3.0, 0.0));
        // Errors leave the data alone.
        assert!(h.push_depth(&snap(1, &[1.0], &[1.0])).is_err());
        assert!(h.push_depth(&snap(2_000, &[1.0, 2.0], &[1.0])).is_err());
        assert!(h.push_depth(&snap(2_000, &[f64::NAN], &[1.0])).is_err());
        assert!(h.push_depth(&snap(2_000, &[1.0], &[-1.0])).is_err());
        assert!(h.push_trade(2_000, 1.0, 1.0).is_err());
        assert_eq!(h.grid().columns(), 2);
    }

    #[test]
    fn options_are_checked() {
        let mut o = HeatmapOptions::new(1.0, 0, HeatmapSource::Trades);
        assert!(Heatmap::new(o).is_err());
        o.column_ms = 1_000;
        for range in [
            HeatmapRange::Auto { percentile: 0.0 },
            HeatmapRange::Auto { percentile: 1.5 },
            HeatmapRange::Auto {
                percentile: f32::NAN,
            },
            HeatmapRange::Fixed { lo: 2.0, hi: 1.0 },
            HeatmapRange::Fixed { lo: -1.0, hi: 1.0 },
        ] {
            o.range = range;
            assert!(Heatmap::new(o).is_err(), "{range:?}");
        }
        o.range = HeatmapRange::Fixed { lo: 0.0, hi: 10.0 };
        let mut h = Heatmap::new(o).unwrap();
        assert!(
            h.push_depth(&DepthSnapshot {
                time: 0,
                prices: &[1.0],
                sizes: &[1.0]
            })
            .is_err()
        );
        h.push_trade(5, 1.0, 2.0).unwrap();
        h.push_trade(6, 1.0, 3.0).unwrap();
        assert_eq!(h.grid().column(0).unwrap().at(1), (5.0, 0.0));
        let colors = HeatmapColors::default();
        assert!(
            h.set_display(
                ColorScale::Linear,
                HeatmapRange::Auto { percentile: 2.0 },
                colors
            )
            .is_err()
        );
        assert_eq!(h.options().scale, ColorScale::Log);
    }

    #[test]
    fn scale_and_ramp() {
        let lin = Scale {
            lo: 0.0,
            hi: 10.0,
            log: false,
        };
        assert_eq!(scale_t(5.0, lin), 0.5);
        assert_eq!(scale_t(50.0, lin), 1.0);
        let log = Scale {
            lo: 0.0,
            hi: 99.0,
            log: true,
        };
        assert!((scale_t(9.0, log) - 0.5).abs() < 1e-6);
        assert_eq!(
            scale_t(
                1.0,
                Scale {
                    lo: 2.0,
                    hi: 2.0,
                    log: false
                }
            ),
            1.0
        );
        let stops = HeatmapColors::default().stops;
        assert_eq!(ramp(0.0, &stops), stops[0]);
        assert_eq!(ramp(1.0, &stops), stops[4]);
        assert_eq!(ramp(0.5, &stops), stops[2]);
        let mid = ramp(0.125, &stops);
        assert_eq!(mid.a, 115);
        // 3 = 2 × 1.5 is in [2 × 1.5, 2 × 1.75).
        assert_eq!(bin_upper(bin(3.0)), 3.5);
        assert_eq!(bin_upper(bin(1.0)), 1.25);
        assert_eq!(bin_upper(bin(1.9)), 2.0);
        assert_eq!(bin(0.0), 0);
        assert_eq!(bin(f32::MAX), 255);
    }
}
