//! TPO / market profile (rules in `docs/book/src/orderflow.md`):
//! one slot per trading session; each period of the session marks the price
//! levels it traded at with its letter.

use core::cell::RefCell;

use ferrochart_core::data::{OhlcColumns, Timestamp};
use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::error::{Error, Result};
use ferrochart_core::frame::TextAlign;
use ferrochart_core::geom::{Css, PointPx, RectPx, Rgba};
use ferrochart_core::plugin::{AxisMode, CustomSeries, MinMaxIndex, SeriesCtx};
use ferrochart_core::theme::Theme;

use crate::levels::PriceLevels;

const DAY: i64 = 86_400_000;

/// The kind name of TPO series.
pub const TPO_KIND: &str = "orderflow.tpo";

/// Below this slot width (CSS px) nothing is drawn.
pub const TPO_MIN_SPACING: f32 = 4.0;

/// Smallest letter size (CSS px); with smaller rows the profile is drawn
/// as a histogram.
pub const TPO_MIN_FONT: f32 = 7.0;

/// The letter of period `k` of a session: A–Z, then a–z, repeating.
///
/// # Examples
///
/// ```
/// use ferrochart_orderflow::tpo_letter;
///
/// assert_eq!((tpo_letter(0), tpo_letter(25), tpo_letter(26), tpo_letter(52)), ('A', 'Z', 'a', 'A'));
/// ```
pub fn tpo_letter(k: u16) -> char {
    let k = (k % 52) as u8;
    char::from(if k < 26 { b'A' + k } else { b'a' + k - 26 })
}

/// Colors of a TPO profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TpoColors {
    /// Letters (dimmed outside the value area).
    pub letters: Rgba,
    /// Histogram bars (dimmed outside the value area).
    pub profile: Rgba,
    /// Point of control: background of its letters, or its bar.
    pub poc: Rgba,
    /// The initial balance bar on the profile's left edge.
    pub initial_balance: Rgba,
}

impl TpoColors {
    /// Colors matching `theme`.
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            letters: theme.text,
            profile: theme.line,
            poc: theme.down,
            initial_balance: theme.up,
        }
    }
}

/// Options of a [`Tpo`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TpoOptions {
    /// Minimum price increment.
    pub tick_size: f64,
    /// Ticks per row.
    pub row_size: u32,
    /// Period length, ms (one letter each).
    pub period_ms: i64,
    /// Session length, ms (at most a day). A session starts every UTC day
    /// at `session_offset_ms`; bars outside the sessions are skipped.
    pub session_ms: i64,
    /// Start of the sessions after UTC midnight, e.g. 14 h 30 min for
    /// 9:30–16:00 New York (winter time).
    pub session_offset_ms: i64,
    /// Share of the TPOs in the value area, in (0, 1].
    pub value_area: f64,
    /// Colors.
    pub colors: TpoColors,
}

impl TpoOptions {
    /// One tick per row, 30 min periods, UTC days, a 70% value area,
    /// colors from `theme`.
    pub fn new(tick_size: f64, theme: &Theme) -> Self {
        Self {
            tick_size,
            row_size: 1,
            period_ms: 30 * 60_000,
            session_ms: 24 * 3_600_000,
            session_offset_ms: 0,
            value_area: 0.7,
            colors: TpoColors::from_theme(theme),
        }
    }
}

/// The profile of one session, in levels (see [`PriceLevels`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionProfile {
    /// Lowest and highest level traded.
    pub low: i32,
    /// Highest level traded.
    pub high: i32,
    /// Point of control: the level with the most TPOs.
    pub poc: i32,
    /// TPOs at the point of control (the most at any level).
    pub max_count: u32,
    /// TPOs of the session.
    pub tpos: u32,
    /// Value area, lowest and highest level.
    pub value_area: (i32, i32),
    /// Range of the first two periods; `None` if neither traded.
    pub initial_balance: Option<(i32, i32)>,
}

/// A TPO series: add it with
/// [`Chart::add_custom_series`](ferrochart_core::Chart::add_custom_series)
/// and feed it with [`crate::append_bars`].
///
/// Each session takes one slot (`AxisMode::Slots`, at the session start).
/// A profile needs a wide slot: raise `ChartOptions::max_bar_spacing` (50
/// CSS px by default). When the letters of the drawn sessions do not all
/// fit, the profiles are drawn as histograms of TPO counts.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::SeriesOptions;
/// use ferrochart_core::data::OhlcColumns;
/// use ferrochart_core::theme::Theme;
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_orderflow::{Tpo, TpoOptions, append_bars};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let tpo = Tpo::new(TpoOptions::new(1.0, &Theme::light()))?;
/// let id = chart.add_custom_series(chart.main_pane(), Box::new(tpo), SeriesOptions::default())?;
/// // Three 30 min bars: periods A, B and C of the first UTC day.
/// let bars = OhlcColumns {
///     time: vec![0, 1_800_000, 3_600_000],
///     open: vec![10.0, 12.0, 11.0],
///     high: vec![12.0, 13.0, 12.0],
///     low: vec![10.0, 11.0, 11.0],
///     close: vec![12.0, 11.0, 12.0],
///     volume: None,
/// };
/// append_bars(&mut chart, id, &bars)?;
/// let p = chart.custom_series::<Tpo>(id).unwrap().profile(0).unwrap();
/// // 11 and 12 have 3 TPOs each, equally near the middle: the lower wins.
/// assert_eq!((p.low, p.high, p.poc, p.max_count, p.tpos), (10, 13, 11, 3, 8));
/// assert_eq!(p.initial_balance, Some((10, 13)));
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Debug)]
pub struct Tpo {
    opts: TpoOptions,
    levels: PriceLevels,
    /// Session start times.
    sessions: Vec<Timestamp>,
    /// Offset of each session's first period in `period`/`lo`/`hi`.
    sess_start: Vec<u32>,
    /// Period number within its session, ascending per session.
    period: Vec<u16>,
    lo: Vec<i32>,
    hi: Vec<i32>,
    profiles: Vec<SessionProfile>,
    extremes: MinMaxIndex,
    /// Difference array for the profile of the last session.
    counts: Vec<i32>,
    /// Per-level letter counters while drawing.
    scratch: RefCell<Vec<u32>>,
}

fn reserve<T>(v: &mut Vec<T>) -> Result<()> {
    v.try_reserve(1).map_err(|_| Error::OutOfMemory)
}

impl Tpo {
    /// An empty TPO series.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for a bad price grid, a non-positive
    /// period, a session shorter than a period, longer than a day or than
    /// 65 535 periods, or a value area outside (0, 1].
    pub fn new(opts: TpoOptions) -> Result<Self> {
        let levels = PriceLevels::new(opts.tick_size, opts.row_size)?;
        if opts.period_ms <= 0
            || opts.session_ms < opts.period_ms
            || opts.session_ms > DAY
            || opts.session_ms / opts.period_ms > i64::from(u16::MAX)
        {
            return Err(Error::InvalidArgument(
                "period_ms must be positive and session_ms one period to one day",
            ));
        }
        if !(opts.value_area > 0.0 && opts.value_area <= 1.0) {
            return Err(Error::InvalidArgument("value_area must be in (0, 1]"));
        }
        Ok(Self {
            opts,
            levels,
            sessions: Vec::new(),
            sess_start: Vec::new(),
            period: Vec::new(),
            lo: Vec::new(),
            hi: Vec::new(),
            profiles: Vec::new(),
            extremes: MinMaxIndex::new(),
            counts: Vec::new(),
            scratch: RefCell::new(Vec::new()),
        })
    }

    /// The options.
    pub fn options(&self) -> &TpoOptions {
        &self.opts
    }

    /// Changes the colors.
    pub fn set_colors(&mut self, colors: TpoColors) {
        self.opts.colors = colors;
    }

    /// The price grid.
    pub fn price_levels(&self) -> PriceLevels {
        self.levels
    }

    /// Session start times.
    pub fn sessions(&self) -> &[Timestamp] {
        &self.sessions
    }

    /// The profile of session `i`.
    pub fn profile(&self, i: usize) -> Option<SessionProfile> {
        self.profiles.get(i).copied()
    }

    fn bounds(&self, i: usize) -> (usize, usize) {
        let s = self.sess_start[i] as usize;
        let e = self
            .sess_start
            .get(i + 1)
            .map_or(self.period.len(), |&x| x as usize);
        (s, e)
    }

    /// The traded periods of session `i`: period numbers (ascending) and
    /// their lowest and highest levels.
    pub fn periods(&self, i: usize) -> Option<(&[u16], &[i32], &[i32])> {
        if i >= self.sessions.len() {
            return None;
        }
        let (s, e) = self.bounds(i);
        Some((&self.period[s..e], &self.lo[s..e], &self.hi[s..e]))
    }

    /// Start of the last session starting at or before `time` (it contains
    /// `time` if [`Self::in_session`]).
    pub fn session_start(&self, time: Timestamp) -> Timestamp {
        let o = self.opts.session_offset_ms;
        crate::column_start(time.saturating_sub(o), DAY).saturating_add(o)
    }

    /// Whether `time` falls in a session.
    pub fn in_session(&self, time: Timestamp) -> bool {
        time - self.session_start(time) < self.opts.session_ms
    }

    /// The session of the last bar, if any.
    pub(crate) fn last_session(&self) -> Option<Timestamp> {
        self.sessions.last().copied()
    }

    /// Adds a bar (its period is the one containing `time`; bars must be no
    /// longer than a period); returns whether it opened a session. A bar
    /// again at a time already seen (an update of a live bar) widens its
    /// period's range. A bar outside the sessions is skipped.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for non-finite prices, `high < low` or
    /// prices outside the level range; [`Error::TimeNotIncreasing`] if its
    /// session is before the last one; [`Error::OutOfMemory`].
    pub fn push_bar(&mut self, time: Timestamp, high: f64, low: f64) -> Result<bool> {
        let opened = self.push_raw(time, high, low)?;
        self.finish_last();
        Ok(opened)
    }

    /// [`Self::push_bar`] without updating the last session's profile
    /// (batches call [`Self::finish_last`] once at the end).
    pub(crate) fn push_raw(&mut self, time: Timestamp, high: f64, low: f64) -> Result<bool> {
        let (Some(l), Some(h)) = (self.levels.level(low), self.levels.level(high)) else {
            return Err(Error::InvalidArgument(
                "bar prices must be finite and in range",
            ));
        };
        if high < low {
            return Err(Error::InvalidArgument("bar high must not be below low"));
        }
        let sess = self.session_start(time);
        if !self.in_session(time) {
            return match self.sessions.last() {
                Some(&last) if sess < last => Err(Error::TimeNotIncreasing {
                    index: self.sessions.len(),
                }),
                _ => Ok(false),
            };
        }
        let opened = match self.sessions.last() {
            Some(&last) if sess < last => {
                return Err(Error::TimeNotIncreasing {
                    index: self.sessions.len(),
                });
            }
            Some(&last) if sess == last => false,
            _ => true,
        };
        let k = u16::try_from((time - sess) / self.opts.period_ms).unwrap_or(u16::MAX);
        if opened {
            let start = u32::try_from(self.period.len()).map_err(|_| Error::OutOfMemory)?;
            reserve(&mut self.sessions)?;
            reserve(&mut self.sess_start)?;
            reserve(&mut self.profiles)?;
            reserve(&mut self.period)?;
            reserve(&mut self.lo)?;
            reserve(&mut self.hi)?;
            if !self.sessions.is_empty() {
                self.finish_last();
            }
            let (lo, hi) = (self.levels.edges(l).0, self.levels.edges(h).1);
            self.extremes.push(lo, hi)?;
            self.sessions.push(sess);
            self.sess_start.push(start);
            self.profiles.push(SessionProfile::default());
            self.period.push(k);
            self.lo.push(l);
            self.hi.push(h);
            return Ok(true);
        }
        let (s, e) = self.bounds(self.sessions.len() - 1);
        match self.period[s..e].binary_search(&k) {
            Ok(j) => {
                let j = s + j;
                self.lo[j] = self.lo[j].min(l);
                self.hi[j] = self.hi[j].max(h);
            }
            Err(j) => {
                reserve(&mut self.period)?;
                reserve(&mut self.lo)?;
                reserve(&mut self.hi)?;
                self.period.insert(s + j, k);
                self.lo.insert(s + j, l);
                self.hi.insert(s + j, h);
            }
        }
        Ok(false)
    }

    /// Recomputes the last session's profile and range.
    pub(crate) fn finish_last(&mut self) {
        let Some(i) = self.sessions.len().checked_sub(1) else {
            return;
        };
        let (s, e) = self.bounds(i);
        let (lo, hi) = (&self.lo[s..e], &self.hi[s..e]);
        let (Some(&low), Some(&high)) = (lo.iter().min(), hi.iter().max()) else {
            return;
        };
        let n = (i64::from(high) - i64::from(low) + 1) as usize;
        // TPO counts per level by a difference array: O(periods + levels).
        self.counts.clear();
        if self.counts.try_reserve(n + 1).is_err() {
            return;
        }
        self.counts.resize(n + 1, 0);
        for (&a, &b) in lo.iter().zip(hi) {
            self.counts[(a - low) as usize] += 1;
            self.counts[(b - low) as usize + 1] -= 1;
        }
        let mut run = 0;
        for c in &mut self.counts[..n] {
            run += *c;
            *c = run;
        }
        let counts = &self.counts[..n];
        let tpos: u32 = counts.iter().map(|&c| c as u32).sum();
        // POC: most TPOs; ties: nearest the middle of the range, then lower.
        let mid2 = i64::from(low) + i64::from(high);
        let mut poc = 0;
        for (j, &c) in counts.iter().enumerate() {
            let dist = |k: usize| (2 * (i64::from(low) + k as i64) - mid2).abs();
            if c > counts[poc] || (c == counts[poc] && dist(j) < dist(poc)) {
                poc = j;
            }
        }
        // Value area (CBOT): from the POC, add the two rows above or the two
        // below, whichever have more TPOs (above on a tie), until it holds
        // `value_area` of the TPOs.
        let target = (self.opts.value_area * f64::from(tpos)).ceil() as u32;
        let (mut va_lo, mut va_hi) = (poc, poc);
        let mut acc = counts[poc] as u32;
        while acc < target {
            let up: Vec2 = (1..=2)
                .filter_map(|d| counts.get(va_hi + d).copied())
                .fold((0, 0), add);
            let down: Vec2 = (1..=2)
                .filter_map(|d| va_lo.checked_sub(d).map(|k| counts[k]))
                .fold((0, 0), add);
            match (up.1, down.1) {
                (0, 0) => break,
                (u, d) if u > 0 && (d == 0 || up.0 >= down.0) => {
                    va_hi += u;
                    acc += up.0 as u32;
                }
                (_, d) => {
                    va_lo -= d;
                    acc += down.0 as u32;
                }
            }
        }
        let periods = &self.period[s..e];
        let ib = periods
            .iter()
            .zip(lo.iter().zip(hi))
            .filter(|(k, _)| **k < 2)
            .fold(None, |acc: Option<(i32, i32)>, (_, (&a, &b))| {
                Some(acc.map_or((a, b), |(x, y)| (x.min(a), y.max(b))))
            });
        let at = |k: usize| low + k as i32;
        self.profiles[i] = SessionProfile {
            low,
            high,
            poc: at(poc),
            max_count: counts[poc] as u32,
            tpos,
            value_area: (at(va_lo), at(va_hi)),
            initial_balance: ib,
        };
        self.extremes
            .set_last(self.levels.edges(low).0, self.levels.edges(high).1);
    }
}

/// (TPOs, rows).
type Vec2 = (i32, usize);

fn add(a: Vec2, c: i32) -> Vec2 {
    (a.0 + c, a.1 + 1)
}

/// Checks bar columns for [`crate::append_bars`].
pub(crate) fn check_bars(bars: &OhlcColumns, levels: PriceLevels) -> Result<()> {
    let n = bars.time.len();
    if bars.high.len() != n || bars.low.len() != n {
        return Err(Error::InvalidArgument(
            "bar time, high and low must have the same length",
        ));
    }
    if let Some(i) = bars.time.windows(2).position(|w| w[1] < w[0]) {
        return Err(Error::TimeNotIncreasing { index: i + 1 });
    }
    let bad = |p: f64| levels.level(p).is_none();
    if bars
        .high
        .iter()
        .zip(&bars.low)
        .any(|(&h, &l)| bad(h) || bad(l) || h < l)
    {
        return Err(Error::InvalidArgument(
            "bar prices must be finite, in range and high >= low",
        ));
    }
    Ok(())
}

impl CustomSeries for Tpo {
    fn kind(&self) -> &'static str {
        TPO_KIND
    }

    fn axis_mode(&self) -> AxisMode {
        AxisMode::Slots
    }

    fn times(&self) -> &[Timestamp] {
        &self.sessions
    }

    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        self.extremes.query(from, to)
    }

    fn crosshair_value(&self, point: usize) -> Option<f64> {
        self.profiles.get(point).map(|p| self.levels.price(p.poc))
    }

    #[allow(clippy::too_many_lines)]
    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
        let c = ctx.coords();
        let dpr = c.dpr();
        let spacing = ctx.bar_spacing().0;
        if spacing < TPO_MIN_SPACING * dpr {
            return;
        }
        let Ok(mut scratch) = self.scratch.try_borrow_mut() else {
            return;
        };
        let o = &self.opts;
        let w = (spacing * 0.9).round().max(1.0);
        let ibw = (2.0 * dpr).round().max(1.0);
        let gap = (2.0 * dpr).round();
        let avail = w - ibw - gap;
        if avail < 1.0 {
            return;
        }
        let plot = c.plot();
        let (pa, pb) = (c.price_at(plot.y), c.price_at(plot.y + plot.h));
        let (Some(vis_lo), Some(vis_hi)) =
            (self.levels.level(pa.min(pb)), self.levels.level(pa.max(pb)))
        else {
            return;
        };
        let (vis_lo, vis_hi) = (vis_lo.saturating_sub(1), vis_hi.saturating_add(1));
        // Rows of a level: its rounded edges.
        let rows = |l: i32| {
            let (a, b) = self.levels.edges(l);
            let (y1, y2) = (c.y_of(b)?, c.y_of(a)?);
            let (top, bottom) = (y1.min(y2).round(), y1.max(y2).round());
            Some((top, bottom - top))
        };
        let dim = |color: Rgba, k: f32| color.with_alpha((f32::from(color.a) * k).round() as u8);
        // Letters or histograms for the whole frame: letters when the most
        // TPOs at any level of the drawn sessions fit in a slot at a
        // readable size (rows are as high as the first POC's).
        let drawn = || self.profiles.get(from..to).unwrap_or(&[]).iter();
        let max_count = drawn().map(|p| p.max_count).max().unwrap_or(0);
        let row_h = drawn()
            .find(|p| p.max_count > 0)
            .and_then(|p| rows(p.poc))
            .map_or(0.0, |(_, h)| h);
        let font_css = 11.0_f32.min(row_h / dpr * 0.85);
        let font = c.font(Css(font_css));
        let cell = c.text_width("M", font).round().max(1.0);
        let letters = font_css >= TPO_MIN_FONT && max_count as f32 * cell <= avail;
        for i in from..to {
            let (Some(p), Some(xc)) = (self.profiles.get(i).copied(), ctx.x_of_point(i)) else {
                continue;
            };
            let (lo, hi) = (p.low.max(vis_lo), p.high.min(vis_hi));
            if lo > hi || p.max_count == 0 {
                continue;
            }
            let x0 = (xc - w / 2.0).round();
            let cx = x0 + ibw + gap;
            if let Some((a, b)) = p.initial_balance
                && let (Some((top, _)), Some((t2, h2))) = (rows(b.min(vis_hi)), rows(a.max(vis_lo)))
                && a.max(vis_lo) <= b.min(vis_hi)
            {
                let rect = RectPx {
                    x: x0,
                    y: top,
                    w: ibw,
                    h: t2 + h2 - top,
                };
                out.rect(rect, o.colors.initial_balance);
            }
            // Per-level TPO counters (visible levels only: a letter's place
            // depends on the earlier periods at its level).
            scratch.clear();
            scratch.resize((hi - lo + 1) as usize, 0);
            let Some((ks, plo, phi)) = self.periods(i) else {
                continue;
            };
            if letters {
                if p.poc >= lo
                    && p.poc <= hi
                    && let Some((top, h)) = rows(p.poc)
                {
                    let rect = RectPx {
                        x: cx,
                        y: top,
                        w: p.max_count as f32 * cell,
                        h,
                    };
                    out.rect(rect, dim(o.colors.poc, 0.35));
                }
                for ((&k, &a), &b) in ks.iter().zip(plo).zip(phi) {
                    for l in a.max(lo)..=b.min(hi) {
                        let n = &mut scratch[(l - lo) as usize];
                        let x = cx + *n as f32 * cell;
                        *n += 1;
                        let Some((top, h)) = rows(l) else {
                            continue;
                        };
                        if h < 1.0 {
                            continue;
                        }
                        let in_va = l >= p.value_area.0 && l <= p.value_area.1;
                        let color = if in_va {
                            o.colors.letters
                        } else {
                            dim(o.colors.letters, 0.45)
                        };
                        out.text_fmt(
                            PointPx {
                                x,
                                y: top + h / 2.0,
                            },
                            format_args!("{}", tpo_letter(k)),
                            Label {
                                font,
                                color,
                                align: TextAlign::Left,
                                bg: None,
                            },
                        );
                    }
                }
            } else {
                for (&a, &b) in plo.iter().zip(phi) {
                    for l in a.max(lo)..=b.min(hi) {
                        scratch[(l - lo) as usize] += 1;
                    }
                }
                let px1 = dpr.round().max(1.0);
                for l in lo..=hi {
                    let n = scratch[(l - lo) as usize];
                    let Some((top, h)) = rows(l) else {
                        continue;
                    };
                    if n == 0 || h < 1.0 {
                        continue;
                    }
                    let bh = if h >= 4.0 { h - px1 } else { h };
                    let bw = (n as f32 / p.max_count as f32 * avail).round().max(1.0);
                    let color = if l == p.poc {
                        o.colors.poc
                    } else if l >= p.value_area.0 && l <= p.value_area.1 {
                        dim(o.colors.profile, 0.7)
                    } else {
                        dim(o.colors.profile, 0.3)
                    };
                    out.rect(
                        RectPx {
                            x: cx,
                            y: top,
                            w: bw,
                            h: bh,
                        },
                        color,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn tpo() -> Tpo {
        Tpo::new(TpoOptions::new(1.0, &Theme::dark())).unwrap()
    }

    const P: i64 = 1_800_000;

    #[test]
    fn profile_value_area_and_initial_balance() {
        let mut t = tpo();
        // Periods: A 10–14, B 12–13, C 11–12, D 12–12, E 20–20.
        for (k, lo, hi) in [
            (0, 10.0, 14.0),
            (1, 12.0, 13.0),
            (2, 11.0, 12.0),
            (3, 12.0, 12.0),
            (4, 20.0, 20.0),
        ] {
            t.push_bar(k * P, hi, lo).unwrap();
        }
        let p = t.profile(0).unwrap();
        // Counts: 10:1 11:2 12:4 13:2 14:1 20:1; total 11, target 8.
        assert_eq!(
            (p.low, p.high, p.poc, p.max_count, p.tpos),
            (10, 20, 12, 4, 11)
        );
        // From 12 (4): above 13+14 = 3, below 11+10 = 3: above → 7; then
        // above 15+16 = 0 < below 3 → 10–14.
        assert_eq!(p.value_area, (10, 14));
        assert_eq!(p.initial_balance, Some((10, 14)));
        // A live update widens its period; a new day opens a session.
        assert!(!t.push_bar(4 * P + 60_000, 21.0, 20.0).unwrap());
        assert_eq!(t.periods(0).unwrap().2[4], 21);
        assert!(t.push_bar(48 * P, 5.0, 5.0).unwrap());
        assert_eq!(t.sessions(), &[0, 86_400_000]);
        assert_eq!(t.profile(1).unwrap().initial_balance, Some((5, 5)));
        assert!(matches!(
            t.push_bar(0, 1.0, 1.0),
            Err(Error::TimeNotIncreasing { .. })
        ));
        assert!(t.push_bar(48 * P, 1.0, 2.0).is_err());
        assert!(t.push_bar(48 * P, f64::NAN, 2.0).is_err());
    }

    #[test]
    fn poc_ties_go_to_the_middle() {
        let mut t = tpo();
        // Counts 1: 10, 11, 12 (and 13 by B): 10–13 → middle 11.5; 11 and 12
        // are equally near; the lower wins.
        t.push_bar(0, 12.0, 10.0).unwrap();
        t.push_bar(P, 13.0, 13.0).unwrap();
        assert_eq!(t.profile(0).unwrap().poc, 11);
        // No initial balance when the first two periods did not trade.
        let mut u = tpo();
        u.push_bar(3 * P, 2.0, 1.0).unwrap();
        assert_eq!(u.profile(0).unwrap().initial_balance, None);
    }

    #[test]
    fn options_and_sessions() {
        let theme = Theme::dark();
        let mut o = TpoOptions::new(1.0, &theme);
        o.session_ms = o.period_ms - 1;
        assert!(Tpo::new(o).is_err());
        o.session_ms = DAY + 1;
        assert!(Tpo::new(o).is_err());
        o.session_ms = 6 * 3_600_000 + 1_800_000;
        o.value_area = 0.0;
        assert!(Tpo::new(o).is_err());
        o.value_area = 1.0;
        o.session_offset_ms = 14 * 3_600_000 + 1_800_000;
        let mut t = Tpo::new(o).unwrap();
        let open = o.session_offset_ms;
        assert_eq!(t.session_start(open + 5), open);
        assert_eq!(t.session_start(0), open - DAY);
        assert!(t.in_session(open) && !t.in_session(open + o.session_ms) && !t.in_session(0));
        // Bars outside the session are skipped; periods count from the open.
        assert!(!t.push_bar(open - 60_000, 2.0, 1.0).unwrap());
        assert!(t.sessions().is_empty());
        assert!(t.push_bar(open + P + 5, 2.0, 1.0).unwrap());
        assert!(!t.push_bar(open + o.session_ms, 9.0, 1.0).unwrap());
        assert_eq!(t.periods(0).unwrap(), (&[1_u16][..], &[1][..], &[2][..]));
        assert_eq!(tpo_letter(51), 'z');
    }
}
