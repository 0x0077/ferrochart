<!-- source: 0c5fc454f5 -->
# 데스크톱 (Rust + winit)

`ferrochart-winit`는 창을 열고, wgpu로 렌더링하며, 마우스·트랙패드·키보드 입력을
처리합니다. `Chart`와, 필요하다면 콜백을 넘겨주면 됩니다.

```toml
[dependencies]
ferrochart-core = "0.1"
ferrochart-winit = "0.1"
```

## 데이터가 있는 창

```rust,ignore
use ferrochart_core::data::OhlcColumns;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_indicators::named::ChartExt; // add_indicator_by_name

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut chart = Chart::new(ChartOptions::default());
    let candles = chart.add_candlestick()?;
    chart.add_volume(candles)?;
    chart.set_data(candles, OhlcColumns {
        time: vec![/* UTC ms */],
        open: vec![], high: vec![], low: vec![], close: vec![],
        volume: Some(vec![]),
    })?;
    chart.add_indicator_by_name(candles, "ema", &[("n", 20.into())])?;
    ferrochart_winit::show(chart)?;
    Ok(())
}
```

`show`는 기본 제목과 크기로 창을 열고 창이 닫히면 반환합니다. 제목, 크기, 콜백
(실시간 데이터, 이벤트)이 필요하면 아래처럼 `ChartApp`과 함께
`run(WindowOptions, chart, app)`을 사용하세요. `run`은 창이 닫히면 반환합니다.
차트는 무언가 바뀌었을 때(입력, 새 데이터, 관성 스크롤)만 다시 그리므로 유휴
상태의 차트는 CPU를 쓰지 않습니다.

## 실시간 데이터

`ChartApp::update`를 구현하세요. 이벤트 루프가 돌 때마다 호출되며, 다음에 호출받고
싶은 시점을 전달받은 것과 같은 밀리초 시계 기준으로 반환합니다.

```rust,ignore
use ferrochart_core::Chart;
use ferrochart_core::data::{BarUpdate, SeriesId};
use ferrochart_winit::ChartApp;

struct Feed { series: SeriesId, bar: BarUpdate, next: f64 }

impl ChartApp for Feed {
    fn update(&mut self, chart: &mut Chart, now_ms: f64) -> Option<f64> {
        if now_ms >= self.next {
            self.bar.close += 0.01;
            self.bar.high = self.bar.high.max(self.bar.close);
            let _ = chart.update_last(self.series, self.bar);
            self.next = now_ms + 10.0; // 100 updates per second
        }
        Some(self.next)
    }
}
```

`ChartApp::on_event`는 크로스헤어, 표시 범위, 과거 데이터 이벤트를 받고,
`ChartApp::on_frame`은 렌더링된 각 프레임의 CPU 시간을 받습니다.

## 데모

```bash
cargo run -p example-desktop --release
```

1분봉 1,000만 개와 초당 100틱의 시뮬레이션 피드입니다. 드래그로 이동(튕기면 관성
스크롤), 휠이나 핀치로 확대·축소, 축을 드래그해 스케일 조정, 축을 더블클릭해
초기화, 화살표 키와 Home/End로 탐색합니다.

## 직접 만든 이벤트 루프

앱이 이미 창을 소유하고 있다면(egui, 게임 엔진) `ferrochart_winit::InputConverter`로
winit 이벤트를 차트 입력으로 바꾸고, `Chart::build_frame`과
`WgpuRenderer::render`를 직접 호출하세요.
[렌더링의 동작 방식](rendering.md)을 참고하세요.
