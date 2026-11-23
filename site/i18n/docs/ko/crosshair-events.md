<!-- source: 9cee954393 -->
# 크로스헤어와 이벤트

차트는 자신에게 일어나는 일을 이벤트로 알려 줍니다. 크로스헤어 이동, 클릭,
표시 범위 변경, 드로잉 생성·편집 등이 있습니다. 이 장에서는 모든 이벤트와
그 페이로드, 발생 시점을 정리하고, 자주 쓰이는 두 가지 활용 예를 보여 줍니다.
크로스헤어를 따라가는 범례, 그리고 마커와 시리즈 클릭에 반응하기입니다.

## 크로스헤어 모드

크로스헤어는 플롯 위에서 포인터를 따라갑니다. 세로선은 모든 패널을 가로지르고,
가로선과 가격 레이블은 포인터 아래에 있는 패널에 속합니다. 시간 레이블은 시간
축에 표시됩니다.

| 모드 | 동작 |
|---|---|
| `'normal'` (기본값) | x는 가장 가까운 봉의 중심에 맞춰지고, y는 포인터를 따라갑니다 |
| `'magnet'` | `'normal'`과 같으며, y가 해당 봉(패널의 첫 번째 가격 시리즈)의 시가, 고가, 저가, 종가 중 가장 가까운 값에 맞춰집니다 |
| `'hidden'` | 크로스헤어를 그리지 않으며, `crosshairMove`는 `null`을 보고합니다 |

```ts
chart.setCrosshairMode('magnet');
console.log(chart.crosshairMode); // 'magnet'
```

알 수 없는 모드를 지정하면 예외가 발생합니다. 모드는 저장되는 상태의
일부입니다(`saveState().crosshairMode`, [상태 저장](state.md) 참고).

마그넷 모드는 드로잉을 배치할 때에도 적용되어, 앵커가 가장 가까운 시가, 고가,
저가 또는 종가에 맞춰집니다([드로잉 도구](drawings.md) 참고).

## 구독

```ts
import type { ChartEventMap } from '@ferrochart/core';

const onMove = (e: ChartEventMap['crosshairMove']) => {
  console.log(e.point?.time);
};

chart.on('crosshairMove', onMove);
// later
chart.off('crosshairMove', onMove);
```

- `on(name, callback)`은 리스너를 추가합니다. 알 수 없는 이벤트 이름이면 예외가
  발생합니다.
- `off(name, callback)`은 리스너를 제거합니다. `on`에 전달한 것과 같은 함수
  객체를 전달하세요.
- 같은 함수를 두 번 추가하면 두 번 호출되며, `off`는 둘 다 제거합니다.
- 콜백 안에서 `on`과 `off`를 포함한 모든 차트 메서드를 호출할 수 있습니다.
- 콜백 안에서 발생한 예외는 처리되지 않은 오류처럼 보고됩니다
  (`reportError`: 콘솔에 표시되고 `window.onerror`에 전달됩니다).
  나머지 콜백은 그대로 실행됩니다.

콜백 매개변수의 타입은 이벤트 이름으로부터 정해지므로
`chart.on('crosshairMove', (e) => e.point?.time)`에는 타입 표기가 필요 없습니다.
위처럼 콜백을 따로 선언할 때는 패키지가 내보내는 `ChartEventMap[name]`으로
페이로드 타입을 지정합니다.

## 전달 시점

차트는 `requestAnimationFrame` 안에서 그립니다. 매 프레임마다 먼저 지난
프레임 이후의 입력과 API 호출을 반영하고, 프레임을 만들어 렌더링한 다음, 그
프레임에서 생긴 이벤트를 전달합니다. 그 결과는 다음과 같습니다.

- 이벤트는 DOM 입력 핸들러 안이나 이벤트를 일으킨 메서드 호출 안에서 전달되지
  않습니다. `chart.drawings.add(...)`는 즉시 id를 반환하고, 해당
  `drawingCreated` 이벤트는 다음 프레임 이후에 도착합니다.
- 한 번에 최대 한 프레임 분량의 이벤트만 전달됩니다. 지난 프레임 이후 크로스헤어가
  여러 번 움직였다면 포인터 이벤트마다 하나씩이 아니라, 최신 위치를 담은
  `crosshairMove` 하나만 받습니다. `visibleRangeChanged`와 `sync`도 마찬가지입니다.
- 브라우저는 백그라운드 탭에서 `requestAnimationFrame`을 멈추므로, 탭이 다시
  보일 때까지 이벤트가 지연됩니다.

## 수명

차트는 캔버스와 window에 등록한 리스너를 통해 동작을 유지하며, 이 리스너들은
차트를 약한 참조로만 가지고 있습니다. 페이지에 차트가 있는 동안에는 `FerroChart`
객체에 대한 참조를 유지하세요. 객체가 가비지 컬렉션되면 차트는 그리기와 이벤트
전송을 멈춥니다. 차트를 다 쓰고 나면 `destroy()`를 호출하세요. 모든 이벤트
리스너(사용자 리스너와 DOM 리스너)를 제거하고 GPU 리소스를 해제합니다. 이후에는
객체를 사용할 수 없습니다.

```ts
chart.destroy();
```

## 이벤트 레퍼런스

| 이벤트 | 페이로드 | 발생 시점 |
|---|---|---|
| `click` | `ClickEvent` | 플롯 안에서 누르고 떼는 동안 이동 거리가 3 CSS px 이하일 때 |
| `crosshairMove` | `{ point: CrosshairPoint \| null }` | 크로스헤어가 다른 봉, 패널 또는 가격으로 이동했거나 플롯을 벗어났을 때 |
| `visibleRangeChanged` | `{ from, to }` | 표시되는 논리 범위가 바뀌었을 때 |
| `needMoreHistory` | `{ barsBeforeStart }` | 뷰의 왼쪽 끝이 첫 번째 봉으로부터 한 화면 이내로 들어왔을 때 |
| `drawingCreated` | `{ id }` | 드로잉이 생성되었을 때 |
| `drawingModified` | `{ id }` | 드로잉이 이동되었거나, 스타일이 바뀌었거나, 매개변수가 변경되었을 때 |
| `drawingRemoved` | `{ id }` | 드로잉이 제거되었을 때 |
| `drawingSelected` | `{ id: number \| null }` | 선택된 드로잉이 바뀌었을 때 |
| `sync` | `{ message: SyncMessage }` | 차트가 동기화 그룹에 메시지를 보냈을 때 |
| `drawingSync` | `{}` | 차트에 가져가기를 기다리는 드로잉 동기화 작업이 있을 때 |
| `warning` | `ChartWarning` | 차트가 스스로 무언가를 조정했을 때 |

### click

```ts
interface ClickEvent {
  x: number;              // CSS px from the canvas' top-left corner
  y: number;
  pane: number;           // pane id
  logicalIndex: number;   // fractional logical index at x
  time: number | null;    // UTC ms of the nearest bar, null beyond the data
  price: number;          // price at y on the pane's price scale
  target: ClickTarget | null;
}

type ClickTarget =
  | { type: 'drawing'; id: number }
  | { type: 'marker'; series: Series; index: number }
  | { type: 'series'; series: Series };
```

클릭은 플롯 안에서 누르고 떼는 동작(마우스, 펜, 터치)으로, 그동안 포인터의 이동
거리가 3 CSS px 이하인 경우입니다. 그보다 많이 움직이면 이동(팬)으로 처리됩니다.
드로잉을 생성하는 중에는 클릭이 전송되지 않습니다.

`target`은 포인터 아래에 있던 대상이며, 다음 순서로 확인합니다.

1. **드로잉**(본체 또는 핸들): `{ type: 'drawing', id }`.
   드로잉을 누르면 그 드로잉이 선택되기도 합니다.
2. **마커**: `{ type: 'marker', series, index }`. 여기서 `index`는
   `series.markers()`(시간순으로 정렬됨)에서 마커의 위치입니다.
3. **시리즈**: 포인터가 캔들의 몸통이나 꼬리 위, 선에서 4 CSS px 이내,
   히스토그램 막대 안, 또는 점 위에 있을 때 `{ type: 'series', series }`.
   시리즈가 겹치면 마지막에 추가된 시리즈가 우선합니다.
4. 빈 공간이면 `null`.

target에 담긴 `Series` 객체는 기존 시리즈를 가리키는 새 핸들입니다. `===`가 아니라
`id`로 비교하세요.

```ts
chart.on('click', (e: ChartEventMap['click']) => {
  if (e.target?.type === 'series' && e.target.series.id === candles.id) {
    console.log('clicked the candles at', e.time);
  }
});
```

지표 출력도 일반 시리즈이므로, 지표 선을 클릭하면 `indicator.outputs()` 중
하나와 `id`가 일치하는 `series` target을 받습니다.

### crosshairMove

```ts
interface CrosshairPoint {
  logicalIndex: number;   // the bar the crosshair is on (whole number)
  time: number | null;    // UTC ms, null beyond the data
  pane: number;           // pane under the pointer
  price: number;          // price at the crosshair; NaN for a synced crosshair
  bar: BarValues | null;  // OHLCV of the first OHLC series with a bar here
  x?: number;             // pointer position, CSS px; absent for a synced crosshair
  y?: number;
}

interface BarValues {
  open: number; high: number; low: number; close: number;
  volume: number | null;
}
```

포인터가 플롯을 벗어나거나 크로스헤어가 숨겨져 있으면 `point`는 `null`입니다.
이벤트는 point의 내용이 바뀔 때만 전송되며, 같은 봉과 같은 가격 안에서 포인터를
움직여도 반복되지 않습니다.

마그넷 모드에서 `price`는 맞춰진 가격입니다. 해당 인덱스에 봉이 있는 OHLC
시리즈가 없으면(예: 라인 시리즈만 있는 차트나 데이터 공백) `bar`는 `null`입니다.
크로스헤어 위치의 다른 시리즈 값을 읽으려면 `series.dataAt(point.logicalIndex)`를
사용하세요.

[동기화 그룹](sync.md)에 속한 차트는 다른 차트의 크로스헤어도 보고합니다. 이런
point는 `price: NaN`이고, `pane`은 메인 패널로 설정되며, `x`와 `y`가 없습니다.

### visibleRangeChanged

`{ from: number, to: number }`는 플롯 왼쪽과 오른쪽 끝의 논리 인덱스이며 소수일
수 있습니다. 이동, 확대·축소, 크기 조정, 그리고 뷰를 바꾸는 메서드
(`setVisibleLogicalRange`, `fitContent`, `scrollToRealtime`,
`setBarSpacing`, `setRightOffset`)가 모두 이 이벤트를 일으킵니다. 같은 값은
언제든 `chart.visibleLogicalRange()`로 얻을 수 있습니다.

### needMoreHistory

`{ barsBeforeStart: number }`는 첫 번째 봉과 뷰 왼쪽 끝 사이의 봉 개수입니다.
뷰가 이미 데이터 앞의 빈 공간을 보여 주고 있으면 음수가 됩니다. 이 이벤트는 첫
번째 봉마다 한 번 전송됩니다. 더 오래된 데이터를 추가한 뒤 사용자가 새로운 시작
지점까지 스크롤하면 다시 발생할 수 있습니다.

```ts
let loading = false;
chart.on('needMoreHistory', async (e: ChartEventMap['needMoreHistory']) => {
  if (loading) return;
  loading = true;
  try {
    await loadOlderBars(e.barsBeforeStart); // your code: fetch older bars, then series.prepend(them)
  } finally {
    loading = false;
  }
});
```

데이터 로딩은 [데이터](data.md)를 참고하세요.

### 드로잉 이벤트

`drawingCreated`, `drawingModified`, `drawingRemoved`는 드로잉의 `id`를 담고,
`drawingSelected`는 선택된 id 또는 `null`을 담습니다.

| 이벤트 | 원인 |
|---|---|
| `drawingCreated` | 사용자가 드로잉을 완성함, `drawings.add`, 드로잉을 (이전 id 그대로) 되살리는 실행 취소 또는 다시 실행, `loadState`(불러온 드로잉마다 하나씩), 동기화 그룹에서 받은 드로잉 |
| `drawingModified` | 드로잉이나 핸들을 드래그함, `setAnchors`, `setStyle`, `setParam`, 그런 변경의 실행 취소 또는 다시 실행, 동기화된 변경 |
| `drawingRemoved` | Delete 키, `drawings.remove`, 생성의 실행 취소, `loadState`(대체되는 드로잉에 대해), 패널 제거, 동기화된 제거 |
| `drawingSelected` | 드로잉이나 빈 공간을 누름, 새 드로잉 완성(완성된 드로잉이 선택됨), `drawings.select`, 선택된 드로잉을 숨기거나 제거함 |

드로잉을 표시하거나 숨기거나 잠그거나 z 순서를 바꾸는 것은 이벤트를 보내지
않습니다. [드로잉 도구](drawings.md)를 참고하세요.

### sync

`{ message: SyncMessage }`는 차트가 [동기화 그룹](sync.md)에 크로스헤어 또는
표시 범위 메시지를 보냈을 때 전송됩니다.

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }
  | { type: 'visibleRange'; from: number; to: number };
```

한 페이지 안에서 같은 `SyncGroup`에 속한 차트들은 따로 할 일이 없습니다. 이
이벤트는 워커 등 다른 곳에 있는 차트로 메시지를 전달하기 위한 것이며, 그쪽
차트는 `applySync(message)`로 메시지를 적용합니다. 그룹에 속하지 않은 차트나,
그룹이 해당 종류의 메시지를 공유하지 않는 차트는 `sync` 이벤트를 보내지 않습니다.

### drawingSync

`{}`. 동기화 그룹에서 드로잉을 공유하는 차트에 페이지가 아직 가져가지 않은 드로잉
작업이 있을 때 전송됩니다. 한 번만 전송되며, `chart.takeDrawingSyncOps()`를
호출한 뒤에야 다시 전송됩니다. [차트
동기화](sync.md)를 참고하세요.

### warning

차트가 스스로 무언가를 바로잡았으며, 그 원인이 된 호출은 성공했습니다.

```ts
type ChartWarning =
  | { kind: 'timeAxisMerged'; series: Series; count: number }
  | { kind: 'ohlcRepaired'; series: Series; count: number }
  | { kind: 'dataDropped'; series: Series }
  | { kind: 'other' };
```

| `kind` | 의미 |
|---|---|
| `'timeAxisMerged'` | `series`의 시간이 시간 축의 봉 사이에 들어갔습니다. 축이 모든 시리즈 시간의 합집합으로 다시 만들어졌고, 처음 삽입된 봉 이후의 논리 인덱스가 이동했습니다. `count`개의 봉이 삽입되었습니다. |
| `'ohlcRepaired'` | `series`의 봉 `count`개에서 고가가 다른 가격보다 낮거나 저가가 다른 가격보다 높아, 범위를 넓혀 보정했습니다. (JavaScript API는 이런 봉을 거부하므로 JavaScript에서 다루는 차트는 이 경고를 보내지 않습니다.) |
| `'dataDropped'` | `series`의 데이터를 다시 만든 시간 축으로 옮길 수 없어(메모리 부족) 데이터가 비워졌습니다. |
| `'other'` | 이후 버전에서 추가된 종류입니다. |

```ts
chart.on('warning', (w) => {
  if (w.kind === 'timeAxisMerged') console.warn(`time axis rebuilt: ${w.count} bars inserted by series ${w.series.id}`);
});
```

## 예제: 크로스헤어를 따라가는 범례

차트 위에 놓인 DOM 요소가 크로스헤어 아래의 봉을 보여 주고, 포인터가 다른 곳에
있으면 최신 봉을 보여 줍니다.

```html
<div style="position: relative; height: 480px">
  <canvas id="chart" style="width: 100%; height: 100%"></canvas>
  <div id="legend" style="position: absolute; left: 12px; top: 8px;
       font: 12px sans-serif; pointer-events: none"></div>
</div>
```

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const canvas = document.getElementById('chart') as HTMLCanvasElement;
const legend = document.getElementById('legend') as HTMLDivElement;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars); // your OHLC columns
const sma = chart.addIndicator(candles, 'sma', { n: 20 }).outputs()[0];

const fmt = (v: number | undefined) => (v === undefined || Number.isNaN(v) ? '–' : v.toFixed(2));

function show(index: number) {
  const bar = candles.dataAt(index);
  const avg = sma.dataAt(index);
  if (!bar) {
    legend.textContent = '';
    return;
  }
  const time = chart.timeAtIndex(index);
  const date = time === null ? '' : new Date(time).toISOString().slice(0, 16).replace('T', ' ');
  legend.textContent =
    `${date}  O ${fmt(bar.open)}  H ${fmt(bar.high)}  L ${fmt(bar.low)}  C ${fmt(bar.close)}` +
    `  SMA 20 ${fmt(avg?.value)}`;
}

function showLatest() {
  const range = chart.dataRange();
  if (range) show(range.to);
}

chart.on('crosshairMove', (e: ChartEventMap['crosshairMove']) => {
  if (e.point) show(e.point.logicalIndex);
  else showLatest();
});
showLatest();
```

`dataAt`은 OHLC 시리즈에 대해서는 `{ open, high, low, close, volume }`을, 그 밖의
시리즈에 대해서는 `{ value }`를, 데이터가 없거나 공백이면 `null`을 반환합니다.
지표 출력은 워밍업 구간의 봉에서 `NaN`입니다. 실시간 업데이트로 최신 봉이 바뀔
때 포인터가 차트 위에 없다면 `update` 다음에 `showLatest()`를 호출하세요.

## 예제: 마커와 시리즈 클릭

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars);

candles.setMarkers([
  { time: bars.time[120], position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: bars.time[180], position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
]);

chart.on('click', (e: ChartEventMap['click']) => {
  const t = e.target;
  if (!t) {
    console.log(`empty space at ${e.price.toFixed(2)}`);
    return;
  }
  switch (t.type) {
    case 'marker': {
      const marker = t.series.markers()[t.index];
      console.log(`marker "${marker.text}" at`, new Date(Number(marker.time)));
      break;
    }
    case 'series':
      if (t.series.id === candles.id) {
        const bar = candles.dataAt(Math.round(e.logicalIndex));
        console.log('candle', bar);
      }
      break;
    case 'drawing':
      console.log('drawing', t.id, chart.drawings.get(t.id)?.kind);
      break;
  }
});
```

마커 인덱스는 `setMarkers`에 전달한 순서가 아니라 `series.markers()`가 반환하는
시간순 목록을 기준으로 합니다. 나중에 다시 찾아야 하는 데이터는 시간을 키로 따로
보관하세요. [마커와 가격선](markers.md)을 참고하세요.
