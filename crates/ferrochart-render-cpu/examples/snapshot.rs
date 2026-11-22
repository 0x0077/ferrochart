//! Renders a chart with a crosshair to a PNG: `cargo run -p ferrochart-render-cpu --example snapshot -- out.png`.
#![allow(missing_docs, clippy::unwrap_used)]

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::{PointCss, SizeCss};
use ferrochart_core::interaction::{InputEvent, Modifiers};
use ferrochart_core::render::Renderer;
use ferrochart_core::testing::random_walk;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_cpu::CpuRenderer;

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "snapshot.png".into());
    let dark = std::env::args().any(|a| a == "--dark");
    let mut chart = Chart::new(ChartOptions {
        theme: if dark { Theme::dark() } else { Theme::light() },
        ..ChartOptions::default()
    });
    chart.resize(SizeCss { w: 800.0, h: 450.0 }, 2.0);
    let pane = chart.main_pane();
    let id = chart
        .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    chart
        .add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(id))
        .unwrap();
    chart
        .set_data(
            id,
            SeriesData::Ohlc(random_walk(12, 500, 1_704_067_200_000, 60_000).unwrap()),
        )
        .unwrap();
    let mut renderer = CpuRenderer::new();
    chart.build_frame(0.0, &renderer.measurer());
    chart.handle_input(InputEvent::PointerMove {
        id: 0,
        pos: PointCss { x: 430.0, y: 170.0 },
        mods: Modifiers::default(),
        time_ms: 0.0,
    });
    chart.build_frame(0.0, &renderer.measurer());
    renderer.render(chart.frame(), &chart.data_view()).unwrap();
    std::fs::write(&out, renderer.encode_png().unwrap()).unwrap();
}
