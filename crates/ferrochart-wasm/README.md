# @ferrochart/core

High-performance financial charts for the web, written in Rust and compiled to
WebAssembly. Renders with WebGPU, falling back to WebGL2.

- Millions of bars: 1M bars load and paint in about 40 ms and pan at full
  frame rate, even when 100 000 of them are on screen.
- Real-time updates at any rate; the chart redraws at most once per frame.
- Candlestick, OHLC, Heikin-Ashi, line, area, baseline, histogram and volume
  series; multiple panes; price lines and markers; normal, logarithmic,
  percentage and indexed price scales.
- 30 built-in indicators (`indicatorNames()`), updated incrementally.
- 16 drawing tools with undo/redo; synced crosshair and visible range across
  charts; save and restore the view, layout and drawings as JSON.
- Panning with kinetic scrolling, wheel and pinch zoom, axis scaling,
  crosshair with magnet mode.
- TypeScript types included.

```bash
npm install @ferrochart/core
```

```js
import { FerroChart, SyncGroup } from '@ferrochart/core';

// create() loads the WebAssembly module the first time; no init() needed.
const chart = await FerroChart.create(document.querySelector('canvas'), { theme: 'dark' });
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);
candles.setData([{ time: Date.UTC(2026, 0, 2), open: 1, high: 2, low: 0.5, close: 1.5, volume: 100 }]);
// Columns are faster for large data: { time, open, high, low, close, volume } as typed arrays, UTC ms
candles.setData({ time, open, high, low, close, volume });
candles.update({ time: Date.now(), open, high, low, close });
chart.on('crosshairMove', ({ point }) => console.log(point));
```

```js
const ma = chart.addIndicator(candles, 'ema', { n: 20 });         // on the price pane
chart.addIndicator(candles, 'macd');                                // on a new pane
chart.drawings.startTool('trend_line');                             // click twice on the chart
const group = new SyncGroup();
chart.joinSync(group);
other.joinSync(group, { crosshair: true, visibleRange: true });
localStorage.layout = JSON.stringify(chart.saveState());
```

The canvas is sized with CSS; FerroChart follows its size and the device pixel
ratio. Requires WebGPU or WebGL2 (all current browsers).

## Plots

`FerroPlot` draws charts whose x axis is not time: numbers (strikes, terms),
categories (sectors, cost items) or dates, with lines, areas, scatter, grouped
or stacked bars, waterfalls and heatmaps, on the same renderer.

```js
import { FerroPlot } from '@ferrochart/core';

const plot = await FerroPlot.create(canvas, { theme: 'dark', crosshair: true });
plot.axis('x').applyOptions({ type: 'category' });
plot.axis('x').setCategories(['Tech', 'Energy', 'Banks']);
const bars = plot.addMark('bars', { showValues: true });
bars.setData({ x: [0, 1, 2], y: [4.2, -1.3, 2.8] });
plot.addRule({ value: 0, lineStyle: 'solid' });
plot.on('hover', ({ hit }) => hit && console.log(hit.index, hit.y));
```

`chart.attachSidePlot(plot)` lines a plot up with a chart's price axis, e.g.
order book depth as horizontal bars beside the candles.

## Migrating from lightweight-charts

`@ferrochart/core/lightweight-charts` implements the commonly used part of the
lightweight-charts v4/v5 API on top of FerroChart:

```js
import { createChart, CandlestickSeries, LineSeries } from '@ferrochart/core/lightweight-charts';

const chart = createChart(document.getElementById('chart'), { layout: { background: { color: '#131722' } } });
const candles = chart.addSeries(CandlestickSeries, { upColor: '#26a69a', downColor: '#ef5350' });
candles.setData([{ time: 1700000000, open: 1, high: 2, low: 0.5, close: 1.5 }]);
chart.timeScale().fitContent();
await chart.whenReady();   // optional: the GPU is set up asynchronously
```

Differences:

- `createChart` returns at once, but the GPU is set up asynchronously. Calls
  made before that are queued; `chart.whenReady()` resolves when they have run,
  and `chart.ferrochart` is the underlying `FerroChart`. Getters such as
  `getVisibleLogicalRange()` return `null` until then.
- There are no overlay or left price scales: a series with a `priceScaleId`
  other than `'right'` gets a pane of its own.
- `layout.background`, `layout.textColor`, `layout.fontSize`, the grid
  colors and visibility, `rightPriceScale.visible`, `timeScale.visible`,
  `handleScroll`, `handleScale` and the crosshair lines' `visible` and
  `labelVisible` map to FerroChart's theme and look options, in `createChart`
  and in `applyOptions`. Wick and border colors and options without an
  equivalent are ignored.
- `subscribeClick`, `priceToCoordinate` and `coordinateToPrice` are not
  supported yet. Whitespace items of candlestick and bar series are dropped.
- Logical indices count from the first bar of all series, as in
  lightweight-charts. Times in events and getters use the format of the first
  time passed in (UTC seconds, `'YYYY-MM-DD'` or a business day).

Documentation, examples and benchmarks:
<https://github.com/0x0077/ferrochart>

Licensed under Apache-2.0.
