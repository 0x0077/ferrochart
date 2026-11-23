<!-- source: 9914dfe76f -->
# 價格軸與窗格

圖表是一疊共用時間軸的窗格。每個窗格在右側都有自己的價格軸。本章說明如何建立窗格並設定其大小、將系列放到窗格上，以及價格軸的選項。

## 窗格

新建立的圖表有一個窗格，即主窗格。`chart.mainPane` 是最上方窗格的 id，也是所有接受選用窗格參數之方法的預設值。

```ts
const main = chart.mainPane;
const volumePane = chart.addPane(0.25); // below the others, a quarter of the main pane's height
const oscPane = chart.addPane(0.3);

console.log(chart.panes());             // [main, volumePane, oscPane], top to bottom
```

| 方法 | 作用 |
|---|---|
| `addPane(stretch?)` | 在其他窗格下方新增一個窗格並傳回其 id。`stretch` 是相對於其他窗格的高度（預設 1）；必須是大於 0 的有限數字，否則 `addPane` 會擲出例外。 |
| `removePane(pane)` | 移除窗格及其上的系列、繪圖與圖層外掛。最後一個窗格無法移除。 |
| `panes()` | 窗格 id，由上而下。 |
| `mainPane` | 最上方窗格的 id。 |
| `setPaneStretch(pane, stretch)` | 變更窗格的相對高度。`stretch` 必須是有限的正數。 |
| `paneStretch(pane)` | 窗格的相對高度，id 未知時為 `undefined`。 |

高度與 stretch 係數成正比：係數為 1、0.25 與 0.3 時，各窗格分別取得可用高度的 1/1.55、0.25/1.55 與 0.3/1.55。使用者可以拖曳兩個窗格之間的分隔線，這會改變那兩個窗格的係數。

窗格被移除後，其 id 不會再被重複使用。若移除最上方的窗格，下一個窗格會成為主窗格。除了 `paneStretch` 之外，傳入未知窗格 id 的方法都會擲出例外。

```ts
chart.setPaneStretch(oscPane, 0.5);
console.log(chart.paneStretch(oscPane)); // 0.5

chart.removePane(oscPane);               // its series go with it
```

窗格版面（窗格數量、其 stretch 係數與價格軸設定）是已儲存狀態的一部分；參見[儲存狀態](state.md)。

## 將系列放到窗格上

每個新增系列的方法都把窗格當作最後一個參數；省略時系列會放到主窗格。`series.pane` 會告訴你系列位於哪裡。

```ts
const candles = chart.addCandlestickSeries();          // main pane
const sma = chart.addSeries('line', { color: '#2962ff' });
const pane = chart.addPane(0.3);
const rsi = chart.addSeries('line', { color: '#7e57c2' }, pane);
rsi.setData(rsiValues);

console.log(rsi.pane === pane); // true
```

系列會固定在其窗格上；要移動它，請先移除再加到另一個窗格。指標透過放置參數選擇窗格；參見[指標](indicators.md)。

同一窗格的所有系列共用該窗格的價格軸，並一起自動縮放。

### 成交量的放置

`addVolumeSeries(source, pane?)` 的行為取決於窗格上還有什麼：

- **在有價格系列的窗格上**（預設：主窗格，與 K 線並列），成交量有自己獨立的刻度，占據窗格底部四分之一。它不會影響價格範圍，價格軸仍顯示價格。
- **在獨立的窗格上**，成交量使用該窗格的價格軸並填滿窗格：柱子立在底邊上（下邊距為 0），座標軸標籤使用 K、M 與 B 字尾（`1.25M`）。

```ts
// Volume under the candles, in the same pane.
chart.addVolumeSeries(candles);

// Or in a pane of its own below.
const volPane = chart.addPane(0.25);
chart.addVolumeSeries(candles, volPane);
```

## 價格軸

`applyPriceScaleOptions(options, pane?)` 會變更某個窗格（預設：主窗格）的價格軸；省略的鍵保留原值。它是全有或全無的：只要有一個值無效就會擲出例外，價格軸保留原有的所有選項。`priceScale(pane?)` 會傳回全部選項。

```ts
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
chart.applyPriceScaleOptions({ margins: { top: 0.2, bottom: 0.1 } }, pane);

const s = chart.priceScale();
// { mode: 'logarithmic', autoScale: true, inverted: false,
//   margins: { top: 0.1, bottom: 0.1 }, range: { low: 98.2, high: 112.7 } }
```

| 選項 | 型別 | 預設值 | 意義 |
|---|---|---|---|
| `mode` | `'normal' \| 'logarithmic' \| 'percentage' \| 'indexedTo100'` | `'normal'` | 價格如何對應到高度，參見[模式](#modes) |
| `autoScale` | `boolean` | `true` | 範圍是否跟隨可見資料 |
| `inverted` | `boolean` | `false` | 把較高的價格畫在較低處 |
| `margins` | `{ top, bottom }` | `{ top: 0.1, bottom: 0.1 }` | 資料上方與下方的空白，以窗格高度的比例表示 |
| `range` | `{ low, high }` | 跟隨資料 | 顯示的價格範圍（不含邊距）。設定它會關閉自動縮放。 |

`priceScale().range` 永遠是目前顯示的範圍，在自動縮放期間也一樣。

選項依 `mode`、`margins`、`inverted`、`range`、`autoScale` 的順序套用。無效的值會擲出例外；在此順序中排在它之前的選項已經套用。

### 模式

| 模式 | 座標軸顯示 |
|---|---|
| `'normal'` | 線性刻度的價格 |
| `'logarithmic'` | 對數刻度的價格：相同距離代表相同比率。適合價格變動很大的長期歷史資料。 |
| `'percentage'` | 相對於第一個可見值的變動百分比：(p − base) / \|base\| × 100，標籤顯示 2 位小數並帶 `%` |
| `'indexedTo100'` | 以第一個可見值為 100 的指數化價格：p / \|base\| × 100，標籤顯示 2 位小數 |

在 `'percentage'` 與 `'indexedTo100'` 模式下，窗格的每個系列都以各自的第一個可見值正規化：第一根可見 K 棒的收盤價，或數值系列的第一個可見值。如此一來，價位差異很大的系列也能在同一刻度上比較。使用者平移時，基準會隨第一根可見 K 棒移動。範圍、座標軸標籤與十字游標價格都以百分比或指數點表示。

在價格模式（`'normal'`、`'logarithmic'`）與相對模式（`'percentage'`、`'indexedTo100'`）之間切換會開啟自動縮放，因為舊的範圍使用的是不同的單位。

### 自動縮放與固定範圍

預設情況下，範圍會跟隨窗格上所有系列可見資料的最高與最低值（加上邊距），並在平移、縮放或收到新資料時更新。設定 `range` 可將其固定：

```ts
chart.applyPriceScaleOptions({ range: { low: 90, high: 110 } }); // autoScale is now false
chart.applyPriceScaleOptions({ autoScale: true });               // follow the data again
```

`low` 必須小於 `high`，且兩者皆為有限值；在對數模式下 `low` 必須大於 0。在相對模式下，範圍以百分比或指數點表示。

`range` 會關閉自動縮放，因此 `{ range, autoScale: true }` 自相矛盾而會擲出例外；`{ range, autoScale: false }` 則沒有問題。

使用者也可以改變範圍：拖曳價格軸，或在其上滾動滾輪（或在繪圖區內按 Alt + 滾輪），都會縮放價格並關閉自動縮放。雙擊價格軸會重新開啟它。參見[手勢與鍵盤](interaction.md#gestures)。

### 邊距

`margins` 讓資料與窗格的上下邊緣保持距離。兩個值都必須介於 0 到 1 之間（不含 1），且總和必須小於 1。兩個鍵都必須提供。

```ts
// Leave the top 30 % free for a legend.
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0.05 } });
```

### 反轉

`inverted: true` 會翻轉刻度：較高的價格畫在較低處。有時用於以殖利率對照債券價格，或用於反向貨幣對。

```ts
chart.applyPriceScaleOptions({ inverted: true });
```

## 範例：比較兩個商品

同一窗格上的兩個折線系列在百分比模式下，顯示自視圖左緣起的相對表現。

```ts
const a = chart.addSeries('line', { color: '#2962ff' });
const b = chart.addSeries('line', { color: '#ff6d00' });
a.setData({ time, value: closesA }); // around 180
b.setData({ time, value: closesB }); // around 0.95

chart.applyPriceScaleOptions({ mode: 'percentage' });
```

## 範例：三窗格版面

主窗格是 K 線加上移動平均線，下方是成交量，最底部是一個震盪指標。

```ts
const candles = chart.addCandlestickSeries();
candles.setData(bars);

const volPane = chart.addPane(0.2);
chart.addVolumeSeries(candles, volPane);

const oscPane = chart.addPane(0.25);
const osc = chart.addSeries('histogram', { color: '#26a69a' }, oscPane);
osc.setData({ time: bars.time, value: oscillator });

// Fixed range for the oscillator, a little room at the top of the main pane.
chart.applyPriceScaleOptions({ range: { low: -1, high: 1 } }, oscPane);
chart.applyPriceScaleOptions({ margins: { top: 0.15, bottom: 0.1 } });
```
