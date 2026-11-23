<!-- source: b0ef03b731 -->
# 标记与价格线

标记是附着在 K 线上的小图标，可带文字，例如买卖信号或新闻。价格线是位于固定价格处的水平线，例如委托单、止损或目标价。两者都属于某个系列。每个系列还有一个最新值标签和一条最新值线，通过系列选项开启或关闭。

## 标记

### 设置标记

```ts
candles.setMarkers([
  { time: Date.UTC(2024, 0, 15), position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: Date.UTC(2024, 1, 2), position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
  { time: Date.UTC(2024, 1, 20), shape: 'circle', text: 'Earnings' },
]);
```

`setMarkers` 会替换该系列的所有标记。要添加一个标记，先读取当前列表，追加后再重新设置；要清空标记，传入空数组：

```ts
candles.setMarkers([...candles.markers(), { time: Date.UTC(2024, 2, 1), text: 'Split' }]);
candles.setMarkers([]);
```

### 标记字段

| 字段 | 类型 | 默认值 | 含义 |
|---|---|---|---|
| `time` | `number \| bigint \| Date` | 必填 | UTC 毫秒（整数）或 `Date`；`markers()` 返回数字 |
| `position` | `'aboveBar' \| 'belowBar'` | `'aboveBar'` | 位于 K 线最高价之上或最低价之下。对于单值系列，位于数值之上或之下。 |
| `shape` | `'circle' \| 'square' \| 'arrowUp' \| 'arrowDown'` | `'circle'` | 实心图标 |
| `color` | `Color \| null` | `null` | 图标和文字颜色；`null` 表示使用系列的主色（K 线和柱使用上涨颜色） |
| `text` | `string` | `''` | 图标旁的文字，位于远离 K 线的一侧；为空则不显示 |

未知的 `position` 或 `shape`、缺失或非整数的 `time`，或无效的颜色都会抛出异常，标记保持不变。

### 读取标记

`series.markers()` 返回按时间排序的标记，所有字段都已填充（使用默认值的地方 `color` 为 `null`）。排序是稳定的：时间相同的标记保持你给出的顺序。

标记在此列表中的索引就是 `click` 事件报告的 `target.index`（见[十字光标与事件](crosshair-events.md)）。由于列表经过排序，该索引可能与你传给 `setMarkers` 的数组中的位置不同。

### 放置与吸附

- 标记绘制在时间等于 `time` 的 K 线上。落在两根 K 线之间的时间会吸附到**后一根** K 线；晚于最后一根 K 线的时间吸附到最后一根。早于第一根 K 线的时间吸附到第一根。
- 如果系列在该 K 线处没有值（缺口），则不绘制该标记。
- 标记跟随系列的 `shift` 选项。
- 图标直径 10 CSS px，与 K 线相距 4 CSS px。
- 同一根 K 线同一侧的多个标记按 `markers()` 的顺序向远离 K 线的方向堆叠。带文字的标记占用图标加一行文字的空间。
- 文字以 K 线为中心水平居中，`aboveBar` 时在图标上方，`belowBar` 时在图标下方，使用标记的颜色。
- 标记会被裁剪到绘图区内。

```ts
// Two signals on the same bar: the first sits next to the low, the second below it.
candles.setMarkers([
  { time: t, position: 'belowBar', shape: 'arrowUp', text: 'Entry' },
  { time: t, position: 'belowBar', shape: 'circle', text: 'Alert' },
]);
```

### 哪些系列可以有标记

除成交量系列外的所有系列类型。在成交量系列上调用 `setMarkers` 会抛出异常。标记也可以放在指标输出上（`Indicator` 的 `outputs()` 就是普通系列）：

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];
rsi.setMarkers([{ time: t, position: 'aboveBar', shape: 'square', text: 'OB' }]);
```

### 示例：根据信号生成标记

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

## 价格线

### 创建价格线

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

`createPriceLine` 返回一个 `PriceLine`。价格以系列的单位表示，因此在百分比或指数化价格轴上，价格线会随系列移动（见[价格轴](price-scale.md)）。

### 选项

| 选项 | 类型 | 默认值 | 含义 |
|---|---|---|---|
| `price` | `number` | `0` | 价格，以系列的单位表示；必须是有限值 |
| `color` | `Color \| null` | `null` | 线条、标题和坐标轴标签的颜色；`null` 表示使用系列的主色 |
| `lineWidth` | `number` | `1` | CSS px，大于 0 |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'dashed'` | 虚线样式 |
| `axisLabelVisible` | `boolean` | `true` | 在价格轴上显示价格标签 |
| `title` | `string` | `''` | 在绘图区右边缘、线条上方显示的文字；为空则不显示 |

坐标轴标签使用系列的价格格式（其 `pricePrecision`）。价格轴上的标签在会重叠时会被错开。位于可见价格范围之外的价格线不会绘制。

### 修改与移除

```ts
entry.applyOptions({ price: 101.5 });         // only the keys you pass change
console.log(entry.options());                   // every option: { price, color, lineWidth, ... }
stop.applyOptions({ title: 'Stop (moved)', lineStyle: 'dotted' });

stop.remove();
```

`remove()` 之后，在该对象上调用 `options`、`applyOptions` 或 `remove` 都会抛出异常。移除系列会一并移除其价格线。

对于成交量系列、非有限的价格、不是对象的选项参数或无效的选项值，`createPriceLine` 会抛出异常。

### 示例：可拖动的委托线

价格线本身不处理输入。可以把它与图表的事件和坐标转换结合起来，或者使用用户可以拖动的 `horizontal_line` 画线（见[画线工具](drawings.md)）：

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

## 最新值标签与最新值线

每个系列都会在价格轴上以标签形式、并在绘图区中以一条水平点线显示其最新值。当最后一根 K 线收盘价不低于开盘价时（对于基线系列：不低于基准值），两者使用上涨颜色，否则使用下跌颜色；折线、面积、柱状图和散点系列使用其主色。

| 系列选项 | 默认值 | 含义 |
|---|---|---|
| `lastValueVisible` | `true` | 在价格轴上显示最新值标签 |
| `priceLineVisible` | `true` | 在最新值处显示点线 |
| `pricePrecision` | `null` | 标签的小数位数；`null` 表示自动 |

```ts
const line = chart.addSeries('line', { lastValueVisible: true, priceLineVisible: false });
candles.applyOptions({ priceLineVisible: false });
```

成交量系列两者都不绘制。指标输出创建时为 `priceLineVisible: false`；其最新值标签会显示。可以逐个输出关闭：

```ts
for (const s of chart.addIndicator(candles, 'boll').outputs()) {
  s.applyOptions({ lastValueVisible: false });
}
```

系列选项见[系列](series.md)。
