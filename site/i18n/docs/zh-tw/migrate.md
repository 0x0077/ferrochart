<!-- source: 5a2a4a05ee -->
# 從 lightweight-charts 遷移

為 lightweight-charts 撰寫的程式碼有兩種方式可以接上 FerroChart：

1. **相容入口** `@ferrochart/core/lightweight-charts`。它在 FerroChart 之上實作了 lightweight-charts v4 與 v5 API 中常用的部分，因此既有圖表往往只要改掉 import 就能執行。適合在既有頁面上試用 FerroChart，或分階段遷移。
2. **原生 API** `@ferrochart/core`。本指南其餘章節介紹的都是它，可以使用所有功能：適用於大量資料的欄式資料、指標、繪圖工具、同步、訂單流與繪圖（plots）。

## 相容入口

```ts
// Before:
// import { createChart, CandlestickSeries, LineSeries } from 'lightweight-charts';
import { createChart, CandlestickSeries, LineSeries, type UTCTimestamp } from '@ferrochart/core/lightweight-charts';

const container = document.getElementById('chart') as HTMLElement;
const chart = createChart(container, { layout: { background: { color: '#131722' } } });

const candles = chart.addSeries(CandlestickSeries, { upColor: '#26a69a', downColor: '#ef5350' });
candles.setData([
  { time: 1700000000 as UTCTimestamp, open: 10, high: 12, low: 9, close: 11 },
  { time: 1700000060 as UTCTimestamp, open: 11, high: 13, low: 10, close: 12 },
]);

const sma = chart.addSeries(LineSeries, { color: '#2962ff', lineWidth: 2 });
sma.setData([{ time: 1700000060 as UTCTimestamp, value: 11.5 }]);

chart.timeScale().fitContent();
```

### 支援範圍

| 領域 | 支援項目 |
|---|---|
| 建立 | `createChart(container, options)`，可傳入元素或其 id |
| 系列 | `addSeries(definition, options?, paneIndex?)`，搭配 `CandlestickSeries`、`BarSeries`、`LineSeries`、`AreaSeries`、`BaselineSeries`、`HistogramSeries`；v4 方法 `addCandlestickSeries`、`addBarSeries`、`addLineSeries`、`addAreaSeries`、`addBaselineSeries`、`addHistogramSeries`；`removeSeries` |
| 資料 | `setData`、`update`、`dataByIndex`；時間可為 `UTCTimestamp` 秒數、`'YYYY-MM-DD'` 字串或 `BusinessDay` 物件；直方圖資料的逐根 `color` |
| 系列 API | `applyOptions`、`options`、`seriesType`、`priceScale`、`createPriceLine`、`removePriceLine` |
| 標記 | v4 的 `series.setMarkers` / `series.markers`；v5 的 `createSeriesMarkers(series, markers)`，含 `setMarkers`、`markers`、`detach` |
| 時間軸 | `fitContent`、`scrollToRealTime`、`scrollToPosition`、`scrollPosition`、`getVisibleLogicalRange`、`setVisibleLogicalRange`、`getVisibleRange`、`setVisibleRange`、`logicalToCoordinate`、`coordinateToLogical`、`timeToCoordinate`、`coordinateToTime`、可見邏輯範圍與時間範圍的訂閱、`applyOptions` / `options`（`barSpacing`、`rightOffset`）、`width` |
| 價格軸 | `chart.priceScale('right', paneIndex)` 與 `series.priceScale()`：`applyOptions` / `options`，含 `scaleMargins`、`mode`、`invertScale`、`autoScale` |
| 十字游標 | `subscribeCrosshairMove` / `unsubscribeCrosshairMove`，提供 `time`、`logical`、`point`、`paneIndex` 與 `seriesData` |
| 圖表 | `applyOptions` / `options`（`width`、`height`、`layout.background.color`、`crosshair.mode`、`timeScale.barSpacing`、`timeScale.rightOffset`）、`resize`、`remove`、`timeScale`、`priceScale` |
| 常數 | `ColorType`、`CrosshairMode`、`LineStyle`、`LineType`、`PriceScaleMode`、`isUTCTimestamp`、`isBusinessDay` |

內附 TypeScript 型別宣告（`ISeriesApi`、`IChartApi`、`Time`、`UTCTimestamp`、`SeriesMarker` 等）。

### 非同步啟動

GPU 是以非同步方式初始化的，但 `createChart` 和 lightweight-charts 一樣會立即回傳。圖表就緒前發出的呼叫會被排入佇列，待就緒後依序執行。在需要的場合，有兩項額外成員可以幫忙：

| 成員 | 說明 |
|---|---|
| `chart.whenReady(): Promise<void>` | GPU 初始化完成且佇列中的呼叫都已執行後 resolve |
| `chart.ferrochart: FerroChart \| null` | 底層的原生圖表，就緒前為 `null` |

`getVisibleLogicalRange()`、`getVisibleRange()`、`coordinateToTime()`、`dataByIndex()` 等讀取方法在圖表就緒前會回傳 `null`（`priceScale().options()` 則回傳 `{}`）。讀取之前請先 await `whenReady()`：

```ts
import { createChart } from '@ferrochart/core/lightweight-charts';

declare const container: HTMLElement;

const chart = createChart(container);
// ... add series and data ...
await chart.whenReady();
console.log(chart.timeScale().getVisibleLogicalRange());
```

佇列中的呼叫若發生錯誤，會以 `console.error` 寫到主控台而不是拋出，因為引發錯誤的那次呼叫早已返回。圖表就緒後，錯誤則照常拋出。

### 差異

**渲染與尺寸**

- 圖表繪製在 `createChart` 附加到容器中的 `<canvas>` 上。若未指定 `width`/`height` 選項，canvas 會填滿容器（`100%` × `100%`）並跟隨其尺寸，因此請為容器設定高度。`autoSize` 不再需要，會被忽略。
- `width`、`height` 與 `resize(width, height)` 設定的是 canvas 的 CSS 尺寸。
- `remove()` 會銷毀圖表並移除其 canvas。
- 需要 WebGPU 或 WebGL2；請見[主題與樣式](styling.md#renderer)。

**主題與顏色**

- `layout.background.color` 決定主題：以 `#rrggbb`（或 `#rrggbbaa`）寫成的深色會套用深色主題，其他則套用淺色主題。其他 `layout` 選項（`textColor`、`fontSize`、`fontFamily`、漸層）會被忽略。`layout` 只在 GPU 初始化完成時讀取一次；之後再修改會記錄一則警告且沒有效果。
- `crosshair.mode`：`Normal`、`Magnet` 與 `Hidden` 可正常運作；`MagnetOHLC` 視同 `Magnet`。
- `grid`、`watermark`、`handleScroll`、`handleScale`、`localization`、`kineticScroll` 以及其他沒有對應功能的圖表選項會被接受但忽略。
- 顏色必須是 FerroChart 接受的格式：`#rgb`、`#rrggbb`、`#rrggbbaa`、`rgb()`、`rgba()` 或 `transparent`。`'red'` 這類具名顏色會造成錯誤。請見[顏色](styling.md#colors)。

**系列選項。** 只會使用下列選項；其餘選項會被接受但忽略：

| 系列 | 使用的選項 |
|---|---|
| Candlestick、Bar | `upColor`、`downColor` |
| Line | `color` |
| Area | `lineColor`、`topColor`、`bottomColor` |
| Baseline | `topLineColor`、`bottomLineColor`、`topFillColor1`、`bottomFillColor1`、`baseValue: { type: 'price', price }` |
| Histogram | `color`、`base` |
| 全部 | `lineWidth`、`lineStyle`、`lineType`、`lastValueVisible`、`priceLineVisible`、`priceFormat.precision` |

因此 `wickUpColor`、`borderUpColor` 等 K 線顏色，以及 `crosshairMarker*`、`priceFormat.minMove`、`priceFormat.type`、`title` 與 `visible` 都沒有效果。`LineStyle.LargeDashed` 畫成虛線，`LineStyle.SparseDotted` 畫成點線；`LineType.WithSteps` 畫成階梯線，`LineType.Curved` 則畫成直線段。

**價格軸**

- 每個窗格只有一個價格軸，位於右側。沒有左側或疊加價格軸：以 `'right'` 以外的 `priceScaleId` 建立的系列（包括成交量疊加常用的 `''`）會獲得一個獨立窗格，高度為預設窗格的 30%，並記錄一則警告。成交量的話，改用原生的 `addVolumeSeries`（見下文）可將其繪製在主窗格底部。
- 以 `'right'` 以外的 id 呼叫 `chart.priceScale(id)` 會記錄一則警告，並回傳右側價格軸。

**時間**

- 時間會轉換為 UTC 毫秒。`'YYYY-MM-DD'` 字串與 `BusinessDay` 物件視為 UTC 午夜；其他字串格式會拋出錯誤。
- 圖表第一次看到的時間（在 `setData` 或 `update` 中）決定它回傳時間的格式：事件、`getVisibleRange`、`coordinateToTime`、`dataByIndex` 與十字游標的 `time` 會相應地使用秒數、字串或 `BusinessDay` 物件。
- 邏輯索引在第一根 K 線為 0，與 lightweight-charts 相同。

**資料**

- 空白項目（只有 `{ time }`）在 K 線與美國線資料中會被捨棄，在折線、面積、基準線與直方圖資料中則成為缺口。
- 在此 API 中 OHLC 資料不含成交量。成交量系列請使用原生 API。

**標記**

- `position: 'inBar'` 會繪製在 K 線上方，未知的 `shape` 則畫成圓形。`size` 是預設大小的倍數（1 為 10 CSS px，限制在 4–64 px 之間）；`id` 會被忽略。

**尚未支援**

- `subscribeClick` 不做任何事並記錄一則警告。請改透過 `chart.ferrochart` 使用原生的 `click` 事件。
- `series.priceToCoordinate` 與 `series.coordinateToPrice` 回傳 `null` 並記錄一則警告。
- 自訂系列、系列 primitive 與窗格 primitive。FerroChart 自己的擴充機制請見[外掛](plugins.md)。
- `scrollToPosition(position, animated)` 會忽略 `animated`。
- `IPriceLine.options()` 回傳的是你傳入的選項，而非全部選項。

### 在相容入口中使用原生功能

`chart.ferrochart` 就是原生的 `FerroChart`，因此頁面可以保留既有的 lightweight-charts 呼叫，同時使用原生功能：

```ts
import { createChart, CandlestickSeries } from '@ferrochart/core/lightweight-charts';

declare const container: HTMLElement;

const chart = createChart(container);
chart.addSeries(CandlestickSeries);
await chart.whenReady();

const fc = chart.ferrochart;
if (fc) {
  fc.drawings.startTool('trend_line');
  fc.on('click', (e) => console.log('clicked at', e.time, e.price));
}
```

透過原生物件加入的系列不會被相容層得知（例如它們不會出現在十字游標事件的 `seriesData` 中）。

## 改用原生 API

原生 API 與 lightweight-charts 在幾個大方向上有所不同：

- **建立是非同步的。** `FerroChart.create(canvas, options)` 回傳一個 promise，且接受的是由 CSS 決定尺寸的 `<canvas>` 元素，而非容器。
- **時間是 UTC 毫秒**，可為 `number`、`bigint` 或（在列資料中）`Date`。請將 lightweight-charts 的秒數乘以 1000。
- **資料可以是欄式。** `setData` 接受列陣列，但對大量資料而言，以型別陣列組成的欄要快得多。
- **類型以字串表示。** `chart.addSeries('candlestick', options, pane)`；窗格以 id 表示，而非索引。
- **選項是扁平的。** 系列顏色為 `color`、`downColor`、`topFill` 與 `bottomFill`；主題為 `'light'` 或 `'dark'`。
- **事件使用 `on`/`off`。** 以 `chart.on('crosshairMove', ...)` 取代 `subscribeCrosshairMove(...)`。

### 對照

lightweight-charts：

```js
import { createChart, CandlestickSeries, HistogramSeries } from 'lightweight-charts';

const chart = createChart(document.getElementById('chart'), {
  layout: { background: { color: '#131722' }, textColor: '#d1d4dc' },
});
const candles = chart.addSeries(CandlestickSeries, { upColor: '#26a69a', downColor: '#ef5350' });
candles.setData(rows); // [{ time: 1700000000, open, high, low, close }]

const volume = chart.addSeries(HistogramSeries, { priceScaleId: '' });
volume.priceScale().applyOptions({ scaleMargins: { top: 0.8, bottom: 0 } });
volume.setData(rows.map((r) => ({ time: r.time, value: r.volume })));

chart.subscribeCrosshairMove((param) => {
  const bar = param.seriesData.get(candles);
  if (bar) legend.textContent = `C ${bar.close}`;
});
chart.timeScale().fitContent();
```

FerroChart：

```ts
import { FerroChart } from '@ferrochart/core';

declare const rows: { time: number; open: number; high: number; low: number; close: number; volume: number }[];
declare const legend: HTMLElement;

const canvas = document.getElementById('chart') as HTMLCanvasElement; // style="width: 100%; height: 480px"
const chart = await FerroChart.create(canvas, { theme: 'dark' });

const candles = chart.addCandlestickSeries({ color: '#26a69a', downColor: '#ef5350' });
candles.setData(rows.map((r) => ({ ...r, time: r.time * 1000 }))); // seconds to ms; volume included
chart.addVolumeSeries(candles); // reads the volume column, bottom of the main pane

chart.on('crosshairMove', ({ point }) => {
  if (point?.bar) legend.textContent = `C ${point.bar.close}`;
});
chart.fitContent();
```

### 常用呼叫對照表

| lightweight-charts | FerroChart |
|---|---|
| `createChart(container, options)` | `await FerroChart.create(canvas, { theme, renderer })` |
| `chart.addSeries(CandlestickSeries, o)` / `addCandlestickSeries(o)` | `chart.addCandlestickSeries(o)` 或 `chart.addSeries('candlestick', o)` |
| `BarSeries` / `addBarSeries` | `chart.addSeries('ohlc', o)` |
| `LineSeries`、`AreaSeries`、`BaselineSeries`、`HistogramSeries` | `chart.addSeries('line' \| 'area' \| 'baseline' \| 'histogram', o)` |
| `chart.addSeries(def, o, paneIndex)` | `chart.addSeries(kind, o, chart.panes()[paneIndex])`；以 `chart.addPane()` 新增窗格 |
| 使用 `priceScaleId: ''` 的成交量直方圖 | `chart.addVolumeSeries(candles)` |
| `chart.removeSeries(series)` | `series.remove()` |
| `series.setData(items)` | `series.setData(rows)`（時間以 ms 計）或 `series.setData({ time, open, high, low, close, volume })` 欄式資料 |
| `series.update(item)` | `series.update(bar)`（時間以 ms 計） |
| `series.applyOptions({ upColor, downColor })` | `series.applyOptions({ color, downColor })` |
| `series.applyOptions({ lineColor, topColor, bottomColor })` | `series.applyOptions({ color, topFill, bottomFill })` |
| `series.applyOptions({ priceFormat: { precision } })` | `series.applyOptions({ pricePrecision })` |
| `series.createPriceLine(o)` / `removePriceLine(line)` | `series.createPriceLine(o)` / `line.remove()` |
| `series.setMarkers(m)` / `createSeriesMarkers(series, m)` | `series.setMarkers(m)`（時間以 ms 計） |
| `series.dataByIndex(i)` | `series.dataAt(logicalIndex)` |
| `chart.timeScale().fitContent()` | `chart.fitContent()` |
| `chart.timeScale().scrollToRealTime()` | `chart.scrollToRealtime()` |
| `timeScale().scrollToPosition(n)` / `scrollPosition()` | `chart.setRightOffset(n)` / `chart.rightOffset` |
| `timeScale().getVisibleLogicalRange()` | `chart.visibleLogicalRange()` |
| `timeScale().setVisibleLogicalRange({ from, to })` | `chart.setVisibleLogicalRange(from, to)` |
| `timeScale().setVisibleRange({ from, to })` | `chart.applySync({ type: 'visibleRange', from, to })`（ms） |
| `timeScale().applyOptions({ barSpacing, rightOffset })` | `chart.setBarSpacing(px)`、`chart.setRightOffset(bars)` |
| `timeScale().logicalToCoordinate(i)` / `coordinateToLogical(x)` | `chart.logicalToCoordinate(i)` / `chart.coordinateToLogical(x)` |
| `timeScale().timeToCoordinate(t)` | `chart.logicalToCoordinate(chart.indexOfTime(ms))` |
| `timeScale().coordinateToTime(x)` | `chart.timeAtIndex(chart.coordinateToLogical(x))` |
| `timeScale().subscribeVisibleLogicalRangeChange(h)` | `chart.on('visibleRangeChanged', h)` |
| `chart.priceScale('right').applyOptions({ scaleMargins, mode, invertScale, autoScale })` | `chart.applyPriceScaleOptions({ margins, mode, inverted, autoScale }, pane)` |
| `PriceScaleMode.Logarithmic` | `mode: 'logarithmic'`（另有 `'normal'`、`'percentage'`、`'indexedTo100'`） |
| `chart.applyOptions({ crosshair: { mode: CrosshairMode.Magnet } })` | `chart.setCrosshairMode('magnet')` |
| `chart.subscribeCrosshairMove(h)` | `chart.on('crosshairMove', h)` |
| `chart.subscribeClick(h)` | `chart.on('click', h)` |
| `chart.applyOptions({ width, height })` / `resize(w, h)` | 設定 canvas 的 CSS 尺寸；圖表會跟著調整 |
| `chart.remove()` | `chart.destroy()` |

邏輯索引有一點不同：在 FerroChart 中，於前方插入較舊的資料時，K 線會保留原本的邏輯索引，因此第一根 K 線的索引可能為負數。`chart.dataRange()` 會回傳第一根與最後一根 K 線的索引。

各原生呼叫的細節請見[系列](series.md)、[資料](data.md)、[時間軸](time-scale.md)、[價格軸](price-scale.md)、[十字游標與事件](crosshair-events.md)與[標記](markers.md)。
