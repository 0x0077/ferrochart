<!-- source: 37e04d175b -->
# オフスクリーンとサーバーサイドのレンダリング

どちらのレンダラーもウィンドウなしで描画でき、PNGの書き出し、レポート、テストに使えます。

## GPUで描画する

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

GPUがない環境では、ソフトウェアアダプター（Linuxではlavapipe、WindowsではWARP）が使えます。

## CPUで描画する

`ferrochart-render-cpu`はtiny-skiaで同じフレームを描画し、CPU以外には何も必要としません。GPUバックエンドと同じピクセルを出力します（どちらも同じ黄金画像と照合されています）。

```rust,ignore
use ferrochart_render_cpu::CpuRenderer;

let mut renderer = CpuRenderer::new();
chart.build_frame(0.0, &renderer.measurer());
renderer.render(chart.frame(), &chart.data_view())?;
let pixmap = renderer.pixmap(); // tiny_skia::Pixmap; encode it as PNG
```

CPUレンダラーには性能目標がありません。GPUが使えない場所で使ってください。

## ピクセルサイズ

`resize`はCSSピクセル単位のサイズとデバイスピクセル比を受け取ります。画像は`size × dpr`ピクセルになります。`resize(SizeCss { w: 800.0, h: 450.0 }, 2.0)`は、テキストとラインもそれに合わせて拡大した1600 × 900の画像をレンダリングします。
