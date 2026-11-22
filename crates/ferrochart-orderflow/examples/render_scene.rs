//! Renders an orderflow golden scene with the CPU backend to a PNG (for
//! looking at changes): `cargo run -p ferrochart-orderflow --features testing
//! --example render_scene -- <scene> <out.png> [dark]`, where `<scene>` is
//! `footprint`, `footprint_delta`, `footprint_cells`, `heatmap`,
//! `heatmap_trades`, `tpo` or `tpo_histogram`.

use std::process::ExitCode;

use ferrochart_core::geom::{Css, SizeCss};
use ferrochart_core::render::Renderer;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_orderflow::testing::{footprint_scene, heatmap_scene, tpo_scene};
use ferrochart_orderflow::{FootprintMode, HeatmapSource};
use ferrochart_render_cpu::CpuRenderer;

const START: i64 = 1_704_067_200_000;

fn run(scene: &str, out: &str, theme: Theme) -> Result<(), Box<dyn std::error::Error>> {
    let mut chart = Chart::new(ChartOptions {
        theme,
        // Footprint numbers need 60 px per bar, TPO letters more.
        max_bar_spacing: Css(240.0),
        ..ChartOptions::default()
    });
    chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
    match scene {
        "footprint" => {
            footprint_scene(&mut chart, &theme, START, FootprintMode::BidAsk, 90.0, 4).map(drop)?;
        }
        "footprint_delta" => {
            footprint_scene(&mut chart, &theme, START, FootprintMode::Delta, 70.0, 4).map(drop)?;
        }
        "footprint_cells" => {
            footprint_scene(&mut chart, &theme, START, FootprintMode::BidAsk, 20.0, 1).map(drop)?;
        }
        "heatmap" => {
            heatmap_scene(&mut chart, START, HeatmapSource::Depth, 120.0).map(drop)?;
        }
        "heatmap_trades" => {
            heatmap_scene(&mut chart, START, HeatmapSource::Trades, 18.0).map(drop)?;
        }
        "tpo" => {
            tpo_scene(&mut chart, &theme, START, 3, 230.0).map(drop)?;
        }
        "tpo_histogram" => {
            tpo_scene(&mut chart, &theme, START, 12, 60.0).map(drop)?;
        }
        _ => return Err(format!("unknown scene {scene}").into()),
    }
    let mut r = CpuRenderer::new();
    chart.build_frame(0.0, &r.measurer());
    r.render(chart.frame(), &chart.data_view())?;
    std::fs::write(out, r.encode_png()?)?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(scene), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("usage: render_scene <scene> <out.png> [dark]");
        return ExitCode::FAILURE;
    };
    let theme = if args.get(2).is_some_and(|a| a == "dark") {
        Theme::dark()
    } else {
        Theme::light()
    };
    match run(scene, out, theme) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
