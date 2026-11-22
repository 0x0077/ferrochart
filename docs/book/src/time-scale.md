# Time scale and viewport

The time scale decides which bars are visible and how wide they are. It is
shared by all panes. This chapter covers bar spacing, the right offset, the
visible range, converting between pixels, logical indices and times, and the
events that report view changes.

## Logical indices

The time scale works in logical indices, not times. After `setData` the first
bar has index 0, the next 1, and so on; bar `i` is centered on `i` and covers
`i − 0.5` to `i + 0.5`. Positions between bars and beyond the data have
fractional or out-of-range indices. Use `timeAtIndex` and `indexOfTime` to
convert to and from times (see [Data](data.md#reading-data-back)).

Indices count bars, not time: a weekend or a trading halt with no bars takes
no space.

## Bar spacing

The bar spacing is the distance between the centers of two neighboring bars,
in CSS pixels. Zooming changes it.

```ts
console.log(chart.barSpacing); // 6 on a new chart
chart.setBarSpacing(12);        // zoom in
```

| | Value |
|---|---|
| Default | 6 CSS px |
| Smallest | 0.5 CSS px, or less when needed to show all data at once (90 % of the plot width divided by the number of bars) |
| Largest | `maxBarSpacing`, default 50 CSS px |

`setBarSpacing` clamps the value to this range. Values that are not finite and
positive are ignored. The bar at the right edge stays in place.

The largest spacing is set when the chart is created:

```ts
const chart = await FerroChart.create(canvas, { maxBarSpacing: 200 });
```

`maxBarSpacing` must be a number of at least 1, or `create` rejects. Raise it
when single bars need room: footprint numbers need more than 60 px, TPO
letters several hundred (see [Order flow](orderflow.md)).

## Right offset

The right offset is the empty space between the last bar and the right edge
of the plot, in bars. It can be fractional and negative (the last bar is then
beyond the right edge, which is how a view of older data is described).

```ts
console.log(chart.rightOffset); // 3 on a new chart
chart.setRightOffset(10);        // more room for the forming bar
chart.scrollToRealtime();        // back to the latest bar, with the default offset of 3
```

`setRightOffset` keeps at least one bar on screen; values outside that are
clamped, and values that are not finite are ignored.

The view is anchored to the latest bar by the right offset. When `update` or
`setData` adds bars at the end, the right offset stays the same, so the content
moves left by one bar per new bar. While the user looks at the latest data,
new bars scroll into view; when they have scrolled back, the bars they look at
move left too. `scrollToRealtime()` brings the latest bar back.

## The visible range

`visibleLogicalRange()` returns the logical indices at the left and right edges
of the plot, as fractional numbers:

```ts
const { from, to } = chart.visibleLogicalRange();
const visibleBars = to - from;
```

`setVisibleLogicalRange(from, to)` shows exactly that range across the plot,
as far as the spacing limits allow (if they don't, the right edge is kept):

```ts
// The last 100 bars.
const data = chart.dataRange();
if (data) chart.setVisibleLogicalRange(data.to - 99.5, data.to + 0.5);
```

`from` must be below `to`, both finite; otherwise it throws and the view is
unchanged. You can call it right after `create`, before the first frame:
the range is applied as soon as the plot width is known. Panning or zooming
before that cancels it.

`fitContent()` shows all bars, from the left edge to the right edge, with no
right offset. With few bars, the largest bar spacing may leave space on the
left. Like `setVisibleLogicalRange`, it waits for the plot width.

`dataRange()` returns the indices of the first and last bar (`null` without
data), for computing ranges relative to the data:

```ts
const data = chart.dataRange();
if (data) {
  // The first 200 bars.
  chart.setVisibleLogicalRange(data.from - 0.5, data.from + 199.5);
}
```

### A range by time

Convert times to indices with `indexOfTime`. It needs a bar with exactly that
time; to find the nearest bar for an arbitrary time, search your own copy of
the times.

```ts
function showTimes(fromMs: number, toMs: number) {
  const a = chart.indexOfTime(fromMs);
  const b = chart.indexOfTime(toMs);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a - 0.5, b + 0.5);
}
```

## Pixels, indices and times

| Method | Converts |
|---|---|
| `logicalToCoordinate(index)` | Logical index → x of the bar's center, CSS px from the canvas' left edge |
| `coordinateToLogical(x)` | x (CSS px from the canvas' left edge) → logical index, fractional |
| `timeAtIndex(index)` | Logical index → time (UTC ms) of that bar, `null` outside the data; rounds to the nearest bar |
| `indexOfTime(time)` | Time → logical index of the bar with exactly that time, `null` if there is none |

The coordinates refer to the layout of the last frame, so use them after the
first frame (for example in an event handler). x coordinates are the same in
every pane.

```ts
// The bar under a point of the canvas.
canvas.addEventListener('dblclick', (e) => {
  const rect = canvas.getBoundingClientRect();
  const index = Math.round(chart.coordinateToLogical(e.clientX - rect.left));
  const time = chart.timeAtIndex(index);
  if (time !== null) console.log(new Date(time).toISOString(), candles.dataAt(index));
});

// Position an HTML element over a bar.
function placeOverBar(el: HTMLElement, time: number) {
  const index = chart.indexOfTime(time);
  if (index === null) return;
  el.style.left = `${chart.logicalToCoordinate(index)}px`;
}
```

For the crosshair and clicks, the events already carry the logical index,
time and price; see [Crosshair and events](crosshair-events.md).

## The visibleRangeChanged event

`visibleRangeChanged` is sent whenever the visible logical range changes: by a
gesture, by one of the methods above, by a resize, or by data that moves the
view.

```ts
chart.on('visibleRangeChanged', ({ from, to }) => {
  const a = chart.timeAtIndex(Math.ceil(from));
  const b = chart.timeAtIndex(Math.floor(to));
  label.textContent = a !== null && b !== null
    ? `${new Date(a).toISOString()} – ${new Date(b).toISOString()}`
    : '';
});
```

Events are delivered after the frame that produced them, at most once per
frame: during a fast pan you get the latest range, not every intermediate one.
Nothing is sent while the chart has no data. Remove a listener with
`chart.off('visibleRangeChanged', callback)`.

The same frame may also send `needMoreHistory` when the view comes within one
screen of the first bar; see [Loading older history](data.md#loading-older-history).

### Keeping the view across a reload

Loading older bars with `prepend` keeps every logical index and the view.
`setData` numbers the bars from 0 again, so logical indices change; times do
not. To restore a view after replacing the data (another symbol with the same
timestamps, say), save it as times:

```ts
function saveView() {
  const { from, to } = chart.visibleLogicalRange();
  return { from: chart.timeAtIndex(from), to: chart.timeAtIndex(to) };
}

function restoreView(view: { from: number | null; to: number | null }) {
  if (view.from === null || view.to === null) return chart.fitContent();
  const a = chart.indexOfTime(view.from);
  const b = chart.indexOfTime(view.to);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a, b);
}
```

To save the bar spacing and right offset together with drawings and pane
layout, use `saveState` (see [Saving state](state.md)). To keep several charts
on the same range, see [Syncing charts](sync.md).

## User gestures

These inputs change the time scale (see [Gestures and keyboard](interaction.md)
for the full table):

| Input | Effect |
|---|---|
| Drag in the plot | Pan (release while moving: kinetic scrolling) |
| Wheel / trackpad scroll | Zoom time around the pointer |
| Horizontal scroll or Shift + wheel | Pan |
| Pinch (trackpad, touch) | Zoom |
| Drag the time axis | Change the bar spacing |
| Double-click the time axis | Reset the bar spacing |
| ← → | Pan by 10 % |
| ↑ ↓ | Zoom |
| Home / End | Jump to the first / latest bar |

Zooming keeps the bar under the pointer in place. Every change sends
`visibleRangeChanged`.
