<!-- source: 80fd5ef585 -->
# 时间轴与视口

时间轴决定哪些 K 线可见以及它们有多宽。它由所有窗格共用。本章介绍 K 线间距、右侧留白、可见范围、像素/逻辑索引/时间之间的转换，以及报告视图变化的事件。

## 逻辑索引

时间轴以逻辑索引而非时间工作。`setData` 之后，第一根 K 线的索引为 0，下一根为 1，依此类推；K 线 `i` 以 `i` 为中心，覆盖 `i − 0.5` 到 `i + 0.5`。K 线之间以及数据范围之外的位置，对应带小数或超出范围的索引。用 `timeAtIndex` 和 `indexOfTime` 在索引与时间之间转换（见[数据](data.md#reading-data-back)）。

索引计的是 K 线根数而不是时间：没有 K 线的周末或停牌时段不占空间。

## K 线间距

K 线间距是两根相邻 K 线中心之间的距离，单位为 CSS 像素。缩放会改变它。

```ts
console.log(chart.barSpacing); // 6 on a new chart
chart.setBarSpacing(12);        // zoom in
```

| | 值 |
|---|---|
| 默认 | 6 CSS px |
| 最小 | 0.5 CSS px；为了一次显示全部数据，必要时可以更小（绘图区宽度的 90 % 除以 K 线数量） |
| 最大 | `maxBarSpacing`，默认 50 CSS px |

`setBarSpacing` 会把值限制在此范围内。非有限正数的值会被忽略。右边缘处的 K 线位置保持不变。

最大间距在创建图表时设置：

```ts
const chart = await FerroChart.create(canvas, { maxBarSpacing: 200 });
```

`maxBarSpacing` 必须是不小于 1 的数，否则 `create` 会 reject。当单根 K 线需要更多空间时请调大它：足迹图的数字需要 60 px 以上，TPO 字母需要几百 px（见[订单流](orderflow.md)）。

## 右侧留白

右侧留白是最后一根 K 线与绘图区右边缘之间的空白，以 K 线根数计。它可以是小数，也可以是负数（此时最后一根 K 线位于右边缘之外，查看较早数据的视图就是这样描述的）。

```ts
console.log(chart.rightOffset); // 3 on a new chart
chart.setRightOffset(10);        // more room for the forming bar
chart.scrollToRealtime();        // back to the latest bar, with the default offset of 3
```

`setRightOffset` 会保证屏幕上至少有一根 K 线；超出范围的值会被限制，非有限值会被忽略。

视图通过右侧留白锚定在最新一根 K 线上。当 `update` 或 `setData` 在末尾添加 K 线时，右侧留白保持不变，因此每新增一根 K 线，内容就向左移动一根。用户查看最新数据时，新 K 线会滚入视图；当用户已经往回滚动时，他们正在看的 K 线也会向左移动。`scrollToRealtime()` 会把最新一根 K 线带回来。

## 可见范围

`visibleLogicalRange()` 返回绘图区左右边缘处的逻辑索引，为带小数的数值：

```ts
const { from, to } = chart.visibleLogicalRange();
const visibleBars = to - from;
```

`setVisibleLogicalRange(from, to)` 在间距限制允许的范围内，让绘图区恰好显示该范围（如果不允许，则保持右边缘）：

```ts
// The last 100 bars.
const data = chart.dataRange();
if (data) chart.setVisibleLogicalRange(data.to - 99.5, data.to + 0.5);
```

`from` 必须小于 `to`，且两者都是有限值；否则会抛出异常，视图保持不变。你可以在 `create` 之后、第一帧之前立即调用它：一旦知道绘图区宽度就会应用该范围。在此之前进行平移或缩放会取消它。

`fitContent()` 从左边缘到右边缘显示所有 K 线，不留右侧空白。K 线很少时，受最大 K 线间距限制，左侧可能会留有空白。与 `setVisibleLogicalRange` 一样，它会等待绘图区宽度确定。

`dataRange()` 返回第一根和最后一根 K 线的索引（没有数据时为 `null`），用于计算相对于数据的范围：

```ts
const data = chart.dataRange();
if (data) {
  // The first 200 bars.
  chart.setVisibleLogicalRange(data.from - 0.5, data.from + 199.5);
}
```

### 按时间指定范围

用 `indexOfTime` 把时间转换为索引。它要求存在时间恰好相等的 K 线；要为任意时间找到最近的 K 线，请在你自己保存的时间副本中查找。

```ts
function showTimes(fromMs: number, toMs: number) {
  const a = chart.indexOfTime(fromMs);
  const b = chart.indexOfTime(toMs);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a - 0.5, b + 0.5);
}
```

## 像素、索引与时间

| 方法 | 转换 |
|---|---|
| `logicalToCoordinate(index)` | 逻辑索引 → K 线中心的 x，距 canvas 左边缘的 CSS px |
| `coordinateToLogical(x)` | x（距 canvas 左边缘的 CSS px）→ 逻辑索引，带小数 |
| `timeAtIndex(index)` | 逻辑索引 → 该 K 线的时间（UTC ms），超出数据范围时为 `null`；四舍五入到最近的 K 线 |
| `indexOfTime(time)` | 时间 → 时间恰好相等的 K 线的逻辑索引，没有则为 `null` |

坐标基于上一帧的布局，因此请在第一帧之后使用它们（例如在事件处理函数中）。x 坐标在每个窗格中都相同。

```ts
// The bar under a point of the canvas.
canvas.addEventListener('dblclick', (e) => {
  const rect = canvas.getBoundingClientRect();
  const index = Math.round(chart.coordinateToLogical(e.clientX - rect.left));
  const time = chart.timeAtIndex(index);
  if (time !== null) console.log(new Date(time).toISOString(), candles.dataAt(index));
});

// Position an HTML element over a bar.
function placeOverBar(el: HTMLElement, time: number) {
  const index = chart.indexOfTime(time);
  if (index === null) return;
  el.style.left = `${chart.logicalToCoordinate(index)}px`;
}
```

对于十字光标和点击，事件本身已经携带逻辑索引、时间和价格；见[十字光标与事件](crosshair-events.md)。

## visibleRangeChanged 事件

每当可见逻辑范围发生变化时都会发送 `visibleRangeChanged`：无论是手势、上述某个方法、调整尺寸，还是使视图移动的数据导致的。

```ts
chart.on('visibleRangeChanged', ({ from, to }) => {
  const a = chart.timeAtIndex(Math.ceil(from));
  const b = chart.timeAtIndex(Math.floor(to));
  label.textContent = a !== null && b !== null
    ? `${new Date(a).toISOString()} – ${new Date(b).toISOString()}`
    : '';
});
```

事件在产生它们的那一帧之后投递，每帧最多一次：快速平移时你得到的是最新的范围，而不是每个中间范围。图表没有数据时不会发送任何事件。用 `chart.off('visibleRangeChanged', callback)` 移除监听器。

当视图距第一根 K 线不足一屏时，同一帧还可能发送 `needMoreHistory`；见[加载更早的历史数据](data.md#loading-older-history)。

### 重新加载时保持视图

用 `prepend` 加载更早的 K 线会保持所有逻辑索引和视图不变。`setData` 会从 0 重新给 K 线编号，所以逻辑索引会变，而时间不会。要在替换数据后恢复视图（例如换成时间戳相同的另一个品种），请把视图保存为时间：

```ts
function saveView() {
  const { from, to } = chart.visibleLogicalRange();
  return { from: chart.timeAtIndex(from), to: chart.timeAtIndex(to) };
}

function restoreView(view: { from: number | null; to: number | null }) {
  if (view.from === null || view.to === null) return chart.fitContent();
  const a = chart.indexOfTime(view.from);
  const b = chart.indexOfTime(view.to);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a, b);
}
```

要把 K 线间距和右侧留白与画线、窗格布局一起保存，请使用 `saveState`（见[保存状态](state.md)）。要让多个图表保持相同范围，见[同步图表](sync.md)。

## 用户手势

以下输入会改变时间轴（完整列表见[手势与键盘](interaction.md)）：

| 输入 | 效果 |
|---|---|
| 在绘图区内拖动 | 平移（移动中松开：惯性滚动） |
| 滚轮 / 触控板滚动 | 以指针为中心缩放时间 |
| 水平滚动或 Shift + 滚轮 | 平移 |
| 捏合（触控板、双指） | 缩放 |
| 拖动时间轴 | 改变 K 线间距 |
| 双击时间轴 | 重置 K 线间距 |
| ← → | 平移 10 % |
| ↑ ↓ | 缩放 |
| Home / End | 跳到第一根 / 最新一根 K 线 |

缩放时指针下方的 K 线位置保持不变。每次变化都会发送 `visibleRangeChanged`。
