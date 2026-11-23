<!-- source: a58b5627d3 -->
# 实用配方

面向 FerroChart 应用开发的任务示例。每个浏览器示例都是独立的 TypeScript 模块，需通过浏览器打包工具构建，并安装 `@ferrochart/core`；React 示例还需安装 React。将已挂载且宽度非零的 `HTMLElement` 作为 `host` 传入，例如 `document.body`，并在视图移除时调用返回的清理函数。价格提醒示例则返回 `update` 和 `destroy` 方法。参见 [Web 配置](web.md)。

示例使用模拟数据。时间戳为 UTC 毫秒，K 线按时间升序排列。网络示例说明了自有行情源应提供的数据格式；库不附带行情服务。Python 示例注明了所需的包，并将 PNG 文件写入当前目录。

## 通过 WebSocket 推送实时 K 线

将图表连接到自己的 WebSocket 端点，每条文本消息发送一根 JSON 格式的 OHLC K 线。

```ts
import { FerroChart, type Bar } from '@ferrochart/core';

export async function mountLiveChart(host: HTMLElement, feedUrl: string) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const candles = chart.addCandlestickSeries();
  const socket = new WebSocket(feedUrl);
  const receive = (event: MessageEvent<string>) => {
    try {
      const bar = JSON.parse(event.data) as Bar;
      candles.update(bar);
    } catch (error) {
      console.error('Invalid or out-of-order bar', error);
    }
  };
  socket.addEventListener('message', receive);
  socket.addEventListener('error', () => console.error('WebSocket error'));
  return () => {
    socket.removeEventListener('message', receive);
    socket.close();
    chart.destroy();
    canvas.remove();
  };
}
```

- 行情源必须提供以 UTC 毫秒表示的 `time`，以及 `open`、`high`、`low`、`close`（`volume` 可选）；调用 `update` 前先转换数据提供方的消息格式。
- 相同的开盘时间会替换最后一根 K 线；更晚的时间会追加一根；更早的时间会抛出异常。每次更新都需提供当前 K 线的完整数据，而非单笔成交的增量。
- 卸载时调用返回的清理函数；连接恢复由行情适配层负责。参见[实时更新](data.md#real-time-updates)。

## 向左滚动时加载更早的历史数据

加载上一页历史数据，同时保持当前视图不变；这个独立示例使用内存中的历史数据代替后端。

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountHistory(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const archive = Array.from({ length: 120 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const initial = archive.slice(-40);
  const candles = chart.addCandlestickSeries();
  let oldest = initial[0].time;
  let loading = false, exhausted = false, disposed = false;
  const button = document.createElement('button');
  button.textContent = 'Load earlier / retry';
  host.append(button);
  async function loadBefore(before: number) {
    return archive.filter(bar => bar.time < before).slice(-40);
  }
  async function loadOlder() {
    if (loading || exhausted || disposed) return;
    loading = true;
    button.disabled = true;
    try {
      const older = await loadBefore(oldest);
      if (disposed) return;
      if (!older.length) { exhausted = true; return; }
      candles.prepend(older);
      oldest = older[0].time;
    } catch (error) {
      console.error('History load failed; use the retry button', error);
    } finally {
      loading = false;
      button.disabled = exhausted;
    }
  }
  chart.on('needMoreHistory', loadOlder);
  button.onclick = () => { void loadOlder(); };
  candles.setData(initial);
  chart.fitContent();
  return () => {
    disposed = true;
    chart.destroy();
    canvas.remove();
    button.remove();
  };
}
```

- 将 `loadBefore` 替换为后端请求：返回严格早于 `before`、时间戳唯一且按升序排列的数据。成交量列应与初始数据保持一致。
- 每当最早一根 K 线改变时，`needMoreHistory` 可触发一次。空响应会结束分页；请求失败后可通过按钮主动重试。
- `prepend` 会保留现有逻辑索引和视图；不要在每页加载后调用 `fitContent`。参见[加载更早的历史数据](data.md#loading-older-history)。

## 将指标放在独立窗格中

为同一 K 线系列添加叠加显示的移动平均线和独立的 RSI 窗格。

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountIndicators(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  chart.addIndicator(candles, 'ema', { n: 20 }, 'overlay');
  chart.addIndicator(candles, 'rsi', { n: 14 }, 'newPane');
  return () => { chart.destroy(); canvas.remove(); };
}
```

- 第四个参数指定放置位置：`'overlay'`、`'newPane'` 或已有窗格的 id。默认位置取决于指标。
- 指标随源数据更新，无需单独传入数据。请为所选周期提供足够的历史数据。参见[指标位置](indicators.md#placement)。

## 将买卖记录关联到标记

用户点击标记时，取回自己的成交对象。

```ts
import { FerroChart, type Marker } from '@ferrochart/core';

type Trade = { id: number; time: number; side: 'buy' | 'sell'; quantity: number };

export async function mountTradeMarkers(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  const trades: Trade[] = [
    { id: 1, time: bars[10].time, side: 'buy', quantity: 2 },
    { id: 2, time: bars[30].time, side: 'sell', quantity: 1 },
  ];
  const markers: (Marker & { trade: Trade })[] = trades.map(trade => ({
    id: trade.id, time: trade.time, trade,
    position: trade.side === 'buy' ? 'belowBar' : 'aboveBar',
    shape: trade.side === 'buy' ? 'arrowUp' : 'arrowDown',
    text: trade.side,
  }));
  candles.setMarkers(markers);
  const info = document.createElement('p');
  info.textContent = 'Click a buy or sell marker';
  host.append(info);
  chart.on('click', ({ target }) => {
    if (target?.type !== 'marker' || target.series.id !== candles.id) return;
    const marker = markers.find(candidate => candidate === target.marker);
    if (marker) info.textContent = `Trade ${target.id}: ${marker.trade.side} ${marker.trade.quantity}`;
  });
  return () => { chart.destroy(); canvas.remove(); info.remove(); };
}
```

- `id` 是自定义的非负安全整数标识符；`target.marker` 是传入 `setMarkers` 的对象，包含自定义的 `trade` 字段。
- 事件中的索引对应按时间排序后的标记，并非原数组顺序。折叠徽章的类型为 `'markerOverflow'`，可按需单独处理。
- `setMarkers` 会替换整个标记列表。参见[标记](markers.md#markers)和[点击事件](crosshair-events.md#click)。

## 绘制价格提醒线并检测穿越

显示价格阈值，在传入 K 线的收盘价向上穿越阈值时通知应用。

```ts
import { FerroChart, type Bar } from '@ferrochart/core';

export async function mountPriceAlert(host: HTMLElement, notify: (price: number) => void) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const candles = chart.addCandlestickSeries();
  const first = { time: Date.UTC(2025, 0, 1), open: 100, high: 102, low: 99, close: 101 };
  candles.setData([first]);
  const threshold = 105;
  candles.createPriceLine({ price: threshold, title: 'Alert', color: '#ef5350', lineStyle: 'dashed' });
  let previous = first.close;
  return {
    update(bar: Bar) {
      candles.update(bar);
      const crossed = previous < threshold && bar.close >= threshold;
      previous = bar.close;
      if (crossed) notify(bar.close);
    },
    destroy() { chart.destroy(); canvas.remove(); },
  };
}
```

- 通过返回的 `update` 方法传入行情 K 线；价格线本身不会监控价格或发送提醒。
- 这里比较相邻两次收盘价，包括尚未结束的 K 线更新。只有价格先跌破阈值、再向上穿越时才会再次触发；这不是后台服务。
- 卸载时调用 `destroy`。参见[价格线](markers.md#price-lines)和[实时更新](data.md#real-time-updates)。

## 联动两个图表

在两个图表间同步十字光标时间和可见时间范围。

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

export async function mountSyncedCharts(host: HTMLElement) {
  const canvases = [document.createElement('canvas'), document.createElement('canvas')];
  for (const canvas of canvases) {
    canvas.style.cssText = 'display:block;width:100%;height:260px';
    host.append(canvas);
  }
  const first = await FerroChart.create(canvases[0]);
  let second: FerroChart;
  try { second = await FerroChart.create(canvases[1]); }
  catch (error) { first.destroy(); canvases.forEach(canvas => canvas.remove()); throw error; }
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  first.addCandlestickSeries().setData(bars);
  second.addSeries('line').setData(bars.map(bar => ({ time: bar.time, value: bar.close })));
  first.fitContent();
  second.fitContent();
  const group = new SyncGroup();
  first.joinSync(group, { crosshair: true, visibleRange: true });
  second.joinSync(group, { crosshair: true, visibleRange: true });
  return () => {
    first.destroy(); second.destroy();
    group.free();
    canvases.forEach(canvas => canvas.remove());
  };
}
```

- 同步依据时间，而非 K 线索引；每个图表仍需自己的数据。K 线间距限制可能约束图表实际显示的范围。
- 销毁图表会使其退出同步组。先销毁组内图表，再释放组；绘图同步需单独设置，默认关闭。参见[图表联动](sync.md#time-not-bar-numbers)。

## 保存绘图和视口

将用户的视图和绘图保存在 localStorage 中，图表数据由应用自行重建。

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

export async function mountSavedChart(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  const key = 'ferrochart-cookbook-view';
  const status = document.createElement('p');
  const draw = document.createElement('button');
  draw.textContent = 'Draw trend line';
  draw.onclick = () => chart.drawings.startTool('trend_line');
  const save = document.createElement('button');
  save.textContent = 'Save view and drawings';
  save.onclick = () => {
    try {
      localStorage.setItem(key, JSON.stringify(chart.saveState()));
      status.textContent = 'Saved';
    } catch (error) { status.textContent = String(error); }
  };
  host.append(draw, save, status);
  try {
    const stored = localStorage.getItem(key);
    if (stored) chart.loadState(JSON.parse(stored) as ChartState);
  } catch (error) { status.textContent = `Could not restore: ${String(error)}`; }
  return () => { chart.destroy(); canvas.remove(); draw.remove(); save.remove(); status.remove(); };
}
```

- 调用 `loadState` 前，先重建数据、系列、指标及相同的窗格布局；状态不包含行情数据或系列定义。
- 恢复状态会替换绘图并清空撤销历史。为不兼容的布局或不同品种使用不同的存储键，并显示存储或恢复时的错误。
- 卸载前点击 Save。参见[状态内容](state.md#what-a-state-contains)和[加载状态](state.md#loading-a-state)。

## 在 React 中创建和销毁图表

让 GPU 资源随已挂载的画布创建和释放，同时处理初始化尚未完成就卸载的情况。

```tsx
'use client';

import { useEffect, useRef } from 'react';
import { FerroChart, type OhlcRow } from '@ferrochart/core';

const bars: OhlcRow[] = Array.from({ length: 60 }, (_, i) => ({
  time: Date.UTC(2025, 0, 1) + i * 60_000,
  open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
}));

export function PriceChart() {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    let disposed = false;
    let chart: FerroChart | undefined;
    void FerroChart.create(element).then(created => {
      if (disposed) { created.destroy(); return; }
      chart = created;
      created.addCandlestickSeries().setData(bars);
      created.fitContent();
    }).catch(error => console.error('Chart initialization failed', error));
    return () => { disposed = true; chart?.destroy(); };
  }, []);
  return <canvas ref={canvas} style={{ display: 'block', width: '100%', height: 360 }} />;
}
```

- 在客户端组件中渲染 `<PriceChart />`。effect 在浏览器中运行；disposed 检查也能处理 React 开发模式中的初始化与清理循环。
- 实时数据场景中，将系列保存在 ref 中并调用 `update`；不要每次收到行情都重建图表。参见 [React](frameworks.md#react)。

## 切换浅色与深色主题

使用另一个主题重建图表，同时保留视口和绘图。

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountThemeSwitch(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  let theme: 'light' | 'dark' = 'light';
  const create = async () => {
    const result = await FerroChart.create(canvas, { theme });
    result.addCandlestickSeries().setData(bars);
    return result;
  };
  let chart: FerroChart | undefined = await create();
  chart.fitContent();
  let disposed = false;
  const button = document.createElement('button');
  button.textContent = 'Switch theme';
  host.append(button);
  button.onclick = async () => {
    if (!chart) return;
    button.disabled = true;
    const state = chart.saveState();
    chart.destroy();
    chart = undefined;
    theme = theme === 'light' ? 'dark' : 'light';
    try {
      const next = await create();
      if (disposed) { next.destroy(); return; }
      chart = next;
      chart.loadState(state);
      button.disabled = false;
    } catch (error) { console.error('Theme rebuild failed', error); }
  };
  return () => {
    disposed = true;
    chart?.destroy();
    canvas.remove(); button.remove();
  };
}
```

- 主题在创建时指定，不能通过 `applyOptions` 更新。恢复状态前，重建相同的数据、系列和窗格，并重新订阅事件。
- 未显式设置颜色的系列会跟随新主题。此示例在重建期间禁用切换；重建失败时会报告错误，不会自动重试。参见[主题](styling.md#theme)和[状态不包含的内容](state.md#what-it-does-not-contain)。

## 为十字光标创建 DOM 提示框

在自己的 HTML 浮层中显示悬停 K 线的收盘价。

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountTooltip(host: HTMLElement) {
  const wrapper = document.createElement('div');
  wrapper.style.position = 'relative';
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  const tip = document.createElement('div');
  tip.hidden = true;
  tip.style.cssText = 'position:absolute;pointer-events:none;background:#111;color:#fff;padding:6px';
  wrapper.append(canvas, tip);
  host.append(wrapper);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  chart.on('crosshairMove', ({ point }) => {
    if (!point?.bar || point.time === null || point.x === undefined || point.y === undefined) {
      tip.hidden = true;
      return;
    }
    tip.textContent = `${new Date(point.time).toISOString()} · ${point.bar.close.toFixed(2)}`;
    tip.style.left = `${point.x + 12}px`;
    tip.style.top = `${point.y + 12}px`;
    tip.hidden = false;
  });
  return () => { chart.destroy(); wrapper.remove(); };
}
```

- 坐标是相对于画布的 CSS 像素，因此浮层应放在同一个已定位的容器中。显示行情源提供的文本时使用 `textContent`。
- 离开绘图区时会收到 `point: null`；数据缺口处可能没有 K 线。同步的十字光标没有指针坐标，因此本例将其提示框隐藏。参见[十字光标事件](crosshair-events.md#crosshairmove)。

## 将 pandas DataFrame 导出为 PNG

使用 pandas DataFrame 中的 OHLCV 数据绘图，无需打开窗口即可保存图表图片。

```python
import pandas as pd
import ferrochart as fc

df = pd.DataFrame({
    "time": pd.to_datetime([
        "2026-01-01T00:00:00Z", "2026-01-01T00:01:00Z", "2026-01-01T00:02:00Z"
    ]),
    "open": [100.0, 101.0, 100.5],
    "high": [102.0, 102.5, 103.0],
    "low": [99.0, 100.0, 100.0],
    "close": [101.0, 100.5, 102.0],
    "volume": [1200.0, 950.0, 1400.0],
})
chart = fc.Chart(theme="dark", width=800, height=450)
candles = chart.add_series("candlestick")
candles.set_data(df)
chart.add_volume_series(candles)
chart.fit_content()
chart.to_png("pandas-chart.png")
```

- 安装 `ferrochart` 和 `pandas`。日期时间列会转换为 UTC 毫秒；此例显式提供 UTC 时间戳。
- 数据行应按时间排序，并包含所有 OHLC 列。`to_png(path)` 会写入 PNG 文件，无需调用 `show()`。参见 [Python 快速入门](python.md#quick-start)。

## 在仅有 CPU 的服务器上渲染 PNG

无需窗口或 GPU 适配器即可生成 PNG 字节数据，用于写入文件或构造自己的 HTTP 响应。

```python
from pathlib import Path
import ferrochart as fc

chart = fc.Chart(width=800, height=450)
chart.add_series("line", color="#2196f3").set_data({
    "time": [1767225600000, 1767225660000, 1767225720000],
    "value": [100.0, 101.5, 100.8],
})
chart.fit_content()
png = chart.to_png(backend="cpu")
Path("server-chart.png").write_bytes(png)
```

- 安装 `ferrochart`。`backend="cpu"` 是 `to_png` 的参数，不是 `Chart` 构造函数的参数；这条渲染路径不会创建 GPU 适配器。
- 不传路径时，`to_png` 返回字节数据。应根据输出需求指定尺寸，而非依赖浏览器画布。参见 [CPU 离屏渲染](offscreen.md#on-the-cpu)和 [Python](python.md#quick-start)。
