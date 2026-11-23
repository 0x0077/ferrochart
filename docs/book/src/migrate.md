# Migrating from lightweight-charts

FerroChart has two ways in for code written for lightweight-charts:

1. **The compatibility entry** `@ferrochart/core/lightweight-charts`. It
   implements the commonly used part of the lightweight-charts v4 and v5 API on
   top of FerroChart, so an existing chart often runs after changing the
   import. Use it to try FerroChart on an existing page, or to migrate in
   steps.
2. **The native API** `@ferrochart/core`. It is what the rest of this guide
   describes, and gives access to everything: columnar data for large data
   sets, indicators, drawings, sync, order flow and plots.

## The compatibility entry

```ts
// Before:
// import { createChart, CandlestickSeries, LineSeries } from 'lightweight-charts';
import { createChart, CandlestickSeries, LineSeries, type UTCTimestamp } from '@ferrochart/core/lightweight-charts';

const container = document.getElementById('chart') as HTMLElement;
const chart = createChart(container, { layout: { background: { color: '#131722' } } });

const candles = chart.addSeries(CandlestickSeries, { upColor: '#26a69a', downColor: '#ef5350' });
candles.setData([
  { time: 1700000000 as UTCTimestamp, open: 10, high: 12, low: 9, close: 11 },
  { time: 1700000060 as UTCTimestamp, open: 11, high: 13, low: 10, close: 12 },
]);

const sma = chart.addSeries(LineSeries, { color: '#2962ff', lineWidth: 2 });
sma.setData([{ time: 1700000060 as UTCTimestamp, value: 11.5 }]);

chart.timeScale().fitContent();
```

### Supported

| Area | Supported |
|---|---|
| Creating | `createChart(container, options)`, with an element or its id |
| Series | `addSeries(definition, options?, paneIndex?)` with `CandlestickSeries`, `BarSeries`, `LineSeries`, `AreaSeries`, `BaselineSeries`, `HistogramSeries`; the v4 methods `addCandlestickSeries`, `addBarSeries`, `addLineSeries`, `addAreaSeries`, `addBaselineSeries`, `addHistogramSeries`; `removeSeries` |
| Data | `setData`, `update`, `dataByIndex`; times as `UTCTimestamp` seconds, `'YYYY-MM-DD'` strings or `BusinessDay` objects; per-bar `color` of histogram data |
| Series API | `applyOptions`, `options`, `seriesType`, `priceScale`, `createPriceLine`, `removePriceLine` |
| Markers | v4 `series.setMarkers` / `series.markers`; v5 `createSeriesMarkers(series, markers)` with `setMarkers`, `markers`, `detach` |
| Time scale | `fitContent`, `scrollToRealTime`, `scrollToPosition`, `scrollPosition`, `getVisibleLogicalRange`, `setVisibleLogicalRange`, `getVisibleRange`, `setVisibleRange`, `logicalToCoordinate`, `coordinateToLogical`, `timeToCoordinate`, `coordinateToTime`, the visible logical and time range subscriptions, `applyOptions` / `options` (`barSpacing`, `rightOffset`), `width` |
| Price scale | `chart.priceScale('right', paneIndex)` and `series.priceScale()`: `applyOptions` / `options` with `scaleMargins`, `mode`, `invertScale`, `autoScale` |
| Crosshair | `subscribeCrosshairMove` / `unsubscribeCrosshairMove`, with `time`, `logical`, `point`, `paneIndex` and `seriesData` |
| Chart | `applyOptions` / `options` (`width`, `height`, `layout.background.color`, `crosshair.mode`, `timeScale.barSpacing`, `timeScale.rightOffset`), `resize`, `remove`, `timeScale`, `priceScale` |
| Constants | `ColorType`, `CrosshairMode`, `LineStyle`, `LineType`, `PriceScaleMode`, `isUTCTimestamp`, `isBusinessDay` |

TypeScript declarations are included (`ISeriesApi`, `IChartApi`, `Time`,
`UTCTimestamp`, `SeriesMarker` and so on).

### Asynchronous start

The GPU is set up asynchronously, but `createChart` returns at once, as in
lightweight-charts. Calls made before the chart is ready are queued and run in
order once it is. Two additions help where that matters:

| Member | Description |
|---|---|
| `chart.whenReady(): Promise<void>` | Resolves when the GPU is set up and the queued calls have run |
| `chart.ferrochart: FerroChart \| null` | The underlying native chart, `null` until ready |

Getters such as `getVisibleLogicalRange()`, `getVisibleRange()`,
`coordinateToTime()` or `dataByIndex()` return `null` until the chart is ready
(and `priceScale().options()` returns `{}`). Await `whenReady()` before
reading them:

```ts
import { createChart } from '@ferrochart/core/lightweight-charts';

declare const container: HTMLElement;

const chart = createChart(container);
// ... add series and data ...
await chart.whenReady();
console.log(chart.timeScale().getVisibleLogicalRange());
```

An error in a queued call is written to the console with `console.error`
instead of being thrown, since the call that caused it has already returned.
After the chart is ready, errors are thrown as usual.

### Differences

**Rendering and size**

- The chart draws into a `<canvas>` that `createChart` appends to the
  container. Without `width`/`height` options the canvas fills the container
  (`100%` × `100%`) and follows its size, so give the container a height.
  `autoSize` is not needed and is ignored.
- `width`, `height` and `resize(width, height)` set the canvas's CSS size.
- `remove()` destroys the chart and removes its canvas.
- WebGPU or WebGL2 is required; see [Themes and styling](styling.md#renderer).

**Theme and colors**

- `layout.background.color` picks the theme: a dark color written as
  `#rrggbb` (or `#rrggbbaa`) gives the dark theme, anything else the light
  theme. Other `layout` options (`textColor`, `fontSize`, `fontFamily`,
  gradients) are ignored. `layout` is read once, when the GPU setup finishes;
  changing it after that logs a warning and has no effect.
- `crosshair.mode`: `Normal`, `Magnet` and `Hidden` work; `MagnetOHLC` acts
  as `Magnet`.
- `grid`, `watermark`, `handleScroll`, `handleScale`, `localization`,
  `kineticScroll` and other chart options without an equivalent are accepted
  and ignored.
- Colors must be in a form FerroChart accepts: `#rgb`, `#rrggbb`, `#rrggbbaa`,
  `rgb()`, `rgba()` or `transparent`. Named colors such as `'red'` are an
  error. See [Colors](styling.md#colors).

**Series options.** Only these options are used; the others are accepted and
ignored:

| Series | Options used |
|---|---|
| Candlestick, Bar | `upColor`, `downColor` |
| Line | `color` |
| Area | `lineColor`, `topColor`, `bottomColor` |
| Baseline | `topLineColor`, `bottomLineColor`, `topFillColor1`, `bottomFillColor1`, `baseValue: { type: 'price', price }` |
| Histogram | `color`, `base` |
| All | `lineWidth`, `lineStyle`, `lineType`, `lastValueVisible`, `priceLineVisible`, `priceFormat.precision` |

So `wickUpColor`, `borderUpColor` and similar candle colors, `crosshairMarker*`,
`priceFormat.minMove`, `priceFormat.type`, `title` and `visible` have no
effect. `LineStyle.LargeDashed` draws dashed, `LineStyle.SparseDotted` dotted;
`LineType.WithSteps` draws steps and `LineType.Curved` straight segments.

**Price scales**

- Each pane has one price scale, on the right. There are no left or overlay
  price scales: a series created with a `priceScaleId` other than `'right'`
  (including the common `''` for a volume overlay) gets a pane of its own,
  30% as high as a default pane, and a warning is logged. For volume, the
  native `addVolumeSeries` (below) draws it in the bottom of the main pane
  instead.
- `chart.priceScale(id)` with an id other than `'right'` logs a warning and
  returns the right scale.

**Time**

- Times are converted to UTC milliseconds. `'YYYY-MM-DD'` strings and
  `BusinessDay` objects are midnight UTC; other string formats throw.
- The first time the chart sees (in `setData` or `update`) decides the format
  of times it returns: events, `getVisibleRange`, `coordinateToTime`,
  `dataByIndex` and the crosshair's `time` use seconds, strings or
  `BusinessDay` objects accordingly.
- Logical indices are 0 at the first bar, as in lightweight-charts.

**Data**

- Whitespace items (`{ time }` only) are dropped from candlestick and bar
  data, and are gaps in line, area, baseline and histogram data.
- OHLC data has no volume in this API. Use the native API for a volume series.

**Markers**

- `position: 'inBar'` is drawn above the bar, and an unknown `shape` as a
  circle. `size` is a multiple of the default size (1 is 10 CSS px, kept
  within 4–64 px); `id` is ignored.

**Not supported yet**

- `subscribeClick` is a no-op and logs a warning. Use the native `click`
  event through `chart.ferrochart` instead.
- `series.priceToCoordinate` and `series.coordinateToPrice` return `null` and
  log a warning.
- Custom series, series primitives and pane primitives. See
  [Plugins](plugins.md) for FerroChart's own extensions.
- `scrollToPosition(position, animated)` ignores `animated`.
- `IPriceLine.options()` returns the options you passed, not every option.

### Using native features from the compatibility entry

`chart.ferrochart` is the native `FerroChart`, so a page can keep the
lightweight-charts calls it has and use native features alongside:

```ts
import { createChart, CandlestickSeries } from '@ferrochart/core/lightweight-charts';

declare const container: HTMLElement;

const chart = createChart(container);
chart.addSeries(CandlestickSeries);
await chart.whenReady();

const fc = chart.ferrochart;
if (fc) {
  fc.drawings.startTool('trend_line');
  fc.on('click', (e) => console.log('clicked at', e.time, e.price));
}
```

Series added through the native object are not known to the compatibility
layer (they are not in `seriesData` of crosshair events, for example).

## Moving to the native API

The native API differs from lightweight-charts in a few general ways:

- **Creation is async.** `FerroChart.create(canvas, options)` returns a
  promise, and takes a `<canvas>` element sized by CSS instead of a container.
- **Times are UTC milliseconds**, as `number`, `bigint` or (in rows) `Date`.
  Multiply lightweight-charts' seconds by 1000.
- **Data can be columns.** `setData` accepts arrays of rows, but columns of
  typed arrays are much faster for large data.
- **Kinds are strings.** `chart.addSeries('candlestick', options, pane)`;
  panes are ids, not indices.
- **Options are flat.** Series colors are `color`, `downColor`, `topFill` and
  `bottomFill`; the theme is `'light'` or `'dark'`.
- **Events use `on`/`off`.** `chart.on('crosshairMove', ...)` instead of
  `subscribeCrosshairMove(...)`.

### Side by side

lightweight-charts:

```js
import { createChart, CandlestickSeries, HistogramSeries } from 'lightweight-charts';

const chart = createChart(document.getElementById('chart'), {
  layout: { background: { color: '#131722' }, textColor: '#d1d4dc' },
});
const candles = chart.addSeries(CandlestickSeries, { upColor: '#26a69a', downColor: '#ef5350' });
candles.setData(rows); // [{ time: 1700000000, open, high, low, close }]

const volume = chart.addSeries(HistogramSeries, { priceScaleId: '' });
volume.priceScale().applyOptions({ scaleMargins: { top: 0.8, bottom: 0 } });
volume.setData(rows.map((r) => ({ time: r.time, value: r.volume })));

chart.subscribeCrosshairMove((param) => {
  const bar = param.seriesData.get(candles);
  if (bar) legend.textContent = `C ${bar.close}`;
});
chart.timeScale().fitContent();
```

FerroChart:

```ts
import { FerroChart } from '@ferrochart/core';

declare const rows: { time: number; open: number; high: number; low: number; close: number; volume: number }[];
declare const legend: HTMLElement;

const canvas = document.getElementById('chart') as HTMLCanvasElement; // style="width: 100%; height: 480px"
const chart = await FerroChart.create(canvas, { theme: 'dark' });

const candles = chart.addCandlestickSeries({ color: '#26a69a', downColor: '#ef5350' });
candles.setData(rows.map((r) => ({ ...r, time: r.time * 1000 }))); // seconds to ms; volume included
chart.addVolumeSeries(candles); // reads the volume column, bottom of the main pane

chart.on('crosshairMove', ({ point }) => {
  if (point?.bar) legend.textContent = `C ${point.bar.close}`;
});
chart.fitContent();
```

### Mapping of common calls

| lightweight-charts | FerroChart |
|---|---|
| `createChart(container, options)` | `await FerroChart.create(canvas, { theme, renderer })` |
| `chart.addSeries(CandlestickSeries, o)` / `addCandlestickSeries(o)` | `chart.addCandlestickSeries(o)` or `chart.addSeries('candlestick', o)` |
| `BarSeries` / `addBarSeries` | `chart.addSeries('ohlc', o)` |
| `LineSeries`, `AreaSeries`, `BaselineSeries`, `HistogramSeries` | `chart.addSeries('line' \| 'area' \| 'baseline' \| 'histogram', o)` |
| `chart.addSeries(def, o, paneIndex)` | `chart.addSeries(kind, o, chart.panes()[paneIndex])`; new panes with `chart.addPane()` |
| Volume histogram with `priceScaleId: ''` | `chart.addVolumeSeries(candles)` |
| `chart.removeSeries(series)` | `series.remove()` |
| `series.setData(items)` | `series.setData(rows)` (times in ms) or `series.setData({ time, open, high, low, close, volume })` columns |
| `series.update(item)` | `series.update(bar)` (time in ms) |
| `series.applyOptions({ upColor, downColor })` | `series.applyOptions({ color, downColor })` |
| `series.applyOptions({ lineColor, topColor, bottomColor })` | `series.applyOptions({ color, topFill, bottomFill })` |
| `series.applyOptions({ priceFormat: { precision } })` | `series.applyOptions({ pricePrecision })` |
| `series.createPriceLine(o)` / `removePriceLine(line)` | `series.createPriceLine(o)` / `line.remove()` |
| `series.setMarkers(m)` / `createSeriesMarkers(series, m)` | `series.setMarkers(m)` (time in ms) |
| `series.dataByIndex(i)` | `series.dataAt(logicalIndex)` |
| `chart.timeScale().fitContent()` | `chart.fitContent()` |
| `chart.timeScale().scrollToRealTime()` | `chart.scrollToRealtime()` |
| `timeScale().scrollToPosition(n)` / `scrollPosition()` | `chart.setRightOffset(n)` / `chart.rightOffset` |
| `timeScale().getVisibleLogicalRange()` | `chart.visibleLogicalRange()` |
| `timeScale().setVisibleLogicalRange({ from, to })` | `chart.setVisibleLogicalRange(from, to)` |
| `timeScale().setVisibleRange({ from, to })` | `chart.applySync({ type: 'visibleRange', from, to })` (ms) |
| `timeScale().applyOptions({ barSpacing, rightOffset })` | `chart.setBarSpacing(px)`, `chart.setRightOffset(bars)` |
| `timeScale().logicalToCoordinate(i)` / `coordinateToLogical(x)` | `chart.logicalToCoordinate(i)` / `chart.coordinateToLogical(x)` |
| `timeScale().timeToCoordinate(t)` | `chart.logicalToCoordinate(chart.indexOfTime(ms))` |
| `timeScale().coordinateToTime(x)` | `chart.timeAtIndex(chart.coordinateToLogical(x))` |
| `timeScale().subscribeVisibleLogicalRangeChange(h)` | `chart.on('visibleRangeChanged', h)` |
| `chart.priceScale('right').applyOptions({ scaleMargins, mode, invertScale, autoScale })` | `chart.applyPriceScaleOptions({ margins, mode, inverted, autoScale }, pane)` |
| `PriceScaleMode.Logarithmic` | `mode: 'logarithmic'` (also `'normal'`, `'percentage'`, `'indexedTo100'`) |
| `chart.applyOptions({ crosshair: { mode: CrosshairMode.Magnet } })` | `chart.setCrosshairMode('magnet')` |
| `chart.subscribeCrosshairMove(h)` | `chart.on('crosshairMove', h)` |
| `chart.subscribeClick(h)` | `chart.on('click', h)` |
| `chart.applyOptions({ width, height })` / `resize(w, h)` | Set the canvas's CSS size; the chart follows |
| `chart.remove()` | `chart.destroy()` |

Logical indices differ in one respect: in FerroChart a bar keeps its logical
index when older data is prepended, so the first bar can have a negative
index. `chart.dataRange()` returns the indices of the first and last bar.

For the details of each native call, see [Series](series.md), [Data](data.md),
[Time scale](time-scale.md), [Price scale](price-scale.md),
[Crosshair and events](crosshair-events.md) and [Markers](markers.md).
