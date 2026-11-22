//! Zero-allocation tests of the plot hot paths: after warm-up, building frames while
//! panning, zooming
//! and hovering, and `update_last`, allocate nothing. Scenes: a large
//! scatter, a long sorted line, a heatmap, stacked bars with values, and a
//! waterfall.
//!
//! The counting allocator lives in an integration test because the library
//! forbids `unsafe`. Counting is per thread (tests run in parallel). All
//! zero-allocation tests of this crate go in this file.
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

#[test]
fn counter_counts() {
    let (v, n) = count_allocations(|| vec![1u8; 16]);
    assert_eq!(v.len(), 16);
    assert!(n >= 1);
}

use ferrochart_core::geom::{Css, PointCss, SizeCss};
use ferrochart_core::interaction::{Button, InputEvent, Modifiers, WheelDelta};
use ferrochart_core::testing::MonoMeasurer;
use ferrochart_plot::{
    AxisFormat, AxisId, AxisOptions, MarkData, MarkKind, MarkOptions, MarkPoint, Plot, PlotEvent,
    PlotOptions, WaterfallStep,
};

const W: f32 = 800.0;
const H: f32 = 450.0;

fn plot(crosshair: bool) -> Plot {
    let mut p = Plot::new(PlotOptions {
        crosshair,
        ..PlotOptions::default()
    });
    p.resize(SizeCss { w: W, h: H }, 2.0);
    p
}

/// A deterministic pseudo-random sequence in [0, 1).
fn noise(n: usize, seed: u64) -> Vec<f64> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (s >> 11) as f64 / (1u64 << 53) as f64
        })
        .collect()
}

fn scatter() -> Plot {
    let mut p = plot(true);
    let n = 200_000;
    let m = p
        .add_mark(
            MarkKind::Scatter,
            MarkOptions {
                point_size: Css(3.0),
                ..MarkOptions::default()
            },
        )
        .unwrap();
    let (x, y, v) = (noise(n, 1), noise(n, 2), noise(n, 3));
    let mut d = MarkData::xy(
        x.iter().map(|a| a * 100.0).collect(),
        y.iter().map(|a| a * 50.0).collect(),
    );
    d.value = Some(v);
    p.set_data(m, d).unwrap();
    p
}

fn line() -> Plot {
    let mut p = plot(true);
    let n = 1_000_000;
    let m = p.add_mark(MarkKind::Line, MarkOptions::default()).unwrap();
    let mut acc = 0.0;
    let y: Vec<f64> = noise(n, 4)
        .into_iter()
        .map(|r| {
            acc += r - 0.5;
            acc
        })
        .collect();
    p.set_data(m, MarkData::xy((0..n).map(|i| i as f64).collect(), y))
        .unwrap();
    p
}

fn cells() -> Plot {
    let mut p = plot(false);
    p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
    p.set_axis(AxisId::Right, AxisOptions::category()).unwrap();
    let names: Vec<String> = (0..100).map(|k| format!("C{k}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    p.set_categories(AxisId::X, &refs).unwrap();
    p.set_categories(AxisId::Right, &refs).unwrap();
    let m = p
        .add_mark(
            MarkKind::Cells,
            MarkOptions {
                show_values: true,
                value_format: AxisFormat::Fixed(1),
                ..MarkOptions::default()
            },
        )
        .unwrap();
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for r in 0..100 {
        for c in 0..100 {
            x.push(f64::from(c));
            y.push(f64::from(r));
        }
    }
    let mut d = MarkData::xy(x, y);
    d.value = Some(noise(10_000, 5));
    p.set_data(m, d).unwrap();
    p
}

fn bars() -> Plot {
    let mut p = plot(true);
    p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
    for k in 0..4 {
        let m = p
            .add_mark(
                MarkKind::Bars,
                MarkOptions {
                    stack: Some(1),
                    show_values: true,
                    ..MarkOptions::default()
                },
            )
            .unwrap();
        let y = noise(500, 10 + k)
            .into_iter()
            .map(|r| r * 20.0 - 5.0)
            .collect();
        p.set_data(m, MarkData::xy((0..500).map(f64::from).collect(), y))
            .unwrap();
    }
    let w = p
        .add_mark(
            MarkKind::Waterfall,
            MarkOptions {
                show_values: true,
                y_axis: AxisId::Left,
                ..MarkOptions::default()
            },
        )
        .unwrap();
    let mut d = MarkData::xy(
        (0..20).map(f64::from).collect(),
        noise(20, 20).into_iter().map(|r| r * 10.0 - 4.0).collect(),
    );
    let mut steps = vec![WaterfallStep::Delta; 20];
    steps[9] = WaterfallStep::Subtotal;
    steps[19] = WaterfallStep::Total;
    d.step = Some(steps);
    p.set_data(w, d).unwrap();
    p
}

/// One round of interaction: hover across the plot, a wheel zoom, a drag
/// and its kinetic tail, frames in between. Returns how many hover and
/// view-change events it caused.
fn interact(p: &mut Plot, t0: f64) -> (usize, usize) {
    let (mut hovers, mut views) = (0, 0);
    let mut count = |p: &mut Plot| {
        for e in p.drain_events() {
            match e {
                PlotEvent::Hover(_) => hovers += 1,
                PlotEvent::ViewChanged => views += 1,
                PlotEvent::Click { .. } => {}
            }
        }
    };
    let text = MonoMeasurer;
    let mods = Modifiers::default();
    let mut t = t0;
    for k in 0..20 {
        let pos = PointCss {
            x: 100.0 + 25.0 * k as f32,
            y: 150.0 + 5.0 * k as f32,
        };
        p.handle_input(InputEvent::PointerMove {
            id: 0,
            pos,
            mods,
            time_ms: t,
        });
        p.build_frame(t, &text);
        count(p);
        t += 16.0;
    }
    let pos = PointCss { x: 400.0, y: 200.0 };
    for dy in [-120.0, 80.0] {
        p.handle_input(InputEvent::Wheel {
            pos,
            delta: WheelDelta::Pixels {
                x: Css(0.0),
                y: Css(dy),
            },
            mods,
        });
        p.build_frame(t, &text);
        count(p);
        t += 16.0;
    }
    p.handle_input(InputEvent::PointerDown {
        id: 0,
        pos,
        button: Button::Left,
        mods,
        time_ms: t,
    });
    for k in 1..=8 {
        t += 10.0;
        let q = PointCss {
            x: pos.x + 12.0 * k as f32,
            y: pos.y + 3.0 * k as f32,
        };
        p.handle_input(InputEvent::PointerMove {
            id: 0,
            pos: q,
            mods,
            time_ms: t,
        });
        p.build_frame(t, &text);
    }
    p.handle_input(InputEvent::PointerUp {
        id: 0,
        pos: PointCss {
            x: pos.x + 96.0,
            y: pos.y + 24.0,
        },
        button: Button::Left,
        mods,
        time_ms: t,
    });
    for _ in 0..30 {
        t += 16.0;
        p.build_frame(t, &text);
    }
    count(p);
    (hovers, views)
}

/// Warms `p` up with two rounds (pools and scratch buffers reach their
/// size), then asserts a third round allocates nothing.
fn assert_zero_alloc(name: &str, mut p: Plot) {
    p.build_frame(0.0, &MonoMeasurer);
    interact(&mut p, 0.0);
    p.fit();
    interact(&mut p, 2000.0);
    p.fit();
    let ((hovers, views), n) = count_allocations(|| interact(&mut p, 4000.0));
    assert_eq!(
        n, 0,
        "{name}: {n} allocations while panning, zooming and hovering"
    );
    // The round did hover and move the view (not a vacuous pass).
    assert!(
        hovers > 0 && views > 0,
        "{name}: {hovers} hovers, {views} view changes"
    );
}

#[test]
fn scatter_200k_pan_zoom_hover() {
    assert_zero_alloc("scatter", scatter());
}

#[test]
fn sorted_line_1m_pan_zoom_hover() {
    assert_zero_alloc("line", line());
}

#[test]
fn heatmap_10k_pan_zoom_hover() {
    assert_zero_alloc("cells", cells());
}

#[test]
fn stacked_bars_and_waterfall_pan_zoom_hover() {
    assert_zero_alloc("bars", bars());
}

#[test]
fn update_last_and_frame() {
    for (name, mut p) in [("scatter", scatter()), ("line", line()), ("bars", bars())] {
        let id = p.marks().next().unwrap();
        let text = MonoMeasurer;
        let tick = |p: &mut Plot, k: u32| {
            let n = p.mark_data(id).unwrap().len();
            let last = n as f64 - 1.0;
            let x = if name == "scatter" { 50.0 } else { last };
            p.update_last(id, MarkPoint::xy(x, f64::from(k % 7)))
                .unwrap();
            p.build_frame(f64::from(k), &text);
        };
        for k in 0..3 {
            tick(&mut p, k);
        }
        let ((), n) = count_allocations(|| {
            for k in 3..20 {
                tick(&mut p, k);
            }
        });
        assert_eq!(n, 0, "{name}: {n} allocations in update_last + build_frame");
    }
}

/// An intro animation: progress from 0 to 1 with the pen, every
/// frame, allocates nothing after one warm-up run.
#[test]
fn reveal_frames_do_not_allocate() {
    use ferrochart_core::frame::Reveal;
    use ferrochart_core::geom::Rgba;
    let mut p = line();
    let ramp = |p: &mut Plot| {
        for i in 0..=120 {
            p.set_reveal(Some(Reveal {
                progress: i as f32 / 120.0,
                pen: Some(Rgba::hex(0x004c_c9f0)),
            }));
            p.build_frame(0.0, &MonoMeasurer);
        }
        p.set_reveal(None);
        p.build_frame(0.0, &MonoMeasurer);
    };
    ramp(&mut p);
    let ((), n) = count_allocations(|| ramp(&mut p));
    assert_eq!(n, 0, "{n} allocations while revealing");
}
