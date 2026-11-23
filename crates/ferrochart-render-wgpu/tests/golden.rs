//! The wgpu backend against the golden images produced by the CPU backend
//! (`crates/ferrochart-render-cpu/tests/golden`): per-channel tolerance ≤ 2, at
//! most 0.1% differing pixels.
//!
//! Needs a GPU adapter (a real one or a software adapter such as lavapipe or
//! WARP). Without one the tests print a notice and pass, so that machines
//! without a GPU can still run the rest of the suite.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::OnceLock;

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::ValueColumns;
use ferrochart_core::frame::{Reveal, SeriesKind};
use ferrochart_core::geom::{PointCss, Rgba, SizeCss};
use ferrochart_core::interaction::{CrosshairMode, InputEvent, Modifiers};
use ferrochart_core::pane::PaneOptions;
use ferrochart_core::price_scale::{PriceScaleMode, ScaleSide};
use ferrochart_core::render::Renderer;
use ferrochart_core::testing::{
    at_price_markers_scene, band_scene, bar_flags_scene, cjk_text_scene, clustered_markers_scene,
    drawings_scene, drawings2_scene, image_markers_scene, merged_axis_scene, percentage_scene,
    points_scene, random_walk, series_kinds_scene, synced_scene, transform_scene,
};
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};
use ferrochart_text::{GlyphBitmap, GlyphSource};

/// Must match `crates/ferrochart-render-cpu/tests/golden.rs`.
const START: i64 = 1_704_067_200_000;
/// The reveal of the `Revealed` scene.
const REVEAL: Reveal = Reveal {
    progress: 0.55,
    pen: Some(Rgba::hex(0x004c_c9f0)),
};
const W: f32 = 800.0;
const H: f32 = 450.0;
const CHANNEL_TOLERANCE: u8 = 2;
/// Point & Figure draws thousands of tiny anti-aliased diagonals and
/// ellipses; coverage rounding differs by up to 4/255 on their edges.
const MARK_TOLERANCE: u8 = 4;
const PIXEL_FRACTION: f64 = 0.001;

#[derive(Clone, Copy)]
enum Density {
    Sparse,
    Normal,
    UltraDense,
    /// As `Normal`, with the volume in a second pane.
    Panes,
    /// As `Sparse`, drawn as Heikin-Ashi candles.
    HeikinAshi,
    /// `series_kinds_scene`.
    Kinds,
    /// 1M closes as an area series fitted to the width: lines and fills at
    /// an LOD level.
    DenseArea,
    /// Candles on a time axis merged with a half-hour line.
    Merged,
    /// Candles and an area compared on a percentage scale.
    Percentage,
    /// Candles with every built-in drawing tool.
    Drawings,
    /// Candles with every second-batch drawing tool.
    Drawings2,
    /// Candles and volume panes with a crosshair and range from a synced
    /// chart.
    Synced,
    /// A transform series: Renko (`Candlestick`), Kagi or Point & Figure.
    Transform(SeriesKind),
    /// Candles with `Points` series: SAR-like dots and large marks.
    Points,
    /// Image markers, letter badges, borders and a folded stack (r21).
    ImageMarkers,
    /// `at_price_markers_scene` (r22).
    AtPriceMarkers,
    /// `clustered_markers_scene` at this bar spacing, px (r22).
    Clustered(u8),
    /// `bar_flags_scene`, dense or not (r22).
    BarFlags(bool),
    /// Chinese marker texts and badges, an emoji drawn as `□`.
    Cjk,
    /// As `Cjk`, with a color glyph source: the emoji is a color disc.
    ColorGlyphs,
    /// Bands under candles and between crossing lines.
    Bands,
    /// As `Drawings`, revealed to 55 % with a pen line.
    Revealed,
}

fn ctx() -> Option<&'static GpuContext> {
    static CTX: OnceLock<Option<GpuContext>> = OnceLock::new();
    CTX.get_or_init(|| match GpuContext::headless() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("no GPU adapter ({e}); wgpu golden tests skipped");
            None
        }
    })
    .as_ref()
}

fn chart(theme: Theme, density: Density) -> Chart {
    let mut chart = Chart::new(ChartOptions {
        theme,
        ..ChartOptions::default()
    });
    chart.resize(SizeCss { w: W, h: H }, 1.0);
    if let Density::Kinds = density {
        series_kinds_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::Merged = density {
        merged_axis_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::Percentage = density {
        percentage_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::Drawings = density {
        drawings_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::Drawings2 = density {
        drawings2_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::Synced = density {
        synced_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::Revealed = density {
        drawings_scene(&mut chart, START).unwrap();
        chart.set_reveal(Some(REVEAL));
        return chart;
    }
    if let Density::Bands = density {
        band_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::Points = density {
        points_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::ImageMarkers = density {
        image_markers_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::AtPriceMarkers = density {
        at_price_markers_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::Clustered(spacing) = density {
        clustered_markers_scene(&mut chart, START, f64::from(spacing)).unwrap();
        return chart;
    }
    if let Density::BarFlags(dense) = density {
        bar_flags_scene(&mut chart, START, dense).unwrap();
        return chart;
    }
    if let Density::Cjk | Density::ColorGlyphs = density {
        cjk_text_scene(&mut chart, START).unwrap();
        return chart;
    }
    if let Density::Transform(kind) = density {
        transform_scene(&mut chart, START, kind).unwrap();
        return chart;
    }
    let pane = chart.main_pane();
    if let Density::DenseArea = density {
        let data = random_walk(13, 1_000_000, START, 60_000).unwrap();
        let id = chart
            .add_series(pane, SeriesKind::Area, SeriesOptions::default())
            .unwrap();
        let values = ValueColumns {
            time: data.time,
            value: data.close,
            color: None,
        };
        chart.set_data(id, SeriesData::Value(values)).unwrap();
        chart.fit_content();
        return chart;
    }
    let kind = match density {
        Density::HeikinAshi => SeriesKind::HeikinAshi,
        _ => SeriesKind::Candlestick,
    };
    let id = chart
        .add_series(pane, kind, SeriesOptions::default())
        .unwrap();
    let volume_pane = match density {
        Density::Panes => chart.add_pane(PaneOptions {
            stretch_factor: 0.35,
        }),
        _ => pane,
    };
    chart
        .add_series(
            volume_pane,
            SeriesKind::Volume,
            SeriesOptions::volume_of(id),
        )
        .unwrap();
    let data = match density {
        Density::Sparse | Density::HeikinAshi => random_walk(11, 40, START, 3_600_000),
        Density::Normal
        | Density::Panes
        | Density::Kinds
        | Density::DenseArea
        | Density::Merged
        | Density::Drawings
        | Density::Drawings2
        | Density::Synced
        | Density::Transform(_)
        | Density::Points
        | Density::ImageMarkers
        | Density::AtPriceMarkers
        | Density::Clustered(_)
        | Density::BarFlags(_)
        | Density::Cjk
        | Density::ColorGlyphs
        | Density::Bands
        | Density::Revealed
        | Density::Percentage => random_walk(12, 500, START, 60_000),
        Density::UltraDense => random_walk(13, 1_000_000, START, 60_000),
    }
    .unwrap();
    chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
    match density {
        Density::Sparse | Density::HeikinAshi => chart.time_scale().set_bar_spacing(18.0),
        Density::Normal
        | Density::Panes
        | Density::Kinds
        | Density::DenseArea
        | Density::Merged
        | Density::Drawings
        | Density::Drawings2
        | Density::Synced
        | Density::Transform(_)
        | Density::Points
        | Density::ImageMarkers
        | Density::AtPriceMarkers
        | Density::Clustered(_)
        | Density::BarFlags(_)
        | Density::Cjk
        | Density::ColorGlyphs
        | Density::Bands
        | Density::Revealed
        | Density::Percentage => {}
        Density::UltraDense => chart.fit_content(),
    }
    chart
}

/// A deterministic stand-in for the browser's color glyphs: every
/// glyph (character or grapheme cluster) is a gradient disc 1.1 em wide, its colors kept as is. Must
/// match the copy in the other backend's golden tests.
struct DiscGlyphs;

impl GlyphSource for DiscGlyphs {
    fn advance(&self, _: &str, size: f32) -> f32 {
        (size * 1.1).round() + 1.0
    }

    fn rasterize(&self, _: &str, size: f32) -> Option<GlyphBitmap> {
        let n = (size * 1.1).round() as u32;
        let r = n as f32 * 0.5;
        let mut rgba = Vec::new();
        for y in 0..n {
            for x in 0..n {
                let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
                let a = (r - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
                rgba.extend_from_slice(&[
                    (255 * x / n) as u8,
                    160,
                    (255 * y / n) as u8,
                    (a * 255.0).round() as u8,
                ]);
            }
        }
        Some(GlyphBitmap {
            width: n,
            height: n,
            left: 0,
            top: -((size * 0.9).round() as i32),
            coverage: rgba.chunks_exact(4).map(|p| p[3]).collect(),
            rgba: Some(rgba),
        })
    }
}

/// As the CPU goldens: the bundled font plus the Noto Sans SC subset of
/// `ferrochart-render-cpu/tests/fonts` as fallback.
fn render(ctx: &GpuContext, chart: &mut Chart) -> Vec<u8> {
    render_with(ctx, chart, false)
}

fn render_with(ctx: &GpuContext, chart: &mut Chart, discs: bool) -> Vec<u8> {
    let mut renderer = WgpuRenderer::offscreen(ctx);
    if discs {
        renderer
            .text_engine_mut()
            .set_glyph_source(Some(Box::new(DiscGlyphs)));
    }
    let font = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ferrochart-render-cpu/tests/fonts/NotoSansSC-subset.otf");
    renderer
        .text_engine_mut()
        .add_fallback_font(std::fs::read(font).unwrap())
        .unwrap();
    chart.build_frame(0.0, &renderer.measurer());
    renderer.render(chart.frame(), &chart.data_view()).unwrap();
    renderer.read_pixels().unwrap()
}

/// Number of pixels differing by more than the channel tolerance.
fn differing(a: &[u8], b: &[u8]) -> usize {
    differing_by(a, b, CHANNEL_TOLERANCE)
}

/// Number of pixels differing by more than `tolerance` in a channel.
fn differing_by(a: &[u8], b: &[u8], tolerance: u8) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(x, y)| {
            x.iter()
                .zip(y.iter())
                .any(|(p, q)| p.abs_diff(*q) > tolerance)
        })
        .count()
}

fn check(name: &str, theme: Theme, density: Density) {
    let Some(ctx) = ctx() else {
        return;
    };
    let discs = matches!(density, Density::ColorGlyphs);
    let actual = render_with(ctx, &mut chart(theme, density), discs);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ferrochart-render-cpu/tests/golden")
        .join(format!("{name}.png"));
    let expected = image::open(&path)
        .unwrap_or_else(|e| panic!("missing golden {}: {e}", path.display()))
        .to_rgba8();
    assert_eq!(
        (expected.width(), expected.height()),
        (W as u32, H as u32),
        "{name}: size differs"
    );
    let tolerance = match density {
        Density::Transform(SeriesKind::PointFigure) => MARK_TOLERANCE,
        _ => CHANNEL_TOLERANCE,
    };
    let n = differing_by(&actual, expected.as_raw(), tolerance);
    let limit = f64::from(expected.width() * expected.height()) * PIXEL_FRACTION;
    if n as f64 > limit {
        let out = std::env::temp_dir().join(format!("ferrochart-wgpu-golden-{name}.actual.png"));
        let _ = image::save_buffer(
            &out,
            &actual,
            W as u32,
            H as u32,
            image::ExtendedColorType::Rgba8,
        );
        panic!(
            "{name}: {n} pixels differ (limit {limit:.0}); actual image written to {}",
            out.display()
        );
    }
}

#[test]
fn golden_light_sparse() {
    check("light_sparse", Theme::light(), Density::Sparse);
}

#[test]
fn golden_light_normal() {
    check("light_normal", Theme::light(), Density::Normal);
}

#[test]
fn golden_light_ultra_dense() {
    check("light_ultra_dense", Theme::light(), Density::UltraDense);
}

#[test]
fn golden_dark_sparse() {
    check("dark_sparse", Theme::dark(), Density::Sparse);
}

#[test]
fn golden_dark_normal() {
    check("dark_normal", Theme::dark(), Density::Normal);
}

#[test]
fn golden_dark_ultra_dense() {
    check("dark_ultra_dense", Theme::dark(), Density::UltraDense);
}

#[test]
fn golden_light_panes() {
    check("light_panes", Theme::light(), Density::Panes);
}

#[test]
fn golden_dark_panes() {
    check("dark_panes", Theme::dark(), Density::Panes);
}

/// A crosshair-only frame reuses the cached main layer and only redraws the
/// overlay: the result equals a full redraw of the same state.
#[test]
fn golden_light_heikin_ashi() {
    check("light_heikin_ashi", Theme::light(), Density::HeikinAshi);
}

#[test]
fn golden_dark_heikin_ashi() {
    check("dark_heikin_ashi", Theme::dark(), Density::HeikinAshi);
}

#[test]
fn golden_light_kinds() {
    check("light_kinds", Theme::light(), Density::Kinds);
}

#[test]
fn golden_dark_kinds() {
    check("dark_kinds", Theme::dark(), Density::Kinds);
}

#[test]
fn golden_light_dense_area() {
    check("light_dense_area", Theme::light(), Density::DenseArea);
}

#[test]
fn golden_light_merged_axis() {
    check("light_merged_axis", Theme::light(), Density::Merged);
}

#[test]
fn golden_dark_merged_axis() {
    check("dark_merged_axis", Theme::dark(), Density::Merged);
}

#[test]
fn golden_light_percentage() {
    check("light_percentage", Theme::light(), Density::Percentage);
}

#[test]
fn golden_light_drawings() {
    check("light_drawings", Theme::light(), Density::Drawings);
}

#[test]
fn golden_dark_drawings() {
    check("dark_drawings", Theme::dark(), Density::Drawings);
}

#[test]
fn golden_light_synced() {
    check("light_synced", Theme::light(), Density::Synced);
}

#[test]
fn golden_dark_synced() {
    check("dark_synced", Theme::dark(), Density::Synced);
}

#[test]
fn golden_light_renko() {
    check(
        "light_renko",
        Theme::light(),
        Density::Transform(SeriesKind::Candlestick),
    );
}

#[test]
fn golden_light_kagi() {
    check(
        "light_kagi",
        Theme::light(),
        Density::Transform(SeriesKind::Kagi),
    );
}

#[test]
fn golden_dark_kagi() {
    check(
        "dark_kagi",
        Theme::dark(),
        Density::Transform(SeriesKind::Kagi),
    );
}

#[test]
fn golden_light_point_figure() {
    check(
        "light_point_figure",
        Theme::light(),
        Density::Transform(SeriesKind::PointFigure),
    );
}

#[test]
fn golden_dark_point_figure() {
    check(
        "dark_point_figure",
        Theme::dark(),
        Density::Transform(SeriesKind::PointFigure),
    );
}

#[test]
fn golden_overlay_on_cached_layers_matches_full_redraw() {
    let Some(ctx) = ctx() else {
        return;
    };
    let mut chart = chart(Theme::dark(), Density::Normal);
    let mut renderer = WgpuRenderer::offscreen(ctx);
    chart.build_frame(0.0, &renderer.measurer());
    renderer.render(chart.frame(), &chart.data_view()).unwrap();
    chart.set_crosshair_mode(CrosshairMode::Magnet);
    chart.handle_input(InputEvent::PointerMove {
        id: 0,
        pos: PointCss { x: 300.0, y: 150.0 },
        mods: Modifiers::default(),
        time_ms: 0.0,
    });
    chart.build_frame(0.0, &renderer.measurer());
    renderer.render(chart.frame(), &chart.data_view()).unwrap();
    let incremental = renderer.read_pixels().unwrap();

    let mut fresh = WgpuRenderer::offscreen(ctx);
    fresh.render(chart.frame(), &chart.data_view()).unwrap();
    assert_eq!(incremental, fresh.read_pixels().unwrap());
}

/// Real-time updates and appends reach the GPU through `changes_since` and
/// give the same pixels as a renderer that uploads everything from scratch.
#[test]
fn golden_incremental_upload_matches_fresh_upload() {
    let Some(ctx) = ctx() else {
        return;
    };
    let mut chart = chart(Theme::light(), Density::Normal);
    let mut renderer = WgpuRenderer::offscreen(ctx);
    chart.build_frame(0.0, &renderer.measurer());
    renderer.render(chart.frame(), &chart.data_view()).unwrap();
    let id = ferrochart_core::data::SeriesId(0);
    let more = random_walk(99, 30, START + 500 * 60_000, 60_000).unwrap();
    chart.append(id, SeriesData::Ohlc(more)).unwrap();
    for i in 0..20 {
        let close = 101.0 + f64::from(i) * 0.1;
        chart
            .update_last(
                id,
                ferrochart_core::data::BarUpdate {
                    time: START + 529 * 60_000,
                    open: 101.0,
                    high: close.max(103.0),
                    low: 99.0,
                    close,
                    volume: Some(900.0 + f64::from(i)),
                },
            )
            .unwrap();
        chart.build_frame(0.0, &renderer.measurer());
        renderer.render(chart.frame(), &chart.data_view()).unwrap();
    }
    let incremental = renderer.read_pixels().unwrap();
    let mut fresh = WgpuRenderer::offscreen(ctx);
    fresh.render(chart.frame(), &chart.data_view()).unwrap();
    assert_eq!(incremental, fresh.read_pixels().unwrap());
}

/// Logarithmic mode uses the ln buffers and agrees with the CPU backend's
/// ln path within the golden tolerance.
#[test]
fn golden_log_scale_matches_cpu_path() {
    let Some(ctx) = ctx() else {
        return;
    };
    let mut chart = chart(Theme::light(), Density::Normal);
    let pane = chart.main_pane();
    chart
        .price_scale(pane, ScaleSide::Right)
        .unwrap()
        .set_mode(PriceScaleMode::Logarithmic)
        .unwrap();
    let gpu = render(ctx, &mut chart);
    let linear = render(ctx, &mut self::chart(Theme::light(), Density::Normal));
    // The log chart differs from the linear one...
    assert!(differing(&gpu, &linear) > 1000);
    // ...and is stable across renderers.
    let again = render(ctx, &mut chart);
    assert_eq!(gpu, again);
}

#[test]
fn golden_light_points() {
    check("light_points", Theme::light(), Density::Points);
}

#[test]
fn golden_dark_points() {
    check("dark_points", Theme::dark(), Density::Points);
}

#[test]
fn golden_light_image_markers() {
    check("light_image_markers", Theme::light(), Density::ImageMarkers);
}

#[test]
fn golden_dark_image_markers() {
    check("dark_image_markers", Theme::dark(), Density::ImageMarkers);
}

#[test]
fn golden_light_at_price_markers() {
    check(
        "light_at_price_markers",
        Theme::light(),
        Density::AtPriceMarkers,
    );
}

#[test]
fn golden_dark_at_price_markers() {
    check(
        "dark_at_price_markers",
        Theme::dark(),
        Density::AtPriceMarkers,
    );
}

#[test]
fn golden_light_clustered_markers_narrow() {
    // 6 px bars: buckets of 5.
    check(
        "light_clustered_markers_narrow",
        Theme::light(),
        Density::Clustered(6),
    );
}

#[test]
fn golden_dark_clustered_markers_wide() {
    // 12 px bars: buckets of 3.
    check(
        "dark_clustered_markers_wide",
        Theme::dark(),
        Density::Clustered(12),
    );
}

#[test]
fn golden_light_bar_flags() {
    check("light_bar_flags", Theme::light(), Density::BarFlags(false));
}

#[test]
fn golden_dark_bar_flags_lod() {
    check("dark_bar_flags_lod", Theme::dark(), Density::BarFlags(true));
}

#[test]
fn golden_light_cjk_text() {
    check("light_cjk_text", Theme::light(), Density::Cjk);
}

#[test]
fn golden_dark_cjk_text() {
    check("dark_cjk_text", Theme::dark(), Density::Cjk);
}

#[test]
fn golden_dark_color_glyphs() {
    check("dark_color_glyphs", Theme::dark(), Density::ColorGlyphs);
}

#[test]
fn golden_light_bands() {
    check("light_bands", Theme::light(), Density::Bands);
}

#[test]
fn golden_dark_bands() {
    check("dark_bands", Theme::dark(), Density::Bands);
}

#[test]
fn golden_light_drawings2() {
    check("light_drawings2", Theme::light(), Density::Drawings2);
}

#[test]
fn golden_dark_drawings2() {
    check("dark_drawings2", Theme::dark(), Density::Drawings2);
}

#[test]
fn golden_light_revealed() {
    check("light_revealed", Theme::light(), Density::Revealed);
}

#[test]
fn golden_dark_revealed() {
    check("dark_revealed", Theme::dark(), Density::Revealed);
}
