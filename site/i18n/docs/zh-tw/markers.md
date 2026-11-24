<!-- source: 610fb1bc72 -->
# 標記與價格線

標記是附加在 K 棒上、可附帶文字的小圖形，例如買賣訊號或新聞。標記也可以顯示圖片（錢包或帳戶頭像）或簡短的徽章（例如 `DB`），擁擠的 K 棒還可以把標記收合成「+k」徽章。價格線是位於固定價格的水平線，例如委託單、停損或目標價。兩者都隸屬於某個系列。每個系列還有一個最新值標籤與一條最新值線，可透過系列選項開啟或關閉。

## 標記

### 設定標記

```ts
candles.setMarkers([
  { time: Date.UTC(2024, 0, 15), position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: Date.UTC(2024, 1, 2), position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
  { time: Date.UTC(2024, 1, 20), shape: 'circle', text: 'Earnings' },
]);
```

`setMarkers` 會取代系列的所有標記。要新增一個，請讀取目前的清單、附加後再重新設定；要清除，請傳入空陣列：

```ts
candles.setMarkers([...candles.markers(), { time: Date.UTC(2024, 2, 1), text: 'Split' }]);
candles.setMarkers([]);
```

### 標記欄位

| 欄位 | 型別 | 預設值 | 意義 |
|---|---|---|---|
| `time` | `number \| bigint \| Date` | 必填 | UTC 毫秒（整數）或 `Date`；`markers()` 傳回數字 |
| `id` | `number` | `0` | 自訂識別碼：0 到 2^53−1 的整數；0 表示無。在點擊和懸停事件中傳回。 |
| `position` | `'aboveBar' \| 'belowBar' \| 'atPrice'` | `'aboveBar'` | 位於最高價之上、最低價之下，或以 `price` 為中心。對單值系列而言，則位於數值之上或之下。 |
| `price` | `number` | 未設定 | 使用 `'atPrice'` 時必填且必須為有限值；其他位置不允許設定。以系列的價格單位表示。 |
| `shape` | `'circle' \| 'square' \| 'arrowUp' \| 'arrowDown'` | `'circle'` | 實心圖示 |
| `color` | `Color \| null` | `null` | 圖示與文字顏色；`null` 使用系列的主色（K 線與棒使用上漲顏色） |
| `text` | `string` | `''` | 圖示旁、遠離 K 棒那一側的文字；空字串表示不顯示 |
| `size` | `number \| null` | `null` | 直徑（方形則為邊長），4–64 CSS px；`null` 表示 10 |
| `icon` | `string \| null` | `null` | 以 `chart.registerIcon` 加入的圖片名稱，繪製在圓形或方形內 |
| `label` | `string` | `''` | 繪製在圓形或方形內、最多 8 個字元的文字，例如 `'DB'`；不可與 `icon` 併用 |
| `border` | `Color \| null` | `null` | 圖形邊緣內側的外環；`null` 表示不繪製 |
| `borderWidth` | `number` | `2` | 外環寬度，CSS px |

未知的 `position` 或 `shape`、缺少 `time` 或其非整數、無效的顏色、超出 4–64 的 `size`、超過 8 個字元的 `label`，或在箭頭上使用 `icon` 或 `label`、或在同一個標記上同時使用兩者，都會擲出例外，標記維持原狀。`id` 超出非負安全整數範圍、`atPrice` 缺少 `price` 或其值非有限值，或在其他位置設定 `price`，也會擲出例外。

### 讀取標記

`series.markers()` 傳回依時間排序、所有欄位都已填入的標記（使用預設值處 `color` 為 `null`）。排序是穩定的：時間相同的標記會保持你給定的順序。

標記在此清單中的索引，就是 `click` 事件回報的 `target.index`（參見[十字游標與事件](crosshair-events.md)）。由於清單已排序，此索引可能與你傳給 `setMarkers` 之陣列中的位置不同。

### 在標記上附加自己的資料

為每個標記指定自己的 `id`，並在同一物件上附加應用程式資料。JavaScript 的 `click` 和 `markerHover` 事件會以 `marker` 傳回原始物件，包括自訂欄位：

```ts
import type { Marker } from '@ferrochart/core';

const trades: (Marker & { trade: { wallet: string; quantity: number } })[] = [
  { id: 101, time: Date.UTC(2024, 0, 15), position: 'belowBar', text: 'Buy',
    trade: { wallet: 'wallet-a', quantity: 2 } },
];
candles.setMarkers(trades);

chart.on('click', ({ target }) => {
  if (target?.type !== 'marker' || target.series.id !== candles.id) return;
  const marker = target.marker as (typeof trades)[number];
  console.log(target.id, marker.trade.wallet, marker.trade.quantity);
});
chart.on('markerHover', (e) => {
  if (!e || e.series.id !== candles.id || e.overflow > 0) return;
  const marker = e.marker as (typeof trades)[number];
  console.log(e.id, marker.trade.wallet);
});
```

上面的型別斷言描述了這個系列標記的自訂欄位。`index` 是依時間排序後的位置，並非應用程式識別碼；建議用 `id` 或原始 `marker` 將事件關聯到自己的資料。對於「+k」徽章，`id` 和 `marker` 指向第一個隱藏標記，而非獨立的徽章物件。`markers()` 傳回正規化的標記欄位，而非這些原始物件。若要在透過 `setMarkers` 取代標記時保留自訂欄位，請自行保留原始清單。

### 放置與吸附

- 標記繪製在時間等於 `time` 的 K 棒上。落在兩根 K 棒之間的時間會吸附到**下一根** K 棒；晚於最後一根 K 棒的時間會吸附到最後一根。早於第一根 K 棒的時間會吸附到第一根。
- 若系列在該 K 棒上沒有值（缺口），則不繪製標記。
- 標記會跟隨系列的 `shift` 選項。
- 圖示寬 `size` CSS px（預設為 10）。上方或下方的標記與 K 棒相距 4 CSS px。
- 同一根 K 棒同一側的多個上方或下方標記會依 `markers()` 的順序朝遠離 K 棒的方向堆疊。帶有文字的標記占用一個圖示加一行文字的空間。
- 文字以 K 棒為中心，`aboveBar` 時位於圖示上方，`belowBar` 時位於圖示下方，並使用標記的顏色。
- `'atPrice'` 標記以其有限值 `price` 為中心，文字顯示在右側。它仍依時間吸附，但不參與堆疊或聚合；同一時間和價格的多個標記可能重疊。
- 標記會被裁切在繪圖區內。

```ts
// Two signals on the same bar: the first sits next to the low, the second below it.
candles.setMarkers([
  { time: t, position: 'belowBar', shape: 'arrowUp', text: 'Entry' },
  { time: t, position: 'belowBar', shape: 'circle', text: 'Alert' },
]);
```

對於發生在確切價格上的事件：

```ts
candles.setMarkers([
  { id: 102, time: t, position: 'atPrice', price: 101.25, shape: 'circle', text: 'Fill' },
]);
```

### 圖片、徽章與外環

先以某個名稱註冊一次圖片，之後就能在任意數量的標記中使用該名稱。來源可以是 URL（會被下載；來自其他來源的圖片需要 CORS 標頭）、`HTMLImageElement`、`ImageBitmap`、`ImageData` 或 canvas。圖表會保留一份縮放至 64 × 64 像素以內的副本：

```ts
await chart.registerIcon('whale', '/avatars/whale.png');
await chart.registerIcon('dev', document.querySelector('img#dev') as HTMLImageElement);

candles.setMarkers([
  { time: t1, icon: 'whale', size: 24, border: '#2fbf71' },             // a buy
  { time: t2, icon: 'whale', size: 24, border: '#ef5350' },             // a sell
  { time: t2, position: 'belowBar', label: 'DB', color: '#2962ff', size: 20 },
  { time: t3, position: 'belowBar', label: '狙', color: '#ff9800', size: 20 },
]);
```

- 圖片會填滿圓形（裁切成圓形）或方形，疊在標記顏色之上，因此透明圖片會露出其後的顏色。箭頭不能有圖片或標籤。
- `registerIcon` 傳回一個 promise；使用該名稱前請先等待它完成。以未註冊的名稱呼叫 `setMarkers` 會擲出例外。圖片無法載入或讀取時（下載失敗、來自其他來源且沒有 CORS 標頭的圖片），promise 會被拒絕：此時請讓該標記改用 `label` 或單純的圖形。
- 再次註冊同一名稱，會替換所有使用它之標記中的圖片。`chart.unregisterIcon(name)` 會將其移除（傳回原本是否存在）；使用它的標記之後只繪製圖形，且 `markers()` 回報其 `icon` 為 `null`。一個圖表最多保存 1024 張圖片。
- `label` 會置中繪製，顏色與標記顏色形成對比，並縮放以符合大小：拉丁字母約為直徑的 0.45，中文、日文或韓文字元約為 0.6，文字較寬時會更小。
- `border` 外環繪製在邊緣內側，因此不會改變標記的大小，例如買進用綠色外環、賣出用紅色外環。

### 收合擁擠的 K 棒

當許多標記落在同一根 K 棒上時（例如十幾位交易者在同一分鐘內買進），請在系列上設定 `markerStackLimit`。K 棒每一側最多使用這麼多個位置；最後一個位置顯示「+k」，代表 k 個未繪製的標記：

```ts
candles.applyOptions({ markerStackLimit: 4 });   // 3 markers, then "+k"
candles.applyOptions({ markerStackLimit: null }); // no limit (the default)
```

徽章是系列主色的圓形，大小與它所取代的標記相同。點擊它會傳送一個 `click`，其 target 為 `{ type: 'markerOverflow', series, first, count, id, marker, bars }`。`first` 是第一個隱藏標記在排序清單中的索引，`count` 是該側隱藏標記的數量，`id`/`marker` 識別第一個隱藏標記。`bars` 提供起迄邏輯 K 棒索引；單根 K 棒收合時兩者相同。排序清單中可能夾雜另一側的標記，因此 `count` 並不表示該清單中的一個連續切片。

### 跨 K 棒聚合

將 `markerCluster` 設為以 CSS 像素表示的正有限距離，即可聚合相鄰 K 棒上的標記。`null`（預設值）停用聚合：

```ts
candles.applyOptions({ markerCluster: 28, markerStackLimit: 4 });
// Later, keep per-bar folding but turn off cross-bar clustering:
candles.applyOptions({ markerCluster: null });
```

每個分組包含 `ceil(markerCluster / bar spacing)` 個邏輯索引（至少一個）。分組與邏輯索引對齊，因此平移不會重新分組；縮放可能改變分組大小。若一個分組的某側在兩根或更多 K 棒上有標記，該側僅顯示第一個標記和一個「+k」徽章，顯示位置為該側標記最多的 K 棒。`atPrice` 標記不參與聚合。

對於這樣的分組，聚合優先於 `markerStackLimit`。其他情況下，`markerStackLimit` 仍控制單根 K 棒上的收合。點擊徽章會得到 `markerOverflow` target，懸停事件則有 `overflow > 0`。兩種事件的 `bars: { from, to }` 包含徽章所代表的首尾邏輯索引，兩端均包含在內。結合這個範圍、`marker` 所在側和自己保留的標記資料來建立詳細清單；僅憑 `first`/`index` 和數量無法取得連續的成員清單。參見[邏輯索引](data.md#logical-indices)和[點擊事件](crosshair-events.md#click)。

### 懸停標記

當指標移到某個標記或「+k」徽章上時，會觸發 `markerHover` 事件；移開時則以 `null` 觸發。事件帶有標記的索引，以及以 CSS px 表示的中心與直徑，這正是提示框所需的資訊：

```ts
chart.on('markerHover', (e) => {
  if (!e) return hideTooltip();
  if (e.overflow > 0) {
    showTooltip(e.x, e.y - e.size / 2, `${e.overflow} more · bars ${e.bars.from}–${e.bars.to}`);
  } else {
    showTooltip(e.x, e.y - e.size / 2, e.marker.text ?? '');
  }
});
```

`index` 是在 `series.markers()` 中的位置；對徽章而言，它是第一個未繪製的標記，而 `overflow` 為 k（標記則為 0）。`id` 和 `marker` 識別該標記；`bars` 是該標記或徽章所代表的邏輯範圍，包含兩端。懸停中的標記是在繪製幀時判定的，因此此事件每幀最多觸發一次。開場動畫播放期間，不會有任何標記被視為懸停中。參見[十字游標與事件](crosshair-events.md)。

### 中文、日文、韓文與 emoji

標記文字與標籤可以使用任何文字。內建字型只涵蓋拉丁字母、數字與常用符號；其他字元的來源為：

- **Web**：瀏覽器本身的字型（系統的 CJK 字型與彩色 emoji），不需任何設定，也不會增加套件大小。
- **桌面、Python 與 C**：由你加入的字型，依序查找：winit 使用 `WindowOptions { fonts, .. }`，Python 使用 `fc.Chart(fonts=[...])`，C 使用 `ferrochart_add_font`。不會掃描系統字型。通常存在的字型：`/System/Library/Fonts/Hiragino Sans GB.ttc`（macOS）、`C:/Windows/Fonts/msyh.ttc`（Windows）、`/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`（Linux，需安裝 Noto CJK 套件）；或隨應用程式附帶 Noto Sans SC/JP/KR。

所有字型都沒有的字元會畫成 `□`。原生環境不支援彩色 emoji 字型：輪廓 emoji 字型（Noto Emoji）會以文字顏色繪製。文字依字素叢集排版：在網頁端，帶膚色的 emoji、家庭等 ZWJ 序列、旗幟以及帶組合附加符號的字母，都由瀏覽器當作一個字形測量與繪製。原生環境仍逐字繪製這些序列（字型不做塑形）。不支援字形塑形（shaping）與由右至左排版，因此阿拉伯文、希伯來文無法正確顯示。

### Python、C 與 Rust

各個綁定採用各自的命名慣例：

```python
red = chart.add_icon(avatar)          # a (height, width, 4) or (height, width, 3) uint8 array
candles.set_markers([
    {"time": t, "icon": red, "size": 24, "border": "#2fbf71"},
    {"time": t, "position": "below_bar", "label": "DB", "color": "#2962ff"},
])
candles.apply_options(marker_stack_limit=4, marker_cluster=28)
chart.on("marker_hover", lambda e: print(e))   # None when the pointer leaves
```

在 Python 中，標記字典接受 `id`，以及搭配 `price` 使用的 `position="at_price"`；事件會傳回 `id` 和徽章的 `bars`。透過 `marker` 傳回原始物件是 JavaScript 的功能；在原生 Python 中，請維護 id 到自有物件的對應表。Rust 使用 `Marker::id`、`Marker::price` 和 `MarkerPosition::AtPrice`；`SeriesOptions::marker_cluster` 控制聚合。C 使用 `FerroChartMarker.id`、`.price` 和 `FERROCHART_MARKER_AT_PRICE`；系列選項使用 `marker_cluster`。

在 Python 中，`add_icon` 傳回一個整數 id，`remove_icon(id)` 會移除該圖片。在 C 中，`ferrochart_add_icon` 與 `ferrochart_set_markers` 接受相同的欄位（參見 [C、C++ 與 C#](native.md)）。在 Rust 中，`Chart::add_icon` 接受一個 `IconImage`（非預乘 RGBA，最大 256 × 256），並傳回供 `Marker::icon` 使用的 `IconId`；`ChartEvent::MarkerHover` 與 `ClickTarget::MarkerOverflow` 分別是對應的事件與點擊 target。

### 哪些系列可以有標記

除了成交量系列之外的所有系列種類。對成交量系列呼叫 `setMarkers` 會擲出例外。標記也可以放在指標輸出上（`Indicator` 的 `outputs()` 就是一般的系列）：

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];
rsi.setMarkers([{ time: t, position: 'aboveBar', shape: 'square', text: 'OB' }]);
```

### 範例：由訊號產生標記

```ts
import type { Marker, OhlcData } from '@ferrochart/core';

// Mark crossings of the close over a 20-bar SMA.
function crossMarkers(bars: OhlcData, sma: number[]): Marker[] {
  const out: Marker[] = [];
  const close = bars.close;
  for (let i = 1; i < close.length; i++) {
    const was = close[i - 1] - sma[i - 1];
    const now = close[i] - sma[i];
    if (Number.isNaN(was) || Number.isNaN(now)) continue;
    if (was <= 0 && now > 0) {
      out.push({ time: bars.time[i], position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' });
    } else if (was >= 0 && now < 0) {
      out.push({ time: bars.time[i], position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' });
    }
  }
  return out;
}

candles.setMarkers(crossMarkers(bars, smaValues));
```

## 價格線

### 建立價格線

```ts
const entry = candles.createPriceLine({
  price: 101.25,
  color: '#2962ff',
  lineWidth: 1,
  lineStyle: 'solid',
  title: 'Entry',
});
const stop = candles.createPriceLine({ price: 98.5, color: '#ef5350', title: 'Stop' });
```

`createPriceLine` 傳回一個 `PriceLine`。價格以系列的單位表示，因此在百分比或指數化的價格軸上，價格線會隨系列移動（參見[價格軸](price-scale.md)）。

### 選項

| 選項 | 型別 | 預設值 | 意義 |
|---|---|---|---|
| `price` | `number` | `0` | 價格，以系列的單位表示；必須為有限值 |
| `color` | `Color \| null` | `null` | 線條、標題與座標軸標籤的顏色；`null` 使用系列的主色 |
| `lineWidth` | `number` | `1` | CSS px，大於 0 |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'dashed'` | 虛線樣式 |
| `axisLabelVisible` | `boolean` | `true` | 在價格軸上顯示價格標籤 |
| `title` | `string` | `''` | 在繪圖區右緣、線條上方的文字；空字串表示不顯示 |

座標軸標籤使用系列的價格格式（其 `pricePrecision`）。價格軸上的標籤若會重疊，會被錯開。位於可見價格範圍之外的價格線不會繪製。

### 變更與移除

```ts
entry.applyOptions({ price: 101.5 });         // only the keys you pass change
console.log(entry.options());                   // every option: { price, color, lineWidth, ... }
stop.applyOptions({ title: 'Stop (moved)', lineStyle: 'dotted' });

stop.remove();
```

`remove()` 之後，對該物件呼叫 `options`、`applyOptions` 或 `remove` 都會擲出例外。移除系列也會移除其價格線。

對成交量系列、非有限的價格、不是物件的選項參數，或無效的選項值，`createPriceLine` 都會擲出例外。

### 範例：可拖曳的委託單線

價格線本身不處理輸入。請將它與圖表的事件和座標轉換結合使用，或改用使用者可以拖曳的 `horizontal_line` 繪圖（參見[繪圖工具](drawings.md)）：

```ts
const id = chart.drawings.add({
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 1), price: 101.25 }],
  style: { line: '#2962ff' },
});

chart.on('drawingModified', (e: ChartEventMap['drawingModified']) => {
  if (e.id !== id) return;
  const price = chart.drawings.get(id)?.anchors[0].price;
  if (price !== undefined) console.log('order moved to', price);
});
```

## 最新值標籤與線

每個系列都會在價格軸上以標籤顯示其最新值，並在繪圖區畫一條橫跨的水平點線。當最後一根 K 棒收盤價高於或等於開盤價時（基準線系列：高於或等於基準值），兩者使用上漲顏色，否則使用下跌顏色；折線、面積、直方圖與點系列使用其主色。

| 系列選項 | 預設值 | 意義 |
|---|---|---|
| `lastValueVisible` | `true` | 在價格軸上顯示最新值標籤 |
| `priceLineVisible` | `true` | 位於最新值的點線 |
| `pricePrecision` | `null` | 標籤的小數位數；`null` 為自動 |

```ts
const line = chart.addSeries('line', { lastValueVisible: true, priceLineVisible: false });
candles.applyOptions({ priceLineVisible: false });
```

成交量系列兩者都不繪製。指標輸出以 `priceLineVisible: false` 建立；其最新值標籤會顯示。可針對個別輸出關閉：

```ts
for (const s of chart.addIndicator(candles, 'boll').outputs()) {
  s.applyOptions({ lastValueVisible: false });
}
```

系列選項請參見[系列](series.md)。
