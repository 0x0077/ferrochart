<!-- source: 37271b3d45 -->
# 同步多个图表

加入同一个 `SyncGroup` 的图表会相互跟随：在其中一个上移动十字光标，其他图表也会显示；平移或缩放其中一个，其他图表会滚动到相同的时间范围。还可以选择让在一个图表上画的线同时出现在其他图表上。

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const left: HTMLCanvasElement;
declare const right: HTMLCanvasElement;

const a = await FerroChart.create(left);
const b = await FerroChart.create(right);

const group = new SyncGroup();
a.joinSync(group);
b.joinSync(group);
```

`new SyncGroup()` 需要 WebAssembly 模块。该模块由第一次 `FerroChart.create` 加载；如果要在创建任何图表之前创建组，请先调用 `await init()`（`import init from '@ferrochart/core'`）。

## 按时间，而不是按 K 线序号

图表之间的每条消息携带的都是**时间**（UTC ms），而不是逻辑索引。因此，同一组中的图表可以显示不同的品种、不同的周期，或者缺口位置不同的数据：

- 1 分钟图上 10:37 处的十字光标，在 1 小时图上显示在 10:00 那根 K 线上（该时间点或之前的最后一根 K 线）。超出数据两端时，时间会外推。
- 平移或缩放之后，其他图表会在其 K 线间距限制允许的范围内，从左边缘到右边缘显示相同的时间跨度（保持右边缘不变）。一个跟随显示两小时的 1 分钟图的 1 小时图，需要用两根 K 线占满整个绘图区；在默认最大 K 线间距 50 px 下，它会显示更多 K 线，结束于同一时间。

没有数据的图表会忽略收到的范围。

## 加入与离开

### `chart.joinSync(group, options?)`

加入 `group`；如果图表已在某个组中，会先离开那个组。在下一帧，图表会接管该组当前的十字光标和可见范围。

`SyncOptions` 选择图表共享哪些内容。每个选项都是双向的：设置了 `crosshair: false` 的图表既不显示其他图表的十字光标，也不发送自己的十字光标。

| 选项 | 共享内容 | 默认值 |
|---|---|---|
| `crosshair` | 十字光标所在时间 | `true` |
| `visibleRange` | 可见时间范围（平移和缩放） | `true` |
| `drawings` | 主窗格中的画线 | `false` |

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const group: SyncGroup;
declare const overview: FerroChart;
declare const detail: FerroChart;

// The overview shows the crosshair of the others but keeps its own zoom.
overview.joinSync(group, { visibleRange: false });
detail.joinSync(group);
```

要修改某个图表的选项，用同一个组再次调用 `joinSync`。

### `chart.leaveSync()`

离开组。该图表在其他图表上显示的十字光标会被移除。`chart.destroy()` 也会离开组。

### `chart.syncOptions()`

图表共享的内容，所有选项都已填充；不在任何组中时为 `null`。

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const opts = chart.syncOptions();
if (opts === null) console.log('not synced');
else console.log(opts.crosshair, opts.visibleRange, opts.drawings);
```

## 发送什么，何时发送

- **十字光标。** 指针在图表绘图区上移动时，图表发送十字光标所在的时间；指针离开时，发送十字光标已消失。其他图表在该时间显示十字光标。此时它们的 `crosshairMove` 事件中的 `point` 带有 `price: NaN`，没有 `x`/`y`，且 `pane` 设为主窗格。
- **可见范围。** 当视图因用户操作（拖动、滚轮、双指缩放、按键）或你的代码（`setVisibleLogicalRange`、`setBarSpacing`、`fitContent` 等）而改变时，图表发送其左右边缘处的时间。如果图表在同一帧中自己改变了视图，该改变优先于收到的范围。
- 应用收到的消息不会再次发送它，因此消息不会在图表之间来回反弹。

发送消息的图表会为组内其他图表安排一帧；它们在绘制该帧时应用消息。

## 同步画线

设置 `drawings: true` 后，图表**主窗格**上的画线会复制到其他共享画线的图表的主窗格：包括添加、移动、修改样式、显示、隐藏、锁定和移除。由于锚点是价格，请将其用于同一品种的图表（例如多个周期）。

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const group: SyncGroup;
declare const m5: FerroChart;
declare const h1: FerroChart;

m5.joinSync(group, { drawings: true });
h1.joinSync(group, { drawings: true });

// Appears on h1 at its next frame.
m5.drawings.add({
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 2), price: 42_000 }],
});
```

细节：

- 以 `drawings: true` 加入的图表会先接管组中已有的画线，然后再共享自己的画线。
- 其他窗格上的画线不共享（每个图表的其他窗格可能显示不同的指标）。收到的画线总是放到主窗格。
- 收到的修改不会加入接收方图表的撤销历史。在做出修改的图表上执行的撤销和重做会像其他修改一样被发送，因此所有图表最终保持一致。
- 选中状态和正在创建中的画线不共享。
- 每个图表都必须认识该画线类型。用 JavaScript 编写的画线工具（`chart.drawings.register`）对页面上的所有图表只需注册一次。

## 不共享同一个组对象的图表

`SyncGroup` 连接的是同一页面上由同一个 WebAssembly 模块实例创建的图表。其他地方的图表，例如另一个窗口、标签页或 iframe 中的图表，或者原生应用中的图表，无法加入它。对于这些图表，图表会报告它发送的内容，由你通过任意传输方式（`postMessage`、`BroadcastChannel`、WebSocket）转发。

发送方图表必须在某个组中（只有一个成员的组也可以）：只有在组中的图表才会报告它的消息。

### 十字光标和可见范围

`sync` 事件会投递该图表发送给其组的每条消息。在另一端，由 `chart.applySync(message)` 应用它。

```ts
import { FerroChart, SyncGroup, type SyncMessage } from '@ferrochart/core';

declare const chart: FerroChart;

const channel = new BroadcastChannel('chart-sync');
const group = new SyncGroup();
chart.joinSync(group);

// Out: what this chart sent.
chart.on('sync', ({ message }: { message: SyncMessage }) => {
  channel.postMessage(message);
});

// In: what a chart in another window sent.
channel.onmessage = (e: MessageEvent<SyncMessage>) => {
  chart.applySync(e.data);
};
```

`SyncMessage` 是普通数据：

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }       // null: the crosshair left
  | { type: 'visibleRange'; from: number; to: number }; // times at the plot edges, UTC ms
```

`applySync` 只影响调用它的那个图表；它既不会被再次发送，也不会传给该图表自己所在的组。在接收端，请对每个需要跟随的图表调用它。

`applySync({ type: 'visibleRange', from, to })` 也是让单个图表显示某个时间范围的便捷方法。

### 画线

画线的变更以 `DrawingSyncOp` 值传递。每个画线都有一个 `key`，用于在不同图表之间标识它：

```ts
type DrawingSyncOp =
  | { type: 'add'; key: number; data: DrawingData; visible: boolean; locked: boolean }
  | { type: 'update'; key: number; data: DrawingData }
  | { type: 'setVisible'; key: number; visible: boolean }
  | { type: 'setLocked'; key: number; locked: boolean }
  | { type: 'remove'; key: number };
```

| 成员 | 说明 |
|---|---|
| `drawingSync` 事件 | 图表发送了画线操作。只发送一次；下一次要在你调用 `takeDrawingSyncOps()` 之后才会到来 |
| `takeDrawingSyncOps()` | 已发送但尚未取走的操作，按从旧到新排列。最多保留 1024 个；更早的会被丢弃 |
| `applyDrawingSync(op)` | 应用一个来自其他图表的操作（作用于主窗格，不可撤销，不会再次发送） |
| `drawingSyncState()` | 以 `'add'` 操作形式给出该图表的所有已同步画线，供稍后加入的图表使用 |
| `loadDrawingSyncState(ops)` | 使已同步画线与这样的列表一致：添加、更新，并移除不在列表中的已同步画线 |

key 由组分配：组的 `origin` 乘以 2³²，再加上一个计数器。当多个页面相互转发画线时，用 `new SyncGroup(origin)` 为每个页面的组指定各自的 origin，以免不同页面的 key 冲突。origin 是 0（默认值）到 2²¹ − 1 之间的整数；更大的值会抛出异常，这样可以保证 key 作为 JavaScript number 时是精确的。

```ts
import { FerroChart, SyncGroup, type DrawingSyncOp } from '@ferrochart/core';

declare const chart: FerroChart;
declare const pageNumber: number; // different on every page, 0 to 2^21 - 1

type Msg =
  | { type: 'ops'; ops: DrawingSyncOp[] }
  | { type: 'hello' }
  | { type: 'state'; ops: DrawingSyncOp[] };

const channel = new BroadcastChannel('chart-drawings');
const group = new SyncGroup(pageNumber);
chart.joinSync(group, { drawings: true });

const send = (m: Msg) => channel.postMessage(m);

// Out: forward this chart's changes.
chart.on('drawingSync', () => send({ type: 'ops', ops: chart.takeDrawingSyncOps() }));

channel.onmessage = (e: MessageEvent<Msg>) => {
  const m = e.data;
  if (m.type === 'ops') {
    for (const op of m.ops) {
      try {
        chart.applyDrawingSync(op);
      } catch (err) {
        console.warn('drawing op skipped:', err);
      }
    }
  } else if (m.type === 'hello') {
    // A page that just opened asks for the current drawings.
    send({ type: 'state', ops: chart.drawingSyncState() });
  } else {
    chart.loadDrawingSyncState(m.ops);
  }
};

send({ type: 'hello' });
```

错误：

- 对图表中不存在的 key 执行 `update`、`setVisible`、`setLocked` 或 `remove`，以及无法构建的画线（未注册的类型、错误的锚点或参数），`applyDrawingSync` 都会抛出异常。对图表中已有的 key 执行 `add` 会更新该画线。
- 如果某个操作不是 `'add'`、某个 key 出现了两次，或某个画线无法构建，`loadDrawingSyncState` 会抛出异常，图表保持不变。

## 示例：多周期布局

同一品种的四个图表，周期分别为 1 分钟、5 分钟、1 小时和 1 天。三个日内图表共享十字光标、缩放和画线。日线图显示其他图表的十字光标，但保持自己的视图，从而始终作为概览。

```html
<div class="grid">
  <canvas id="m1"></canvas><canvas id="m5"></canvas>
  <canvas id="h1"></canvas><canvas id="d1"></canvas>
</div>
<style>
  .grid { display: grid; grid-template: 1fr 1fr / 1fr 1fr; gap: 4px; height: 100vh; }
  .grid canvas { width: 100%; height: 100%; min-height: 0; }
</style>
```

```ts
import { FerroChart, SyncGroup, randomWalk, type SyncOptions } from '@ferrochart/core';

const MIN = 60_000;
const start = Date.UTC(2024, 0, 1);

const layout: { id: string; interval: number; sync: SyncOptions }[] = [
  { id: 'm1', interval: MIN, sync: { drawings: true } },
  { id: 'm5', interval: 5 * MIN, sync: { drawings: true } },
  { id: 'h1', interval: 60 * MIN, sync: { drawings: true } },
  { id: 'd1', interval: 1_440 * MIN, sync: { visibleRange: false } },
];

const group = new SyncGroup();
const charts: FerroChart[] = [];

for (const { id, interval, sync } of layout) {
  const canvas = document.getElementById(id) as HTMLCanvasElement;
  const chart = await FerroChart.create(canvas, { theme: 'dark' });
  const candles = chart.addCandlestickSeries();
  // Replace with your data; all four cover the same period.
  const bars = Math.round((90 * 1_440 * MIN) / interval);
  candles.setData(randomWalk(7, bars, start, interval));
  chart.addVolumeSeries(candles);
  chart.joinSync(group, sync);
  charts.push(chart);
}

// Show the last two days of the 1-minute chart; m5 and h1 follow it.
const range = charts[0].dataRange();
if (range) charts[0].setVisibleLogicalRange(range.to - 2 * 1_440, range.to);
charts[3].fitContent();
```

本示例中的数据是随机的，所以四个图表显示的价格并不相同；使用同一品种的真实数据时它们会对齐。

关于这个布局：

- 对 `charts[0]` 调用 `setVisibleLogicalRange` 会像用户平移一样改变它的视图，因此该变化会发送给组，`m5` 和 `h1` 随之跟随。（`applySync` 只会改变 `charts[0]`：被应用的消息不会继续发送。）
- 日线图以 `visibleRange: false` 加入：它既不跟随其他图表，也不带动其他图表，但仍显示它们的十字光标。
- 已同步的图表在收到消息或自身内容变化时重绘；空闲的图表不会重绘。

## Rust

在 Rust 中，对应的功能是 `Chart::join_sync(&group, SyncOptions)`、`Chart::leave_sync`、`Chart::apply_sync`、`ChartEvent::Sync`、`Chart::take_drawing_sync_ops`、`Chart::apply_drawing_sync`、`Chart::drawing_sync_state` 和 `Chart::load_drawing_sync_state`，以及 `SyncGroup::new()` 和 `SyncGroup::with_origin(origin)`。
