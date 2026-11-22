//! The global time scale (bar spacing, right offset, index <-> x) and the
//! weight-based time tick algorithm, with pure-algorithm calendar math.
//!

use core::fmt::{self, Write};

use crate::data::{LogicalIndex, TimeIndex, Timestamp};
use crate::error::{Error, Result};
use crate::geom::Px;

const DAY_MS: i64 = 86_400_000;

/// UTC offset provider for label formatting. Full IANA time zones are injected by
/// the binding layer (JS `Intl`, Python `zoneinfo`); the core only ships [`Utc`]
/// and [`FixedOffset`].
///
/// # Examples
///
/// ```
/// use ferrochart_core::time_scale::{FixedOffset, TimeZoneProvider, Utc};
///
/// assert_eq!(Utc.offset_ms(0), 0);
/// assert_eq!(FixedOffset(8 * 3_600_000).offset_ms(0), 28_800_000);
/// ```
pub trait TimeZoneProvider: Send {
    /// Offset of local time from UTC at instant `utc`, in milliseconds.
    fn offset_ms(&self, utc: Timestamp) -> i64;
}

/// UTC.
#[derive(Clone, Copy, Debug, Default)]
pub struct Utc;

impl TimeZoneProvider for Utc {
    fn offset_ms(&self, _: Timestamp) -> i64 {
        0
    }
}

/// A fixed offset from UTC in milliseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedOffset(pub i64);

impl TimeZoneProvider for FixedOffset {
    fn offset_ms(&self, _: Timestamp) -> i64 {
        self.0
    }
}

/// Civil date `(year, month 1..=12, day 1..=31)` of a day count since
/// 1970-01-01 (Howard Hinnant's `civil_from_days`).
///
/// # Examples
///
/// ```
/// use ferrochart_core::time_scale::civil_from_days;
///
/// assert_eq!(civil_from_days(0), (1970, 1, 1));
/// assert_eq!(civil_from_days(19_723), (2024, 1, 1));
/// assert_eq!(civil_from_days(-1), (1969, 12, 31));
/// ```
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m as u32, d as u32)
}

/// How big a calendar boundary a time label marks. Higher is more important.
///
/// # Examples
///
/// ```
/// use ferrochart_core::time_scale::TickWeight;
///
/// assert!(TickWeight::Year > TickWeight::Month);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TickWeight {
    /// A new second.
    Second,
    /// A new minute.
    Minute,
    /// A new 5-minute slot.
    Min5,
    /// A new 15-minute slot.
    Min15,
    /// A new 30-minute slot.
    Min30,
    /// A new hour.
    Hour,
    /// A new day.
    Day,
    /// A new month.
    Month,
    /// A new year.
    Year,
}

const WEIGHTS_DESC: [TickWeight; 9] = [
    TickWeight::Year,
    TickWeight::Month,
    TickWeight::Day,
    TickWeight::Hour,
    TickWeight::Min30,
    TickWeight::Min15,
    TickWeight::Min5,
    TickWeight::Minute,
    TickWeight::Second,
];

/// A number identifying the calendar unit of `weight` that local time `t` lies in.
fn unit(t: i64, weight: TickWeight) -> i64 {
    let slot = |ms: i64| t.div_euclid(ms);
    match weight {
        TickWeight::Year => civil_from_days(t.div_euclid(DAY_MS)).0,
        TickWeight::Month => {
            let (y, m, _) = civil_from_days(t.div_euclid(DAY_MS));
            y * 12 + i64::from(m)
        }
        TickWeight::Day => slot(DAY_MS),
        TickWeight::Hour => slot(3_600_000),
        TickWeight::Min30 => slot(1_800_000),
        TickWeight::Min15 => slot(900_000),
        TickWeight::Min5 => slot(300_000),
        TickWeight::Minute => slot(60_000),
        TickWeight::Second => slot(1000),
    }
}

/// The largest calendar boundary crossed going from local time `a` to `b`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::time_scale::{weight_between, TickWeight};
///
/// let jan1_2024 = 1_704_067_200_000;
/// assert_eq!(weight_between(jan1_2024 - 60_000, jan1_2024), TickWeight::Year);
/// assert_eq!(weight_between(jan1_2024, jan1_2024 + 60_000), TickWeight::Minute);
/// ```
pub fn weight_between(a: i64, b: i64) -> TickWeight {
    WEIGHTS_DESC
        .into_iter()
        .find(|&w| unit(a, w) != unit(b, w))
        .unwrap_or(TickWeight::Second)
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Writes the label of a tick of `weight` at local time `t`: `2024`, `Mar`, `15`,
/// `14:30` or `14:30:05`.
///
/// # Errors
///
/// Only if `out` fails.
///
/// # Examples
///
/// ```
/// use ferrochart_core::text::StackStr;
/// use ferrochart_core::time_scale::{format_tick, TickWeight};
///
/// let t = 1_710_513_000_000; // 2024-03-15T14:30:00Z
/// let mut s = StackStr::<16>::new();
/// format_tick(t, TickWeight::Hour, &mut s).unwrap();
/// assert_eq!(s.as_str(), "14:30");
/// ```
pub fn format_tick(t: i64, weight: TickWeight, out: &mut impl Write) -> fmt::Result {
    let (y, m, d) = civil_from_days(t.div_euclid(DAY_MS));
    let ms = t.rem_euclid(DAY_MS);
    let (hh, mm, ss) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60);
    match weight {
        TickWeight::Year => write!(out, "{y}"),
        TickWeight::Month => out.write_str(MONTHS[(m as usize).saturating_sub(1) % 12]),
        TickWeight::Day => write!(out, "{d}"),
        TickWeight::Second => write!(out, "{hh:02}:{mm:02}:{ss:02}"),
        _ => write!(out, "{hh:02}:{mm:02}"),
    }
}

/// Writes the crosshair label of local time `t`: `2024-03-15 14:30`, or only the
/// date for bars that start at midnight.
///
/// # Errors
///
/// Only if `out` fails.
///
/// # Examples
///
/// ```
/// use ferrochart_core::text::StackStr;
/// use ferrochart_core::time_scale::format_crosshair_time;
///
/// let mut s = StackStr::<32>::new();
/// format_crosshair_time(1_710_513_000_000, &mut s).unwrap();
/// assert_eq!(s.as_str(), "2024-03-15 14:30");
/// ```
pub fn format_crosshair_time(t: i64, out: &mut impl Write) -> fmt::Result {
    let (y, m, d) = civil_from_days(t.div_euclid(DAY_MS));
    let ms = t.rem_euclid(DAY_MS);
    write!(out, "{y:04}-{m:02}-{d:02}")?;
    if ms != 0 {
        write!(out, " {:02}:{:02}", ms / 3_600_000, ms / 60_000 % 60)?;
    }
    Ok(())
}

/// Days since 1970-01-01 of a proleptic Gregorian date (inverse of
/// [`civil_from_days`]).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Steps of [`time_ticks`]: milliseconds, or months when negative.
const TIME_STEPS: [i64; 31] = [
    1000,
    2000,
    5000,
    10_000,
    15_000,
    30_000, // seconds
    60_000,
    120_000,
    300_000,
    600_000,
    900_000,
    1_800_000, // minutes
    3_600_000,
    7_200_000,
    10_800_000,
    21_600_000,
    43_200_000, // hours
    DAY_MS,
    2 * DAY_MS,
    7 * DAY_MS,
    14 * DAY_MS, // days
    -1,
    -2,
    -3,
    -6, // months
    -12,
    -24,
    -60,
    -120,
    -240,
    -600, // years
];

/// Ticks for a numeric axis whose values are local times in milliseconds
/// (`ferrochart-plot`'s date axes): times `lo..=hi` on an axis `length`
/// px long, at least `min_spacing` px apart, aligned to whole seconds,
/// minutes, hours, days, months or years, each with the weight of the
/// calendar boundary it falls on (for [`format_tick`]). Written to `out`
/// (cleared first, ascending); at most 1000 ticks; no allocation once `out`
/// has capacity.
///
/// ```
/// use ferrochart_core::time_scale::{time_ticks, TickWeight};
///
/// let jan1_2024 = 1_704_067_200_000;
/// let year = 366 * 86_400_000;
/// let mut out = Vec::new();
/// time_ticks(jan1_2024, jan1_2024 + year, 800.0, 60.0, &mut out);
/// assert_eq!(out[0], (jan1_2024, TickWeight::Year));
/// assert_eq!(out[1].1, TickWeight::Month);
/// ```
pub fn time_ticks(
    lo: i64,
    hi: i64,
    length: f64,
    min_spacing: f64,
    out: &mut Vec<(i64, TickWeight)>,
) {
    out.clear();
    if lo >= hi || length <= 0.0 || min_spacing <= 0.0 {
        return;
    }
    let max_ticks = (length / min_spacing).floor().max(1.0);
    let span = (hi - lo) as f64;
    let step = TIME_STEPS
        .into_iter()
        .find(|&s| {
            let ms = if s > 0 {
                s as f64
            } else {
                -s as f64 * 30.44 * DAY_MS as f64
            };
            span / ms <= max_ticks
        })
        .unwrap_or(-1200);
    let mut push = |t: i64| {
        if out.len() < 1000 && (out.len() < out.capacity() || out.try_reserve(1).is_ok()) {
            out.push((t, weight_between(t - 1, t)));
        }
    };
    if step > 0 {
        let mut t = lo.div_euclid(step) * step;
        if t < lo {
            t += step;
        }
        while t <= hi {
            push(t);
            t += step;
        }
    } else {
        let months = -step;
        let (y, m, _) = civil_from_days(lo.div_euclid(DAY_MS));
        let mut k = (y * 12 + i64::from(m) - 1).div_euclid(months) * months;
        loop {
            let t = days_from_civil(k.div_euclid(12), (k.rem_euclid(12) + 1) as u32, 1) * DAY_MS;
            if t > hi {
                break;
            }
            if t >= lo {
                push(t);
            }
            k += months;
        }
    }
}

/// A time tick candidate: the first bar (logical index) of a calendar unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimeTick {
    pub index: i64,
    pub weight: TickWeight,
}

/// Collects tick candidates for logical indices `from..=to` into `out` (cleared
/// first). Bars are sampled every `2^shift` indices, aligned to the fixed origin
/// so that candidates do not flicker while panning; for every sampled interval
/// that crosses a calendar boundary, a binary search finds the exact first bar of
/// the new unit. Work is O(samples · log stride), independent of the data size.
pub(crate) fn collect_ticks(
    index: &TimeIndex,
    tz: &dyn TimeZoneProvider,
    from: i64,
    to: i64,
    shift: u32,
    out: &mut Vec<TimeTick>,
) {
    out.clear();
    let first = index.first_index();
    let last = index.end_index() - 1;
    let local = |i: i64| index.time_at(i).map(|t| t + tz.offset_ms(t));
    let stride = 1_i64 << shift;
    let lo = from.max(first + 1);
    let hi = to.min(last);
    if lo > hi {
        return;
    }
    let anchor = crate::data::ANCHOR;
    let mut j = anchor + (((lo - anchor) + stride - 1) >> shift << shift);
    while j <= hi {
        let prev = (j - stride).max(first);
        if let (Some(tp), Some(tj)) = (local(prev), local(j)) {
            let w = weight_between(tp, tj);
            let up = unit(tp, w);
            // First index in (prev, j] whose unit differs from prev's.
            let (mut a, mut b) = (prev + 1, j);
            while a < b {
                let mid = a + (b - a) / 2;
                if local(mid).is_some_and(|t| unit(t, w) != up) {
                    b = mid;
                } else {
                    a = mid + 1;
                }
            }
            if out.len() < out.capacity() || out.try_reserve(1).is_ok() {
                out.push(TimeTick {
                    index: a,
                    weight: w,
                });
            }
        }
        j += stride;
    }
}

/// The global time scale.
///
/// `x(i) = width - (last_index + right_offset - i + 0.5) * bar_spacing` is the
/// center of bar `i` in physical pixels relative to the plot's left edge.
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::geom::Px;
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let ts = chart.time_scale();
/// ts.set_bar_spacing(8.0);
/// ts.scroll_by(Px(16.0)); // drag right by two bars
/// assert_eq!(ts.bar_spacing(), 8.0);
/// ```
#[derive(Clone, Debug)]
pub struct TimeScale {
    bar_spacing: f64,
    right_offset: f64,
    min_spacing: f64,
    max_spacing: f64,
    width: f64,
    first: i64,
    last: i64,
    bars: usize,
    pending: Option<Pending>,
    pub(crate) changed: bool,
}

/// A view that depends on the plot width. It is applied at once and again
/// whenever the width or the data extent changes, until the layout of the
/// next frame has settled (the price axis width, and so the plot width, is
/// only known then), or until the user pans or zooms.
#[derive(Clone, Copy, Debug)]
enum Pending {
    /// All bars.
    Fit,
    /// Logical indices at the left and the right plot edge.
    Span { from: f64, to: f64 },
}

impl TimeScale {
    pub(crate) fn new(bar_spacing: f64, right_offset: f64, min: f64, max: f64) -> Self {
        Self {
            bar_spacing,
            right_offset,
            min_spacing: min,
            max_spacing: max,
            width: 0.0,
            first: 0,
            last: -1,
            bars: 0,
            pending: None,
            changed: true,
        }
    }

    /// Width of one bar in physical pixels.
    pub fn bar_spacing(&self) -> f64 {
        self.bar_spacing
    }

    /// Empty space right of the last bar, in bars (may be negative).
    pub fn right_offset(&self) -> f64 {
        self.right_offset
    }

    /// Smallest allowed bar spacing: `min(min_bar_spacing, 0.9 · width / bars)`, so
    /// that all data can always be shown at once.
    pub fn effective_min_spacing(&self) -> f64 {
        if self.bars == 0 || self.width <= 0.0 {
            return self.min_spacing;
        }
        self.min_spacing.min(0.9 * self.width / self.bars as f64)
    }

    /// Sets the bar spacing (physical pixels), clamped to the allowed range;
    /// non-finite values are ignored.
    pub fn set_bar_spacing(&mut self, px: f64) {
        if px.is_finite() && px > 0.0 {
            self.bar_spacing = px;
            self.pending = None;
            self.clamp();
            self.changed = true;
        }
    }

    /// Sets the right offset in bars; non-finite values are ignored.
    pub fn set_right_offset(&mut self, bars: f64) {
        if bars.is_finite() {
            self.right_offset = bars;
            self.pending = None;
            self.clamp();
            self.changed = true;
        }
    }

    /// Pans by `dx` (positive = content moves right, showing earlier bars).
    pub fn scroll_by(&mut self, dx: Px) {
        let d = f64::from(dx.0) / self.bar_spacing;
        if d.is_finite() {
            self.right_offset -= d;
            self.pending = None;
            self.clamp();
            self.changed = true;
        }
    }

    /// Zooms by `factor` (> 1 zooms in) keeping the bar under `anchor_x` fixed.
    pub fn zoom(&mut self, anchor_x: Px, factor: f64) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let x = f64::from(anchor_x.0);
        let at = self.x_to_index(anchor_x);
        let spacing = (self.bar_spacing * factor)
            .clamp(self.effective_min_spacing(), self.max_spacing.max(1e-9));
        self.bar_spacing = spacing;
        self.right_offset = at - self.last as f64 - 0.5 + (self.width - x) / spacing;
        self.pending = None;
        self.clamp();
        self.changed = true;
    }

    /// Shows all bars.
    pub fn fit_content(&mut self) {
        self.pending = Some(Pending::Fit);
        self.apply_pending();
        self.changed = true;
    }

    /// Shows logical indices `from..to` across the plot (the left and right
    /// edges), as far as the spacing limits allow (the right edge is kept).
    /// Like [`fit_content`](Self::fit_content) it waits for the plot width,
    /// so it may be called before the first frame or
    /// together with a resize; panning or zooming drops it.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] unless `from` and `to` are finite and
    /// `from < to`; the view is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::{Chart, ChartOptions};
    /// use ferrochart_core::chart::{SeriesData, SeriesOptions};
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::geom::SizeCss;
    /// use ferrochart_core::testing::{random_walk, MonoMeasurer};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
    /// let pane = chart.main_pane();
    /// let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
    /// chart.set_data(candles, SeriesData::Ohlc(random_walk(1, 500, 0, 60_000)?))?;
    /// // Before the first frame: applied once the layout is known.
    /// chart.time_scale().set_visible_range(100.0, 200.0)?;
    /// chart.build_frame(0.0, &MonoMeasurer);
    /// let (from, to) = chart.time_scale().visible_range();
    /// assert!((from - 100.0).abs() < 1e-6 && (to - 200.0).abs() < 1e-6);
    /// assert!(chart.time_scale().set_visible_range(5.0, 5.0).is_err());
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn set_visible_range(&mut self, from: LogicalIndex, to: LogicalIndex) -> Result<()> {
        if !(from.is_finite() && to.is_finite() && from < to) {
            return Err(Error::InvalidArgument(
                "the visible range must be finite with from < to",
            ));
        }
        self.show_span(from, to);
        Ok(())
    }

    /// Shows logical indices `from..to` across the plot (`from < to`), as far
    /// as the spacing limits allow (the right edge is kept).
    pub(crate) fn show_span(&mut self, from: f64, to: f64) {
        self.pending = Some(Pending::Span { from, to });
        self.apply_pending();
        self.changed = true;
    }

    /// Applies the pending view; `false` while it cannot be applied yet (no
    /// width or no bars).
    fn apply_pending(&mut self) -> bool {
        let Some(pending) = self.pending else {
            return false;
        };
        if self.width <= 0.0 || self.bars == 0 {
            return false;
        }
        match pending {
            Pending::Fit => {
                self.bar_spacing = self.width / self.bars as f64;
                self.right_offset = 0.0;
            }
            Pending::Span { from, to } => {
                self.bar_spacing = self.width / (to - from);
                self.right_offset = to - self.last as f64 - 0.5;
            }
        }
        self.clamp();
        true
    }

    /// End of the layout: the plot width is final, so a pending view that
    /// could be applied is done.
    pub(crate) fn settle(&mut self) {
        if self.apply_pending() {
            self.pending = None;
        }
    }

    /// Visible logical range `(from, to)` (left and right plot edges).
    pub fn visible_range(&self) -> (LogicalIndex, LogicalIndex) {
        let to = self.last as f64 + self.right_offset + 0.5;
        (to - self.width / self.bar_spacing, to)
    }

    /// x of the center of logical index `i`, relative to the plot's left edge.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let ts = chart.time_scale();
    /// let x = ts.index_to_x(3.0);
    /// assert!((ts.x_to_index(x) - 3.0).abs() < 1e-3);
    /// ```
    pub fn index_to_x(&self, i: LogicalIndex) -> Px {
        Px(self.index_to_x_f64(i) as f32)
    }

    pub(crate) fn index_to_x_f64(&self, i: f64) -> f64 {
        self.width - (self.last as f64 + self.right_offset - i + 0.5) * self.bar_spacing
    }

    /// Logical index at x (relative to the plot's left edge).
    pub fn x_to_index(&self, x: Px) -> LogicalIndex {
        self.last as f64 + self.right_offset + 0.5
            - (self.width - f64::from(x.0)) / self.bar_spacing
    }

    /// Scrolls so that the first bar sits at the left edge.
    pub(crate) fn scroll_to_first(&mut self) {
        if self.bars == 0 || self.bar_spacing <= 0.0 {
            return;
        }
        let to = self.first as f64 - 0.5 + self.width / self.bar_spacing;
        self.set_right_offset(to - self.last as f64 - 0.5);
    }

    /// Updates the data extent; called by the chart after data changes.
    pub(crate) fn set_extent(&mut self, first: i64, end: i64) {
        self.first = first;
        self.last = end - 1;
        self.bars = usize::try_from(end - first).unwrap_or(0);
        self.apply_pending();
        self.clamp();
    }

    /// Updates the plot width (physical pixels).
    pub(crate) fn set_width(&mut self, width: f64) {
        if (width - self.width).abs() > f64::EPSILON {
            self.width = width.max(0.0);
            self.apply_pending();
            self.clamp();
        }
    }

    /// Rescales the spacing when the device pixel ratio changes.
    pub(crate) fn rescale(&mut self, ratio: f64) {
        self.bar_spacing *= ratio;
        self.min_spacing *= ratio;
        self.max_spacing *= ratio;
        self.clamp();
        self.changed = true;
    }

    /// Keeps the spacing in range and at least one bar on screen.
    fn clamp(&mut self) {
        self.bar_spacing = self
            .bar_spacing
            .clamp(self.effective_min_spacing(), self.max_spacing.max(1e-9));
        if self.bars == 0 || self.width <= 0.0 {
            return;
        }
        let lo = (self.first - self.last) as f64 + 0.5;
        let hi = (self.width / self.bar_spacing - 1.0).max(lo);
        self.right_offset = self.right_offset.clamp(lo, hi);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_from_civil_inverts_civil_from_days() {
        for d in [-800_000_i64, -1, 0, 59, 60, 11_016, 19_723, 2_000_000] {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
    }

    #[test]
    fn time_ticks_are_aligned_and_spaced() {
        let t0 = 1_710_513_000_000; // 2024-03-15T14:30:00Z
        let mut out = Vec::new();
        // Six hours over 600 px, 60 px apart: at most 10 ticks of 1 h.
        time_ticks(t0, t0 + 6 * 3_600_000, 600.0, 60.0, &mut out);
        assert_eq!(out.len(), 6);
        assert!(
            out.iter()
                .all(|&(t, w)| t % 3_600_000 == 0 && w == TickWeight::Hour)
        );
        // Ten years: whole years.
        time_ticks(t0, t0 + 3650 * DAY_MS, 600.0, 60.0, &mut out);
        assert!(out.iter().all(|&(_, w)| w == TickWeight::Year), "{out:?}");
        assert!(out.windows(2).all(|w| w[0].0 < w[1].0));
        // Empty or degenerate ranges.
        time_ticks(t0, t0, 600.0, 60.0, &mut out);
        assert!(out.is_empty());
    }
    use proptest::prelude::*;

    #[test]
    fn pending_view_follows_width_until_settled() {
        let mut ts = TimeScale::new(6.0, 0.0, 0.5, 50.0);
        ts.set_extent(0, 100);
        ts.set_width(600.0);
        // Fit with a stale width; the layout then narrows the plot.
        ts.fit_content();
        ts.set_width(540.0);
        ts.settle();
        let (from, to) = ts.visible_range();
        assert!(
            (from + 0.5).abs() < 1e-9 && (to - 99.5).abs() < 1e-9,
            "{from} {to}"
        );
        // Settled: a later width change keeps the spacing.
        ts.set_width(600.0);
        assert!((ts.bar_spacing() - 5.4).abs() < 1e-9);

        ts.show_span(20.0, 60.0);
        ts.set_width(400.0);
        ts.settle();
        let (from, to) = ts.visible_range();
        assert!(
            (from - 20.0).abs() < 1e-9 && (to - 60.0).abs() < 1e-9,
            "{from} {to}"
        );
        // A user zoom drops a pending view.
        ts.show_span(0.0, 50.0);
        ts.set_bar_spacing(4.0);
        ts.set_width(500.0);
        ts.settle();
        assert!((ts.bar_spacing() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn known_dates() {
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(-719_468), (0, 3, 1));
        assert_eq!(civil_from_days(2_932_896), (9999, 12, 31));
    }

    #[test]
    fn labels() {
        let mut s = crate::text::StackStr::<16>::new();
        let t = 1_704_067_200_000; // 2024-01-01
        for (w, want) in [
            (TickWeight::Year, "2024"),
            (TickWeight::Month, "Jan"),
            (TickWeight::Day, "1"),
            (TickWeight::Minute, "00:00"),
            (TickWeight::Second, "00:00:00"),
        ] {
            s.clear();
            format_tick(t, w, &mut s).unwrap();
            assert_eq!(s.as_str(), want);
        }
    }

    #[test]
    fn ticks_find_exact_boundaries() {
        // Daily bars from 2023-12-20 for 40 days, sampled every 8 bars.
        let start = 1_703_030_400_000;
        let index = TimeIndex::from_times((0..40).map(|d| start + d * DAY_MS).collect()).unwrap();
        let mut out = Vec::new();
        collect_ticks(&index, &Utc, 0, 39, 3, &mut out);
        let year = out.iter().find(|t| t.weight == TickWeight::Year).unwrap();
        assert_eq!(index.time_at(year.index), Some(1_704_067_200_000));
    }

    proptest! {
        /// civil_from_days is consistent with stepping one day at a time.
        #[test]
        fn civil_steps_by_one_day(days in -1_000_000i64..1_000_000) {
            let (y0, m0, d0) = civil_from_days(days);
            let (y1, m1, d1) = civil_from_days(days + 1);
            let next_day = d1 == d0 + 1 && m1 == m0 && y1 == y0;
            let next_month = d1 == 1 && ((m1 == m0 + 1 && y1 == y0) || (m0 == 12 && m1 == 1 && y1 == y0 + 1));
            prop_assert!(next_day || next_month);
            prop_assert!((28..=31).contains(&d0) || next_day);
        }

        /// Zooming keeps the anchor's logical index fixed (when not clamped).
        #[test]
        fn zoom_keeps_anchor(x in 0.0f32..800.0, factor in 0.5f64..2.0) {
            let mut ts = TimeScale::new(6.0, 3.0, 0.5, 50.0);
            ts.set_extent(0, 1000);
            ts.set_width(800.0);
            let before = ts.x_to_index(Px(x));
            ts.zoom(Px(x), factor);
            prop_assert!((ts.x_to_index(Px(x)) - before).abs() < 1e-6);
        }
    }
}
