<!-- source: 37e04d175b -->
# 離屏與伺服器端渲染

兩種渲染器都可以在沒有視窗的情況下繪製，用於匯出 PNG、產生報表或測試。

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

在沒有 GPU 的環境中，可以使用軟體轉接器（Linux 上的 lavapipe、Windows 上的 WARP）。

## 在 CPU 上

`ferrochart-render-cpu` 以 tiny-skia 繪製相同的影格，除了 CPU 之外不需要任何東西。它產生的像素與 GPU 後端相同（兩者都以相同的黃金圖像檢驗）。

```rust,ignore
use ferrochart_render_cpu::CpuRenderer;

let mut renderer = CpuRenderer::new();
chart.build_frame(0.0, &renderer.measurer());
renderer.render(chart.frame(), &chart.data_view())?;
let pixmap = renderer.pixmap(); // tiny_skia::Pixmap; encode it as PNG
```

CPU 渲染器沒有效能目標；請在無法使用 GPU 的地方使用它。

## 像素尺寸

`resize` 接受以 CSS 像素表示的尺寸加上裝置像素比。影像為 `size × dpr` 像素：`resize(SizeCss { w: 800.0, h: 450.0 }, 2.0)` 會渲染出 1600 × 900 的影像，文字與線條也會相應縮放。
