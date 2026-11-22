# Plots (FerroPlot)

`FerroChart` is built around bars in time. For everything else, such as a
volatility smile, a returns histogram, a sector bar chart, a P&L waterfall or
a correlation heatmap, use `FerroPlot`: a general x/y plot with numeric,
logarithmic, category and date axes. It uses the same renderers as charts
(WebGPU, then WebGL2), the same canvas handling and the same themes.

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

A plot is made of:

- **marks**: lines, areas, scatter, bars, waterfalls and cells, each with its
  own data and options;
- three **axes**: `'x'` at the bottom, `'right'`, and `'left'` (hidden by
  default);
- **rules**: horizontal or vertical lines, or shaded bands, at a value;
- **labels**: texts or symbols at data coordinates.

## Creating a plot

`FerroPlot.create(canvas, options?)` returns a promise, like
`FerroChart.create`, and loads the WebAssembly module on first use. The canvas
is sized by CSS; see [Themes and styling](styling.md#sizing-the-canvas).

| Option | Values | Default |
|---|---|---|
| `theme` | `'light'`, `'dark'` | `'light'` |
| `renderer` | `'auto'`, `'webgpu'`, `'webgl2'` | `'auto'` |
| `crosshair` | Crosshair lines through the hovered element | `false` |
| `hoverHighlight` | Highlight the hovered element | `true` |
| `attributionLogo` | The FerroChart logo at the bottom-left of the plot area | `true` |

Other members of `FerroPlot`:

| Member | Description |
|---|---|
| `addMark(kind, options?): Mark` | Adds a mark, drawn above the earlier ones |
| `axis(name): Axis` | `'x'`, `'left'` or `'right'` |
| `addRule(options): Rule` | A line or band at a value |
| `addLabel(options): Label` | A text or symbol at a point |
| `fit()` | Every axis back to its configured range; automatic axes follow the data again |
| `on(event, callback)`, `off(event, callback)` | Events: `hover`, `click`, `viewChange` |
| `playIntro(options?)`, `finishIntro()` | Intro animation, as for charts |
| `setAttributionLogo(on)` | Shows or removes the logo |
| `renderer` | `'webgpu'` or `'webgl2'` |
| `destroy()` | Removes listeners and the logo, releases GPU resources (`dispose()` is the same method, under its earlier name) |

The user can pan and zoom a plot: dragging pans, the wheel or a pinch zooms the
x axis around the pointer (over a y axis, or with Alt, it zooms that axis),
Shift+wheel and horizontal scrolling pan, dragging along an axis zooms it, and
a double click fits. Category axes pan but do not zoom.

## Marks

`plot.addMark(kind, options?)` adds a mark of a `MarkKind` with `MarkOptions`
and returns a `Mark` handle.

| Kind | Draws | Data used |
|---|---|---|
| `'line'` | Points joined in data order | `x`, `y` |
| `'area'` | The area between the line and `y0` (or `baseValue`) | `x`, `y`, `y0?` |
| `'scatter'` | A symbol per point | `x`, `y`, `size?`, `value?`, `color?`, `label?` |
| `'bars'` | Bars from `y0` (or `baseValue`) to `y` | `x`, `y`, `y0?`, `value?`, `color?`, `label?` |
| `'waterfall'` | Running-total bars on a category axis | `x`, `y`, `step?` |
| `'cells'` | Colored cells of a grid (heatmaps) | `x`, `y`, `value` |

### Data

`mark.setData(data)` replaces the data. All columns have the same length;
typed arrays are copied in one `memcpy` per column.

| Column | Type | Meaning |
|---|---|---|
| `x` | `Float64Array \| number[]` | Numbers, UTC ms (date axes) or category numbers. For horizontal bars: the values |
| `y` | `Float64Array \| number[]` | Values; `NaN` is a gap. For horizontal bars: the position on the y axis |
| `y0` | `Float64Array \| number[]` | Lower edge of areas, start of bars |
| `value` | `Float64Array \| number[]` | Values colored by the color scale (cells, bars, scatter) |
| `size` | `Float32Array \| number[]` | Scatter diameters, CSS px |
| `color` | `(string \| null)[] \| Uint32Array` | Per-point colors (`null`: the mark color), or `0xRRGGBBAA` |
| `step` | `('delta' \| 'subtotal' \| 'total')[]` | Waterfall roles; default all `'delta'` |
| `label` | `(string \| null)[]` | Per-point texts: scatter labels; replaces the value text of bars and cells |

`setData` throws and keeps the old data when columns have different lengths,
an `x` is not finite, or a value on a category axis is not a whole number from
0.

Three ways to change data:

| Method | Use it for |
|---|---|
| `setData(data)` | Replacing everything |
| `append(data)` | Adding points at the end; `data` must have the same columns as the mark's data |
| `update(point)` | Replacing the last point, e.g. a live value; O(1), no allocation |

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

`update` takes a `MarkPoint` (`{ x, y, y0?, value?, size?, color? }`); fields
for columns the mark does not have are ignored. It throws if the mark has no
data or `x` is not finite. The color-scale domain and the automatic cell size
keep the values computed by the last `setData` or `append`.

### Options

Options are passed to `addMark` and changed with `mark.applyOptions(options)`;
keys you leave out keep their value. `mark.options()` returns every option.
A `null` color means the default: the next color of the plot's palette for
`color` (the first mark gets the theme's line color), or the theme's color for
the others.

| Option | Applies to | Meaning | Default |
|---|---|---|---|
| `yAxis` | all | `'left'` or `'right'` | `'right'` |
| `orientation` | bars, waterfall | `'vertical'` or `'horizontal'` | `'vertical'` |
| `color` | all | Main color | palette |
| `downColor` | area, waterfall | Areas below the base value; decreasing waterfall items | theme |
| `totalColor` | waterfall | Subtotals and totals | theme |
| `fillAlpha` | area | Opacity of the fill, 0 to 1 | `0.25` |
| `lineWidth` | line, area | CSS px, > 0 | `2` |
| `lineStyle` | line, area | `'solid'`, `'dashed'`, `'dotted'` | `'solid'` |
| `stepLine` | line, area | Steps: horizontal to the next x, then vertical | `false` |
| `pointSize` | line, scatter | Line: a symbol at each point (0: none). Scatter: the diameter when there is no `size` column | `0` |
| `icon` | line, scatter | `'circle'`, `'square'`, `'arrowUp'`, `'arrowDown'` | `'circle'` |
| `baseValue` | area, bars | Start of areas and bars without `y0` | `0` |
| `group` | bars | Bars of the same group share a slot side by side; `null` for none | `null` |
| `stack` | bars, area | Marks of the same stack add up (positive and negative values apart); `null` for none | `null` |
| `band` | bars, cells | Fraction of the slot taken, 0 (exclusive) to 1; `null` for the default (0.8 for bars, 1 for cells) | `null` |
| `colorScale` | cells, bars, scatter | How `value` maps to colors (below); `null` for the default sequential scale | `null` |
| `showValues` | bars, waterfall, cells | Write the value on each element when it fits | `false` |
| `valueFormat` | bars, waterfall, cells | Format of those values (see [Formats](#formats)) | `'auto'` |
| `cellSize` | cells | `[width, height]` in data units on numeric axes; `null`: the smallest gap between distinct x and between distinct y | `null` |
| `hoverable` | all | Hit by hover and click | `true` |
| `visible` | all | Drawn | `true` |

Invalid values throw, for example `'fillAlpha' must be between 0 and 1` or
`'icon' must be 'circle', 'square', 'arrowUp' or 'arrowDown'`.

`mark.id` is unique in its plot, `mark.kind` is the kind, and `mark.remove()`
removes the mark.

### Bars on a category axis

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

Category `k` covers the slot from `k` to `k + 1` and is labelled
`labels[k]`. With `stack: 1` instead of `group: 1`, the bars would be stacked.
For horizontal bars, set `orientation: 'horizontal'`, make the left or right
axis the category axis and swap the meaning of the columns: `x` holds the
values and `y` the category numbers.

### Waterfall

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

A `'delta'` adds its value to the running total. A `'subtotal'` or `'total'`
draws a bar from 0 to the running total and leaves it unchanged; its `y` is
not used. In hover and click events, `hit.y` of a subtotal or total is the
running total.

### Scatter with sizes, colors and labels

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

### Cells (heatmaps)

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

On numeric axes, cells are `cellSize` data units wide and high, or as large as
the smallest gap between distinct `x` and between distinct `y` values.

## Axes

`plot.axis(name)` returns the `'x'`, `'left'` or `'right'` axis. The left axis
is hidden by default; a mark uses it with `yAxis: 'left'`.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('left').applyOptions({ visible: true, title: 'Volume', format: 'compact' });
plot.addMark('bars', { yAxis: 'left', color: 'rgba(120, 120, 120, 0.5)' });
plot.addMark('line', { yAxis: 'right' });
```

### `AxisOptions`

`axis.applyOptions(options)` changes options; keys left out keep their value.
`axis.options()` returns every option.

| Option | Meaning | Default |
|---|---|---|
| `type` | `'linear'`, `'log'` or `'category'` | `'linear'` |
| `range` | `'auto'`, following the data, or a fixed `[lo, hi]` | `'auto'` |
| `includeZero` | Automatic range: keep 0 inside | `false` |
| `margin` | Automatic range: fraction added at both ends, 0 to 1 | `0.05` (category: `0`) |
| `format` | How tick labels are written (below) | `'auto'` |
| `inverted` | Values decrease to the right (x) or upwards (y) | `false` |
| `visible` | Ticks, labels and the axis line are drawn | `true` (left: `false`) |
| `grid` | Grid lines at the ticks (category axes: at slot boundaries) | `true` (category: `false`) |
| `title` | Text beside the axis; `null` removes it | `null` |
| `locked` | Panning and zooming leave the axis alone | `false` |

Changing `type` starts from that type's defaults for `grid`, `margin` and
`range`; `visible`, `title`, `format`, `inverted` and `locked` are kept.

The automatic range covers the visible data of the marks on that axis. For
bars, areas and waterfalls, an end at zero gets no margin, so bars stand on the
axis. A category axis spans all categories.

Errors: a fixed range must be two finite, different numbers, and positive on a
log axis.

### Log axes

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('right').applyOptions({ type: 'log' });
```

Values of 0 or less have no position on a log axis and are not drawn.

### Category axes

A category axis has equal slots; category `k` covers `[k, k + 1)` and its
center is `k + 0.5`. Data on it are whole numbers from 0. Name the categories
with `axis.setCategories(labels)` (up to 1 000 000).

### Date axes

A date axis is a linear axis whose values are UTC milliseconds, with the
`'date'` format:

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('x').applyOptions({ format: 'date' });                              // UTC
plot.axis('x').applyOptions({ format: { type: 'date', utcOffsetMinutes: 60 } }); // UTC+1
```

### Formats

`PlotFormat` is used by `AxisOptions.format` and `MarkOptions.valueFormat`:

| Format | Writes |
|---|---|
| `'auto'` | At least 2 decimals and 4 significant digits, with a subscript count of zeros for small values (like chart price labels) |
| `'compact'` | K, M and B suffixes |
| `{ type: 'fixed', decimals }` | Fixed decimals, 0 to 12 |
| `{ type: 'percent', decimals }` | Fixed decimals and a `%` sign; values are already percent (`5` is written `5%`) |
| `'date'` | Values are UTC ms, written as dates and times in UTC |
| `{ type: 'date', utcOffsetMinutes }` | The same at an offset from UTC, −1440 to 1440 minutes |

### The visible range

| Method | Description |
|---|---|
| `axis.view(): [number, number]` | The visible range as of the last frame |
| `axis.setView(lo, hi)` | Shows `lo..hi` (category units on a category axis); the axis stops following the data until `plot.fit()`. Sends no `viewChange` event |
| `plot.fit()` | Every axis back to its configured range |

## Rules

A rule is a line across the plot at a value, or a shaded band between two
values.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const target = plot.addRule({ value: 100, label: 'target' });          // horizontal line on the right axis
plot.addRule({ axis: 'x', value: Date.UTC(2024, 5, 1), label: 'FOMC' }); // vertical line
plot.addRule({ value: 30, to: 70, color: 'rgba(120, 120, 255, 0.12)' }); // band

target.applyOptions({ value: 105 });
target.remove();
```

| Option | Meaning | Default |
|---|---|---|
| `axis` | The axis of `value`: `'x'` gives a vertical rule | `'right'` |
| `value` | Position | `0` |
| `to` | Other edge of a band; `null` for a line | `null` |
| `color` | Color; `null` for the theme's crosshair color (bands at 12% opacity) | `null` |
| `lineWidth` | CSS px | `1` |
| `lineStyle` | `'solid'`, `'dashed'`, `'dotted'` | `'dashed'` |
| `label` | Text on the axis at the value; `null` for none | `null` |
| `labelBackground` | The label sits on a box of the rule color (otherwise it is written in the rule color) | `true` |

## Labels

A label is a text, a symbol or both at a point in data coordinates.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const peak = plot.addLabel({ x: 3, y: 14.7, text: 'peak', icon: 'arrowDown', align: 'center', offset: [0, -14] });
peak.remove();
```

| Option | Meaning | Default |
|---|---|---|
| `x`, `y` | The point (required) | |
| `yAxis` | `'left'` or `'right'` | `'right'` |
| `text` | Text (may be empty with an icon) | `''` |
| `icon` | `'circle'`, `'square'`, `'arrowUp'`, `'arrowDown'` or `null` | `null` |
| `color` | `null` for the theme's text color | `null` |
| `align` | Text alignment relative to the point: `'left'`, `'center'`, `'right'` | `'left'` |
| `offset` | Offset of the text from the point, CSS px right and down | `[6, 0]` |

Labels cannot be changed; remove one and add another.

## Color scales

Cells always color their `value` column by a color scale. Bars and scatter
marks do so when they have a `value` column; otherwise they use `color`. A
point's own color in the `color` column takes precedence over its value.

| `colorScale` | Meaning |
|---|---|
| `null` or `'sequential'` | Dark blue to yellow, low to high |
| `'diverging'` | The theme's down color, a neutral middle and the up color, centered on 0 |
| `{ stops?, center?, domain?, quantile? }` | Your own scale |

The object form:

| Key | Meaning |
|---|---|
| `stops` | `[position, color][]`, positions 0 to 1 in ascending order (at least one stop) |
| `center` | Makes the scale diverging around this value: the middle of the scale is at `center`, and the side farther from it decides the extent of both sides |
| `domain` | A fixed `[lo, hi]`; values outside it get the end colors |
| `quantile` | Without `domain`: the scale covers the values from the `quantile` to the `1 − quantile` quantile, so outliers saturate. 0 to 0.5, default 0.01 |

Without `stops`, a scale with `center` uses the diverging colors and one
without uses the sequential colors.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const heat = plot.addMark('cells', {
  colorScale: { stops: [[0, '#0d1b4c'], [0.5, '#219ebc'], [1, '#f4e04d']], quantile: 0.05 },
});
```

The domain is computed from the data by `setData` and `append`; `update`
keeps it.

## Hit testing and events

The plot finds the element under the pointer: a point of a line or scatter, a
bar or a cell. It highlights it (`hoverHighlight`), draws crosshair lines
through it (`crosshair`), and reports it.

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

In `click`, `x` and `y` are the axis values at the pointer (`y` on the right
axis, or on the left axis if the right one is hidden). Marks with
`hoverable: false` are never hit.

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

Events are delivered after the frame that produced them.

## Lining a plot up with a chart pane

`chart.attachSidePlot(plot, pane?)` places a plot beside a chart pane so that
a price is at the same height in both. Every frame, the chart sets the plot's
right axis to the prices at the top and bottom of the plot area, as the pane
maps them. The canvases' positions on the page are read each frame, so the
plot may be anywhere beside the chart, and its plot area may be taller or
shorter than the pane.

- The plot's right axis is locked: the chart is its only source. If the pane's
  price scale is logarithmic when you attach, the right axis becomes a log
  axis.
- The plot's attribution logo is removed; the chart's stands for both.
- `chart.detachSidePlot(plot)` stops following; the right axis stays locked
  and keeps its last range.

### Example: a volume profile

The profile shows how much volume traded at each price in the visible bars, as
horizontal bars beside the candles.

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

Horizontal bars on a numeric axis are as thick as `band` times the smallest
gap between their `y` values, so rows of equal size touch with `band: 1`.
When the user scrolls or zooms the chart, `visibleRangeChanged` recomputes the
profile for the visible bars, and the price range follows by itself. For
order book depth beside the candles, use the same setup with bid and ask
sizes as `x` and their prices as `y`, and `rows.update` for live changes of
the last level.

## Cleaning up

`plot.destroy()` removes the plot's listeners and logo and releases its GPU
resources; a side plot also stops following its chart. The `Mark`, `Axis`,
`Rule` and `Label` handles then throw (`the FerroPlot was destroyed`). The
canvas element stays in the page.
