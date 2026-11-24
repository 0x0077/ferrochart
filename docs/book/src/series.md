# Series types

A series is one set of data drawn one way: candles, a line, a histogram and
so on. A chart can hold any number of series on any number of panes. All
series of a chart share one time axis (see [Data](data.md)).

## Adding a series

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas);

const candles = chart.addSeries('candlestick');           // main pane
const sma = chart.addSeries('line', { color: '#2962ff' }); // with options
const pane = chart.addPane(0.3);
const osc = chart.addSeries('histogram', {}, pane);        // on another pane
```

`addSeries(kind, options?, pane?)` returns a `Series` handle. Leaving out
`pane` puts the series on the main (top) pane. `addCandlestickSeries(options?,
pane?)` is a shorthand for `addSeries('candlestick', ...)`. An unknown kind or
pane id throws.

A `Series` has three read-only properties:

| Property | Meaning |
|---|---|
| `id` | Number, unique within the chart. Handles of the same series (for example one from a `click` event) have the same id. |
| `kind` | The kind passed to `addSeries`; `'volume'` for volume series, `'kagi'` / `'pointFigure'` for those transforms (Renko and range bars report `'candlestick'` or `'ohlc'`), `'custom'` for plugin series. |
| `pane` | Id of the pane the series is on. |

## Kinds

| Kind | Data | Draws |
|---|---|---|
| `'candlestick'` | OHLC | Candles: body from open to close, wick from low to high |
| `'ohlc'` | OHLC | Bars with an open tick on the left and a close tick on the right |
| `'heikinAshi'` | OHLC | Heikin-Ashi candles computed from the bars you pass |
| `'line'` | value | A line through the values |
| `'area'` | value | A line with a gradient fill down to the bottom of the pane |
| `'baseline'` | value | A line and fills above and below `baseValue`, in different colors |
| `'histogram'` | value | Bars from `baseValue` to each value, optionally colored per point |
| `'points'` | value | A dot at each value |

Volume histograms are added with `addVolumeSeries` (see [Volume](#volume)),
and Renko, range bars, Kagi and Point & Figure with `addTransformSeries` (see
[Bar transforms](#bar-transforms)).

OHLC series take `{ time, open, high, low, close, volume? }`; value series
take `{ time, value, color? }`. [Data](data.md) describes both formats, row
arrays, validation and real-time updates.

### Candlestick

```ts
const candles = chart.addCandlestickSeries({
  color: '#26a69a',      // up candles (close >= open)
  downColor: '#ef5350',  // down candles
});
candles.setData(randomWalk(1, 500, Date.UTC(2024, 0, 1), 60_000));
```

With neither `color` nor `downColor` set, wicks use the theme's wick colors.
As soon as one of them is set, wicks have the same color as the body.

### OHLC bars

`'ohlc'` takes the same data and options as `'candlestick'` and draws
traditional bars.

### Heikin-Ashi

`'heikinAshi'` takes ordinary OHLC bars; the chart computes the Heikin-Ashi
values from them, also for real-time updates. Colors work as for candlesticks.

### Line

```ts
const line = chart.addSeries('line', { color: '#2962ff', lineWidth: 1, lineStyle: 'dashed' });
line.setData({ time: [0, 60_000, 120_000, 180_000], value: [10, 11, NaN, 12] });
```

A `NaN` value is a gap: no line is drawn through it.

### Area

An area is a line with a vertical gradient below it. `topFill` is the color at
the line, `bottomFill` the color at the bottom of the pane.

```ts
chart.addSeries('area', {
  color: '#2962ff',
  topFill: 'rgba(41, 98, 255, 0.4)',
  bottomFill: 'rgba(41, 98, 255, 0)',
});
```

By default both fills are `color` with an opacity of 40 % at the top and 2 %
at the bottom.

### Baseline

A baseline series splits at `baseValue`: above it the line has `color` and
the fill `topFill`; below it the line has `downColor` and the fill
`bottomFill`.

```ts
chart.addSeries('baseline', {
  baseValue: 100,
  color: '#26a69a',
  downColor: '#ef5350',
});
```

By default the fills are the two line colors at 20 % opacity.

### Histogram

Bars go from `baseValue` (default 0) to each value. The auto-scaled range
always includes `baseValue`.

Each point can have its own color: pass a `color` column next to `value`. Its
entries are CSS colors, `null` for the series color, or a `Uint32Array` of
`0xRRGGBBAA` values where `0` means the series color.

```ts
const hist = chart.addSeries('histogram', { color: '#26a69a' }, pane);
hist.setData({
  time: [0, 60_000, 120_000],
  value: [1.5, -0.7, 0.4],
  color: [null, '#ef5350', null],
});

// The same colors as packed integers.
hist.setData({
  time: [0, 60_000, 120_000],
  value: [1.5, -0.7, 0.4],
  color: new Uint32Array([0, 0xef5350ff, 0]),
});
```

Per-point colors are drawn only at full detail. When you zoom out so far that
several bars share a pixel and are merged, the series color is used. Other
kinds accept a `color` column but ignore it. `update` cannot set a color; an
appended point has the series color.

### Points

`'points'` draws a dot at each value; `lineWidth` is the dot's diameter in CSS
px. Gaps (`NaN`) have no dot.

```ts
const dots = chart.addSeries('points', { color: '#ff9800', lineWidth: 4 });
```

### Volume

A volume series draws the `volume` column of an OHLC series. It has no data of
its own: `setData` and `update` on the source series update it.

```ts
const candles = chart.addCandlestickSeries();
const volume = chart.addVolumeSeries(candles);          // main pane
// or: chart.addVolumeSeries(candles, chart.addPane(0.25));
```

The source must be an OHLC series (candlestick, OHLC bars, Heikin-Ashi) or a
transform series. Bars are colored by whether the source bar is up or down,
in the theme's volume colors. On a pane that also shows prices, the volume
has its own scale in the bottom quarter of the pane; on a pane of its own it
fills the pane. See [Price scale and panes](price-scale.md#volume-placement).

Removing the source series also removes its volume series.

## Series options

Options are passed to `addSeries` and changed later with `applyOptions`; keys
you leave out keep their value. `options()` returns every key.

```ts
const s = chart.addSeries('line', { color: '#2962ff' });
s.applyOptions({ lineWidth: 3 });       // color stays '#2962ff'
console.log(s.options().lineWidth);     // 3
s.applyOptions({ color: null });        // back to the theme color
```

| Option | Type | Default | Used by | Meaning |
|---|---|---|---|---|
| `color` | `Color \| null` | theme | all | Line color (line, area, points); up candles and bars; histogram bars; baseline line above the base; Kagi yang lines and Point & Figure X columns |
| `downColor` | `Color \| null` | theme | candlestick, ohlc, heikinAshi, baseline, transforms | Down candles and bars; baseline line below the base; Kagi yin lines and Point & Figure O columns |
| `topFill` | `Color \| null` | see [Area](#area), [Baseline](#baseline) | area, baseline | Area fill at the line; baseline fill above the base |
| `bottomFill` | `Color \| null` | see [Area](#area), [Baseline](#baseline) | area, baseline | Area fill at the bottom; baseline fill below the base |
| `lineWidth` | `number` | `2` | line, area, baseline, points, Kagi, Point & Figure | Line width in CSS px; for points, the dot diameter. Must be positive. |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | line, area, baseline | Dash pattern of the line |
| `step` | `boolean` | `false` | line, area, baseline | Step line, see [Step lines](#step-lines) |
| `baseValue` | `number` | `0` | baseline, histogram | Where the baseline fills split; where histogram bars start. Must be finite. |
| `lastValueVisible` | `boolean` | `true` | all but volume | Label with the last value on the price axis |
| `priceLineVisible` | `boolean` | `true` | all but volume | Dotted horizontal line at the last value |
| `shift` | `number` | `0` | all | Display shift in bars, see [Shift](#shift) |
| `pricePrecision` | `number \| null` | `null` | all | Decimals of price labels, see [Price precision](#price-precision) |
| `markerStackLimit` | `number \| null` | `null` | series with markers | Integer 1–65535: places per bar side, including the "+k" badge; `null` means no limit. See [Folding](markers.md#folding-crowded-bars). |
| `markerCluster` | `number \| null` | `null` | series with markers | Positive, finite CSS-pixel distance for cross-bar clustering; `null` disables it. Excludes `atPrice`. See [Clustering](markers.md#clustering-across-bars). |
| `filledBarColor` | `Color \| null` | `null` | candlestick, ohlc, heikinAshi | Color of bars flagged `filled`, drawn hollow; `null` uses the main color at 40% opacity. See [Bar quality flags](data.md#bar-quality-flags). |

Invalid values (an unknown line style, a malformed color, a negative width)
throw, and the series keeps its old options.

Volume series always use the theme's volume colors.

### Colors

Colors are CSS strings: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
`rgb(r, g, b)`, `rgba(r, g, b, a)` (alpha from 0 to 1) or `'transparent'`.
Color names such as `'red'` are not accepted. `null` means the theme default.
`options()` returns colors as `#rrggbb`, or `rgba(...)` when they are
translucent, and `null` for theme colors.

### Step lines

With `step: true` the line runs horizontally to the next point's time, then
vertically to its value. Use it for values that hold until they change, such
as an interest rate or a position size.

```ts
const rate = chart.addSeries('line', { step: true, color: '#7e57c2' }, pane);
rate.setData({
  time: [Date.UTC(2024, 0, 1), Date.UTC(2024, 2, 20), Date.UTC(2024, 6, 31)],
  value: [5.5, 5.25, 5.0],
});
```

### Shift

`shift` draws the point at logical index `j` at `j + shift`: positive values
move the series to the right (into the future), negative values to the left.
It must be an integer. Auto-scaling, the crosshair and `dataAt` follow the
drawn positions. Ichimoku's leading spans and lagging line use it.

```ts
// A moving average drawn 10 bars ahead (a "displaced" moving average).
const dma = chart.addSeries('line', { shift: 10 });
```

### Price precision

`pricePrecision` sets the number of decimals of the price axis labels, the
last-value label and the crosshair label, from 0 to 12. `null` (the default)
is automatic: at least 2 decimals and 4 significant digits of the visible
prices. Very small prices are written with a subscript count of zeros:
`0.0₄1234` is 0.00001234.

The labels of a pane use the precision of its first price series. In the
`'percentage'` and `'indexedTo100'` price scale modes the axis always uses 2
decimals. Any other value than an integer from 0 to 12 or `null` throws.

```ts
candles.applyOptions({ pricePrecision: 5 }); // FX quotes
candles.applyOptions({ pricePrecision: null }); // automatic again
```

## Bar transforms

A transform series takes ordinary bars (or ticks) and draws what a charting
method makes of them: Renko bricks, range bars, Kagi lines or Point & Figure
columns. These methods ignore time; each element gets its own slot on the time
axis.

```ts
const renko = chart.addTransformSeries('renko', { boxSize: 0.5 });
renko.setData(randomWalk(1, 5_000, Date.UTC(2024, 0, 1), 60_000));
chart.addVolumeSeries(renko); // optional
chart.fitContent();
```

`addTransformSeries(name, params, options?, pane?)` returns a `Series`. Feed
it with `setData` and `update` exactly like a candlestick series. A tick is a
bar with four equal prices:

```ts
renko.update({ time: tick.time, open: tick.price, high: tick.price, low: tick.price, close: tick.price, volume: tick.size });
```

### Parameters

Prices are in the units of the data.

| Transform | Parameters | Drawn as |
|---|---|---|
| `'renko'` | `boxSize` (price) **or** `atr` (ATR period, whole number); `kind?` | Candlesticks (default) or OHLC bars |
| `'rangeBars'` | `range` (price); `kind?` | Candlesticks (default) or OHLC bars |
| `'kagi'` | `reversal` (price) **or** `reversalPercent` | Kagi lines |
| `'pointFigure'` | `boxSize` (price); `reversal?` (boxes, whole number, default 3) | X and O columns |

`kind` is `'candlestick'` (default) or `'ohlc'`. Numbers must be finite and
positive. Giving both alternatives (`boxSize` and `atr`, or `reversal` and
`reversalPercent`), neither, or an unknown name throws.

```ts
chart.addTransformSeries('renko', { atr: 14, kind: 'ohlc' });
chart.addTransformSeries('rangeBars', { range: 1 });
chart.addTransformSeries('kagi', { reversalPercent: 2 });
chart.addTransformSeries('pointFigure', { boxSize: 1, reversal: 3 });
```

(Each of these lines needs a chart of its own, see
[Constraints](#constraints).)

### Renko

Renko uses closing prices only. A brick is added each time the close moves a
full `boxSize` beyond the last brick: continuing in the same direction takes
one box, reversing takes two. The first brick is anchored at the first close.

With `atr: n` the box size is the average true range of the first `n` bars,
fixed from then on; nothing is drawn before that.

Renko has no forming brick: a brick appears only once it is complete.

### Range bars

Every bar spans exactly `range` from low to high. Within an input bar the
price is assumed to move open → low → high → close for an up bar and open →
high → low → close for a down bar. When the price leaves the current bar's
range, that bar is completed and the next one opens at its edge. The last bar
is forming and changes with updates.

### Kagi

A Kagi line follows the close in its direction and turns when the close
reverses by at least `reversal` (or `reversalPercent` percent of the line's
end price). A line becomes thick (yang, drawn with `color`) when it rises above
the previous shoulder and thin (yin, `downColor`) when it falls below the
previous waist. The last line is forming.

### Point & Figure

Prices are divided into boxes of `boxSize`. A column of X rises while the close
fills new boxes upward; a reversal of `reversal` boxes starts a column of O,
and the other way round. X columns use `color`, O columns `downColor`. The
last column is forming.

### Constraints

- A chart has at most one transform series, and no other series next to it
  except volume. Adding a second transform or any other series throws, and so
  does adding a transform to a chart that already has series. Use a separate
  chart to show the original bars, and keep the two in step with
  [sync](sync.md) if needed.
- Each element's time is the time of the input bar that completed it (one
  millisecond later than the previous element if that would not be
  increasing). `timeAtIndex` and the time axis show these times.
- The input volume of each bar is added to the element forming after it.
  Without input volume, the volume is empty.
- A single input bar can produce at most 65536 elements. If the box or range
  is so small compared with the price moves that a bar exceeds this, the call
  throws and the series is cleared.
- Bars that are gaps (all prices `NaN`) are skipped.

## Bands between two series

`addBand(upper, lower, options?)` fills the area between two single-value
series (line, area, baseline, histogram or points) of the same pane. `fill` is
used where `upper` is at or above `lower`, `fillCrossed` where it is below.
Bands are drawn behind the series of the pane (above the volume).

```ts
const upper = chart.addSeries('line', { color: '#26a69a' });
const lower = chart.addSeries('line', { color: '#ef5350' });
upper.setData({ time, value: highs });
lower.setData({ time, value: lows });

const band = chart.addBand(upper, lower, {
  fill: 'rgba(38, 166, 154, 0.15)',
  fillCrossed: 'rgba(239, 83, 80, 0.15)',
});

chart.setBandOptions(band, { fillCrossed: null }); // keys left out keep their value
console.log(chart.bandOptions(band));               // { fill: 'rgba(...)', fillCrossed: null }
chart.removeBand(band);
```

`addBand` returns a band id. A `null` (or missing) `fill` is the upper series'
color at 15 % opacity; a `null` `fillCrossed` is the same as `fill`.
`bandOptions` returns `null` for an unknown id. The band is removed
automatically with either series. Two OHLC series, series on different panes,
or the same series twice throw.

Indicators with bands (Bollinger, Keltner, Ichimoku) create them for you; see
[Indicators](indicators.md).

## Removing a series

```ts
line.remove();
```

`remove()` deletes the series, its data, markers and price lines, and the
bands that use it. Removing an OHLC series also removes its volume series.
After that, every method of the handle throws (`'the series was removed'`).
Removing the last series with data resets the time axis.

Indicator outputs cannot be removed one by one: `remove()` on one throws;
remove the indicator instead (`indicator.remove()`).

To remove a whole pane with all its series, use `chart.removePane(id)` (see
[Price scale and panes](price-scale.md)).
