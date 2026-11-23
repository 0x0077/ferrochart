<!-- source: 37271b3d45 -->
# 同步多個圖表

加入同一個 `SyncGroup` 的圖表會彼此跟隨：在其中一個圖表上移動十字游標，其他圖表也會顯示；平移或縮放其中一個，其他圖表會捲動到相同的時間範圍。也可以選擇讓在某個圖表上的繪圖出現在其他圖表上。

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

`new SyncGroup()` 需要 WebAssembly 模組。模組會在第一次呼叫 `FerroChart.create` 時載入；若要在建立任何圖表之前建立群組，請先呼叫 `await init()`（`import init from '@ferrochart/core'`）。

## 以時間為準，而非 K 棒編號

圖表之間的每則訊息都攜帶**時間**（UTC ms），絕不是邏輯索引。因此同一群組中的圖表可以顯示不同的商品、不同的週期，或缺口位置各不相同的資料：

- 1 分鐘圖上 10:37 的十字游標，在 1 小時圖上會顯示在 10:00 那根 K 棒上（該時間或之前的最後一根 K 棒）。超出資料兩端時，時間會以外推方式計算。
- 平移或縮放之後，其他圖表會在其 K 棒間距限制允許的範圍內，顯示從左緣到右緣相同的時間跨度（右緣保持不變）。若 1 小時圖跟隨一張顯示兩小時的 1 分鐘圖，整個繪圖區只需要兩根 K 棒；在預設最大 K 棒間距 50 px 下，它會顯示更多 K 棒，並結束於相同的時間。

沒有資料的圖表會忽略收到的範圍。

## 加入與離開

### `chart.joinSync(group, options?)`

加入 `group`；若圖表原本已在某個群組中，會先離開該群組。在下一影格，圖表會接手群組目前的十字游標與可見範圍。

`SyncOptions` 決定圖表要共享哪些內容。每個選項都是雙向的：設為 `crosshair: false` 的圖表既不顯示其他圖表的十字游標，也不傳送自己的。

| 選項 | 共享內容 | 預設值 |
|---|---|---|
| `crosshair` | 十字游標的時間 | `true` |
| `visibleRange` | 可見的時間範圍（平移與縮放） | `true` |
| `drawings` | 主窗格的繪圖 | `false` |

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const group: SyncGroup;
declare const overview: FerroChart;
declare const detail: FerroChart;

// The overview shows the crosshair of the others but keeps its own zoom.
overview.joinSync(group, { visibleRange: false });
detail.joinSync(group);
```

要變更圖表的選項，請以同一個群組再次呼叫 `joinSync`。

### `chart.leaveSync()`

離開群組。此圖表先前在其他圖表上顯示的十字游標會被移除。`chart.destroy()` 也會離開群組。

### `chart.syncOptions()`

傳回圖表共享的內容（所有選項都已填入），若不在群組中則傳回 `null`。

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const opts = chart.syncOptions();
if (opts === null) console.log('not synced');
else console.log(opts.crosshair, opts.visibleRange, opts.drawings);
```

## 傳送的內容與時機

- **十字游標。** 當指標在圖表繪圖區上移動時，圖表會傳送十字游標所在的時間；指標離開時，則傳送十字游標已消失的訊息。其他圖表會在該時間顯示十字游標。此時它們的 `crosshairMove` 事件會帶有一個 `price: NaN`、沒有 `x`/`y`、且 `pane` 設為主窗格的 `point`。
- **可見範圍。** 當視圖改變時，無論是使用者操作（拖曳、滾輪、雙指縮放、鍵盤）或你的程式碼（`setVisibleLogicalRange`、`setBarSpacing`、`fitContent` 等），圖表都會傳送其左、右邊緣的時間。若圖表在同一影格中自行改變了視圖，該變更優先於收到的範圍。
- 套用收到的訊息絕不會再次傳送它，因此訊息不會在圖表之間來回反彈。

傳送訊息的圖表會為群組中的其他圖表排定一個影格；它們在繪製該影格時套用訊息。

## 同步繪圖

設定 `drawings: true` 時，圖表**主窗格**上的繪圖會複製到其他共享繪圖之圖表的主窗格：包括新增、移動、更改樣式、顯示、隱藏、鎖定與移除。由於錨點是價格，請將它用於同一商品的圖表（例如多個時間週期）。

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

細節：

- 以 `drawings: true` 加入的圖表，會先接手群組既有的繪圖，再共享自己的繪圖。
- 其他窗格上的繪圖不會共享（每個圖表的其他窗格可能顯示不同的指標）。收到的繪圖一律放到主窗格。
- 收到的變更不會加入接收端圖表的復原歷程。在發生變更的圖表上執行的復原與重做，會像其他變更一樣傳送出去，因此所有圖表最終會一致。
- 選取狀態與仍在建立中的繪圖不會共享。
- 每個圖表都必須認得該繪圖種類。以 JavaScript 撰寫的繪圖工具（`chart.drawings.register`）只需為頁面上的所有圖表註冊一次。

## 不共用群組物件的圖表

`SyncGroup` 連接的是同一頁面上、由同一個 WebAssembly 模組實例建立的圖表。其他地方的圖表，例如另一個視窗、分頁或 iframe，或原生應用程式中的圖表，無法加入。對這些圖表，圖表會回報它傳送的內容，由你以任何喜歡的傳輸方式轉發（`postMessage`、`BroadcastChannel`、WebSocket）。

傳送端的圖表必須在某個群組中（只有一個成員的群組也可以）：只有在群組中的圖表才會回報其訊息。

### 十字游標與可見範圍

`sync` 事件會傳遞此圖表傳送給其群組的每一則訊息。在另一端，由 `chart.applySync(message)` 套用。

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

`SyncMessage` 是純資料：

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }       // null: the crosshair left
  | { type: 'visibleRange'; from: number; to: number }; // times at the plot edges, UTC ms
```

`applySync` 只影響被呼叫的那個圖表；它既不會再次傳送，也不會傳給該圖表自己的群組。在接收端，請對每個應該跟隨的圖表呼叫它。

`applySync({ type: 'visibleRange', from, to })` 也是在單一圖表上顯示某段時間範圍的便利方法。

### 繪圖

繪圖變更以 `DrawingSyncOp` 值傳遞。每個繪圖都有一個 `key`，用來在不同圖表之間識別它：

```ts
type DrawingSyncOp =
  | { type: 'add'; key: number; data: DrawingData; visible: boolean; locked: boolean }
  | { type: 'update'; key: number; data: DrawingData }
  | { type: 'setVisible'; key: number; visible: boolean }
  | { type: 'setLocked'; key: number; locked: boolean }
  | { type: 'remove'; key: number };
```

| 成員 | 說明 |
|---|---|
| `drawingSync` 事件 | 圖表已傳送繪圖操作。只傳送一次；下一次要在你呼叫 `takeDrawingSyncOps()` 之後才會到來 |
| `takeDrawingSyncOps()` | 已傳送但尚未取用的操作，由舊到新。最多保留 1024 個；更舊的會被丟棄 |
| `applyDrawingSync(op)` | 套用來自另一個圖表的一個操作（主窗格、不可復原、不會再次傳送） |
| `drawingSyncState()` | 此圖表所有已同步的繪圖，以 `'add'` 操作表示，供之後才加入的圖表使用 |
| `loadDrawingSyncState(ops)` | 讓已同步的繪圖與這樣的清單一致：新增、更新，並移除不在清單中的已同步繪圖 |

key 由群組指派：群組的 `origin` 乘以 2³²，再加上一個計數器。當多個頁面互相轉發繪圖時，請用 `new SyncGroup(origin)` 為每個頁面的群組指定各自的 origin，以免不同頁面的 key 衝突。origin 是 0（預設）到 2²¹ − 1 之間的整數；更大的值會擲出例外，如此可確保 key 作為 JavaScript 數字時仍保持精確。

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

錯誤：

- 若對圖表沒有的 key 執行 `update`、`setVisible`、`setLocked` 或 `remove`，或遇到無法建立的繪圖（未註冊的種類、錯誤的錨點或參數），`applyDrawingSync` 會擲出例外。對圖表已有的 key 執行 `add` 則會更新該繪圖。
- 若有操作不是 `'add'`、某個 key 出現兩次，或某個繪圖無法建立，`loadDrawingSyncState` 會擲出例外，圖表保持不變。

## 範例：多時間週期版面

同一商品的四張圖表，週期分別為 1 分鐘、5 分鐘、1 小時與 1 日。三張日內圖表共享十字游標、縮放與繪圖。日線圖顯示其他圖表的十字游標，但保留自己的視圖，因此始終作為總覽。

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

此範例中的資料是隨機產生的，因此四張圖表顯示的價格並不相同；若使用同一商品的真實資料，它們就會對齊。

關於此版面的說明：

- 對 `charts[0]` 呼叫 `setVisibleLogicalRange` 會像使用者平移一樣改變其視圖，因此變更會傳送給群組，`m5` 與 `h1` 隨之跟隨。（`applySync` 只會改變 `charts[0]`：套用的訊息不會再轉送出去。）
- 日線圖以 `visibleRange: false` 加入：它既不跟隨其他圖表，也不移動其他圖表，但仍會顯示它們的十字游標。
- 已同步的圖表在收到訊息或自身內容改變時才重繪；閒置的圖表不會重繪。

## Rust

在 Rust 中，相同功能為 `Chart::join_sync(&group, SyncOptions)`、`Chart::leave_sync`、`Chart::apply_sync`、`ChartEvent::Sync`、`Chart::take_drawing_sync_ops`、`Chart::apply_drawing_sync`、`Chart::drawing_sync_state` 與 `Chart::load_drawing_sync_state`，搭配 `SyncGroup::new()` 與 `SyncGroup::with_origin(origin)`。
