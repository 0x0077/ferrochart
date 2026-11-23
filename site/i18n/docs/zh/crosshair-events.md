<!-- source: 5f948529f4 -->
# 十字光标与事件

图表通过事件告诉你它上面发生了什么：十字光标移动、点击、可见范围变化、画线被创建或编辑，等等。本章列出所有事件、它们的载荷和触发时机，并演示两个常见用法：跟随十字光标的图例，以及响应对标记和系列的点击。

## 十字光标模式

十字光标在绘图区内跟随指针。它的竖线贯穿所有窗格；横线和价格标签属于指针所在的窗格。时间标签显示在时间轴上。

| 模式 | 行为 |
|---|---|
| `'normal'`（默认） | x 吸附到最近的 K 线中心，y 跟随指针 |
| `'magnet'` | 与 `'normal'` 相同，另外 y 吸附到该 K 线（窗格中第一个价格系列）的开盘价、最高价、最低价、收盘价中最近的一个 |
| `'hidden'` | 不绘制十字光标，`crosshairMove` 报告 `null` |

```ts
chart.setCrosshairMode('magnet');
console.log(chart.crosshairMode); // 'magnet'
```

传入未知模式会抛出异常。模式是保存状态的一部分（`saveState().crosshairMode`，见[保存状态](state.md)）。

放置画线时同样适用磁吸模式：锚点会吸附到最近的开盘价、最高价、最低价或收盘价（见[画线工具](drawings.md)）。

## 订阅

```ts
import type { ChartEventMap } from '@ferrochart/core';

const onMove = (e: ChartEventMap['crosshairMove']) => {
  console.log(e.point?.time);
};

chart.on('crosshairMove', onMove);
// later
chart.off('crosshairMove', onMove);
```

- `on(name, callback)` 添加监听器。未知的事件名会抛出异常。
- `off(name, callback)` 移除监听器。要传入与传给 `on` 时相同的函数对象。
- 同一个函数添加两次会被调用两次；`off` 会把两个都移除。
- 回调中可以调用任何图表方法，包括 `on` 和 `off`。
- 回调内抛出的异常按未捕获错误处理（`reportError`：会出现在控制台并到达 `window.onerror`）。其他回调照常执行。

回调参数的类型由事件名推断，因此 `chart.on('crosshairMove', (e) => e.point?.time)` 不需要类型标注。像上面那样单独声明回调时，可以用 `ChartEventMap[name]`（由包导出）表示载荷类型。

## 投递时机

图表在 `requestAnimationFrame` 中绘制。每一帧先应用自上一帧以来的输入和 API 调用，构建并渲染该帧，然后投递这一帧产生的事件。由此可知：

- 事件不会在 DOM 输入处理函数内部、也不会在引发它的方法调用内部投递。`chart.drawings.add(...)` 会立即返回 id；对应的 `drawingCreated` 事件在下一帧之后才到达。
- 每次最多投递一帧的事件。如果自上一帧以来十字光标移动了多次，你只会收到一个带最新位置的 `crosshairMove`，而不是每个指针事件一个。`visibleRangeChanged` 和 `sync` 同理。
- 浏览器会在后台标签页中暂停 `requestAnimationFrame`，因此事件会延迟到标签页重新可见时才投递。

## 生命周期

图表依靠其 canvas 和 window 上的监听器保持运行，这些监听器只持有对它的弱引用。只要图表还在页面上，就要保留对 `FerroChart` 对象的引用。如果该对象被垃圾回收，图表会停止绘制，也不再发送事件。用完图表后调用 `destroy()`：它会移除所有事件监听器（你的和 DOM 上的）并释放 GPU 资源。此后该对象不可再使用。

```ts
chart.destroy();
```

## 事件参考

| 事件 | 载荷 | 触发时机 |
|---|---|---|
| `click` | `ClickEvent` | 在绘图区内按下并松开，期间移动不超过 3 CSS px |
| `crosshairMove` | `{ point: CrosshairPoint \| null }` | 十字光标移到另一根 K 线、另一个窗格或另一个价格，或离开绘图区 |
| `visibleRangeChanged` | `{ from, to }` | 可见逻辑范围发生变化 |
| `needMoreHistory` | `{ barsBeforeStart }` | 视图左边缘距第一根 K 线不足一屏 |
| `drawingCreated` | `{ id }` | 创建了一个画线 |
| `drawingModified` | `{ id }` | 画线被移动、修改样式或修改了参数 |
| `drawingRemoved` | `{ id }` | 移除了一个画线 |
| `drawingSelected` | `{ id: number \| null }` | 选中的画线发生变化 |
| `sync` | `{ message: SyncMessage }` | 图表向其同步组发出了一条消息 |
| `drawingSync` | `{}` | 图表有待取走的画线同步操作 |
| `warning` | `ChartWarning` | 图表自行调整了某些内容 |
| `markerHover` | `MarkerHoverEvent \| null` | 指针移到某个标记或“+k”徽标上，或从其上移开 |

### click

```ts
interface ClickEvent {
  x: number;              // CSS px from the canvas' top-left corner
  y: number;
  pane: number;           // pane id
  logicalIndex: number;   // fractional logical index at x
  time: number | null;    // UTC ms of the nearest bar, null beyond the data
  price: number;          // price at y on the pane's price scale
  target: ClickTarget | null;
}

type ClickTarget =
  | { type: 'drawing'; id: number }
  | { type: 'marker'; series: Series; index: number }
  | { type: 'markerOverflow'; series: Series; first: number; count: number }
  | { type: 'series'; series: Series };
```

点击是指在绘图区内的一次按下和松开（鼠标、触控笔或触摸），期间指针移动不超过 3 CSS px；移动更多则视为平移。正在创建画线时不会发送点击事件。

`target` 是指针下方的对象，按以下顺序检查：

1. **画线**（其主体或某个控制点）：`{ type: 'drawing', id }`。按下画线也会选中它。
2. **标记**：`{ type: 'marker', series, index }`，其中 `index` 是该标记在 `series.markers()`（按时间排序）中的位置。被折叠标记的“+k”徽标给出 `{ type: 'markerOverflow', series, first, count }`：未绘制的标记是 `series.markers().slice(first, first + count)`（见[标记与价格线](markers.md)）。
3. **系列**：当指针位于 K 线实体或影线上、距线条 4 CSS px 以内、在柱状图的柱内或在圆点上时，为 `{ type: 'series', series }`。系列重叠时，最后添加的那个胜出。
4. 空白处为 `null`。

target 中的 `Series` 对象是指向已有系列的新句柄。请用 `id` 比较它们，不要用 `===`：

```ts
chart.on('click', (e: ChartEventMap['click']) => {
  if (e.target?.type === 'series' && e.target.series.id === candles.id) {
    console.log('clicked the candles at', e.time);
  }
});
```

指标输出就是普通的系列，因此点击指标线会得到一个 `series` target，其 `id` 与 `indicator.outputs()` 中的某一个相同。

### crosshairMove

```ts
interface CrosshairPoint {
  logicalIndex: number;   // the bar the crosshair is on (whole number)
  time: number | null;    // UTC ms, null beyond the data
  pane: number;           // pane under the pointer
  price: number;          // price at the crosshair; NaN for a synced crosshair
  bar: BarValues | null;  // OHLCV of the first OHLC series with a bar here
  x?: number;             // pointer position, CSS px; absent for a synced crosshair
  y?: number;
}

interface BarValues {
  open: number; high: number; low: number; close: number;
  volume: number | null;
}
```

指针离开绘图区或十字光标被隐藏时，`point` 为 `null`。只有当 point 中的某项发生变化时才发送该事件；在同一根 K 线和同一价格内移动指针不会重复发送。

在磁吸模式下，`price` 是吸附后的价格。当该索引处没有任何 OHLC 系列的 K 线时（例如只有折线系列的图表，或数据缺口），`bar` 为 `null`。要读取十字光标处其他系列的数据，请使用 `series.dataAt(point.logicalIndex)`。

处于[同步组](sync.md)中的图表也会报告其他图表的十字光标。这样的 point 中 `price: NaN`，`pane` 设为主窗格，并且没有 `x` 或 `y`。

### visibleRangeChanged

`{ from: number, to: number }` 是绘图区左右边缘处的逻辑索引，带小数。平移、缩放、调整尺寸以及改变视图的方法（`setVisibleLogicalRange`、`fitContent`、`scrollToRealtime`、`setBarSpacing`、`setRightOffset`）都会触发它。同样的值随时可以通过 `chart.visibleLogicalRange()` 获取。

### needMoreHistory

`{ barsBeforeStart: number }` 是第一根 K 线与视图左边缘之间的 K 线数量。如果视图已经显示了数据之前的空白区域，它为负数。每个第一根 K 线只发送一次该事件：添加更早的数据后，当用户滚动到新的开头时，它可以再次触发。

```ts
let loading = false;
chart.on('needMoreHistory', async (e: ChartEventMap['needMoreHistory']) => {
  if (loading) return;
  loading = true;
  try {
    await loadOlderBars(e.barsBeforeStart); // your code: fetch older bars, then series.prepend(them)
  } finally {
    loading = false;
  }
});
```

加载数据见[数据](data.md)。

### 画线事件

`drawingCreated`、`drawingModified` 和 `drawingRemoved` 携带画线的 `id`；`drawingSelected` 携带选中的 id 或 `null`。

| 事件 | 触发原因 |
|---|---|
| `drawingCreated` | 用户完成一个画线、`drawings.add`、使画线恢复的撤销或重做（沿用其原 id）、`loadState`（每个载入的画线一次）、从同步组收到的画线 |
| `drawingModified` | 拖动画线或其控制点、`setAnchors`、`setStyle`、`setParam`、对此类修改的撤销或重做、同步过来的修改 |
| `drawingRemoved` | Delete 键、`drawings.remove`、撤销一次创建、`loadState`（针对被它替换的画线）、移除窗格、同步过来的移除 |
| `drawingSelected` | 按下画线或空白处、完成一个新画线（它会被选中）、`drawings.select`、隐藏或移除选中的画线 |

显示、隐藏、锁定画线以及改变其 z 顺序都不会发送事件。见[画线工具](drawings.md)。

### sync

当图表向其[同步组](sync.md)发出十字光标或可见范围消息时，发送 `{ message: SyncMessage }`：

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }
  | { type: 'visibleRange'; from: number; to: number };
```

同一页面上同一个 `SyncGroup` 中的图表无需你做任何事。该事件用于把消息转发给其他地方的图表（例如 worker 中的图表），由它们通过 `applySync(message)` 应用。不在任何组中的图表，或其所在组不共享该类消息的图表，不会发送 `sync` 事件。

### drawingSync

`{}`。当图表在同步组中共享画线、且有页面尚未取走的画线操作时发送。它只发送一次；只有在你调用 `chart.takeDrawingSyncOps()` 之后才会再次发送。见[同步多个图表](sync.md)。

### warning

图表自行修正了某些内容，引发它的调用已成功。

```ts
type ChartWarning =
  | { kind: 'timeAxisMerged'; series: Series; count: number }
  | { kind: 'ohlcRepaired'; series: Series; count: number }
  | { kind: 'dataDropped'; series: Series }
  | { kind: 'other' };
```

| `kind` | 含义 |
|---|---|
| `'timeAxisMerged'` | `series` 的时间落在时间轴的 K 线之间。时间轴已按所有系列时间的并集重建，第一根插入的 K 线之后的逻辑索引发生了移动。共插入了 `count` 根 K 线。 |
| `'ohlcRepaired'` | `series` 中有 `count` 根 K 线的最高价低于、或最低价高于其他价格，已被扩展。（JavaScript API 会直接拒绝这样的 K 线，因此由 JavaScript 驱动的图表不会发送它。） |
| `'dataDropped'` | `series` 的数据无法迁移到重建后的时间轴上（内存不足），已被清空。 |
| `'other'` | 后续版本新增的类型。 |

```ts
chart.on('warning', (w) => {
  if (w.kind === 'timeAxisMerged') console.warn(`time axis rebuilt: ${w.count} bars inserted by series ${w.series.id}`);
});
```

### markerHover

```ts
interface MarkerHoverEvent {
  series: Series;
  index: number;          // position in series.markers(); for a badge, the first marker not drawn
  overflow: number;       // k for a "+k" badge, 0 for a marker
  x: number;              // center, CSS px from the canvas' top-left corner
  y: number;
  size: number;           // diameter, CSS px
}
```

指针离开标记时，载荷为 `null`。从一个标记直接移到另一个标记时，只发送新的那个。指针下方的标记在绘制帧时确定，因此该事件每帧最多发送一次；不考虑画线，入场动画播放期间也不会有标记被视为悬停。可用于在头像或徽标上显示提示框（见[标记与价格线](markers.md)）。

## 示例：跟随十字光标的图例

图表上方的一个 DOM 元素显示十字光标所在的 K 线；指针在别处时显示最新一根 K 线。

```html
<div style="position: relative; height: 480px">
  <canvas id="chart" style="width: 100%; height: 100%"></canvas>
  <div id="legend" style="position: absolute; left: 12px; top: 8px;
       font: 12px sans-serif; pointer-events: none"></div>
</div>
```

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const canvas = document.getElementById('chart') as HTMLCanvasElement;
const legend = document.getElementById('legend') as HTMLDivElement;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars); // your OHLC columns
const sma = chart.addIndicator(candles, 'sma', { n: 20 }).outputs()[0];

const fmt = (v: number | undefined) => (v === undefined || Number.isNaN(v) ? '–' : v.toFixed(2));

function show(index: number) {
  const bar = candles.dataAt(index);
  const avg = sma.dataAt(index);
  if (!bar) {
    legend.textContent = '';
    return;
  }
  const time = chart.timeAtIndex(index);
  const date = time === null ? '' : new Date(time).toISOString().slice(0, 16).replace('T', ' ');
  legend.textContent =
    `${date}  O ${fmt(bar.open)}  H ${fmt(bar.high)}  L ${fmt(bar.low)}  C ${fmt(bar.close)}` +
    `  SMA 20 ${fmt(avg?.value)}`;
}

function showLatest() {
  const range = chart.dataRange();
  if (range) show(range.to);
}

chart.on('crosshairMove', (e: ChartEventMap['crosshairMove']) => {
  if (e.point) show(e.point.logicalIndex);
  else showLatest();
});
showLatest();
```

`dataAt` 对 OHLC 系列返回 `{ open, high, low, close, volume }`，对其他系列返回 `{ value }`，没有数据或处于缺口时返回 `null`。指标输出在预热期内的 K 线上为 `NaN`。如果实时更新改变了最新一根 K 线且指针不在图表上，请在 `update` 之后调用 `showLatest()`。

## 示例：点击标记和系列

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars);

candles.setMarkers([
  { time: bars.time[120], position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: bars.time[180], position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
]);

chart.on('click', (e: ChartEventMap['click']) => {
  const t = e.target;
  if (!t) {
    console.log(`empty space at ${e.price.toFixed(2)}`);
    return;
  }
  switch (t.type) {
    case 'marker': {
      const marker = t.series.markers()[t.index];
      console.log(`marker "${marker.text}" at`, new Date(Number(marker.time)));
      break;
    }
    case 'series':
      if (t.series.id === candles.id) {
        const bar = candles.dataAt(Math.round(e.logicalIndex));
        console.log('candle', bar);
      }
      break;
    case 'drawing':
      console.log('drawing', t.id, chart.drawings.get(t.id)?.kind);
      break;
  }
});
```

标记索引对应的是 `series.markers()` 返回的按时间排序的列表，而不是你传给 `setMarkers` 的顺序。如果之后需要再次找到对应数据，请自行以时间为键保存。见[标记与价格线](markers.md)。
