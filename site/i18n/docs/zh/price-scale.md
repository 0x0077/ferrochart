<!-- source: 9914dfe76f -->
# 价格轴与窗格

图表是由共用时间轴的多个窗格上下堆叠而成的。每个窗格在右侧都有自己的价格轴。本章介绍窗格的创建和尺寸调整、把系列放到窗格上，以及价格轴选项。

## 窗格

新建的图表只有一个窗格，即主窗格。`chart.mainPane` 是最上方窗格的 id，也是所有接受可选窗格参数的方法的默认值。

```ts
const main = chart.mainPane;
const volumePane = chart.addPane(0.25); // below the others, a quarter of the main pane's height
const oscPane = chart.addPane(0.3);

console.log(chart.panes());             // [main, volumePane, oscPane], top to bottom
```

| 方法 | 作用 |
|---|---|
| `addPane(stretch?)` | 在其他窗格下方添加一个窗格并返回其 id。`stretch` 是它相对于其他窗格的高度（默认 1）；必须是大于 0 的有限数，否则 `addPane` 会抛出异常。 |
| `removePane(pane)` | 移除一个窗格及其系列、画线和图层插件。最后一个窗格不能移除。 |
| `panes()` | 窗格 id，从上到下。 |
| `mainPane` | 最上方窗格的 id。 |
| `setPaneStretch(pane, stretch)` | 修改窗格的相对高度。`stretch` 必须是有限正数。 |
| `paneStretch(pane)` | 窗格的相对高度，id 未知时为 `undefined`。 |

高度与 stretch 系数成正比：系数为 1、0.25 和 0.3 时，各窗格分别获得可用高度的 1/1.55、0.25/1.55 和 0.3/1.55。用户可以拖动两个窗格之间的分隔条，这会改变这两个窗格的系数。

窗格被移除后，其 id 不会被重用。如果移除最上方的窗格，下一个窗格会成为主窗格。除 `paneStretch` 外，传入未知窗格 id 的方法都会抛出异常。

```ts
chart.setPaneStretch(oscPane, 0.5);
console.log(chart.paneStretch(oscPane)); // 0.5

chart.removePane(oscPane);               // its series go with it
```

窗格布局（窗格数量、stretch 系数和价格轴设置）是保存状态的一部分；见[保存状态](state.md)。

## 把系列放到窗格上

所有添加系列的方法都把窗格作为最后一个参数；省略时系列会放在主窗格上。`series.pane` 表示系列所在的窗格。

```ts
const candles = chart.addCandlestickSeries();          // main pane
const sma = chart.addSeries('line', { color: '#2962ff' });
const pane = chart.addPane(0.3);
const rsi = chart.addSeries('line', { color: '#7e57c2' }, pane);
rsi.setData(rsiValues);

console.log(rsi.pane === pane); // true
```

系列始终留在它所在的窗格上；要移动它，先移除，再在另一个窗格上重新添加。指标通过 placement 参数选择窗格；见[指标](indicators.md)。

同一窗格中的所有系列共用该窗格的价格轴，并一起自动缩放。

### 成交量的放置

`addVolumeSeries(source, pane?)` 的行为取决于该窗格上还有什么：

- **放在有价格系列的窗格上**（默认：主窗格，K 线旁边）时，成交量有一个独立的刻度，占据窗格底部四分之一。它不影响价格范围，价格轴仍显示价格。
- **放在单独的窗格上**时，成交量使用该窗格的价格轴并填满窗格：柱子立在底边上（底部边距为 0），坐标轴标签使用 K、M 和 B 后缀（`1.25M`）。

```ts
// Volume under the candles, in the same pane.
chart.addVolumeSeries(candles);

// Or in a pane of its own below.
const volPane = chart.addPane(0.25);
chart.addVolumeSeries(candles, volPane);
```

## 价格轴

`applyPriceScaleOptions(options, pane?)` 修改某个窗格（默认：主窗格）的价格轴；省略的键保持原值。它要么全部生效，要么全不生效：只要有一个值无效就会抛出异常，价格轴保留原有的所有选项。`priceScale(pane?)` 返回全部选项。

```ts
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
chart.applyPriceScaleOptions({ margins: { top: 0.2, bottom: 0.1 } }, pane);

const s = chart.priceScale();
// { mode: 'logarithmic', autoScale: true, inverted: false,
//   margins: { top: 0.1, bottom: 0.1 }, range: { low: 98.2, high: 112.7 } }
```

| 选项 | 类型 | 默认值 | 含义 |
|---|---|---|---|
| `mode` | `'normal' \| 'logarithmic' \| 'percentage' \| 'indexedTo100'` | `'normal'` | 价格如何映射到高度，见[模式](#modes) |
| `autoScale` | `boolean` | `true` | 范围是否跟随可见数据 |
| `inverted` | `boolean` | `false` | 较高的价格画在下方 |
| `margins` | `{ top, bottom }` | `{ top: 0.1, bottom: 0.1 }` | 数据上方和下方的空白，占窗格高度的比例 |
| `range` | `{ low, high }` | 跟随数据 | 显示的价格范围（不含边距）。设置后会关闭自动缩放。 |

`priceScale().range` 始终是当前显示的范围，自动缩放时也是如此。

各选项按 `mode`、`margins`、`inverted`、`range`、`autoScale` 的顺序应用。无效的值会抛出异常；按此顺序排在它之前的选项已经应用。

### 模式

| 模式 | 坐标轴显示 |
|---|---|
| `'normal'` | 线性刻度上的价格 |
| `'logarithmic'` | 对数刻度上的价格：相同的距离代表相同的比率。适用于价格变化很大的长期历史数据。 |
| `'percentage'` | 相对于第一个可见值的百分比变化：(p − base) / \|base\| × 100，标签保留 2 位小数并带 `%` |
| `'indexedTo100'` | 以第一个可见值为 100 的指数化价格：p / \|base\| × 100，标签保留 2 位小数 |

在 `'percentage'` 和 `'indexedTo100'` 模式下，窗格中的每个系列都以各自的第一个可见值为基准归一化：第一根可见 K 线的收盘价，或数值系列的第一个可见值。这样，价格水平相差很大的系列也能在同一刻度上比较。用户平移时，基准随第一根可见 K 线移动。范围、坐标轴标签和十字光标价格都以百分比或指数点表示。

在价格模式（`'normal'`、`'logarithmic'`）与相对模式（`'percentage'`、`'indexedTo100'`）之间切换会开启自动缩放，因为原来的范围单位不同。

### 自动缩放与固定范围

默认情况下，范围跟随窗格中所有系列可见数据的最高价和最低价（加上边距），并在你平移、缩放或收到新数据时更新。设置 `range` 会把它固定：

```ts
chart.applyPriceScaleOptions({ range: { low: 90, high: 110 } }); // autoScale is now false
chart.applyPriceScaleOptions({ autoScale: true });               // follow the data again
```

`low` 必须小于 `high`，且两者都是有限值；在对数模式下 `low` 必须大于 0。在相对模式下，范围以百分比或指数点表示。

`range` 会关闭自动缩放，所以 `{ range, autoScale: true }` 自相矛盾，会抛出异常；`{ range, autoScale: false }` 则没有问题。

用户也可以改变范围：拖动价格轴或在其上滚动滚轮（或在绘图区内按 Alt + 滚轮）会缩放价格，并关闭自动缩放。双击价格轴会重新开启自动缩放。见[手势与键盘](interaction.md#gestures)。

### 边距

`margins` 使数据与窗格的上下边缘保持距离。两个值都必须在 0 到 1 之间（含 0，不含 1），且两者之和必须小于 1。两个键都必须提供。

```ts
// Leave the top 30 % free for a legend.
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0.05 } });
```

### 反转

`inverted: true` 会翻转刻度：较高的价格画在下方。有时用于把债券收益率与价格对照，或用于反向货币对。

```ts
chart.applyPriceScaleOptions({ inverted: true });
```

## 示例：比较两个品种

在同一窗格上以百分比模式显示两个折线系列，展示从视图左边缘起的相对表现。

```ts
const a = chart.addSeries('line', { color: '#2962ff' });
const b = chart.addSeries('line', { color: '#ff6d00' });
a.setData({ time, value: closesA }); // around 180
b.setData({ time, value: closesB }); // around 0.95

chart.applyPriceScaleOptions({ mode: 'percentage' });
```

## 示例：三窗格布局

主窗格显示 K 线和一条移动平均线，下方是成交量，最底部是一个震荡指标。

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
