<!-- source: c2e84c153f -->
# 系列類型

系列是以一種方式繪製的一組資料：K 線、折線、直方圖等。一張圖表可以在任意數量的窗格上容納任意數量的系列。同一張圖表的所有系列共用一條時間軸（請見[資料](data.md)）。

## 加入系列

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas);

const candles = chart.addSeries('candlestick');           // main pane
const sma = chart.addSeries('line', { color: '#2962ff' }); // with options
const pane = chart.addPane(0.3);
const osc = chart.addSeries('histogram', {}, pane);        // on another pane
```

`addSeries(kind, options?, pane?)` 回傳一個 `Series` handle。省略 `pane` 時，系列會放在主（最上方）窗格。`addCandlestickSeries(options?,
pane?)` 是 `addSeries('candlestick', ...)` 的簡寫。未知的類型或窗格 id 會拋出錯誤。

`Series` 有三個唯讀屬性：

| 屬性 | 意義 |
|---|---|
| `id` | 數字，在圖表內唯一。指向同一系列的 handle（例如來自 `click` 事件的 handle）具有相同的 id。 |
| `kind` | 傳給 `addSeries` 的類型；成交量系列為 `'volume'`，對應的變換為 `'kagi'` / `'pointFigure'`（Renko 與 range bars 回報 `'candlestick'` 或 `'ohlc'`），外掛系列為 `'custom'`。 |
| `pane` | 系列所在窗格的 id。 |

## 類型

| 類型 | 資料 | 繪製方式 |
|---|---|---|
| `'candlestick'` | OHLC | K 線：實體從開盤到收盤，影線從最低到最高 |
| `'ohlc'` | OHLC | 美國線：左側為開盤刻線，右側為收盤刻線 |
| `'heikinAshi'` | OHLC | 由你傳入的 K 線計算出的平均 K 線（Heikin-Ashi） |
| `'line'` | value | 連接各數值的折線 |
| `'area'` | value | 一條折線，下方以漸層填色直到窗格底部 |
| `'baseline'` | value | 一條折線，並在 `baseValue` 上下以不同顏色填色 |
| `'histogram'` | value | 從 `baseValue` 延伸到各數值的柱狀，可逐點著色 |
| `'points'` | value | 在每個數值處畫一個點 |

成交量直方圖以 `addVolumeSeries` 加入（請見[成交量](#volume)），Renko、range bars、Kagi 與點數圖（Point & Figure）則以 `addTransformSeries` 加入（請見 [K 線變換](#bar-transforms)）。

OHLC 系列接受 `{ time, open, high, low, close, volume? }`；數值系列接受 `{ time, value, color? }`。[資料](data.md)一章說明這兩種格式、列陣列、驗證與即時更新。

### Candlestick

```ts
const candles = chart.addCandlestickSeries({
  color: '#26a69a',      // up candles (close >= open)
  downColor: '#ef5350',  // down candles
});
candles.setData(randomWalk(1, 500, Date.UTC(2024, 0, 1), 60_000));
```

若 `color` 與 `downColor` 都未設定，影線使用主題的影線顏色。只要設定了其中之一，影線就會與實體同色。

### 美國線（OHLC bars）

`'ohlc'` 接受與 `'candlestick'` 相同的資料與選項，繪製傳統的美國線。

### Heikin-Ashi

`'heikinAshi'` 接受一般的 OHLC K 線；圖表會由此計算 Heikin-Ashi 數值，即時更新時也一樣。顏色的用法與 K 線相同。

### 折線

```ts
const line = chart.addSeries('line', { color: '#2962ff', lineWidth: 1, lineStyle: 'dashed' });
line.setData({ time: [0, 60_000, 120_000, 180_000], value: [10, 11, NaN, 12] });
```

`NaN` 值代表缺口：折線不會穿過它。

### 面積圖

面積圖是下方帶有垂直漸層的折線。`topFill` 是折線處的顏色，`bottomFill` 是窗格底部的顏色。

```ts
chart.addSeries('area', {
  color: '#2962ff',
  topFill: 'rgba(41, 98, 255, 0.4)',
  bottomFill: 'rgba(41, 98, 255, 0)',
});
```

預設兩種填色都是 `color`，頂部不透明度為 40 %，底部為 2 %。

### 基準線

基準線系列以 `baseValue` 為界分開：在其上方，折線使用 `color`、填色使用 `topFill`；在其下方，折線使用 `downColor`、填色使用 `bottomFill`。

```ts
chart.addSeries('baseline', {
  baseValue: 100,
  color: '#26a69a',
  downColor: '#ef5350',
});
```

預設的填色是兩種折線顏色，不透明度為 20 %。

### 直方圖

柱狀從 `baseValue`（預設 0）延伸到各數值。自動縮放的範圍一定會包含 `baseValue`。

每個點可以有自己的顏色：在 `value` 旁傳入一個 `color` 欄。其元素為 CSS 顏色，`null` 代表使用系列顏色；或者是由 `0xRRGGBBAA` 值組成的 `Uint32Array`，其中 `0` 代表系列顏色。

```ts
const hist = chart.addSeries('histogram', { color: '#26a69a' }, pane);
hist.setData({
  time: [0, 60_000, 120_000],
  value: [1.5, -0.7, 0.4],
  color: [null, '#ef5350', null],
});

// The same colors as packed integers.
hist.setData({
  time: [0, 60_000, 120_000],
  value: [1.5, -0.7, 0.4],
  color: new Uint32Array([0, 0xef5350ff, 0]),
});
```

逐點顏色只會在完整細節下繪製。當縮小到好幾根 K 線共用一個像素而被合併時，會使用系列顏色。其他類型也接受 `color` 欄，但會忽略它。`update` 無法設定顏色；附加的點使用系列顏色。

### 點

`'points'` 在每個數值處畫一個點；`lineWidth` 是點的直徑，以 CSS px 計。缺口（`NaN`）不畫點。

```ts
const dots = chart.addSeries('points', { color: '#ff9800', lineWidth: 4 });
```

### 成交量

成交量系列繪製 OHLC 系列的 `volume` 欄。它沒有自己的資料：對來源系列呼叫 `setData` 與 `update` 時會一併更新它。

```ts
const candles = chart.addCandlestickSeries();
const volume = chart.addVolumeSeries(candles);          // main pane
// or: chart.addVolumeSeries(candles, chart.addPane(0.25));
```

來源必須是 OHLC 系列（K 線、美國線、Heikin-Ashi）或變換系列。柱狀依來源 K 線是漲或跌，以主題的成交量顏色著色。在同時顯示價格的窗格上，成交量在窗格底部四分之一處有自己的座標軸；在獨立窗格上則填滿整個窗格。請見[價格軸與窗格](price-scale.md#volume-placement)。

移除來源系列時，也會移除其成交量系列。

## 系列選項

選項在 `addSeries` 時傳入，之後可用 `applyOptions` 修改；省略的鍵會保留原值。`options()` 回傳所有鍵。

```ts
const s = chart.addSeries('line', { color: '#2962ff' });
s.applyOptions({ lineWidth: 3 });       // color stays '#2962ff'
console.log(s.options().lineWidth);     // 3
s.applyOptions({ color: null });        // back to the theme color
```

| 選項 | 型別 | 預設值 | 適用於 | 意義 |
|---|---|---|---|---|
| `color` | `Color \| null` | 主題 | 全部 | 折線顏色（line、area、points）；上漲 K 線與美國線；直方圖柱狀；基準以上的基準線；Kagi 的陽線與點數圖的 X 欄 |
| `downColor` | `Color \| null` | 主題 | candlestick、ohlc、heikinAshi、baseline、變換 | 下跌 K 線與美國線；基準以下的基準線；Kagi 的陰線與點數圖的 O 欄 |
| `topFill` | `Color \| null` | 見[面積圖](#area)、[基準線](#baseline) | area、baseline | 面積圖在折線處的填色；基準線在基準以上的填色 |
| `bottomFill` | `Color \| null` | 見[面積圖](#area)、[基準線](#baseline) | area、baseline | 面積圖在底部的填色；基準線在基準以下的填色 |
| `lineWidth` | `number` | `2` | line、area、baseline、points、Kagi、點數圖 | 線寬，以 CSS px 計；對 points 而言是點的直徑。必須為正數。 |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | line、area、baseline | 線條的虛線樣式 |
| `step` | `boolean` | `false` | line、area、baseline | 階梯線，請見[階梯線](#step-lines) |
| `baseValue` | `number` | `0` | baseline、histogram | 基準線填色的分界；直方圖柱狀的起點。必須為有限值。 |
| `lastValueVisible` | `boolean` | `true` | 成交量以外全部 | 在價格軸上顯示最新值的標籤 |
| `priceLineVisible` | `boolean` | `true` | 成交量以外全部 | 在最新值處畫一條水平點線 |
| `shift` | `number` | `0` | 全部 | 以 K 線數為單位的顯示位移，請見[位移](#shift) |
| `pricePrecision` | `number \| null` | `null` | 全部 | 價格標籤的小數位數，請見[價格精度](#price-precision) |

無效的值（未知的線條樣式、格式錯誤的顏色、負的寬度）會拋出錯誤，系列保留原本的選項。

成交量系列一律使用主題的成交量顏色。

### 顏色

顏色是 CSS 字串：`#rgb`、`#rgba`、`#rrggbb`、`#rrggbbaa`、`rgb(r, g, b)`、`rgba(r, g, b, a)`（alpha 介於 0 到 1）或 `'transparent'`。不接受 `'red'` 這類顏色名稱。`null` 代表主題預設值。`options()` 以 `#rrggbb` 回傳顏色，半透明時回傳 `rgba(...)`，主題顏色則回傳 `null`。

### 階梯線

設定 `step: true` 時，折線會先水平延伸到下一個點的時間，再垂直移到該點的數值。適用於在變動前維持不變的數值，例如利率或部位大小。

```ts
const rate = chart.addSeries('line', { step: true, color: '#7e57c2' }, pane);
rate.setData({
  time: [Date.UTC(2024, 0, 1), Date.UTC(2024, 2, 20), Date.UTC(2024, 6, 31)],
  value: [5.5, 5.25, 5.0],
});
```

### 位移

`shift` 會把邏輯索引 `j` 的點畫在 `j + shift` 處：正值將系列往右（未來）移，負值往左移。它必須是整數。自動縮放、十字游標與 `dataAt` 都依照繪製後的位置。一目均衡表（Ichimoku）的先行帶與遲行線就使用了它。

```ts
// A moving average drawn 10 bars ahead (a "displaced" moving average).
const dma = chart.addSeries('line', { shift: 10 });
```

### 價格精度

`pricePrecision` 設定價格軸標籤、最新值標籤與十字游標標籤的小數位數，範圍為 0 到 12。`null`（預設）為自動：至少 2 位小數，且可見價格至少有 4 位有效數字。非常小的價格會以下標表示零的個數：`0.0₄1234` 即 0.00001234。

窗格的標籤使用其第一個價格系列的精度。在 `'percentage'` 與 `'indexedTo100'` 價格軸模式下，座標軸一律使用 2 位小數。0 到 12 的整數與 `null` 以外的任何值都會拋出錯誤。

```ts
candles.applyOptions({ pricePrecision: 5 }); // FX quotes
candles.applyOptions({ pricePrecision: null }); // automatic again
```

## K 線變換

變換系列接受一般的 K 線（或逐筆成交），並繪製某種製圖方法產生的結果：Renko 磚塊、range bars、Kagi 線或點數圖欄位。這些方法不考慮時間；每個元素在時間軸上各占一格。

```ts
const renko = chart.addTransformSeries('renko', { boxSize: 0.5 });
renko.setData(randomWalk(1, 5_000, Date.UTC(2024, 0, 1), 60_000));
chart.addVolumeSeries(renko); // optional
chart.fitContent();
```

`addTransformSeries(name, params, options?, pane?)` 回傳一個 `Series`。以 `setData` 與 `update` 餵入資料，用法與 K 線系列完全相同。逐筆成交就是四個價格相等的 K 線：

```ts
renko.update({ time: tick.time, open: tick.price, high: tick.price, low: tick.price, close: tick.price, volume: tick.size });
```

### 參數

價格以資料本身的單位表示。

| 變換 | 參數 | 繪製為 |
|---|---|---|
| `'renko'` | `boxSize`（價格）**或** `atr`（ATR 週期，整數）；`kind?` | K 線（預設）或美國線 |
| `'rangeBars'` | `range`（價格）；`kind?` | K 線（預設）或美國線 |
| `'kagi'` | `reversal`（價格）**或** `reversalPercent` | Kagi 線 |
| `'pointFigure'` | `boxSize`（價格）；`reversal?`（格數，整數，預設 3） | X 與 O 欄 |

`kind` 為 `'candlestick'`（預設）或 `'ohlc'`。數字必須為有限的正數。兩個替代參數同時給（`boxSize` 與 `atr`，或 `reversal` 與 `reversalPercent`）、都不給，或使用未知名稱，都會拋出錯誤。

```ts
chart.addTransformSeries('renko', { atr: 14, kind: 'ohlc' });
chart.addTransformSeries('rangeBars', { range: 1 });
chart.addTransformSeries('kagi', { reversalPercent: 2 });
chart.addTransformSeries('pointFigure', { boxSize: 1, reversal: 3 });
```

（上面每一行都需要各自的圖表，請見[限制](#constraints)。）

### Renko

Renko 只使用收盤價。每當收盤價超出上一塊磚整整一個 `boxSize` 時就新增一塊磚：同方向延續需要一格，反轉則需要兩格。第一塊磚以第一個收盤價為錨點。

使用 `atr: n` 時，格子大小是前 `n` 根 K 線的平均真實區間（ATR），此後固定不變；在此之前不繪製任何東西。

Renko 沒有形成中的磚塊：磚塊只在完成後才會出現。

### Range bars

每根 K 線從最低到最高恰好跨越 `range`。在一根輸入 K 線內，上漲 K 線假設價格依開盤 → 最低 → 最高 → 收盤移動，下跌 K 線則依開盤 → 最高 → 最低 → 收盤移動。當價格離開目前 K 線的範圍時，該 K 線即告完成，下一根從其邊界開盤。最後一根 K 線處於形成中，會隨更新而改變。

### Kagi

Kagi 線沿著收盤價的方向延伸，當收盤價反轉至少 `reversal`（或線段末端價格的 `reversalPercent` 百分比）時轉折。線段在升破前一個肩部時變粗（陽線，以 `color` 繪製），跌破前一個腰部時變細（陰線，`downColor`）。最後一條線處於形成中。

### 點數圖（Point & Figure）

價格被分成大小為 `boxSize` 的格子。當收盤價向上填滿新格子時，X 欄持續上升；反轉 `reversal` 格則開始一個 O 欄，反之亦然。X 欄使用 `color`，O 欄使用 `downColor`。最後一欄處於形成中。

### 限制

- 一張圖表最多只能有一個變換系列，而且旁邊除了成交量之外不能有其他系列。加入第二個變換或任何其他系列都會拋出錯誤，在已有系列的圖表中加入變換也一樣。若要顯示原始 K 線，請使用另一張圖表，必要時以[同步](sync.md)讓兩者保持一致。
- 每個元素的時間是使其完成的那根輸入 K 線的時間（若該時間不會遞增，則為前一個元素的時間再加一毫秒）。`timeAtIndex` 與時間軸顯示的就是這些時間。
- 每根 K 線的輸入成交量會加到其後形成中的元素上。沒有輸入成交量時，成交量為空。
- 單一輸入 K 線最多只能產生 65536 個元素。如果格子或範圍相對於價格波動太小，使得一根 K 線超過此數，呼叫會拋出錯誤，並清空該系列。
- 屬於缺口的 K 線（所有價格皆為 `NaN`）會被略過。

## 兩個系列之間的帶狀區域

`addBand(upper, lower, options?)` 填滿同一窗格中兩個單值系列（line、area、baseline、histogram 或 points）之間的區域。`upper` 位於 `lower` 之上或相等時使用 `fill`，位於其下時使用 `fillCrossed`。帶狀區域繪製在窗格中各系列的後方（成交量的上方）。

```ts
const upper = chart.addSeries('line', { color: '#26a69a' });
const lower = chart.addSeries('line', { color: '#ef5350' });
upper.setData({ time, value: highs });
lower.setData({ time, value: lows });

const band = chart.addBand(upper, lower, {
  fill: 'rgba(38, 166, 154, 0.15)',
  fillCrossed: 'rgba(239, 83, 80, 0.15)',
});

chart.setBandOptions(band, { fillCrossed: null }); // keys left out keep their value
console.log(chart.bandOptions(band));               // { fill: 'rgba(...)', fillCrossed: null }
chart.removeBand(band);
```

`addBand` 回傳帶狀區域的 id。`fill` 為 `null`（或省略）時，使用上方系列的顏色、不透明度 15 %；`fillCrossed` 為 `null` 時與 `fill` 相同。對未知的 id，`bandOptions` 回傳 `null`。任一系列被移除時，帶狀區域會自動移除。兩個 OHLC 系列、位於不同窗格的系列，或同一系列傳入兩次，都會拋出錯誤。

帶有帶狀區域的指標（布林通道、肯特納通道、一目均衡表）會自動為你建立；請見[指標](indicators.md)。

## 移除系列

```ts
line.remove();
```

`remove()` 會刪除系列、其資料、標記與價格線，以及使用它的帶狀區域。移除 OHLC 系列也會移除其成交量系列。之後對該 handle 呼叫任何方法都會拋出錯誤（`'the series was removed'`）。移除最後一個含資料的系列會重設時間軸。

指標的輸出無法逐一移除：對其中之一呼叫 `remove()` 會拋出錯誤；請改為移除指標本身（`indicator.remove()`）。

若要移除整個窗格及其所有系列，請使用 `chart.removePane(id)`（請見[價格軸與窗格](price-scale.md)）。
