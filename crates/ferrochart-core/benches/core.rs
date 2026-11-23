//! Criterion micro-benchmarks of the core:
//! P1 `set_data`, P3 `build_frame` while panning/zooming, P5 `update_last`.
//!
//! Run: `cargo bench -p ferrochart-core`. Compare against a baseline with
//! `--save-baseline before` / `--baseline before`.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ferrochart_core::chart::{Marker, MarkerPosition, MarkerShape, SeriesData, SeriesOptions};
use ferrochart_core::data::{BarUpdate, OhlcColumns, SeriesId};
use ferrochart_core::drawing::{Anchor, BUILTIN_KINDS, DrawingData};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::{Px, SizeCss};
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions};

const START: i64 = 1_704_067_200_000;
const INTERVAL: i64 = 60_000;

fn empty_chart() -> (Chart, SeriesId) {
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
    (chart, id)
}

fn loaded_chart(data: &OhlcColumns) -> (Chart, SeriesId) {
    let (mut chart, id) = empty_chart();
    chart.set_data(id, SeriesData::Ohlc(data.clone())).unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    (chart, id)
}

fn set_data(c: &mut Criterion) {
    let mut g = c.benchmark_group("set_data");
    g.sample_size(10);
    for (name, n) in [("1M", 1_000_000), ("10M", 10_000_000)] {
        let data = random_walk(1, n, START, INTERVAL).unwrap();
        g.bench_function(name, |b| {
            b.iter_batched(
                || (empty_chart(), data.clone()),
                |((mut chart, id), d)| {
                    chart.set_data(id, SeriesData::Ohlc(d)).unwrap();
                    chart
                },
                BatchSize::PerIteration,
            );
        });
    }
    g.finish();
}

fn build_frame(c: &mut Criterion) {
    let mut g = c.benchmark_group("build_frame");
    for (name, n) in [("1M", 1_000_000), ("10M", 10_000_000)] {
        let data = random_walk(2, n, START, INTERVAL).unwrap();
        let (mut chart, _) = loaded_chart(&data);
        let mut dir = 1.0_f32;
        let mut moved = 0;
        g.bench_function(format!("pan_{name}"), |b| {
            b.iter(|| {
                // Pan back and forth over ~2000 bars so new labels keep appearing.
                moved += 1;
                if moved % 400 == 0 {
                    dir = -dir;
                }
                chart.time_scale().scroll_by(Px(dir * 61.0));
                chart.build_frame(0.0, &MonoMeasurer);
                black_box(chart.frame().layers[1].cmds.len())
            });
        });
        let (mut chart, _) = loaded_chart(&data);
        chart.fit_content();
        chart.build_frame(0.0, &MonoMeasurer);
        let mut step = 0_u32;
        g.bench_function(format!("zoom_{name}"), |b| {
            b.iter(|| {
                // 30 steps in, 30 out: spans level 0 up to the top LOD levels.
                step = (step + 1) % 60;
                let f = if step < 30 { 1.3 } else { 1.0 / 1.3 };
                chart.time_scale().zoom(Px(1500.0), f);
                chart.build_frame(0.0, &MonoMeasurer);
                black_box(chart.frame().layers[1].cmds.len())
            });
        });
    }
    // The same pan with every built-in drawing tool in view.
    let data = random_walk(2, 1_000_000, START, INTERVAL).unwrap();
    let (mut chart, _) = loaded_chart(&data);
    let pane = chart.main_pane();
    let last = data.time.len() - 1;
    for (k, kind) in BUILTIN_KINDS.iter().enumerate() {
        // Anchor counts as in tests/zero_alloc.rs.
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
            "brush" => 500,
            "path" => 12,
            _ => 2,
        };
        let at = last - 3000 + 80 * k;
        let anchors = (0..n)
            .map(|i| Anchor {
                time: data.time[at + if n > 20 { i / 10 } else { 30 * i }],
                price: data.close[at] * (1.0 + 0.01 * (i % 3) as f64),
            })
            .collect();
        let id = chart
            .drawings()
            .add(pane, &DrawingData::new(kind, anchors))
            .unwrap();
        if k == 6 {
            chart.drawings().select(Some(id)).unwrap();
        }
    }
    chart.build_frame(0.0, &MonoMeasurer);
    let (mut dir, mut moved) = (1.0_f32, 0);
    g.bench_function("pan_1M_drawings", |b| {
        b.iter(|| {
            moved += 1;
            if moved % 400 == 0 {
                dir = -dir;
            }
            chart.time_scale().scroll_by(Px(dir * 61.0));
            chart.build_frame(0.0, &MonoMeasurer);
            black_box(chart.frame().layers[1].cmds.len())
        });
    });
    // The same pan over 4000 bars with markers: one, two or three per bar,
    // every fifth with text.
    let (mut chart, id) = loaded_chart(&data);
    let times = &data.time;
    let markers = (last - 4000..=last)
        .step_by(2)
        .flat_map(|i| {
            let n = 1 + i % 3;
            (0..n).map(move |k| Marker {
                time: times[i],
                position: if k % 2 == 0 {
                    MarkerPosition::AboveBar
                } else {
                    MarkerPosition::BelowBar
                },
                shape: MarkerShape::Circle,
                text: if i % 5 == 0 {
                    "B 1.2M".into()
                } else {
                    String::new()
                },
                ..Marker::default()
            })
        })
        .collect();
    chart.set_markers(id, markers).unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    let (mut dir, mut moved) = (1.0_f32, 0);
    g.bench_function("pan_1M_markers", |b| {
        b.iter(|| {
            moved += 1;
            if moved % 400 == 0 {
                dir = -dir;
            }
            chart.time_scale().scroll_by(Px(dir * 61.0));
            chart.build_frame(0.0, &MonoMeasurer);
            black_box(chart.frame().layers[1].cmds.len())
        });
    });
    g.finish();
}

fn update_last(c: &mut Criterion) {
    let mut g = c.benchmark_group("update_last");
    for (name, n) in [("1M", 1_000_000), ("10M", 10_000_000)] {
        let data = random_walk(3, n, START, INTERVAL).unwrap();
        let last_time = *data.time.last().unwrap();
        let (mut chart, id) = loaded_chart(&data);
        let mut i = 0_u32;
        g.bench_function(name, |b| {
            b.iter(|| {
                i = i.wrapping_add(1);
                let close = 100.0 + f64::from(i % 17) * 0.1;
                chart
                    .update_last(
                        id,
                        BarUpdate {
                            time: last_time,
                            open: 100.0,
                            high: 103.0,
                            low: 98.0,
                            close,
                            volume: Some(1000.0 + f64::from(i % 100)),
                        },
                    )
                    .unwrap()
            });
        });
    }
    g.finish();
}

criterion_group!(benches, set_data, build_frame, update_last);
criterion_main!(benches);
