<!-- source: 80fd5ef585 -->
# 時間軸與視窗

時間軸決定哪些 K 棒可見以及它們的寬度。它由所有窗格共用。本章說明 K 棒間距、右側偏移、可見範圍、像素／邏輯索引／時間之間的轉換，以及回報視圖變化的事件。

## 邏輯索引

時間軸使用邏輯索引而非時間運作。`setData` 之後，第一根 K 棒的索引為 0，下一根為 1，依此類推；K 棒 `i` 以 `i` 為中心，涵蓋 `i − 0.5` 到 `i + 0.5`。K 棒之間及資料範圍以外的位置，其索引為小數或超出範圍。請用 `timeAtIndex` 與 `indexOfTime` 在索引與時間之間轉換（參見[資料](data.md#reading-data-back)）。

索引計算的是 K 棒而非時間：沒有 K 棒的週末或交易暫停不占任何空間。

## K 棒間距

K 棒間距是兩根相鄰 K 棒中心之間的距離，單位為 CSS 像素。縮放會改變它。

```ts
console.log(chart.barSpacing); // 6 on a new chart
chart.setBarSpacing(12);        // zoom in
```

| | 值 |
|---|---|
| 預設 | 6 CSS px |
| 最小 | 0.5 CSS px；為了一次顯示所有資料，必要時可以更小（繪圖區寬度的 90 % 除以 K 棒數） |
| 最大 | `maxBarSpacing`，預設 50 CSS px |

`setBarSpacing` 會把值限制在此範圍內。非有限或非正數的值會被忽略。右緣的 K 棒保持在原位。

最大間距在建立圖表時設定：

```ts
const chart = await FerroChart.create(canvas, { maxBarSpacing: 200 });
```

`maxBarSpacing` 必須是至少為 1 的數字，否則 `create` 會 reject。當單根 K 棒需要更多空間時請調高它：足跡圖的數字需要超過 60 px，TPO 字母則需要數百 px（參見[訂單流](orderflow.md)）。

## 右側偏移

右側偏移是最後一根 K 棒與繪圖區右緣之間的空白，以 K 棒數計。它可以是小數，也可以是負數（此時最後一根 K 棒位於右緣之外，這正是描述檢視較舊資料之視圖的方式）。

```ts
console.log(chart.rightOffset); // 3 on a new chart
chart.setRightOffset(10);        // more room for the forming bar
chart.scrollToRealtime();        // back to the latest bar, with the default offset of 3
```

`setRightOffset` 會讓畫面上至少保留一根 K 棒；超出此限制的值會被限制住，非有限的值則被忽略。

視圖透過右側偏移錨定在最新的 K 棒上。當 `update` 或 `setData` 在尾端加入 K 棒時，右側偏移保持不變，因此每新增一根 K 棒，內容就向左移動一根。使用者在看最新資料時，新的 K 棒會捲入視野；使用者往回捲動後，他們正在看的 K 棒也會向左移動。`scrollToRealtime()` 會把最新的 K 棒帶回來。

## 可見範圍

`visibleLogicalRange()` 以小數傳回繪圖區左、右邊緣的邏輯索引：

```ts
const { from, to } = chart.visibleLogicalRange();
const visibleBars = to - from;
```

`setVisibleLogicalRange(from, to)` 會在間距限制允許的範圍內，讓繪圖區恰好顯示該範圍（若無法做到，則保持右緣不變）：

```ts
// The last 100 bars.
const data = chart.dataRange();
if (data) chart.setVisibleLogicalRange(data.to - 99.5, data.to + 0.5);
```

`from` 必須小於 `to`，且兩者皆為有限值；否則會擲出例外，視圖不變。你可以在 `create` 之後、第一影格之前立即呼叫它：一旦得知繪圖區寬度就會套用該範圍。在此之前若發生平移或縮放，則會取消它。

`fitContent()` 會從左緣到右緣顯示所有 K 棒，不留右側偏移。K 棒很少時，最大 K 棒間距可能會在左側留下空白。它與 `setVisibleLogicalRange` 一樣會等待繪圖區寬度確定。

`dataRange()` 傳回第一根與最後一根 K 棒的索引（沒有資料時為 `null`），可用來計算相對於資料的範圍：

```ts
const data = chart.dataRange();
if (data) {
  // The first 200 bars.
  chart.setVisibleLogicalRange(data.from - 0.5, data.from + 199.5);
}
```

### 以時間指定範圍

用 `indexOfTime` 把時間轉換成索引。它需要一根時間完全相同的 K 棒；若要為任意時間找出最近的 K 棒，請在你自己保存的時間副本中搜尋。

```ts
function showTimes(fromMs: number, toMs: number) {
  const a = chart.indexOfTime(fromMs);
  const b = chart.indexOfTime(toMs);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a - 0.5, b + 0.5);
}
```

## 像素、索引與時間

| 方法 | 轉換 |
|---|---|
| `logicalToCoordinate(index)` | 邏輯索引 → 該 K 棒中心的 x，以距畫布左緣的 CSS px 表示 |
| `coordinateToLogical(x)` | x（距畫布左緣的 CSS px）→ 邏輯索引，可為小數 |
| `timeAtIndex(index)` | 邏輯索引 → 該 K 棒的時間（UTC ms），超出資料範圍時為 `null`；會四捨五入到最近的 K 棒 |
| `indexOfTime(time)` | 時間 → 時間完全相同之 K 棒的邏輯索引，不存在時為 `null` |

座標依據的是上一影格的版面，因此請在第一影格之後使用它們（例如在事件處理常式中）。x 座標在每個窗格中都相同。

```ts
// The bar under a point of the canvas.
canvas.addEventListener('dblclick', (e) => {
  const rect = canvas.getBoundingClientRect();
  const index = Math.round(chart.coordinateToLogical(e.clientX - rect.left));
  const time = chart.timeAtIndex(index);
  if (time !== null) console.log(new Date(time).toISOString(), candles.dataAt(index));
});

// Position an HTML element over a bar.
function placeOverBar(el: HTMLElement, time: number) {
  const index = chart.indexOfTime(time);
  if (index === null) return;
  el.style.left = `${chart.logicalToCoordinate(index)}px`;
}
```

對於十字游標與點擊，事件本身已攜帶邏輯索引、時間與價格；參見[十字游標與事件](crosshair-events.md)。

## visibleRangeChanged 事件

每當可見的邏輯範圍改變時就會傳送 `visibleRangeChanged`：無論是手勢、上述某個方法、調整大小，或是使視圖移動的資料所造成。

```ts
chart.on('visibleRangeChanged', ({ from, to }) => {
  const a = chart.timeAtIndex(Math.ceil(from));
  const b = chart.timeAtIndex(Math.floor(to));
  label.textContent = a !== null && b !== null
    ? `${new Date(a).toISOString()} – ${new Date(b).toISOString()}`
    : '';
});
```

事件在產生它的影格之後傳遞，每影格最多一次：快速平移時你會得到最新的範圍，而不是每一個中間範圍。圖表沒有資料時不會傳送任何事件。以 `chart.off('visibleRangeChanged', callback)` 移除監聽器。

當視圖距離第一根 K 棒不到一個畫面寬時，同一影格也可能傳送 `needMoreHistory`；參見[載入更早的歷史資料](data.md#loading-older-history)。

### 重新載入時保留視圖

以 `prepend` 載入更舊的 K 棒會保留所有邏輯索引與視圖。`setData` 會從 0 重新為 K 棒編號，因此邏輯索引會改變，但時間不會。要在取代資料後（例如換成時間戳記相同的另一個商品）還原視圖，請以時間形式儲存它：

```ts
function saveView() {
  const { from, to } = chart.visibleLogicalRange();
  return { from: chart.timeAtIndex(from), to: chart.timeAtIndex(to) };
}

function restoreView(view: { from: number | null; to: number | null }) {
  if (view.from === null || view.to === null) return chart.fitContent();
  const a = chart.indexOfTime(view.from);
  const b = chart.indexOfTime(view.to);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a, b);
}
```

若要把 K 棒間距與右側偏移連同繪圖和窗格版面一起儲存，請使用 `saveState`（參見[儲存狀態](state.md)）。若要讓多個圖表保持在相同範圍，請參見[同步圖表](sync.md)。

## 使用者手勢

以下輸入會改變時間軸（完整表格請參見[手勢與鍵盤](interaction.md)）：

| 輸入 | 效果 |
|---|---|
| 在繪圖區拖曳 | 平移（移動中放開：慣性捲動） |
| 滾輪／觸控板捲動 | 以指標為中心縮放時間 |
| 水平捲動或 Shift + 滾輪 | 平移 |
| 雙指縮放（觸控板、雙指） | 縮放 |
| 拖曳時間軸 | 改變 K 棒間距 |
| 雙擊時間軸 | 重設 K 棒間距 |
| ← → | 平移 10 % |
| ↑ ↓ | 縮放 |
| Home / End | 跳到第一根／最新的 K 棒 |

縮放時，指標下方的 K 棒保持在原位。每次變更都會傳送 `visibleRangeChanged`。
