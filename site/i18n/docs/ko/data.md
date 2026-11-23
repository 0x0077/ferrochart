<!-- source: d69da2a7ed -->
# 데이터

이 장에서는 시리즈에 봉과 값을 넣는 방법을 다룹니다. 데이터 형식, 검증 규칙,
실시간 업데이트, 과거 데이터 불러오기, 데이터 다시 읽기를 설명합니다.

## 열과 행

`setData`는 시리즈의 모든 데이터를 교체합니다. 두 가지 형태를 받습니다. 열
객체(필드마다 배열 하나) 또는 행 객체의 배열입니다.

### 열

OHLC 시리즈(캔들스틱, OHLC 봉, 하이킨아시, 변환 시리즈)는 `OhlcData`를 받습니다.

| 열 | 타입 | 비고 |
|---|---|---|
| `time` | `BigInt64Array`, `Float64Array` 또는 `number[]` | 봉 시작 시간, UTC 밀리초, 엄격하게 증가 |
| `open`, `high`, `low`, `close` | `Float64Array` 또는 `number[]` | |
| `volume` | `Float64Array` 또는 `number[]` | 선택 사항 |

값 시리즈(라인, 영역, 베이스라인, 히스토그램, 포인트)는 `ValueData`를 받습니다.

| 열 | 타입 | 비고 |
|---|---|---|
| `time` | `BigInt64Array`, `Float64Array` 또는 `number[]` | UTC 밀리초, 엄격하게 증가 |
| `value` | `Float64Array` 또는 `number[]` | `NaN`은 공백 |
| `color` | `(string \| null)[]` 또는 `Uint32Array` | 선택 사항인 점별 색상, 히스토그램에서만 그려짐([시리즈 유형](series.md#histogram) 참고) |

모든 열의 길이가 같아야 합니다.

```ts
candles.setData({
  time: new BigInt64Array([1704067200000n, 1704067260000n, 1704067320000n]),
  open: new Float64Array([100.0, 100.4, 100.1]),
  high: new Float64Array([100.6, 100.5, 100.9]),
  low: new Float64Array([99.8, 99.9, 100.0]),
  close: new Float64Array([100.4, 100.1, 100.8]),
  volume: new Float64Array([1200, 900, 1500]),
});

line.setData({ time: [1704067200000, 1704067260000], value: [100.4, 100.1] });
```

### 행

객체 배열도 받습니다. OHLC 시리즈에는 `OhlcRow`(`{ time, open, high, low,
close, volume? }`), 값 시리즈에는 `ValueRow`(`{ time, value?, color? }`)를
사용합니다. 행에서는 `time`에 `Date`도 쓸 수 있습니다.

```ts
candles.setData([
  { time: new Date('2024-01-01T00:00:00Z'), open: 100.0, high: 100.6, low: 99.8, close: 100.4 },
  { time: new Date('2024-01-01T00:01:00Z'), open: 100.4, high: 100.5, low: 99.9, close: 100.1 },
]);

line.setData([
  { time: 1704067200000, value: 100.4 },
  { time: 1704067260000 },                 // no value: a gap
  { time: 1704067320000, value: 100.8 },
]);
```

`value`가 없는(또는 `NaN`인) 행은 공백입니다. OHLC 필드가 빠져 있으면 해당
필드 이름을 담은 오류가 발생합니다(`data[3] has no 'open'`). 일부 행에만
`volume`이 있으면 없는 거래량은 0이 됩니다.

### 어느 쪽을 쓸까

행은 다른 처리보다 먼저 열로 변환되므로 두 형태의 결과는 같습니다. 데이터가
이미 객체로 들어오고(JSON API, 수백 개 정도의 봉) 그편이 코드가 간단하다면 행을
쓰세요. 대용량 데이터에는 열을 쓰세요. 타입 배열은 봉마다 객체를 읽을 필요 없이
열마다 `memcpy` 한 번으로 엔진에 복사되므로, 봉 100만 개의 `setData`가 수십
밀리초면 끝납니다. 바이너리 데이터를 받는다면(예: WebSocket이나 파일) 객체를
거치지 말고 바로 타입 배열을 만드세요.

### 시간

시간은 Unix epoch 이후의 UTC 밀리초이며 정수입니다.

- `BigInt64Array` 시간은 그대로 사용됩니다.
- `Float64Array`와 `number[]` 시간은 정수로 변환됩니다. 유한하지 않거나, 소수부가
  있거나, ±9·10¹⁵를 벗어나는 값이면 예외가 발생합니다
  (`time[5] is not an integer millisecond timestamp`).
- 행과 `update`에서 `time`은 number, bigint 또는 `Date`일 수 있습니다.

원본이 초 단위라면 1000을 곱하세요.

차트는 시간을 UTC로 표시합니다.

## 검증

`setData`와 `update`는 먼저 데이터를 검사합니다. 문제가 있으면 예외를 던지고,
시리즈는 이전 데이터를 유지합니다.

| 규칙 | 오류 메시지 |
|---|---|
| 시간이 엄격하게 증가(중복 없음) | `time is not strictly increasing at index 7` |
| 모든 열의 길이가 `time`과 같음 | ``column `open` has a different length than `time` `` |
| 공백 봉을 제외하고 가격과 거래량이 유한함 | `invalid OHLC values in bar 12` |
| `high` ≥ max(`open`, `close`)이고 `low` ≤ min(`open`, `close`) | `invalid OHLC values in bar 12` |
| 열이 숫자 배열 또는 타입 배열임 | `column 'high' must be a Float64Array or an array of numbers` |

메시지의 인덱스는 전달한 데이터에서의 위치입니다.

네 가격이 모두 `NaN`인 봉은 공백 봉입니다([공백](#gaps) 참고). 유효한 봉이며
거래량은 0으로 설정됩니다.

## 공백

- 값 시리즈에서는 `NaN` 값(또는 `value`가 없는 행)이 공백입니다. 라인, 영역,
  베이스라인은 그 지점에서 끊기고, 히스토그램과 포인트는 아무것도 그리지 않습니다.
- OHLC 시리즈에서는 시가, 고가, 저가, 종가가 모두 `NaN`인 봉이 공백이며 아무것도
  그리지 않습니다.

`dataAt`은 공백에 대해 `null`을 반환합니다.

### 하나의 시간 축에 여러 시리즈

차트의 모든 시리즈는 시간 축 하나를 공유합니다. 논리 인덱스 42는 모든 시리즈에서
같은 시간입니다. 시리즈마다 시간이 다르면 축은 그 시간들의 합집합이 됩니다. 다른
시리즈에만 있는 시간에는 해당 시리즈의 점이 없습니다. OHLC 시리즈, 히스토그램,
포인트는 그 위치에 아무것도 표시하지 않고, 라인, 영역, 베이스라인은 이웃한 점을
잇습니다. 새 시간이 축의 기존 봉 사이에 들어가면 축이 다시 만들어지고 차트가
`warning` 이벤트를 보냅니다.

## 실시간 업데이트

`update`는 마지막 봉을 바꾸거나 새 봉을 추가합니다.

- 마지막 봉과 같은 시간: 그 봉(형성 중인 봉)을 교체합니다.
- 더 나중 시간: 새 봉을 추가합니다.
- 더 이른 시간: 예외가 발생합니다.

```ts
// OHLC series
candles.update({ time: 1704067380000, open: 100.8, high: 101.2, low: 100.7, close: 101.0, volume: 300 });
// Value series
line.update({ time: 1704067380000, value: 101.0 });
```

봉은 `setData` 데이터와 같은 방식으로 검증됩니다. 마지막 봉 교체는 메모리를
할당하지 않고, 차트는 애니메이션 프레임당 최대 한 번만 다시 그리므로 체결마다
`update`를 호출해도 괜찮습니다. 시리즈에 걸린 지표는 증분 방식으로 업데이트됩니다.
거래량 시리즈는 원본 시리즈를 따르며 자체 `update`가 없습니다.

변환 시리즈(Renko, Kagi 등)에서는 각 `update`가 입력 봉 또는 틱이며, 그에 따라
그려지는 요소가 바뀝니다.
[봉 변환](series.md#bar-transforms)을 참고하세요.

### 일괄 추가

끝에 여러 봉을 추가하려면(예: 재연결 후 피드에서 받은 묶음) `setData`가 받는 어떤
형태로든 열이나 행을 `append`에 전달하세요. 시간은 마지막 봉 이후여야 하고,
거래량 열은 시리즈에 거래량이 있을 때에만, 그리고 반드시 있어야 합니다. 그렇지
않으면 예외가 발생하고 데이터는 그대로 유지됩니다.

```ts
candles.append([
  { time: 1704067440000, open: 101.0, high: 101.4, low: 100.9, close: 101.3, volume: 250 },
  { time: 1704067500000, open: 101.3, high: 101.5, low: 101.1, close: 101.2, volume: 180 },
]);
```

형성 중인 봉에는 `update`를, 함께 도착한 완성된 봉에는 `append`를 사용하세요.

## 과거 데이터 불러오기

`prepend`는 첫 번째 봉 앞에 더 오래된 봉을 삽입하며, `setData`가 받는 어떤
형태든 받습니다. 기존 봉은 논리 인덱스를 유지하고, 새 봉은 음수 인덱스를 받습니다
(인덱스 0 바로 앞의 봉은 -1). 뷰는 움직이지 않으며 시리즈에 걸린 지표는 다시
계산됩니다. 시간은 첫 번째 봉보다 앞서야 하며, 거래량 규칙은 `append`와 같습니다.

불러올 시점은 차트가 알려 줍니다. 뷰의 왼쪽 끝 앞에 남은 봉이 한 화면 폭보다
적어지면 `needMoreHistory` 이벤트가 전송됩니다. `barsBeforeStart`는 남은 봉의
개수입니다(데이터 앞의 빈 공간이 이미 보이면 음수). 이벤트는 첫 번째 봉마다 한 번
전송됩니다. `prepend` 후에는 첫 번째 봉이 새 봉이 되므로, 사용자가 그 근처까지
스크롤하면 이벤트가 다시 옵니다.

### 실습 예제

```ts
import { FerroChart, type OhlcRow } from '@ferrochart/core';

// Your backend: up to `count` bars that end before `beforeMs`, oldest first.
declare function fetchBars(symbol: string, beforeMs: number, count: number): Promise<OhlcRow[]>;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const latest = await fetchBars('BTCUSD', Date.now(), 1000);
candles.setData(latest);
let first = Number(latest[0]?.time ?? Date.now());
let loading = false;
let exhausted = false;

chart.on('needMoreHistory', async () => {
  if (loading || exhausted) return;
  loading = true;
  try {
    // Only bars before the first one: an overlapping bar would make prepend throw.
    const older = (await fetchBars('BTCUSD', first, 1000)).filter((b) => Number(b.time) < first);
    if (older.length === 0) {
      exhausted = true; // the backend has nothing older
      return;
    }
    candles.prepend(older);
    first = Number(older[0].time);
  } finally {
    loading = false;
  }
});
```

`loading` 플래그는 사용자 코드에서 요청이 겹치는 것을 막습니다(이벤트 자체는 같은
첫 번째 봉에 대해 반복되지 않습니다). 그동안에도 실시간 봉은 계속 `update`로
들어옵니다. `prepend`와 `update`는 서로 반대쪽 끝에서 동작합니다.

## 데이터 다시 읽기

| 메서드 | 반환값 |
|---|---|
| `series.dataAt(index)` | OHLC 시리즈는 `{ open, high, low, close, volume }`(거래량 열이 없으면 `volume`은 `null`), 값 시리즈는 `{ value }`, 공백이거나 데이터 범위 밖이면 `null` |
| `chart.timeAtIndex(index)` | 논리 인덱스에 있는 봉의 시간(UTC ms), 데이터 범위 밖이면 `null` |
| `chart.indexOfTime(time)` | 정확히 그 시간인 봉의 논리 인덱스, 없으면 `null` |
| `chart.dataRange()` | `{ from, to }`: 첫 번째와 마지막 봉의 논리 인덱스, 데이터가 없으면 `null` |

소수 인덱스는 가장 가까운 봉으로 반올림됩니다.

```ts
const range = chart.dataRange();
if (range) {
  const last = candles.dataAt(range.to);              // the forming bar
  const t = chart.timeAtIndex(range.to);              // its time
  console.log(t, last);
}

const i = chart.indexOfTime(Date.UTC(2024, 0, 2, 9, 30));
if (i !== null) console.log(candles.dataAt(i));
```

`shift`가 있는 시리즈에서 `dataAt(index)`는 `index` 위치에 그려진 점을 반환합니다.

## 논리 인덱스

모든 봉에는 논리 인덱스가 있습니다. 표시 범위, 크로스헤어 위치, 클릭은 이 인덱스로
보고되며, `setVisibleLogicalRange`와 `logicalToCoordinate`도 이 인덱스를
받습니다. [시간 축과 뷰포트](time-scale.md)를 참고하세요. 모든 시리즈가 시간 축을
공유하므로 인덱스 `i`는 모든 시리즈에서 같은 시간입니다.

`setData` 후 첫 번째 봉의 인덱스는 0입니다. 봉 사이의 위치가 의미 있는 곳(뷰의
끝, 포인터)에서는 인덱스가 소수일 수 있습니다. 봉 `i`는 `i - 0.5`부터
`i + 0.5`까지를 차지합니다.

## 상세 수준(LOD)

봉은 열 단위로 저장됩니다. 시간 배열 하나, 시가 배열 하나와 같은 식입니다. 이
덕분에 로딩이 빠르고 메모리가 적게 듭니다. 시간 인덱스와 상세 수준 피라미드를
포함해 봉당 약 58바이트입니다.

여러 봉이 한 픽셀을 공유할 만큼 축소하면, FerroChart는 미리 계산된 피라미드에서
집계된 봉을 그립니다(각 레벨은 개수를 절반으로 줄이되 실제 고가와 저가는
유지합니다). 따라서 프레임당 그리는 요소 수는 데이터 크기와 관계없이 화면 폭,
즉 물리 픽셀당 약 4개로 제한됩니다.

내부적으로 모든 데이터 변경은 증분 방식입니다. 렌더러는 어느 범위가 바뀌었는지
전달받아 그 부분만 업로드합니다.

## 거래량

거래량 시리즈는 OHLC 시리즈의 거래량 열을 읽습니다:
`chart.addVolumeSeries(candles)`. 자체 데이터가 없으므로 캔들의 `setData`와
`update`에 `volume`을 전달하세요. [시리즈 유형](series.md#volume)을 참고하세요.

## 예제: 시뮬레이션 실시간 피드

500개 봉의 과거 데이터와 250 ms마다 새 가격이 들어오는 차트입니다. 시계가 분
경계를 지날 때마다 새 1분봉이 시작됩니다.

```ts
import { FerroChart, randomWalk, type Bar } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const interval = 60_000;
const now = Date.now();
const start = Math.floor(now / interval) * interval - 500 * interval;
const history = randomWalk(42, 500, start, interval); // deterministic demo data
candles.setData(history);

const n = history.close.length;
let bar: Bar = {
  time: Number(history.time[n - 1]),
  open: history.open[n - 1],
  high: history.high[n - 1],
  low: history.low[n - 1],
  close: history.close[n - 1],
  volume: history.volume?.[n - 1] ?? 0,
};

const timer = setInterval(() => {
  const t = Math.floor(Date.now() / interval) * interval;
  const price = bar.close * (1 + (Math.random() - 0.5) * 0.002);
  const size = Math.round(Math.random() * 10);
  if (t > Number(bar.time)) {
    // A new minute: open a new bar at the last close.
    bar = { time: t, open: bar.close, high: Math.max(bar.close, price), low: Math.min(bar.close, price), close: price, volume: size };
  } else {
    bar = { ...bar, high: Math.max(bar.high, price), low: Math.min(bar.low, price), close: price, volume: (bar.volume ?? 0) + size };
  }
  candles.update(bar); // same time: replaces, later time: appends
}, 250);

// When the chart goes away:
// clearInterval(timer); chart.destroy();
```

뷰는 최신 봉과의 거리(오른쪽 여백)를 유지하므로, 사용자가 최신 데이터를 보고
있든 뒤로 스크롤했든 봉이 추가될 때마다 내용이 한 봉씩 왼쪽으로 스크롤됩니다.
`chart.scrollToRealtime()`을 호출하면 최신 봉이 다시 화면에 들어옵니다
([시간 축과 뷰포트](time-scale.md#right-offset) 참고).
