//! FerroChart plugin examples:
//!
//! - [`Watermark`]: a chart-wide layer plugin behind the series;
//! - [`TradeMarks`]: a pane layer plugin that places buy/sell triangles with
//!   the pane's time/price conversions;
//! - [`HourRanges`]: a custom series (`AxisMode::Overlay`) that owns its data
//!   and draws the high–low box of every hour, updated by a simulated feed.
//!
//! ```text
//! cargo run -p example-plugins --release [-- <bars>]
//! ```

use std::process::ExitCode;

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::{BarUpdate, DataChange, SeriesId, Timestamp};
use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::frame::{LineDash, SeriesKind, StrokeStyle, TextAlign};
use ferrochart_core::geom::{Css, PointPx, RectPx, Rgba};
use ferrochart_core::plugin::{
    AxisMode, CustomSeries, LayerCtx, LayerPlugin, LayerPosition, MinMaxIndex, SeriesCtx,
};
use ferrochart_core::testing::random_walk;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_winit::{ChartApp, WindowOptions, run};

const START: i64 = 1_104_537_600_000; // 2005-01-01T00:00:00Z
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const TICK_MS: f64 = 10.0;
const TICKS_PER_BAR: u32 = 20; // a new (simulated) minute every 0.2 s

/// Symbol text in the middle of the chart, behind the series.
struct Watermark(&'static str);

impl LayerPlugin for Watermark {
    fn position(&self) -> LayerPosition {
        LayerPosition::Background
    }

    fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
        let r = ctx.rect();
        let label = Label {
            font: ctx.font(Css(64.0)),
            color: Rgba::hex(0x80_8080).with_alpha(0x30),
            align: TextAlign::Center,
            bg: None,
        };
        let center = PointPx {
            x: r.x + r.w / 2.0,
            y: r.y + r.h / 2.0,
        };
        out.text(center, self.0, label);
    }
}

/// A trade: time, price and side.
#[derive(Clone, Copy)]
struct Trade {
    time: Timestamp,
    price: f64,
    buy: bool,
}

/// Buy (up, green) and sell (down, red) triangles at the trades' prices.
struct TradeMarks {
    /// Sorted by time.
    trades: Vec<Trade>,
}

impl LayerPlugin for TradeMarks {
    fn position(&self) -> LayerPosition {
        LayerPosition::AboveSeries
    }

    fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
        let Some(c) = ctx.coords() else {
            return;
        };
        let plot = c.plot();
        // Only the visible trades: build runs every frame.
        let (Some(t0), Some(t1)) = (c.time_at(plot.x), c.time_at(plot.right())) else {
            return;
        };
        let a = self.trades.partition_point(|t| t.time < t0);
        let b = self.trades.partition_point(|t| t.time <= t1);
        let s = c.px(Css(6.0)).0;
        for t in &self.trades[a..b.max(a)] {
            let (Some(x), Some(y)) = (c.x_of(t.time), c.y_of(t.price)) else {
                continue;
            };
            let (dir, color) = if t.buy {
                (1.0, Rgba::hex(0x26_A69A))
            } else {
                (-1.0, Rgba::hex(0xEF_5350))
            };
            // The tip touches the price; the base is away from the bar.
            let base = y + dir * 2.0 * s;
            out.polygon(
                [
                    PointPx { x, y },
                    PointPx { x: x - s, y: base },
                    PointPx { x: x + s, y: base },
                ],
                color,
            );
        }
    }
}

/// The high–low range of every hour, as a box spanning the hour.
///
/// `Overlay`: the points are placed by their times and add no slots, so the
/// series could hold any times, not only those of the candles.
#[derive(Default)]
struct HourRanges {
    time: Vec<Timestamp>,
    range: Vec<(f64, f64)>,
    index: MinMaxIndex,
}

impl HourRanges {
    /// Adds bar `(time, low, high)`: extends the last hour or starts one.
    fn add(&mut self, time: Timestamp, low: f64, high: f64) -> DataChange {
        let hour = time.div_euclid(HOUR) * HOUR;
        if let (Some(&last), Some(r)) = (self.time.last(), self.range.last_mut())
            && last == hour
        {
            *r = (r.0.min(low), r.1.max(high));
            self.index.set_last(r.0, r.1);
            return DataChange::UpdateLast;
        }
        if self.index.push(low, high).is_err() {
            return DataChange::Append { count: 0 };
        }
        self.time.push(hour);
        self.range.push((low, high));
        DataChange::Append { count: 1 }
    }
}

impl CustomSeries for HourRanges {
    fn kind(&self) -> &'static str {
        "example.hour_ranges"
    }

    fn axis_mode(&self) -> AxisMode {
        AxisMode::Overlay
    }

    fn times(&self) -> &[Timestamp] {
        &self.time
    }

    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        self.index.query(from, to)
    }

    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
        let c = ctx.coords();
        let style = ctx.style();
        let fill = style.up.with_alpha(0x18);
        let stroke = StrokeStyle {
            width: c.px(Css(1.0)),
            color: style.up.with_alpha(0x90),
            dash: LineDash::Dashed,
        };
        // Points are slot centers; the box starts half a slot before the
        // hour's first bar and spans 60 one-minute slots.
        let half = ctx.bar_spacing().0 / 2.0;
        let width = ctx.bar_spacing().0 * (HOUR / MINUTE) as f32;
        for i in from..to {
            let (lo, hi) = self.range[i];
            let (Some(x), Some(y0), Some(y1)) = (ctx.x_of_point(i), c.y_of(hi), c.y_of(lo)) else {
                continue;
            };
            let rect = RectPx {
                x: x - half,
                y: y0,
                w: width,
                h: (y1 - y0).max(1.0),
            };
            out.rect(rect, fill);
            let (l, r) = (rect.x, rect.right());
            out.line(PointPx { x: l, y: y0 }, PointPx { x: r, y: y0 }, stroke);
            out.line(PointPx { x: l, y: y1 }, PointPx { x: r, y: y1 }, stroke);
        }
    }

    fn last_value(&self) -> Option<f64> {
        self.range.last().map(|r| r.1)
    }

    fn crosshair_value(&self, point: usize) -> Option<f64> {
        self.range.get(point).map(|r| r.1)
    }
}

/// Small deterministic generator for the simulated feed.
struct XorShift(u64);

impl XorShift {
    /// Uniform in [-1, 1).
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1_u64 << 52) as f64 - 1.0
    }
}

struct Feed {
    candles: SeriesId,
    ranges: SeriesId,
    bar: BarUpdate,
    ticks: u32,
    next_tick: f64,
    rng: XorShift,
}

impl Feed {
    fn tick(&mut self, chart: &mut Chart) -> ferrochart_core::Result<()> {
        self.ticks += 1;
        if self.ticks >= TICKS_PER_BAR {
            self.ticks = 0;
            let c = self.bar.close;
            self.bar = BarUpdate {
                time: self.bar.time + MINUTE,
                open: c,
                high: c,
                low: c,
                close: c,
                volume: Some(0.0),
            };
        }
        let close = (self.bar.close * (1.0 + 0.0006 * self.rng.next())).max(0.01);
        self.bar.close = close;
        self.bar.high = self.bar.high.max(close);
        self.bar.low = self.bar.low.min(close);
        // The candle first, so that a new hour's time is already on the axis.
        chart.update_last(self.candles, self.bar)?;
        let (t, lo, hi) = (self.bar.time, self.bar.low, self.bar.high);
        chart.update_custom(self.ranges, |r: &mut HourRanges| r.add(t, lo, hi))
    }
}

impl ChartApp for Feed {
    fn update(&mut self, chart: &mut Chart, now_ms: f64) -> Option<f64> {
        let mut budget = 100;
        while self.next_tick <= now_ms && budget > 0 {
            budget -= 1;
            self.next_tick += TICK_MS;
            if let Err(e) = self.tick(chart) {
                eprintln!("feed failed: {e}");
                return None;
            }
        }
        if budget == 0 {
            self.next_tick = now_ms + TICK_MS;
        }
        Some(self.next_tick)
    }
}

/// Every 97th bar a trade at its close, alternating sides.
fn trades(time: &[Timestamp], close: &[f64]) -> Vec<Trade> {
    time.iter()
        .zip(close)
        .enumerate()
        .filter(|(i, _)| i % 97 == 0)
        .map(|(i, (&time, &price))| Trade {
            time,
            price,
            buy: i % 2 == 0,
        })
        .collect()
}

fn setup(chart: &mut Chart, bars: usize) -> ferrochart_core::Result<Feed> {
    let data = random_walk(7, bars, START, MINUTE)?;
    let (Some(&time), Some(&close)) = (data.time.last(), data.close.last()) else {
        return Err(ferrochart_core::Error::InvalidArgument(
            "need at least one bar",
        ));
    };
    let mut ranges = HourRanges::default();
    for ((&t, &lo), &hi) in data.time.iter().zip(&data.low).zip(&data.high) {
        ranges.add(t, lo, hi);
    }
    let marks = TradeMarks {
        trades: trades(&data.time, &data.close),
    };

    let pane = chart.main_pane();
    let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
    chart.set_data(candles, SeriesData::Ohlc(data))?;
    let opts = SeriesOptions {
        color: Some(Rgba::hex(0x42_A5F5)),
        last_value_visible: false,
        price_line_visible: false,
        ..SeriesOptions::default()
    };
    let ranges = chart.add_custom_series(pane, Box::new(ranges), opts)?;
    chart.add_layer(None, Box::new(Watermark("PLUGINS")))?;
    chart.add_layer(Some(pane), Box::new(marks))?;

    Ok(Feed {
        candles,
        ranges,
        bar: BarUpdate {
            time: time + MINUTE,
            open: close,
            high: close,
            low: close,
            close,
            volume: Some(0.0),
        },
        ticks: 0,
        next_tick: 0.0,
        rng: XorShift(0x9E37_79B9_7F4A_7C15),
    })
}

fn main() -> ExitCode {
    let bars = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(100_000_usize);
    let mut chart = Chart::new(ChartOptions {
        theme: Theme::dark(),
        ..ChartOptions::default()
    });
    let feed = match setup(&mut chart, bars) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot set up the chart: {e}");
            return ExitCode::FAILURE;
        }
    };
    let options = WindowOptions {
        title: "FerroChart plugins".to_owned(),
        ..WindowOptions::default()
    };
    match run(options, chart, feed) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use ferrochart_core::testing::MonoMeasurer;

    use super::*;

    #[test]
    fn hour_ranges_group_bars() {
        let mut r = HourRanges::default();
        assert_eq!(r.add(HOUR, 1.0, 2.0), DataChange::Append { count: 1 });
        assert_eq!(r.add(HOUR + MINUTE, 0.5, 1.5), DataChange::UpdateLast);
        assert_eq!(r.add(2 * HOUR, 3.0, 4.0), DataChange::Append { count: 1 });
        assert_eq!(r.time, [HOUR, 2 * HOUR]);
        assert_eq!(r.range, [(0.5, 2.0), (3.0, 4.0)]);
        assert_eq!(r.index.query(0, 2), Some((0.5, 4.0)));
    }

    #[test]
    fn the_feed_keeps_the_ranges_in_sync() {
        let mut chart = Chart::new(ChartOptions::default());
        let mut feed = setup(&mut chart, 500).unwrap();
        for _ in 0..TICKS_PER_BAR * 130 {
            feed.tick(&mut chart).unwrap();
        }
        chart.build_frame(0.0, &MonoMeasurer);
        let r = chart.custom_series::<HourRanges>(feed.ranges).unwrap();
        // 500 + 130 one-minute bars from midnight: 11 hours started.
        assert_eq!(r.time.len(), 11);
        assert_eq!(r.time.len(), r.index.len());
    }
}
