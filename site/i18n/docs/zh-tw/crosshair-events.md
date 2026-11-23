<!-- source: 5f948529f4 -->
# 十字游標與事件

圖表透過事件回報其上發生的事情：十字游標移動、點擊、可見範圍變更、繪圖被建立或編輯，以及其他幾種事件。本章列出所有事件、其內容（payload）與觸發時機，並示範兩種常見用法：跟隨十字游標的圖例，以及回應對標記與系列的點擊。

## 十字游標模式

十字游標會跟隨指標在繪圖區上移動。它的垂直線貫穿所有窗格；水平線與價格標籤則屬於指標所在的窗格。時間標籤顯示在時間軸上。

| 模式 | 行為 |
|---|---|
| `'normal'`（預設） | x 吸附到最近的 K 棒中心，y 跟隨指標 |
| `'magnet'` | 與 `'normal'` 相同，且 y 會吸附到該 K 棒（窗格中第一個價格系列）開、高、低、收中最接近的一個 |
| `'hidden'` | 不繪製十字游標，`crosshairMove` 回報 `null` |

```ts
chart.setCrosshairMode('magnet');
console.log(chart.crosshairMode); // 'magnet'
```

傳入未知的模式會擲出例外。模式是已儲存狀態的一部分（`saveState().crosshairMode`，參見[儲存狀態](state.md)）。

磁吸模式在放置繪圖時同樣適用：錨點會吸附到最接近的開、高、低或收盤價（參見[繪圖工具](drawings.md)）。

## 訂閱

```ts
import type { ChartEventMap } from '@ferrochart/core';

const onMove = (e: ChartEventMap['crosshairMove']) => {
  console.log(e.point?.time);
};

chart.on('crosshairMove', onMove);
// later
chart.off('crosshairMove', onMove);
```

- `on(name, callback)` 新增一個監聽器。未知的事件名稱會擲出例外。
- `off(name, callback)` 將其移除。請傳入與傳給 `on` 相同的函式物件。
- 同一個函式加入兩次就會被呼叫兩次；`off` 會將兩者一併移除。
- 回呼函式中可以呼叫任何圖表方法，包括 `on` 與 `off`。
- 回呼函式內擲出的例外會像未捕捉的錯誤一樣被回報（`reportError`：會出現在主控台並傳到 `window.onerror`）。其他回呼函式仍會照常執行。

回呼參數的型別由事件名稱推導，因此 `chart.on('crosshairMove', (e) => e.point?.time)` 不需要型別標註。若像上例那樣另外宣告回呼函式，可用 `ChartEventMap[name]`（由套件匯出）取得 payload 型別。

## 傳遞時機

圖表在 `requestAnimationFrame` 中繪製。每一影格，它會先套用自上一影格以來的輸入與 API 呼叫，建構並渲染該影格，然後才傳遞該影格產生的事件。因此：

- 事件絕不會在 DOM 輸入處理常式內，或在引發它的方法呼叫中傳遞。`chart.drawings.add(...)` 會立即傳回 id；對應的 `drawingCreated` 事件則在下一影格之後才到達。
- 每次最多只傳遞一個影格的事件。若十字游標自上一影格以來移動了好幾次，你只會收到一個帶有最新位置的 `crosshairMove`，而不是每個指標事件各一個。`visibleRangeChanged` 與 `sync` 也是如此。
- 瀏覽器會在背景分頁中暫停 `requestAnimationFrame`，因此事件會延遲到分頁再次可見時才傳遞。

## 生命週期

圖表透過畫布與 window 上的監聽器維持運作，而這些監聽器只持有對它的弱參照。只要圖表還在頁面上，就要保留對 `FerroChart` 物件的參照。若該物件被垃圾回收，圖表會停止繪製，也不再傳送事件。用完圖表時請呼叫 `destroy()`：它會移除所有事件監聽器（你的與 DOM 的），並釋放 GPU 資源。之後該物件便無法再使用。

```ts
chart.destroy();
```

## 事件參考

| 事件 | Payload | 觸發時機 |
|---|---|---|
| `click` | `ClickEvent` | 在繪圖區內按下並放開，且移動不超過 3 CSS px |
| `crosshairMove` | `{ point: CrosshairPoint \| null }` | 十字游標移到另一根 K 棒、另一個窗格或價格，或離開繪圖區 |
| `visibleRangeChanged` | `{ from, to }` | 可見的邏輯範圍改變 |
| `needMoreHistory` | `{ barsBeforeStart }` | 視圖左緣距離第一根 K 棒不到一個畫面寬 |
| `drawingCreated` | `{ id }` | 建立了一個繪圖 |
| `drawingModified` | `{ id }` | 繪圖被移動、更改樣式或變更參數 |
| `drawingRemoved` | `{ id }` | 移除了一個繪圖 |
| `drawingSelected` | `{ id: number \| null }` | 選取的繪圖改變 |
| `sync` | `{ message: SyncMessage }` | 圖表向其同步群組發送了一則訊息 |
| `drawingSync` | `{}` | 圖表有等待取用的繪圖同步操作 |
| `warning` | `ChartWarning` | 圖表自行調整了某些東西 |
| `markerHover` | `MarkerHoverEvent \| null` | 指標移到某個標記或「+k」徽章上，或從其上移開 |

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
  | { type: 'markerOverflow'; series: Series; first: number; count: number }
  | { type: 'series'; series: Series };
```

點擊是指在繪圖區內按下並放開（滑鼠、觸控筆或觸控），且期間指標移動不超過 3 CSS px；移動更多則視為平移。建立繪圖的過程中不會傳送點擊事件。

`target` 是指標下方的物件，依下列順序判斷：

1. **繪圖**（其本體或任一控制點）：`{ type: 'drawing', id }`。按下繪圖也會選取它。
2. **標記**：`{ type: 'marker', series, index }`，其中 `index` 是該標記在 `series.markers()`（依時間排序）中的位置。點擊收合標記的「+k」徽章會得到 `{ type: 'markerOverflow', series, first, count }`：未繪製的標記為 `series.markers().slice(first, first + count)`（參見[標記](markers.md)）。
3. **系列**：當指標位於 K 線實體或影線上、距離線條 4 CSS px 以內、在直方圖柱內或在點上時，為 `{ type: 'series', series }`。若系列互相重疊，以最後加入者為準。
4. 空白處則為 `null`。

target 中的 `Series` 物件是指向既有系列的新控制代碼。請以 `id` 比較，而不是用 `===`：

```ts
chart.on('click', (e: ChartEventMap['click']) => {
  if (e.target?.type === 'series' && e.target.series.id === candles.id) {
    console.log('clicked the candles at', e.time);
  }
});
```

指標輸出就是一般的系列，因此點擊指標線會得到一個 `series` target，其 `id` 與 `indicator.outputs()` 中的某一個相符。

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

當指標離開繪圖區或十字游標隱藏時，`point` 為 `null`。只有在 point 中的內容改變時才會傳送此事件；在同一根 K 棒與同一價格內移動指標不會重複觸發。

在磁吸模式下，`price` 是吸附後的價格。若該索引處沒有任何 OHLC 系列的 K 棒（例如只有折線系列的圖表，或資料缺口），`bar` 為 `null`。要讀取其他系列在十字游標處的值，請使用 `series.dataAt(point.logicalIndex)`。

位於[同步群組](sync.md)中的圖表也會回報其他圖表的十字游標。這類 point 的 `price: NaN`，`pane` 設為主窗格，且沒有 `x` 或 `y`。

### visibleRangeChanged

`{ from: number, to: number }` 是繪圖區左、右邊緣的邏輯索引，可為小數。平移、縮放、調整大小，以及改變視圖的方法（`setVisibleLogicalRange`、`fitContent`、`scrollToRealtime`、`setBarSpacing`、`setRightOffset`）都會觸發此事件。相同的值隨時可從 `chart.visibleLogicalRange()` 取得。

### needMoreHistory

`{ barsBeforeStart: number }` 是第一根 K 棒與視圖左緣之間的 K 棒數。若視圖已經顯示資料之前的空白，此值為負數。每個第一根 K 棒只會傳送一次此事件：在你加入更舊的資料之後，使用者捲動到新的開頭時，它可能再次觸發。

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

載入資料請參見[資料](data.md)。

### 繪圖事件

`drawingCreated`、`drawingModified` 與 `drawingRemoved` 帶有繪圖的 `id`；`drawingSelected` 帶有被選取的 id 或 `null`。

| 事件 | 觸發來源 |
|---|---|
| `drawingCreated` | 使用者完成繪圖、`drawings.add`、使繪圖恢復的復原或重做（沿用其舊 id）、`loadState`（每個載入的繪圖各一次）、從同步群組收到的繪圖 |
| `drawingModified` | 拖曳繪圖或控制點、`setAnchors`、`setStyle`、`setParam`、此類變更的復原或重做、同步而來的變更 |
| `drawingRemoved` | Delete 鍵、`drawings.remove`、建立動作的復原、`loadState`（針對被取代的繪圖）、移除窗格、同步而來的移除 |
| `drawingSelected` | 按下繪圖或空白處、完成新繪圖（新繪圖會被選取）、`drawings.select`、隱藏或移除被選取的繪圖 |

顯示、隱藏、鎖定繪圖以及變更其 z 順序都不會傳送事件。參見[繪圖工具](drawings.md)。

### sync

當圖表向其[同步群組](sync.md)發送十字游標或可見範圍訊息時，會傳送 `{ message: SyncMessage }`：

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }
  | { type: 'visibleRange'; from: number; to: number };
```

同一頁面上、同一個 `SyncGroup` 中的圖表不需要你做任何事。此事件用於把訊息轉發給其他地方的圖表（例如 worker 中的圖表），由它們以 `applySync(message)` 套用。不在群組中的圖表，或其群組不共享該類訊息的圖表，不會傳送 `sync` 事件。

### drawingSync

`{}`。當圖表在同步群組中共享繪圖，且有頁面尚未取用的繪圖操作時傳送。它只傳送一次；要等你呼叫 `chart.takeDrawingSyncOps()` 之後才會再次傳送。參見[同步多個圖表](sync.md)。

### warning

圖表自行修正了某些東西，而引發它的呼叫已成功完成。

```ts
type ChartWarning =
  | { kind: 'timeAxisMerged'; series: Series; count: number }
  | { kind: 'ohlcRepaired'; series: Series; count: number }
  | { kind: 'dataDropped'; series: Series }
  | { kind: 'other' };
```

| `kind` | 意義 |
|---|---|
| `'timeAxisMerged'` | `series` 的時間落在時間軸的 K 棒之間。時間軸已重建為所有系列時間的聯集，第一根插入的 K 棒之後的邏輯索引隨之移動。共插入了 `count` 根 K 棒。 |
| `'ohlcRepaired'` | `series` 中有 `count` 根 K 棒的最高價低於、或最低價高於其他價格，已被擴展修正。（JavaScript API 會直接拒絕此類 K 棒，因此由 JavaScript 驅動的圖表不會傳送此警告。） |
| `'dataDropped'` | `series` 的資料無法移到重建後的時間軸上（記憶體不足），已被清除。 |
| `'other'` | 後續版本新增的類型。 |

```ts
chart.on('warning', (w) => {
  if (w.kind === 'timeAxisMerged') console.warn(`time axis rebuilt: ${w.count} bars inserted by series ${w.series.id}`);
});
```

### markerHover

```ts
interface MarkerHoverEvent {
  series: Series;
  index: number;          // position in series.markers(); for a badge, the first marker not drawn
  overflow: number;       // k for a "+k" badge, 0 for a marker
  x: number;              // center, CSS px from the canvas' top-left corner
  y: number;
  size: number;           // diameter, CSS px
}
```

指標離開標記時，事件內容為 `null`。從一個標記直接移到另一個標記時，只會傳送新的那一個。指標下方的標記是在繪製幀時判定的，因此此事件每幀最多觸發一次；不考慮繪圖，且在開場動畫播放期間，不會有任何標記被視為懸停中。可用於在頭像或徽章上顯示提示框（參見[標記](markers.md)）。

## 範例：跟隨十字游標的圖例

圖表上方的一個 DOM 元素會顯示十字游標所在的 K 棒；指標不在圖表上時則顯示最新的 K 棒。

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

`dataAt` 對 OHLC 系列傳回 `{ open, high, low, close, volume }`，對其他系列傳回 `{ value }`，無資料或遇到缺口時傳回 `null`。指標輸出在暖機期間的 K 棒為 `NaN`。當即時更新改變了最新的 K 棒，且指標不在圖表上時，請在 `update` 之後呼叫 `showLatest()`。

## 範例：點擊標記與系列

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

標記索引指的是 `series.markers()` 傳回的、依時間排序的清單，而不是你傳給 `setMarkers` 的順序。若之後需要再找到對應資料，請自行以時間為鍵保存資料。參見[標記與價格線](markers.md)。
