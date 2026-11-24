//! P8 end-to-end plot frame time: 200 000
//! scatter points, a 1 000 000 point sorted line and 10 000 heatmap cells,
//! panned, zoomed and hovered on the offscreen wgpu backend. Target: steady
//! 60 fps, i.e. p99 < 16.7 ms. Also reports `set_data` of 1M points
//! (target < 50 ms).
//!
//! Each frame: `handle_input` (a drag step, a wheel notch or a pointer move)
//! → `build_frame` → `render` → wait for the GPU, as in `frame_time`.
//!
//! Run: `cargo bench -p ferrochart-bench --bench plot`. Needs a GPU adapter;
//! without one it prints a notice and exits successfully.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::geom::{Css, PointCss, SizeCss};
use ferrochart_core::interaction::{Button, InputEvent, Modifiers, WheelDelta};
use ferrochart_core::render::Renderer;
use ferrochart_plot::{AxisId, AxisOptions, MarkData, MarkKind, MarkOptions, Plot, PlotOptions};
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

const FRAMES: usize = 600;
const WARMUP: usize = 60;
const TARGET_MS: f64 = 16.7;
const SET_DATA_TARGET_MS: f64 = 50.0;
const SIZE: SizeCss = SizeCss {
    w: 1600.0,
    h: 900.0,
};
const DPR: f32 = 2.0;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
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

fn plot() -> Plot {
    let mut p = Plot::new(PlotOptions {
        crosshair: true,
        ..PlotOptions::default()
    });
    p.resize(SIZE, DPR);
    p
}

fn scatter() -> Plot {
    let mut p = plot();
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
    let (x, y) = (noise(n, 1), noise(n, 2));
    let mut d = MarkData::xy(
        x.iter().map(|a| a * 100.0).collect(),
        y.iter().map(|a| a * 50.0).collect(),
    );
    d.value = Some(noise(n, 3));
    p.set_data(m, d).unwrap();
    p
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

/// The line, and how long `set_data` took (ms).
fn line() -> (Plot, f64) {
    let mut p = plot();
    let m = p.add_mark(MarkKind::Line, MarkOptions::default()).unwrap();
    let d = line_data(1_000_000);
    let t0 = Instant::now();
    p.set_data(m, d).unwrap();
    (p, t0.elapsed().as_secs_f64() * 1000.0)
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

fn measure(
    ctx: &GpuContext,
    p: &mut Plot,
    r: &mut WgpuRenderer,
    mut script: impl FnMut(&mut Plot, usize, f64),
) -> Vec<f64> {
    let clock = Instant::now();
    let empty = ChartDataView::empty();
    let mut times = Vec::with_capacity(FRAMES);
    for i in 0..WARMUP + FRAMES {
        let start = Instant::now();
        let now = clock.elapsed().as_secs_f64() * 1000.0;
        script(p, i, now);
        p.build_frame(now, &r.measurer());
        p.drain_events().for_each(drop);
        r.render(p.frame(), &empty).unwrap();
        ctx.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU wait");
        if i >= WARMUP {
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    assert!(r.take_gpu_error().is_none());
    times.sort_by(f64::total_cmp);
    times
}

fn report(name: &str, times: &[f64]) -> bool {
    let (p50, p95, p99) = (
        percentile(times, 0.50),
        percentile(times, 0.95),
        percentile(times, 0.99),
    );
    let max = times.last().copied().unwrap_or(0.0);
    let pass = p99 < TARGET_MS;
    println!(
        "{name:<20} p50 {p50:6.3} ms  p95 {p95:6.3} ms  p99 {p99:6.3} ms  max {max:6.3} ms  {}",
        if pass { "PASS" } else { "FAIL" }
    );
    pass
}

fn main() {
    let ctx = match GpuContext::headless() {
        Ok(c) => c,
        Err(e) => {
            println!("no GPU adapter ({e}); plot skipped");
            return;
        }
    };
    let info = ctx.adapter.get_info();
    println!(
        "adapter: {} ({:?}); {}×{} css @ {DPR}x; {FRAMES} frames after {WARMUP} warm-up",
        info.name, info.backend, SIZE.w, SIZE.h
    );
    let (line, set_ms) = line();
    let set_ok = set_ms < SET_DATA_TARGET_MS;
    println!(
        "set_data, line 1M    {set_ms:.1} ms  {}",
        if set_ok { "PASS" } else { "FAIL" }
    );
    let mods = Modifiers::default();
    let center = PointCss {
        x: SIZE.w * 0.45,
        y: SIZE.h * 0.5,
    };
    let mut ok = set_ok;
    for (name, mut p) in [
        ("scatter 200k", scatter()),
        ("line 1M", line),
        ("cells 10k", cells()),
    ] {
        let mut r = WgpuRenderer::offscreen(&ctx);
        p.build_frame(0.0, &r.measurer());
        // Drag back and forth.
        p.handle_input(InputEvent::PointerDown {
            id: 0,
            kind: ferrochart_core::interaction::PointerKind::Mouse,
            pos: center,
            button: Button::Left,
            mods,
            time_ms: 0.0,
        });
        let times = measure(&ctx, &mut p, &mut r, |p, i, now| {
            let phase = (i % 80) as f32;
            let dx = if phase < 40.0 { phase } else { 80.0 - phase } * 10.0 - 200.0;
            p.handle_input(InputEvent::PointerMove {
                id: 0,
                kind: ferrochart_core::interaction::PointerKind::Mouse,
                pos: PointCss {
                    x: center.x + dx,
                    y: center.y + dx * 0.1,
                },
                mods,
                time_ms: now,
            });
        });
        p.handle_input(InputEvent::PointerUp {
            id: 0,
            kind: ferrochart_core::interaction::PointerKind::Mouse,
            pos: center,
            button: Button::Left,
            mods,
            time_ms: 0.0,
        });
        p.fit();
        ok &= report(&format!("pan, {name}"), &times);
        // 30 wheel notches in, 30 out.
        let times = measure(&ctx, &mut p, &mut r, |p, i, _| {
            p.handle_input(InputEvent::Wheel {
                pos: center,
                delta: WheelDelta::Pixels {
                    x: Css(0.0),
                    y: Css(if (i / 30) % 2 == 0 { -60.0 } else { 60.0 }),
                },
                mods,
            });
        });
        p.fit();
        ok &= report(&format!("zoom, {name}"), &times);
        // The pointer sweeps across the plot.
        let plot = p.layout().plot;
        let (x0, y0) = (plot.x / DPR, plot.y / DPR);
        let (w, h) = (plot.w / DPR, plot.h / DPR);
        let times = measure(&ctx, &mut p, &mut r, |p, i, now| {
            let u = (i % 400) as f32 / 400.0;
            p.handle_input(InputEvent::PointerMove {
                id: 0,
                kind: ferrochart_core::interaction::PointerKind::Mouse,
                pos: PointCss {
                    x: x0 + w * u,
                    y: y0 + h * (0.5 + 0.4 * (u * 20.0).sin()),
                },
                mods,
                time_ms: now,
            });
        });
        ok &= report(&format!("hover, {name}"), &times);
    }
    if !ok {
        println!("plot target missed");
    }
}
