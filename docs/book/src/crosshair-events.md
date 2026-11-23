# Crosshair and events

The chart reports what happens on it through events: the crosshair moving, a
click, the visible range changing, drawings being created or edited, and a few
more. This chapter lists every event, its payload and when it fires, and shows
two common uses: a legend that follows the crosshair and reacting to clicks on
markers and series.

## Crosshair modes

The crosshair follows the pointer over a plot. Its vertical line crosses every
pane; the horizontal line and the price label belong to the pane under the
pointer. The time label is on the time axis.

| Mode | Behaviour |
|---|---|
| `'normal'` (default) | x snaps to the nearest bar center, y follows the pointer |
| `'magnet'` | As `'normal'`, and y snaps to the nearest of the bar's open, high, low and close (of the first price series in the pane) |
| `'hidden'` | No crosshair is drawn, and `crosshairMove` reports `null` |

```ts
chart.setCrosshairMode('magnet');
console.log(chart.crosshairMode); // 'magnet'
```

An unknown mode throws. The mode is part of the saved state
(`saveState().crosshairMode`, see [Saving state](state.md)).

The magnet mode also applies while placing drawings: anchors snap to the
nearest open, high, low or close (see [Drawing tools](drawings.md)).

## Subscribing

```ts
import type { ChartEventMap } from '@ferrochart/core';

const onMove = (e: ChartEventMap['crosshairMove']) => {
  console.log(e.point?.time);
};

chart.on('crosshairMove', onMove);
// later
chart.off('crosshairMove', onMove);
```

- `on(name, callback)` adds a listener. An unknown event name throws.
- `off(name, callback)` removes it. Pass the same function object you passed
  to `on`.
- Adding the same function twice calls it twice; `off` removes both.
- Callbacks may call any chart method, including `on` and `off`.
- An exception thrown inside a callback is reported like an uncaught error
  (`reportError`: it appears in the console and reaches `window.onerror`).
  The other callbacks still run.

The callback parameter is typed from the event name, so
`chart.on('crosshairMove', (e) => e.point?.time)` needs no annotation.
`ChartEventMap[name]` (exported by the package) names a payload type when you
declare the callback separately, as above.

## Delivery timing

The chart draws in `requestAnimationFrame`. Each frame, it first applies the
input and API calls made since the last frame, builds and renders the frame,
and then delivers the events that frame produced. Consequences:

- Events are never delivered inside the DOM input handler or inside the method
  call that caused them. `chart.drawings.add(...)` returns the id at once; the
  `drawingCreated` event for it arrives after the next frame.
- At most one frame's worth of events is delivered at a time. If the
  crosshair moved several times since the last frame, you get one
  `crosshairMove` with the latest position, not one per pointer event. The
  same holds for `visibleRangeChanged` and `sync`.
- Browsers pause `requestAnimationFrame` in background tabs, so events are
  delayed until the tab is visible again.

## Lifetime

The chart keeps itself running through listeners on its canvas and the
window, which hold only weak references to it. Keep a reference to the
`FerroChart` object for as long as it is on the page. If the object is
garbage-collected, the chart stops drawing and stops sending events. When you
are done with a chart, call `destroy()`: it removes all event listeners (yours
and the DOM ones) and releases the GPU resources. The object cannot be used
afterwards.

```ts
chart.destroy();
```

## Event reference

| Event | Payload | Fires when |
|---|---|---|
| `click` | `ClickEvent` | A press and release in a plot that moved at most 3 CSS px |
| `crosshairMove` | `{ point: CrosshairPoint \| null }` | The crosshair moved to another bar, pane or price, or left the plot |
| `visibleRangeChanged` | `{ from, to }` | The visible logical range changed |
| `needMoreHistory` | `{ barsBeforeStart }` | The left edge of the view came within one screen of the first bar |
| `drawingCreated` | `{ id }` | A drawing was created |
| `drawingModified` | `{ id }` | A drawing was moved, restyled or had a parameter changed |
| `drawingRemoved` | `{ id }` | A drawing was removed |
| `drawingSelected` | `{ id: number \| null }` | The selected drawing changed |
| `sync` | `{ message: SyncMessage }` | The chart posted a message to its sync group |
| `drawingSync` | `{}` | The chart has drawing sync operations waiting to be taken |
| `warning` | `ChartWarning` | The chart adjusted something on its own |
| `markerHover` | `MarkerHoverEvent \| null` | The pointer moved onto a marker or a "+k" badge, or off it |

### click

```ts
interface ClickEvent {
  x: number;              // CSS px from the canvas' top-left corner
  y: number;
  pane: number;           // pane id
  logicalIndex: number;   // fractional logical index at x
  time: number | null;    // UTC ms of the nearest bar, null beyond the data
  price: number;          // price at y on the pane's price scale
  target: ClickTarget | null;
}

type ClickTarget =
  | { type: 'drawing'; id: number }
  | { type: 'marker'; series: Series; index: number }
  | { type: 'markerOverflow'; series: Series; first: number; count: number }
  | { type: 'series'; series: Series };
```

A click is a press and release (mouse, pen or touch) within a plot during
which the pointer moved at most 3 CSS px; a longer movement is a pan. No click is
sent while a drawing is being created.

`target` is what was under the pointer, checked in this order:

1. **A drawing** (its body or one of its handles): `{ type: 'drawing', id }`.
   Pressing a drawing also selects it.
2. **A marker**: `{ type: 'marker', series, index }`, where `index` is the
   marker's position in `series.markers()` (which is sorted by time). A "+k"
   badge of folded markers gives `{ type: 'markerOverflow', series, first,
   count }`: the markers not drawn are `series.markers().slice(first, first +
   count)` (see [Markers](markers.md)).
3. **A series**: `{ type: 'series', series }` when the pointer is on a
   candle's body or wick, within 4 CSS px of a line, inside a histogram bar or
   on a dot. With overlapping series, the one added last wins.
4. `null` for empty space.

The `Series` objects in a target are new handles to existing series. Compare
them by `id`, not with `===`:

```ts
chart.on('click', (e: ChartEventMap['click']) => {
  if (e.target?.type === 'series' && e.target.series.id === candles.id) {
    console.log('clicked the candles at', e.time);
  }
});
```

Indicator outputs are ordinary series, so clicking an indicator line gives a
`series` target whose `id` matches one of `indicator.outputs()`.

### crosshairMove

```ts
interface CrosshairPoint {
  logicalIndex: number;   // the bar the crosshair is on (whole number)
  time: number | null;    // UTC ms, null beyond the data
  pane: number;           // pane under the pointer
  price: number;          // price at the crosshair; NaN for a synced crosshair
  bar: BarValues | null;  // OHLCV of the first OHLC series with a bar here
  x?: number;             // pointer position, CSS px; absent for a synced crosshair
  y?: number;
}

interface BarValues {
  open: number; high: number; low: number; close: number;
  volume: number | null;
}
```

`point` is `null` when the pointer leaves the plot or the crosshair is hidden.
The event is sent only when something in the point changes; moving the pointer
within one bar and price does not repeat it.

In magnet mode, `price` is the snapped price. `bar` is `null` when no OHLC
series has a bar at that index (for example a chart with only line series, or
a gap). To read other series at the crosshair, use `series.dataAt(point.logicalIndex)`.

A chart in a [sync group](sync.md) also reports the crosshair of the other
charts. Such a point has `price: NaN`, `pane` set to the main pane and no `x`
or `y`.

### visibleRangeChanged

`{ from: number, to: number }` are the logical indices at the left and right
edges of the plot, fractional. Panning, zooming, resizing and the methods that
change the view (`setVisibleLogicalRange`, `fitContent`, `scrollToRealtime`,
`setBarSpacing`, `setRightOffset`) all cause it. The same values are available
at any time from `chart.visibleLogicalRange()`.

### needMoreHistory

`{ barsBeforeStart: number }` is the number of bars between the first bar and
the left edge of the view. It is negative when the view already shows empty
space before the data. The event is sent once per first bar: after you add
older data, it can fire again when the user scrolls to the new beginning.

```ts
let loading = false;
chart.on('needMoreHistory', async (e: ChartEventMap['needMoreHistory']) => {
  if (loading) return;
  loading = true;
  try {
    await loadOlderBars(e.barsBeforeStart); // your code: fetch older bars, then series.prepend(them)
  } finally {
    loading = false;
  }
});
```

See [Data](data.md) for loading data.

### Drawing events

`drawingCreated`, `drawingModified` and `drawingRemoved` carry the drawing
`id`; `drawingSelected` carries the selected id or `null`.

| Event | Caused by |
|---|---|
| `drawingCreated` | The user finishing a drawing, `drawings.add`, an undo or redo that brings a drawing back (with its old id), `loadState` (one per loaded drawing), a drawing received from a sync group |
| `drawingModified` | Dragging a drawing or a handle, `setAnchors`, `setStyle`, `setParam`, an undo or redo of such a change, a synced change |
| `drawingRemoved` | Delete key, `drawings.remove`, an undo of a creation, `loadState` (for the drawings it replaces), removing a pane, a synced removal |
| `drawingSelected` | Pressing a drawing or empty space, finishing a new drawing (it becomes selected), `drawings.select`, hiding or removing the selected drawing |

Showing, hiding, locking and changing the z order of a drawing send no event.
See [Drawing tools](drawings.md).

### sync

`{ message: SyncMessage }` is sent when the chart posted a crosshair or
visible-range message to its [sync group](sync.md):

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }
  | { type: 'visibleRange'; from: number; to: number };
```

Charts in the same `SyncGroup` on one page need nothing from you. The event is
for forwarding messages to charts elsewhere, such as in a worker, which apply
them with `applySync(message)`. A chart outside a group, or one whose group
does not share that kind of message, sends no `sync` events.

### drawingSync

`{}`. Sent when the chart, sharing drawings in a sync group, has drawing
operations that the page has not taken yet. It is sent once; it is sent again
only after you call `chart.takeDrawingSyncOps()`. See [Synchronizing
charts](sync.md).

### warning

The chart fixed something on its own and the call that caused it succeeded.

```ts
type ChartWarning =
  | { kind: 'timeAxisMerged'; series: Series; count: number }
  | { kind: 'ohlcRepaired'; series: Series; count: number }
  | { kind: 'dataDropped'; series: Series }
  | { kind: 'other' };
```

| `kind` | Meaning |
|---|---|
| `'timeAxisMerged'` | The times of `series` fell between the bars of the time axis. The axis was rebuilt as the union of all series' times, and the logical indices after the first inserted bar moved. `count` bars were inserted. |
| `'ohlcRepaired'` | `count` bars of `series` had a high below or a low above their other prices and were widened. (The JavaScript API rejects such bars instead, so a chart driven from JavaScript does not send it.) |
| `'dataDropped'` | The data of `series` could not be moved onto a rebuilt time axis (out of memory) and was cleared. |
| `'other'` | A kind added in a later version. |

```ts
chart.on('warning', (w) => {
  if (w.kind === 'timeAxisMerged') console.warn(`time axis rebuilt: ${w.count} bars inserted by series ${w.series.id}`);
});
```

### markerHover

```ts
interface MarkerHoverEvent {
  series: Series;
  index: number;          // position in series.markers(); for a badge, the first marker not drawn
  overflow: number;       // k for a "+k" badge, 0 for a marker
  x: number;              // center, CSS px from the canvas' top-left corner
  y: number;
  size: number;           // diameter, CSS px
}
```

The payload is `null` when the pointer leaves the marker. Moving from one
marker straight onto another sends only the new one. The marker under the
pointer is found when a frame is drawn, so the event comes at most once per
frame; drawings are not considered, and while the intro animation plays no
marker counts as hovered. Use it for tooltips over avatars or badges (see
[Markers](markers.md)).

## Example: a legend that follows the crosshair

A DOM element over the chart shows the bar under the crosshair, and the
latest bar when the pointer is elsewhere.

```html
<div style="position: relative; height: 480px">
  <canvas id="chart" style="width: 100%; height: 100%"></canvas>
  <div id="legend" style="position: absolute; left: 12px; top: 8px;
       font: 12px sans-serif; pointer-events: none"></div>
</div>
```

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const canvas = document.getElementById('chart') as HTMLCanvasElement;
const legend = document.getElementById('legend') as HTMLDivElement;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars); // your OHLC columns
const sma = chart.addIndicator(candles, 'sma', { n: 20 }).outputs()[0];

const fmt = (v: number | undefined) => (v === undefined || Number.isNaN(v) ? '–' : v.toFixed(2));

function show(index: number) {
  const bar = candles.dataAt(index);
  const avg = sma.dataAt(index);
  if (!bar) {
    legend.textContent = '';
    return;
  }
  const time = chart.timeAtIndex(index);
  const date = time === null ? '' : new Date(time).toISOString().slice(0, 16).replace('T', ' ');
  legend.textContent =
    `${date}  O ${fmt(bar.open)}  H ${fmt(bar.high)}  L ${fmt(bar.low)}  C ${fmt(bar.close)}` +
    `  SMA 20 ${fmt(avg?.value)}`;
}

function showLatest() {
  const range = chart.dataRange();
  if (range) show(range.to);
}

chart.on('crosshairMove', (e: ChartEventMap['crosshairMove']) => {
  if (e.point) show(e.point.logicalIndex);
  else showLatest();
});
showLatest();
```

`dataAt` returns `{ open, high, low, close, volume }` for OHLC series,
`{ value }` for the others, and `null` for no data or a gap. Indicator outputs
are `NaN` during their warm-up bars. When live updates change the latest bar,
call `showLatest()` after `update` if the pointer is not over the chart.

## Example: clicking markers and series

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars);

candles.setMarkers([
  { time: bars.time[120], position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: bars.time[180], position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
]);

chart.on('click', (e: ChartEventMap['click']) => {
  const t = e.target;
  if (!t) {
    console.log(`empty space at ${e.price.toFixed(2)}`);
    return;
  }
  switch (t.type) {
    case 'marker': {
      const marker = t.series.markers()[t.index];
      console.log(`marker "${marker.text}" at`, new Date(Number(marker.time)));
      break;
    }
    case 'series':
      if (t.series.id === candles.id) {
        const bar = candles.dataAt(Math.round(e.logicalIndex));
        console.log('candle', bar);
      }
      break;
    case 'drawing':
      console.log('drawing', t.id, chart.drawings.get(t.id)?.kind);
      break;
  }
});
```

Marker indices refer to the time-sorted list that `series.markers()` returns,
not to the order you passed to `setMarkers`. Keep your own data keyed by time
if you need to find it again. See [Markers and price lines](markers.md).
