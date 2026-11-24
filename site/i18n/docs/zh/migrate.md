<!-- source: 2b4219641f -->
# 从 lightweight-charts 迁移

为 lightweight-charts 编写的代码有两种方式接入 FerroChart：

1. **兼容入口** `@ferrochart/core/lightweight-charts`。它在 FerroChart 之上实现了
   lightweight-charts v4 和 v5 API 中常用的部分，因此现有图表往往只需改一下
   import 就能运行。可以用它在现有页面上试用 FerroChart，或者分步迁移。
2. **原生 API** `@ferrochart/core`。本指南其余部分介绍的就是它，它提供全部功能：
   面向大数据集的列式数据、指标、画线、联动、订单流和绘图。

## 兼容入口

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

### 已支持

| 范围 | 已支持 |
|---|---|
| 创建 | `createChart(container, options)`，传入元素或其 id |
| 系列 | `addSeries(definition, options?, paneIndex?)`，支持 `CandlestickSeries`、`BarSeries`、`LineSeries`、`AreaSeries`、`BaselineSeries`、`HistogramSeries`；v4 方法 `addCandlestickSeries`、`addBarSeries`、`addLineSeries`、`addAreaSeries`、`addBaselineSeries`、`addHistogramSeries`；`removeSeries` |
| 数据 | `setData`、`update`、`dataByIndex`；时间可以是 `UTCTimestamp` 秒数、`'YYYY-MM-DD'` 字符串或 `BusinessDay` 对象；柱状图数据的逐根 `color` |
| 系列 API | `applyOptions`、`options`、`seriesType`、`priceScale`、`createPriceLine`、`removePriceLine` |
| 标记 | v4 的 `series.setMarkers` / `series.markers`；v5 的 `createSeriesMarkers(series, markers)`，含 `setMarkers`、`markers`、`detach` |
| 时间轴 | `fitContent`、`scrollToRealTime`、`scrollToPosition`、`scrollPosition`、`getVisibleLogicalRange`、`setVisibleLogicalRange`、`getVisibleRange`、`setVisibleRange`、`logicalToCoordinate`、`coordinateToLogical`、`timeToCoordinate`、`coordinateToTime`，可见逻辑范围和时间范围的订阅，`applyOptions` / `options`（`barSpacing`、`rightOffset`），`width` |
| 价格轴 | `chart.priceScale('right', paneIndex)` 和 `series.priceScale()`：`applyOptions` / `options`，支持 `scaleMargins`、`mode`、`invertScale`、`autoScale` |
| 十字光标 | `subscribeCrosshairMove` / `unsubscribeCrosshairMove`，提供 `time`、`logical`、`point`、`paneIndex` 和 `seriesData` |
| 图表 | `applyOptions` / `options`（`width`、`height`、`layout`、`grid`、`crosshair`、`rightPriceScale.visible`、`handleScroll`、`handleScale`、`timeScale.barSpacing`、`timeScale.rightOffset`、`timeScale.visible`），`resize`、`remove`、`timeScale`、`priceScale` |
| 常量 | `ColorType`、`CrosshairMode`、`LineStyle`、`LineType`、`PriceScaleMode`、`isUTCTimestamp`、`isBusinessDay` |

附带 TypeScript 类型声明（`ISeriesApi`、`IChartApi`、`Time`、
`UTCTimestamp`、`SeriesMarker` 等）。

### 异步启动

GPU 是异步初始化的，但 `createChart` 会像 lightweight-charts 一样立即返回。在图表就绪前发起的调用会被放入队列，待就绪后按顺序执行。在需要的场合，有两个新增成员可以帮忙：

| 成员 | 说明 |
|---|---|
| `chart.whenReady(): Promise<void>` | GPU 初始化完成且队列中的调用执行完毕后 resolve |
| `chart.ferrochart: FerroChart \| null` | 底层的原生图表，就绪前为 `null` |

`getVisibleLogicalRange()`、`getVisibleRange()`、`coordinateToTime()` 或
`dataByIndex()` 等读取方法在图表就绪前返回 `null`（`priceScale().options()`
返回 `{}`）。读取之前请先 await `whenReady()`：

```ts
import { createChart } from '@ferrochart/core/lightweight-charts';

declare const container: HTMLElement;

const chart = createChart(container);
// ... add series and data ...
await chart.whenReady();
console.log(chart.timeScale().getVisibleLogicalRange());
```

队列中的调用若出错，会用 `console.error` 输出到控制台，而不是抛出异常，因为引发错误的那次调用早已返回。图表就绪后，错误照常抛出。

### 差异

**渲染与尺寸**

- 图表绘制在 `createChart` 追加到容器中的 `<canvas>` 上。不设置 `width`/`height`
  选项时，canvas 会填满容器（`100%` × `100%`）并跟随容器尺寸变化，所以请给容器设定高度。
  `autoSize` 不再需要，会被忽略。
- `width`、`height` 和 `resize(width, height)` 设置的是 canvas 的 CSS 尺寸。
- `remove()` 会销毁图表并移除其 canvas。
- 需要 WebGPU 或 WebGL2；参见[主题与样式](styling.md#renderer)。

**主题与颜色**

- `layout.background.color` 设置背景并决定基础主题：以 `#rrggbb`（或 `#rrggbbaa`）写出的深色得到深色主题，其他任何值得到浅色主题。`layout.textColor` 和 `layout.fontSize` 设置文字；渐变背景按其 `topColor` 绘制；`fontFamily` 被忽略。这些都可以用 `applyOptions` 修改。
- `grid.vertLines` / `grid.horzLines`：`visible` 分别隐藏竖线和横线；网格只有一种颜色（两者都给出时取横线的颜色）。
- `rightPriceScale.visible`（或 `priceScale('right').applyOptions({ visible })`）和 `timeScale.visible` 隐藏坐标轴；价格轴在所有窗格中一起隐藏。
- `handleScroll` 和 `handleScale`：`false`，或所有子选项都为 `false` 的对象，关闭平移或缩放；任一子选项为 `true` 则保持开启（FerroChart 各只有一个开关）。
- `crosshair.mode`：`Normal`、`Magnet` 和 `Hidden` 可用；`MagnetOHLC` 按 `Magnet` 处理。`vertLine.visible` 和 `horzLine.visible` 分别隐藏两条线；除非两条线的 `labelVisible` 都为 `false`，否则显示标签。
- `watermark`、`localization`、`kineticScroll` 以及其他没有对应功能的图表选项会被接受并忽略。
- 颜色必须采用 FerroChart 接受的格式：`#rgb`、`#rrggbb`、`#rrggbbaa`、
  `rgb()`、`rgba()` 或 `transparent`。`'red'` 这样的颜色名称会报错。参见[颜色](styling.md#colors)。

**系列选项。** 只有下列选项会生效；其他选项会被接受并忽略：

| 系列 | 生效的选项 |
|---|---|
| Candlestick、Bar | `upColor`、`downColor` |
| Line | `color` |
| Area | `lineColor`、`topColor`、`bottomColor` |
| Baseline | `topLineColor`、`bottomLineColor`、`topFillColor1`、`bottomFillColor1`、`baseValue: { type: 'price', price }` |
| Histogram | `color`、`base` |
| 全部 | `lineWidth`、`lineStyle`、`lineType`、`lastValueVisible`、`priceLineVisible`、`priceFormat.precision` |

因此 `wickUpColor`、`borderUpColor` 等 K 线颜色，以及 `crosshairMarker*`、
`priceFormat.minMove`、`priceFormat.type`、`title` 和 `visible` 都不起作用。`LineStyle.LargeDashed` 画成虚线，`LineStyle.SparseDotted` 画成点线；
`LineType.WithSteps` 画成阶梯线，`LineType.Curved` 画成直线段。

**价格轴**

- 每个窗格只有一个价格轴，位于右侧。没有左侧或叠加价格轴：以 `'right'` 以外的
  `priceScaleId` 创建的系列（包括常用于叠加成交量的 `''`）会获得一个独立窗格，
  高度为默认窗格的 30%，并输出一条警告。对于成交量，改用原生的 `addVolumeSeries`（见下文）可以把它画在主窗格底部。
- 以 `'right'` 以外的 id 调用 `chart.priceScale(id)` 会输出警告并返回右侧价格轴。

**时间**

- 时间会被转换为 UTC 毫秒。`'YYYY-MM-DD'` 字符串和 `BusinessDay` 对象表示 UTC 零点；其他字符串格式会抛出异常。
- 图表第一次看到的时间（在 `setData` 或 `update` 中）决定了它返回时间的格式：事件、`getVisibleRange`、`coordinateToTime`、
  `dataByIndex` 和十字光标的 `time` 会相应地使用秒数、字符串或 `BusinessDay` 对象。
- 与 lightweight-charts 一样，第一根 K 线的逻辑索引为 0。

**数据**

- 空白项（只有 `{ time }`）在 K 线和 OHLC 柱数据中会被丢弃，在折线、面积、基线和柱状图数据中则表示缺口。
- 在这套 API 中 OHLC 数据不含成交量。成交量系列请使用原生 API。

**标记**

- `position: 'inBar'` 会画在 K 线上方，未知的 `shape` 画成圆形。`size` 是默认大小的倍数（1 为 10 CSS px，限制在 4–64 px 之间）；`id` 会被忽略。

**尚不支持**

- `subscribeClick` 不执行任何操作并输出警告。请改为通过 `chart.ferrochart` 使用原生的 `click`
  事件。
- `series.priceToCoordinate` 和 `series.coordinateToPrice` 返回 `null` 并输出警告。
- 自定义系列、系列图元（series primitives）和窗格图元（pane primitives）。FerroChart 自身的扩展机制见[插件](plugins.md)。
- `scrollToPosition(position, animated)` 会忽略 `animated`。
- `IPriceLine.options()` 返回你传入的选项，而不是全部选项。

### 在兼容入口中使用原生功能

`chart.ferrochart` 就是原生的 `FerroChart`，因此页面可以保留已有的
lightweight-charts 调用，同时使用原生功能：

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

通过原生对象添加的系列，兼容层并不知道（例如它们不会出现在十字光标事件的 `seriesData` 中）。

## 迁移到原生 API

原生 API 与 lightweight-charts 有几处总体上的不同：

- **创建是异步的。** `FerroChart.create(canvas, options)` 返回一个
  promise，并且接收一个由 CSS 设定尺寸的 `<canvas>` 元素，而不是容器。
- **时间是 UTC 毫秒**，类型为 `number`、`bigint` 或（在行数据中）`Date`。
  把 lightweight-charts 的秒数乘以 1000 即可。
- **数据可以是列。** `setData` 接受行数组，但对大数据量来说，类型化数组构成的列要快得多。
- **类型用字符串表示。** `chart.addSeries('candlestick', options, pane)`；
  窗格用 id 表示，而不是索引。
- **选项是扁平的。** 系列颜色为 `color`、`downColor`、`topFill` 和
  `bottomFill`；主题为 `'light'` 或 `'dark'`。
- **事件使用 `on`/`off`。** 用 `chart.on('crosshairMove', ...)` 代替
  `subscribeCrosshairMove(...)`。

### 对照

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

### 常用调用对照表

| lightweight-charts | FerroChart |
|---|---|
| `createChart(container, options)` | `await FerroChart.create(canvas, { theme, renderer })` |
| `chart.addSeries(CandlestickSeries, o)` / `addCandlestickSeries(o)` | `chart.addCandlestickSeries(o)` 或 `chart.addSeries('candlestick', o)` |
| `BarSeries` / `addBarSeries` | `chart.addSeries('ohlc', o)` |
| `LineSeries`、`AreaSeries`、`BaselineSeries`、`HistogramSeries` | `chart.addSeries('line' \| 'area' \| 'baseline' \| 'histogram', o)` |
| `chart.addSeries(def, o, paneIndex)` | `chart.addSeries(kind, o, chart.panes()[paneIndex])`；用 `chart.addPane()` 新建窗格 |
| 使用 `priceScaleId: ''` 的成交量柱状图 | `chart.addVolumeSeries(candles)` |
| `chart.removeSeries(series)` | `series.remove()` |
| `series.setData(items)` | `series.setData(rows)`（时间为毫秒）或 `series.setData({ time, open, high, low, close, volume })` 列 |
| `series.update(item)` | `series.update(bar)`（时间为毫秒） |
| `series.applyOptions({ upColor, downColor })` | `series.applyOptions({ color, downColor })` |
| `series.applyOptions({ lineColor, topColor, bottomColor })` | `series.applyOptions({ color, topFill, bottomFill })` |
| `series.applyOptions({ priceFormat: { precision } })` | `series.applyOptions({ pricePrecision })` |
| `series.createPriceLine(o)` / `removePriceLine(line)` | `series.createPriceLine(o)` / `line.remove()` |
| `series.setMarkers(m)` / `createSeriesMarkers(series, m)` | `series.setMarkers(m)`（时间为毫秒） |
| `series.dataByIndex(i)` | `series.dataAt(logicalIndex)` |
| `chart.timeScale().fitContent()` | `chart.fitContent()` |
| `chart.timeScale().scrollToRealTime()` | `chart.scrollToRealtime()` |
| `timeScale().scrollToPosition(n)` / `scrollPosition()` | `chart.setRightOffset(n)` / `chart.rightOffset` |
| `timeScale().getVisibleLogicalRange()` | `chart.visibleLogicalRange()` |
| `timeScale().setVisibleLogicalRange({ from, to })` | `chart.setVisibleLogicalRange(from, to)` |
| `timeScale().setVisibleRange({ from, to })` | `chart.applySync({ type: 'visibleRange', from, to })`（毫秒） |
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
| `chart.applyOptions({ width, height })` / `resize(w, h)` | 设置 canvas 的 CSS 尺寸，图表会随之调整 |
| `chart.remove()` | `chart.destroy()` |

逻辑索引有一点不同：在 FerroChart 中，向前插入更早的数据时，K 线会保持其逻辑索引不变，因此第一根 K 线的索引可能为负数。`chart.dataRange()` 返回第一根和最后一根 K 线的索引。

各个原生调用的详细说明见[系列](series.md)、[数据](data.md)、
[时间轴](time-scale.md)、[价格轴](price-scale.md)、
[十字光标与事件](crosshair-events.md)和[标记](markers.md)。
