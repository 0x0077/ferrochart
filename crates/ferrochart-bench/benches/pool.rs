//! A watchlist of sparklines sharing one GPU device: 100 charts of 200×60
//! CSS px at 2×, each a 390-minute line with the sparkline look, every
//! renderer built on one `SharedResources` (pipelines and glyph atlases). Each frame: 10 charts
//! (a different 10 each frame) get an `update_last` → `build_frame` +
//! `render` of the charts that need it → wait for the GPU. Presenting is
//! not included. A baseline: p50/p95 are recorded, there is no target.
//!
//!
//! Run: `cargo bench -p ferrochart-bench --bench pool`. Needs a GPU adapter;
//! without one it prints a notice and exits successfully.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use ferrochart_core::appearance::{Appearance, CrosshairStyle, DotStyle, InteractionOptions};
use ferrochart_core::data::{BarUpdate, SeriesId};
use ferrochart_core::frame::{Invalidation, SeriesKind};
use ferrochart_core::geom::SizeCss;
use ferrochart_core::render::Renderer;
use ferrochart_core::testing::quote_card_scene;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_wgpu::{GpuContext, SharedResources, WgpuRenderer};

const CHARTS: usize = 100;
const UPDATES_PER_FRAME: usize = 10;
const FRAMES: usize = 1000;
const WARMUP: usize = 60;
const SIZE: SizeCss = SizeCss { w: 200.0, h: 60.0 };
const DPR: f32 = 2.0;
const START: i64 = 1_700_000_000_000;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn sparkline() -> (Chart, SeriesId) {
    let mut chart = Chart::new(ChartOptions {
        appearance: Appearance::sparkline(),
        interaction: InteractionOptions::none(),
        crosshair_style: CrosshairStyle {
            horizontal: false,
            labels: false,
            dot: Some(DotStyle::default()),
            ..CrosshairStyle::default()
        },
        ..ChartOptions::default()
    });
    chart.resize(SIZE, DPR);
    let id = quote_card_scene(&mut chart, START, SeriesKind::Line).unwrap();
    (chart, id)
}

fn main() {
    let ctx = match GpuContext::headless() {
        Ok(c) => c,
        Err(e) => {
            println!("no GPU adapter ({e}); pool skipped");
            return;
        }
    };
    let info = ctx.adapter.get_info();
    println!(
        "adapter: {} ({:?}); {CHARTS} sparklines of {}×{} css @ {DPR}x on one device; \
         {UPDATES_PER_FRAME} update_last per frame; {FRAMES} frames after {WARMUP} warm-up",
        info.name, info.backend, SIZE.w, SIZE.h
    );
    let shared = SharedResources::offscreen(&ctx);
    let mut charts: Vec<(Chart, SeriesId)> = (0..CHARTS).map(|_| sparkline()).collect();
    let mut renderers: Vec<WgpuRenderer> = (0..CHARTS)
        .map(|_| WgpuRenderer::offscreen_shared(&ctx, &shared))
        .collect();
    let last = START + 390 * 60_000;

    let clock = Instant::now();
    let mut times = Vec::with_capacity(FRAMES);
    let mut drawn = 0usize;
    for i in 0..WARMUP + FRAMES {
        let start = Instant::now();
        let now = clock.elapsed().as_secs_f64() * 1000.0;
        for k in 0..UPDATES_PER_FRAME {
            let (chart, id) = &mut charts[(i * UPDATES_PER_FRAME + k) % CHARTS];
            let v = 100.0 + ((i + k) % 17) as f64 * 0.1;
            chart
                .update_last(
                    *id,
                    BarUpdate {
                        time: last,
                        open: v,
                        high: v,
                        low: v,
                        close: v,
                        volume: None,
                    },
                )
                .unwrap();
        }
        for ((chart, _), renderer) in charts.iter_mut().zip(&mut renderers) {
            if chart.needs_redraw() == Invalidation::None {
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
    println!(
        "100 pooled sparklines, 10 updates/frame  p50 {p50:6.3} ms  p95 {p95:6.3} ms  p99 {p99:6.3} ms  max {max:6.3} ms"
    );
    println!(
        "charts redrawn per frame: {:.2}",
        drawn as f64 / FRAMES as f64
    );
}
