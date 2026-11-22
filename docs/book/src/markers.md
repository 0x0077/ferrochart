# Markers and price lines

Markers are small icons with optional text attached to bars, such as buy and
sell signals or news. Price lines are horizontal lines at a fixed price, such
as an order, a stop or a target. Both belong to a series. Each series also has
a last-value label and a last-value line, which are switched on and off with
series options.

## Markers

### Setting markers

```ts
candles.setMarkers([
  { time: Date.UTC(2024, 0, 15), position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: Date.UTC(2024, 1, 2), position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
  { time: Date.UTC(2024, 1, 20), shape: 'circle', text: 'Earnings' },
]);
```

`setMarkers` replaces all markers of the series. To add one, read the current
list, append, and set it again; to clear them, pass an empty array:

```ts
candles.setMarkers([...candles.markers(), { time: Date.UTC(2024, 2, 1), text: 'Split' }]);
candles.setMarkers([]);
```

### Marker fields

| Field | Type | Default | Meaning |
|---|---|---|---|
| `time` | `number \| bigint \| Date` | required | UTC milliseconds (an integer) or a `Date`; `markers()` returns numbers |
| `position` | `'aboveBar' \| 'belowBar'` | `'aboveBar'` | Above the bar's high or below its low. For single-value series, above or below the value. |
| `shape` | `'circle' \| 'square' \| 'arrowUp' \| 'arrowDown'` | `'circle'` | Filled icon |
| `color` | `Color \| null` | `null` | Icon and text color; `null` uses the series' main color (the up color for candles and bars) |
| `text` | `string` | `''` | Text next to the icon, on the side away from the bar; empty for none |

An unknown `position` or `shape`, a missing or non-integer `time`, or an
invalid color throws, and the markers are left as they were.

### Reading markers

`series.markers()` returns the markers sorted by time, with every field filled
in (`color` is `null` where the default is used). The sort is stable: markers
with the same time keep the order you gave them.

The index of a marker in this list is what a `click` event reports as
`target.index` (see [Crosshair and events](crosshair-events.md)). Because the
list is sorted, the index can differ from the position in the array you passed
to `setMarkers`.

### Placement and snapping

- A marker is drawn at the bar whose time equals `time`. A time between two
  bars snaps to the **next** bar; a time after the last bar snaps to the last
  bar. A time before the first bar snaps to the first bar.
- If the series has no value at that bar (a gap), the marker is not drawn.
- Markers follow the series' `shift` option.
- The icon is 10 CSS px across, 4 CSS px away from the bar.
- Several markers on the same bar and side stack away from the bar, in the
  order of `markers()`. A marker with text takes the icon plus one line of
  text.
- The text is centered on the bar, above the icon for `aboveBar` and below it
  for `belowBar`, in the marker's color.
- Markers are clipped to the plot area.

```ts
// Two signals on the same bar: the first sits next to the low, the second below it.
candles.setMarkers([
  { time: t, position: 'belowBar', shape: 'arrowUp', text: 'Entry' },
  { time: t, position: 'belowBar', shape: 'circle', text: 'Alert' },
]);
```

### Which series can have markers

Every series kind except volume series. `setMarkers` on a volume series
throws. Markers can be put on an indicator output too (an `Indicator`'s
`outputs()` are ordinary series):

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];
rsi.setMarkers([{ time: t, position: 'aboveBar', shape: 'square', text: 'OB' }]);
```

### Example: markers from signals

```ts
import type { Marker, OhlcData } from '@ferrochart/core';

// Mark crossings of the close over a 20-bar SMA.
function crossMarkers(bars: OhlcData, sma: number[]): Marker[] {
  const out: Marker[] = [];
  const close = bars.close;
  for (let i = 1; i < close.length; i++) {
    const was = close[i - 1] - sma[i - 1];
    const now = close[i] - sma[i];
    if (Number.isNaN(was) || Number.isNaN(now)) continue;
    if (was <= 0 && now > 0) {
      out.push({ time: bars.time[i], position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' });
    } else if (was >= 0 && now < 0) {
      out.push({ time: bars.time[i], position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' });
    }
  }
  return out;
}

candles.setMarkers(crossMarkers(bars, smaValues));
```

## Price lines

### Creating a price line

```ts
const entry = candles.createPriceLine({
  price: 101.25,
  color: '#2962ff',
  lineWidth: 1,
  lineStyle: 'solid',
  title: 'Entry',
});
const stop = candles.createPriceLine({ price: 98.5, color: '#ef5350', title: 'Stop' });
```

`createPriceLine` returns a `PriceLine`. The price is in the series' units, so
on a percentage or indexed price scale the line moves with the series (see
[Price scale](price-scale.md)).

### Options

| Option | Type | Default | Meaning |
|---|---|---|---|
| `price` | `number` | `0` | Price, in the series' units; must be finite |
| `color` | `Color \| null` | `null` | Line, title and axis label color; `null` uses the series' main color |
| `lineWidth` | `number` | `1` | CSS px, greater than 0 |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'dashed'` | Dash pattern |
| `axisLabelVisible` | `boolean` | `true` | Label with the price on the price axis |
| `title` | `string` | `''` | Text above the line at the right edge of the plot; empty for none |

The axis label uses the series' price format (its `pricePrecision`). Labels on
the price axis are moved apart when they would overlap. A price line that is
outside the visible price range is not drawn.

### Changing and removing

```ts
entry.applyOptions({ price: 101.5 });         // only the keys you pass change
console.log(entry.options());                   // every option: { price, color, lineWidth, ... }
stop.applyOptions({ title: 'Stop (moved)', lineStyle: 'dotted' });

stop.remove();
```

After `remove()`, calling `options`, `applyOptions` or `remove` on that
object throws. Removing the series removes its price lines.

`createPriceLine` throws for a volume series, a non-finite price, an options
argument that is not an object, or an invalid option value.

### Example: a draggable order line

Price lines do not handle input themselves. Combine one with the chart's
events and coordinate conversion, or use a `horizontal_line` drawing, which
the user can drag (see [Drawing tools](drawings.md)):

```ts
const id = chart.drawings.add({
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 1), price: 101.25 }],
  style: { line: '#2962ff' },
});

chart.on('drawingModified', (e: ChartEventMap['drawingModified']) => {
  if (e.id !== id) return;
  const price = chart.drawings.get(id)?.anchors[0].price;
  if (price !== undefined) console.log('order moved to', price);
});
```

## Last value label and line

Each series shows its latest value as a label on the price axis and as a
dotted horizontal line across the plot. Both use the up color when the last
bar closed at or above its open (for baseline series: at or above the base
value) and the down color otherwise; line, area, histogram and points series
use their main color.

| Series option | Default | Meaning |
|---|---|---|
| `lastValueVisible` | `true` | Label with the last value on the price axis |
| `priceLineVisible` | `true` | Dotted line at the last value |
| `pricePrecision` | `null` | Decimals of the label; `null` is automatic |

```ts
const line = chart.addSeries('line', { lastValueVisible: true, priceLineVisible: false });
candles.applyOptions({ priceLineVisible: false });
```

Volume series draw neither. Indicator outputs are created with
`priceLineVisible: false`; their last-value labels are shown. Turn them off
per output:

```ts
for (const s of chart.addIndicator(candles, 'boll').outputs()) {
  s.applyOptions({ lastValueVisible: false });
}
```

Series options are described in [Series](series.md).
