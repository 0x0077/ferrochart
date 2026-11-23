<!-- source: b0ef03b731 -->
# 標記與價格線

標記是附加在 K 棒上、可附帶文字的小圖示，例如買賣訊號或新聞。價格線是位於固定價格的水平線，例如委託單、停損或目標價。兩者都隸屬於某個系列。每個系列還有一個最新值標籤與一條最新值線，可透過系列選項開啟或關閉。

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
| `position` | `'aboveBar' \| 'belowBar'` | `'aboveBar'` | 位於 K 棒最高價之上或最低價之下。對單值系列而言，則位於數值之上或之下。 |
| `shape` | `'circle' \| 'square' \| 'arrowUp' \| 'arrowDown'` | `'circle'` | 實心圖示 |
| `color` | `Color \| null` | `null` | 圖示與文字顏色；`null` 使用系列的主色（K 線與棒使用上漲顏色） |
| `text` | `string` | `''` | 圖示旁、遠離 K 棒那一側的文字；空字串表示不顯示 |

未知的 `position` 或 `shape`、缺少 `time` 或其非整數，或無效的顏色都會擲出例外，標記維持原狀。

### 讀取標記

`series.markers()` 傳回依時間排序、所有欄位都已填入的標記（使用預設值處 `color` 為 `null`）。排序是穩定的：時間相同的標記會保持你給定的順序。

標記在此清單中的索引，就是 `click` 事件回報的 `target.index`（參見[十字游標與事件](crosshair-events.md)）。由於清單已排序，此索引可能與你傳給 `setMarkers` 之陣列中的位置不同。

### 放置與吸附

- 標記繪製在時間等於 `time` 的 K 棒上。落在兩根 K 棒之間的時間會吸附到**下一根** K 棒；晚於最後一根 K 棒的時間會吸附到最後一根。早於第一根 K 棒的時間會吸附到第一根。
- 若系列在該 K 棒上沒有值（缺口），則不繪製標記。
- 標記會跟隨系列的 `shift` 選項。
- 圖示寬 10 CSS px，與 K 棒相距 4 CSS px。
- 同一根 K 棒同一側的多個標記會依 `markers()` 的順序朝遠離 K 棒的方向堆疊。帶有文字的標記占用一個圖示加一行文字的空間。
- 文字以 K 棒為中心，`aboveBar` 時位於圖示上方，`belowBar` 時位於圖示下方，並使用標記的顏色。
- 標記會被裁切在繪圖區內。

```ts
// Two signals on the same bar: the first sits next to the low, the second below it.
candles.setMarkers([
  { time: t, position: 'belowBar', shape: 'arrowUp', text: 'Entry' },
  { time: t, position: 'belowBar', shape: 'circle', text: 'Alert' },
]);
```

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
