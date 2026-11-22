//! P2 memory benchmark: live heap bytes per bar of a chart holding one OHLCV
//! series. Target: < 64 bytes/bar.
//!
//! A counting global allocator tracks live bytes (allocations minus
//! deallocations). The input columns are moved into the chart, so the
//! measurement is "everything the chart retains after `set_data`" — data,
//! time index, LOD pyramid, frame pools and scratch buffers, including any
//! capacity slack (which P2 excludes, so this is an upper bound).
//!
//! Run: `cargo bench -p ferrochart-bench --bench memory`.
#![allow(missing_docs, clippy::unwrap_used)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::SizeCss;
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions};

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

fn add(n: usize, sign: isize) {
    LIVE.fetch_add(
        sign * isize::try_from(n).unwrap_or(isize::MAX),
        Ordering::Relaxed,
    );
}

// SAFETY: every method forwards to `System` unchanged; the counter is an atomic
// and never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        add(layout.size(), 1);
        // SAFETY: forwarded unchanged; the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        add(layout.size(), 1);
        // SAFETY: as above.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        add(layout.size(), -1);
        add(new_size, 1);
        // SAFETY: as above; `ptr` was allocated by this allocator, i.e. by `System`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        add(layout.size(), -1);
        // SAFETY: as above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const BUDGET: f64 = 64.0;

fn main() {
    let mut failed = false;
    for n in [1_000_000_usize, 10_000_000] {
        let before = LIVE.load(Ordering::Relaxed);
        let mut chart = Chart::new(ChartOptions::default());
        chart.resize(
            SizeCss {
                w: 1600.0,
                h: 900.0,
            },
            2.0,
        );
        let pane = chart.main_pane();
        let id = chart
            .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
            .unwrap();
        chart
            .add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(id))
            .unwrap();
        let data = random_walk(1, n, 1_704_067_200_000, 60_000).unwrap();
        chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
        let live = LIVE.load(Ordering::Relaxed) - before;
        let per_bar = live as f64 / n as f64;
        let core = chart.heap_bytes() as f64 / n as f64;
        let ok = per_bar < BUDGET;
        failed |= !ok;
        println!(
            "memory/{n:>8} bars: {per_bar:6.2} B/bar live heap ({core:6.2} B/bar series data) — P2 < {BUDGET} {}",
            if ok { "OK" } else { "FAIL" }
        );
        drop(chart);
    }
    if failed {
        std::process::exit(1);
    }
}
