# Cookbook

Task-oriented recipes for applications using FerroChart. Each browser recipe is
an independent TypeScript module for a browser bundler with `@ferrochart/core`
installed; the React recipe also needs React. Pass a mounted, nonzero-width
`HTMLElement` as `host`, for example `document.body`, and call the returned
cleanup when the view is removed. The price-alert recipe instead returns
`update` and `destroy` methods. See [Web setup](web.md).

The examples use synthetic data. Timestamps are UTC milliseconds, with bars in
ascending time order. Network recipes describe the input contract for your own
feed; no market-data service is bundled. Python recipes specify their packages
and write PNG files into the current directory.

## Stream candles over WebSocket

Connect a chart to your own WebSocket endpoint that sends one JSON OHLC bar per text message.

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

- The feed must send `time` in UTC milliseconds and `open`, `high`, `low`, `close` (optionally `volume`); adapt your provider's payload before calling `update`.
- The same opening time replaces the last candle; a later time appends one. Earlier times throw. Each update supplies the complete current candle, not a trade delta.
- Call the returned cleanup when unmounting; connection recovery belongs to your feed adapter. See [Real-time updates](data.md#real-time-updates).

## Load older history when scrolling left

Load an earlier page without moving the current view; this standalone example uses an in-memory archive in place of a backend.

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

- Replace `loadBefore` with your backend request: return ascending, unique timestamps strictly before `before`. Keep volume columns consistent with the initial data.
- `needMoreHistory` fires once per first bar. An empty response ends pagination; a failed request can be retried explicitly with the button.
- `prepend` preserves existing logical indices and the view; do not call `fitContent` after each page. See [Loading older history](data.md#loading-older-history).

## Put an indicator in its own pane

Add an overlay moving average and an RSI pane to the same candle series.

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

- Placement is the fourth argument: `'overlay'`, `'newPane'`, or an existing pane id. Defaults depend on the indicator.
- Indicators follow their source data; do not feed them separately. Supply enough history for the chosen period. See [Indicator placement](indicators.md#placement).

## Attach buy and sell records to markers

Return to your own trade object when a user clicks its marker.

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

- `id` is your safe non-negative integer identifier; `target.marker` is the object passed to `setMarkers`, including its custom `trade` field.
- The event's index refers to time-sorted markers, not the original array order. A folded badge has type `'markerOverflow'`; handle it separately if needed.
- `setMarkers` replaces the marker list. See [Markers](markers.md#markers) and [Click events](crosshair-events.md#click).

## Draw a price alert line and detect crossings

Show a threshold and notify your application when incoming candle closes cross it upward.

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

- Send feed bars through the returned `update` method; the line itself does not monitor prices or deliver alerts.
- This detects close-to-close crossings, including updates to the forming candle. It triggers again only after price goes below the threshold and crosses back up; it is not a background service.
- Call `destroy` on unmount. See [Price lines](markers.md#price-lines) and [Real-time updates](data.md#real-time-updates).

## Synchronize two charts

Share crosshair time and visible time range across two charts.

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

- Synchronization uses time, not bar indices; each chart still needs its own data. Bar-spacing limits can constrain the range a chart displays.
- Destroying a chart leaves its group. Release the group after its charts; drawing synchronization is separate and off by default. See [Synchronizing charts](sync.md#time-not-bar-numbers).

## Save drawings and the viewport

Persist the user's view and drawings in localStorage while reconstructing the chart's data yourself.

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

- Recreate data, series, indicators and the same pane layout before `loadState`; the state does not contain market data or series definitions.
- Restoring replaces drawings and clears undo history. Use separate storage keys for incompatible layouts or instruments, and surface storage/restore errors.
- Click Save before unmounting. See [State contents](state.md#what-a-state-contains) and [Loading a state](state.md#loading-a-state).

## Create and destroy a chart in React

Keep GPU resources tied to a mounted canvas, including an unmount that happens while initialization is pending.

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

- Render `<PriceChart />` in a client component. The effect runs in the browser; the disposed check also handles React's development setup/cleanup cycle.
- For live data, retain the series in a ref and call `update`; do not recreate the chart on every tick. See [React](frameworks.md#react).

## Switch between light and dark themes

Rebuild the chart with a different theme while carrying its viewport and drawings across.

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

- Theme is a creation option, not an `applyOptions` update. Recreate the same data, series and panes before restoring state; reattach your event subscriptions as well.
- Unset series colors follow the new theme. This example disables switching while rebuilding; a rebuild failure is reported rather than retried automatically. See [Theme](styling.md#theme) and [State exclusions](state.md#what-it-does-not-contain).

## Build a DOM tooltip for the crosshair

Display the hovered candle close in your own HTML overlay.

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

- Coordinates are CSS pixels relative to the canvas, so keep the overlay in the same positioned wrapper. Use `textContent` for feed-derived text.
- Leaving the plot sends `point: null`; gaps can have no bar. A synchronized crosshair has no pointer coordinates and is hidden here. See [Crosshair events](crosshair-events.md#crosshairmove).

## Export a pandas DataFrame to PNG

Draw OHLCV data from a pandas DataFrame and save a chart image without opening a window.

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

- Install `ferrochart` and `pandas`. Datetime columns are converted to UTC milliseconds; this example supplies explicit UTC timestamps.
- Keep rows sorted by time and supply all OHLC columns. `to_png(path)` writes a PNG; `show()` is not needed. See [Python quick start](python.md#quick-start).

## Render a PNG on a CPU-only server

Create PNG bytes without a window or GPU adapter, ready for a file or your own HTTP response.

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

- Install `ferrochart`. `backend="cpu"` belongs to `to_png`, not the `Chart` constructor; this path does not create a GPU adapter.
- Without a path, `to_png` returns bytes. Choose dimensions for your output rather than relying on a browser canvas. See [CPU offscreen rendering](offscreen.md#on-the-cpu) and [Python](python.md#quick-start).
