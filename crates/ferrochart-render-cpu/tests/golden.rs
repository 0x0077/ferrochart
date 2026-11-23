//! Golden-image tests: fixed data, size and theme, rendered by the CPU backend
//! and compared with `tests/golden/*.png` (per-channel tolerance ≤ 2, at most
//! 0.1% differing pixels).
//!
//! Regenerate with `FERROCHART_UPDATE_GOLDEN=1 cargo test golden`. Golden
//! updates go in their own commit that states the reason.
//!
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::ValueColumns;
use ferrochart_core::frame::{Reveal, SeriesKind};
use ferrochart_core::geom::{Rgba, SizeCss};
use ferrochart_core::pane::PaneOptions;
use ferrochart_core::render::Renderer;
use ferrochart_core::testing::{
    band_scene, cjk_text_scene, drawings_scene, drawings2_scene, image_markers_scene,
    merged_axis_scene, percentage_scene, points_scene, random_walk, series_kinds_scene,
    synced_scene, transform_scene,
};
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_cpu::CpuRenderer;
use ferrochart_text::{GlyphBitmap, GlyphSource, TextEngine};
use tiny_skia::Pixmap;

/// 2024-01-01T00:00:00Z.
const START: i64 = 1_704_067_200_000;
const W: f32 = 800.0;
const H: f32 = 450.0;
/// The reveal of the `Revealed` scene.
const REVEAL: Reveal = Reveal {
    progress: 0.55,
    pen: Some(Rgba::hex(0x004c_c9f0)),
};
/// Per-channel tolerance.
const CHANNEL_TOLERANCE: u8 = 2;
/// Maximum fraction of differing pixels.
const PIXEL_FRACTION: f64 = 0.001;

#[derive(Clone, Copy)]
enum Density {
    /// 40 hourly bars, wide spacing: borders, wide bodies.
    Sparse,
    /// 500 one-minute bars at the default spacing.
    Normal,
    /// 1M one-minute bars fitted to the width: LOD level, wicks only.
    UltraDense,
    /// As `Normal`, with the volume in a second pane.
    Panes,
    /// As `Sparse`, drawn as Heikin-Ashi candles.
    HeikinAshi,
    /// OHLC bars with line overlays, an area with a gap, and a baseline with a
    /// colored histogram, in three panes.
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
    /// Bands under candles and between crossing lines.
    Bands,
    /// As `Drawings`, revealed to 55 % with a pen line.
    Revealed,
    /// Chinese marker texts and badges, an emoji drawn as `□`.
    Cjk,
    /// As `Cjk`, with a color glyph source: the emoji is a color disc.
    ColorGlyphs,
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

/// The text engine of every golden: the bundled font plus a Noto Sans SC
/// subset (`tests/fonts`, the characters of `CJK_SCENE_CHARS`) as fallback.
fn text_engine() -> TextEngine {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/NotoSansSC-subset.otf");
    let mut engine = TextEngine::new();
    engine
        .add_fallback_font(std::fs::read(path).unwrap())
        .unwrap();
    engine
}

/// Renders the chart with the CPU backend.
fn finish(chart: Chart) -> Pixmap {
    finish_with(chart, text_engine())
}

fn finish_with(mut chart: Chart, engine: TextEngine) -> Pixmap {
    let mut renderer = CpuRenderer::with_text_engine(engine);
    chart.build_frame(0.0, &renderer.measurer());
    renderer.render(chart.frame(), &chart.data_view()).unwrap();
    renderer.pixmap().unwrap().clone()
}

/// The `Kinds` scene.
fn render_kinds(theme: Theme) -> Pixmap {
    let mut chart = Chart::new(ChartOptions {
        theme,
        ..ChartOptions::default()
    });
    chart.resize(SizeCss { w: W, h: H }, 1.0);
    series_kinds_scene(&mut chart, START).unwrap();
    finish(chart)
}

fn render(theme: Theme, density: Density) -> Pixmap {
    if let Density::Kinds = density {
        return render_kinds(theme);
    }
    if let Density::Merged = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        merged_axis_scene(&mut chart, START).unwrap();
        return finish(chart);
    }
    if let Density::Drawings = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        drawings_scene(&mut chart, START).unwrap();
        return finish(chart);
    }
    if let Density::Drawings2 = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        drawings2_scene(&mut chart, START).unwrap();
        return finish(chart);
    }
    if let Density::Revealed = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        drawings_scene(&mut chart, START).unwrap();
        chart.set_reveal(Some(REVEAL));
        return finish(chart);
    }
    if let Density::Bands = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        band_scene(&mut chart, START).unwrap();
        return finish(chart);
    }
    if let Density::Points = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        points_scene(&mut chart, START).unwrap();
        return finish(chart);
    }
    if let Density::Cjk | Density::ColorGlyphs = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        cjk_text_scene(&mut chart, START).unwrap();
        let mut engine = text_engine();
        if let Density::ColorGlyphs = density {
            engine.set_glyph_source(Some(Box::new(DiscGlyphs)));
        }
        return finish_with(chart, engine);
    }
    if let Density::ImageMarkers = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        image_markers_scene(&mut chart, START).unwrap();
        return finish(chart);
    }
    if let Density::Transform(kind) = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        transform_scene(&mut chart, START, kind).unwrap();
        return finish(chart);
    }
    if let Density::Synced = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        synced_scene(&mut chart, START).unwrap();
        return finish(chart);
    }
    if let Density::Percentage = density {
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        percentage_scene(&mut chart, START).unwrap();
        return finish(chart);
    }
    let mut chart = Chart::new(ChartOptions {
        theme,
        ..ChartOptions::default()
    });
    chart.resize(SizeCss { w: W, h: H }, 1.0);
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
        return finish(chart);
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
        | Density::Cjk
        | Density::ColorGlyphs
        | Density::Bands
        | Density::Revealed
        | Density::Percentage => {}
        Density::UltraDense => chart.fit_content(),
    }
    finish(chart)
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"))
}

fn check(name: &str, theme: Theme, density: Density) {
    let actual = render(theme, density);
    let path = golden_path(name);
    if std::env::var_os("FERROCHART_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        actual.save_png(&path).unwrap();
        return;
    }
    let expected = Pixmap::load_png(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {}: {e}; run with FERROCHART_UPDATE_GOLDEN=1",
            path.display()
        )
    });
    assert_eq!(
        (actual.width(), actual.height()),
        (expected.width(), expected.height()),
        "{name}: size differs"
    );
    let differing = actual
        .data()
        .chunks_exact(4)
        .zip(expected.data().chunks_exact(4))
        .filter(|(a, e)| {
            a.iter()
                .zip(e.iter())
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
        })
        .count();
    let total = f64::from(actual.width() * actual.height());
    if differing as f64 > total * PIXEL_FRACTION {
        let out = std::env::temp_dir().join(format!("ferrochart-golden-{name}.actual.png"));
        let _ = actual.save_png(&out);
        panic!(
            "{name}: {differing} pixels differ (limit {:.0}); actual image written to {}",
            total * PIXEL_FRACTION,
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

/// Rendering the same chart twice gives identical pixels (no hidden state).
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
fn golden_rendering_is_deterministic() {
    let a = render(Theme::light(), Density::Normal);
    let b = render(Theme::light(), Density::Normal);
    assert_eq!(a.data(), b.data());
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
