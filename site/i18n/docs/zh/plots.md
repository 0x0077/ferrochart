<!-- source: 0b3cf203ce -->
# 绘图（FerroPlot）

`FerroChart` 是围绕时间序列上的 K 线构建的。对于其他一切，例如波动率微笑、收益分布直方图、行业柱状图、盈亏瀑布图或相关性热力图，请使用 `FerroPlot`：一种通用的 x/y 绘图，支持数值、对数、分类和日期坐标轴。它与图表使用相同的渲染器（先 WebGPU，后 WebGL2）、相同的 canvas 处理方式和相同的主题。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const plot = await FerroPlot.create(canvas, { theme: 'dark' });

const smile = plot.addMark('line', { pointSize: 5 });
smile.setData({
  x: [80, 90, 100, 110, 120],          // strikes
  y: [0.31, 0.26, 0.24, 0.25, 0.29],   // implied volatility
});
plot.axis('right').applyOptions({ format: { type: 'percent', decimals: 0 }, title: 'IV' });
plot.axis('x').applyOptions({ title: 'Strike' });
```

一个绘图由以下部分组成：

- **图元（marks）**：折线、面积、散点、柱、瀑布和单元格，各自有自己的数据和选项；
- 三条**坐标轴**：底部的 `'x'`、`'right'`，以及 `'left'`（默认隐藏）；
- **参考线（rules）**：位于某个值处的水平或垂直线，或带阴影的区间；
- **标签（labels）**：位于数据坐标处的文本或符号。

## 创建绘图

`FerroPlot.create(canvas, options?)` 与
`FerroChart.create` 一样返回一个 promise，并在首次使用时加载 WebAssembly 模块。canvas 的尺寸由 CSS 决定；参见[主题与样式](styling.md#sizing-the-canvas)。

| 选项 | 取值 | 默认值 |
|---|---|---|
| `theme` | `'light'`、`'dark'` | `'light'` |
| `renderer` | `'auto'`、`'webgpu'`、`'webgl2'` | `'auto'` |
| `crosshair` | 穿过悬停元素的十字光标线 | `false` |
| `hoverHighlight` | 高亮悬停的元素 | `true` |
| `attributionLogo` | 绘图区左下角的 FerroChart 标志 | `true` |
| `touchScroll` | `'chart'`、`'page'`：由谁处理竖向滑动；见[触屏](web.md#touch-screens) | `'chart'` |

`FerroPlot` 的其他成员：

| 成员 | 说明 |
|---|---|
| `addMark(kind, options?): Mark` | 添加一个图元，绘制在先前的图元之上 |
| `axis(name): Axis` | `'x'`、`'left'` 或 `'right'` |
| `addRule(options): Rule` | 位于某个值处的线或区间 |
| `addLabel(options): Label` | 位于某个点的文本或符号 |
| `fit()` | 所有坐标轴恢复到配置的范围；自动坐标轴重新跟随数据 |
| `on(event, callback)`、`off(event, callback)` | 事件：`hover`、`click`、`viewChange` |
| `playIntro(options?)`、`finishIntro()` | 入场动画，与图表相同 |
| `setAttributionLogo(on)` | 显示或移除标志 |
| `renderer` | `'webgpu'` 或 `'webgl2'` |
| `destroy()` | 移除监听器和标志，释放 GPU 资源（`dispose()` 是同一个方法的旧名称） |

用户可以平移和缩放绘图：拖动即平移，滚轮或双指捏合以指针为中心缩放 x 轴（在 y 轴上方或按住 Alt 时缩放该轴），Shift+滚轮和水平滚动用于平移，沿坐标轴拖动可缩放该轴，双击则恢复适配。分类坐标轴可以平移但不能缩放。

在触屏上，单指平移，双指以两指中点为中心缩放；各方向的缩放由两指在该方向上的距离变化决定（某方向上两指相距不足 16 CSS px 时，该方向不缩放）。手指按在元素上会显示其悬停状态，抬起后仍保留，除非手指进行了平移；手指对点的命中范围比鼠标多 8 CSS px。

## 图元

`plot.addMark(kind, options?)` 以 `MarkOptions` 添加一个 `MarkKind` 类型的图元，并返回一个 `Mark` 句柄。

| 类型 | 绘制内容 | 使用的数据 |
|---|---|---|
| `'line'` | 按数据顺序连接的点 | `x`、`y` |
| `'area'` | 折线与 `y0`（或 `baseValue`）之间的区域 | `x`、`y`、`y0?` |
| `'scatter'` | 每个点一个符号 | `x`、`y`、`size?`、`value?`、`color?`、`label?` |
| `'bars'` | 从 `y0`（或 `baseValue`）到 `y` 的柱子 | `x`、`y`、`y0?`、`value?`、`color?`、`label?` |
| `'waterfall'` | 分类坐标轴上的累计值柱 | `x`、`y`、`step?` |
| `'cells'` | 网格中的着色单元格（热力图） | `x`、`y`、`value` |

### 数据

`mark.setData(data)` 替换数据。所有列长度相同；类型化数组以每列一次 `memcpy` 的方式复制。

| 列 | 类型 | 含义 |
|---|---|---|
| `x` | `Float64Array \| number[]` | 数值、UTC 毫秒（日期坐标轴）或分类编号。对于水平柱：数值 |
| `y` | `Float64Array \| number[]` | 数值；`NaN` 表示缺口。对于水平柱：在 y 轴上的位置 |
| `y0` | `Float64Array \| number[]` | 面积的下边缘，柱子的起点 |
| `value` | `Float64Array \| number[]` | 按颜色映射着色的值（单元格、柱、散点） |
| `size` | `Float32Array \| number[]` | 散点直径，CSS px |
| `color` | `(string \| null)[] \| Uint32Array` | 逐点颜色（`null`：图元颜色），或 `0xRRGGBBAA` |
| `step` | `('delta' \| 'subtotal' \| 'total')[]` | 瀑布图中的角色；默认全部为 `'delta'` |
| `label` | `(string \| null)[]` | 逐点文本：散点的标签；替换柱和单元格的数值文本 |

列长度不同、某个 `x` 不是有限值，或分类坐标轴上的值不是从 0 开始的整数时，`setData` 会抛出异常并保留原有数据。

修改数据有三种方式：

| 方法 | 用途 |
|---|---|
| `setData(data)` | 替换全部数据 |
| `append(data)` | 在末尾添加点；`data` 的列必须与图元数据的列相同 |
| `update(point)` | 替换最后一个点，例如实时值；O(1)，不分配内存 |

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const pnl = plot.addMark('area', { color: '#26a69a', downColor: '#ef5350' });
plot.axis('x').applyOptions({ format: 'date' });

const t0 = Date.UTC(2024, 0, 2);
pnl.setData({ x: [t0, t0 + 60_000, t0 + 120_000], y: [0, 120, 80] });
pnl.append({ x: [t0 + 180_000], y: [150] });   // a new point
pnl.update({ x: t0 + 180_000, y: 165 });         // the last point changed
```

`update` 接收一个 `MarkPoint`（`{ x, y, y0?, value?, size?, color? }`）；图元没有的列对应的字段会被忽略。图元没有数据或 `x` 不是有限值时会抛出异常。颜色映射的值域和自动单元格大小保持上一次 `setData` 或 `append` 计算出的值。

### 选项

选项在 `addMark` 时传入，用 `mark.applyOptions(options)` 修改；省略的键保持原值。`mark.options()` 返回所有选项。
颜色为 `null` 表示默认值：对 `color` 而言是绘图调色板中的下一种颜色（第一个图元使用主题的线条颜色），对其他颜色而言是主题颜色。

| 选项 | 适用于 | 含义 | 默认值 |
|---|---|---|---|
| `yAxis` | 全部 | `'left'` 或 `'right'` | `'right'` |
| `orientation` | bars、waterfall | `'vertical'` 或 `'horizontal'` | `'vertical'` |
| `color` | 全部 | 主颜色 | 调色板 |
| `downColor` | area、waterfall | 基准值以下的区域；瀑布图中的减少项 | 主题 |
| `totalColor` | waterfall | 小计和总计 | 主题 |
| `fillAlpha` | area | 填充不透明度，0 到 1 | `0.25` |
| `lineWidth` | line、area | CSS px，> 0 | `2` |
| `lineStyle` | line、area | `'solid'`、`'dashed'`、`'dotted'` | `'solid'` |
| `stepLine` | line、area | 阶梯线：先水平延伸到下一个 x，再垂直移动 | `false` |
| `pointSize` | line、scatter | 折线：每个点处的符号大小（0：不显示）。散点：没有 `size` 列时的直径 | `0` |
| `icon` | line、scatter | `'circle'`、`'square'`、`'arrowUp'`、`'arrowDown'` | `'circle'` |
| `baseValue` | area、bars | 没有 `y0` 时面积和柱子的起点 | `0` |
| `group` | bars | 同一组的柱子在一个位置内并排；`null` 表示不分组 | `null` |
| `stack` | bars、area | 同一堆叠中的图元相互累加（正值和负值分开）；`null` 表示不堆叠 | `null` |
| `band` | bars、cells | 占据位置宽度的比例，0（不含）到 1；`null` 表示默认值（柱为 0.8，单元格为 1） | `null` |
| `colorScale` | cells、bars、scatter | `value` 如何映射为颜色（见下文）；`null` 表示默认的顺序色阶 | `null` |
| `showValues` | bars、waterfall、cells | 空间足够时在每个元素上写出数值 | `false` |
| `valueFormat` | bars、waterfall、cells | 这些数值的格式（参见[格式](#formats)） | `'auto'` |
| `cellSize` | cells | 数值坐标轴上以数据单位表示的 `[width, height]`；`null`：不同 x 之间以及不同 y 之间的最小间距 | `null` |
| `hoverable` | 全部 | 可被悬停和点击命中 | `true` |
| `visible` | 全部 | 是否绘制 | `true` |

无效的值会抛出异常，例如 `'fillAlpha' must be between 0 and 1` 或
`'icon' must be 'circle', 'square', 'arrowUp' or 'arrowDown'`。

`mark.id` 在其所属绘图中唯一，`mark.kind` 是其类型，`mark.remove()`
用于移除该图元。

### 分类坐标轴上的柱状图

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const x = plot.axis('x');
x.applyOptions({ type: 'category' });
x.setCategories(['Tech', 'Energy', 'Banks', 'Health']);

// Two series side by side in each slot.
const q1 = plot.addMark('bars', { group: 1, showValues: true });
const q2 = plot.addMark('bars', { group: 1, showValues: true });
q1.setData({ x: [0, 1, 2, 3], y: [4.2, -1.3, 2.1, 0.8] });
q2.setData({ x: [0, 1, 2, 3], y: [3.1, 0.6, -0.4, 1.9] });

plot.axis('right').applyOptions({ format: { type: 'percent', decimals: 1 } });
plot.addRule({ value: 0, lineStyle: 'solid' });
```

分类 `k` 占据从 `k` 到 `k + 1` 的位置，标签为
`labels[k]`。如果用 `stack: 1` 代替 `group: 1`，柱子就会堆叠。
要绘制水平柱，请设置 `orientation: 'horizontal'`，把左侧或右侧坐标轴设为分类坐标轴，并交换各列的含义：`x` 存放数值，`y` 存放分类编号。

### 瀑布图

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const x = plot.axis('x');
x.applyOptions({ type: 'category' });
x.setCategories(['Revenue', 'COGS', 'Gross', 'Opex', 'Tax', 'Net']);

const pnl = plot.addMark('waterfall', {
  color: '#26a69a',       // increases
  downColor: '#ef5350',   // decreases
  totalColor: '#5d6d7e',  // subtotals and totals
  showValues: true,
  valueFormat: 'compact',
});
pnl.setData({
  x: [0, 1, 2, 3, 4, 5],
  y: [1_200_000, -450_000, 0, -380_000, -90_000, 0],
  step: ['delta', 'delta', 'subtotal', 'delta', 'delta', 'total'],
});
```

`'delta'` 把它的值加到累计值上。`'subtotal'` 或 `'total'`
绘制一根从 0 到当前累计值的柱子，并保持累计值不变；它的 `y` 不会被使用。在悬停和点击事件中，小计或总计的 `hit.y` 就是当前累计值。

### 带大小、颜色和标签的散点图

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const assets = plot.addMark('scatter', { colorScale: 'diverging' });
assets.setData({
  x: [12.1, 18.4, 25.0, 9.8],          // volatility, %
  y: [6.2, 9.1, 14.7, 3.9],            // return, %
  size: [10, 18, 24, 8],               // diameter, CSS px
  value: [0.4, -0.2, 1.1, -0.6],       // e.g. Sharpe ratio, colored
  label: ['Bonds', 'Gold', 'Tech', 'Cash'],
});
plot.axis('x').applyOptions({ title: 'Volatility %' });
plot.axis('right').applyOptions({ title: 'Return %', includeZero: true });
```

### 单元格（热力图）

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const names = ['SPX', 'NDX', 'GLD', 'TLT'];
const corr = [
  [1, 0.92, 0.05, -0.31],
  [0.92, 1, 0.02, -0.35],
  [0.05, 0.02, 1, 0.21],
  [-0.31, -0.35, 0.21, 1],
];

for (const name of ['x', 'right'] as const) {
  const a = plot.axis(name);
  a.applyOptions({ type: 'category' });
  a.setCategories(names);
}

const x: number[] = [], y: number[] = [], value: number[] = [];
corr.forEach((row, i) => row.forEach((v, j) => { x.push(j); y.push(i); value.push(v); }));

const cells = plot.addMark('cells', {
  colorScale: { center: 0, domain: [-1, 1] },
  showValues: true,
  valueFormat: { type: 'fixed', decimals: 2 },
});
cells.setData({ x, y, value });
```

在数值坐标轴上，单元格的宽和高为 `cellSize` 个数据单位，或者等于不同 `x` 值之间以及不同 `y` 值之间的最小间距。

## 坐标轴

`plot.axis(name)` 返回 `'x'`、`'left'` 或 `'right'` 坐标轴。左侧坐标轴默认隐藏；图元通过 `yAxis: 'left'` 使用它。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('left').applyOptions({ visible: true, title: 'Volume', format: 'compact' });
plot.addMark('bars', { yAxis: 'left', color: 'rgba(120, 120, 120, 0.5)' });
plot.addMark('line', { yAxis: 'right' });
```

### `AxisOptions`

`axis.applyOptions(options)` 修改选项；省略的键保持原值。
`axis.options()` 返回所有选项。

| 选项 | 含义 | 默认值 |
|---|---|---|
| `type` | `'linear'`、`'log'` 或 `'category'` | `'linear'` |
| `range` | `'auto'` 表示跟随数据，或固定的 `[lo, hi]` | `'auto'` |
| `includeZero` | 自动范围：始终包含 0 | `false` |
| `margin` | 自动范围：两端各增加的比例，0 到 1 | `0.05`（分类：`0`） |
| `format` | 刻度标签的书写方式（见下文） | `'auto'` |
| `inverted` | 数值向右（x）或向上（y）递减 | `false` |
| `visible` | 绘制刻度、标签和轴线 | `true`（左侧：`false`） |
| `grid` | 在刻度处绘制网格线（分类坐标轴：在位置边界处） | `true`（分类：`false`） |
| `title` | 坐标轴旁的文字；`null` 表示移除 | `null` |
| `locked` | 平移和缩放不影响该坐标轴 | `false` |

修改 `type` 时，`grid`、`margin` 和
`range` 会从该类型的默认值重新开始；`visible`、`title`、`format`、`inverted` 和 `locked` 保持不变。

自动范围覆盖该坐标轴上各图元的可见数据。对于柱、面积和瀑布图，位于零的一端不加边距，因此柱子立在坐标轴上。分类坐标轴覆盖所有分类。

错误：固定范围必须是两个不同的有限数，在对数坐标轴上还必须为正数。

### 对数坐标轴

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('right').applyOptions({ type: 'log' });
```

小于或等于 0 的值在对数坐标轴上没有位置，不会被绘制。

### 分类坐标轴

分类坐标轴由等宽的位置组成；分类 `k` 覆盖 `[k, k + 1)`，其中心为 `k + 0.5`。其上的数据是从 0 开始的整数。用 `axis.setCategories(labels)` 为分类命名（最多 1 000 000 个）。

### 日期坐标轴

日期坐标轴是数值为 UTC 毫秒、采用 `'date'` 格式的线性坐标轴：

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('x').applyOptions({ format: 'date' });                              // UTC
plot.axis('x').applyOptions({ format: { type: 'date', utcOffsetMinutes: 60 } }); // UTC+1
```

### 格式

`PlotFormat` 用于 `AxisOptions.format` 和 `MarkOptions.valueFormat`：

| 格式 | 输出 |
|---|---|
| `'auto'` | 至少 2 位小数和 4 位有效数字，较小的值用下标表示零的个数（与图表的价格标签相同） |
| `'compact'` | K、M、B 后缀 |
| `{ type: 'fixed', decimals }` | 固定小数位数，0 到 12 |
| `{ type: 'percent', decimals }` | 固定小数位数并加 `%` 号；数值本身已是百分数（`5` 写作 `5%`） |
| `'date'` | 数值为 UTC 毫秒，以 UTC 日期和时间书写 |
| `{ type: 'date', utcOffsetMinutes }` | 同上，但相对 UTC 有偏移，−1440 到 1440 分钟 |

### 可见范围

| 方法 | 说明 |
|---|---|
| `axis.view(): [number, number]` | 截至上一帧的可见范围 |
| `axis.setView(lo, hi)` | 显示 `lo..hi`（在分类坐标轴上以分类为单位）；在调用 `plot.fit()` 之前，该坐标轴不再跟随数据。不发送 `viewChange` 事件 |
| `plot.fit()` | 所有坐标轴恢复到配置的范围 |

## 参考线

参考线是在某个值处横贯绘图的线，或两个值之间带阴影的区间。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const target = plot.addRule({ value: 100, label: 'target' });          // horizontal line on the right axis
plot.addRule({ axis: 'x', value: Date.UTC(2024, 5, 1), label: 'FOMC' }); // vertical line
plot.addRule({ value: 30, to: 70, color: 'rgba(120, 120, 255, 0.12)' }); // band

target.applyOptions({ value: 105 });
target.remove();
```

| 选项 | 含义 | 默认值 |
|---|---|---|
| `axis` | `value` 所在的坐标轴：`'x'` 得到垂直参考线 | `'right'` |
| `value` | 位置 | `0` |
| `to` | 区间的另一侧边缘；`null` 表示线 | `null` |
| `color` | 颜色；`null` 表示主题的十字光标颜色（区间为 12% 不透明度） | `null` |
| `lineWidth` | CSS px | `1` |
| `lineStyle` | `'solid'`、`'dashed'`、`'dotted'` | `'dashed'` |
| `label` | 坐标轴上该值处的文字；`null` 表示不显示 | `null` |
| `labelBackground` | 标签放在参考线颜色的底框上（否则以参考线颜色书写） | `true` |

## 标签

标签是位于数据坐标某个点处的文本、符号或两者兼有。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const peak = plot.addLabel({ x: 3, y: 14.7, text: 'peak', icon: 'arrowDown', align: 'center', offset: [0, -14] });
peak.remove();
```

| 选项 | 含义 | 默认值 |
|---|---|---|
| `x`、`y` | 该点（必填） | |
| `yAxis` | `'left'` 或 `'right'` | `'right'` |
| `text` | 文本（有图标时可以为空） | `''` |
| `icon` | `'circle'`、`'square'`、`'arrowUp'`、`'arrowDown'` 或 `null` | `null` |
| `color` | `null` 表示主题的文字颜色 | `null` |
| `align` | 文本相对于该点的对齐方式：`'left'`、`'center'`、`'right'` | `'left'` |
| `offset` | 文本相对于该点的偏移，单位 CSS px，向右和向下为正 | `[6, 0]` |

标签不能修改；请移除后重新添加。

## 颜色映射

单元格始终按颜色映射为其 `value` 列着色。柱和散点图元在有 `value` 列时也这样做；否则使用 `color`。`color` 列中某个点自己的颜色优先于其值。

| `colorScale` | 含义 |
|---|---|
| `null` 或 `'sequential'` | 从深蓝到黄色，由低到高 |
| `'diverging'` | 主题的下跌色、中性的中间色和上涨色，以 0 为中心 |
| `{ stops?, center?, domain?, quantile? }` | 自定义色阶 |

对象形式：

| 键 | 含义 |
|---|---|
| `stops` | `[position, color][]`，位置从 0 到 1 升序排列（至少一个色标） |
| `center` | 使色阶围绕该值发散：色阶的中点位于 `center`，离它较远的一侧决定两侧的范围 |
| `domain` | 固定的 `[lo, hi]`；超出范围的值取两端的颜色 |
| `quantile` | 未设置 `domain` 时：色阶覆盖从 `quantile` 分位数到 `1 − quantile` 分位数的值，使离群值饱和。取值 0 到 0.5，默认 0.01 |

未设置 `stops` 时，带 `center` 的色阶使用发散色，不带的使用顺序色。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const heat = plot.addMark('cells', {
  colorScale: { stops: [[0, '#0d1b4c'], [0.5, '#219ebc'], [1, '#f4e04d']], quantile: 0.05 },
});
```

值域由 `setData` 和 `append` 根据数据计算；`update` 会保持不变。

## 命中测试与事件

绘图会找出指针下的元素：折线或散点的一个点、一根柱子或一个单元格。它会高亮该元素（`hoverHighlight`），绘制穿过它的十字光标线（`crosshair`），并报告它。

```ts
interface PlotEventMap {
  hover: { hit: PlotHit | null };                 // the hovered element changed
  click: { hit: PlotHit | null; x: number; y: number };
  viewChange: {};                                  // an axis range changed by pan, zoom or fit()
}

interface PlotHit {
  mark: Mark;      // compare with mark.id: it is a new handle each time
  index: number;   // index of the point in the mark's data
  x: number;
  y: number;       // waterfall subtotals and totals: the running total
  position: { x: number; y: number }; // where it is drawn, CSS px from the canvas' top-left
}
```

在 `click` 中，`x` 和 `y` 是指针处的坐标轴数值（`y` 取右侧坐标轴的值，右侧坐标轴隐藏时取左侧坐标轴的值）。设置了
`hoverable: false` 的图元永远不会被命中。

```ts
import { FerroPlot, type PlotHit } from '@ferrochart/core';

declare const plot: FerroPlot;
declare const tooltip: HTMLElement;

const bars = plot.addMark('bars', { showValues: true });
bars.setData({ x: [0, 1, 2], y: [5, 6, 7] });

plot.on('hover', ({ hit }: { hit: PlotHit | null }) => {
  if (!hit || hit.mark.id !== bars.id) {
    tooltip.hidden = true;
    return;
  }
  tooltip.hidden = false;
  tooltip.textContent = `#${hit.index}: ${hit.y}`;
  tooltip.style.transform = `translate(${hit.position.x}px, ${hit.position.y}px)`;
});

plot.on('click', ({ hit, x, y }: { hit: PlotHit | null; x: number; y: number }) => {
  console.log(hit ? `clicked point ${hit.index}` : `empty space at ${x}, ${y}`);
});
```

事件会在产生它们的那一帧之后派发。

## 让绘图与图表窗格对齐

`chart.attachSidePlot(plot, pane?)` 把绘图放在图表窗格旁边，使同一价格在两者中处于相同高度。每一帧，图表都会把绘图的右侧坐标轴设置为窗格在绘图区顶部和底部所映射的价格。canvas 在页面上的位置每帧都会读取，因此绘图可以放在图表旁边的任何位置，其绘图区也可以比窗格高或矮。

- 绘图的右侧坐标轴被锁定：图表是它唯一的来源。如果附加时窗格的价格轴是对数坐标，右侧坐标轴也会变为对数坐标轴。
- 绘图的标志会被移除；图表的标志代表两者。
- `chart.detachSidePlot(plot)` 停止跟随；右侧坐标轴保持锁定，并保留最后的范围。

### 示例：成交量分布图

成交量分布图以 K 线旁的水平柱显示可见 K 线中每个价位的成交量。

```html
<div style="display: flex; height: 520px">
  <canvas id="chart" style="flex: 1; min-width: 0"></canvas>
  <canvas id="profile" style="width: 160px"></canvas>
</div>
```

```ts
import { FerroChart, FerroPlot, randomWalk, type OhlcData } from '@ferrochart/core';

const chart = await FerroChart.create(document.getElementById('chart') as HTMLCanvasElement, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
const bars: OhlcData = randomWalk(3, 3_000, Date.UTC(2024, 0, 1), 60_000);
candles.setData(bars);
chart.addVolumeSeries(candles);

const profile = await FerroPlot.create(document.getElementById('profile') as HTMLCanvasElement, {
  theme: 'dark',
  hoverHighlight: false,
});
profile.axis('x').applyOptions({ visible: false, grid: false });
profile.axis('right').applyOptions({ visible: false, grid: false }); // the chart shows the prices
const rows = profile.addMark('bars', { orientation: 'horizontal', color: 'rgba(41, 98, 255, 0.5)', band: 0.9 });
chart.attachSidePlot(profile);

/** Volume per price row of the bars `from..=to` (indices into `bars`). */
function volumeProfile(from: number, to: number, rowSize: number) {
  const byRow = new Map<number, number>();
  for (let i = from; i <= to; i++) {
    const row = Math.floor(bars.close[i] / rowSize);
    byRow.set(row, (byRow.get(row) ?? 0) + (bars.volume?.[i] ?? 0));
  }
  const y: number[] = [], x: number[] = [];
  for (const [row, volume] of byRow) {
    y.push((row + 0.5) * rowSize); // price at the middle of the row
    x.push(volume);
  }
  return { x, y };
}

const n = bars.close.length;
const update = (from: number, to: number) => {
  const a = Math.max(0, Math.floor(from));
  const b = Math.min(n - 1, Math.ceil(to));
  if (a <= b) rows.setData(volumeProfile(a, b, 0.5));
};
update(n - 200, n - 1);
chart.on('visibleRangeChanged', ({ from, to }: { from: number; to: number }) => update(from, to));
```

数值坐标轴上的水平柱，其粗细为 `band` 乘以各 `y` 值之间的最小间距，因此在 `band: 1` 时，大小相等的行会彼此相接。
用户滚动或缩放图表时，`visibleRangeChanged` 会针对可见 K 线重新计算分布图，价格范围则会自动跟随。要在 K 线旁显示订单簿深度，可以使用同样的设置，把买卖挂单量作为 `x`、价格作为 `y`，并用 `rows.update` 处理最后一档的实时变化。

## 清理

`plot.destroy()` 移除绘图的监听器和标志，并释放其 GPU 资源；侧边绘图也会停止跟随其图表。此后 `Mark`、`Axis`、
`Rule` 和 `Label` 句柄都会抛出异常（`the FerroPlot was destroyed`）。canvas 元素仍保留在页面中。
