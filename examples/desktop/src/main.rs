//! FerroChart desktop demo: 10 million one-minute bars of random walk plus a
//! simulated real-time feed of 100 ticks per second.
//!
//! Drag to pan (release while moving for kinetic scrolling), wheel or pinch to
//! zoom, drag the axes to scale, double-click an axis to reset it, arrow keys
//! and Home/End to navigate. The window title shows frame statistics.
//!
//! ```text
//! cargo run -p example-desktop --release [-- <bars>]
//! ```

use std::process::ExitCode;

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::{BarUpdate, SeriesId};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::interaction::CrosshairMode;
use ferrochart_core::testing::random_walk;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_winit::winit::window::Window;
use ferrochart_winit::{ChartApp, WindowOptions, run};

const START: i64 = 1_104_537_600_000; // 2005-01-01T00:00:00Z
const MINUTE: i64 = 60_000;
const TICK_MS: f64 = 10.0; // 100 ticks per second
const TICKS_PER_BAR: u32 = 100; // a new (simulated) minute every second

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
    series: SeriesId,
    bar: BarUpdate,
    ticks: u32,
    next_tick: f64,
    rng: XorShift,
    // Frame statistics for the title.
    frames: u32,
    cpu_ms: f64,
    worst_ms: f64,
    since: f64,
}

impl ChartApp for Feed {
    fn update(&mut self, chart: &mut Chart, now_ms: f64) -> Option<f64> {
        // Catch up if the loop was delayed, but never more than a second.
        let mut budget = 100;
        while self.next_tick <= now_ms && budget > 0 {
            budget -= 1;
            self.next_tick += TICK_MS;
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
            let step = self.bar.close * 0.0004 * self.rng.next();
            let close = (self.bar.close + step).max(0.01);
            self.bar.close = close;
            self.bar.high = self.bar.high.max(close);
            self.bar.low = self.bar.low.min(close);
            self.bar.volume = Some(self.bar.volume.unwrap_or(0.0) + 1.0 + self.rng.next().abs());
            if let Err(e) = chart.update_last(self.series, self.bar) {
                eprintln!("update_last failed: {e}");
                return None;
            }
        }
        if budget == 0 {
            self.next_tick = now_ms + TICK_MS;
        }
        Some(self.next_tick)
    }

    fn on_frame(&mut self, window: &Window, cpu_ms: f64) {
        self.frames += 1;
        self.cpu_ms += cpu_ms;
        self.worst_ms = self.worst_ms.max(cpu_ms);
        let now = self.next_tick;
        if now - self.since >= 1000.0 {
            window.set_title(&format!(
                "FerroChart — 10M bars, 100 ticks/s — {} fps, CPU {:.2} ms/frame (max {:.2})",
                self.frames,
                self.cpu_ms / f64::from(self.frames.max(1)),
                self.worst_ms
            ));
            self.frames = 0;
            self.cpu_ms = 0.0;
            self.worst_ms = 0.0;
            self.since = now;
        }
    }
}

fn main() -> ExitCode {
    let bars = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(10_000_000_usize);
    eprintln!("generating {bars} bars…");
    let data = match random_walk(2026, bars, START, MINUTE) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("cannot generate data: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (Some(&time), Some(&close)) = (data.time.last(), data.close.last()) else {
        eprintln!("need at least one bar");
        return ExitCode::FAILURE;
    };
    let mut chart = Chart::new(ChartOptions {
        theme: Theme::dark(),
        ..ChartOptions::default()
    });
    let pane = chart.main_pane();
    let series = match chart
        .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
        .and_then(|id| {
            chart.add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(id))?;
            chart.set_data(id, SeriesData::Ohlc(data))?;
            Ok(id)
        }) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("cannot set up the chart: {e}");
            return ExitCode::FAILURE;
        }
    };
    chart.set_crosshair_mode(CrosshairMode::Magnet);
    let feed = Feed {
        series,
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
        frames: 0,
        cpu_ms: 0.0,
        worst_ms: 0.0,
        since: 0.0,
    };
    let options = WindowOptions {
        title: "FerroChart".to_owned(),
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
