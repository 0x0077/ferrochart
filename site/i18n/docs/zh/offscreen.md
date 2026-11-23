<!-- source: 37e04d175b -->
# 离屏与服务端渲染

两种渲染器都可以在没有窗口的情况下绘制，用于导出 PNG、生成报告或测试。

## 在 GPU 上

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

在没有 GPU 的地方，可以使用软件适配器（Linux 上的 lavapipe、Windows 上的 WARP）。

## 在 CPU 上

`ferrochart-render-cpu` 用 tiny-skia 绘制相同的帧，只需要 CPU。它产生的像素与 GPU 后端相同（两者都与同一组黄金图像进行比对）。

```rust,ignore
use ferrochart_render_cpu::CpuRenderer;

let mut renderer = CpuRenderer::new();
chart.build_frame(0.0, &renderer.measurer());
renderer.render(chart.frame(), &chart.data_view())?;
let pixmap = renderer.pixmap(); // tiny_skia::Pixmap; encode it as PNG
```

CPU 渲染器没有性能目标；请在没有 GPU 可用的地方使用它。

## 像素尺寸

`resize` 接受以 CSS 像素表示的尺寸以及设备像素比。图像大小为 `size × dpr` 像素：`resize(SizeCss { w: 800.0, h: 450.0 }, 2.0)` 会渲染出 1600 × 900 的图像，文字和线条也会相应缩放。
