//! Zero-allocation test of `update_last` with indicators attached.
//! Same per-thread counting allocator as
//! `crates/ferrochart-core/tests/zero_alloc.rs`; indicator hot paths are tested
//! here because core cannot depend on this crate.
//!
//! The counting allocator lives here, in an integration test, because the library
//! forbids `unsafe` and a global allocator needs `unsafe impl`. Counting is
//! **per thread**: the test harness runs tests in parallel, and allocations made by
//! other test threads must not be counted. All zero-allocation tests go in this
//! file, so it also builds as its own test binary.
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

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::BarUpdate;
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::SizeCss;
use ferrochart_core::plugin::Indicator;
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_indicators as ind;
use ferrochart_indicators::{Source, VwapAnchor};

#[test]
fn counter_detects_allocations() {
    let (v, n) = count_allocations(|| vec![1u8; 16]);
    assert_eq!(v.len(), 16);
    assert!(n >= 1);
}

/// Real-time ticks of the last bar with all 30 indicators attached, and the
/// frames that follow them, do not allocate.
#[test]
fn update_last_with_indicators_does_not_allocate() {
    let c = Source::Close;
    let indicators: Vec<Box<dyn Indicator>> = vec![
        Box::new(ind::sma(20, c).unwrap()),
        Box::new(ind::ema(20, c).unwrap()),
        Box::new(ind::wma(20, c).unwrap()),
        Box::new(ind::hma(20, c).unwrap()),
        Box::new(ind::vwma(20, c).unwrap()),
        Box::new(ind::dema(20, c).unwrap()),
        Box::new(ind::tema(20, c).unwrap()),
        Box::new(ind::volume_ma(20).unwrap()),
        Box::new(ind::boll(20, 2.0, c).unwrap()),
        Box::new(ind::keltner(20, 2.0, 10).unwrap()),
        Box::new(ind::donchian(20).unwrap()),
        Box::new(ind::ichimoku(9, 26, 52, 26).unwrap()),
        Box::new(ind::sar(0.02, 0.02, 0.2).unwrap()),
        Box::new(ind::supertrend(10, 3.0).unwrap()),
        Box::new(ind::vwap(VwapAnchor::Day, 0).unwrap()),
        Box::new(ind::macd(12, 26, 9, c).unwrap()),
        Box::new(ind::rsi(14, c).unwrap()),
        Box::new(ind::stoch(14, 3, 3).unwrap()),
        Box::new(ind::stochrsi(14, 14, 3, 3, c).unwrap()),
        Box::new(ind::cci(20).unwrap()),
        Box::new(ind::willr(14).unwrap()),
        Box::new(ind::mfi(14).unwrap()),
        Box::new(ind::adx(14).unwrap()),
        Box::new(ind::atr(14).unwrap()),
        Box::new(ind::obv().unwrap()),
        Box::new(ind::cmf(20).unwrap()),
        Box::new(ind::roc(10, c).unwrap()),
        Box::new(ind::momentum(10, c).unwrap()),
        Box::new(ind::trix(15, c).unwrap()),
        Box::new(ind::aroon(14).unwrap()),
    ];
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(
        SizeCss {
            w: 1200.0,
            h: 800.0,
        },
        2.0,
    );
    let candles = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    let data = random_walk(5, 100_000, 0, 60_000).unwrap();
    let last = data.time.len() - 1;
    let (t, o, v) = (
        data.time[last],
        data.open[last],
        data.volume.as_ref().unwrap()[last],
    );
    chart.set_data(candles, SeriesData::Ohlc(data)).unwrap();
    // A volume overlay: the volume MA goes on its scale (`Placement::Volume`).
    chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Volume,
            SeriesOptions::volume_of(candles),
        )
        .unwrap();
    for i in indicators {
        let p = i.default_placement();
        chart.add_indicator(candles, i, p).unwrap();
    }
    let tick = |k: u32| {
        let close = o + f64::from(k % 7) * 0.01 - 0.03;
        BarUpdate {
            time: t,
            open: o,
            high: o.max(close) + 0.02,
            low: o.min(close) - 0.02,
            close,
            volume: Some(v + f64::from(k)),
        }
    };
    for k in 0..20 {
        chart.update_last(candles, tick(k)).unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
    }
    let ((), n_update) = count_allocations(|| {
        for k in 20..120 {
            chart.update_last(candles, tick(k)).unwrap();
        }
    });
    let ((), n_frame) = count_allocations(|| {
        for k in 120..220 {
            chart.update_last(candles, tick(k)).unwrap();
            chart.build_frame(0.0, &MonoMeasurer);
        }
    });
    assert_eq!((n_update, n_frame), (0, 0));
}
