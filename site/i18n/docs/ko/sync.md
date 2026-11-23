<!-- source: 37271b3d45 -->
# 차트 동기화

같은 `SyncGroup`에 참여한 차트는 서로를 따라갑니다. 한 차트 위에서 크로스헤어를
움직이면 다른 차트에도 표시되고, 한 차트를 이동하거나 확대·축소하면 다른 차트도
같은 시간 범위로 스크롤됩니다. 선택적으로, 한 차트에서 그린 드로잉을 다른 차트에도
표시할 수 있습니다.

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const left: HTMLCanvasElement;
declare const right: HTMLCanvasElement;

const a = await FerroChart.create(left);
const b = await FerroChart.create(right);

const group = new SyncGroup();
a.joinSync(group);
b.joinSync(group);
```

`new SyncGroup()`에는 WebAssembly 모듈이 필요합니다. 모듈은 첫 번째
`FerroChart.create`에서 로드됩니다. 차트보다 먼저 그룹을 만들려면 먼저
`await init()`을 호출하세요(`import init from '@ferrochart/core'`).

## 봉 번호가 아니라 시간

차트 사이의 모든 메시지는 논리 인덱스가 아니라 **시간**(UTC ms)을 담습니다.
따라서 한 그룹의 차트들이 서로 다른 종목, 다른 주기, 또는 서로 다른 위치에 공백이
있는 데이터를 보여 줘도 됩니다.

- 1분 차트에서 10:37에 있는 크로스헤어는 1시간 차트에서 10:00 봉(그 시간 또는 그
  이전의 마지막 봉)에 표시됩니다. 데이터의 양 끝을 넘어서면 시간이 외삽됩니다.
- 이동이나 확대·축소 후 다른 차트들은 봉 간격 제한이 허용하는 한 왼쪽 끝부터
  오른쪽 끝까지 같은 시간 구간을 보여 줍니다(오른쪽 끝은 유지됩니다). 두 시간을
  보여 주는 1분 차트를 따라가는 1시간 차트는 플롯 전체에 봉 두 개만 그려야 하지만,
  기본 최대 봉 간격이 50 px이므로 같은 시간에서 끝나되 더 넓은 범위를 보여 줍니다.

데이터가 없는 차트는 받은 범위를 무시합니다.

## 참여와 탈퇴

### `chart.joinSync(group, options?)`

`group`에 참여합니다. 이미 다른 그룹에 있었다면 그 그룹에서 탈퇴합니다. 차트는
다음 프레임에서 그룹의 현재 크로스헤어와 표시 범위를 이어받습니다.

`SyncOptions`로 차트가 공유할 항목을 고릅니다. 각 옵션은 양방향으로 동작합니다.
`crosshair: false`인 차트는 다른 차트의 크로스헤어를 표시하지도, 자신의
크로스헤어를 보내지도 않습니다.

| 옵션 | 공유 대상 | 기본값 |
|---|---|---|
| `crosshair` | 크로스헤어 시간 | `true` |
| `visibleRange` | 표시 시간 범위(이동과 확대·축소) | `true` |
| `drawings` | 메인 패널의 드로잉 | `false` |

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const group: SyncGroup;
declare const overview: FerroChart;
declare const detail: FerroChart;

// The overview shows the crosshair of the others but keeps its own zoom.
overview.joinSync(group, { visibleRange: false });
detail.joinSync(group);
```

차트의 옵션을 바꾸려면 같은 그룹으로 `joinSync`를 다시 호출하세요.

### `chart.leaveSync()`

그룹에서 탈퇴합니다. 이 차트가 다른 차트에 표시하던 크로스헤어는 제거됩니다.
`chart.destroy()`도 그룹에서 탈퇴합니다.

### `chart.syncOptions()`

차트가 공유하는 항목을 모든 옵션이 채워진 형태로 반환하며, 그룹에 속해 있지
않으면 `null`을 반환합니다.

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const opts = chart.syncOptions();
if (opts === null) console.log('not synced');
else console.log(opts.crosshair, opts.visibleRange, opts.drawings);
```

## 무엇을 언제 보내는가

- **크로스헤어.** 포인터가 차트의 플롯 위에서 움직이면 차트는 크로스헤어 아래의
  시간을 보내고, 포인터가 벗어나면 크로스헤어가 사라졌다고 보냅니다. 다른 차트들은
  그 시간에 크로스헤어를 표시합니다. 이때 그 차트들의 `crosshairMove` 이벤트는
  `price: NaN`이고 `x`/`y`가 없으며 `pane`이 메인 패널로 설정된 `point`를 담습니다.
- **표시 범위.** 사용자(드래그, 휠, 핀치, 키)나 코드(`setVisibleLogicalRange`,
  `setBarSpacing`, `fitContent` 등)에 의해 뷰가 바뀌면, 차트는 왼쪽과 오른쪽 끝의
  시간을 보냅니다. 같은 프레임에서 차트가 자신의 뷰를 바꿨다면 받은 범위보다 그
  변경이 우선합니다.
- 받은 메시지를 적용해도 다시 보내지 않으므로, 메시지가 차트 사이를 오가며 되돌아올
  수 없습니다.

메시지를 보낸 차트는 그룹의 다른 차트들에 프레임을 예약하며, 그 차트들은 해당
프레임을 그릴 때 메시지를 적용합니다.

## 드로잉 동기화

`drawings: true`이면 차트의 **메인 패널**에 있는 드로잉이 드로잉을 공유하는 다른
차트의 메인 패널로 복사됩니다. 추가, 이동, 스타일 변경, 표시, 숨김, 잠금, 제거가
모두 해당됩니다. 앵커가 가격이므로 같은 종목의 차트(예: 여러 타임프레임)에
사용하세요.

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const group: SyncGroup;
declare const m5: FerroChart;
declare const h1: FerroChart;

m5.joinSync(group, { drawings: true });
h1.joinSync(group, { drawings: true });

// Appears on h1 at its next frame.
m5.drawings.add({
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 2), price: 42_000 }],
});
```

세부 사항:

- `drawings: true`로 참여한 차트는 먼저 그룹에 이미 있는 드로잉을 이어받은 다음
  자신의 드로잉을 공유합니다.
- 다른 패널의 드로잉은 공유되지 않습니다(다른 패널은 차트마다 다른 지표를 보여 줄
  수 있습니다). 받은 드로잉은 항상 메인 패널로 갑니다.
- 받은 변경은 받는 차트의 실행 취소 기록에 추가되지 않습니다. 변경이 일어난
  차트에서의 실행 취소와 다시 실행은 다른 변경과 똑같이 전송되므로, 모든 차트가
  같은 상태가 됩니다.
- 선택 상태와 아직 생성 중인 드로잉은 공유되지 않습니다.
- 모든 차트가 해당 드로잉 종류를 알고 있어야 합니다. JavaScript로 작성한 드로잉
  도구(`chart.drawings.register`)는 페이지의 모든 차트에 대해 한 번만 등록됩니다.

## 그룹 객체를 공유하지 않는 차트

`SyncGroup`은 한 페이지에서 같은 WebAssembly 모듈 인스턴스가 만든 차트들을
연결합니다. 다른 창, 탭, iframe, 네이티브 애플리케이션 등 다른 곳에 있는 차트는
참여할 수 없습니다. 이런 경우 차트가 보내는 내용을 보고하므로, 원하는 전송 수단
(`postMessage`, `BroadcastChannel`, WebSocket)으로 전달하면 됩니다.

보내는 차트는 그룹에 속해 있어야 합니다(차트 하나뿐인 그룹도 괜찮습니다). 그룹에
속한 차트만 메시지를 보고합니다.

### 크로스헤어와 표시 범위

`sync` 이벤트는 이 차트가 그룹에 보낸 각 메시지를 전달합니다. 받는 쪽에서는
`chart.applySync(message)`로 적용합니다.

```ts
import { FerroChart, SyncGroup, type SyncMessage } from '@ferrochart/core';

declare const chart: FerroChart;

const channel = new BroadcastChannel('chart-sync');
const group = new SyncGroup();
chart.joinSync(group);

// Out: what this chart sent.
chart.on('sync', ({ message }: { message: SyncMessage }) => {
  channel.postMessage(message);
});

// In: what a chart in another window sent.
channel.onmessage = (e: MessageEvent<SyncMessage>) => {
  chart.applySync(e.data);
};
```

`SyncMessage`는 단순한 데이터입니다.

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }       // null: the crosshair left
  | { type: 'visibleRange'; from: number; to: number }; // times at the plot edges, UTC ms
```

`applySync`는 호출된 차트에만 영향을 줍니다. 다시 전송되지도, 그 차트의 그룹으로
전달되지도 않습니다. 받는 쪽에서는 따라가야 할 모든 차트에 대해 호출하세요.

`applySync({ type: 'visibleRange', from, to })`는 차트 하나에 특정 시간 범위를
보여 주는 간편한 방법이기도 합니다.

### 드로잉

드로잉 변경은 `DrawingSyncOp` 값으로 전달됩니다. 각 드로잉에는 차트 간에
드로잉을 식별하는 `key`가 있습니다.

```ts
type DrawingSyncOp =
  | { type: 'add'; key: number; data: DrawingData; visible: boolean; locked: boolean }
  | { type: 'update'; key: number; data: DrawingData }
  | { type: 'setVisible'; key: number; visible: boolean }
  | { type: 'setLocked'; key: number; locked: boolean }
  | { type: 'remove'; key: number };
```

| 멤버 | 설명 |
|---|---|
| `drawingSync` 이벤트 | 차트가 드로잉 작업을 보냈습니다. 한 번만 전송되며, 다음 이벤트는 `takeDrawingSyncOps()`를 호출한 뒤에 옵니다 |
| `takeDrawingSyncOps()` | 보냈지만 아직 가져가지 않은 작업을 오래된 순서로 반환합니다. 최대 1024개까지 보관하며 그보다 오래된 것은 버립니다 |
| `applyDrawingSync(op)` | 다른 차트에서 온 작업 하나를 적용합니다(메인 패널, 실행 취소 불가, 다시 전송하지 않음) |
| `drawingSyncState()` | 나중에 참여하는 차트를 위해, 이 차트의 동기화된 드로잉 전체를 `'add'` 작업으로 반환합니다 |
| `loadDrawingSyncState(ops)` | 동기화된 드로잉을 그 목록과 일치시킵니다. 추가하고, 업데이트하고, 목록에 없는 동기화된 드로잉을 제거합니다 |

키는 그룹이 할당합니다. 그룹의 `origin`에 2³²을 곱한 값에 카운터를 더한
값입니다. 여러 페이지가 서로 드로잉을 전달할 때는 `new SyncGroup(origin)`으로
페이지마다 그룹에 고유한 origin을 지정해, 서로 다른 페이지의 키가 충돌하지 않게
하세요. origin은 0(기본값)부터 2²¹ − 1까지의 정수입니다. 더 크면 예외가
발생하며, 이렇게 해서 키가 JavaScript 숫자로 정확하게 유지됩니다.

```ts
import { FerroChart, SyncGroup, type DrawingSyncOp } from '@ferrochart/core';

declare const chart: FerroChart;
declare const pageNumber: number; // different on every page, 0 to 2^21 - 1

type Msg =
  | { type: 'ops'; ops: DrawingSyncOp[] }
  | { type: 'hello' }
  | { type: 'state'; ops: DrawingSyncOp[] };

const channel = new BroadcastChannel('chart-drawings');
const group = new SyncGroup(pageNumber);
chart.joinSync(group, { drawings: true });

const send = (m: Msg) => channel.postMessage(m);

// Out: forward this chart's changes.
chart.on('drawingSync', () => send({ type: 'ops', ops: chart.takeDrawingSyncOps() }));

channel.onmessage = (e: MessageEvent<Msg>) => {
  const m = e.data;
  if (m.type === 'ops') {
    for (const op of m.ops) {
      try {
        chart.applyDrawingSync(op);
      } catch (err) {
        console.warn('drawing op skipped:', err);
      }
    }
  } else if (m.type === 'hello') {
    // A page that just opened asks for the current drawings.
    send({ type: 'state', ops: chart.drawingSyncState() });
  } else {
    chart.loadDrawingSyncState(m.ops);
  }
};

send({ type: 'hello' });
```

오류:

- `applyDrawingSync`는 차트에 없는 키에 대한 `update`, `setVisible`,
  `setLocked`, `remove`, 그리고 만들 수 없는 드로잉(등록되지 않은 종류, 잘못된
  앵커나 매개변수)에 대해 예외를 던집니다. 차트에 이미 있는 키에 대한 `add`는 그
  드로잉을 업데이트합니다.
- `loadDrawingSyncState`는 작업이 `'add'`가 아니거나, 키가 두 번 나오거나,
  드로잉을 만들 수 없으면 차트를 바꾸지 않은 채 예외를 던집니다.

## 예제: 멀티 타임프레임 레이아웃

한 종목의 1분, 5분, 1시간, 1일 차트 네 개입니다. 장중 차트 세 개는 크로스헤어,
확대·축소, 드로잉을 공유합니다. 일봉 차트는 다른 차트의 크로스헤어를 표시하지만
자신의 뷰를 유지하므로 개요 차트 역할을 합니다.

```html
<div class="grid">
  <canvas id="m1"></canvas><canvas id="m5"></canvas>
  <canvas id="h1"></canvas><canvas id="d1"></canvas>
</div>
<style>
  .grid { display: grid; grid-template: 1fr 1fr / 1fr 1fr; gap: 4px; height: 100vh; }
  .grid canvas { width: 100%; height: 100%; min-height: 0; }
</style>
```

```ts
import { FerroChart, SyncGroup, randomWalk, type SyncOptions } from '@ferrochart/core';

const MIN = 60_000;
const start = Date.UTC(2024, 0, 1);

const layout: { id: string; interval: number; sync: SyncOptions }[] = [
  { id: 'm1', interval: MIN, sync: { drawings: true } },
  { id: 'm5', interval: 5 * MIN, sync: { drawings: true } },
  { id: 'h1', interval: 60 * MIN, sync: { drawings: true } },
  { id: 'd1', interval: 1_440 * MIN, sync: { visibleRange: false } },
];

const group = new SyncGroup();
const charts: FerroChart[] = [];

for (const { id, interval, sync } of layout) {
  const canvas = document.getElementById(id) as HTMLCanvasElement;
  const chart = await FerroChart.create(canvas, { theme: 'dark' });
  const candles = chart.addCandlestickSeries();
  // Replace with your data; all four cover the same period.
  const bars = Math.round((90 * 1_440 * MIN) / interval);
  candles.setData(randomWalk(7, bars, start, interval));
  chart.addVolumeSeries(candles);
  chart.joinSync(group, sync);
  charts.push(chart);
}

// Show the last two days of the 1-minute chart; m5 and h1 follow it.
const range = charts[0].dataRange();
if (range) charts[0].setVisibleLogicalRange(range.to - 2 * 1_440, range.to);
charts[3].fitContent();
```

이 예제의 데이터는 무작위이므로 네 차트의 가격이 서로 맞지 않습니다. 한 종목의
실제 데이터를 쓰면 가격이 일치합니다.

이 레이아웃에 대한 참고 사항:

- `charts[0]`에 대한 `setVisibleLogicalRange`는 사용자의 이동처럼 뷰를 바꾸므로,
  변경이 그룹으로 전송되어 `m5`와 `h1`이 따라갑니다.
  (`applySync`라면 `charts[0]`만 바뀝니다. 적용된 메시지는 다시 전송되지
  않습니다.)
- 일봉 차트는 `visibleRange: false`로 참여합니다. 다른 차트를 따라가지도 움직이지도
  않지만, 그 차트들의 크로스헤어는 표시합니다.
- 동기화된 차트는 메시지가 도착하거나 자신의 내용이 바뀔 때 다시 그립니다. 유휴
  상태의 차트는 다시 그리지 않습니다.

## Rust

Rust에서는 같은 기능이 `Chart::join_sync(&group, SyncOptions)`,
`Chart::leave_sync`, `Chart::apply_sync`, `ChartEvent::Sync`,
`Chart::take_drawing_sync_ops`, `Chart::apply_drawing_sync`,
`Chart::drawing_sync_state`, `Chart::load_drawing_sync_state`이며,
`SyncGroup::new()`와 `SyncGroup::with_origin(origin)`을 사용합니다.
