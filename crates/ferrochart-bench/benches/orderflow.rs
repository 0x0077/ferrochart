//! Order flow frame time: 1M trades
//! as a 1 min footprint and a 1 s trade heatmap over 1 min candles, and as a
//! TPO profile, panned and zoomed on the offscreen wgpu backend. Target:
//! 60 fps, i.e. p99 < 16.7 ms.
//!
//! Each frame: input (drag step, wheel notch or scroll) → `build_frame` →
//! `render` → wait for the GPU, as in `frame_time`. The heatmap uses its
//! native path (compute shader) when the device supports it; the portable
//! path is also measured, for information (it is what WebGL2 gets and has
//! no target).
//!
//! Run: `cargo bench -p ferrochart-bench --bench orderflow`. Needs a GPU
//! adapter; without one it prints a notice and exits successfully.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::OhlcColumns;
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::{Css, PointCss, Px, SizeCss};
use ferrochart_core::interaction::{Button, InputEvent, Modifiers, WheelDelta};
use ferrochart_core::render::Renderer;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_orderflow::testing::{candles_of, random_trades};
use ferrochart_orderflow::{
    Footprint, FootprintOptions, Heatmap, HeatmapOptions, HeatmapSource, TickColumns, Tpo,
    TpoOptions, append_bars, append_trades,
};
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

const TRADES: usize = 1_000_000;
const FRAMES: usize = 600;
const WARMUP: usize = 60;
const TARGET_MS: f64 = 16.7;
/// 2024-01-01T00:00:00Z.
const START: i64 = 1_704_067_200_000;
const SIZE: SizeCss = SizeCss {
    w: 1600.0,
    h: 900.0,
};
const DPR: f32 = 2.0;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn new_chart(max_spacing: f32) -> Chart {
    let mut chart = Chart::new(ChartOptions {
        theme: Theme::dark(),
        max_bar_spacing: Css(max_spacing),
        ..ChartOptions::default()
    });
    chart.resize(SIZE, DPR);
    chart
}

/// Candles, footprint and heatmap from `trades`.
fn flow_chart(trades: &TickColumns, candles: &OhlcColumns) -> Chart {
    let theme = Theme::dark();
    let mut chart = new_chart(120.0);
    let pane = chart.main_pane();
    let heatmap = Heatmap::new(HeatmapOptions::new(0.25, 1_000, HeatmapSource::Trades)).unwrap();
    let h = chart
        .add_custom_series(pane, Box::new(heatmap), SeriesOptions::default())
        .unwrap();
    append_trades(&mut chart, h, trades).unwrap();
    let c = chart
        .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    chart
        .set_data(c, SeriesData::Ohlc(candles.clone()))
        .unwrap();
    let mut fo = FootprintOptions::new(0.25, 60_000, &theme);
    fo.row_size = 4;
    let f = chart
        .add_custom_series(
            pane,
            Box::new(Footprint::new(fo).unwrap()),
            SeriesOptions::default(),
        )
        .unwrap();
    append_trades(&mut chart, f, trades).unwrap();
    chart
}

fn tpo_chart(candles: &OhlcColumns) -> Chart {
    let theme = Theme::dark();
    let mut chart = new_chart(600.0);
    let mut opts = TpoOptions::new(0.25, &theme);
    opts.row_size = 4;
    let id = chart
        .add_custom_series(
            chart.main_pane(),
            Box::new(Tpo::new(opts).unwrap()),
            SeriesOptions::default(),
        )
        .unwrap();
    append_bars(&mut chart, id, candles).unwrap();
    chart.time_scale().set_bar_spacing(500.0);
    chart.scroll_to_realtime();
    chart
}

fn measure(
    ctx: &GpuContext,
    chart: &mut Chart,
    renderer: &mut WgpuRenderer,
    mut script: impl FnMut(&mut Chart, usize, f64),
) -> Vec<f64> {
    chart.set_native_kinds(renderer.native_kinds());
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
    assert!(renderer.take_gpu_error().is_none());
    times.sort_by(f64::total_cmp);
    times
}

fn report(name: &str, times: &[f64], gated: bool) -> bool {
    let (p50, p95, p99) = (
        percentile(times, 0.50),
        percentile(times, 0.95),
        percentile(times, 0.99),
    );
    let max = times.last().copied().unwrap_or(0.0);
    let pass = p99 < TARGET_MS;
    let verdict = match (gated, pass) {
        (false, _) => "info",
        (true, true) => "PASS",
        (true, false) => "FAIL",
    };
    println!(
        "{name:<34} p50 {p50:6.3} ms  p95 {p95:6.3} ms  p99 {p99:6.3} ms  max {max:6.3} ms  {verdict}"
    );
    pass || !gated
}

/// A renderer with the native heatmap when `native` (and supported).
fn renderer(ctx: &GpuContext, native: bool) -> WgpuRenderer {
    let mut r = WgpuRenderer::offscreen(ctx);
    if native {
        ferrochart_orderflow::register_gpu(ctx, &mut r);
    }
    r
}

fn main() {
    let ctx = match GpuContext::headless() {
        Ok(c) => c,
        Err(e) => {
            println!("no GPU adapter ({e}); orderflow skipped");
            return;
        }
    };
    let info = ctx.adapter.get_info();
    let native = ferrochart_orderflow::supports_native_heatmap(&ctx);
    let trades = random_trades(2026, TRADES, START, 200, 0.25).unwrap();
    let candles = candles_of(&trades, 60_000).unwrap();
    let t0 = Instant::now();
    let mut flow = flow_chart(&trades, &candles);
    let load_flow = t0.elapsed().as_secs_f64() * 1000.0;
    let t0 = Instant::now();
    let mut tpo = tpo_chart(&candles);
    let load_tpo = t0.elapsed().as_secs_f64() * 1000.0;
    println!(
        "adapter: {} ({:?}); {TRADES} trades → {} candles; {}×{} css @ {DPR}x; {FRAMES} frames after {WARMUP} warm-up; native heatmap: {native}",
        info.name,
        info.backend,
        candles.time.len(),
        SIZE.w,
        SIZE.h
    );
    println!("load: footprint + heatmap {load_flow:.0} ms, TPO {load_tpo:.0} ms");
    let mods = Modifiers::default();
    let center = PointCss {
        x: SIZE.w * 0.5,
        y: SIZE.h * 0.5,
    };
    let mut ok = true;

    let pan = |chart: &mut Chart, i: usize, now: f64| {
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
    };
    let press = |chart: &mut Chart| {
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
    };
    let release = |chart: &mut Chart| {
        chart.handle_input(InputEvent::PointerUp {
            id: 0,
            pos: center,
            button: Button::Left,
            mods,
            time_ms: 0.0,
        });
    };

    for native_heatmap in [true, false] {
        if native_heatmap && !native {
            println!("(no compute shaders: native heatmap not measured)");
            continue;
        }
        let path = if native_heatmap { "native" } else { "portable" };
        let gated = native_heatmap || !native;
        let mut r = renderer(&ctx, native_heatmap);

        // Footprint numbers (70 px per bar) with the heatmap under them.
        flow.time_scale().set_bar_spacing(70.0);
        flow.scroll_to_realtime();
        press(&mut flow);
        let times = measure(&ctx, &mut flow, &mut r, pan);
        release(&mut flow);
        ok &= report(&format!("pan, footprint numbers ({path})"), &times, gated);

        // Footprint cells (20 px per bar).
        flow.time_scale().set_bar_spacing(20.0);
        flow.scroll_to_realtime();
        press(&mut flow);
        let times = measure(&ctx, &mut flow, &mut r, pan);
        release(&mut flow);
        ok &= report(&format!("pan, footprint cells ({path})"), &times, gated);

        // Wheel zoom from 70 px per bar out to all 3 334 bars (200 000
        // heatmap columns) and back.
        flow.time_scale().set_bar_spacing(70.0);
        flow.scroll_to_realtime();
        let times = measure(&ctx, &mut flow, &mut r, |chart, i, _| {
            let out = (i / 60) % 2 == 0;
            chart.handle_input(InputEvent::Wheel {
                pos: center,
                delta: WheelDelta::Pixels {
                    x: Css(0.0),
                    y: Css(if out { 60.0 } else { -60.0 }),
                },
                mods,
            });
        });
        ok &= report(
            &format!("zoom, footprint + heatmap ({path})"),
            &times,
            gated,
        );

        // Everything visible: 200 000 heatmap columns in 3 200 px.
        flow.fit_content();
        let times = measure(&ctx, &mut flow, &mut r, |chart, i, _| {
            chart
                .time_scale()
                .scroll_by(Px(if (i / 40) % 2 == 0 { 8.0 } else { -8.0 }));
        });
        ok &= report(&format!("pan, fit content ({path})"), &times, gated);
    }

    // TPO letters (500 px per session).
    {
        let mut r = renderer(&ctx, native);
        press(&mut tpo);
        let times = measure(&ctx, &mut tpo, &mut r, pan);
        release(&mut tpo);
        ok &= report("pan, TPO letters", &times, true);
        tpo.time_scale().set_bar_spacing(40.0);
        let times = measure(&ctx, &mut tpo, &mut r, |chart, i, _| {
            chart
                .time_scale()
                .scroll_by(Px(if (i / 40) % 2 == 0 { 8.0 } else { -8.0 }));
        });
        ok &= report("pan, TPO histograms", &times, true);
    }

    if !ok {
        println!("order flow target ({TARGET_MS} ms at p99) missed");
    }
}
