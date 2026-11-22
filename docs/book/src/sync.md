# Synchronizing charts

Charts that join the same `SyncGroup` follow each other: moving the crosshair
over one shows it on the others, and panning or zooming one scrolls the others
to the same time range. Optionally, drawings made on one appear on the others.

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const left: HTMLCanvasElement;
declare const right: HTMLCanvasElement;

const a = await FerroChart.create(left);
const b = await FerroChart.create(right);

const group = new SyncGroup();
a.joinSync(group);
b.joinSync(group);
```

`new SyncGroup()` needs the WebAssembly module. It is loaded by the first
`FerroChart.create`; to create a group before any chart, call `await init()`
first (`import init from '@ferrochart/core'`).

## Time, not bar numbers

Every message between charts carries a **time** (UTC ms), never a logical
index. Charts in a group may therefore show different symbols, different
intervals, or data with gaps in different places:

- A crosshair at 10:37 on a 1-minute chart is shown on a 1-hour chart on the
  10:00 bar (the last bar at or before that time). Past either end of the data
  the time is extrapolated.
- After a pan or zoom, the other charts show the same time span from the left
  edge to the right edge, as far as their bar spacing limits allow (the right
  edge is kept). A 1-hour chart following a 1-minute chart that shows two
  hours would need two bars across the whole plot; with the default maximum
  bar spacing of 50 px it shows more, ending at the same time.

A chart without data ignores received ranges.

## Joining and leaving

### `chart.joinSync(group, options?)`

Joins `group`, leaving the group the chart was in, if any. At its next frame,
the chart takes over the group's current crosshair and visible range.

`SyncOptions` choose what the chart shares. Each option works in both
directions: a chart with `crosshair: false` neither shows other charts'
crosshairs nor sends its own.

| Option | Shares | Default |
|---|---|---|
| `crosshair` | The crosshair time | `true` |
| `visibleRange` | The visible time range (pan and zoom) | `true` |
| `drawings` | The drawings of the main pane | `false` |

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const group: SyncGroup;
declare const overview: FerroChart;
declare const detail: FerroChart;

// The overview shows the crosshair of the others but keeps its own zoom.
overview.joinSync(group, { visibleRange: false });
detail.joinSync(group);
```

To change the options of a chart, call `joinSync` again with the same group.

### `chart.leaveSync()`

Leaves the group. A crosshair this chart showed on the others is removed.
`chart.destroy()` leaves the group as well.

### `chart.syncOptions()`

What the chart shares, with every option filled in, or `null` when it is not
in a group.

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const opts = chart.syncOptions();
if (opts === null) console.log('not synced');
else console.log(opts.crosshair, opts.visibleRange, opts.drawings);
```

## What is sent, and when

- **Crosshair.** When the pointer moves over a chart's plot, the chart sends
  the time under the crosshair; when the pointer leaves, it sends that the
  crosshair is gone. The other charts show a crosshair at that time. Their
  `crosshairMove` events then have a `point` with `price: NaN`, no `x`/`y`,
  and `pane` set to the main pane.
- **Visible range.** When the view changes, by the user (drag, wheel, pinch,
  keys) or by your code (`setVisibleLogicalRange`, `setBarSpacing`,
  `fitContent` and so on), the chart sends the times at its left and right
  edges. If the chart changed its own view in the same frame, that change wins
  over a received range.
- Applying a received message never sends it again, so messages cannot bounce
  between charts.

A chart that sends a message schedules a frame of the other charts in the
group; they apply it when they draw that frame.

## Synchronizing drawings

With `drawings: true`, drawings on the **main pane** of a chart are copied to
the main pane of the other charts that share drawings: adding, moving,
restyling, showing, hiding, locking and removing. Use it for charts of the
same symbol (for example several timeframes), since anchors are prices.

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const group: SyncGroup;
declare const m5: FerroChart;
declare const h1: FerroChart;

m5.joinSync(group, { drawings: true });
h1.joinSync(group, { drawings: true });

// Appears on h1 at its next frame.
m5.drawings.add({
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 2), price: 42_000 }],
});
```

Details:

- A chart that joins with `drawings: true` first takes over the drawings the
  group already has, then shares its own.
- Drawings on other panes are not shared (other panes may show different
  indicators on each chart). Received drawings always go to the main pane.
- Received changes are not added to the receiving chart's undo history. Undo
  and redo on the chart where a change was made are sent like any other
  change, so all charts end up the same.
- The selection and a drawing still being created are not shared.
- Every chart must know the drawing kind. A drawing tool written in JavaScript
  (`chart.drawings.register`) is registered once for all charts of the page.

## Charts that do not share a group object

A `SyncGroup` connects charts created by the same instance of the WebAssembly
module on one page. Charts elsewhere, such as in another window, tab or
iframe, or in a native application, cannot join it. For those, the chart
reports what it sends, and you forward it with any transport you like
(`postMessage`, `BroadcastChannel`, a WebSocket).

The chart that sends must be in a group (a group of one is fine): only a chart
in a group reports its messages.

### Crosshair and visible range

The `sync` event delivers each message this chart sent to its group. On the
other side, `chart.applySync(message)` applies it.

```ts
import { FerroChart, SyncGroup, type SyncMessage } from '@ferrochart/core';

declare const chart: FerroChart;

const channel = new BroadcastChannel('chart-sync');
const group = new SyncGroup();
chart.joinSync(group);

// Out: what this chart sent.
chart.on('sync', ({ message }: { message: SyncMessage }) => {
  channel.postMessage(message);
});

// In: what a chart in another window sent.
channel.onmessage = (e: MessageEvent<SyncMessage>) => {
  chart.applySync(e.data);
};
```

A `SyncMessage` is plain data:

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }       // null: the crosshair left
  | { type: 'visibleRange'; from: number; to: number }; // times at the plot edges, UTC ms
```

`applySync` affects only the chart it is called on; it is neither sent again
nor passed to that chart's own group. On the receiving side, call it on every
chart that should follow.

`applySync({ type: 'visibleRange', from, to })` is also a convenient way to
show a time range on a single chart.

### Drawings

Drawing changes travel as `DrawingSyncOp` values. Each drawing has a `key`
that identifies it across charts:

```ts
type DrawingSyncOp =
  | { type: 'add'; key: number; data: DrawingData; visible: boolean; locked: boolean }
  | { type: 'update'; key: number; data: DrawingData }
  | { type: 'setVisible'; key: number; visible: boolean }
  | { type: 'setLocked'; key: number; locked: boolean }
  | { type: 'remove'; key: number };
```

| Member | Description |
|---|---|
| `drawingSync` event | The chart has sent drawing operations. Sent once; the next one comes after you call `takeDrawingSyncOps()` |
| `takeDrawingSyncOps()` | The operations sent and not yet taken, oldest first. At most 1024 are kept; older ones are dropped |
| `applyDrawingSync(op)` | Applies one operation from another chart (main pane, not undoable, not sent again) |
| `drawingSyncState()` | All synced drawings of this chart as `'add'` operations, for a chart that joins later |
| `loadDrawingSyncState(ops)` | Makes the synced drawings match such a list: adds, updates, and removes synced drawings that are not in it |

Keys are assigned by the group: the group's `origin` times 2³², plus a
counter. When several pages forward drawings to each other, give each page's
group its own origin with `new SyncGroup(origin)`, so keys from different
pages do not collide. The origin is an integer from 0 (the default) to
2²¹ − 1; a larger one throws, which keeps keys exact as JavaScript numbers.

```ts
import { FerroChart, SyncGroup, type DrawingSyncOp } from '@ferrochart/core';

declare const chart: FerroChart;
declare const pageNumber: number; // different on every page, 0 to 2^21 - 1

type Msg =
  | { type: 'ops'; ops: DrawingSyncOp[] }
  | { type: 'hello' }
  | { type: 'state'; ops: DrawingSyncOp[] };

const channel = new BroadcastChannel('chart-drawings');
const group = new SyncGroup(pageNumber);
chart.joinSync(group, { drawings: true });

const send = (m: Msg) => channel.postMessage(m);

// Out: forward this chart's changes.
chart.on('drawingSync', () => send({ type: 'ops', ops: chart.takeDrawingSyncOps() }));

channel.onmessage = (e: MessageEvent<Msg>) => {
  const m = e.data;
  if (m.type === 'ops') {
    for (const op of m.ops) {
      try {
        chart.applyDrawingSync(op);
      } catch (err) {
        console.warn('drawing op skipped:', err);
      }
    }
  } else if (m.type === 'hello') {
    // A page that just opened asks for the current drawings.
    send({ type: 'state', ops: chart.drawingSyncState() });
  } else {
    chart.loadDrawingSyncState(m.ops);
  }
};

send({ type: 'hello' });
```

Errors:

- `applyDrawingSync` throws for an `update`, `setVisible`, `setLocked` or
  `remove` of a key the chart does not have, and for a drawing it cannot build
  (an unregistered kind, wrong anchors or parameters). An `add` of a key the
  chart already has updates that drawing.
- `loadDrawingSyncState` throws, leaving the chart unchanged, if an operation
  is not an `'add'`, a key appears twice, or a drawing cannot be built.

## Example: a multi-timeframe layout

Four charts of one symbol at 1 minute, 5 minutes, 1 hour and 1 day. The three
intraday charts share crosshair, zoom and drawings. The daily chart shows the
crosshair of the others but keeps its own view, so it stays an overview.

```html
<div class="grid">
  <canvas id="m1"></canvas><canvas id="m5"></canvas>
  <canvas id="h1"></canvas><canvas id="d1"></canvas>
</div>
<style>
  .grid { display: grid; grid-template: 1fr 1fr / 1fr 1fr; gap: 4px; height: 100vh; }
  .grid canvas { width: 100%; height: 100%; min-height: 0; }
</style>
```

```ts
import { FerroChart, SyncGroup, randomWalk, type SyncOptions } from '@ferrochart/core';

const MIN = 60_000;
const start = Date.UTC(2024, 0, 1);

const layout: { id: string; interval: number; sync: SyncOptions }[] = [
  { id: 'm1', interval: MIN, sync: { drawings: true } },
  { id: 'm5', interval: 5 * MIN, sync: { drawings: true } },
  { id: 'h1', interval: 60 * MIN, sync: { drawings: true } },
  { id: 'd1', interval: 1_440 * MIN, sync: { visibleRange: false } },
];

const group = new SyncGroup();
const charts: FerroChart[] = [];

for (const { id, interval, sync } of layout) {
  const canvas = document.getElementById(id) as HTMLCanvasElement;
  const chart = await FerroChart.create(canvas, { theme: 'dark' });
  const candles = chart.addCandlestickSeries();
  // Replace with your data; all four cover the same period.
  const bars = Math.round((90 * 1_440 * MIN) / interval);
  candles.setData(randomWalk(7, bars, start, interval));
  chart.addVolumeSeries(candles);
  chart.joinSync(group, sync);
  charts.push(chart);
}

// Show the last two days of the 1-minute chart; m5 and h1 follow it.
const range = charts[0].dataRange();
if (range) charts[0].setVisibleLogicalRange(range.to - 2 * 1_440, range.to);
charts[3].fitContent();
```

The data in this example is random, so the four charts do not show the same
prices; with real data from one symbol they line up.

Notes on this layout:

- `setVisibleLogicalRange` on `charts[0]` changes its view like a pan by the
  user would, so the change is sent to the group and `m5` and `h1` follow.
  (`applySync` would change only `charts[0]`: applied messages are not sent
  on.)
- The daily chart joins with `visibleRange: false`: it neither follows nor
  moves the others, but still shows their crosshair.
- A synced chart redraws when a message arrives or its own content changes;
  idle charts do not redraw.

## Rust

In Rust the same features are `Chart::join_sync(&group, SyncOptions)`,
`Chart::leave_sync`, `Chart::apply_sync`, `ChartEvent::Sync`,
`Chart::take_drawing_sync_ops`, `Chart::apply_drawing_sync`,
`Chart::drawing_sync_state` and `Chart::load_drawing_sync_state`, with
`SyncGroup::new()` and `SyncGroup::with_origin(origin)`.
