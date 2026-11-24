//! Criterion micro-benchmarks of plots:
//! `set_data` of 1M points and core CPU per frame while panning, zooming
//! and hovering 200k scatter points, a 1M point sorted line and 10k heatmap
//! cells, plus stacked bars and `update_last`. Targets: frames < 2 ms,
//! `set_data` of 1M < 50 ms.
//!
//! Run: `cargo bench -p ferrochart-plot`. Compare against a baseline with
//! `--save-baseline before` / `--baseline before`.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ferrochart_core::geom::{Css, PointCss, SizeCss};
use ferrochart_core::interaction::{Button, InputEvent, Modifiers, WheelDelta};
use ferrochart_core::testing::MonoMeasurer;
use ferrochart_plot::{
    AxisId, AxisOptions, MarkData, MarkId, MarkKind, MarkOptions, MarkPoint, Plot, PlotOptions,
};

const W: f32 = 1600.0;
const H: f32 = 900.0;

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

fn plot() -> Plot {
    let mut p = Plot::new(PlotOptions {
        crosshair: true,
        ..PlotOptions::default()
    });
    p.resize(SizeCss { w: W, h: H }, 2.0);
    p
}

fn scatter_data(n: usize) -> MarkData {
    let (x, y, v) = (noise(n, 1), noise(n, 2), noise(n, 3));
    let mut d = MarkData::xy(
        x.iter().map(|a| a * 100.0).collect(),
        y.iter().map(|a| a * 50.0).collect(),
    );
    d.value = Some(v);
    d
}

fn line_data(n: usize) -> MarkData {
    let mut acc = 0.0;
    let y = noise(n, 4)
        .into_iter()
        .map(|r| {
            acc += r - 0.5;
            acc
        })
        .collect();
    MarkData::xy((0..n).map(|i| i as f64).collect(), y)
}

fn with_mark(kind: MarkKind, opts: MarkOptions) -> (Plot, MarkId) {
    let mut p = plot();
    let m = p.add_mark(kind, opts).unwrap();
    (p, m)
}

fn scatter() -> Plot {
    let (mut p, m) = with_mark(
        MarkKind::Scatter,
        MarkOptions {
            point_size: Css(3.0),
            ..MarkOptions::default()
        },
    );
    p.set_data(m, scatter_data(200_000)).unwrap();
    p
}

fn line() -> Plot {
    let (mut p, m) = with_mark(MarkKind::Line, MarkOptions::default());
    p.set_data(m, line_data(1_000_000)).unwrap();
    p
}

fn cells() -> Plot {
    let mut p = plot();
    p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
    p.set_axis(AxisId::Right, AxisOptions::category()).unwrap();
    let m = p
        .add_mark(
            MarkKind::Cells,
            MarkOptions {
                show_values: true,
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

/// Four stacked bar marks over 2 000 categories.
fn bars() -> Plot {
    let mut p = plot();
    p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
    for k in 0..4 {
        let m = p
            .add_mark(
                MarkKind::Bars,
                MarkOptions {
                    stack: Some(1),
                    ..MarkOptions::default()
                },
            )
            .unwrap();
        let y = noise(2_000, 10 + k)
            .into_iter()
            .map(|r| r * 20.0 - 5.0)
            .collect();
        p.set_data(m, MarkData::xy((0..2_000).map(f64::from).collect(), y))
            .unwrap();
    }
    p
}

const MODS: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
    meta: false,
};

fn center() -> PointCss {
    PointCss {
        x: W * 0.45,
        y: H * 0.5,
    }
}

/// Frames while dragging back and forth.
fn bench_pan(
    g: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    name: &str,
    mut p: Plot,
) {
    p.build_frame(0.0, &MonoMeasurer);
    let c = center();
    p.handle_input(InputEvent::PointerDown {
        id: 0,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos: c,
        button: Button::Left,
        mods: MODS,
        time_ms: 0.0,
    });
    let mut i = 0_u32;
    g.bench_function(format!("pan_{name}"), |b| {
        b.iter(|| {
            i += 1;
            let phase = (i % 80) as f32;
            let dx = if phase < 40.0 { phase } else { 80.0 - phase } * 10.0 - 200.0;
            let t = f64::from(i);
            p.handle_input(InputEvent::PointerMove {
                id: 0,
                kind: ferrochart_core::interaction::PointerKind::Mouse,
                pos: PointCss {
                    x: c.x + dx,
                    y: c.y + dx * 0.1,
                },
                mods: MODS,
                time_ms: t,
            });
            p.build_frame(t, &MonoMeasurer);
            p.drain_events().for_each(drop);
            black_box(p.frame().layers[1].cmds.len())
        });
    });
}

/// Frames while zooming 30 wheel notches in and 30 out.
fn bench_zoom(
    g: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    name: &str,
    mut p: Plot,
) {
    p.build_frame(0.0, &MonoMeasurer);
    let mut i = 0_u32;
    g.bench_function(format!("zoom_{name}"), |b| {
        b.iter(|| {
            i = (i + 1) % 60;
            p.handle_input(InputEvent::Wheel {
                pos: center(),
                delta: WheelDelta::Pixels {
                    x: Css(0.0),
                    y: Css(if i < 30 { -60.0 } else { 60.0 }),
                },
                mods: MODS,
            });
            p.build_frame(0.0, &MonoMeasurer);
            p.drain_events().for_each(drop);
            black_box(p.frame().layers[1].cmds.len())
        });
    });
}

/// Frames while the pointer sweeps across the plot (hit test, highlight,
/// crosshair).
fn bench_hover(
    g: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    name: &str,
    mut p: Plot,
) {
    p.build_frame(0.0, &MonoMeasurer);
    let r = p.layout().plot;
    let mut i = 0_u32;
    g.bench_function(format!("hover_{name}"), |b| {
        b.iter(|| {
            i = (i + 1) % 400;
            let u = i as f32 / 400.0;
            p.handle_input(InputEvent::PointerMove {
                id: 0,
                kind: ferrochart_core::interaction::PointerKind::Mouse,
                pos: PointCss {
                    x: r.x + r.w * u,
                    y: r.y + r.h * (0.5 + 0.4 * (u * 20.0).sin()),
                },
                mods: MODS,
                time_ms: f64::from(i),
            });
            p.build_frame(f64::from(i), &MonoMeasurer);
            p.drain_events().for_each(drop);
            black_box(p.frame().layers[2].cmds.len())
        });
    });
}

fn build_frame(c: &mut Criterion) {
    let mut g = c.benchmark_group("plot_frame");
    for (name, make) in [
        ("scatter_200k", scatter as fn() -> Plot),
        ("line_1M", line),
        ("cells_10k", cells),
        ("stacked_bars_4x2k", bars),
    ] {
        bench_pan(&mut g, name, make());
        bench_zoom(&mut g, name, make());
        bench_hover(&mut g, name, make());
    }
    g.finish();
}

fn set_data(c: &mut Criterion) {
    let mut g = c.benchmark_group("plot_set_data");
    g.sample_size(10);
    for (name, kind, data) in [
        ("line_1M", MarkKind::Line, line_data(1_000_000)),
        ("scatter_1M", MarkKind::Scatter, scatter_data(1_000_000)),
    ] {
        g.bench_function(name, |b| {
            b.iter_batched(
                || (with_mark(kind, MarkOptions::default()), data.clone()),
                |((mut p, m), d)| {
                    p.set_data(m, d).unwrap();
                    p
                },
                BatchSize::PerIteration,
            );
        });
    }
    g.finish();
}

fn update_last(c: &mut Criterion) {
    let mut g = c.benchmark_group("plot_update_last");
    for (name, mut p) in [("line_1M", line()), ("scatter_200k", scatter())] {
        let id = p.marks().next().unwrap();
        let n = p.mark_data(id).unwrap().len();
        p.build_frame(0.0, &MonoMeasurer);
        let mut k = 0_u32;
        g.bench_function(format!("{name}_and_frame"), |b| {
            b.iter(|| {
                k = k.wrapping_add(1);
                let x = if name == "line_1M" {
                    n as f64 - 1.0
                } else {
                    50.0
                };
                p.update_last(id, MarkPoint::xy(x, f64::from(k % 7)))
                    .unwrap();
                p.build_frame(0.0, &MonoMeasurer);
                black_box(p.frame().layers[1].cmds.len())
            });
        });
    }
    g.finish();
}

criterion_group!(benches, set_data, build_frame, update_last);
criterion_main!(benches);
