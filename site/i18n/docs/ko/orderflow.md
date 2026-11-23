<!-- source: 118e97bde8 -->
# 오더플로우

`ferrochart-orderflow`는 체결과 호가창으로 만드는 시리즈 세 가지를 추가합니다.
이들은 [커스텀 시리즈](plugins.md#custom-series)이므로 캔들과 시간 축, 가격 축,
크로스헤어, 패널을 공유합니다.

| 시리즈 | 표시 내용 | 입력 |
|---|---|---|
| **풋프린트** | 캔들마다 각 가격 레벨에서 매도·매수된 거래량 | 체결 |
| **히트맵** | 시간 열과 가격 레벨별 호가 잔량(또는 체결 거래량) | 호가창 스냅샷 또는 체결 |
| **TPO**(마켓 프로파일) | 세션마다 각 가격에서 거래된 기간, POC, 밸류 에어리어, 이니셜 밸런스 | 캔들 |

세 가지 모두 모든 렌더러에서 동작합니다. WebGPU와 네이티브 GPU에서는 히트맵이
컴퓨트 셰이더로 셀을 집계합니다. WebGL2와 CPU 렌더러에서는 이식 가능한 경로를
사용하며, 결과는 같습니다.

## 공통 규칙

- **가격 레벨.** 가격은 `step = tickSize × rowSize` 간격의 격자로 반올림됩니다.
  레벨은 `round(price / step)`이며 0.5는 올림합니다. 레벨은 중심 가격 기준
  `±step / 2` 범위를 차지합니다.
- **시간 열**은 UTC 기준 주기의 배수에서 시작합니다(풋프린트의 타임프레임,
  히트맵의 열 길이).
- **체결**은 길이가 같은 열들입니다. 시간(UTC ms, 감소하지 않음), 가격,
  수량(≥ 0, 0은 무시), 그리고 선택 사항인 방향입니다. 배치는 먼저 검사되며,
  유효하지 않으면 차트는 바뀌지 않습니다. 마지막 열에 속하는 체결은 그 열을
  갱신하고, 그 이후의 체결은 새 열을 엽니다. 마지막 열보다 이른 체결은 거부됩니다.
- **공격 주체 방향.** 매수와 매도는 주어진 대로 사용합니다. 방향을 모르는 체결은
  틱 규칙을 따릅니다. 직전 체결가보다 높으면 매수, 낮으면 매도, 같으면 직전
  체결의 방향입니다(첫 체결은 매수로 셉니다).

## 풋프린트

```js
const chart = await FerroChart.create(canvas, { maxBarSpacing: 200 });
const candles = chart.addCandlestickSeries();
const footprint = chart.addFootprint({
  tickSize: 0.25,
  timeframeMs: 60_000,   // the candles' period
  rowSize: 4,            // ticks per row
  mode: 'bidAsk',        // 'bidAsk' | 'delta' | 'volume'
});
footprint.appendTrades({
  time: new Float64Array([...]),
  price: new Float64Array([...]),
  size: new Float64Array([...]),
  side: new Int8Array([...]), // 1 buy, -1 sell, 0 unknown; optional
});
```

그려지는 내용은 봉 간격에 따라 달라집니다.

| 봉 간격(CSS px) | 그려지는 것 |
|---|---|
| < 12 | 없음(캔들은 계속 보임) |
| 12 – 60 | 음영 셀 |
| ≥ 60 | 셀과 숫자 |

기본 `maxBarSpacing`은 50이므로, 숫자가 보일 때까지 확대하려면 더 큰 값으로
차트를 만드세요.

- **모드.** `bidAsk`: 왼쪽 절반에 매도 거래량, 오른쪽에 매수 거래량. `delta`:
  매수 − 매도, 부호에 따라 색상 지정. `volume`: 합계. 음영 강도는 같은 열의 최댓값
  대비 상대적입니다.
- **불균형(imbalance)**은 대각선으로 비교합니다. 한 레벨의 매수가 한 레벨 아래
  매도의 `imbalanceRatio`배 이상이면 매수 불균형(오른쪽 끝에 막대)입니다. 한
  레벨의 매도가 한 레벨 위 매수의 ratio배 이상이면 매도 불균형(왼쪽 끝에
  막대)입니다. 기본 비율은 3이며, `null`이면 불균형 표시를 끕니다.
- **POC.** `showPoc`(기본값)이면 각 열에서 거래량이 가장 많은 레벨에 테두리가
  그려집니다.
- 색상은 기본적으로 테마의 상승/하락/선/텍스트 색상을 따르며,
  `colors: { buy, sell, volume, text, poc }`로 재정의할 수 있습니다.

## 히트맵

```js
const book = chart.addHeatmap({
  source: 'depth',       // or 'trades' (fed with appendTrades)
  tickSize: 0.25,
  columnMs: 1_000,       // may be finer than the candles
});
book.appendDepth({ time: Date.now(), prices: bookPrices, sizes: bookSizes });
```

- 열은 캔들 슬롯 사이에 시간 기준으로 배치되므로, 1초 열이 1분 캔들 아래에 놓일
  수 있습니다. 열은 슬롯을 추가하지 않습니다.
- **호가 잔량.** 한 스냅샷 안에서 같은 레벨에 들어가는 인접 가격은 합산됩니다.
  한 열 안에서 각 레벨은 여러 스냅샷 중 가장 큰 수량을 유지합니다.
- **체결.** 수량은 열과 레벨별로 합산되며, 방향은 사용하지 않습니다.
- **색상.** 작은 값부터 큰 값까지 다섯 개의 색상 정지점(`colors`, 알파 포함)을
  `log`(기본값) 또는 `linear` 스케일로 사용합니다. 범위는 기본적으로 보이는 셀의
  `{ percentile: 0.99 }`이며, 이동 중 색상이 깜박이지 않도록 양자화됩니다.
  `{ lo, hi }`로 고정할 수도 있습니다.
- 호가창은 캔들보다 훨씬 넓은 가격 범위에 걸치므로 히트맵은 기본적으로 자동
  스케일에 참여하지 않습니다. 포함하려면 `autoScale: true`를 설정하세요.
- 픽셀 행당 8개보다 많은 레벨이 보이면 아무것도 그리지 않습니다. 확대하거나 더 큰
  `rowSize`를 사용하세요.

## TPO

```js
const chart = await FerroChart.create(canvas, { maxBarSpacing: 600 });
const tpo = chart.addTpo({
  tickSize: 0.25,
  rowSize: 4,
  periodMs: 30 * 60_000,          // one letter per period (default 30 min)
  sessionOffsetMs: 14.5 * 3_600_000, // New York cash session: 14:30 UTC…
  sessionMs: 6.5 * 3_600_000,        // …for 6.5 hours
});
tpo.appendBars(oneMinuteBars);     // time, high and low are used
```

- **세션**은 UTC 하루에 한 번 `sessionOffsetMs`에 시작해 `sessionMs` 동안
  이어집니다(한 기간부터 하루까지, 기본값: UTC 하루). 각 세션은 시간 축의 슬롯
  하나입니다. 어느 세션에도 속하지 않는 봉은 건너뜁니다.
- **기간**에는 A–Z, 이어서 a–z 문자가 붙습니다. 봉은 시작 시간이 속한 기간에
  속합니다. 실시간 봉을 다시 보내면 그 기간의 범위가 넓어집니다.
- 세션별 **통계**:
  - **POC**: TPO가 가장 많은 레벨. 동률이면 세션 중앙에 가장 가까운 레벨, 그다음
    낮은 레벨이 선택됩니다.
  - **밸류 에어리어**: TPO의 `valueArea`(기본값 70%)이며, POC에서 시작해 TPO가 더
    많은 쪽으로 두 레벨씩 넓혀 갑니다(동률이면 위쪽).
  - **이니셜 밸런스**: 처음 두 기간의 범위.
- **그리기.** 보이는 모든 세션의 가장 넓은 행이 읽을 수 있는 글꼴 크기로 들어가면
  문자로, 그렇지 않으면 히스토그램으로 그립니다. 프레임마다 하나를 선택합니다.
  이니셜 밸런스는 왼쪽의 막대로 표시되고, 밸류 에어리어 밖의 행은 흐리게
  표시됩니다.

## Rust

```rust
use ferrochart_core::chart::SeriesOptions;
use ferrochart_orderflow::{Footprint, FootprintOptions, Side, TickColumns, append_trades};

let fp = Footprint::new(FootprintOptions::new(0.25, 60_000, &theme))?;
let id = chart.add_custom_series(chart.main_pane(), Box::new(fp), SeriesOptions::default())?;
append_trades(&mut chart, id, &TickColumns {
    time, price, size,
    side: Vec::new(), // all unknown: tick rule
})?;
```

| 시리즈 | 옵션 | 데이터 |
|---|---|---|
| `Footprint` | `FootprintOptions::new(tick_size, timeframe_ms, &theme)` | `append_trades` |
| `Heatmap` | `HeatmapOptions::new(tick_size, column_ms, HeatmapSource::Depth \| Trades)` | `append_depth(&DepthSnapshot)` / `append_trades` |
| `Tpo` | `TpoOptions::new(tick_size, &theme)` | `append_bars(&OhlcColumns)` |

풋프린트 숫자와 TPO 문자를 보려면 `ChartOptions::max_bar_spacing`을 설정하세요.
`ferrochart-render-wgpu`를 사용할 때는 `ferrochart_orderflow::register_gpu(&ctx,
&mut renderer)`를 한 번 호출하세요(`ferrochart-winit`에서는
`ChartApp::on_renderer` 안에서). 이 함수는 디바이스가 네이티브 히트맵을
지원하는지 여부를 반환합니다. 기본 feature인 `gpu`가 이를 제공하며, 이 feature가
없으면 크레이트는 wgpu에 의존하지 않습니다.

세션 통계는 `Tpo::profile(i)`(POC, 밸류 에어리어, 이니셜 밸런스)로, 원시 셀은
`Footprint::grid()`와 `Heatmap::grid()`로 얻을 수 있습니다.

## 성능

체결 1,000,000건에 대한 1분 풋프린트와 1초 체결 히트맵의 프레임당 시간입니다
(Apple M4 Max, 1600×900 CSS px, 2배율, 네이티브 히트맵).

| 시나리오 | p50 | p99 |
|---|---|---|
| 이동, 풋프린트 숫자 | 0.47 ms | 1.6 ms |
| 이동, 풋프린트 셀 | 0.65 ms | 1.6 ms |
| 확대·축소, 풋프린트 + 히트맵 | 0.78 ms | 3.3 ms |
| 이동, 모든 요소 표시 | 3.9 ms | 6.8 ms |
| 이동, TPO 문자 | 0.28 ms | 1.1 ms |

마지막 열에 이미 있는 가격의 실시간 체결, 마지막 열로 들어가는 호가창 스냅샷,
마지막 TPO 세션의 실시간 봉은 메모리를 할당하지 않습니다. 새 가격 레벨과 새 열은
캔들 추가와 마찬가지로 할당합니다.

`cargo bench -p ferrochart-bench --bench orderflow`로 재현할 수 있습니다.
