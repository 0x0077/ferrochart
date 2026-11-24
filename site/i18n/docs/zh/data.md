<!-- source: 8ace0a0ae7 -->
# 数据

本章介绍如何把 K 线和数值放进系列：数据格式、校验规则、实时更新、加载更早的历史数据，以及读回数据。

## 列与行

`setData` 会替换系列的全部数据。它接受两种形式：列对象（每个字段一个数组）或行对象数组。

### 列

OHLC 系列（K 线、OHLC 柱、Heikin-Ashi 以及各种变换）接受 `OhlcData`：

| 列 | 类型 | 说明 |
|---|---|---|
| `time` | `BigInt64Array`、`Float64Array` 或 `number[]` | K 线开盘时间，UTC 毫秒，严格递增 |
| `open`, `high`, `low`, `close` | `Float64Array` 或 `number[]` | |
| `volume` | `Float64Array` 或 `number[]` | 可选 |

数值系列（折线、面积、基线、柱状图、散点）接受 `ValueData`：

| 列 | 类型 | 说明 |
|---|---|---|
| `time` | `BigInt64Array`、`Float64Array` 或 `number[]` | UTC 毫秒，严格递增 |
| `value` | `Float64Array` 或 `number[]` | `NaN` 表示缺口 |
| `color` | `(string \| null)[]` 或 `Uint32Array` | 可选的逐点颜色，仅柱状图会使用（见[系列类型](series.md#histogram)） |

所有列的长度必须相同。

```ts
candles.setData({
  time: new BigInt64Array([1704067200000n, 1704067260000n, 1704067320000n]),
  open: new Float64Array([100.0, 100.4, 100.1]),
  high: new Float64Array([100.6, 100.5, 100.9]),
  low: new Float64Array([99.8, 99.9, 100.0]),
  close: new Float64Array([100.4, 100.1, 100.8]),
  volume: new Float64Array([1200, 900, 1500]),
});

line.setData({ time: [1704067200000, 1704067260000], value: [100.4, 100.1] });
```

### 行

也可以传入对象数组：OHLC 系列用 `OhlcRow`（`{ time, open, high, low,
close, volume? }`），数值系列用 `ValueRow`（`{ time, value?, color? }`）。在行中，`time` 也可以是 `Date`。

```ts
candles.setData([
  { time: new Date('2024-01-01T00:00:00Z'), open: 100.0, high: 100.6, low: 99.8, close: 100.4 },
  { time: new Date('2024-01-01T00:01:00Z'), open: 100.4, high: 100.5, low: 99.9, close: 100.1 },
]);

line.setData([
  { time: 1704067200000, value: 100.4 },
  { time: 1704067260000 },                 // no value: a gap
  { time: 1704067320000, value: 100.8 },
]);
```

没有 `value`（或值为 `NaN`）的行表示缺口。缺少 OHLC 字段会抛出指明该字段的错误（`data[3] has no 'open'`）。如果部分行有 `volume` 而其他行没有，缺失的成交量按 0 处理。

### 如何选择

行在进行任何处理之前都会先转换成列，所以两种形式结果相同。如果你的数据本来就是对象（JSON API、几百根 K 线），用行写代码更简单。大数据量请用列：类型化数组每列只需一次 `memcpy` 就能复制进引擎，无需逐根读取对象，因此对一百万根 K 线调用 `setData` 只需几十毫秒。如果你收到的是二进制数据（例如来自 WebSocket 或文件），直接构建类型化数组，完全跳过对象。

### 时间

时间是自 Unix 纪元起的 UTC 毫秒数，为整数。

- `BigInt64Array` 时间按原样使用。
- `Float64Array` 和 `number[]` 时间会转换为整数；非有限值、带小数部分的值或超出 ±9·10¹⁵ 的值会抛出异常（`time[5] is not an integer millisecond timestamp`）。
- 在行和 `update` 中，`time` 可以是 number、bigint 或 `Date`。

如果数据源使用秒，请乘以 1000。

图表以 UTC 显示时间。

## 校验

`setData` 和 `update` 会先检查数据。如有问题会抛出异常，系列保留原来的数据。

| 规则 | 错误信息 |
|---|---|
| 时间严格递增（不能重复） | `time is not strictly increasing at index 7` |
| 所有列与 `time` 等长 | ``column `open` has a different length than `time` `` |
| 价格和成交量为有限值（缺口 K 线除外） | `invalid OHLC values in bar 12` |
| `high` ≥ max(`open`, `close`) 且 `low` ≤ min(`open`, `close`) | `invalid OHLC values in bar 12` |
| 列是数字数组或类型化数组 | `column 'high' must be a Float64Array or an array of numbers` |

信息中的索引是在你传入的数据中的位置。

四个价格全为 `NaN` 的 K 线是缺口 K 线（见[缺口](#gaps)）；它是合法的，其成交量会被设为 0。

## 缺口

- 在数值系列中，`NaN` 值（或没有 `value` 的行）表示缺口。折线、面积和基线在此处断开；柱状图和散点在此处不绘制任何内容。
- 在 OHLC 系列中，开盘价、最高价、最低价、收盘价均为 `NaN` 的 K 线是缺口，不绘制任何内容。

对于缺口，`dataAt` 返回 `null`。

### 多个系列共用一条时间轴

图表中的所有系列共用一条时间轴：逻辑索引 42 在每个系列中都对应同一时间。当各系列的时间不同时，时间轴是它们时间的并集。某个系列在只有其他系列才有的时间点上没有数据点：OHLC 系列、柱状图和散点在那里不显示任何内容；折线、面积和基线则连接相邻的数据点。如果新时间落在时间轴已有的 K 线之间，时间轴会被重建，图表会发送 `warning` 事件。

## K 线质量标记

在 K 线、OHLC 柱或 Heikin-Ashi 系列上使用 `setBarFlags`，描述应用补齐或裁剪过的数据。标记不会改变价格；请先传入处理好的 OHLC 数据：

```ts
const start = Date.UTC(2024, 0, 1);
candles.setData([
  { time: start, open: 100, high: 102, low: 99, close: 101 },
  { time: start + 60_000, open: 101, high: 101, low: 101, close: 101 },
  { time: start + 120_000, open: 101, high: 104, low: 100, close: 103 },
]);
candles.applyOptions({ filledBarColor: '#78909c' });
candles.setBarFlags([
  { time: start + 60_000, filled: true },
  { time: start + 120_000, clipped: true },
]);
// Clear all quality flags when they are no longer needed:
// candles.setBarFlags([]);
```

- `filled` 标识数据源在没有真实成交时合成的 K 线。它以 `filledBarColor` 绘制为空心（默认使用系列主色，不透明度为 40%）。真正缺失的 K 线也可表示为[缺口](#gaps)；此标记不会自动补齐缺失数据。
- `clipped` 标识已裁剪异常影线的 K 线。影线两端各显示一条短刻度线。请在应用中按时间保存原始最高价和最低价，并在提示框中展示；图表只有你传入的裁剪后数值。同一根 K 线可以同时设置这两个标记。
- 每次调用都会替换全部标记；`[]` 清空标记。时间为 UTC 毫秒（也可使用 `Date`/`bigint`，与数据相同）；时间重复时最后一项生效。缩小时若多根 K 线合并，仅当所有原始 K 线都为 filled 时，合并后的 K 线才为 filled；只要任意一根为 clipped，合并后的 K 线就为 clipped。

Python 使用 `candles.set_bar_flags(times, filled=..., clipped=...)`，布尔数组的长度需与 `times` 相同，并使用 `candles.apply_options(filled_bar_color="#78909c")`。参见 [Python](python.md#options-indicators-and-drawings)和[系列选项](series.md#series-options)。

## 实时更新

`update` 修改最后一根 K 线或添加一根新 K 线：

- 时间与最后一根相同：替换它（正在形成的 K 线）。
- 时间更晚：追加一根新 K 线。
- 时间更早：抛出异常。

```ts
// OHLC series
candles.update({ time: 1704067380000, open: 100.8, high: 101.2, low: 100.7, close: 101.0, volume: 300 });
// Value series
line.update({ time: 1704067380000, value: 101.0 });
```

K 线的校验方式与 `setData` 的数据相同。替换最后一根 K 线不会分配内存，且图表每个动画帧最多重绘一次，因此每笔成交都调用 `update` 也没问题。系列上的指标会增量更新。成交量系列跟随其源系列，自身没有 `update`。

对于变换系列（Renko、Kagi 等），每次 `update` 是一根输入 K 线或一笔 tick，绘制的元素随之变化；见[K 线变换](series.md#bar-transforms)。

### 批量追加

要在末尾添加多根 K 线（例如重连后从行情源收到的一批），用列或行（`setData` 接受的任意形式）调用 `append`。时间必须晚于最后一根 K 线，并且当且仅当系列有成交量列时才提供成交量列；否则会抛出异常，数据保持不变。

```ts
candles.append([
  { time: 1704067440000, open: 101.0, high: 101.4, low: 100.9, close: 101.3, volume: 250 },
  { time: 1704067500000, open: 101.3, high: 101.5, low: 101.1, close: 101.2, volume: 180 },
]);
```

正在形成的 K 线用 `update`，一起到达的已完成 K 线用 `append`。

## 加载更早的历史数据

`prepend` 在第一根 K 线之前插入更早的 K 线，接受 `setData` 支持的任意形式。已有的 K 线保持其逻辑索引不变；新 K 线得到负索引（索引 0 之前的那根是 -1）。视图不会移动，系列上的指标会重新计算。时间必须早于第一根 K 线；成交量规则与 `append` 相同。

图表会告诉你何时加载：当视图左边缘之前剩余的 K 线不足一屏宽度时，会发送 `needMoreHistory` 事件。`barsBeforeStart` 是剩余的 K 线数量（如果数据之前的空白区域已经可见，则为负数）。每个第一根 K 线只发送一次：`prepend` 之后第一根 K 线换成了新的，所以当用户滚动到它附近时事件会再次到来。

### 完整示例

```ts
import { FerroChart, type OhlcRow } from '@ferrochart/core';

// Your backend: up to `count` bars that end before `beforeMs`, oldest first.
declare function fetchBars(symbol: string, beforeMs: number, count: number): Promise<OhlcRow[]>;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const latest = await fetchBars('BTCUSD', Date.now(), 1000);
candles.setData(latest);
let first = Number(latest[0]?.time ?? Date.now());
let loading = false;
let exhausted = false;

chart.on('needMoreHistory', async () => {
  if (loading || exhausted) return;
  loading = true;
  try {
    // Only bars before the first one: an overlapping bar would make prepend throw.
    const older = (await fetchBars('BTCUSD', first, 1000)).filter((b) => Number(b.time) < first);
    if (older.length === 0) {
      exhausted = true; // the backend has nothing older
      return;
    }
    candles.prepend(older);
    first = Number(older[0].time);
  } finally {
    loading = false;
  }
});
```

`loading` 标志用于防止你自己的代码发出重叠的请求（对同一个第一根 K 线，事件本身不会重复）。与此同时，实时 K 线照常通过 `update` 进入：`prepend` 和 `update` 作用于两端。

## 读回数据

| 方法 | 返回值 |
|---|---|
| `series.dataAt(index)` | OHLC 系列返回 `{ open, high, low, close, volume }`（没有成交量列时 `volume` 为 `null`），数值系列返回 `{ value }`；缺口或超出数据范围时返回 `null` |
| `chart.timeAtIndex(index)` | 某逻辑索引处 K 线的时间（UTC ms），超出数据范围时为 `null` |
| `chart.indexOfTime(time)` | 时间恰好等于该值的 K 线的逻辑索引，没有则为 `null` |
| `chart.dataRange()` | `{ from, to }`：第一根和最后一根 K 线的逻辑索引；没有数据时为 `null` |

带小数的索引会四舍五入到最近的 K 线。

```ts
const range = chart.dataRange();
if (range) {
  const last = candles.dataAt(range.to);              // the forming bar
  const t = chart.timeAtIndex(range.to);              // its time
  console.log(t, last);
}

const i = chart.indexOfTime(Date.UTC(2024, 0, 2, 9, 30));
if (i !== null) console.log(candles.dataAt(i));
```

对于设置了 `shift` 的系列，`dataAt(index)` 返回绘制在 `index` 处的数据点。

## 逻辑索引

每根 K 线都有一个逻辑索引。可见范围、十字光标位置和点击都以逻辑索引报告，`setVisibleLogicalRange` 和 `logicalToCoordinate` 接受的也是逻辑索引；见[时间轴与视口](time-scale.md)。所有系列共用时间轴，因此索引 `i` 在每个系列中都对应同一时间。

`setData` 之后第一根 K 线的索引为 0。在 K 线之间的位置有意义的地方（视图边缘、指针），索引带小数：K 线 `i` 覆盖 `i - 0.5` 到 `i + 0.5`。

## 细节层级

K 线按列存储：一个时间数组、一个开盘价数组，依此类推。这使得加载速度快、内存占用小：每根 K 线约 58 字节，包括时间索引和细节层级金字塔。

当你缩小到多根 K 线共用一个像素时，FerroChart 会从预先计算好的金字塔中绘制聚合后的 K 线（每一层数量减半，保留真实的最高价和最低价）。因此无论数据量多大，每帧绘制的元素数量都受屏幕宽度限制——大约每个物理像素四个。

所有数据变更在内部都是增量的：渲染器会被告知哪个范围发生了变化，并且只上传这部分。

## 成交量

成交量系列读取某个 OHLC 系列的成交量列：`chart.addVolumeSeries(candles)`。它没有自己的数据；请把 `volume` 传给 K 线系列的 `setData` 和 `update`。见[系列类型](series.md#volume)。

## 示例：模拟的实时行情

一个带 500 根历史 K 线的图表，每 250 ms 产生一个新价格。每当时钟跨过整分钟，就开始一根新的一分钟 K 线。

```ts
import { FerroChart, randomWalk, type Bar } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const interval = 60_000;
const now = Date.now();
const start = Math.floor(now / interval) * interval - 500 * interval;
const history = randomWalk(42, 500, start, interval); // deterministic demo data
candles.setData(history);

const n = history.close.length;
let bar: Bar = {
  time: Number(history.time[n - 1]),
  open: history.open[n - 1],
  high: history.high[n - 1],
  low: history.low[n - 1],
  close: history.close[n - 1],
  volume: history.volume?.[n - 1] ?? 0,
};

const timer = setInterval(() => {
  const t = Math.floor(Date.now() / interval) * interval;
  const price = bar.close * (1 + (Math.random() - 0.5) * 0.002);
  const size = Math.round(Math.random() * 10);
  if (t > Number(bar.time)) {
    // A new minute: open a new bar at the last close.
    bar = { time: t, open: bar.close, high: Math.max(bar.close, price), low: Math.min(bar.close, price), close: price, volume: size };
  } else {
    bar = { ...bar, high: Math.max(bar.high, price), low: Math.min(bar.low, price), close: price, volume: (bar.volume ?? 0) + size };
  }
  candles.update(bar); // same time: replaces, later time: appends
}, 250);

// When the chart goes away:
// clearInterval(timer); chart.destroy();
```

视图会与最新一根 K 线保持固定距离（右侧留白），因此无论用户正在看最新数据还是已经往回滚动，每追加一根 K 线内容都会向左滚动一根。`chart.scrollToRealtime()` 会把最新一根 K 线带回视图中（见[时间轴与视口](time-scale.md#right-offset)）。
