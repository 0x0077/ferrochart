//! Zero-allocation tests for hot paths.
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

use ferrochart_core::data::{ChangeLog, DataDelta, DeqColumn, IdxSpan, TimeIndex};

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
fn counter_detects_allocations() {
    let (v, n) = count_allocations(|| Vec::<u64>::with_capacity(16));
    assert_eq!(n, 1);
    drop(v);
    let ((), n) = count_allocations(|| {});
    assert_eq!(n, 0);
}

/// `update_last` path: a tick rewrites the last value of each column and records
/// the change; a renderer then asks what changed.
#[test]
fn realtime_tick_does_not_allocate() {
    let mut close = DeqColumn::from_vec((0..100_000).map(f64::from).collect());
    let mut log = ChangeLog::new();
    let last = i64::try_from(close.len()).unwrap() - 1;

    let (synced, n) = count_allocations(|| {
        let mut synced = log.generation();
        for i in 0..10_000 {
            if let Some(v) = close.last_mut() {
                *v = f64::from(i);
            }
            log.record(IdxSpan {
                start: last,
                end: last + 1,
            });
            if i % 7 == 0 {
                assert_ne!(log.changes_since(synced), DataDelta::Full);
                synced = log.generation();
            }
        }
        synced
    });
    assert_eq!(n, 0, "{n} allocations");
    assert!(synced > 1);
}

/// Change-log overflow (many disjoint changes) must not allocate either.
#[test]
fn change_log_overflow_does_not_allocate() {
    let mut log = ChangeLog::new();
    let ((), n) = count_allocations(|| {
        for i in 0..10_000 {
            log.record(IdxSpan {
                start: i * 3,
                end: i * 3 + 1,
            });
        }
        assert!(matches!(log.changes_since(1), DataDelta::Range(_)));
    });
    assert_eq!(n, 0, "{n} allocations");
}

/// Crosshair / pan path: time lookups on the index.
#[test]
fn time_lookups_do_not_allocate() {
    let index = TimeIndex::from_times((0..1_000_000).map(|i| i * 60_000).collect()).unwrap();
    let (sum, n) = count_allocations(|| {
        let mut sum = 0_i64;
        for t in (0..60_000_000_000_i64).step_by(7_777_777) {
            sum += index.lower_bound(t);
            sum += index.index_of(t).unwrap_or(0);
            sum += index.time_at(t / 60_000).unwrap_or(0);
        }
        sum
    });
    assert_eq!(n, 0, "{n} allocations");
    assert!(sum > 0);
}

mod chart_hot_paths {
    //! `build_frame` (pan, zoom) and `Chart::update_last` after warm-up.

    use super::count_allocations;
    use ferrochart_core::chart::{BandOptions, SeriesData, SeriesOptions};
    use ferrochart_core::data::{BarUpdate, DataChange, SeriesId};
    use ferrochart_core::frame::{Reveal, SeriesKind};
    use ferrochart_core::geom::{Px, SizeCss};
    use ferrochart_core::testing::{MonoMeasurer, random_walk};
    use ferrochart_core::{Chart, ChartOptions};

    const START: i64 = 1_704_067_200_000;

    fn chart(bars: usize) -> (Chart, SeriesId) {
        let mut chart = Chart::new(ChartOptions::default());
        chart.resize(
            SizeCss {
                w: 1200.0,
                h: 600.0,
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
        let data = random_walk(5, bars, START, 60_000).unwrap();
        chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
        (chart, id)
    }

    #[test]
    fn build_frame_pan_does_not_allocate() {
        let (mut chart, _) = chart(1_000_000);
        // Warm up: pan back through a different region than the one measured.
        for _ in 0..60 {
            chart.time_scale().scroll_by(Px(97.0));
            chart.build_frame(0.0, &MonoMeasurer);
        }
        let ((), n) = count_allocations(|| {
            for _ in 0..300 {
                chart.time_scale().scroll_by(Px(41.0));
                chart.build_frame(0.0, &MonoMeasurer);
            }
        });
        assert_eq!(n, 0);
        assert!(!chart.frame().layers[1].cmds.is_empty());
    }

    /// An intro animation: the host advances `set_reveal` every
    /// frame from 0 to 1, with the pen.
    #[test]
    fn reveal_frames_do_not_allocate() {
        let (mut chart, _) = chart(100_000);
        let pen = Some(ferrochart_core::geom::Rgba::hex(0x004c_c9f0));
        let ramp = |chart: &mut Chart| {
            for i in 0..=120 {
                chart.set_reveal(Some(Reveal {
                    progress: i as f32 / 120.0,
                    pen,
                }));
                chart.build_frame(0.0, &MonoMeasurer);
            }
            chart.set_reveal(None);
            chart.build_frame(0.0, &MonoMeasurer);
        };
        ramp(&mut chart);
        let ((), n) = count_allocations(|| ramp(&mut chart));
        assert_eq!(n, 0);
    }

    /// Automatic precision on a meme-coin price: labels with
    /// subscript zeros are measured on the fast path while panning.
    #[test]
    fn subscript_price_labels_do_not_allocate() {
        let mut chart = Chart::new(ChartOptions::default());
        chart.resize(
            SizeCss {
                w: 1200.0,
                h: 600.0,
            },
            2.0,
        );
        let pane = chart.main_pane();
        let id = chart
            .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
            .unwrap();
        let mut data = random_walk(5, 200_000, START, 60_000).unwrap();
        for col in [
            &mut data.open,
            &mut data.high,
            &mut data.low,
            &mut data.close,
        ] {
            for v in col.iter_mut() {
                *v *= 1e-7;
            }
        }
        chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
        for _ in 0..60 {
            chart.time_scale().scroll_by(Px(97.0));
            chart.build_frame(0.0, &MonoMeasurer);
        }
        let ((), n) = count_allocations(|| {
            for _ in 0..300 {
                chart.time_scale().scroll_by(Px(41.0));
                chart.build_frame(0.0, &MonoMeasurer);
            }
        });
        assert_eq!(n, 0);
        let f = chart.frame();
        let subscript = |c: char| ('\u{2080}'..='\u{2089}').contains(&c);
        assert!(f.pools.text.chars().any(subscript), "{}", f.pools.text);
    }

    /// Price lines with titles, value labels and markers.
    #[test]
    fn price_lines_and_markers_do_not_allocate() {
        let (mut chart, id) = chart(10_000);
        for (n, price) in [100.0, 101.0, 99.0, 100.5].into_iter().enumerate() {
            chart
                .create_price_line(
                    id,
                    ferrochart_core::chart::PriceLineOptions {
                        price,
                        title: format!("line {n}"),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let times = chart.data_view().time_index().times().to_vec();
        let markers = times
            .iter()
            .step_by(7)
            .enumerate()
            .map(|(n, &time)| ferrochart_core::chart::Marker {
                time,
                position: if n % 2 == 0 {
                    ferrochart_core::chart::MarkerPosition::AboveBar
                } else {
                    ferrochart_core::chart::MarkerPosition::BelowBar
                },
                text: if n % 3 == 0 {
                    "buy".into()
                } else {
                    String::new()
                },
                ..Default::default()
            })
            .collect();
        chart.set_markers(id, markers).unwrap();
        for _ in 0..30 {
            chart.time_scale().scroll_by(Px(53.0));
            chart.build_frame(0.0, &MonoMeasurer);
        }
        let ((), n) = count_allocations(|| {
            for _ in 0..100 {
                chart.time_scale().scroll_by(Px(-29.0));
                chart.build_frame(0.0, &MonoMeasurer);
            }
        });
        assert_eq!(n, 0);
    }

    /// Every built-in drawing tool, one of them selected (handles), while
    /// panning, zooming and moving the crosshair.
    #[test]
    fn drawings_do_not_allocate() {
        use ferrochart_core::drawing::{Anchor, BUILTIN_KINDS, DrawingData};
        use ferrochart_core::frame::DrawCmd;
        use ferrochart_core::geom::PointCss;
        use ferrochart_core::interaction::{InputEvent, Modifiers};

        let (mut chart, _) = chart(10_000);
        let pane = chart.main_pane();
        let times = chart.data_view().time_index().times().to_vec();
        let mut last = None;
        for (k, kind) in BUILTIN_KINDS.iter().enumerate() {
            let n = match *kind {
                "parallel_channel"
                | "fib_extension"
                | "pitchfork"
                | "schiff_pitchfork"
                | "modified_schiff_pitchfork" => 3,
                "horizontal_line" | "horizontal_ray" | "vertical_line" | "text" => 1,
                "elliott_correction" => 4,
                "xabcd_pattern" => 5,
                "elliott_impulse" => 6,
                "head_and_shoulders" => 7,
                // A long stroke and a path with many vertices.
                "brush" => 500,
                "path" => 12,
                _ => 2,
            };
            let anchors = (0..n)
                .map(|i| Anchor {
                    time: times[9_000 + 30 * k + if n > 20 { i / 10 } else { 5 * i }],
                    price: 98.0 + 2.0 * (i % 3) as f64,
                })
                .collect();
            let id = chart
                .drawings()
                .add(pane, &DrawingData::new(kind, anchors))
                .unwrap();
            // Selected: 7 handles (r11: written into a reused buffer).
            if *kind == "head_and_shoulders" {
                last = Some(id);
            }
        }
        chart.drawings().select(last).unwrap();
        let step = |chart: &mut Chart, k: usize| {
            let dx = if k % 20 < 10 { 37.0 } else { -37.0 };
            chart.time_scale().scroll_by(Px(dx));
            if k.is_multiple_of(7) {
                chart
                    .time_scale()
                    .zoom(Px(300.0), if k.is_multiple_of(14) { 1.1 } else { 0.9 });
            }
            chart.handle_input(InputEvent::PointerMove {
                id: 0,
                pos: PointCss {
                    x: 100.0 + (k % 50) as f32 * 10.0,
                    y: 200.0,
                },
                mods: Modifiers::default(),
                time_ms: k as f64,
            });
            chart.build_frame(k as f64, &MonoMeasurer);
        };
        // Warm up over the same states as the measured run: with a long
        // brush the pools peak at a pan/zoom state after step 100.
        for k in 0..200 {
            step(&mut chart, k);
        }
        // The drawings are in view: polygons (channel, ellipse) and handles.
        let cmds = &chart.frame().layers[1].cmds;
        assert!(cmds.iter().any(|c| matches!(c, DrawCmd::Polygon { .. })));
        assert!(cmds.iter().any(|c| matches!(c, DrawCmd::Icon(_))));
        let ((), n) = count_allocations(|| {
            for k in 0..200 {
                step(&mut chart, k);
            }
        });
        assert_eq!(n, 0);
    }

    /// Image markers with borders, labels and folded stacks, hovered by the
    /// crosshair while panning, zooming and ticking the last bar (r21).
    #[test]
    fn image_markers_do_not_allocate() {
        use ferrochart_core::chart::{Marker, MarkerPosition};
        use ferrochart_core::geom::{Css, PointCss, Rgba};
        use ferrochart_core::icon::IconImage;
        use ferrochart_core::interaction::{InputEvent, Modifiers};
        use ferrochart_core::testing::test_avatar;

        let (mut chart, id) = chart(10_000);
        chart
            .set_series_options(
                id,
                SeriesOptions {
                    marker_stack_limit: Some(3),
                    ..SeriesOptions::default()
                },
            )
            .unwrap();
        let icons: Vec<_> = (0..4_u8)
            .map(|k| {
                let px = test_avatar(48, k * 60);
                chart
                    .add_icon(IconImage {
                        width: 48,
                        height: 48,
                        rgba: &px,
                    })
                    .unwrap()
            })
            .collect();
        let times = chart.data_view().time_index().times().to_vec();
        // 500 markers on the last ~200 bars, stacks of 1..=7 (folded above 3).
        let mut markers = Vec::new();
        let mut i = times.len() - 200;
        while markers.len() < 500 {
            for k in 0..=(i % 7).min(500 - markers.len() - 1) {
                markers.push(Marker {
                    time: times[i],
                    position: if i % 2 == 0 {
                        MarkerPosition::AboveBar
                    } else {
                        MarkerPosition::BelowBar
                    },
                    size: Some(Css(20.0)),
                    border: Some(Rgba::hex(0x00EF_5350)),
                    // Every fifth bar starts with a letter badge (an icon or
                    // a label, not both).
                    ..if k == 0 && i % 5 == 0 {
                        Marker {
                            label: "DS".into(),
                            ..Marker::default()
                        }
                    } else {
                        Marker {
                            icon: Some(icons[k % icons.len()]),
                            ..Marker::default()
                        }
                    }
                });
            }
            i += 1;
        }
        chart.set_markers(id, markers).unwrap();
        let last_time = *times.last().unwrap();
        let step = |chart: &mut Chart, k: usize| {
            let dx = if k % 20 < 10 { 23.0 } else { -23.0 };
            chart.time_scale().scroll_by(Px(dx));
            if k.is_multiple_of(7) {
                chart
                    .time_scale()
                    .zoom(Px(900.0), if k.is_multiple_of(14) { 1.1 } else { 0.9 });
            }
            chart.handle_input(InputEvent::PointerMove {
                id: 0,
                pos: PointCss {
                    x: 700.0 + (k % 50) as f32 * 9.0,
                    y: 40.0 + (k % 26) as f32 * 20.0,
                },
                mods: Modifiers::default(),
                time_ms: k as f64,
            });
            let k = k as u32;
            chart
                .update_last(
                    id,
                    BarUpdate {
                        time: last_time,
                        open: 100.0,
                        high: 103.0 + f64::from(k % 7),
                        low: 97.0 - f64::from(k % 5),
                        close: 100.0 + f64::from(k % 3),
                        volume: Some(1000.0),
                    },
                )
                .unwrap();
            chart.build_frame(f64::from(k), &MonoMeasurer);
            chart.drain_events().for_each(drop);
        };
        for k in 0..200 {
            step(&mut chart, k);
        }
        let frame = chart.frame();
        let icons_drawn = frame.layers[1]
            .cmds
            .iter()
            .filter(|c| matches!(c, ferrochart_core::frame::DrawCmd::Icon(_)))
            .count();
        assert!(icons_drawn > 0, "no markers in view");
        assert!(frame.pools.text.contains('+'), "no folded stack in view");
        let ((), n) = count_allocations(|| {
            for k in 0..200 {
                step(&mut chart, k);
            }
        });
        assert_eq!(n, 0);
    }

    #[test]
    fn build_frame_zoom_does_not_allocate() {
        let (mut chart, _) = chart(1_000_000);
        for f in [0.5, 0.25, 0.1, 0.05, 20.0, 40.0, 10.0] {
            chart.time_scale().zoom(Px(1000.0), f);
            chart.build_frame(0.0, &MonoMeasurer);
        }
        let ((), n) = count_allocations(|| {
            // Zoom out across several LOD levels and back in.
            for _ in 0..40 {
                chart.time_scale().zoom(Px(1300.0), 0.8);
                chart.build_frame(0.0, &MonoMeasurer);
            }
            for _ in 0..40 {
                chart.time_scale().zoom(Px(700.0), 1.25);
                chart.build_frame(0.0, &MonoMeasurer);
            }
        });
        assert_eq!(n, 0);
    }

    #[test]
    fn chart_update_last_does_not_allocate() {
        let (mut chart, id) = chart(100_000);
        let last_time = START + 99_999 * 60_000;
        let tick = |i: u32| BarUpdate {
            time: last_time,
            open: 100.0,
            high: 103.0 + f64::from(i % 7),
            low: 99.0 - f64::from(i % 5),
            close: 100.0 + f64::from(i % 3),
            volume: Some(1000.0 + f64::from(i)),
        };
        for i in 0..10 {
            chart.update_last(id, tick(i)).unwrap();
            chart.build_frame(0.0, &MonoMeasurer);
        }
        let ((), n) = count_allocations(|| {
            for i in 10..1010 {
                assert_eq!(
                    chart.update_last(id, tick(i)).unwrap(),
                    DataChange::UpdateLast
                );
                chart.build_frame(0.0, &MonoMeasurer);
            }
        });
        assert_eq!(n, 0);
    }
    #[test]
    fn crosshair_and_input_driven_pan_do_not_allocate() {
        use ferrochart_core::geom::PointCss;
        use ferrochart_core::interaction::{Button, InputEvent, Modifiers};

        let (mut chart, _) = chart(1_000_000);
        let mods = Modifiers::default();
        let mv = |x: f32, t: f64| InputEvent::PointerMove {
            id: 0,
            pos: PointCss { x, y: 250.0 },
            mods,
            time_ms: t,
        };
        // Warm up: crosshair moves, a pan and some kinetic frames.
        for i in 0..100 {
            chart.handle_input(mv(100.0 + i as f32 * 7.0, 0.0));
            chart.build_frame(0.0, &MonoMeasurer);
        }
        let ((), n) = count_allocations(|| {
            for i in 0..500 {
                chart.handle_input(mv(50.0 + (i % 900) as f32, f64::from(i)));
                chart.build_frame(f64::from(i), &MonoMeasurer);
            }
            chart.handle_input(InputEvent::PointerDown {
                id: 0,
                pos: PointCss { x: 600.0, y: 250.0 },
                button: Button::Left,
                mods,
                time_ms: 1000.0,
            });
            for i in 1..200 {
                chart.handle_input(mv(600.0 - i as f32 * 3.0, 1000.0 + f64::from(i) * 8.0));
                chart.build_frame(1000.0 + f64::from(i) * 8.0, &MonoMeasurer);
            }
            chart.handle_input(InputEvent::PointerUp {
                id: 0,
                pos: PointCss { x: 3.0, y: 250.0 },
                button: Button::Left,
                mods,
                time_ms: 2600.0,
            });
            let mut now = 2600.0;
            while chart.is_animating() {
                now += 16.0;
                chart.build_frame(now, &MonoMeasurer);
            }
        });
        assert_eq!(n, 0);
        // Events are coalesced; the queue stays short.
        assert!(chart.drain_events().count() <= 3);
    }
    #[test]
    fn multi_pane_frames_do_not_allocate() {
        use ferrochart_core::geom::PointCss;
        use ferrochart_core::interaction::{Button, InputEvent, Modifiers};
        use ferrochart_core::pane::PaneOptions;

        let mut chart = Chart::new(ChartOptions::default());
        chart.resize(
            SizeCss {
                w: 1200.0,
                h: 600.0,
            },
            2.0,
        );
        let id = chart
            .add_series(
                chart.main_pane(),
                SeriesKind::Candlestick,
                SeriesOptions::default(),
            )
            .unwrap();
        let below = chart.add_pane(PaneOptions {
            stretch_factor: 0.3,
        });
        chart.add_pane(PaneOptions::default());
        chart
            .add_series(below, SeriesKind::Volume, SeriesOptions::volume_of(id))
            .unwrap();
        let data = random_walk(5, 1_000_000, START, 60_000).unwrap();
        chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
        let mods = Modifiers::default();
        let mv = |y: f32, t: f64| InputEvent::PointerMove {
            id: 0,
            pos: PointCss { x: 500.0, y },
            mods,
            time_ms: t,
        };
        // The separator between the first two panes, in CSS px.
        let sep = chart.pane_layout(below).unwrap().plot.y / 2.0;
        let run = |chart: &mut Chart| {
            for i in 0..60 {
                chart.time_scale().scroll_by(Px(37.0));
                // The crosshair walks through all three panes.
                chart.handle_input(mv(20.0 + (i * 9) as f32, f64::from(i)));
                chart.build_frame(f64::from(i), &MonoMeasurer);
            }
            chart.handle_input(InputEvent::PointerDown {
                id: 0,
                pos: PointCss { x: 500.0, y: sep },
                button: Button::Left,
                mods,
                time_ms: 100.0,
            });
            for i in 0..40 {
                chart.handle_input(mv(sep + ((i % 10) as f32 - 5.0) * 4.0, 100.0));
                chart.build_frame(100.0, &MonoMeasurer);
            }
            chart.handle_input(InputEvent::PointerUp {
                id: 0,
                pos: PointCss { x: 500.0, y: sep },
                button: Button::Left,
                mods,
                time_ms: 200.0,
            });
            chart.build_frame(200.0, &MonoMeasurer);
        };
        run(&mut chart);
        let ((), n) = count_allocations(|| run(&mut chart));
        assert_eq!(n, 0);
    }

    /// Every series kind: pan, zoom across LOD levels, crosshair, and
    /// `update_last` on Heikin-Ashi and value series.
    #[test]
    fn all_series_kinds_do_not_allocate() {
        use ferrochart_core::data::ValueColumns;
        use ferrochart_core::geom::PointCss;
        use ferrochart_core::interaction::{InputEvent, Modifiers};
        use ferrochart_core::pane::PaneOptions;

        const N: usize = 200_000;
        let mut chart = Chart::new(ChartOptions::default());
        chart.resize(
            SizeCss {
                w: 1200.0,
                h: 600.0,
            },
            2.0,
        );
        let main = chart.main_pane();
        let ha = chart
            .add_series(main, SeriesKind::HeikinAshi, SeriesOptions::default())
            .unwrap();
        let data = random_walk(6, N, START, 60_000).unwrap();
        chart.set_data(ha, SeriesData::Ohlc(data.clone())).unwrap();
        let bars = chart
            .add_series(main, SeriesKind::OhlcBar, SeriesOptions::default())
            .unwrap();
        chart
            .set_data(bars, SeriesData::Ohlc(data.clone()))
            .unwrap();
        let lower = chart.add_pane(PaneOptions::default());
        let mut value_ids = Vec::new();
        for (pane, kind) in [
            (main, SeriesKind::Line),
            (main, SeriesKind::Area),
            (lower, SeriesKind::Baseline),
            (lower, SeriesKind::Histogram),
            (main, SeriesKind::Points),
        ] {
            let id = chart
                .add_series(
                    pane,
                    kind,
                    SeriesOptions {
                        step: kind == SeriesKind::Line,
                        base_value: 100.0,
                        ..SeriesOptions::default()
                    },
                )
                .unwrap();
            let color = (kind == SeriesKind::Histogram).then(|| vec![0x2196_F3FF; N]);
            let values = ValueColumns {
                time: data.time.clone(),
                value: data.close.clone(),
                color,
            };
            chart.set_data(id, SeriesData::Value(values)).unwrap();
            value_ids.push(id);
        }
        // A band between the line and the area.
        chart
            .add_band(value_ids[0], value_ids[1], BandOptions::default())
            .unwrap();
        let last_time = START + (N as i64 - 1) * 60_000;
        let tick = |i: u32| BarUpdate {
            time: last_time,
            open: 100.0,
            high: 103.0 + f64::from(i % 7),
            low: 99.0 - f64::from(i % 5),
            close: 100.0 + f64::from(i % 3),
            volume: Some(1000.0 + f64::from(i)),
        };
        let mods = Modifiers::default();
        let run = |chart: &mut Chart, base: u32| {
            for i in 0..30 {
                chart.time_scale().zoom(Px(1300.0), 0.7);
                chart.build_frame(0.0, &MonoMeasurer);
                chart.time_scale().scroll_by(Px(29.0 + i as f32));
                chart.handle_input(InputEvent::PointerMove {
                    id: 0,
                    pos: PointCss {
                        x: 100.0 + (i * 13) as f32,
                        y: 30.0 + (i * 17) as f32,
                    },
                    mods,
                    time_ms: f64::from(i),
                });
                chart.build_frame(f64::from(i), &MonoMeasurer);
            }
            for _ in 0..30 {
                chart.time_scale().zoom(Px(700.0), 1.0 / 0.7);
                chart.build_frame(0.0, &MonoMeasurer);
            }
            for i in base..base + 50 {
                chart.update_last(ha, tick(i)).unwrap();
                for &id in &value_ids {
                    chart.update_last(id, tick(i)).unwrap();
                }
                chart.build_frame(0.0, &MonoMeasurer);
            }
        };
        run(&mut chart, 0);
        let ((), n) = count_allocations(|| run(&mut chart, 100));
        assert_eq!(n, 0);
    }

    /// Synced charts: crosshair moves and pans on one chart, followed
    /// by the others, do not allocate.
    #[test]
    fn synced_charts_do_not_allocate() {
        use ferrochart_core::geom::PointCss;
        use ferrochart_core::interaction::{InputEvent, Modifiers};
        use ferrochart_core::sync::{SyncGroup, SyncOptions};

        let group = SyncGroup::new();
        let mut charts: Vec<Chart> = (0..4).map(|_| chart(100_000).0).collect();
        // Drawing sync on, with a synced drawing: frames without drawing
        // changes do not allocate either.
        let opts = SyncOptions {
            drawings: true,
            ..SyncOptions::default()
        };
        for c in &mut charts {
            c.join_sync(&group, opts);
        }
        let pane = charts[0].main_pane();
        let line = ferrochart_core::drawing::DrawingData::new(
            "horizontal_line",
            vec![ferrochart_core::drawing::Anchor {
                time: START + 50_000 * 60_000,
                price: 100.0,
            }],
        );
        charts[0].drawings().add(pane, &line).unwrap();
        let step = |charts: &mut [Chart], k: usize| {
            let src = k % charts.len();
            charts[src].handle_input(InputEvent::PointerMove {
                id: 0,
                pos: PointCss {
                    x: 100.0 + (k % 50) as f32 * 10.0,
                    y: 200.0,
                },
                mods: Modifiers::default(),
                time_ms: k as f64,
            });
            if k.is_multiple_of(3) {
                let dx = if k % 30 < 15 { 23.0 } else { -23.0 };
                charts[src].time_scale().scroll_by(Px(dx));
            }
            for c in charts.iter_mut() {
                c.build_frame(k as f64, &MonoMeasurer);
            }
        };
        for k in 0..100 {
            step(&mut charts, k);
        }
        // The charts follow each other (same data, so the same range, up to
        // the millisecond rounding of the message) once all have built a
        // frame after the last change.
        for c in &mut charts {
            c.build_frame(100.0, &MonoMeasurer);
        }
        assert!(charts.iter_mut().all(|c| c.drawings().ids().count() == 1));
        let r0 = charts[0].time_scale().visible_range();
        let r3 = charts[3].time_scale().visible_range();
        assert!((r0.0 - r3.0).abs() < 1e-3 && (r0.1 - r3.1).abs() < 1e-3);
        let ((), n) = count_allocations(|| {
            for k in 0..200 {
                step(&mut charts, k);
            }
        });
        assert_eq!(n, 0);
    }

    /// A custom series (on the candles' slots) and layer plugins: real-time
    /// `update_custom` and frames while panning.
    #[test]
    fn plugins_do_not_allocate() {
        use ferrochart_core::data::Timestamp;
        use ferrochart_core::drawing::{FrameBuilder, Label};
        use ferrochart_core::frame::TextAlign;
        use ferrochart_core::geom::{PointPx, RectPx, Rgba};
        use ferrochart_core::plugin::{
            AxisMode, CustomSeries, LayerCtx, LayerPlugin, LayerPosition, MinMaxIndex, SeriesCtx,
        };

        struct Bands {
            time: Vec<Timestamp>,
            range: Vec<(f64, f64)>,
            index: MinMaxIndex,
        }

        impl CustomSeries for Bands {
            fn kind(&self) -> &'static str {
                "test.bands"
            }
            fn axis_mode(&self) -> AxisMode {
                AxisMode::Slots
            }
            fn times(&self) -> &[Timestamp] {
                &self.time
            }
            fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
                self.index.query(from, to)
            }
            fn build(
                &self,
                ctx: &SeriesCtx<'_>,
                from: usize,
                to: usize,
                out: &mut FrameBuilder<'_>,
            ) {
                for i in from..to {
                    let (lo, hi) = self.range[i];
                    let (Some(x), Some(y0), Some(y1)) = (
                        ctx.x_of_point(i),
                        ctx.coords().y_of(hi),
                        ctx.coords().y_of(lo),
                    ) else {
                        continue;
                    };
                    let rect = RectPx {
                        x: x - 1.0,
                        y: y0,
                        w: 2.0,
                        h: y1 - y0,
                    };
                    out.rect(rect, ctx.style().up.with_alpha(0x40));
                }
            }
            fn last_value(&self) -> Option<f64> {
                self.range.last().map(|r| r.1)
            }
        }

        struct Watermark;

        impl LayerPlugin for Watermark {
            fn position(&self) -> LayerPosition {
                LayerPosition::Background
            }
            fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
                let r = ctx.rect();
                let label = Label {
                    font: ctx.font(ferrochart_core::geom::Css(32.0)),
                    color: Rgba::hex(0x80_8080).with_alpha(0x40),
                    align: TextAlign::Center,
                    bg: None,
                };
                let at = PointPx {
                    x: r.x + r.w / 2.0,
                    y: r.y + r.h / 2.0,
                };
                out.text(at, "BTCUSD", label);
            }
        }

        let (mut chart, _) = chart(10_000);
        let pane = chart.main_pane();
        let time = chart.data_view().time_index().times().to_vec();
        let mut bands = Bands {
            range: (0..time.len())
                .map(|i| (90.0 + (i % 7) as f64, 110.0))
                .collect(),
            time,
            index: MinMaxIndex::new(),
        };
        for &(lo, hi) in &bands.range {
            bands.index.push(lo, hi).unwrap();
        }
        let id = chart
            .add_custom_series(pane, Box::new(bands), SeriesOptions::default())
            .unwrap();
        chart.add_layer(Some(pane), Box::new(Watermark)).unwrap();
        chart.add_layer(None, Box::new(Watermark)).unwrap();
        let run = |chart: &mut Chart, k: usize| {
            // As the host helpers do before every frame.
            chart.set_native_kinds(if k >= 1000 { &["test.bands"] } else { &[] });
            let hi = 110.0 + (k % 5) as f64;
            chart
                .update_custom(id, |b: &mut Bands| {
                    if let Some(last) = b.range.last_mut() {
                        last.1 = hi;
                    }
                    b.index.set_last(100.0, hi);
                    DataChange::UpdateLast
                })
                .unwrap();
            let dx = if k % 40 < 20 { 37.0 } else { -37.0 };
            chart.time_scale().scroll_by(Px(dx));
            chart.build_frame(0.0, &MonoMeasurer);
        };
        for k in 0..60 {
            run(&mut chart, k);
        }
        let ((), n) = count_allocations(|| {
            for k in 0..200 {
                run(&mut chart, k);
            }
        });
        assert_eq!(n, 0);
        // Natively drawn: a `DrawCmd::Custom` instead of the primitives.
        for k in 1000..1060 {
            run(&mut chart, k);
        }
        let ((), n) = count_allocations(|| {
            for k in 1000..1200 {
                run(&mut chart, k);
            }
        });
        assert_eq!(n, 0);
        assert!(
            chart.frame().layers[1]
                .cmds
                .iter()
                .any(|c| matches!(c, ferrochart_core::frame::DrawCmd::Custom(_)))
        );
    }

    /// Transform series: ticks on the last input that keep the
    /// number of forming elements, panning, and the Kagi / P&F primitives
    /// at normal and pixel-merged spacing.
    #[test]
    fn transform_series_do_not_allocate() {
        use ferrochart_core::plugin::SeriesTransform;
        use ferrochart_core::transform::{Kagi, PointFigure, RangeBars, Renko, Reversal};

        let all: [(&str, Box<dyn SeriesTransform>); 4] = [
            ("renko", Box::new(Renko::fixed(0.5).unwrap())),
            ("range", Box::new(RangeBars::new(0.5).unwrap())),
            ("kagi", Box::new(Kagi::new(Reversal::Fixed(1.0)).unwrap())),
            ("pf", Box::new(PointFigure::new(0.5, 3).unwrap())),
        ];
        for (name, t) in all {
            let mut chart = Chart::new(ChartOptions {
                min_bar_spacing: ferrochart_core::geom::Css(0.05),
                ..ChartOptions::default()
            });
            chart.resize(
                SizeCss {
                    w: 1200.0,
                    h: 600.0,
                },
                2.0,
            );
            let pane = chart.main_pane();
            let id = chart
                .add_transform_series(pane, t, SeriesOptions::default())
                .unwrap();
            chart
                .add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(id))
                .unwrap();
            let data = random_walk(5, 200_000, START, 60_000).unwrap();
            let p = data.time.len() - 1;
            let last = BarUpdate {
                time: data.time[p],
                open: data.open[p],
                high: data.high[p] + 0.01,
                low: data.low[p] - 0.01,
                close: data.close[p],
                volume: Some(1.0),
            };
            chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
            // The wider last bar may change the output once.
            chart.update_last(id, last).unwrap();
            chart.fit_content();
            for spacing in [6.0, 0.3] {
                chart.time_scale().set_bar_spacing(spacing);
                let mut run = |k: usize| {
                    let mut bar = last;
                    // Moves far smaller than a box: the tail keeps its length.
                    bar.close += 1e-6 * (k % 5) as f64;
                    bar.volume = Some(1.0 + (k % 3) as f64);
                    let change = chart.update_last(id, bar).unwrap();
                    assert!(
                        matches!(
                            change,
                            DataChange::UpdateLast | DataChange::Append { count: 0 }
                        ),
                        "{name}: {change:?}"
                    );
                    let dx = if k % 40 < 20 { 37.0 } else { -37.0 };
                    chart.time_scale().scroll_by(Px(dx));
                    chart.build_frame(0.0, &MonoMeasurer);
                };
                for k in 0..60 {
                    run(k);
                }
                let ((), n) = count_allocations(|| {
                    for k in 0..200 {
                        run(k);
                    }
                });
                assert_eq!(n, 0, "{name} at {spacing} px: {n} allocations");
            }
        }
    }
}
