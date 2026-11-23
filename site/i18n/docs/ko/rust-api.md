<!-- source: f3b74de278 -->
# Rust API

API 문서는 rustdoc으로 생성됩니다. 로컬에서는 다음과 같이 확인합니다.

```bash
cargo doc --workspace --exclude ferrochart-py --no-deps --open
```

docs.rs에 게시된 문서:

| 크레이트 | 내용 |
|---|---|
| [`ferrochart-core`](https://docs.rs/ferrochart-core) | `Chart`, 데이터 모델, 스케일, 입력, `Frame`, `Renderer` 트레이트, `transform_helpers` |
| [`ferrochart-render-wgpu`](https://docs.rs/ferrochart-render-wgpu) | `GpuContext`, `WgpuRenderer`(서피스 또는 오프스크린) |
| [`ferrochart-render-cpu`](https://docs.rs/ferrochart-render-cpu) | `CpuRenderer`(tiny-skia) |
| [`ferrochart-text`](https://docs.rs/ferrochart-text) | `TextEngine`, 번들 폰트 |
| [`ferrochart-winit`](https://docs.rs/ferrochart-winit) | `run`, `ChartApp`, `InputConverter` |
| [`ferrochart-indicators`](https://docs.rs/ferrochart-indicators) | 증분 지표 |
| [`ferrochart-orderflow`](https://docs.rs/ferrochart-orderflow) | 풋프린트, 히트맵, TPO 시리즈([오더플로우](orderflow.md)) |

차트 확장(레이어, 커스텀 시리즈, 네이티브 GPU 시리즈, 변환, 드로잉 도구)은
[플러그인](plugins.md)에서 설명합니다.

## 어디서부터 시작할까

- `ferrochart_core::Chart`: `ChartOptions`로 생성하고, `resize`하고, 시리즈를
  추가하고, 데이터를 설정하고, `handle_input`으로 `InputEvent`를 전달하고,
  `build_frame`을 호출한 뒤 `drain_events`로 이벤트를 읽습니다.
- `ferrochart_core::render::Renderer`: 두 렌더러 모두 이 트레이트를 구현합니다.
  `render(chart.frame(), &chart.data_view())`를 호출하면 됩니다.

## API가 따르는 규칙

- **패닉 없음.** 실패할 수 있는 메서드는 `Result`를 반환하며, id로 조회하는
  메서드는 `Result` 또는 `Option`을 반환합니다.
- **핫 패스에서 할당 없음.** 이동, 확대·축소, 크로스헤어 이동(`build_frame`)과
  `update_last`는 워밍업 이후 아무것도 할당하지 않습니다. 테스트에서 카운팅
  할당자로 이를 검증합니다.
- **타입이 있는 좌표.** `Css`와 `Px`(그리고 이를 기반으로 한 점, 크기, 사각형
  타입)는 CSS 픽셀과 장치 픽셀을 구분합니다. 장치 픽셀 비율은 `Chart::resize`에
  전달합니다.
- **순수한 코어.** `ferrochart-core`는 표준 라이브러리 외의 의존성이 없습니다
  (허용되는 것은 `smallvec`뿐입니다). GPU, 창, 폰트, 시간, 비동기 라이브러리를
  쓰지 않으며 `unsafe`도 없습니다.
