//! P6 synced charts: 16 charts of 1M
//! bars each in one `SyncGroup`, the crosshair moving over one of them. Target:
//! a stable 60 fps, i.e. p99 of the whole frame < 16.7 ms.
//!
//! Each frame: one `PointerMove` on the hovered chart (the pointer visits a
//! different chart every 50 frames, and the chart under it pans every few
//! frames, which the others follow) → `build_frame` + `render` of all 16
//! charts → wait for the GPU. The charts form a 4 × 4 grid of a 1600×900 css
//! window at 2× (400×225 css each), each with its own offscreen renderer on
//! one shared device. Presenting is not included.
//!
//! Run: `cargo bench -p ferrochart-bench --bench sync`. Needs a GPU adapter;
//! without one it prints a notice and exits successfully.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::{PointCss, Px, SizeCss};
use ferrochart_core::interaction::{CrosshairMode, InputEvent, Modifiers};
use ferrochart_core::render::Renderer;
use ferrochart_core::sync::{SyncGroup, SyncOptions};
use ferrochart_core::testing::random_walk;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

const CHARTS: usize = 16;
const BARS: usize = 1_000_000;
const FRAMES: usize = 1000;
const WARMUP: usize = 60;
const TARGET_MS: f64 = 1000.0 / 60.0;
const SIZE: SizeCss = SizeCss { w: 400.0, h: 225.0 };
const DPR: f32 = 2.0;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn chart(seed: u64, group: &SyncGroup) -> Chart {
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
    let data = random_walk(seed, BARS, 1_104_537_600_000, 60_000).unwrap();
    chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
    chart.set_crosshair_mode(CrosshairMode::Magnet);
    chart.join_sync(group, SyncOptions::default());
    chart
}

fn main() {
    let ctx = match GpuContext::headless() {
        Ok(c) => c,
        Err(e) => {
            println!("no GPU adapter ({e}); sync skipped");
            return;
        }
    };
    let info = ctx.adapter.get_info();
    println!(
        "adapter: {} ({:?}); {CHARTS} charts × {BARS} bars; {}×{} css @ {DPR}x each; {FRAMES} frames after {WARMUP} warm-up",
        info.name, info.backend, SIZE.w, SIZE.h
    );
    let group = SyncGroup::new();
    let mut charts: Vec<Chart> = (0..CHARTS).map(|k| chart(k as u64 + 1, &group)).collect();
    let mut renderers: Vec<WgpuRenderer> =
        (0..CHARTS).map(|_| WgpuRenderer::offscreen(&ctx)).collect();
    let mods = Modifiers::default();

    let clock = Instant::now();
    let mut times = Vec::with_capacity(FRAMES);
    let mut drawn = 0usize;
    for i in 0..WARMUP + FRAMES {
        let start = Instant::now();
        let now = clock.elapsed().as_secs_f64() * 1000.0;
        let hovered = (i / 50) % CHARTS;
        let phase = (i % 50) as f32;
        let src = &mut charts[hovered];
        src.handle_input(InputEvent::PointerMove {
            id: 0,
            kind: ferrochart_core::interaction::PointerKind::Mouse,
            pos: PointCss {
                x: 20.0 + phase * 6.0,
                y: SIZE.h * 0.4,
            },
            mods,
            time_ms: now,
        });
        if i % 5 == 0 {
            let dx = if (i / 25) % 2 == 0 { 12.0 } else { -12.0 };
            src.time_scale().scroll_by(Px(dx));
        }
        for (chart, renderer) in charts.iter_mut().zip(&mut renderers) {
            if chart.needs_redraw() == ferrochart_core::frame::Invalidation::None {
                continue;
            }
            chart.build_frame(now, &renderer.measurer());
            renderer.render(chart.frame(), &chart.data_view()).unwrap();
            if i >= WARMUP {
                drawn += 1;
            }
        }
        ctx.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU wait");
        if i >= WARMUP {
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    times.sort_by(f64::total_cmp);
    let (p50, p95, p99) = (
        percentile(&times, 0.50),
        percentile(&times, 0.95),
        percentile(&times, 0.99),
    );
    let max = times.last().copied().unwrap_or(0.0);
    let pass = p99 < TARGET_MS;
    println!(
        "16 synced charts, crosshair  p50 {p50:6.3} ms  p95 {p95:6.3} ms  p99 {p99:6.3} ms  max {max:6.3} ms  {}",
        if pass { "PASS" } else { "FAIL" }
    );
    println!(
        "charts redrawn per frame: {:.2}",
        drawn as f64 / FRAMES as f64
    );
    if !pass {
        println!("P6 target ({TARGET_MS:.1} ms at p99) missed");
    }
}
