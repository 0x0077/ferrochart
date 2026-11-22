//! Golden images of plots, next to the other
//! goldens in `crates/ferrochart-render-cpu/tests/golden/` with a `plot_`
//! prefix. Each scene is rendered by the CPU backend and, when a GPU adapter
//! is available, by the wgpu backend; both must match the same image
//! (per-channel tolerance ≤ 2, at most 0.1% differing pixels).
//!
//! Regenerate (CPU) with `FERROCHART_UPDATE_GOLDEN=1 cargo test golden`.
//! Golden updates go in their own commit that states the reason.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::OnceLock;

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::frame::{LineDash, Reveal, TextAlign};
use ferrochart_core::geom::{Css, PointCss, Rgba, SizeCss};
use ferrochart_core::interaction::{InputEvent, Modifiers};
use ferrochart_core::render::Renderer;
use ferrochart_core::text::TextMeasurer;
use ferrochart_core::theme::Theme;
use ferrochart_core::transform_helpers::{ICON_ARROW_DOWN, ICON_SQUARE};
use ferrochart_plot::{
    AxisFormat, AxisId, AxisOptions, AxisRange, ColorScale, LabelOptions, MarkData, MarkKind,
    MarkOptions, Orientation, Plot, PlotEvent, PlotOptions, RuleOptions, WaterfallStep,
};
use ferrochart_render_cpu::CpuRenderer;
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

/// 2024-01-01T00:00:00Z.
const START: f64 = 1_704_067_200_000.0;
const DAY: f64 = 86_400_000.0;
const W: f32 = 800.0;
const H: f32 = 450.0;
const CHANNEL_TOLERANCE: u8 = 2;
const PIXEL_FRACTION: f64 = 0.001;

#[derive(Clone, Copy, Debug)]
enum Scene {
    /// Numeric axes: two smiles (one stepped, with points and a gap), a
    /// rule, a band, labels and titles.
    Lines,
    /// Log x, sizes, value colors, labels.
    Scatter,
    /// Category x with a percent left axis.
    Category,
    /// Date x, compact right axis.
    Dates,
    /// Category x: three grouped bars with values, negatives, a down color.
    BarsGrouped,
    /// Horizontal stacked bars (positive and negative) on a category y axis.
    BarsStacked,
    /// A profit/loss area in two colors over a y0 band.
    Areas,
    /// Three stacked step and line areas on a date axis.
    AreasStacked,
    /// Deltas, a subtotal and a total with connectors and values.
    Waterfall,
    /// A category grid on a diverging scale with values and a missing cell.
    Cells,
    /// Bars on a numeric x (a histogram) under a density line.
    Histogram,
    /// A hovered scatter point with the crosshair.
    HoverScatter,
    /// A hovered bar (tinted).
    HoverBars,
    /// `Lines` revealed to 0.55 with a pen: marks, rules and labels
    /// clipped, axes whole, rule labels hidden.
    Revealed,
}

fn plot(theme: Theme, scene: Scene) -> Plot {
    if let Scene::Revealed = scene {
        let mut p = plot(theme, Scene::Lines);
        p.set_reveal(Some(Reveal {
            progress: 0.55,
            pen: Some(Rgba::hex(0x004c_c9f0)),
        }));
        return p;
    }
    let mut p = Plot::new(PlotOptions {
        theme,
        ..PlotOptions::default()
    });
    p.resize(SizeCss { w: W, h: H }, 1.0);
    match scene {
        Scene::Lines => {
            p.set_axis(
                AxisId::X,
                AxisOptions {
                    title: Some("Strike".into()),
                    ..AxisOptions::linear()
                },
            )
            .unwrap();
            p.set_axis(
                AxisId::Right,
                AxisOptions {
                    title: Some("Implied vol".into()),
                    ..AxisOptions::linear()
                },
            )
            .unwrap();
            let strikes: Vec<f64> = (0..=16).map(|k| 80.0 + 2.5 * f64::from(k)).collect();
            let smile = |s: f64, c: f64| 0.2 + c * ((s - 100.0) / 20.0).powi(2);
            let a = p.add_mark(MarkKind::Line, MarkOptions::default()).unwrap();
            let y: Vec<f64> = strikes.iter().map(|&s| smile(s, 0.12)).collect();
            p.set_data(a, MarkData::xy(strikes.clone(), y)).unwrap();
            let b = p
                .add_mark(
                    MarkKind::Line,
                    MarkOptions {
                        step_line: true,
                        point_size: Css(6.0),
                        line_style: LineDash::Dashed,
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            let mut y: Vec<f64> = strikes.iter().map(|&s| smile(s, 0.08) + 0.02).collect();
            y[8] = f64::NAN;
            p.set_data(b, MarkData::xy(strikes, y)).unwrap();
            p.add_rule(RuleOptions {
                axis: AxisId::X,
                value: 100.0,
                label: Some("ATM".into()),
                ..RuleOptions::default()
            })
            .unwrap();
            p.add_rule(RuleOptions {
                axis: AxisId::Right,
                value: 0.22,
                to: Some(0.26),
                color: Some(Rgba::hex(0x10_B981)),
                ..RuleOptions::default()
            })
            .unwrap();
            p.add_rule(RuleOptions {
                axis: AxisId::Right,
                value: 0.3,
                color: Some(Rgba::hex(0xEF_4444)),
                line_style: LineDash::Solid,
                label: Some("0.30".into()),
                ..RuleOptions::default()
            })
            .unwrap();
            p.add_label(LabelOptions {
                x: 85.0,
                y: 0.29,
                text: "skew".into(),
                icon: Some(ICON_ARROW_DOWN),
                ..LabelOptions::default()
            })
            .unwrap();
        }
        Scene::Scatter => {
            p.set_axis(AxisId::X, AxisOptions::log()).unwrap();
            let m = p
                .add_mark(
                    MarkKind::Scatter,
                    MarkOptions {
                        icon: ICON_SQUARE,
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            let n = 40;
            let x: Vec<f64> = (0..n).map(|i| 10f64.powf(f64::from(i) / 10.0)).collect();
            let y: Vec<f64> = (0..n)
                .map(|i| (f64::from(i) * 0.7).sin() * 5.0 + f64::from(i) * 0.3)
                .collect();
            let mut d = MarkData::xy(x, y);
            d.value = Some((0..n).map(f64::from).collect());
            d.size = Some((0..n).map(|i| 4.0 + (i % 5) as f32 * 3.0).collect());
            d.label = Some(
                (0..n)
                    .map(|i| {
                        if i % 6 == 0 {
                            format!("P{i}")
                        } else {
                            String::new()
                        }
                    })
                    .collect(),
            );
            p.set_data(m, d).unwrap();
            let s = p
                .add_mark(
                    MarkKind::Scatter,
                    MarkOptions {
                        point_size: Css(10.0),
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            let mut d = MarkData::xy(vec![2.0, 20.0, 200.0], vec![12.0, -4.0, 6.0]);
            d.color = Some(vec![0, 0xEC48_99FF, 0]);
            p.set_data(s, d).unwrap();
        }
        Scene::Category => {
            p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
            p.set_categories(
                AxisId::X,
                &[
                    "Tech",
                    "Energy",
                    "Health",
                    "Finance",
                    "Utilities",
                    "Materials",
                    "Industrials",
                    "Staples",
                ],
            )
            .unwrap();
            p.set_axis(
                AxisId::Left,
                AxisOptions {
                    format: AxisFormat::Percent(1),
                    range: AxisRange::Auto {
                        include_zero: true,
                        margin: 0.1,
                    },
                    ..AxisOptions::linear()
                },
            )
            .unwrap();
            p.set_axis(AxisId::Right, AxisOptions::linear().hidden())
                .unwrap();
            let x: Vec<f64> = (0..8).map(f64::from).collect();
            for (k, ys) in [
                [4.2, -1.3, 2.1, 0.8, -0.4, 1.9, 3.0, -2.2],
                [1.0, 2.5, -0.6, 1.4, 0.3, -1.1, 0.9, 0.5],
            ]
            .into_iter()
            .enumerate()
            {
                let m = p
                    .add_mark(
                        MarkKind::Line,
                        MarkOptions {
                            y_axis: AxisId::Left,
                            point_size: Css(7.0),
                            line_width: Css(1.5 + k as f32),
                            ..MarkOptions::default()
                        },
                    )
                    .unwrap();
                p.set_data(m, MarkData::xy(x.clone(), ys.to_vec())).unwrap();
            }
            p.add_rule(RuleOptions {
                axis: AxisId::Left,
                value: 0.0,
                line_style: LineDash::Solid,
                ..RuleOptions::default()
            })
            .unwrap();
        }
        Scene::Dates => {
            p.set_axis(AxisId::X, AxisOptions::date()).unwrap();
            p.set_axis(
                AxisId::Right,
                AxisOptions {
                    format: AxisFormat::Compact,
                    ..AxisOptions::linear()
                },
            )
            .unwrap();
            let x: Vec<f64> = [7.0, 30.0, 60.0, 91.0, 182.0, 365.0, 547.0, 730.0]
                .iter()
                .map(|d| START + d * DAY)
                .collect();
            let m = p
                .add_mark(
                    MarkKind::Line,
                    MarkOptions {
                        point_size: Css(5.0),
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            let y = vec![1.2e6, 1.5e6, 1.9e6, 2.4e6, 2.1e6, 2.8e6, 3.3e6, 3.1e6];
            p.set_data(m, MarkData::xy(x, y)).unwrap();
            p.add_label(LabelOptions {
                x: START + 365.0 * DAY,
                y: 2.8e6,
                text: "1Y".into(),
                align: TextAlign::Center,
                offset: (Css(0.0), Css(-12.0)),
                ..LabelOptions::default()
            })
            .unwrap();
        }
        Scene::BarsGrouped => {
            p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
            p.set_categories(AxisId::X, &["Q1", "Q2", "Q3", "Q4", "Q5"])
                .unwrap();
            let x: Vec<f64> = (0..5).map(f64::from).collect();
            for (k, ys) in [
                [12.0, 15.5, 9.0, 18.25, 14.0],
                [8.0, -3.5, 11.0, 6.0, 9.5],
                [4.0, 6.0, -2.0, 7.0, 3.0],
            ]
            .into_iter()
            .enumerate()
            {
                let m = p
                    .add_mark(
                        MarkKind::Bars,
                        MarkOptions {
                            group: Some(1),
                            show_values: true,
                            down_color: (k == 1).then(|| Rgba::hex(0xEF_4444)),
                            ..MarkOptions::default()
                        },
                    )
                    .unwrap();
                p.set_data(m, MarkData::xy(x.clone(), ys.to_vec())).unwrap();
            }
        }
        Scene::BarsStacked => {
            p.set_axis(AxisId::Right, AxisOptions::category()).unwrap();
            p.set_categories(
                AxisId::Right,
                &["Equities", "Rates", "Credit", "FX", "Commodities"],
            )
            .unwrap();
            let y: Vec<f64> = (0..5).map(f64::from).collect();
            for (k, xs) in [
                [30.0, 12.0, 18.0, 6.0, 9.0],
                [14.0, 20.0, 7.0, 11.0, 5.0],
                [-9.0, -4.0, -12.0, -6.0, -15.0],
            ]
            .into_iter()
            .enumerate()
            {
                let m = p
                    .add_mark(
                        MarkKind::Bars,
                        MarkOptions {
                            orientation: Orientation::Horizontal,
                            stack: Some(1),
                            show_values: true,
                            band: 0.6,
                            color: (k == 2).then(|| Rgba::hex(0xEF_4444)),
                            ..MarkOptions::default()
                        },
                    )
                    .unwrap();
                p.set_data(m, MarkData::xy(xs.to_vec(), y.clone())).unwrap();
            }
        }
        Scene::Areas => {
            let x: Vec<f64> = (0..=60).map(|i| 70.0 + f64::from(i)).collect();
            let band = p
                .add_mark(
                    MarkKind::Area,
                    MarkOptions {
                        line_width: Css(0.0),
                        fill_alpha: 0.18,
                        color: Some(Rgba::hex(0x8B_5CF6)),
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            let mid: Vec<f64> = x.iter().map(|&s| (s - 100.0) * 0.4).collect();
            let mut d = MarkData::xy(x.clone(), mid.iter().map(|v| v + 4.0).collect());
            d.y0 = Some(mid.iter().map(|v| v - 4.0).collect());
            p.set_data(band, d).unwrap();
            let pnl = p
                .add_mark(
                    MarkKind::Area,
                    MarkOptions {
                        color: Some(Rgba::hex(0x10_B981)),
                        down_color: Some(Rgba::hex(0xEF_4444)),
                        fill_alpha: 0.35,
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            // A long call spread: flat loss, ramp, flat gain.
            let y: Vec<f64> = x
                .iter()
                .map(|&s| (s - 95.0).clamp(0.0, 15.0) - 6.0)
                .collect();
            p.set_data(pnl, MarkData::xy(x, y)).unwrap();
        }
        Scene::AreasStacked => {
            p.set_axis(AxisId::X, AxisOptions::date()).unwrap();
            let x: Vec<f64> = (0..24).map(|m| START + f64::from(m) * 30.0 * DAY).collect();
            for k in 0..3 {
                let m = p
                    .add_mark(
                        MarkKind::Area,
                        MarkOptions {
                            stack: Some(1),
                            step_line: k == 2,
                            fill_alpha: 0.5,
                            line_width: Css(1.0),
                            ..MarkOptions::default()
                        },
                    )
                    .unwrap();
                let y: Vec<f64> = (0..24)
                    .map(|i| {
                        let t = f64::from(i);
                        f64::from(k + 2) * (1.0 + 0.3 * (t * 0.4 + f64::from(k)).sin()) + t * 0.1
                    })
                    .collect();
                p.set_data(m, MarkData::xy(x.clone(), y)).unwrap();
            }
        }
        Scene::Waterfall => {
            p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
            p.set_categories(
                AxisId::X,
                &["Revenue", "COGS", "Opex", "Gross", "Tax", "Other", "Net"],
            )
            .unwrap();
            let m = p
                .add_mark(
                    MarkKind::Waterfall,
                    MarkOptions {
                        show_values: true,
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            let mut d = MarkData::xy(
                (0..7).map(f64::from).collect(),
                vec![120.0, -45.0, -30.0, f64::NAN, -12.5, 6.0, f64::NAN],
            );
            d.step = Some(vec![
                WaterfallStep::Delta,
                WaterfallStep::Delta,
                WaterfallStep::Delta,
                WaterfallStep::Subtotal,
                WaterfallStep::Delta,
                WaterfallStep::Delta,
                WaterfallStep::Total,
            ]);
            p.set_data(m, d).unwrap();
        }
        Scene::Cells => {
            p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
            p.set_axis(AxisId::Right, AxisOptions::category()).unwrap();
            let cols = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug"];
            let rows = ["SPX", "NDX", "RUT", "DAX", "N225"];
            p.set_categories(AxisId::X, &cols).unwrap();
            p.set_categories(AxisId::Right, &rows).unwrap();
            let m = p
                .add_mark(
                    MarkKind::Cells,
                    MarkOptions {
                        show_values: true,
                        band: 0.94,
                        value_format: AxisFormat::Fixed(1),
                        color_scale: Some(ColorScale::diverging(0.0, &theme)),
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            let (mut x, mut y, mut v) = (Vec::new(), Vec::new(), Vec::new());
            for r in 0..5 {
                for c in 0..8 {
                    x.push(f64::from(c));
                    y.push(f64::from(r));
                    let val = ((f64::from(c) * 1.3 + f64::from(r) * 0.7).sin() * 6.0).round() / 2.0;
                    v.push(if (r, c) == (2, 5) { f64::NAN } else { val });
                }
            }
            let mut d = MarkData::xy(x, y);
            d.value = Some(v);
            p.set_data(m, d).unwrap();
        }
        Scene::Histogram => {
            let edges: Vec<f64> = (0..24)
                .map(|i| -3.0 + 0.25 * f64::from(i) + 0.125)
                .collect();
            let density = |x: f64| (-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
            let counts: Vec<f64> = edges
                .iter()
                .map(|&c| {
                    (density(c) * 400.0 * 0.25 + (c * 7.0).sin() * 2.0)
                        .round()
                        .max(0.0)
                })
                .collect();
            let bars = p
                .add_mark(
                    MarkKind::Bars,
                    MarkOptions {
                        band: 0.92,
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            p.set_data(bars, MarkData::xy(edges, counts)).unwrap();
            let xs: Vec<f64> = (0..=120).map(|i| -3.0 + 0.05 * f64::from(i)).collect();
            let ys: Vec<f64> = xs.iter().map(|&x| density(x) * 400.0 * 0.25).collect();
            let line = p.add_mark(MarkKind::Line, MarkOptions::default()).unwrap();
            p.set_data(line, MarkData::xy(xs, ys)).unwrap();
        }
        Scene::HoverScatter => {
            p = Plot::new(PlotOptions {
                theme,
                crosshair: true,
                ..PlotOptions::default()
            });
            p.resize(SizeCss { w: W, h: H }, 1.0);
            let m = p
                .add_mark(
                    MarkKind::Scatter,
                    MarkOptions {
                        point_size: Css(8.0),
                        ..MarkOptions::default()
                    },
                )
                .unwrap();
            let x: Vec<f64> = (0..30).map(f64::from).collect();
            let y: Vec<f64> = x.iter().map(|v| (v * 0.5).sin() * 10.0 + v * 0.4).collect();
            p.set_data(m, MarkData::xy(x, y)).unwrap();
        }
        Scene::HoverBars => {
            p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
            p.set_categories(AxisId::X, &["A", "B", "C", "D"]).unwrap();
            let m = p.add_mark(MarkKind::Bars, MarkOptions::default()).unwrap();
            p.set_data(
                m,
                MarkData::xy(vec![0.0, 1.0, 2.0, 3.0], vec![3.0, 7.0, 5.0, 2.0]),
            )
            .unwrap();
        }
        // Built from `Lines` above.
        Scene::Revealed => {}
    }
    p
}

/// Builds the frame, with the pointer over an element in hover scenes.
fn build(p: &mut Plot, scene: Scene, text: &dyn TextMeasurer) {
    p.build_frame(0.0, text);
    let target = match scene {
        Scene::HoverScatter => Some((AxisId::Right, 12.0, 6.0f64.sin() * 10.0 + 4.8)),
        Scene::HoverBars => Some((AxisId::Right, 1.5, 3.0)),
        _ => None,
    };
    if let Some((axis, x, y)) = target {
        let pos = PointCss {
            x: p.value_to_px(AxisId::X, x).unwrap().0,
            y: p.value_to_px(axis, y).unwrap().0 + 2.0,
        };
        p.handle_input(InputEvent::PointerMove {
            id: 0,
            pos,
            mods: Modifiers::default(),
            time_ms: 0.0,
        });
        assert!(
            p.drain_events()
                .any(|e| matches!(e, PlotEvent::Hover(Some(_))))
        );
        p.build_frame(0.0, text);
    }
}

fn render_cpu(theme: Theme, scene: Scene) -> Vec<u8> {
    let mut p = plot(theme, scene);
    let mut r = CpuRenderer::new();
    build(&mut p, scene, &r.measurer());
    r.render(p.frame(), &ChartDataView::empty()).unwrap();
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

fn render_wgpu(ctx: &GpuContext, theme: Theme, scene: Scene) -> Vec<u8> {
    let mut p = plot(theme, scene);
    let mut r = WgpuRenderer::offscreen(ctx);
    build(&mut p, scene, &r.measurer());
    if let Err(e) = r.render(p.frame(), &ChartDataView::empty()) {
        panic!("{e}: {:?}", r.take_gpu_error());
    }
    r.read_pixels().unwrap()
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
        compare(name, "wgpu", &render_wgpu(ctx, theme, scene), &expected);
    }
}

#[test]
fn golden_plot_lines() {
    check("light_plot_lines", Theme::light(), Scene::Lines);
    check("dark_plot_lines", Theme::dark(), Scene::Lines);
}

#[test]
fn golden_plot_scatter() {
    check("light_plot_scatter", Theme::light(), Scene::Scatter);
    check("dark_plot_scatter", Theme::dark(), Scene::Scatter);
}

#[test]
fn golden_plot_category() {
    check("light_plot_category", Theme::light(), Scene::Category);
    check("dark_plot_category", Theme::dark(), Scene::Category);
}

#[test]
fn golden_plot_dates() {
    check("light_plot_dates", Theme::light(), Scene::Dates);
    check("dark_plot_dates", Theme::dark(), Scene::Dates);
}

#[test]
fn golden_plot_bars_grouped() {
    check(
        "light_plot_bars_grouped",
        Theme::light(),
        Scene::BarsGrouped,
    );
    check("dark_plot_bars_grouped", Theme::dark(), Scene::BarsGrouped);
}

#[test]
fn golden_plot_bars_stacked() {
    check(
        "light_plot_bars_stacked",
        Theme::light(),
        Scene::BarsStacked,
    );
    check("dark_plot_bars_stacked", Theme::dark(), Scene::BarsStacked);
}

#[test]
fn golden_plot_areas() {
    check("light_plot_areas", Theme::light(), Scene::Areas);
    check("dark_plot_areas", Theme::dark(), Scene::Areas);
}

#[test]
fn golden_plot_areas_stacked() {
    check(
        "light_plot_areas_stacked",
        Theme::light(),
        Scene::AreasStacked,
    );
    check(
        "dark_plot_areas_stacked",
        Theme::dark(),
        Scene::AreasStacked,
    );
}

#[test]
fn golden_plot_waterfall() {
    check("light_plot_waterfall", Theme::light(), Scene::Waterfall);
    check("dark_plot_waterfall", Theme::dark(), Scene::Waterfall);
}

#[test]
fn golden_plot_cells() {
    check("light_plot_cells", Theme::light(), Scene::Cells);
    check("dark_plot_cells", Theme::dark(), Scene::Cells);
}

#[test]
fn golden_plot_histogram() {
    check("light_plot_histogram", Theme::light(), Scene::Histogram);
    check("dark_plot_histogram", Theme::dark(), Scene::Histogram);
}

#[test]
fn golden_plot_hover_scatter() {
    check(
        "light_plot_hover_scatter",
        Theme::light(),
        Scene::HoverScatter,
    );
    check(
        "dark_plot_hover_scatter",
        Theme::dark(),
        Scene::HoverScatter,
    );
}

#[test]
fn golden_plot_hover_bars() {
    check("light_plot_hover_bars", Theme::light(), Scene::HoverBars);
    check("dark_plot_hover_bars", Theme::dark(), Scene::HoverBars);
}

#[test]
fn golden_plot_revealed() {
    check("light_plot_revealed", Theme::light(), Scene::Revealed);
    check("dark_plot_revealed", Theme::dark(), Scene::Revealed);
}
