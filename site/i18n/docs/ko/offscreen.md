<!-- source: 37e04d175b -->
# 오프스크린과 서버 사이드 렌더링

두 렌더러 모두 창 없이 그릴 수 있으므로 PNG 내보내기, 리포트, 테스트에 쓸 수
있습니다.

## GPU에서

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

GPU가 없는 환경에서는 소프트웨어 어댑터(Linux의 lavapipe, Windows의 WARP)로
동작합니다.

## CPU에서

`ferrochart-render-cpu`는 같은 프레임을 tiny-skia로 그리며 CPU 외에는 아무것도
필요하지 않습니다. GPU 백엔드와 같은 픽셀을 만들어 냅니다(둘 다 같은 골든
이미지로 검사합니다).

```rust,ignore
use ferrochart_render_cpu::CpuRenderer;

let mut renderer = CpuRenderer::new();
chart.build_frame(0.0, &renderer.measurer());
renderer.render(chart.frame(), &chart.data_view())?;
let pixmap = renderer.pixmap(); // tiny_skia::Pixmap; encode it as PNG
```

CPU 렌더러에는 성능 목표가 없습니다. GPU를 쓸 수 없는 곳에서 사용하세요.

## 픽셀 크기

`resize`는 CSS 픽셀 단위의 크기와 디바이스 픽셀 비율을 받습니다. 이미지는
`size × dpr` 픽셀입니다. `resize(SizeCss { w: 800.0, h: 450.0 }, 2.0)`은
텍스트와 선도 그에 맞게 확대해 1600 × 900으로 렌더링합니다.
