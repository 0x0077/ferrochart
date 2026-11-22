# Price scale and panes

A chart is a stack of panes that share the time axis. Each pane has its own
price scale on the right. This chapter covers creating and sizing panes,
placing series on them, and the price scale options.

## Panes

A new chart has one pane, the main pane. `chart.mainPane` is the id of the top
pane, the default for every method that takes an optional pane.

```ts
const main = chart.mainPane;
const volumePane = chart.addPane(0.25); // below the others, a quarter of the main pane's height
const oscPane = chart.addPane(0.3);

console.log(chart.panes());             // [main, volumePane, oscPane], top to bottom
```

| Method | Does |
|---|---|
| `addPane(stretch?)` | Adds a pane below the others and returns its id. `stretch` is its height relative to the other panes (default 1); it must be a finite number above 0, or `addPane` throws. |
| `removePane(pane)` | Removes a pane with its series, drawings and layer plugins. The last pane cannot be removed. |
| `panes()` | Pane ids, top to bottom. |
| `mainPane` | Id of the top pane. |
| `setPaneStretch(pane, stretch)` | Changes a pane's relative height. `stretch` must be finite and positive. |
| `paneStretch(pane)` | A pane's relative height, `undefined` for an unknown id. |

Heights are proportional to the stretch factors: with factors 1, 0.25 and 0.3
the panes get 1/1.55, 0.25/1.55 and 0.3/1.55 of the height available. Users
can drag the separator between two panes, which changes the factors of those
two panes.

Pane ids are not reused after a pane is removed. If you remove the top pane,
the next one becomes the main pane. Apart from `paneStretch`, methods given an
unknown pane id throw.

```ts
chart.setPaneStretch(oscPane, 0.5);
console.log(chart.paneStretch(oscPane)); // 0.5

chart.removePane(oscPane);               // its series go with it
```

Pane layout (the number of panes, their stretch factors and price scale
settings) is part of the saved state; see [Saving state](state.md).

## Putting series on panes

Every method that adds a series takes the pane as its last argument; without
it the series goes on the main pane. `series.pane` tells where a series is.

```ts
const candles = chart.addCandlestickSeries();          // main pane
const sma = chart.addSeries('line', { color: '#2962ff' });
const pane = chart.addPane(0.3);
const rsi = chart.addSeries('line', { color: '#7e57c2' }, pane);
rsi.setData(rsiValues);

console.log(rsi.pane === pane); // true
```

A series stays on its pane; to move it, remove it and add it again on the
other pane. Indicators choose their pane through a placement argument; see
[Indicators](indicators.md).

All series of a pane share its price scale and are auto-scaled together.

### Volume placement

`addVolumeSeries(source, pane?)` behaves differently depending on what else is
on the pane:

- **On a pane with price series** (the default: the main pane, next to the
  candles), the volume has a separate scale of its own and occupies the bottom
  quarter of the pane. It does not affect the price range, and the price axis
  keeps showing prices.
- **On a pane of its own**, the volume uses the pane's price scale and fills
  the pane: the bars stand on the bottom edge (bottom margin 0), and the axis
  labels use K, M and B suffixes (`1.25M`).

```ts
// Volume under the candles, in the same pane.
chart.addVolumeSeries(candles);

// Or in a pane of its own below.
const volPane = chart.addPane(0.25);
chart.addVolumeSeries(candles, volPane);
```

## The price scale

`applyPriceScaleOptions(options, pane?)` changes the price scale of a pane
(default: the main pane); keys you leave out keep their value. It is all or
nothing: if one value is invalid it throws and the scale keeps every option it
had. `priceScale(pane?)` returns all of them.

```ts
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
chart.applyPriceScaleOptions({ margins: { top: 0.2, bottom: 0.1 } }, pane);

const s = chart.priceScale();
// { mode: 'logarithmic', autoScale: true, inverted: false,
//   margins: { top: 0.1, bottom: 0.1 }, range: { low: 98.2, high: 112.7 } }
```

| Option | Type | Default | Meaning |
|---|---|---|---|
| `mode` | `'normal' \| 'logarithmic' \| 'percentage' \| 'indexedTo100'` | `'normal'` | How prices map to heights, see [Modes](#modes) |
| `autoScale` | `boolean` | `true` | Whether the range follows the visible data |
| `inverted` | `boolean` | `false` | Higher prices drawn lower |
| `margins` | `{ top, bottom }` | `{ top: 0.1, bottom: 0.1 }` | Empty space above and below the data, as fractions of the pane height |
| `range` | `{ low, high }` | follows the data | The price range shown (without margins). Setting it turns auto-scaling off. |

`priceScale().range` is always the range currently shown, also while
auto-scaling.

The options are applied in the order `mode`, `margins`, `inverted`, `range`,
`autoScale`. An invalid value throws; options before it in this order have
already been applied.

### Modes

| Mode | Axis shows |
|---|---|
| `'normal'` | Prices on a linear scale |
| `'logarithmic'` | Prices on a logarithmic scale: equal distances are equal ratios. Useful for long histories with large price changes. |
| `'percentage'` | Percent change from the first visible value: (p − base) / \|base\| × 100, labelled with 2 decimals and `%` |
| `'indexedTo100'` | Prices indexed to 100 at the first visible value: p / \|base\| × 100, labelled with 2 decimals |

In `'percentage'` and `'indexedTo100'` mode every series of the pane is
normalized to its own first visible value: the close of the first visible bar,
or the first visible value of a value series. Series with very different price
levels can then be compared on one scale. As the user pans, the base moves
with the first visible bar. The range, axis labels and crosshair price are in
percent or index points.

Switching between a price mode (`'normal'`, `'logarithmic'`) and a relative mode
(`'percentage'`, `'indexedTo100'`) turns auto-scaling on, because the old range
is in other units.

### Auto-scale and fixed ranges

By default the range follows the high and low of the visible data of all
series on the pane (plus the margins), and updates as you pan, zoom or receive
new data. Setting `range` fixes it:

```ts
chart.applyPriceScaleOptions({ range: { low: 90, high: 110 } }); // autoScale is now false
chart.applyPriceScaleOptions({ autoScale: true });               // follow the data again
```

`low` must be below `high`, both finite; in logarithmic mode `low` must be
above 0. In the relative modes the range is in percent or index points.

`range` turns auto-scaling off, so `{ range, autoScale: true }` contradicts
itself and throws; `{ range, autoScale: false }` is fine.

Users can also change the range: dragging the price axis or the wheel over
it (or Alt + wheel in the plot) scales prices, and turns auto-scaling off.
Double-clicking the price axis turns it back on. See
[Gestures and keyboard](interaction.md#gestures).

### Margins

`margins` keep the data away from the top and bottom edges of the pane. Both
values must be from 0 up to (not including) 1, and their sum must be below 1.
Both keys are required.

```ts
// Leave the top 30 % free for a legend.
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0.05 } });
```

### Inverted

`inverted: true` flips the scale: higher prices are drawn lower. This is
sometimes used for bond yields against prices, or for an inverse pair.

```ts
chart.applyPriceScaleOptions({ inverted: true });
```

## Example: comparing two symbols

Two line series on one pane in percentage mode show relative performance from
the left edge of the view.

```ts
const a = chart.addSeries('line', { color: '#2962ff' });
const b = chart.addSeries('line', { color: '#ff6d00' });
a.setData({ time, value: closesA }); // around 180
b.setData({ time, value: closesB }); // around 0.95

chart.applyPriceScaleOptions({ mode: 'percentage' });
```

## Example: a three-pane layout

Candles with a moving average on the main pane, volume below, an oscillator at
the bottom.

```ts
const candles = chart.addCandlestickSeries();
candles.setData(bars);

const volPane = chart.addPane(0.2);
chart.addVolumeSeries(candles, volPane);

const oscPane = chart.addPane(0.25);
const osc = chart.addSeries('histogram', { color: '#26a69a' }, oscPane);
osc.setData({ time: bars.time, value: oscillator });

// Fixed range for the oscillator, a little room at the top of the main pane.
chart.applyPriceScaleOptions({ range: { low: -1, high: 1 } }, oscPane);
chart.applyPriceScaleOptions({ margins: { top: 0.15, bottom: 0.1 } });
```
