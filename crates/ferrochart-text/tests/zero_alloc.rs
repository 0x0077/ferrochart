//! Zero-allocation tests for text layout (r21): once a grapheme cluster has
//! been seen, laying it out again and measuring it do not allocate.
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

use ferrochart_text::{GlyphBitmap, GlyphCache, GlyphSource, TextEngine};

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

/// One em per cluster, a 1×1 bitmap.
struct Square;

impl GlyphSource for Square {
    fn advance(&self, _: &str, size: f32) -> f32 {
        size
    }

    fn rasterize(&self, _: &str, _: f32) -> Option<GlyphBitmap> {
        Some(GlyphBitmap {
            width: 1,
            height: 1,
            left: 0,
            top: -1,
            coverage: vec![255],
            rgba: None,
        })
    }
}

#[test]
fn counter_detects_allocations() {
    let (_, n) = count_allocations(|| std::hint::black_box(vec![1_u8]));
    assert!(n >= 1);
}

#[test]
fn cluster_layout_does_not_allocate_after_first_use() {
    let mut engine = TextEngine::new();
    engine.set_glyph_source(Some(Box::new(Square)));
    let mut cache = GlyphCache::default();
    // Skin tone, ZWJ family, flag, combining accent, CJK, ASCII.
    let text = "\u{1F44D}\u{1F3FD} \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} \
                \u{1F1E8}\u{1F1F3} e\u{301} 中文 x2";
    let draw = |engine: &TextEngine, cache: &mut GlyphCache| {
        let mut pen = 0.0_f32;
        for g in engine.glyphs(0, text, 12.0) {
            pen += g.advance;
            if let Some(key) = g.key {
                std::hint::black_box(cache.get_key(engine, key, 12.0));
            }
        }
        pen + engine.width(0, text, 12.0)
    };
    // Warm-up: numbers the clusters and fills the cache.
    draw(&engine, &mut cache);
    for _ in 0..3 {
        let (_, n) = count_allocations(|| draw(&engine, &mut cache));
        assert_eq!(n, 0, "allocations after warm-up");
    }
}
