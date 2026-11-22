//! P4 end-to-end frame time: one pan or zoom
//! frame of a 10M-bar chart, from the input event to the GPU finishing the
//! frame, on the offscreen wgpu backend. Target: < 8 ms.
//!
//! Each frame: `handle_input` (a drag step or a wheel notch) → `build_frame`
//! → `render` → wait for the GPU (`device.poll(wait)`), so GPU time is
//! included. Presenting to a window (vsync, compositor) is not. 1000 scripted
//! frames per scenario after a warm-up; reports p50/p95/p99/max and PASS when
//! p99 < 8 ms.
//!
//! Run: `cargo bench -p ferrochart-bench --bench frame_time`. Needs a GPU
//! adapter (software adapters work but are slow); without one it prints a
//! notice and exits successfully.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::{Css, PointCss, SizeCss};
use ferrochart_core::interaction::{Button, CrosshairMode, InputEvent, Modifiers, WheelDelta};
use ferrochart_core::render::Renderer;
use ferrochart_core::testing::random_walk;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

const BARS: usize = 10_000_000;
const FRAMES: usize = 1000;
const WARMUP: usize = 60;
const TARGET_MS: f64 = 8.0;
/// A 1600×900 window on a 2× display: 3200×1800 device pixels.
const SIZE: SizeCss = SizeCss {
    w: 1600.0,
    h: 900.0,
};
const DPR: f32 = 2.0;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn chart() -> Chart {
    let mut chart = Chart::new(ChartOptions {
        theme: Theme::dark(),
        ..ChartOptions::default()
    });
    chart.resize(SIZE, DPR);
    let pane = chart.main_pane();
    let id = chart
        .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    chart
        .add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(id))
        .unwrap();
    let data = random_walk(2026, BARS, 1_104_537_600_000, 60_000).unwrap();
    chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
    chart.set_crosshair_mode(CrosshairMode::Magnet);
    chart
}

/// Runs `script(frame)` → frame for `WARMUP + FRAMES` frames and returns the
/// sorted frame times of the measured ones, in ms.
fn measure(
    ctx: &GpuContext,
    chart: &mut Chart,
    renderer: &mut WgpuRenderer,
    mut script: impl FnMut(&mut Chart, usize, f64),
) -> Vec<f64> {
    let clock = Instant::now();
    let mut times = Vec::with_capacity(FRAMES);
    for i in 0..WARMUP + FRAMES {
        let start = Instant::now();
        let now = clock.elapsed().as_secs_f64() * 1000.0;
        script(chart, i, now);
        chart.build_frame(now, &renderer.measurer());
        renderer.render(chart.frame(), &chart.data_view()).unwrap();
        ctx.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU wait");
        if i >= WARMUP {
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
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
        "{name:<28} p50 {p50:6.3} ms  p95 {p95:6.3} ms  p99 {p99:6.3} ms  max {max:6.3} ms  {}",
        if pass { "PASS" } else { "FAIL" }
    );
    pass
}

fn main() {
    let ctx = match GpuContext::headless() {
        Ok(c) => c,
        Err(e) => {
            println!("no GPU adapter ({e}); frame_time skipped");
            return;
        }
    };
    let info = ctx.adapter.get_info();
    println!(
        "adapter: {} ({:?}); {BARS} bars; {}×{} css @ {DPR}x; {FRAMES} frames after {WARMUP} warm-up",
        info.name, info.backend, SIZE.w, SIZE.h
    );
    let mods = Modifiers::default();
    let center = PointCss {
        x: SIZE.w * 0.5,
        y: SIZE.h * 0.5,
    };
    let mut ok = true;

    // Pan: one drag that sweeps 400 css px left and right; the crosshair
    // follows the pointer, as in real use.
    {
        let mut chart = chart();
        let mut renderer = WgpuRenderer::offscreen(&ctx);
        chart.handle_input(InputEvent::PointerMove {
            id: 0,
            pos: center,
            mods,
            time_ms: 0.0,
        });
        chart.handle_input(InputEvent::PointerDown {
            id: 0,
            pos: center,
            button: Button::Left,
            mods,
            time_ms: 0.0,
        });
        let times = measure(&ctx, &mut chart, &mut renderer, |chart, i, now| {
            let phase = (i % 80) as f32;
            let dx = if phase < 40.0 { phase } else { 80.0 - phase } * 10.0 - 200.0;
            chart.handle_input(InputEvent::PointerMove {
                id: 0,
                pos: PointCss {
                    x: center.x + dx,
                    y: center.y,
                },
                mods,
                time_ms: now,
            });
        });
        ok &= report("pan (default zoom)", &times);
    }

    // Zoom: wheel notches out to ~all 10M bars (LOD levels) and back in.
    {
        let mut chart = chart();
        let mut renderer = WgpuRenderer::offscreen(&ctx);
        let times = measure(&ctx, &mut chart, &mut renderer, |chart, i, _| {
            let out = (i / 100) % 2 == 0;
            chart.handle_input(InputEvent::Wheel {
                pos: center,
                delta: WheelDelta::Pixels {
                    x: Css(0.0),
                    y: Css(if out { 60.0 } else { -60.0 }),
                },
                mods,
            });
        });
        ok &= report("zoom (wheel, 1 → 10M bars)", &times);
    }

    // Pan with everything visible: the densest LOD frame.
    {
        let mut chart = chart();
        chart.fit_content();
        let mut renderer = WgpuRenderer::offscreen(&ctx);
        let times = measure(&ctx, &mut chart, &mut renderer, |chart, i, _| {
            chart
                .time_scale()
                .scroll_by(ferrochart_core::geom::Px(if (i / 40) % 2 == 0 {
                    8.0
                } else {
                    -8.0
                }));
        });
        ok &= report("pan (fit content, 10M)", &times);
    }

    if !ok {
        println!("P4 target ({TARGET_MS} ms at p99) missed");
    }
}
