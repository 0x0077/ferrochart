//! Golden images of the order flow series,
//! next to the other goldens in `crates/ferrochart-render-cpu/tests/golden/`.
//! Each scene is rendered by the CPU backend and, when a GPU adapter is
//! available, by the wgpu backend through the portable path and (feature
//! `gpu`, heatmaps, devices with compute shaders) through the native path;
//! all must match the same image (per-channel tolerance ≤ 2, at most 0.1%
//! differing pixels).
//!
//! Regenerate (CPU) with `FERROCHART_UPDATE_GOLDEN=1 cargo test golden`.
//! Golden updates go in their own commit that states the reason.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::OnceLock;

use ferrochart_core::geom::{Css, SizeCss};
use ferrochart_core::render::Renderer;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_orderflow::testing::{footprint_scene, heatmap_scene, tpo_scene};
use ferrochart_orderflow::{FootprintMode, HeatmapSource};
use ferrochart_render_cpu::CpuRenderer;
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

/// 2024-01-01T00:00:00Z.
const START: i64 = 1_704_067_200_000;
const W: f32 = 800.0;
const H: f32 = 450.0;
const CHANNEL_TOLERANCE: u8 = 2;
const PIXEL_FRACTION: f64 = 0.001;

#[derive(Clone, Copy, Debug)]
enum Scene {
    /// Bid × ask numbers, 4 ticks per row, 90 px per bar.
    Footprint,
    /// Shaded cells, one tick per row, 20 px per bar.
    FootprintCells,
    /// Depth heatmap (1 s columns, about 2 px each) under candles.
    Heatmap,
    /// Trade heatmap at 18 px per candle: 0.3 px per column, so pixel
    /// columns take the largest of several columns.
    HeatmapTrades,
    /// TPO letters: three 6.5 h sessions at 230 px each.
    Tpo,
    /// TPO histograms: twelve sessions at 60 px each.
    TpoHistogram,
}

impl Scene {
    fn is_heatmap(self) -> bool {
        matches!(self, Self::Heatmap | Self::HeatmapTrades)
    }
}

fn chart(theme: Theme, scene: Scene) -> Chart {
    let mut chart = Chart::new(ChartOptions {
        theme,
        max_bar_spacing: Css(240.0),
        ..ChartOptions::default()
    });
    chart.resize(SizeCss { w: W, h: H }, 1.0);
    match scene {
        Scene::Footprint => {
            footprint_scene(&mut chart, &theme, START, FootprintMode::BidAsk, 90.0, 4)
        }
        Scene::FootprintCells => {
            footprint_scene(&mut chart, &theme, START, FootprintMode::BidAsk, 20.0, 1)
        }
        Scene::Heatmap => {
            heatmap_scene(&mut chart, START, HeatmapSource::Depth, 120.0).map(|(h, _)| h)
        }
        Scene::HeatmapTrades => {
            heatmap_scene(&mut chart, START, HeatmapSource::Trades, 18.0).map(|(h, _)| h)
        }
        Scene::Tpo => tpo_scene(&mut chart, &theme, START, 3, 230.0),
        Scene::TpoHistogram => tpo_scene(&mut chart, &theme, START, 12, 60.0),
    }
    .unwrap();
    chart
}

/// The CPU render as RGBA8 (the scenes are opaque, so premultiplied and
/// straight alpha agree).
fn render_cpu(theme: Theme, scene: Scene) -> Vec<u8> {
    let mut chart = chart(theme, scene);
    let mut r = CpuRenderer::new();
    chart.build_frame(0.0, &r.measurer());
    r.render(chart.frame(), &chart.data_view()).unwrap();
    r.pixmap().unwrap().data().to_vec()
}

fn gpu() -> Option<&'static GpuContext> {
    static CTX: OnceLock<Option<GpuContext>> = OnceLock::new();
    CTX.get_or_init(|| match GpuContext::headless() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("no GPU adapter ({e}); wgpu goldens skipped");
            None
        }
    })
    .as_ref()
}

fn render_wgpu(ctx: &GpuContext, theme: Theme, scene: Scene, native: bool) -> Vec<u8> {
    let mut chart = chart(theme, scene);
    let mut r = WgpuRenderer::offscreen(ctx);
    #[cfg(feature = "gpu")]
    if native {
        assert!(ferrochart_orderflow::register_gpu(ctx, &mut r));
        assert_eq!(r.native_kinds(), &[ferrochart_orderflow::HEATMAP_KIND]);
    }
    let _ = native;
    chart.set_native_kinds(r.native_kinds());
    chart.build_frame(0.0, &r.measurer());
    if let Err(e) = r.render(chart.frame(), &chart.data_view()) {
        panic!("{e}: {:?}", r.take_gpu_error());
    }
    r.read_pixels().unwrap()
}

/// Whether the native heatmap can be checked on `ctx`.
fn native(ctx: &GpuContext) -> bool {
    #[cfg(feature = "gpu")]
    {
        let ok = ferrochart_orderflow::supports_native_heatmap(ctx);
        if !ok {
            eprintln!("the GPU has no compute shaders; native heatmap goldens skipped");
        }
        ok
    }
    #[cfg(not(feature = "gpu"))]
    {
        let _ = ctx;
        false
    }
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ferrochart-render-cpu/tests/golden")
        .join(format!("{name}.png"))
}

fn save(path: &std::path::Path, rgba: &[u8]) -> image::ImageResult<()> {
    image::save_buffer(
        path,
        rgba,
        W as u32,
        H as u32,
        image::ExtendedColorType::Rgba8,
    )
}

fn compare(name: &str, backend: &str, actual: &[u8], expected: &[u8]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{name} ({backend}): size differs"
    );
    let differing = actual
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .filter(|(a, e)| {
            a.iter()
                .zip(e.iter())
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
        })
        .count();
    let total = f64::from(W * H);
    if differing as f64 > total * PIXEL_FRACTION {
        let out =
            std::env::temp_dir().join(format!("ferrochart-golden-{name}-{backend}.actual.png"));
        let _ = save(&out, actual);
        panic!(
            "{name} ({backend}): {differing} pixels differ (limit {:.0}); actual image written to {}",
            total * PIXEL_FRACTION,
            out.display()
        );
    }
}

fn check(name: &str, theme: Theme, scene: Scene) {
    let cpu = render_cpu(theme, scene);
    let path = golden_path(name);
    if std::env::var_os("FERROCHART_UPDATE_GOLDEN").is_some() {
        save(&path, &cpu).unwrap();
        return;
    }
    let expected = image::open(&path)
        .unwrap_or_else(|e| {
            panic!(
                "missing golden {}: {e}; run with FERROCHART_UPDATE_GOLDEN=1",
                path.display()
            )
        })
        .into_rgba8()
        .into_raw();
    compare(name, "cpu", &cpu, &expected);
    if let Some(ctx) = gpu() {
        compare(
            name,
            "wgpu",
            &render_wgpu(ctx, theme, scene, false),
            &expected,
        );
        if scene.is_heatmap() && native(ctx) {
            compare(
                name,
                "wgpu-native",
                &render_wgpu(ctx, theme, scene, true),
                &expected,
            );
        }
    }
}

#[test]
fn golden_footprint() {
    check("light_footprint", Theme::light(), Scene::Footprint);
    check("dark_footprint", Theme::dark(), Scene::Footprint);
}

#[test]
fn golden_footprint_cells() {
    check(
        "light_footprint_cells",
        Theme::light(),
        Scene::FootprintCells,
    );
    check("dark_footprint_cells", Theme::dark(), Scene::FootprintCells);
}

#[test]
fn golden_heatmap() {
    check("light_heatmap", Theme::light(), Scene::Heatmap);
    check("dark_heatmap", Theme::dark(), Scene::Heatmap);
}

#[test]
fn golden_heatmap_trades() {
    check("light_heatmap_trades", Theme::light(), Scene::HeatmapTrades);
    check("dark_heatmap_trades", Theme::dark(), Scene::HeatmapTrades);
}

#[test]
fn golden_tpo() {
    check("light_tpo", Theme::light(), Scene::Tpo);
    check("dark_tpo", Theme::dark(), Scene::Tpo);
}

#[test]
fn golden_tpo_histogram() {
    check("light_tpo_histogram", Theme::light(), Scene::TpoHistogram);
    check("dark_tpo_histogram", Theme::dark(), Scene::TpoHistogram);
}

/// The native heatmap uploads only what changed: after snapshots into the
/// last column and new columns, the same renderer still matches the CPU.
#[cfg(feature = "gpu")]
#[test]
fn native_heatmap_follows_updates() {
    use ferrochart_orderflow::{DepthSnapshot, append_depth};

    let Some(ctx) = gpu() else {
        return;
    };
    if !native(ctx) {
        return;
    }
    let mut chart = Chart::new(ChartOptions {
        theme: Theme::dark(),
        max_bar_spacing: Css(120.0),
        ..ChartOptions::default()
    });
    chart.resize(SizeCss { w: W, h: H }, 1.0);
    let (heatmap, _) = heatmap_scene(&mut chart, START, HeatmapSource::Depth, 120.0).unwrap();
    let mut gpu = WgpuRenderer::offscreen(ctx);
    assert!(ferrochart_orderflow::register_gpu(ctx, &mut gpu));
    let mut cpu = CpuRenderer::new();
    let frame = |chart: &mut Chart, gpu: &mut WgpuRenderer, cpu: &mut CpuRenderer| {
        chart.set_native_kinds(gpu.native_kinds());
        chart.build_frame(0.0, &gpu.measurer());
        gpu.render(chart.frame(), &chart.data_view()).unwrap();
        let native = gpu.read_pixels().unwrap();
        chart.set_native_kinds(&[]);
        chart.build_frame(0.0, &cpu.measurer());
        cpu.render(chart.frame(), &chart.data_view()).unwrap();
        (native, cpu.pixmap().unwrap().data().to_vec())
    };
    let (native, portable) = frame(&mut chart, &mut gpu, &mut cpu);
    compare("updates-0", "wgpu-native", &native, &portable);
    // The scene's last snapshot is at START + 2 399 s.
    let last = START + 2_399_000;
    let prices: Vec<f64> = (0..60).map(|k| 120.0 + f64::from(k) * 0.25).collect();
    for (step, t) in [last + 300, last + 1_000, last + 1_500, last + 2_000]
        .into_iter()
        .enumerate()
    {
        let sizes: Vec<f64> = (0..60)
            .map(|k| f64::from((k * 7 + step as i32 * 13) % 90))
            .collect();
        append_depth(
            &mut chart,
            heatmap,
            &DepthSnapshot {
                time: t,
                prices: &prices,
                sizes: &sizes,
            },
        )
        .unwrap();
        let (native, portable) = frame(&mut chart, &mut gpu, &mut cpu);
        compare(
            &format!("updates-{}", step + 1),
            "wgpu-native",
            &native,
            &portable,
        );
    }
    assert!(gpu.take_gpu_error().is_none());
}
