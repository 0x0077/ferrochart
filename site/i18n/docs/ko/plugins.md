<!-- source: 8785086f6e -->
# 플러그인

FerroChart에서는 다섯 가지 방법으로 차트를 확장할 수 있습니다. 모두 프레임에
그리기 명령을 기록하는 방식이므로 모든 렌더러(WebGPU, WebGL2, Vulkan/Metal/DX12,
CPU 렌더러)에서 동작하며, 내장 시리즈와 마찬가지로 스크린샷과 골든 이미지에도
나타납니다.

| 확장 | 용도 | Rust | JavaScript |
|---|---|---|---|
| 레이어 플러그인 | 패널 또는 차트 전체에 추가로 그리기: 워터마크, 매매 표시, 세션 음영 | `LayerPlugin` | `chart.addLayer` |
| 커스텀 시리즈 | 데이터를 직접 보유하고 그리는 시리즈: 프로파일, 히트맵, 레인지 박스 | `CustomSeries` | — |
| 네이티브 GPU 시리즈 | 커스텀 시리즈 종류의 GPU 구현(컴퓨트 셰이더, 대용량 데이터) | `GpuSeries` | — |
| 시리즈 변환 | 봉으로부터 계산한 새로운 OHLC 유사 시리즈: Renko 계열 차트 | `SeriesTransform` | — |
| 드로잉 도구 | 앵커, 핸들, 히트 테스트를 갖춘 사용자가 그리는 객체 | `DrawingTool` | `chart.drawings.register` |

실행 가능한 Rust 예제는 `examples/plugins`(`cargo run -p example-plugins
--release`)입니다. 워터마크, 매매 표시, 그리고 시뮬레이션 피드로 업데이트되는
커스텀 시리즈가 들어 있습니다. 오더플로우 크레이트(`ferrochart-orderflow`,
[오더플로우](orderflow.md) 참고)는 가장 큰 플러그인으로, Rust의 모든 확장 지점을
사용합니다.

## 모든 플러그인이 따르는 규칙

- **매 프레임 할당 없이 그립니다.** `build`는 사용자가 이동하거나 확대·축소하는
  동안을 포함해 모든 프레임마다 호출됩니다. 전달받은 `FrameBuilder`에 기록하고
  (내부 풀은 용량을 유지합니다), 자체 임시 버퍼는 플러그인 안에 두고 비워서
  재사용하십시오. 보이는 부분만 그리십시오. 보이는 범위가 함께 전달됩니다.
- **패닉 없음.** 플러그인은 차트의 프레임 안에서 실행되므로, 패닉 대신 일찍
  반환하십시오.
- **좌표는 Rust에서는 물리 픽셀**(`Px`, `PointPx`, `RectPx`), JavaScript에서는
  CSS 픽셀입니다. Rust에서는 `ctx.px(Css(…))`로 CSS 크기를 변환합니다.
- **색상은 스타일 또는 옵션에서 가져옵니다.** `ctx.style()`(테마에 맞춰 결정된
  시리즈 색상)이나 옵션으로 받은 색상을 사용하면 플러그인이 라이트·다크 테마를
  따라갑니다.
- **변경 사항을 차트에 알립니다.** 내용이 바뀌는 레이어는 `invalidate_layer`를
  호출하고, 커스텀 시리즈는 `update_custom`을 통해서만 데이터를 변경합니다.

## 레이어 플러그인

레이어는 하나의 패널(플롯 영역) 또는 차트 전체에, 네 가지 깊이 중 하나로
그립니다.

| `LayerPosition` | 그려지는 위치 |
|---|---|
| `Background` | 그리드 위, 다른 모든 것의 뒤 |
| `BelowSeries` | 시리즈 아래 |
| `AboveSeries` | 시리즈와 마커 위, 드로잉과 레이블 아래 |
| `Overlay` | 오버레이 레이어, 크로스헤어 아래 |

```rust
use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::frame::TextAlign;
use ferrochart_core::geom::{Css, PointPx, Rgba};
use ferrochart_core::plugin::{LayerCtx, LayerPlugin, LayerPosition};

struct Watermark(&'static str);

impl LayerPlugin for Watermark {
    fn position(&self) -> LayerPosition {
        LayerPosition::Background
    }

    fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
        let r = ctx.rect();
        let center = PointPx { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 };
        let label = Label {
            font: ctx.font(Css(48.0)),
            color: Rgba::hex(0x80_8080).with_alpha(0x40),
            align: TextAlign::Center,
            bg: None,
        };
        out.text(center, self.0, label);
    }
}

// None: the whole chart; Some(pane): that pane's plot.
let id = chart.add_layer(None, Box::new(Watermark("BTCUSD")))?;
```

패널 위의 레이어에서는 `ctx.coords()`로 해당 패널의 시간·가격 변환(`x_of`,
`y_of`, `time_at`, `price_at`, `px`, `plot`, 가격 포맷)을 사용할 수 있습니다.
예제의 `TradeMarks`처럼 `time_at(plot.x)`와 `time_at(plot.right())`, 그리고
이진 탐색으로 데이터 중 보이는 부분을 찾으십시오.

레이어 자체의 데이터가 바뀌면 `chart.invalidate_layer(id)`를 호출해 다음
프레임에서 다시 그리게 합니다. `chart.remove_layer(id)`로 제거합니다.

### JavaScript에서

```js
const id = chart.addLayer({
  pane: 0,                  // omit for the whole chart
  position: 'aboveSeries',  // 'background' | 'belowSeries' | 'aboveSeries' | 'overlay'
  build(ctx) {
    const x = ctx.timeToX(entryTime);
    const y = ctx.priceToY(entryPrice);
    if (x === null || y === null) return [];
    return [
      { type: 'line', x1: ctx.rect.x, y1: y, x2: x, y2: y, color: '#2962FF', dash: 'dashed' },
      { type: 'text', x: x + 4, y, text: ctx.formatPrice(entryPrice), color: '#2962FF', align: 'left' },
    ];
  },
});
chart.invalidateLayer(id); // after the plugin's own data changed
chart.removeLayer(id);
```

`build(ctx)`는 캔버스 왼쪽 위 모서리 기준 CSS 픽셀 단위의 명령 배열을
반환합니다. 명령은 `rect`, `line`, `polyline`, `polygon`, `text`, `icon`이
있습니다(각 필드는 `PluginCommand` 타입에 나와 있습니다). `icon`은 마커와 같은
추가 필드를 받습니다. `image`(`chart.registerIcon`으로 등록한 이름), `label`,
`border`, `borderWidth`가 있으므로, 레이어는 봉 위뿐 아니라 어디에든 아바타를
그릴 수 있습니다([마커](markers.md) 참고). `ctx` 함수
(`timeToX`, `priceToY`, `xToTime`, `yToPrice`, `textWidth`, `formatPrice`)는
호출 중에만 동작합니다. 예외를 던지거나 잘못된 명령을 반환한 플러그인은 해당
프레임에서 아무것도 그리지 않습니다(잘못된 명령 앞의 명령은 유지됩니다). 첫 번째
오류는 `console.warn`으로 기록됩니다.

JavaScript 레이어는 프레임마다 한 번 wasm 경계를 넘어 호출되므로, 명령 수는
보이는 만큼(수십만이 아니라 수백 개)으로 유지하십시오. 대용량 데이터라면 Rust로
커스텀 시리즈를 작성하십시오.

## 커스텀 시리즈

커스텀 시리즈는 자신의 데이터를 직접 보유합니다. 차트는 시리즈에 시간과 가격
극값을 요청하고, 이를 시간 축에 배치하고, 가격 축을 그에 맞춰 조정한 뒤, 보이는
포인트를 그리게 합니다.

```rust
use ferrochart_core::chart::SeriesOptions;
use ferrochart_core::data::{DataChange, Timestamp};
use ferrochart_core::drawing::FrameBuilder;
use ferrochart_core::geom::RectPx;
use ferrochart_core::plugin::{AxisMode, CustomSeries, MinMaxIndex, SeriesCtx};

/// A vertical bar from `lo` to `hi` per point.
#[derive(Default)]
struct Ranges {
    time: Vec<Timestamp>,
    range: Vec<(f64, f64)>,
    index: MinMaxIndex,
}

impl Ranges {
    fn push(&mut self, t: Timestamp, lo: f64, hi: f64) -> DataChange {
        self.time.push(t);
        self.range.push((lo, hi));
        let _ = self.index.push(lo, hi);
        DataChange::Append { count: 1 }
    }
}

impl CustomSeries for Ranges {
    fn kind(&self) -> &'static str {
        "example.ranges"
    }
    fn axis_mode(&self) -> AxisMode {
        AxisMode::Slots
    }
    fn times(&self) -> &[Timestamp] {
        &self.time
    }
    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        self.index.query(from, to)
    }
    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
        let w = (ctx.bar_spacing().0 * 0.6).max(1.0);
        for i in from..to {
            let (lo, hi) = self.range[i];
            let (Some(x), Some(y0), Some(y1)) =
                (ctx.x_of_point(i), ctx.coords().y_of(hi), ctx.coords().y_of(lo))
            else {
                continue;
            };
            out.rect(RectPx { x: x - w / 2.0, y: y0, w, h: (y1 - y0).max(1.0) }, ctx.style().up);
        }
    }
}

let id = chart.add_custom_series(chart.main_pane(), Box::new(Ranges::default()), SeriesOptions::default())?;
chart.update_custom(id, |r: &mut Ranges| r.push(60_000, 1.0, 2.0))?;
let ranges: Option<&Ranges> = chart.custom_series::<Ranges>(id);
```

### 계약

| 메서드 | 호출 시점 | 해야 할 일 |
|---|---|---|
| `kind` | 추가될 때 | 고유한 이름(예: `"myorg.profile"`)을 반환합니다. 네이티브 GPU 구현은 이 이름으로 매칭됩니다 |
| `axis_mode` | 추가될 때 | `Slots` 또는 `Overlay`(아래 참고)를 반환합니다 |
| `times` | 변경될 때마다 | 순증가하는 시간(UTC ms)을 반환합니다. 포인트 `i`의 시간은 `times()[i]`입니다 |
| `extremes(from, to)` | 매 프레임 | 포인트 `from..to`의 가격 범위를 O(log n)으로 반환합니다. `MinMaxIndex`를 사용하십시오 |
| `build(ctx, from, to, out)` | 매 프레임 | 포인트 `from..to`(보이는 포인트와 양쪽으로 하나씩 더)를 할당 없이 그립니다 |
| `last_value` | 매 프레임 | 마지막 값 레이블의 값(`None`: 레이블 없음) |
| `crosshair_value(i)` | 크로스헤어가 움직일 때 | 포인트 `i`에 표시할 값 |
| `native_params` | 매 프레임, 네이티브 경로에서만 | GPU 구현에 넘길 매개변수를 최대 8개 기록합니다 |

**축 모드.** `Slots`: 내장 시리즈의 봉처럼 각 포인트가 시간 축의 슬롯 하나를
차지합니다(캔들마다 풋프린트 열 하나). `Overlay`: 포인트가 시간에 따라 기존
슬롯 사이에 배치되며 슬롯을 추가하지 않습니다(1분 차트 위의 1초 단위 히트맵 열).

**데이터 변경.** 플러그인은
`chart.update_custom(id, |s: &mut MySeries| { …; change })`를 통해서만
변경하며, 무엇이 바뀌었는지 반환합니다.

| `DataChange` | 의미 |
|---|---|
| `UpdateLast` | 마지막 포인트가 바뀌었지만 시간은 그대로(할당 없음: 실시간 경로) |
| `Append { count }` | 끝에 `count`개의 포인트가 추가됨(`count: 0`: 변경 없음) |
| `Prepend { count }` | 앞에 `count`개의 포인트가 추가됨(과거 데이터) |
| `Replace` | 그 밖의 모든 경우 |

데이터와 맞지 않는 변경(개수가 틀리거나 마지막 시간이 바뀐 경우)은 `Replace`로
적용되므로, 잘못 알려도 시간이 더 걸릴 뿐 차트가 손상되지는 않습니다. 시간이
순증가하지 않게 되면 `TimeNotIncreasing`이 반환되고, 유효한 변경이 있을 때까지
시리즈에는 아무것도 표시되지 않습니다.

**극값.** `MinMaxIndex`는 범위 최솟값·최댓값 질의에 O(log n)으로 답하며,
`push` / `set_last`로 최신 상태를 유지합니다. 시리즈마다 하나씩 사용하고,
데이터를 변경하는 클로저 안에서 함께 업데이트하십시오.

**그리기.** `ctx.x_of_point(i)`는 포인트 `i`의 중심입니다
(`SeriesOptions::shift` 반영). `ctx.coords()`는 가격과 시간을 변환하고, 폰트를
제공하며, 가격을 포맷합니다. `ctx.bar_spacing()`은 슬롯 사이의 간격이고,
`ctx.style()`에는 시리즈 색상이 들어 있습니다. `FrameBuilder`는 사각형, 선,
폴리라인, 다각형, 텍스트, 아이콘을 기록합니다. 정수 픽셀에 맞춘 사각형은 어떤
장치 픽셀 비율에서도 선명하게 보입니다.

커스텀 시리즈는 변환 시리즈와 같은 차트에 함께 있을 수 없습니다(둘 다 시간 축의
구성 방식을 결정하기 때문입니다).

## 네이티브 GPU 시리즈

이식 가능한 `build` 경로는 어디서나 동작합니다. 셀이 100만 개인 히트맵처럼
시리즈의 데이터가 이 경로로 처리하기에 너무 많을 때는, 네이티브 구현이 대신
GPU에서 그릴 수 있습니다. 이때 프레임에는 해당 시리즈의 프리미티브 대신
`DrawCmd::Custom` 하나가 들어갑니다.

```rust
use ferrochart_render_wgpu::{GpuSeries, GpuSeriesCtx};
use ferrochart_core::chart::ChartDataView;
use ferrochart_core::frame::CustomDraw;

struct MySeriesGpu { /* pipelines, per-series buffers */ }

impl GpuSeries for MySeriesGpu {
    fn kind(&self) -> &'static str {
        "myorg.profile" // the CustomSeries::kind it draws
    }
    fn prepare(&mut self, ctx: &GpuSeriesCtx<'_>, draw: &CustomDraw, data: &ChartDataView<'_>) {
        // Upload what changed since the last frame:
        //   data.changes_since(draw.series, synced_generation)
        // Read the plugin: data.custom(draw.series), downcast through &dyn Any.
        // Compute work may be submitted to ctx.queue here.
    }
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, draw: &CustomDraw) {
        // Set the pipeline and bind groups and draw; the scissor is already
        // the pane's plot. Do not change the scissor or viewport.
    }
}

renderer.register_series(Box::new(MySeriesGpu { /* … */ }));
// Before each frame (the winit and web hosts do this for you):
chart.set_native_kinds(renderer.native_kinds());
```

`ferrochart-winit`을 사용한다면 GPU 렌더러가 생성될 때마다 호출되는
`ChartApp::on_renderer`에서 등록하십시오.

네이티브 구현의 규칙:

- **이식 가능한 경로와 같은 출력.** 레이아웃, 임계값, 색상 범위는 플러그인
  (코어 측)에서 계산하고 `native_params`와 `data.custom`으로 전달하십시오.
  셰이더는 이를 적용하기만 합니다. 두 경로 모두 같은 골든 이미지로 테스트하십시오.
- **장치를 확인하십시오.** WebGL2에는 컴퓨트 셰이더와 스토리지 버퍼가 없습니다.
  필요한 기능을 장치가 지원할 때만 등록하십시오. 등록되지 않은 종류는 자동으로
  이식 가능한 경로를 사용합니다.
- **증분 업로드.** `CustomDraw::generation`과 `data.changes_since`로 무엇이
  바뀌었는지 알 수 있습니다. 실시간 업데이트는 보통 마지막 포인트만 건드립니다.

`ferrochart_orderflow::register_gpu`가 완전한 구현 예입니다. 컴퓨트 셰이더가
히트맵 셀을 픽셀 단위로 집계하고, 쿼드 하나가 색을 칠합니다.

## 시리즈 변환

변환은 내장 Renko, Range Bars, Kagi, Point & Figure처럼 메인 시리즈의 봉을 새로운
OHLC 유사 시리즈로 바꿉니다. 입력 봉마다 한 번 호출되며, `peek`은 아직 형성 중인
마지막 봉이 무엇을 만들어 낼지 보여 줍니다.

| 메서드 | 해야 할 일 |
|---|---|
| `name` | 이름(예: `"renko"`)을 반환합니다 |
| `output_kind` | `Candlestick`, `OhlcBar`, `Kagi`, `PointFigure` 중 하나를 반환합니다 |
| `push(bar, out)` | `bar`를 확정하고, 그로 인해 완성되는 요소를 `out`에 기록합니다 |
| `peek(bar, out)` | 상태를 바꾸지 않고, `bar`가 완성할 요소와 아직 형성 중인 요소를 기록합니다 |
| `reset` | 모든 입력을 잊습니다 |
| `box_size` | Point & Figure 전용: 박스 하나의 가격 높이 |

```rust
let id = chart.add_transform_series(pane, Box::new(MyTransform::new()), SeriesOptions::default())?;
chart.set_data(id, SeriesData::Ohlc(bars))?;   // input bars
chart.update_last(id, tick)?;                   // live input
```

변환 시리즈의 x축은 시간이 아니라 자체 출력이므로, 다른 시간 기반 시리즈와 같은
차트에 함께 있을 수 없습니다.

## 드로잉 도구

드로잉 도구는 사용자가 마우스로 배치하는 객체입니다. 앵커(시간, 가격), 편집용
핸들, 히트 테스트, 스타일, 그리고 실행 취소/다시 실행과 상태 파일을 위한 저장
형식을 갖습니다.

### Rust에서

`DrawingTool`(`kind`, `anchors`, `anchors_mut`, `anchor_count`,
`build`, `hit_test`, `handles`, `style`, `save`; 선택적으로 `move_handle`)을
구현하고, 저장 형식에서 도구를 다시 만드는 팩토리를 등록합니다.

```rust
fn my_tool(data: &DrawingData) -> ferrochart_core::Result<Box<dyn DrawingTool>> {
    Ok(Box::new(MyTool::from_data(data)?))
}

chart.drawings().register("myorg.zone", my_tool);
```

`AnchorCount`는 생성 방식을 결정합니다. 정해진 횟수의 클릭, 더블 클릭이나
Enter로 끝내는 가변 개수, 또는 자유 그리기 중 하나입니다. 내장 도구는
`ferrochart_core::drawing::BUILTIN_KINDS`에 나열되어 있으며 좋은 참고 자료입니다.

### JavaScript에서

```js
chart.drawings.register('myorg.zone', {
  anchorCount: 2, // or { open: 3 } or 'freehand'
  build(ctx) {
    const [a, b] = ctx.anchors;
    if (a.x === null || b.x === null) return [];
    return [{
      type: 'rect',
      x: Math.min(a.x, b.x), y: Math.min(a.y, b.y),
      width: Math.abs(b.x - a.x), height: Math.abs(b.y - a.y),
      color: ctx.style.fill ?? 'rgba(41, 98, 255, 0.2)',
    }];
  },
  // Optional; default: hit inside the drawn bounds.
  hitTest(ctx, point) {
    return false; // true: the drawing; a number: that anchor's handle
  },
});
chart.drawings.startTool('myorg.zone');
```

JavaScript로 만든 종류는 `drawings.register`를 호출한 차트에서 사용할 수
있습니다(`add`, `startTool`, `loadState`에서). 필요한 차트마다 등록하십시오.
같은 종류를 다시 등록하면 페이지의 모든 차트에서 구현이 교체됩니다. 내장 종류는
교체할 수 없습니다.

## 플러그인 테스트

- **골든 이미지.** `ferrochart-render-cpu`(결정적이며 GPU 불필요)로 고정된 장면을
  렌더링하고 저장된 PNG와 비교합니다. 네이티브 구현이 있다면
  `ferrochart-render-wgpu`로도 렌더링하여 두 경로를 같은 이미지와 비교하십시오.
- **할당 없음.** 몇 프레임 워밍업한 다음, 이동, 확대·축소, 크로스헤어 이동
  (`build_frame`) 중과 마지막 포인트를 업데이트하는 동안의 할당 횟수를 셉니다.
  횟수는 0이어야 합니다. 카운팅 할당자는 `ferrochart-orderflow/tests/zero_alloc.rs`에
  나와 있습니다.
- **헤드리스 프레임.** `ferrochart_core::testing::MonoMeasurer`(기능
  `testing`)를 사용하면 단위 테스트에서 폰트나 GPU 없이 `chart.build_frame`을
  호출할 수 있습니다.
