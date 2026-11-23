<!-- source: 58a84755b2 -->
# 画线工具

画线是由用户或代码放置在图表上的对象：趋势线、矩形、斐波那契工具、多空仓位、形态、文本等等。每个画线对象的锚点都位于数据空间（时间和价格）中，因此在图表平移、缩放或改变尺寸时，它始终附着在 K 线上。创建、选中、拖动、删除和撤销/重做都由 FerroChart 自行处理；你的页面只需启用工具，并在需要时响应画线事件。

## Drawings 对象

所有画线方法都在 `chart.drawings` 上：

```ts
const drawings = chart.drawings;

drawings.startTool('trend_line');   // the next clicks draw a trend line
const id = drawings.add({           // or add one from code
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 2), price: 101.5 }],
});
drawings.setStyle(id, { line: '#ef5350', lineWidth: 2 });
drawings.undo();
```

画线以数字 id 标识。同一图表内 id 永不复用；通过撤销恢复的画线会取回原来的 id。

| 方法 / 属性 | 作用 |
|---|---|
| `kinds()` | `add` 和 `startTool` 接受的类型（29 个内置类型加上已注册的类型） |
| `startTool(kind)` / `cancelTool()` / `activeTool` | 交互式创建 |
| `add(data, pane?)` | 添加一个画线；返回其 id |
| `get(id)` | 该画线的 `DrawingData`（类型、锚点、样式、参数），未知 id 返回 `null` |
| `ids()` | 所有 id，从底层到顶层 |
| `paneOf(id)` | 画线所在窗格的 id |
| `setAnchors(id, anchors)` | 移动锚点 |
| `setStyle(id, style)` | 修改部分样式键 |
| `setParam(id, key, value)` | 设置工具参数（`text`、`levels`） |
| `select(id)` / `selected` | 选中 |
| `setVisible(id, visible)` / `isVisible(id)` | 显示或隐藏 |
| `setLocked(id, locked)` / `isLocked(id)` | 锁定以防拖动 |
| `bringToFront(id)` / `sendToBack(id)` | 叠放顺序 |
| `remove(id)` | 移除一个画线 |
| `undo()` / `redo()` / `canUndo` / `canRedo` / `clearHistory()` | 历史记录 |
| `register(kind, tool)` | 添加一个用 JavaScript 编写的画线工具 |

接收 id 的方法遇到未知 id 时会抛出异常（`get`、`paneOf`、
`isVisible` 和 `isLocked` 除外，它们返回 `null` 或 `undefined`）。画线会随图表状态一起保存和恢复（参见[保存状态](state.md)）。

## 内置类型

类型名称是 snake_case 字符串。

**线条**

| 类型 | 锚点 | 说明 |
|---|---|---|
| `trend_line` | 2 | 两个锚点之间的线段 |
| `ray` | 2 | 从第一个锚点出发，经过第二个锚点，延伸到绘图区边缘 |
| `extended_line` | 2 | 经过两个锚点，向两端延伸到绘图区边缘 |
| `horizontal_line` | 1 | 在锚点价格处横贯绘图区，右侧边缘带价格标签 |
| `horizontal_ray` | 1 | 从锚点延伸到右侧边缘，带价格标签 |
| `vertical_line` | 1 | 在锚点时间处纵贯绘图区 |
| `arrow` | 2 | 在第二个锚点处带箭头的线段 |

**图形与文本**

| 类型 | 锚点 | 说明 |
|---|---|---|
| `parallel_channel` | 3 | 经过前两个锚点的直线，以及经过第三个锚点的平行线；带填充和虚线中线 |
| `rectangle` | 2 | 两个对角；带填充 |
| `ellipse` | 2 | 内切于两个锚点构成的矩形；带填充 |
| `text` | 1 | 从锚点开始的文本。参数 `text`（默认 `'Text'`）。填充色不透明时，绘制在 `fill` 颜色的背景上 |
| `callout` | 2 | 位于第二个锚点的文本框，带一条指向第一个锚点的引线。参数 `text`（默认 `'Callout'`）。文本框以线条颜色填充，文字为白色 |
| `path` | 2 个或更多 | 折线；每个点单击一次，以双击或 Enter 结束 |
| `brush` | 2 个或更多 | 自由手绘线：按下、拖动、松开。没有控制柄；只能整体移动 |

**测量与仓位**

| 类型 | 锚点 | 说明 |
|---|---|---|
| `price_range` | 2 | 带阴影的区间，显示从第一个锚点到第二个锚点的价格变化和百分比变化 |
| `date_range` | 2 | 带阴影的区间，显示 K 线数量和时长 |
| `long_position` | 3（单击 2 次） | 入场（第一个锚点）、止盈（第二个）和止损（第三个）。创建时止损以入场价为中心与止盈对称。显示盈亏区域、带价格和百分比的标签以及盈亏比 |
| `short_position` | 3（单击 2 次） | 同 `long_position`，百分比按空头交易计算 |

**斐波那契与江恩**

| 类型 | 锚点 | 说明 |
|---|---|---|
| `fib_retracement` | 2 | 每个水平在 `p2 + (p1 − p2) · level` 处画一条水平线，标注为 `level (price)`，相邻水平间交替着色。参数 `levels`（默认 `[0, 0.236, 0.382, 0.5, 0.618, 0.786, 1]`） |
| `fib_extension` | 3 | 水平位于 `p3 + (p2 − p1) · level`，从第三个锚点开始绘制。参数 `levels`（默认 `[0, 0.382, 0.618, 1, 1.272, 1.618, 2, 2.618]`） |
| `fib_time_zones` | 2 | 对斐波那契数 F = 0, 1, 2, 3, 5, 8, …，在第 `i1 + (i2 − i1) · F` 根 K 线处画垂直线 |
| `gann_fan` | 2 | 从第一个锚点发出的射线，斜率为到第二个锚点连线斜率的 1/8 … 8 倍，标注为 `1x8` … `8x1` |
| `pitchfork` | 3 | 安德鲁音叉线：中线从第一个锚点出发，经过另外两个锚点的中点，两条平行叉齿分别经过这两个锚点；叉齿之间带填充 |
| `schiff_pitchfork` | 3 | 同 `pitchfork`，但起点在价格上向第二个锚点移动一半 |
| `modified_schiff_pitchfork` | 3 | 同 `pitchfork`，但起点位于前两个锚点的中点 |

**形态与波浪**

| 类型 | 锚点 | 说明 |
|---|---|---|
| `xabcd_pattern` | 5 | X、A、B、C、D 五个点，并标出各段之间的回撤比例；XAB 和 BCD 三角形带填充 |
| `head_and_shoulders` | 7 | 左肩、头部和右肩的标签，以及经过第三和第五个锚点的颈线；带填充 |
| `elliott_impulse` | 6 | 标注为 0 到 5 的波浪 |
| `elliott_correction` | 4 | 标注为 0、A、B、C 的波浪 |

`p1`、`p2`、`p3` 依次为各锚点的价格。形态和波浪的标签放在波峰上方、波谷下方。

## 交互式创建画线

```ts
chart.drawings.startTool('rectangle');
```

`startTool(kind)` 启用一个工具。光标在绘图区上方变为十字，下一次在任意窗格上按下时就在那里开始画线。未知类型会抛出异常。启用工具会取消正在进行的画线。

画线如何完成取决于类型：

- **固定数量的锚点**（大多数类型）：第一次按下放置第一个锚点；下一个锚点跟随指针移动，直到再次单击，依此类推，直到放置最后一个锚点。单锚点类型（`horizontal_line`、
  `horizontal_ray`、`vertical_line`、`text`）在第一次按下时即完成。
  前两个锚点也可以通过按下、拖动、松开来放置。
- **不定数量**（`path`）：每个锚点单击一次。双击或 Enter 结束绘制（双击产生的重复点会被丢弃）。结束时至少需要两个锚点。
- **自由手绘**（`brush`）：按下、拖动、松开。每移动 3 CSS px 添加一个锚点。少于两个锚点的笔画会被丢弃。

放置锚点时，x 会吸附到最近的 K 线（最后一根 K 线之后的时间按外推计算）；在磁吸十字光标模式下，y 会吸附到最近的开盘价、最高价、最低价或收盘价。自由手绘的锚点不吸附。

画线完成后会被选中，并发送 `drawingCreated` 事件，同时**工具会被停用**：一次 `startTool` 只创建一个画线。要连续绘制同一类型，请在 `drawingCreated` 监听器中再次启用它（参见下面的工具栏示例）。

| 按键 | 创建过程中 |
|---|---|
| Escape | 取消正在进行的画线（至少有两个锚点的 `path` 则会结束绘制），或停用已启用的工具 |
| Enter | 结束 `path` |

`cancelTool()` 停用工具并丢弃正在进行的画线，不发送事件。`activeTool` 是已启用的类型，或正在创建的画线的类型；其他情况下为 `undefined`。

创建画线期间不会发送 `click` 事件。

## 选中与编辑

在未启用工具时：

- **按下画线**会选中它（`drawingSelected`）。它的控制柄随即出现：大多数类型每个锚点一个；仓位在入场、止盈、止损和右边缘各有一个控制柄；水平线和垂直线在绘图区中间有一个；画笔没有控制柄。
- **拖动**画线会移动它：水平方向按整根 K 线移动，垂直方向跟随指针。拖动控制柄会移动对应的锚点。拖动结束时发送 `drawingModified`。拖动过程中按 Escape 会把画线放回原处。
- **按下空白处**会清除选中。
- 命中检测的优先顺序是：先是选中画线的控制柄，然后是选中的画线，再从顶层往下是其他画线。
- 隐藏的画线不会被命中。矩形、椭圆和通道在轮廓上命中；填充色不透明时，在内部任意位置都能命中。

在画线上按下不会触发平移。在锁定的画线上按下会选中它，然后平移图表。

### 键盘

canvas 获得焦点时接收键盘输入；按下它时就会获得焦点。

| 按键 | 效果 |
|---|---|
| Delete | 移除选中的画线（包括锁定的画线） |
| Escape | 取消创建、停用工具，或撤销当前的拖动 |
| Enter | 结束正在创建的 `path` |
| Ctrl+Z / ⌘+Z | 撤销 |
| Ctrl+Shift+Z / ⌘+Shift+Z、Ctrl+Y / ⌘+Y | 重做 |

Backspace 没有任何作用。要用其他按键删除，请自己在 canvas 上监听：

```ts
canvas.addEventListener('keydown', (e) => {
  const id = chart.drawings.selected;
  if (e.key === 'Backspace' && id != null) chart.drawings.remove(id);
});
```

## 通过代码添加画线

```ts
const id = chart.drawings.add({
  kind: 'fib_retracement',
  anchors: [
    { time: Date.UTC(2024, 0, 10), price: 120 },
    { time: Date.UTC(2024, 1, 20), price: 95 },
  ],
  style: { line: '#787b86', fill: 'rgba(120, 123, 134, 0.15)' },
  params: { levels: [0, 0.382, 0.5, 0.618, 1] },
});
```

```ts
interface DrawingData {
  kind: string;
  anchors: { time: number | bigint; price: number }[];
  style?: DrawingStyle;
  params?: Record<string, number | boolean | string | number[]>;
}
```

- `time` 是 UTC 毫秒（整数）。不必是某根 K 线的时间。
- `price` 使用窗格中第一个价格系列的单位，必须是有限值。
- 锚点数量必须与类型匹配：大多数类型与表中完全一致；仓位为 2 或 3 个（为 2 个时，止损按止盈的镜像添加）；`path` 和 `brush` 为 2 到 4096 个。
- `pane` 默认为主窗格。传入窗格 id 可在其他窗格上画线。
- 缺省的样式键取下文的默认值；类型不使用的参数键会被忽略。

遇到未知类型、锚点过少或过多、非有限价格、未知窗格、无效样式或无效参数时，`add` 会抛出异常，且不添加任何内容。通过代码添加的画线不会被选中。

`get(id)` 返回包含所有键的保存形式：`kind`、`anchors`、完整的
`style`，以及包含该类型参数的 `params`（`text` 和 `callout` 的 `text`，斐波那契回撤和扩展的 `levels`）。把它传给
`add` 即可复制一个画线：

```ts
const copy = chart.drawings.get(id);
if (copy) chart.drawings.add(copy, chart.drawings.paneOf(id));
```

`setAnchors(id, anchors)` 用于移动画线。传入的锚点数量必须与画线现有的相同（已完成的仓位为 3 个）。

## 样式

```ts
chart.drawings.setStyle(id, { line: '#26a69a', lineWidth: 2, lineDash: 'dashed' });
```

| 键 | 类型 | 默认值 | 用于 |
|---|---|---|---|
| `line` | `Color` | `#2962ff` | 线条和轮廓；区间和仓位的标签背景；标注框 |
| `lineWidth` | `number` | `1` | CSS px，大于 0 |
| `lineDash` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | 线条的虚线样式 |
| `fill` | `Color` | `#2962ff`，不透明度 16% | 区域：通道、矩形、椭圆、区间、斐波那契色带、音叉线、形态三角形、文本背景 |
| `text` | `Color` | `#2962ff` | 文本以及无背景的标签 |
| `fontSize` | `number` | `12` | CSS px，文本和标签 |

`setStyle` 只修改你传入的键。颜色不能为 `null`；不需要填充时使用
`'transparent'`。仓位的盈亏区域使用固定的绿色和红色。

## 参数

```ts
chart.drawings.setParam(textId, 'text', 'Breakout');
chart.drawings.setParam(fibId, 'levels', [0, 0.5, 1, 1.618]);
```

| 参数 | 类型 | 值 |
|---|---|---|
| `text` | `text`、`callout` | 字符串 |
| `levels` | `fib_retracement`、`fib_extension` | 最多 24 个有限数 |

类型错误的值会抛出异常，画线保持不变。样式和参数的修改会发送 `drawingModified`，并且可以撤销。

## 可见性、锁定与叠放顺序

```ts
chart.drawings.setVisible(id, false);   // hidden drawings are not drawn or hit
chart.drawings.setLocked(id, true);     // can be selected and deleted, not dragged
chart.drawings.bringToFront(id);
chart.drawings.sendToBack(id);
console.log(chart.drawings.isVisible(id), chart.drawings.isLocked(id), chart.drawings.ids());
```

隐藏选中的画线会清除选中。锁定不会阻止通过代码调用
`setAnchors`、`setStyle` 或 `remove`。可见性、锁定和叠放顺序不发送事件，也不计入撤销历史；它们会随图表状态一起保存。

## 选中

```ts
chart.drawings.select(id);      // selects it and shows its handles
chart.drawings.select(null);    // clears the selection
const sel = chart.drawings.selected; // the selected id, or undefined
```

## 移除

```ts
chart.drawings.remove(id);
for (const d of chart.drawings.ids()) chart.drawings.remove(d); // remove all
```

移除窗格会移除其上的画线；如果其上有画线，撤销历史会被清空。

## 撤销与重做

以下修改无论由用户还是代码发起都会被记录：添加、移除、移动（拖动、`setAnchors`），以及修改样式或参数。选中、可见性、锁定和叠放顺序不会被记录。

```ts
chart.drawings.undo();          // false if there was nothing to undo
chart.drawings.redo();
if (chart.drawings.canUndo) { /* enable the undo button */ }
chart.drawings.clearHistory();
```

历史记录保留最近 256 次修改。新的修改会清空重做列表。
`undo` 和 `redo` 会取消正在进行的画线。加载状态（`loadState`）会清空历史记录。从联动组接收到的画线不会被记录。

## 画线事件

| 事件 | 负载 | 触发时机 |
|---|---|---|
| `drawingCreated` | `{ id }` | 画线由用户完成、通过代码添加、经撤销/重做恢复或被加载 |
| `drawingModified` | `{ id }` | 画线被移动、修改样式或修改参数 |
| `drawingRemoved` | `{ id }` | 画线被移除 |
| `drawingSelected` | `{ id: number \| null }` | 选中状态改变 |

事件在发生变化的那一帧之后到达，而不是在方法调用内部。参见[十字光标与事件](crosshair-events.md)。

```ts
chart.on('drawingCreated', (e: ChartEventMap['drawingCreated']) => {
  const d = chart.drawings.get(e.id);
  console.log('new', d?.kind, d?.anchors);
});
```

## 示例：工具栏

```html
<div id="toolbar">
  <button data-kind="trend_line">Trend</button>
  <button data-kind="horizontal_line">Horizontal</button>
  <button data-kind="rectangle">Rectangle</button>
  <button data-kind="fib_retracement">Fib</button>
  <button data-kind="long_position">Long</button>
  <button data-kind="path">Path</button>
  <button data-kind="brush">Brush</button>
  <button data-kind="text">Text</button>
  <label><input id="sticky" type="checkbox"> Keep drawing</label>
  <button id="lock">Lock</button>
  <button id="delete">Delete</button>
  <button id="undo">Undo</button>
  <button id="redo">Redo</button>
</div>
<canvas id="chart" style="width: 100%; height: 480px"></canvas>
```

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const canvas = document.getElementById('chart') as HTMLCanvasElement;
const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars);

const d = chart.drawings;
const toolbar = document.getElementById('toolbar') as HTMLDivElement;
const sticky = document.getElementById('sticky') as HTMLInputElement;
const button = (id: string) => document.getElementById(id) as HTMLButtonElement;
let lastKind: string | null = null;

function refresh() {
  const active = d.activeTool ?? null;
  toolbar.querySelectorAll<HTMLButtonElement>('button[data-kind]').forEach((b) => {
    b.classList.toggle('active', b.dataset.kind === active);
  });
  const sel = d.selected;
  button('delete').disabled = sel == null;
  button('lock').disabled = sel == null;
  button('lock').textContent = sel != null && d.isLocked(sel) ? 'Unlock' : 'Lock';
  button('undo').disabled = !d.canUndo;
  button('redo').disabled = !d.canRedo;
}

toolbar.addEventListener('click', (e) => {
  const kind = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-kind]')?.dataset.kind;
  if (!kind) return;
  if (d.activeTool === kind) {
    d.cancelTool();
    lastKind = null;
  } else {
    d.startTool(kind);
    lastKind = kind;
  }
  canvas.focus(); // so that Escape and Enter reach the chart
  refresh();
});

button('delete').onclick = () => {
  const sel = d.selected;
  if (sel != null) d.remove(sel);
};
button('lock').onclick = () => {
  const sel = d.selected;
  if (sel != null) d.setLocked(sel, !d.isLocked(sel));
  refresh();
};
button('undo').onclick = () => d.undo();
button('redo').onclick = () => d.redo();

chart.on('drawingCreated', (_e: ChartEventMap['drawingCreated']) => {
  if (sticky.checked && lastKind && d.activeTool == null) d.startTool(lastKind);
  refresh();
});
chart.on('drawingModified', refresh);
chart.on('drawingRemoved', refresh);
chart.on('drawingSelected', refresh);
// Escape disarms a tool without an event.
canvas.addEventListener('keyup', refresh);
refresh();
```

通过按钮触发的撤销和重做会在下一帧之后发送 `drawingCreated`、
`drawingModified` 或 `drawingRemoved`，从而刷新按钮状态。

## 自定义工具

`chart.drawings.register(kind, tool)` 添加一个用 JavaScript 编写的画线工具。工具需说明其创建方式（`anchorCount`：点击次数、
`{ open: min }` 或 `'freehand'`），并从 `build(ctx)` 返回绘制命令；可选的 `hitTest` 决定按下时命中什么。该类型可用于此图表的 `add`、`startTool` 和 `loadState`；内置类型不能被替换。参见[插件](plugins.md#drawing-tools)。
