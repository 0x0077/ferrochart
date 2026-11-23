<!-- source: 32f28d0f34 -->
# 지표

FerroChart에는 30가지 기술적 지표가 내장되어 있습니다. 지표는 원본 시리즈를
읽어 하나 이상의 출력 시리즈를 계산하고, 원본이 바뀔 때마다 출력을 최신 상태로
유지합니다. 출력은 일반 시리즈이므로, 다른 시리즈와 마찬가지로 스타일을 지정하고
값을 읽고 마커를 붙일 수 있습니다.

## 지표 추가

```ts
const sma = chart.addIndicator(candles, 'sma', { n: 50 });
const boll = chart.addIndicator(candles, 'boll', { n: 20, k: 2 });
const macd = chart.addIndicator(candles, 'macd', { fast: 12, slow: 26, signal: 9 });
const rsi = chart.addIndicator(candles, 'rsi');        // all defaults
```

`chart.addIndicator(source, name, params?, placement?)`는 `Indicator`를
반환합니다.

| 인수 | 의미 |
|---|---|
| `source` | 계산의 기준이 되는 시리즈. 거래량 시리즈와 커스텀 시리즈를 제외한 모든 시리즈. |
| `name` | 아래 표에 있는 이름 중 하나(예: `'sma'`, `'stochRsi'`). `snake_case` 표기(`'stoch_rsi'`, `'volume_ma'`)도 허용됩니다. |
| `params` | camelCase로 쓴 이름별 매개변수. 생략한 매개변수는 기본값을 사용합니다. |
| `placement` | 출력을 배치할 위치. 기본값은 `'default'`. |

지표는 `addIndicator`가 반환되기 전에 기존 데이터 전체에 대해 한 번에
계산됩니다.

### 배치

```ts
type Placement = 'default' | 'overlay' | 'newPane' | 'volume' | number;
```

| 값 | 출력이 배치되는 곳 |
|---|---|
| `'default'` | 지표의 일반적인 위치: 이동평균과 채널은 `'overlay'`, 오실레이터는 `'newPane'`, 거래량 이동평균은 `'volume'`(표 참고) |
| `'overlay'` | 원본의 패널, 원본의 가격 축 |
| `'newPane'` | 이 지표를 위해 다른 패널 아래에 새로 만든 패널 |
| `'volume'` | 원본의 거래량 시리즈가 있는 패널, 거래량 스케일. 원본에 거래량 시리즈가 없으면 새 패널 |
| 패널 id | 해당 패널(`chart.panes()`, `chart.mainPane`, `chart.addPane()` 또는 `series.pane`에서 얻은 id) |

```ts
// Two oscillators in one pane.
const rsi = chart.addIndicator(candles, 'rsi');
const pane = rsi.outputs()[0].pane;
chart.addIndicator(candles, 'stochRsi', {}, pane);
```

`'newPane'`(또는 거래량 시리즈 없이 `'volume'`)으로 만들어진 패널은, 지표를
제거할 때 그 패널에 다른 것이 남아 있지 않으면 함께 제거됩니다.

### 원본 시리즈

OHLC 시리즈(캔들스틱, OHLC 바, Heikin-Ashi)는 지표에 시가, 고가, 저가, 종가,
거래량을 제공합니다. 단일 값 시리즈(line, area, baseline, histogram, points)는
`open = high = low = close = value`를 제공하며 거래량은 없습니다. 따라서 라인
시리즈의 RSI는 계산할 수 있지만 OBV는 계산할 수 없습니다.

거래량이 필요한 지표(VWMA, 거래량 이동평균, VWAP, MFI, OBV, CMF)는 원본이 단일 값
시리즈이거나, 데이터에 거래량 컬럼이 없는 OHLC 시리즈이면 예외를 발생시킵니다.

원본에 값이 없는 봉(예: 다른 시리즈에만 있는 시간)은 건너뜁니다. 그 위치의 출력은
`NaN`이며, 해당 봉은 지표의 계산 구간에 포함되지 않습니다.

### 지표의 지표

원본으로 다른 지표의 출력을 사용할 수 있습니다. 예를 들어 RSI 패널에 RSI의 9봉
SMA를 그리려면 다음과 같이 합니다.

```ts
const rsiLine = chart.addIndicator(candles, 'rsi').outputs()[0];
const rsiAvg = chart.addIndicator(rsiLine, 'sma', { n: 9 }, rsiLine.pane);
```

RSI를 제거하면 SMA도 함께 제거됩니다.

## 내장 지표

`indicatorNames()`는 이 표의 순서대로 30개의 이름을 반환합니다. 출력은
`outputs()`가 반환하는 순서로 나열했습니다. (histogram) 또는 (points)로 표시된
것 외의 출력은 모두 라인입니다.

| 이름 | 지표 | 매개변수(기본값) | 출력 | 기본 배치 |
|---|---|---|---|---|
| `sma` | 단순 이동평균 | `n` (20), `source` | sma | overlay |
| `ema` | 지수 이동평균 | `n` (20), `source` | ema | overlay |
| `wma` | 가중 이동평균 | `n` (20), `source` | wma | overlay |
| `hma` | Hull 이동평균 | `n` (20, 최소 2), `source` | hma | overlay |
| `vwma` | 거래량 가중 이동평균 | `n` (20), `source` | vwma | overlay |
| `dema` | 이중 지수 이동평균(DEMA) | `n` (20), `source` | dema | overlay |
| `tema` | 삼중 지수 이동평균(TEMA) | `n` (20), `source` | tema | overlay |
| `volumeMa` | 거래량 이동평균 | `n` (20) | vma | volume |
| `boll` | 볼린저 밴드 | `n` (20), `k` (2), `source` | upper, mid, lower; 밴드 upper–lower | overlay |
| `keltner` | 켈트너 채널 | `n` (20), `m` (2), `atrN` (10) | upper, mid, lower; 밴드 upper–lower | overlay |
| `donchian` | 돈치안 채널 | `n` (20) | upper, mid, lower | overlay |
| `ichimoku` | 일목균형표(Ichimoku) 구름 | `tenkan` (9), `kijun` (26), `senkou` (52), `shift` (26) | tenkan, kijun, span A, span B, chikou; 밴드 span A–span B | overlay |
| `sar` | 파라볼릭 SAR | `afStart` (0.02), `afStep` (0.02), `afMax` (0.2) | sar (points) | overlay |
| `supertrend` | SuperTrend | `atrN` (10), `m` (3) | long, short | overlay |
| `vwap` | 거래량 가중 평균 가격(VWAP) | `anchor` (`'day'`), `utcOffsetMs` (0) | vwap | overlay |
| `macd` | MACD | `fast` (12), `slow` (26), `signal` (9), `source` | macd, signal, hist (histogram) | 새 패널 |
| `rsi` | 상대강도지수(RSI) | `n` (14), `source` | rsi | 새 패널 |
| `stoch` | 스토캐스틱 오실레이터 | `k` (14), `smoothK` (3), `d` (3) | %K, %D | 새 패널 |
| `stochRsi` | 스토캐스틱 RSI | `rsiN` (14), `stochN` (14), `smoothK` (3), `d` (3), `source` | %K, %D | 새 패널 |
| `cci` | 상품채널지수(CCI) | `n` (20) | cci | 새 패널 |
| `willr` | 윌리엄스 %R | `n` (14) | %R | 새 패널 |
| `mfi` | 자금흐름지수(MFI) | `n` (14) | mfi | 새 패널 |
| `adx` | 방향성 지표를 포함한 ADX | `n` (14) | adx, +DI, −DI | 새 패널 |
| `atr` | 평균 실제 범위(ATR) | `n` (14) | atr | 새 패널 |
| `obv` | 온밸런스 볼륨(OBV) | 없음 | obv | 새 패널 |
| `cmf` | 차이킨 자금흐름(CMF) | `n` (20) | cmf | 새 패널 |
| `roc` | 변화율(%) | `n` (10), `source` | roc | 새 패널 |
| `momentum` | 모멘텀 | `n` (10), `source` | momentum | 새 패널 |
| `trix` | TRIX | `n` (15), `source` | trix | 새 패널 |
| `aroon` | 아룬(Aroon) | `n` (14) | up, down | 새 패널 |

### 매개변수 값

| 매개변수 | 값 |
|---|---|
| 기간(`n`, `fast`, `slow`, `signal`, `stoch`의 `k`, `smoothK`, `d`, `rsiN`, `stochN`, `atrN`, `tenkan`, `kijun`, `senkou`, `shift`) | 1 이상의 정수(`hma`: `n`은 2 이상) |
| 배수(`boll`의 `k`, `m`) | 0보다 큰 유한한 수 |
| `afStart`, `afStep`, `afMax` | 0보다 큰 유한한 수, `afStart`는 `afMax`를 넘지 않음 |
| `source` | `'close'`(기본값), `'open'`, `'high'`, `'low'`, `'hl2'` (H+L)/2, `'hlc3'` (H+L+C)/3, `'ohlc4'` (O+H+L+C)/4 |
| `anchor` (`vwap`) | `'day'`(기본값), `'week'`(한 주는 월요일에 시작), `'month'`, 또는 누적을 시작할 UTC ms 시간 |
| `utcOffsetMs` (`vwap`) | 일·주·월을 나누기 전에 UTC에 더할 밀리초 정수. 예: 뉴욕 표준시는 `-5 * 3600_000` |

```ts
chart.addIndicator(candles, 'ema', { n: 21, source: 'hlc3' });
chart.addIndicator(candles, 'vwap', { anchor: 'week' });
chart.addIndicator(candles, 'vwap', { anchor: Date.UTC(2024, 2, 1) }); // anchored VWAP
chart.addIndicator(candles, 'vwap', { utcOffsetMs: 9 * 3600_000 });   // sessions in UTC+9
```

매개변수 이름은 표에 적힌 그대로 써야 합니다. 지표가 사용하지 않는 키는
(`atr_n` 같은 snake_case 표기 포함) 허용되는 키 목록이 담긴 오류를 발생시킵니다.

```ts
chart.addIndicator(candles, 'sma', { length: 20 });
// Error: unknown parameter 'length' for indicator 'sma' (expected: n, source)
```

`addTransformSeries`도 같은 방식으로 매개변수를 검사합니다.

### 계산식

간단히 정리합니다. `P`는 선택한 `source` 가격입니다. "워밍업"은 출력이 `NaN`인
앞쪽 봉의 개수입니다.

**이동평균과 채널**

- **SMA**: `P`의 최근 `n`개 값의 평균. 워밍업 `n − 1`.
- **EMA**: `e = e + α(P − e)`, `α = 2/(n + 1)`. 처음 `n`개 값의 SMA로
  초기화합니다. 워밍업 `n − 1`.
- **WMA**: 가장 최근 값부터 가중치 `n, n−1, …, 1`. 워밍업 `n − 1`.
- **HMA**: `WMA(2·WMA(P, n/2) − WMA(P, n), √n)`(절반과 제곱근은 내림).
  워밍업 `n + ⌊√n⌋ − 2`.
- **VWMA**: `n`개 봉에 대한 `Σ(P·V) / ΣV`(거래량 합이 0이면 `NaN`).
  워밍업 `n − 1`.
- **DEMA**: `2·e1 − e2`, 여기서 `e1 = EMA(P, n)`, `e2 = EMA(e1, n)`. 워밍업
  `2(n − 1)`.
- **TEMA**: `3·e1 − 3·e2 + e3`, `e3 = EMA(e2, n)`. 워밍업 `3(n − 1)`.
- **거래량 이동평균**: 거래량의 SMA. 워밍업 `n − 1`.
- **BOLL**: mid = `SMA(P, n)`, upper와 lower = mid ± `k` × `n`개 봉의 모집단
  표준편차. 워밍업 `n − 1`.
- **Keltner**: mid = `EMA(close, n)`, upper와 lower = mid ± `m` ×
  `ATR(atrN)`. 워밍업 `max(n − 1, atrN)`.
- **Donchian**: upper = `n`개 봉의 최고 고가, lower = 최저 저가, mid = 둘의
  평균. 워밍업 `n − 1`.
- **Ichimoku**: tenkan(전환선)과 kijun(기준선)은 각각 `tenkan`개, `kijun`개
  봉의 고가-저가 범위의 중간값입니다. span A = (tenkan + kijun)/2, span B =
  `senkou`개 봉에 대한 중간값이며, 둘 다 `shift`개 봉만큼 앞으로 그려집니다(마지막
  `shift`개 값은 마지막 봉의 오른쪽에 나타납니다). chikou(후행 스팬)는 종가를
  `shift`개 봉만큼 뒤로 그린 것입니다.
- **SAR**: 가속 계수가 `afStart`에서 시작해 `afStep`씩 `afMax`까지 증가하는
  파라볼릭 SAR로, TA-Lib과 같은 방식으로 계산합니다. 워밍업 1.
- **SuperTrend**: 추세가 이어지는 동안 좁아지기만 하는 밴드
  `HL2 ± m·ATR(atrN)`. 종가가 활성 밴드를 넘으면 추세가 전환됩니다. `long`(아래
  밴드)은 상승 추세에서, `short`(위 밴드)는 하락 추세에서 값을 가지며, 그 밖에는
  `NaN`입니다. 워밍업 `atrN`.
- **VWAP**: 앵커부터의 `Σ(HLC3·V) / ΣV`. 매일, 매주 또는 매월(UTC에
  `utcOffsetMs`를 더한 기준) 다시 시작하거나, 지정한 시간부터 계산합니다(그
  이전은 `NaN`). 워밍업 0.

**오실레이터**

- **MACD**: macd = `EMA(P, fast) − EMA(P, slow)`, signal = `EMA(macd, signal)`,
  hist = macd − signal. 워밍업은 macd가 `slow − 1`, signal과 hist가
  `slow + signal − 2`.
- **RSI**: 상승폭과 하락폭에 Wilder 평활(`r = r + (x − r)/n`)을 적용하여
  `100·gain/(gain + loss)`. 둘 다 0이면 50. 워밍업 `n`.
- **Stoch**: raw = `k`개 봉에 대한 `100·(C − lowest low)/(highest high − lowest low)`
  (범위가 0이면 50); %K = `SMA(raw, smoothK)`, %D = `SMA(%K, d)`.
  워밍업은 %K가 `k + smoothK − 2`, %D는 그보다 `d − 1` 더.
- **StochRSI**: `RSI(P, rsiN)`에 `stochN`개 봉의 스토캐스틱 식을 적용한 뒤
  Stoch와 같이 평활합니다. 워밍업은 %K가 `rsiN + stochN + smoothK − 2`, %D는
  그보다 `d − 1` 더.
- **CCI**: `(TP − SMA(TP, n)) / (0.015 · mean deviation)`, `TP` = HLC3. 편차가
  0이면 0. 워밍업 `n − 1`.
- **Williams %R**: `n`개 봉에 대한
  `−100·(highest high − C)/(highest high − lowest low)`. 범위가 0이면 −50.
  워밍업 `n − 1`.
- **MFI**: `n`개 봉에 대한 `100·positive flow/(positive + negative flow)`.
  flow = HLC3 × 거래량이며, 부호는 HLC3의 변화 방향을 따릅니다. 둘 다 0이면 50.
  워밍업 `n`.
- **ADX**: +DI와 −DI는 Wilder 평활한 방향성 움직임과 실제 범위로 구하고,
  adx = `100·|+DI − −DI|/(+DI + −DI)`의 Wilder 평균. 워밍업은 DI가 `n`,
  adx가 `2n − 1`.
- **ATR**: 실제 범위의 Wilder 평균. 워밍업 `n`.
- **OBV**: 종가가 오르면 더하고 내리면 빼는 거래량 누적 합계. 워밍업 0.
- **CMF**: `n`개 봉에 대한 `Σ(MFM·V)/ΣV`, `MFM = ((C − L) − (H − C))/(H − L)`.
  워밍업 `n − 1`.
- **ROC**: `100·(P / P[n bars ago] − 1)`. 워밍업 `n`.
- **Momentum**: `P − P[n bars ago]`. 워밍업 `n`.
- **TRIX**: `EMA(EMA(EMA(P, n), n), n)`의 직전 봉 대비 변화율(%).
  워밍업 `3(n − 1) + 1`.
- **Aroon**: 최근 `n + 1`개 봉에 대해 `100·(n − bars since the highest high)/n`
  (up), 최저 저가에 대해 같은 식(down). 워밍업 `n`.

## 출력 스타일 지정

`indicator.outputs()`는 출력 시리즈를 반환합니다. 다른 시리즈와 마찬가지로
`applyOptions`로 스타일을 지정합니다([시리즈](series.md) 참고).

```ts
const [macdLine, signalLine, hist] = macd.outputs();
macdLine.applyOptions({ color: '#2962ff', lineWidth: 2 });
signalLine.applyOptions({ color: '#ff9800', lineStyle: 'dashed' });
hist.applyOptions({ color: '#787b86' });

const [upper, mid, lower] = boll.outputs();
mid.applyOptions({ lineStyle: 'dotted' });
```

출력은 선 두께 1 CSS px(points는 점 지름 3), 출력별 고정 색상,
`priceLineVisible: false`, 마지막 값 레이블 표시 상태로 생성됩니다.
`outputs()`를 호출할 때마다 같은 시리즈에 대한 새 핸들이 반환되므로, `id`로
비교하십시오.

출력은 단독으로 제거할 수 없습니다. 출력에 `series.remove()`를 호출하면 예외가
발생합니다. 대신 지표를 제거하십시오.

### 밴드

BOLL과 Keltner는 위쪽 선과 아래쪽 선 사이의 영역을 채우고, Ichimoku는 span A와
span B 사이의 구름을 채웁니다(A가 B 위이면 녹색, 아래이면 빨간색).
`indicator.bands()`는 이 채우기의 id를 반환하며, `chart.setBandOptions`로 색을
바꿀 수 있습니다.

```ts
const [band] = boll.bands();
chart.setBandOptions(band, { fill: 'rgba(41, 98, 255, 0.08)' });

const ichimoku = chart.addIndicator(candles, 'ichimoku');
const [cloud] = ichimoku.bands();
chart.setBandOptions(cloud, { fill: 'rgba(38, 166, 154, 0.25)', fillCrossed: 'rgba(239, 83, 80, 0.25)' });
console.log(chart.bandOptions(cloud)); // { fill, fillCrossed }
```

| 밴드 옵션 | 의미 |
|---|---|
| `fill` | 위쪽 출력이 아래쪽 출력 이상인 곳 |
| `fillCrossed` | 위쪽 출력이 아래쪽 출력보다 낮은 곳 |

`null`은 기본값으로 되돌립니다. BOLL과 Keltner의 기본값은 위쪽 선 색상에 불투명도
15%를 적용한 색입니다(`fillCrossed`는 `fill`과 같음). 채우기를 숨기려면
`'transparent'`를 전달하십시오. 다른 지표에는 밴드가 없으며, `bands()`는 빈
배열을 반환합니다.

## 제거

```ts
macd.remove();
```

`remove()`는 지표와 그 출력 시리즈, 출력의 가격선과 마커, 밴드, 출력으로부터
계산된 다른 지표, 그리고 지표가 만든 패널이 비게 되면 그 패널까지 제거합니다.
그 뒤에는 해당 객체의 `outputs()`, `bands()`, `remove()`가 예외를 발생시킵니다.

원본 시리즈를 제거하면 그 지표들도 제거됩니다.

## 매개변수 변경

매개변수는 그 자리에서 바꿀 수 없습니다. 지표를 제거하고 다시 추가하십시오.
사용자 지정 스타일을 유지하려면 먼저 스타일을 읽어 두십시오.

```ts
import type { FerroChart, Indicator, Series, Placement } from '@ferrochart/core';

function replaceIndicator(
  chart: FerroChart,
  old: Indicator,
  source: Series,
  name: string,
  params: Record<string, number | string>,
  placement?: Placement,
): Indicator {
  // `shift` is set by the indicator (Ichimoku), so it is not copied.
  const styles = old.outputs().map((s) => {
    const { shift, ...style } = s.options();
    return style;
  });
  old.remove();
  const next = chart.addIndicator(source, name, params, placement);
  next.outputs().forEach((s, i) => {
    if (styles[i]) s.applyOptions(styles[i]);
  });
  return next;
}

let sma = chart.addIndicator(candles, 'sma', { n: 20 });
sma = replaceIndicator(chart, sma, candles, 'sma', { n: 50 });
```

오실레이터를 제거하면 그 오실레이터가 만든 패널도(다른 것이 없을 때) 제거되므로,
새로 추가한 지표는 맨 아래의 새 패널에 배치됩니다. 오실레이터를 고정된 위치에
두려면 `chart.addPane()`으로 직접 패널을 만들고 그 id를 배치로 전달하십시오.
직접 만든 패널은 지표와 함께 제거되지 않습니다.

## 오류

다음 경우 `addIndicator`는 `Error`를 던지고 아무것도 변경하지 않습니다.

| 문제 | 메시지(예) |
|---|---|
| 알 수 없는 이름 | `unknown indicator 'smaa'` |
| 0 이상의 정수가 아닌 기간 | `'n' must be a non-negative integer` |
| 기간 0 | `invalid argument: periods must be at least 1` |
| `n`이 2 미만인 `hma` | `invalid argument: the period is too short` |
| 0 이하이거나 유한하지 않은 배수 | `invalid argument: multipliers must be finite and > 0` |
| `afStart`가 `afMax`보다 큼 | `invalid argument: af_start must not exceed af_max` |
| 숫자가 필요한 곳에 숫자가 아닌 값 | `'k' must be a finite number` |
| 알 수 없는 `source` | `unknown source 'median'` |
| 잘못된 VWAP `anchor` | `'anchor' must be 'day', 'week', 'month' or a time in ms` |
| 정수가 아닌 `utcOffsetMs` | `'utcOffsetMs' must be an integer` |
| 잘못된 `placement` | `placement must be 'default', 'overlay', 'newPane', 'volume' or a pane id` |
| 알 수 없는 패널 id | `unknown id` |
| 원본으로 지정한 거래량 시리즈 | `invalid argument: volume series have no data of their own; use the source series` |
| 거래량이 없는 원본에 거래량 지표 | `invalid argument: the indicator needs volume` |

```ts
try {
  chart.addIndicator(candles, 'rsi', { n: 0 });
} catch (e) {
  console.error((e as Error).message); // invalid argument: periods must be at least 1
}
```

## 실시간 업데이트

지표는 원본을 스스로 따라갑니다.

- 마지막 봉과 같은 시간으로 `series.update(bar)`를 호출하면 모든 출력의 마지막
  값이 업데이트되고, 더 늦은 시간이면 값이 하나 추가됩니다. 시리즈 전체가 아니라
  새로 추가되거나 바뀐 봉만 계산합니다.
- `series.setData(...)`는 새 데이터에 대해 지표를 다시 계산합니다.

증분 업데이트의 결과는 시리즈 전체를 다시 계산한 결과와 같습니다. 출력은 원본과
같은 프레임에서 바뀝니다.

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];

feed.onTick((bar) => {
  candles.update(bar);
  const last = chart.dataRange();
  if (last) console.log('RSI', rsi.dataAt(last.to)?.value);
});
```

## 이름 목록

```ts
import init, { indicatorNames } from '@ferrochart/core';

await init(); // not needed once a chart has been created
console.log(indicatorNames()); // ['sma', 'ema', ..., 'aroon']
```
