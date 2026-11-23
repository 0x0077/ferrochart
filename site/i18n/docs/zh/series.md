<!-- source: c2e84c153f -->
# 系列类型

一个系列就是以某种方式绘制的一组数据：K 线、折线、柱状图等等。一个图表可以在任意多个窗格上容纳任意多个系列。同一图表的所有系列共享一条时间轴（参见[数据](data.md)）。

## 添加系列

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas);

const candles = chart.addSeries('candlestick');           // main pane
const sma = chart.addSeries('line', { color: '#2962ff' }); // with options
const pane = chart.addPane(0.3);
const osc = chart.addSeries('histogram', {}, pane);        // on another pane
```

`addSeries(kind, options?, pane?)` 返回一个 `Series` 句柄。省略
`pane` 时，系列会放在主（最上方）窗格中。`addCandlestickSeries(options?,
pane?)` 是 `addSeries('candlestick', ...)` 的简写。未知的类型或窗格 id 会抛出异常。

`Series` 有三个只读属性：

| 属性 | 含义 |
|---|---|
| `id` | 数字，在图表内唯一。同一系列的不同句柄（例如来自 `click` 事件的句柄）id 相同。 |
| `kind` | 传给 `addSeries` 的类型；成交量系列为 `'volume'`，Kagi / 点数图变换为 `'kagi'` / `'pointFigure'`（Renko 和等幅 K 线报告为 `'candlestick'` 或 `'ohlc'`），插件系列为 `'custom'`。 |
| `pane` | 系列所在窗格的 id。 |

## 类型

| 类型 | 数据 | 绘制内容 |
|---|---|---|
| `'candlestick'` | OHLC | K 线：实体从开盘价到收盘价，影线从最低价到最高价 |
| `'ohlc'` | OHLC | 美国线：左侧短横为开盘价，右侧短横为收盘价 |
| `'heikinAshi'` | OHLC | 由你传入的 K 线计算出的平均 K 线（Heikin-Ashi） |
| `'line'` | value | 穿过各个值的折线 |
| `'area'` | value | 折线，下方带有延伸到窗格底部的渐变填充 |
| `'baseline'` | value | 折线，以及 `baseValue` 上方和下方不同颜色的填充 |
| `'histogram'` | value | 从 `baseValue` 到各个值的柱子，可逐点着色 |
| `'points'` | value | 每个值处的一个圆点 |

成交量柱状图通过 `addVolumeSeries` 添加（参见[成交量](#volume)），
Renko、等幅 K 线、Kagi 和点数图通过 `addTransformSeries` 添加（参见
[K 线变换](#bar-transforms)）。

OHLC 系列接收 `{ time, open, high, low, close, volume? }`；值系列接收
`{ time, value, color? }`。[数据](data.md)一章介绍了这两种格式、行数组、校验和实时更新。

### K 线

```ts
const candles = chart.addCandlestickSeries({
  color: '#26a69a',      // up candles (close >= open)
  downColor: '#ef5350',  // down candles
});
candles.setData(randomWalk(1, 500, Date.UTC(2024, 0, 1), 60_000));
```

`color` 和 `downColor` 都未设置时，影线使用主题的影线颜色。只要设置了其中之一，影线就与实体同色。

### OHLC 柱

`'ohlc'` 接收与 `'candlestick'` 相同的数据和选项，绘制传统的美国线。

### 平均 K 线（Heikin-Ashi）

`'heikinAshi'` 接收普通的 OHLC K 线；图表据此计算 Heikin-Ashi 值，实时更新时也一样。颜色规则与 K 线相同。

### 折线

```ts
const line = chart.addSeries('line', { color: '#2962ff', lineWidth: 1, lineStyle: 'dashed' });
line.setData({ time: [0, 60_000, 120_000, 180_000], value: [10, 11, NaN, 12] });
```

`NaN` 值表示缺口：折线不会穿过它。

### 面积图

面积图是下方带有垂直渐变的折线。`topFill` 是折线处的颜色，`bottomFill` 是窗格底部的颜色。

```ts
chart.addSeries('area', {
  color: '#2962ff',
  topFill: 'rgba(41, 98, 255, 0.4)',
  bottomFill: 'rgba(41, 98, 255, 0)',
});
```

默认情况下，两种填充都使用 `color`，顶部不透明度为 40 %，底部为 2 %。

### 基线图

基线系列在 `baseValue` 处分开：其上方折线使用 `color`，填充使用 `topFill`；其下方折线使用 `downColor`，填充使用
`bottomFill`。

```ts
chart.addSeries('baseline', {
  baseValue: 100,
  color: '#26a69a',
  downColor: '#ef5350',
});
```

默认情况下，填充为两种折线颜色，不透明度 20 %。

### 柱状图

柱子从 `baseValue`（默认 0）延伸到各个值。自动缩放的范围始终包含 `baseValue`。

每个点都可以有自己的颜色：在 `value` 旁边传入一个 `color` 列。其中的元素可以是 CSS 颜色、表示使用系列颜色的 `null`，也可以是由 `0xRRGGBBAA` 值组成的 `Uint32Array`，其中 `0` 表示系列颜色。

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

逐点颜色只在完整细节下绘制。缩小到多根 K 线共用一个像素而被合并时，会使用系列颜色。其他类型也接受 `color` 列，但会忽略它。`update` 不能设置颜色；追加的点使用系列颜色。

### 散点

`'points'` 在每个值处画一个圆点；`lineWidth` 是圆点的直径，单位为 CSS
px。缺口（`NaN`）处没有圆点。

```ts
const dots = chart.addSeries('points', { color: '#ff9800', lineWidth: 4 });
```

### 成交量

成交量系列绘制某个 OHLC 系列的 `volume` 列。它没有自己的数据：对源系列调用 `setData` 和 `update` 就会更新它。

```ts
const candles = chart.addCandlestickSeries();
const volume = chart.addVolumeSeries(candles);          // main pane
// or: chart.addVolumeSeries(candles, chart.addPane(0.25));
```

源系列必须是 OHLC 系列（K 线、OHLC 柱、Heikin-Ashi）或变换系列。柱子按源 K 线的涨跌着色，使用主题的成交量颜色。在同时显示价格的窗格中，成交量在窗格底部四分之一处有自己的坐标；在独立窗格中则填满整个窗格。参见[价格轴与窗格](price-scale.md#volume-placement)。

移除源系列时，其成交量系列也会一并移除。

## 系列选项

选项在 `addSeries` 时传入，之后可以用 `applyOptions` 修改；省略的键保持原值。`options()` 返回所有键。

```ts
const s = chart.addSeries('line', { color: '#2962ff' });
s.applyOptions({ lineWidth: 3 });       // color stays '#2962ff'
console.log(s.options().lineWidth);     // 3
s.applyOptions({ color: null });        // back to the theme color
```

| 选项 | 类型 | 默认值 | 适用于 | 含义 |
|---|---|---|---|---|
| `color` | `Color \| null` | 主题 | 全部 | 线条颜色（折线、面积图、散点）；上涨 K 线和柱；柱状图的柱子；基线上方的线；Kagi 阳线和点数图的 X 列 |
| `downColor` | `Color \| null` | 主题 | candlestick、ohlc、heikinAshi、baseline、变换 | 下跌 K 线和柱；基线下方的线；Kagi 阴线和点数图的 O 列 |
| `topFill` | `Color \| null` | 见[面积图](#area)、[基线图](#baseline) | area、baseline | 面积图在折线处的填充；基线上方的填充 |
| `bottomFill` | `Color \| null` | 见[面积图](#area)、[基线图](#baseline) | area、baseline | 面积图在底部的填充；基线下方的填充 |
| `lineWidth` | `number` | `2` | line、area、baseline、points、Kagi、点数图 | 线宽，单位为 CSS px；对散点而言是圆点直径。必须为正数。 |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | line、area、baseline | 线条的虚线样式 |
| `step` | `boolean` | `false` | line、area、baseline | 阶梯线，参见[阶梯线](#step-lines) |
| `baseValue` | `number` | `0` | baseline、histogram | 基线填充的分界处；柱状图柱子的起点。必须是有限值。 |
| `lastValueVisible` | `boolean` | `true` | 除成交量外全部 | 在价格轴上显示最新值标签 |
| `priceLineVisible` | `boolean` | `true` | 除成交量外全部 | 在最新值处画一条水平点线 |
| `shift` | `number` | `0` | 全部 | 以 K 线数为单位的显示偏移，参见[偏移](#shift) |
| `pricePrecision` | `number \| null` | `null` | 全部 | 价格标签的小数位数，参见[价格精度](#price-precision) |

无效的值（未知的线型、格式错误的颜色、负的宽度）会抛出异常，系列保留原来的选项。

成交量系列始终使用主题的成交量颜色。

### 颜色

颜色是 CSS 字符串：`#rgb`、`#rgba`、`#rrggbb`、`#rrggbbaa`、
`rgb(r, g, b)`、`rgba(r, g, b, a)`（alpha 取值 0 到 1）或 `'transparent'`。
不接受 `'red'` 这样的颜色名称。`null` 表示主题默认值。
`options()` 返回的颜色为 `#rrggbb`，半透明时为 `rgba(...)`，主题颜色则为 `null`。

### 阶梯线

设置 `step: true` 后，折线先水平延伸到下一个点的时间，再垂直移动到它的值。适用于在变化之前保持不变的值，例如利率或持仓规模。

```ts
const rate = chart.addSeries('line', { step: true, color: '#7e57c2' }, pane);
rate.setData({
  time: [Date.UTC(2024, 0, 1), Date.UTC(2024, 2, 20), Date.UTC(2024, 6, 31)],
  value: [5.5, 5.25, 5.0],
});
```

### 偏移

`shift` 把逻辑索引 `j` 处的点画在 `j + shift` 处：正值把系列向右（未来方向）移动，负值向左移动。它必须是整数。自动缩放、十字光标和 `dataAt` 都以绘制位置为准。一目均衡表（Ichimoku）的先行带和迟行线就使用了它。

```ts
// A moving average drawn 10 bars ahead (a "displaced" moving average).
const dma = chart.addSeries('line', { shift: 10 });
```

### 价格精度

`pricePrecision` 设置价格轴标签、最新值标签和十字光标标签的小数位数，取值 0 到 12。`null`（默认）表示自动：至少 2 位小数，并保证可见价格有 4 位有效数字。非常小的价格用下标表示零的个数：
`0.0₄1234` 即 0.00001234。

一个窗格的标签使用其第一个价格系列的精度。在
`'percentage'` 和 `'indexedTo100'` 价格轴模式下，坐标轴始终使用 2 位小数。0 到 12 之间的整数和 `null` 以外的任何值都会抛出异常。

```ts
candles.applyOptions({ pricePrecision: 5 }); // FX quotes
candles.applyOptions({ pricePrecision: null }); // automatic again
```

## K 线变换

变换系列接收普通的 K 线（或逐笔成交），并绘制某种图表方法由此得出的结果：Renko 砖块、等幅 K 线、Kagi 线或点数图的列。这些方法忽略时间；每个元素在时间轴上占据自己的位置。

```ts
const renko = chart.addTransformSeries('renko', { boxSize: 0.5 });
renko.setData(randomWalk(1, 5_000, Date.UTC(2024, 0, 1), 60_000));
chart.addVolumeSeries(renko); // optional
chart.fitContent();
```

`addTransformSeries(name, params, options?, pane?)` 返回一个 `Series`。像 K 线系列一样用 `setData` 和 `update` 为它提供数据。一笔成交就是四个价格相等的 K 线：

```ts
renko.update({ time: tick.time, open: tick.price, high: tick.price, low: tick.price, close: tick.price, volume: tick.size });
```

### 参数

价格以数据本身的单位表示。

| 变换 | 参数 | 绘制为 |
|---|---|---|
| `'renko'` | `boxSize`（价格）**或** `atr`（ATR 周期，整数）；`kind?` | K 线（默认）或 OHLC 柱 |
| `'rangeBars'` | `range`（价格）；`kind?` | K 线（默认）或 OHLC 柱 |
| `'kagi'` | `reversal`（价格）**或** `reversalPercent` | Kagi 线 |
| `'pointFigure'` | `boxSize`（价格）；`reversal?`（格数，整数，默认 3） | X 列和 O 列 |

`kind` 为 `'candlestick'`（默认）或 `'ohlc'`。数值必须是有限正数。同时给出两种互斥参数（`boxSize` 和 `atr`，或 `reversal` 和
`reversalPercent`）、两者都不给，或者使用未知名称，都会抛出异常。

```ts
chart.addTransformSeries('renko', { atr: 14, kind: 'ohlc' });
chart.addTransformSeries('rangeBars', { range: 1 });
chart.addTransformSeries('kagi', { reversalPercent: 2 });
chart.addTransformSeries('pointFigure', { boxSize: 1, reversal: 3 });
```

（以上每一行都需要一个单独的图表，参见
[限制](#constraints)。）

### Renko

Renko 只使用收盘价。每当收盘价超出上一块砖整整一个 `boxSize` 时就添加一块砖：沿同一方向继续需要一格，反转则需要两格。第一块砖以第一个收盘价为锚点。

使用 `atr: n` 时，格子大小为前 `n` 根 K 线的平均真实波幅，此后固定不变；在此之前不绘制任何内容。

Renko 没有正在形成的砖块：一块砖只有在完成后才会出现。

### 等幅 K 线

每根 K 线从最低价到最高价的跨度恰好为 `range`。在一根输入 K 线内部，假定上涨 K 线的价格走势为开盘 → 最低 → 最高 → 收盘，下跌 K 线为开盘 →
最高 → 最低 → 收盘。价格离开当前 K 线的范围时，该 K 线完成，下一根在其边缘开盘。最后一根 K 线处于形成中，会随更新而变化。

### Kagi

Kagi 线沿收盘价的方向延伸，当收盘价反向变动至少 `reversal`（或线末端价格的 `reversalPercent` 个百分点）时转折。线条升破前一个肩部时变粗（阳线，用 `color` 绘制），跌破前一个腰部时变细（阴线，用 `downColor`）。最后一条线处于形成中。

### 点数图（Point & Figure）

价格被划分为大小为 `boxSize` 的格子。收盘价向上填满新格子时，X 列不断上升；反转达到 `reversal` 格时开始一列 O，反之亦然。X 列使用 `color`，O 列使用 `downColor`。最后一列处于形成中。

### 限制

- 一个图表最多只有一个变换系列，除成交量外旁边不能有其他系列。添加第二个变换或任何其他系列都会抛出异常，向已有系列的图表添加变换也一样。请用单独的图表显示原始 K 线，需要时用
  [联动](sync.md)让两者保持同步。
- 每个元素的时间是使其完成的那根输入 K 线的时间（如果这样不能保持递增，则比前一个元素晚一毫秒）。`timeAtIndex` 和时间轴显示的就是这些时间。
- 每根 K 线的输入成交量计入在它之后正在形成的元素。没有输入成交量时，成交量为空。
- 单根输入 K 线最多产生 65536 个元素。如果格子或幅度相对于价格波动太小，导致某根 K 线超出这一数量，调用会抛出异常并清空该系列。
- 缺口 K 线（所有价格均为 `NaN`）会被跳过。

## 两个系列之间的带状区域

`addBand(upper, lower, options?)` 填充同一窗格中两个单值系列（折线、面积图、基线图、柱状图或散点）之间的区域。`upper` 位于 `lower` 上方或与之重合时使用 `fill`，位于下方时使用 `fillCrossed`。
带状区域绘制在窗格中各系列的后面（成交量的上面）。

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

`addBand` 返回一个带状区域 id。`fill` 为 `null`（或缺省）时，使用上方系列的颜色，不透明度 15 %；`fillCrossed` 为 `null` 时与 `fill` 相同。
对未知 id，`bandOptions` 返回 `null`。任一系列被移除时，带状区域会自动移除。传入两个 OHLC 系列、位于不同窗格的系列，或者同一个系列两次，都会抛出异常。

带有带状区域的指标（布林带、肯特纳通道、一目均衡表）会自动创建它们；参见
[指标](indicators.md)。

## 移除系列

```ts
line.remove();
```

`remove()` 会删除该系列、它的数据、标记和价格线，以及使用它的带状区域。移除 OHLC 系列时也会移除其成交量系列。
此后，该句柄的每个方法都会抛出异常（`'the series was removed'`）。
移除最后一个有数据的系列会重置时间轴。

指标的输出不能单独移除：对其中之一调用 `remove()` 会抛出异常；请改为移除整个指标（`indicator.remove()`）。

要移除整个窗格及其所有系列，请使用 `chart.removePane(id)`（参见
[价格轴与窗格](price-scale.md)）。
