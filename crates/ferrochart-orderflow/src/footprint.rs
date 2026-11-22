//! Footprint (rules in `docs/book/src/orderflow.md`): one column
//! per bar with the sell and buy volume traded at each price level.

use core::fmt;
use core::ops::Range;

use ferrochart_core::data::Timestamp;
use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::error::{Error, Result};
use ferrochart_core::frame::TextAlign;
use ferrochart_core::geom::{Css, PointPx, RectPx, Rgba};
use ferrochart_core::plugin::{AxisMode, CustomSeries, SeriesCtx};
use ferrochart_core::theme::Theme;

use crate::grid::{Column, Combine, SparseGrid};
use crate::levels::PriceLevels;
use crate::trades::{Side, TickColumns, TickRule};

/// The kind name of footprint series.
pub const FOOTPRINT_KIND: &str = "orderflow.footprint";

/// Below this bar spacing (CSS px) nothing is drawn.
pub const CELLS_MIN_SPACING: f32 = 12.0;
/// From this bar spacing (CSS px) the volumes are written in the cells.
pub const NUMBERS_MIN_SPACING: f32 = 60.0;

/// What a footprint cell shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FootprintMode {
    /// Sell volume on the left half, buy volume on the right half.
    #[default]
    BidAsk,
    /// Buy minus sell volume.
    Delta,
    /// Total volume.
    Volume,
}

/// Colors of a footprint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FootprintColors {
    /// Buy volume, positive delta, buy imbalances.
    pub buy: Rgba,
    /// Sell volume, negative delta, sell imbalances.
    pub sell: Rgba,
    /// Total volume ([`FootprintMode::Volume`]).
    pub volume: Rgba,
    /// Numbers.
    pub text: Rgba,
    /// Outline of each column's point of control.
    pub poc: Rgba,
}

impl FootprintColors {
    /// Colors matching `theme`'s candles and text.
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            buy: theme.up,
            sell: theme.down,
            volume: theme.line,
            text: theme.text,
            poc: theme.text,
        }
    }
}

/// Options of a [`Footprint`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FootprintOptions {
    /// Minimum price increment.
    pub tick_size: f64,
    /// Ticks per row.
    pub row_size: u32,
    /// Bar period, ms (the host's candle period): columns start at
    /// multiples of it (UTC).
    pub timeframe_ms: i64,
    /// What the cells show.
    pub mode: FootprintMode,
    /// Diagonal imbalance ratio (≥ 1); `None`: no imbalance highlights.
    pub imbalance_ratio: Option<f64>,
    /// Outline each column's point of control (the level with the most
    /// volume).
    pub show_poc: bool,
    /// Colors.
    pub colors: FootprintColors,
}

impl FootprintOptions {
    /// Bid × ask cells of one tick per row, imbalance ratio 3, points of
    /// control shown, colors from `theme`.
    pub fn new(tick_size: f64, timeframe_ms: i64, theme: &Theme) -> Self {
        Self {
            tick_size,
            row_size: 1,
            timeframe_ms,
            mode: FootprintMode::BidAsk,
            imbalance_ratio: Some(3.0),
            show_poc: true,
            colors: FootprintColors::from_theme(theme),
        }
    }
}

/// A footprint series: add it with
/// [`Chart::add_custom_series`](ferrochart_core::Chart::add_custom_series)
/// and feed it with [`crate::append_trades`].
///
/// Columns share the slots of the host's candles (`AxisMode::Slots`): a
/// column's time is the open time of its bar. Below 12 CSS px of bar
/// spacing nothing is drawn (the candles stay visible); from 12 px the
/// cells are shaded; from 60 px the volumes are written in them (raise
/// `ChartOptions::max_bar_spacing`, 50 px by default, to zoom in that far).
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::SeriesOptions;
/// use ferrochart_core::theme::Theme;
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_orderflow::{Footprint, FootprintOptions, Side, TickColumns, append_trades};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let fp = Footprint::new(FootprintOptions::new(0.25, 60_000, &Theme::light()))?;
/// let id = chart.add_custom_series(chart.main_pane(), Box::new(fp), SeriesOptions::default())?;
/// let trades = TickColumns {
///     time: vec![0, 1_000, 61_000],
///     price: vec![100.0, 100.25, 100.0],
///     size: vec![2.0, 1.0, 5.0],
///     side: vec![Side::Buy, Side::Sell, Side::Unknown],
/// };
/// append_trades(&mut chart, id, &trades)?;
/// let fp = chart.custom_series::<Footprint>(id).unwrap();
/// assert_eq!(fp.grid().columns(), 2);
/// // 100.0 is level 400: 2 bought; 100.25 is level 401: 1 sold.
/// assert_eq!(fp.grid().column(0).unwrap().at(400), (0.0, 2.0));
/// assert_eq!(fp.grid().column(0).unwrap().at(401), (1.0, 0.0));
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Footprint {
    opts: FootprintOptions,
    grid: SparseGrid,
    /// Last trade price of each column.
    close: Vec<f64>,
    rule: TickRule,
}

impl Footprint {
    /// An empty footprint.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for a bad price grid, a non-positive
    /// timeframe or an imbalance ratio below 1.
    pub fn new(opts: FootprintOptions) -> Result<Self> {
        let levels = PriceLevels::new(opts.tick_size, opts.row_size)?;
        if opts.timeframe_ms <= 0 {
            return Err(Error::InvalidArgument("timeframe_ms must be positive"));
        }
        if opts
            .imbalance_ratio
            .is_some_and(|r| !(r.is_finite() && r >= 1.0))
        {
            return Err(Error::InvalidArgument("imbalance_ratio must be at least 1"));
        }
        Ok(Self {
            opts,
            grid: SparseGrid::new(levels),
            close: Vec::new(),
            rule: TickRule::default(),
        })
    }

    /// The options.
    pub fn options(&self) -> &FootprintOptions {
        &self.opts
    }

    /// Changes what the cells show and their colors (the data stays).
    pub fn set_display(
        &mut self,
        mode: FootprintMode,
        imbalance_ratio: Option<f64>,
        colors: FootprintColors,
    ) {
        self.opts.mode = mode;
        self.opts.imbalance_ratio = imbalance_ratio.filter(|r| r.is_finite() && *r >= 1.0);
        self.opts.colors = colors;
    }

    /// The cells: `a` is sell volume, `b` buy volume.
    pub fn grid(&self) -> &SparseGrid {
        &self.grid
    }

    /// Last trade price of column `i`.
    pub fn close(&self, i: usize) -> Option<f64> {
        self.close.get(i).copied()
    }

    /// Adds one trade; returns whether it opened a column.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for a non-finite price or a negative size;
    /// [`Error::TimeNotIncreasing`] if its bar is before the last column;
    /// [`Error::OutOfMemory`].
    pub fn push_trade(
        &mut self,
        time: Timestamp,
        price: f64,
        size: f64,
        side: Side,
    ) -> Result<bool> {
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
        let col = crate::column_start(time, self.opts.timeframe_ms);
        if self.grid.times().last().is_some_and(|&t| col < t) {
            return Err(Error::TimeNotIncreasing {
                index: self.grid.columns(),
            });
        }
        if size == 0.0 {
            return Ok(false);
        }
        self.close.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
        let side = self.rule.classify(price, side);
        let v = size as f32;
        let (a, b) = if side == Side::Sell {
            (v, 0.0)
        } else {
            (0.0, v)
        };
        let opened = self.grid.add(col, level, a, b, Combine::Sum)?;
        if opened {
            self.close.push(price);
        } else if let Some(c) = self.close.last_mut() {
            *c = price;
        }
        Ok(opened)
    }
}

impl crate::TradeSink for Footprint {
    fn timeframe(&self) -> i64 {
        self.opts.timeframe_ms
    }

    fn last_column(&self) -> Option<Timestamp> {
        self.grid.times().last().copied()
    }

    fn apply(&mut self, trades: &TickColumns, range: Range<usize>) -> Result<()> {
        for i in range {
            let (t, p, s, side) = trades.get(i);
            self.push_trade(t, p, s, side)?;
        }
        Ok(())
    }
}

/// Opacity of a cell holding `v` of the column's `max`: 40–200 of 255,
/// 0 for an empty cell.
pub(crate) fn intensity(v: f32, max: f32) -> u8 {
    if v > 0.0 && max > 0.0 {
        (40.0 + 160.0 * (v / max).min(1.0)).round() as u8
    } else {
        0
    }
}

fn tint(c: Rgba, intensity: u8) -> Rgba {
    c.with_alpha(((f32::from(c.a) * f32::from(intensity)) / 255.0).round() as u8)
}

/// A volume as a footprint shows it: whole numbers without decimals,
/// small fractional ones with two, the rest with one.
struct Vol(f32, bool);

impl fmt::Display for Vol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (v, signed) = (self.0, self.1);
        // Sums of f32 sizes: "whole" within rounding noise.
        match ((v - v.round()).abs() < 1e-4, v.abs() < 10.0, signed) {
            (true, _, true) => write!(f, "{v:+.0}"),
            (true, _, false) => write!(f, "{v:.0}"),
            (false, true, true) => write!(f, "{v:+.2}"),
            (false, true, false) => write!(f, "{v:.2}"),
            (false, false, true) => write!(f, "{v:+.1}"),
            (false, false, false) => write!(f, "{v:.1}"),
        }
    }
}

/// Per-column maxima for shading and the point of control.
struct Stats {
    side: f32,
    delta: f32,
    total: f32,
    poc: Option<i32>,
}

fn stats(col: &Column<'_>) -> Stats {
    let mut s = Stats {
        side: 0.0,
        delta: 0.0,
        total: 0.0,
        poc: None,
    };
    for k in 0..col.len() {
        let (a, b) = (col.a[k], col.b[k]);
        s.side = s.side.max(a).max(b);
        s.delta = s.delta.max((b - a).abs());
        // Ties: the lowest level.
        if a + b > s.total {
            s.total = a + b;
            s.poc = Some(col.levels[k]);
        }
    }
    s
}

impl CustomSeries for Footprint {
    fn kind(&self) -> &'static str {
        FOOTPRINT_KIND
    }

    fn axis_mode(&self) -> AxisMode {
        AxisMode::Slots
    }

    fn times(&self) -> &[Timestamp] {
        self.grid.times()
    }

    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        self.grid.extremes(from, to)
    }

    fn last_value(&self) -> Option<f64> {
        self.close.last().copied()
    }

    fn crosshair_value(&self, point: usize) -> Option<f64> {
        self.close(point)
    }

    #[allow(clippy::too_many_lines)]
    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
        let c = ctx.coords();
        let dpr = c.dpr();
        let spacing = ctx.bar_spacing().0;
        if spacing < CELLS_MIN_SPACING * dpr {
            return;
        }
        let numbers = spacing >= NUMBERS_MIN_SPACING * dpr;
        let levels = self.grid.price_levels();
        let o = &self.opts;
        let w = (spacing * 0.9).round().max(1.0);
        let px1 = dpr.round().max(1.0);
        let strip = (2.0 * dpr).round().max(1.0);
        // Levels inside the plot (either scale direction).
        let plot = c.plot();
        let (pa, pb) = (c.price_at(plot.y), c.price_at(plot.y + plot.h));
        let (Some(lo), Some(hi)) = (levels.level(pa.min(pb)), levels.level(pa.max(pb))) else {
            return;
        };
        let (lo, hi) = (lo.saturating_sub(1), hi.saturating_add(1));
        for i in from..to {
            let (Some(col), Some(xc)) = (self.grid.column(i), ctx.x_of_point(i)) else {
                continue;
            };
            let st = stats(&col);
            let x0 = (xc - w / 2.0).round();
            let xm = xc.round();
            let first = col.levels.partition_point(|&l| l < lo);
            let last = col.levels.partition_point(|&l| l <= hi);
            for k in first..last {
                let level = col.levels[k];
                let (a, b) = (col.a[k], col.b[k]);
                let (p_lo, p_hi) = levels.edges(level);
                let (Some(y1), Some(y2)) = (c.y_of(p_hi), c.y_of(p_lo)) else {
                    continue;
                };
                let (top, bottom) = (y1.min(y2).round(), y1.max(y2).round());
                let h = bottom - top;
                if h < 2.0 {
                    continue;
                }
                let ch = if h >= 6.0 { h - px1 } else { h };
                let cell = |x: f32, cw: f32| RectPx {
                    x,
                    y: top,
                    w: cw,
                    h: ch,
                };
                match o.mode {
                    FootprintMode::BidAsk => {
                        out.rect(
                            cell(x0, xm - x0),
                            tint(o.colors.sell, intensity(a, st.side)),
                        );
                        out.rect(
                            cell(xm, x0 + w - xm),
                            tint(o.colors.buy, intensity(b, st.side)),
                        );
                    }
                    FootprintMode::Delta => {
                        let d = b - a;
                        let color = if d >= 0.0 {
                            o.colors.buy
                        } else {
                            o.colors.sell
                        };
                        out.rect(cell(x0, w), tint(color, intensity(d.abs(), st.delta)));
                    }
                    FootprintMode::Volume => {
                        out.rect(
                            cell(x0, w),
                            tint(o.colors.volume, intensity(a + b, st.total)),
                        );
                    }
                }
                // Diagonal imbalances: buys at this level against sells one
                // level down, sells against buys one level up.
                let (mut buy_imb, mut sell_imb) = (false, false);
                if let Some(r) = o.imbalance_ratio {
                    let below = k
                        .checked_sub(1)
                        .filter(|&j| Some(col.levels[j]) == level.checked_sub(1))
                        .map_or(0.0, |j| col.a[j]);
                    let above = col
                        .levels
                        .get(k + 1)
                        .filter(|&&l| Some(l) == level.checked_add(1))
                        .map_or(0.0, |_| col.b[k + 1]);
                    buy_imb = below > 0.0 && f64::from(b) >= r * f64::from(below);
                    sell_imb = above > 0.0 && f64::from(a) >= r * f64::from(above);
                }
                // Imbalances: a strip on the side's outer edge (numbers keep
                // the text color, readable on any shade).
                if sell_imb {
                    out.rect(cell(x0, strip), o.colors.sell);
                }
                if buy_imb {
                    out.rect(cell(x0 + w - strip, strip), o.colors.buy);
                }
                let font_css = (11.0_f32).min(ch / dpr * 0.75);
                if numbers && font_css >= 7.0 {
                    let label = |align: TextAlign| Label {
                        font: c.font(Css(font_css)),
                        color: o.colors.text,
                        align,
                        bg: None,
                    };
                    let y = top + ch / 2.0;
                    let gap = (3.0 * dpr).round();
                    match o.mode {
                        FootprintMode::BidAsk => {
                            out.text_fmt(
                                PointPx { x: xm - gap, y },
                                format_args!("{}", Vol(a, false)),
                                label(TextAlign::Right),
                            );
                            out.text_fmt(
                                PointPx { x: xm + gap, y },
                                format_args!("{}", Vol(b, false)),
                                label(TextAlign::Left),
                            );
                        }
                        FootprintMode::Delta => out.text_fmt(
                            PointPx { x: xm, y },
                            format_args!("{}", Vol(b - a, true)),
                            label(TextAlign::Center),
                        ),
                        FootprintMode::Volume => out.text_fmt(
                            PointPx { x: xm, y },
                            format_args!("{}", Vol(a + b, false)),
                            label(TextAlign::Center),
                        ),
                    }
                }
                if o.show_poc && st.poc == Some(level) {
                    let edge = |x: f32, y: f32, ew: f32, eh: f32| RectPx { x, y, w: ew, h: eh };
                    out.rect(edge(x0, top, w, px1), o.colors.poc);
                    out.rect(edge(x0, top + ch - px1, w, px1), o.colors.poc);
                    out.rect(edge(x0, top + px1, px1, ch - 2.0 * px1), o.colors.poc);
                    out.rect(
                        edge(x0 + w - px1, top + px1, px1, ch - 2.0 * px1),
                        o.colors.poc,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;

    fn fp() -> Footprint {
        Footprint::new(FootprintOptions::new(1.0, 60_000, &Theme::dark())).unwrap()
    }

    #[test]
    fn trades_land_in_bars_and_levels() {
        let mut f = fp();
        assert!(f.push_trade(59_999, 10.0, 1.0, Side::Buy).unwrap());
        assert!(!f.push_trade(30_000, 10.4, 2.0, Side::Sell).unwrap());
        assert!(!f.push_trade(30_000, 11.0, 0.0, Side::Sell).unwrap());
        assert!(f.push_trade(60_000, 9.0, 4.0, Side::Unknown).unwrap());
        assert!(!f.push_trade(60_001, 9.0, 1.0, Side::Unknown).unwrap());
        assert_eq!(f.grid().times(), &[0, 60_000]);
        assert_eq!(f.grid().column(0).unwrap().at(10), (2.0, 1.0));
        // Zero sizes add nothing.
        assert_eq!(f.grid().column(0).unwrap().len(), 1);
        // 9 < 10.4: sells by the tick rule, the same at the same price.
        assert_eq!(f.grid().column(1).unwrap().at(9), (5.0, 0.0));
        assert_eq!((f.close(0), f.close(1)), (Some(10.4), Some(9.0)));
        assert!(matches!(
            f.push_trade(1, 9.0, 1.0, Side::Buy),
            Err(Error::TimeNotIncreasing { .. })
        ));
        assert!(f.push_trade(60_002, f64::NAN, 1.0, Side::Buy).is_err());
        assert!(f.push_trade(60_002, 9.0, -1.0, Side::Buy).is_err());
        // Bars before 1970 start at the multiple below.
        let mut g = fp();
        g.push_trade(-1, 1.0, 1.0, Side::Buy).unwrap();
        assert_eq!(g.grid().times(), &[-60_000]);
    }

    #[test]
    fn options_are_checked() {
        let theme = Theme::dark();
        let mut o = FootprintOptions::new(1.0, 0, &theme);
        assert!(Footprint::new(o).is_err());
        o.timeframe_ms = 1;
        o.imbalance_ratio = Some(0.5);
        assert!(Footprint::new(o).is_err());
        o.imbalance_ratio = None;
        o.tick_size = 0.0;
        assert!(Footprint::new(o).is_err());
    }

    #[test]
    fn numbers_and_shading() {
        assert_eq!(Vol(12.0, false).to_string(), "12");
        assert_eq!(Vol(-3.0, true).to_string(), "-3");
        assert_eq!(Vol(3.0, true).to_string(), "+3");
        assert_eq!(Vol(0.125, false).to_string(), "0.12");
        assert_eq!(Vol(12.25, true).to_string(), "+12.2");
        assert_eq!(
            (
                intensity(0.0, 5.0),
                intensity(5.0, 5.0),
                intensity(1.0, 0.0)
            ),
            (0, 200, 0)
        );
        assert_eq!(intensity(2.5, 5.0), 120);
    }
}
