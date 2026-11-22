//! Zero-allocation tests of the order flow series:
//! `build_frame` while panning, zooming and
//! moving the crosshair over a footprint, a heatmap (portable path, and the
//! core side of the native path) and a TPO profile, and real-time trades,
//! snapshots or bars into the last column or session. Same per-thread counting allocator as
//! `crates/ferrochart-core/tests/zero_alloc.rs`; it lives in an integration
//! test because the library forbids `unsafe`. All zero-allocation tests of
//! this crate go in this file.
//!
#![allow(clippy::unwrap_used)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountingAllocator;

thread_local! {
    // `const` initialization: accessing these never allocates, so the allocator
    // cannot recurse into itself.
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

fn note_allocation() {
    // `try_with`: the thread-local may already be destroyed while the thread exits.
    let enabled = ENABLED.try_with(Cell::get).unwrap_or(false);
    if enabled {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
    }
}

// SAFETY: every method forwards to `System` with the caller's arguments unchanged;
// counting only touches const-initialized thread-locals and never allocates.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        // SAFETY: forwarded unchanged; the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        // SAFETY: as above.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note_allocation();
        // SAFETY: as above; `ptr` was allocated by this allocator, i.e. by `System`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: as above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// Runs `f` and returns how many allocations (including reallocations) the
/// current thread made inside it.
fn count_allocations<R>(f: impl FnOnce() -> R) -> (R, usize) {
    COUNT.with(|c| c.set(0));
    ENABLED.with(|e| e.set(true));
    let result = f();
    ENABLED.with(|e| e.set(false));
    (result, COUNT.with(Cell::get))
}

use ferrochart_core::data::OhlcColumns;
use ferrochart_core::data::SeriesId;
use ferrochart_core::geom::{Css, PointCss, Px, SizeCss};
use ferrochart_core::interaction::{InputEvent, Modifiers};
use ferrochart_core::testing::MonoMeasurer;
use ferrochart_core::theme::Theme;
use ferrochart_core::theme::Theme as T;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_orderflow::testing::{footprint_scene, heatmap_scene, tpo_scene};
use ferrochart_orderflow::{
    DepthSnapshot, FootprintMode, HEATMAP_KIND, HeatmapSource, Side, TickColumns, append_bars,
    append_depth, append_trades,
};

#[test]
fn counter_detects_allocations() {
    let (v, n) = count_allocations(|| vec![1u8; 16]);
    assert_eq!(v.len(), 16);
    assert!(n >= 1);
}

/// 2024-01-01T00:00:00Z.
const START: i64 = 1_704_067_200_000;

fn footprint(mode: FootprintMode, spacing: f64, row_size: u32) -> (Chart, SeriesId) {
    let theme = Theme::dark();
    let mut chart = Chart::new(ChartOptions {
        theme,
        max_bar_spacing: Css(120.0),
        ..ChartOptions::default()
    });
    chart.resize(
        SizeCss {
            w: 1200.0,
            h: 800.0,
        },
        2.0,
    );
    let id = footprint_scene(&mut chart, &theme, START, mode, spacing, row_size).unwrap();
    (chart, id)
}

fn step(chart: &mut Chart, k: usize) {
    let dx = if k % 20 < 10 { 23.0 } else { -23.0 };
    chart.time_scale().scroll_by(Px(dx));
    if k.is_multiple_of(7) {
        chart.time_scale().zoom(
            Px(600.0),
            if k.is_multiple_of(14) {
                1.05
            } else {
                1.0 / 1.05
            },
        );
    }
    chart.handle_input(InputEvent::PointerMove {
        id: 0,
        pos: PointCss {
            x: 100.0 + (k % 50) as f32 * 20.0,
            y: 100.0 + (k % 30) as f32 * 20.0,
        },
        mods: Modifiers::default(),
        time_ms: k as f64,
    });
    chart.build_frame(k as f64, &MonoMeasurer);
}

/// Pan, zoom and crosshair frames over a footprint at the numbers spacing
/// (every mode) and at the cells spacing do not allocate.
#[test]
fn footprint_frames_do_not_allocate() {
    for (mode, spacing, rows) in [
        (FootprintMode::BidAsk, 90.0, 4),
        (FootprintMode::Delta, 70.0, 4),
        (FootprintMode::Volume, 70.0, 2),
        (FootprintMode::BidAsk, 20.0, 1),
    ] {
        let (mut chart, _) = footprint(mode, spacing, rows);
        for k in 0..200 {
            step(&mut chart, k);
        }
        let ((), n) = count_allocations(|| {
            for k in 0..200 {
                step(&mut chart, k);
            }
        });
        assert_eq!(n, 0, "{mode:?} at {spacing} px");
    }
}

/// Real-time trades at prices the last column already has, each followed by
/// a frame, do not allocate (the order flow counterpart of `update_last`).
#[test]
fn trades_into_the_last_column_do_not_allocate() {
    let (mut chart, id) = footprint(FootprintMode::BidAsk, 90.0, 4);
    // The scene's last trade is at START + 11 999 × 200 ms; its minute runs
    // to START + 2 400 000 ms. One trade per batch, reused.
    let t0 = START + 11_999 * 200;
    let mut batch = TickColumns {
        time: vec![t0],
        price: vec![100.0],
        size: vec![1.0],
        side: vec![Side::Buy],
    };
    let mut tick = |chart: &mut Chart, k: usize| {
        batch.time[0] = t0 + k as i64;
        batch.price[0] = 100.0 + (k % 3) as f64 * 0.25;
        batch.side[0] = if k.is_multiple_of(2) {
            Side::Buy
        } else {
            Side::Sell
        };
        append_trades(chart, id, &batch).unwrap();
        chart.build_frame(k as f64, &MonoMeasurer);
    };
    for k in 0..20 {
        tick(&mut chart, k);
    }
    let ((), n) = count_allocations(|| {
        for k in 20..120 {
            tick(&mut chart, k);
        }
    });
    assert_eq!(n, 0);
}

fn heatmap(source: HeatmapSource, spacing: f64) -> (Chart, SeriesId) {
    let mut chart = Chart::new(ChartOptions {
        max_bar_spacing: Css(120.0),
        ..ChartOptions::default()
    });
    chart.resize(
        SizeCss {
            w: 1200.0,
            h: 800.0,
        },
        1.0,
    );
    let (id, _) = heatmap_scene(&mut chart, START, source, spacing).unwrap();
    (chart, id)
}

/// Pan, zoom and crosshair frames over depth and trade heatmaps do not
/// allocate, on the portable path (rectangles) and on the core side of the
/// native path (layout and color range for `CustomDraw`).
#[test]
fn heatmap_frames_do_not_allocate() {
    for (source, spacing) in [
        (HeatmapSource::Depth, 120.0),
        (HeatmapSource::Trades, 18.0),
        (HeatmapSource::Trades, 4.0),
    ] {
        for native in [false, true] {
            let (mut chart, _) = heatmap(source, spacing);
            chart.set_native_kinds(if native { &[HEATMAP_KIND] } else { &[] });
            // `step` repeats every 140 steps (pan 20, zoom 14).
            for k in 0..140 {
                step(&mut chart, k);
            }
            let ((), n) = count_allocations(|| {
                for k in 0..140 {
                    step(&mut chart, k);
                }
            });
            assert_eq!(n, 0, "{source:?} at {spacing} px, native {native}");
        }
    }
}

/// Snapshots into the last column at levels it already has, each followed
/// by a frame, do not allocate.
#[test]
fn snapshots_into_the_last_column_do_not_allocate() {
    let (mut chart, id) = heatmap(HeatmapSource::Depth, 120.0);
    // The scene's last snapshot is at START + 2 399 s, around 127.5 with 40
    // levels a side; reuse levels it has.
    let t0 = START + 2_399_000;
    let prices: Vec<f64> = (0..20).map(|k| 125.0 + f64::from(k) * 0.25).collect();
    let mut sizes = vec![0.0; prices.len()];
    let mut tick = |chart: &mut Chart, k: usize| {
        for (j, s) in sizes.iter_mut().enumerate() {
            *s = ((j * 7 + k * 3) % 50) as f64;
        }
        let snap = DepthSnapshot {
            time: t0 + k as i64,
            prices: &prices,
            sizes: &sizes,
        };
        append_depth(chart, id, &snap).unwrap();
        chart.build_frame(k as f64, &MonoMeasurer);
    };
    for k in 0..20 {
        tick(&mut chart, k);
    }
    let ((), n) = count_allocations(|| {
        for k in 20..120 {
            tick(&mut chart, k);
        }
    });
    assert_eq!(n, 0);
}

fn tpo(days: usize, spacing: f64) -> (Chart, SeriesId) {
    let theme = T::light();
    let mut chart = Chart::new(ChartOptions {
        theme,
        max_bar_spacing: Css(240.0),
        ..ChartOptions::default()
    });
    chart.resize(
        SizeCss {
            w: 1200.0,
            h: 800.0,
        },
        2.0,
    );
    let id = tpo_scene(&mut chart, &theme, START, days, spacing).unwrap();
    (chart, id)
}

/// Pan, zoom and crosshair frames over TPO letters and histograms do not
/// allocate.
#[test]
fn tpo_frames_do_not_allocate() {
    for (days, spacing) in [(3, 230.0), (12, 60.0)] {
        let (mut chart, _) = tpo(days, spacing);
        for k in 0..140 {
            step(&mut chart, k);
        }
        let ((), n) = count_allocations(|| {
            for k in 0..140 {
                step(&mut chart, k);
            }
        });
        assert_eq!(n, 0, "{days} days at {spacing} px");
    }
}

/// Updates of a live bar in the last session (within its period's range
/// and widening it), each followed by a frame, do not allocate.
#[test]
fn live_bars_do_not_allocate() {
    let (mut chart, id) = tpo(3, 230.0);
    // The last bar of the scene's last session: 21:00 UTC of day 3 − 1 min.
    let t = START + 2 * 86_400_000 + 21 * 3_600_000 - 60_000;
    let mut bar = OhlcColumns {
        time: vec![t],
        open: vec![104.0],
        high: vec![104.0],
        low: vec![104.0],
        close: vec![104.0],
        volume: None,
    };
    let mut tick = |chart: &mut Chart, k: usize| {
        let d = (k % 5) as f64 * 0.25;
        bar.high[0] = 104.0 + d;
        bar.low[0] = 104.0 - d;
        append_bars(chart, id, &bar).unwrap();
        chart.build_frame(k as f64, &MonoMeasurer);
    };
    for k in 0..20 {
        tick(&mut chart, k);
    }
    let ((), n) = count_allocations(|| {
        for k in 20..120 {
            tick(&mut chart, k);
        }
    });
    assert_eq!(n, 0);
}
