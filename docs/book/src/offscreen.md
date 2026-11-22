# Offscreen and server-side rendering

Both renderers can draw without a window, for PNG export, reports or tests.

## On the GPU

```rust,ignore
use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::SizeCss;
use ferrochart_core::render::Renderer;
use ferrochart_core::testing::random_walk; // feature "testing"
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

let ctx = GpuContext::headless()?;
let mut renderer = WgpuRenderer::offscreen(&ctx);
let mut chart = Chart::new(ChartOptions::default());
chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
let id = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
chart.set_data(id, SeriesData::Ohlc(random_walk(1, 1000, 0, 60_000)?))?;
chart.build_frame(0.0, &renderer.measurer());
renderer.render(chart.frame(), &chart.data_view())?;
let rgba: Vec<u8> = renderer.read_pixels()?; // 800 × 450 × 4 bytes
```

A software adapter (lavapipe on Linux, WARP on Windows) works where there is
no GPU.

## On the CPU

`ferrochart-render-cpu` draws the same frames with tiny-skia and needs nothing
but the CPU. It produces the same pixels as the GPU backend (both are checked
against the same golden images).

```rust,ignore
use ferrochart_render_cpu::CpuRenderer;

let mut renderer = CpuRenderer::new();
chart.build_frame(0.0, &renderer.measurer());
renderer.render(chart.frame(), &chart.data_view())?;
let pixmap = renderer.pixmap(); // tiny_skia::Pixmap; encode it as PNG
```

The CPU renderer has no performance targets; use it where a GPU is not
available.

## Pixel size

`resize` takes the size in CSS pixels plus the device pixel ratio. The image is
`size × dpr` pixels: `resize(SizeCss { w: 800.0, h: 450.0 }, 2.0)` renders
1600 × 900 with text and lines scaled to match.
