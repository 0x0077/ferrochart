# Markers and price lines

Markers are small shapes with optional text attached to bars, such as buy and
sell signals or news. A marker can also show an image (a wallet or account
avatar) or a short badge such as `DB`, and crowded bars can fold their markers
into a "+k" badge. Price lines are horizontal lines at a fixed price, such
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
| `size` | `number \| null` | `null` | Diameter (side of a square), 4–64 CSS px; `null` for 10 |
| `icon` | `string \| null` | `null` | Name of an image added with `chart.registerIcon`, drawn inside a circle or square |
| `label` | `string` | `''` | Up to 8 characters drawn inside a circle or square, such as `'DB'`; not with `icon` |
| `border` | `Color \| null` | `null` | Ring inside the shape's edge; `null` for none |
| `borderWidth` | `number` | `2` | Ring width, CSS px |

An unknown `position` or `shape`, a missing or non-integer `time`, an
invalid color, a `size` outside 4–64, a `label` longer than 8 characters, or
an `icon` or `label` on an arrow or both on one marker throws, and the markers
are left as they were.

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
- The icon is `size` CSS px across (10 by default), 4 CSS px away from the
  bar.
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

### Images, badges and borders

Register an image once under a name, then use the name in any number of
markers. The source can be a URL (fetched; an image from another origin needs
CORS headers), an `HTMLImageElement`, `ImageBitmap`, `ImageData` or a canvas.
The chart keeps a copy scaled to fit 64 × 64 pixels:

```ts
await chart.registerIcon('whale', '/avatars/whale.png');
await chart.registerIcon('dev', document.querySelector('img#dev') as HTMLImageElement);

candles.setMarkers([
  { time: t1, icon: 'whale', size: 24, border: '#2fbf71' },             // a buy
  { time: t2, icon: 'whale', size: 24, border: '#ef5350' },             // a sell
  { time: t2, position: 'belowBar', label: 'DB', color: '#2962ff', size: 20 },
  { time: t3, position: 'belowBar', label: '狙', color: '#ff9800', size: 20 },
]);
```

- The image fills a circle (clipped round) or a square, over the marker's
  color, so a transparent image shows the color behind it. Arrows cannot have
  an image or a label.
- `registerIcon` returns a promise; wait for it before using the name.
  `setMarkers` with a name that is not registered throws. The promise
  rejects when the image cannot be loaded or read (a failed fetch, an image
  from another origin without CORS headers): fall back to a `label` or a
  plain shape for that marker.
- Registering a name again replaces the image in every marker using it.
  `chart.unregisterIcon(name)` removes it (it returns whether there was one);
  markers that used it then draw the shape alone, and `markers()` reports
  their `icon` as `null`. A chart holds at most 1024 images.
- A `label` is drawn centered in a color that contrasts with the marker's,
  sized to fit: about 0.45 of the diameter for Latin letters and 0.6 for
  Chinese, Japanese or Korean characters, smaller when the text is wider.
- The `border` ring is drawn inside the edge, so it does not change the
  marker's size, for example a green ring for buys and a red one for sells.

### Folding crowded bars

When many markers land on one bar, such as a dozen traders buying in the same
minute, set `markerStackLimit` on the series. At most that many places are
used on each side of a bar; the last one shows "+k" for the k markers not
drawn:

```ts
candles.applyOptions({ markerStackLimit: 4 });   // 3 markers, then "+k"
candles.applyOptions({ markerStackLimit: null }); // no limit (the default)
```

The badge is a circle in the series' main color, as large as the marker it
replaces. Clicking it sends a `click` whose target is
`{ type: 'markerOverflow', series, first, count }`: the markers not drawn are
`series.markers().slice(first, first + count)`, for example to list them in a
popup.

### Hovering markers

The `markerHover` event fires when the pointer moves onto a marker or a "+k"
badge, and with `null` when it leaves. It carries the marker's index, its
center and diameter in CSS px, which is what a tooltip needs:

```ts
chart.on('markerHover', (e) => {
  if (!e) return hideTooltip();
  const list = e.series.markers();
  if (e.overflow > 0) {
    showTooltip(e.x, e.y - e.size / 2, `${e.overflow} more`, list.slice(e.index, e.index + e.overflow));
  } else {
    showTooltip(e.x, e.y - e.size / 2, list[e.index].text ?? '');
  }
});
```

`index` is the position in `series.markers()`; for a badge it is the first
marker not drawn and `overflow` is k (0 for a marker). The hovered marker is
found when a frame is drawn, so the event comes at most once per frame. While
the intro animation plays, no marker counts as hovered. See
[Crosshair and events](crosshair-events.md).

### Chinese, Japanese, Korean and emoji

Marker text and labels can use any script. The bundled font only covers
Latin letters, digits and common symbols; other characters come from:

- **Web**: the browser's own fonts (the system's CJK fonts and color
  emoji), with nothing to configure and nothing added to the package.
- **Desktop, Python and C**: fonts you add, searched in order:
  `WindowOptions { fonts, .. }` with winit, `fc.Chart(fonts=[...])` in
  Python, `ferrochart_add_font` in C. No system fonts are scanned. Fonts that
  are usually present: `/System/Library/Fonts/Hiragino Sans GB.ttc` (macOS),
  `C:/Windows/Fonts/msyh.ttc` (Windows),
  `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc` (Linux, with the
  Noto CJK package); or ship Noto Sans SC/JP/KR with your application.

A character that no font has is drawn as `□`. Natively, color emoji fonts are
not supported: an outline emoji font (Noto Emoji) draws in the text color.
Text is drawn character by character, without shaping: right-to-left scripts
(Arabic, Hebrew) and emoji built from several characters (skin tones, family
and flag sequences) do not come out right.

### Python, C and Rust

The fields are the same in every binding:

```python
red = chart.add_icon(avatar)          # a (height, width, 4) or (height, width, 3) uint8 array
candles.set_markers([
    {"time": t, "icon": red, "size": 24, "border": "#2fbf71"},
    {"time": t, "position": "below_bar", "label": "DB", "color": "#2962ff"},
])
candles.apply_options(marker_stack_limit=4)
chart.on("marker_hover", lambda e: print(e))   # None when the pointer leaves
```

In Python, `add_icon` returns an integer id and `remove_icon(id)` removes the
image. In C, `ferrochart_add_icon` and `ferrochart_set_markers` take the same
fields (see [C, C++ and C#](native.md)). In Rust, `Chart::add_icon` takes an
`IconImage` (straight RGBA, up to 256 × 256) and returns an `IconId` for
`Marker::icon`; `ChartEvent::MarkerHover` and `ClickTarget::MarkerOverflow`
are the event and click target.

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
