# Drawing tools

Drawings are objects placed on the chart by the user or by code: trend lines,
rectangles, Fibonacci tools, positions, patterns, text and more. Each drawing
has anchors in data space (time and price), so it stays attached to the bars
when the chart is panned, zoomed or resized. FerroChart handles creation,
selection, dragging, deletion and undo/redo itself; your page only arms a tool
and, if it wants, reacts to the drawing events.

## The Drawings object

All drawing methods are on `chart.drawings`:

```ts
const drawings = chart.drawings;

drawings.startTool('trend_line');   // the next clicks draw a trend line
const id = drawings.add({           // or add one from code
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 2), price: 101.5 }],
});
drawings.setStyle(id, { line: '#ef5350', lineWidth: 2 });
drawings.undo();
```

Drawings are identified by numeric ids. Ids are never reused within a chart;
an undo that brings a drawing back restores its old id.

| Method / property | What it does |
|---|---|
| `kinds()` | Kinds that `add` and `startTool` accept (the 29 built-in ones plus registered ones) |
| `startTool(kind)` / `cancelTool()` / `activeTool` | Interactive creation |
| `add(data, pane?)` | Adds a drawing; returns its id |
| `get(id)` | The drawing's `DrawingData` (kind, anchors, style, params), `null` for an unknown id |
| `ids()` | All ids, bottom to top |
| `paneOf(id)` | The pane id of a drawing |
| `setAnchors(id, anchors)` | Moves the anchors |
| `setStyle(id, style)` | Changes some style keys |
| `setParam(id, key, value)` | Sets a tool parameter (`text`, `levels`) |
| `select(id)` / `selected` | Selection |
| `setVisible(id, visible)` / `isVisible(id)` | Show or hide |
| `setLocked(id, locked)` / `isLocked(id)` | Lock against dragging |
| `bringToFront(id)` / `sendToBack(id)` | Z order |
| `remove(id)` | Removes a drawing |
| `undo()` / `redo()` / `canUndo` / `canRedo` / `clearHistory()` | History |
| `register(kind, tool)` | Adds a drawing tool written in JavaScript |

Methods that take an id throw for an unknown id (except `get`, `paneOf`,
`isVisible` and `isLocked`, which return `null` or `undefined`). Drawings are
saved and restored with the chart state (see [Saving state](state.md)).

## Built-in kinds

Kind names are snake_case strings.

**Lines**

| Kind | Anchors | Notes |
|---|---|---|
| `trend_line` | 2 | Segment between the anchors |
| `ray` | 2 | From the first anchor through the second to the plot edge |
| `extended_line` | 2 | Through both anchors, to both plot edges |
| `horizontal_line` | 1 | Across the plot at the anchor's price, with a price label at the right edge |
| `horizontal_ray` | 1 | From the anchor to the right edge, with a price label |
| `vertical_line` | 1 | Across the plot at the anchor's time |
| `arrow` | 2 | Segment with an arrow head at the second anchor |

**Shapes and text**

| Kind | Anchors | Notes |
|---|---|---|
| `parallel_channel` | 3 | Line through the first two anchors and a parallel copy through the third; filled, with a dashed middle line |
| `rectangle` | 2 | Opposite corners; filled |
| `ellipse` | 2 | Inscribed in the rectangle of the two anchors; filled |
| `text` | 1 | Text starting at the anchor. Parameter `text` (default `'Text'`). Drawn on a `fill`-colored background when the fill is not transparent |
| `callout` | 2 | A text box at the second anchor with a leader line to the first. Parameter `text` (default `'Callout'`). The box is filled with the line color, the text is white |
| `path` | 2 or more | Polyline; one click per point, finished with a double click or Enter |
| `brush` | 2 or more | Freehand line: press, drag and release. No handles; it is moved as a whole |

**Measurements and positions**

| Kind | Anchors | Notes |
|---|---|---|
| `price_range` | 2 | Shaded range with the price change and percent change from the first anchor to the second |
| `date_range` | 2 | Shaded range with the number of bars and the duration |
| `long_position` | 3 (2 clicks) | Entry (first anchor), target (second) and stop (third). The stop mirrors the target around the entry when created. Profit and loss areas, labels with prices and percentages, and the reward/risk ratio |
| `short_position` | 3 (2 clicks) | As `long_position`, with percentages for a short trade |

**Fibonacci and Gann**

| Kind | Anchors | Notes |
|---|---|---|
| `fib_retracement` | 2 | A level line at `p2 + (p1 − p2) · level` for each level, labelled `level (price)`, with alternating shaded bands. Parameter `levels` (default `[0, 0.236, 0.382, 0.5, 0.618, 0.786, 1]`) |
| `fib_extension` | 3 | Levels at `p3 + (p2 − p1) · level`, drawn from the third anchor. Parameter `levels` (default `[0, 0.382, 0.618, 1, 1.272, 1.618, 2, 2.618]`) |
| `fib_time_zones` | 2 | Vertical lines at bar `i1 + (i2 − i1) · F` for the Fibonacci numbers F = 0, 1, 2, 3, 5, 8, … |
| `gann_fan` | 2 | Rays from the first anchor with slopes 1/8 … 8 times that of the line to the second anchor, labelled `1x8` … `8x1` |
| `pitchfork` | 3 | Andrews' pitchfork: median line from the first anchor through the midpoint of the other two, with parallel tines through them; filled between the tines |
| `schiff_pitchfork` | 3 | As `pitchfork`, with the origin moved halfway towards the second anchor in price |
| `modified_schiff_pitchfork` | 3 | As `pitchfork`, with the origin at the midpoint of the first two anchors |

**Patterns and waves**

| Kind | Anchors | Notes |
|---|---|---|
| `xabcd_pattern` | 5 | Points X, A, B, C, D with the retracement ratios between the legs; the XAB and BCD triangles are filled |
| `head_and_shoulders` | 7 | Labels for left shoulder, head and right shoulder, and the neckline through the third and fifth anchors; filled |
| `elliott_impulse` | 6 | Waves labelled 0 to 5 |
| `elliott_correction` | 4 | Waves labelled 0, A, B, C |

`p1`, `p2`, `p3` are the anchors' prices in order. Labels of patterns and
waves go above peaks and below troughs.

## Creating drawings interactively

```ts
chart.drawings.startTool('rectangle');
```

`startTool(kind)` arms a tool. The cursor becomes a crosshair over the plots,
and the next press on any pane starts a drawing there. An unknown kind
throws. Arming a tool cancels a drawing in progress.

How the drawing is completed depends on the kind:

- **Fixed number of anchors** (most kinds): the first press places the first
  anchor; the next anchor follows the pointer until you click again, and so
  on until the last anchor is placed. One-anchor kinds (`horizontal_line`,
  `horizontal_ray`, `vertical_line`, `text`) are finished by the first press.
  The first two anchors can also be placed by pressing, dragging and
  releasing.
- **Open** (`path`): one click per anchor. A double click or Enter finishes
  it (the duplicate point of the double click is dropped). Finishing needs at
  least two anchors.
- **Freehand** (`brush`): press, drag and release. An anchor is added every
  3 CSS px of movement. A stroke with fewer than two anchors is dropped.

While placing anchors, x snaps to the nearest bar (times past the last bar are
extrapolated), and in magnet crosshair mode y snaps to the nearest open, high,
low or close. Freehand anchors are not snapped.

When a drawing is finished, it is selected, the `drawingCreated` event is
sent, and **the tool is disarmed**: one `startTool` creates one drawing. To
keep drawing the same kind, arm it again in the `drawingCreated` listener (see
the toolbar example below).

| Key | While creating |
|---|---|
| Escape | Cancels the drawing in progress (a `path` with at least two anchors is finished instead), or disarms an armed tool |
| Enter | Finishes a `path` |

`cancelTool()` disarms the tool and drops a drawing in progress, without an
event. `activeTool` is the armed kind, or the kind of the drawing being
created; it is `undefined` otherwise.

No `click` events are sent while a drawing is being created.

## Selecting and editing

Without an armed tool:

- **Pressing a drawing** selects it (`drawingSelected`). Its handles appear:
  one per anchor for most kinds; positions have handles for the entry, target,
  stop and right edge; horizontal and vertical lines have one in the middle of
  the plot; brushes have none.
- **Dragging** the drawing moves it: whole bars horizontally, following the
  pointer vertically. Dragging a handle moves that anchor. A finished drag
  sends `drawingModified`. Escape during a drag puts the drawing back.
- **Pressing empty space** clears the selection.
- The selected drawing's handles are hit first, then the selected drawing,
  then the other drawings from the top.
- Hidden drawings cannot be hit. Rectangles, ellipses and channels are hit on
  their outline, or anywhere inside when their fill is not transparent.

A press on a drawing is not a pan. A press on a locked drawing selects it and
then pans the chart.

### Keyboard

The canvas takes keyboard input when it has focus; it is focused when pressed.

| Key | Effect |
|---|---|
| Delete | Removes the selected drawing (locked ones too) |
| Escape | Cancels creation, disarms the tool, or undoes the current drag |
| Enter | Finishes a `path` being created |
| Ctrl+Z / ⌘+Z | Undo |
| Ctrl+Shift+Z / ⌘+Shift+Z, Ctrl+Y / ⌘+Y | Redo |

Backspace does nothing. To delete with another key, listen on the canvas
yourself:

```ts
canvas.addEventListener('keydown', (e) => {
  const id = chart.drawings.selected;
  if (e.key === 'Backspace' && id != null) chart.drawings.remove(id);
});
```

## Adding drawings from code

```ts
const id = chart.drawings.add({
  kind: 'fib_retracement',
  anchors: [
    { time: Date.UTC(2024, 0, 10), price: 120 },
    { time: Date.UTC(2024, 1, 20), price: 95 },
  ],
  style: { line: '#787b86', fill: 'rgba(120, 123, 134, 0.15)' },
  params: { levels: [0, 0.382, 0.5, 0.618, 1] },
});
```

```ts
interface DrawingData {
  kind: string;
  anchors: { time: number | bigint; price: number }[];
  style?: DrawingStyle;
  params?: Record<string, number | boolean | string | number[]>;
}
```

- `time` is UTC milliseconds (an integer). It need not be the time of a bar.
- `price` is in the units of the pane's first price series and must be finite.
- The number of anchors must match the kind: exactly as in the table for most
  kinds; 2 or 3 for positions (with 2, the stop is added as the mirror of the
  target); 2 to 4096 for `path` and `brush`.
- `pane` defaults to the main pane. Pass a pane id to draw on another pane.
- Missing style keys take the defaults below; parameter keys a kind does not
  use are ignored.

`add` throws, and adds nothing, for an unknown kind, too few or too many
anchors, a non-finite price, an unknown pane, an invalid style or an invalid
parameter. A drawing added from code is not selected.

`get(id)` returns the saved form with every key: `kind`, `anchors`, the full
`style`, and `params` with the kind's parameters (`text` for `text` and
`callout`, `levels` for the Fibonacci retracement and extension). Pass it to
`add` to copy a drawing:

```ts
const copy = chart.drawings.get(id);
if (copy) chart.drawings.add(copy, chart.drawings.paneOf(id));
```

`setAnchors(id, anchors)` moves a drawing. It needs the same number of anchors
the drawing has (3 for a finished position).

## Style

```ts
chart.drawings.setStyle(id, { line: '#26a69a', lineWidth: 2, lineDash: 'dashed' });
```

| Key | Type | Default | Used for |
|---|---|---|---|
| `line` | `Color` | `#2962ff` | Lines and outlines; label backgrounds of ranges and positions; callout boxes |
| `lineWidth` | `number` | `1` | CSS px, greater than 0 |
| `lineDash` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | Dash pattern of lines |
| `fill` | `Color` | `#2962ff` at 16% opacity | Areas: channels, rectangles, ellipses, ranges, Fibonacci bands, pitchforks, pattern triangles, the text background |
| `text` | `Color` | `#2962ff` | Text and labels without a background |
| `fontSize` | `number` | `12` | CSS px, texts and labels |

`setStyle` changes only the keys you pass. Colors cannot be `null`; use
`'transparent'` for no fill. The profit and loss areas of positions have fixed
green and red colors.

## Parameters

```ts
chart.drawings.setParam(textId, 'text', 'Breakout');
chart.drawings.setParam(fibId, 'levels', [0, 0.5, 1, 1.618]);
```

| Parameter | Kinds | Value |
|---|---|---|
| `text` | `text`, `callout` | A string |
| `levels` | `fib_retracement`, `fib_extension` | Up to 24 finite numbers |

A value of the wrong type throws and leaves the drawing unchanged. Style and
parameter changes send `drawingModified` and can be undone.

## Visibility, locking and order

```ts
chart.drawings.setVisible(id, false);   // hidden drawings are not drawn or hit
chart.drawings.setLocked(id, true);     // can be selected and deleted, not dragged
chart.drawings.bringToFront(id);
chart.drawings.sendToBack(id);
console.log(chart.drawings.isVisible(id), chart.drawings.isLocked(id), chart.drawings.ids());
```

Hiding the selected drawing clears the selection. Locking does not prevent
`setAnchors`, `setStyle` or `remove` from code. Visibility, locking and order
send no events and are not part of the undo history; they are saved with the
chart state.

## Selection

```ts
chart.drawings.select(id);      // selects it and shows its handles
chart.drawings.select(null);    // clears the selection
const sel = chart.drawings.selected; // the selected id, or undefined
```

## Removing

```ts
chart.drawings.remove(id);
for (const d of chart.drawings.ids()) chart.drawings.remove(d); // remove all
```

Removing a pane removes its drawings; if it had any, the undo history is
cleared.

## Undo and redo

These changes are recorded, whether made by the user or by code: adding,
removing, moving (dragging, `setAnchors`), and changing the style or a
parameter. Selection, visibility, locking and z order are not.

```ts
chart.drawings.undo();          // false if there was nothing to undo
chart.drawings.redo();
if (chart.drawings.canUndo) { /* enable the undo button */ }
chart.drawings.clearHistory();
```

The history keeps the last 256 changes. A new change clears the redo list.
`undo` and `redo` cancel a drawing in progress. Loading a state
(`loadState`) clears the history. Drawings received from a sync group are not
recorded.

## Drawing events

| Event | Payload | When |
|---|---|---|
| `drawingCreated` | `{ id }` | A drawing was finished by the user, added from code, restored by undo/redo or loaded |
| `drawingModified` | `{ id }` | It was moved, restyled or had a parameter changed |
| `drawingRemoved` | `{ id }` | It was removed |
| `drawingSelected` | `{ id: number \| null }` | The selection changed |

Events arrive after the frame in which the change happened, not inside the
method call. See [Crosshair and events](crosshair-events.md).

```ts
chart.on('drawingCreated', (e: ChartEventMap['drawingCreated']) => {
  const d = chart.drawings.get(e.id);
  console.log('new', d?.kind, d?.anchors);
});
```

## Example: a toolbar

```html
<div id="toolbar">
  <button data-kind="trend_line">Trend</button>
  <button data-kind="horizontal_line">Horizontal</button>
  <button data-kind="rectangle">Rectangle</button>
  <button data-kind="fib_retracement">Fib</button>
  <button data-kind="long_position">Long</button>
  <button data-kind="path">Path</button>
  <button data-kind="brush">Brush</button>
  <button data-kind="text">Text</button>
  <label><input id="sticky" type="checkbox"> Keep drawing</label>
  <button id="lock">Lock</button>
  <button id="delete">Delete</button>
  <button id="undo">Undo</button>
  <button id="redo">Redo</button>
</div>
<canvas id="chart" style="width: 100%; height: 480px"></canvas>
```

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const canvas = document.getElementById('chart') as HTMLCanvasElement;
const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars);

const d = chart.drawings;
const toolbar = document.getElementById('toolbar') as HTMLDivElement;
const sticky = document.getElementById('sticky') as HTMLInputElement;
const button = (id: string) => document.getElementById(id) as HTMLButtonElement;
let lastKind: string | null = null;

function refresh() {
  const active = d.activeTool ?? null;
  toolbar.querySelectorAll<HTMLButtonElement>('button[data-kind]').forEach((b) => {
    b.classList.toggle('active', b.dataset.kind === active);
  });
  const sel = d.selected;
  button('delete').disabled = sel == null;
  button('lock').disabled = sel == null;
  button('lock').textContent = sel != null && d.isLocked(sel) ? 'Unlock' : 'Lock';
  button('undo').disabled = !d.canUndo;
  button('redo').disabled = !d.canRedo;
}

toolbar.addEventListener('click', (e) => {
  const kind = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-kind]')?.dataset.kind;
  if (!kind) return;
  if (d.activeTool === kind) {
    d.cancelTool();
    lastKind = null;
  } else {
    d.startTool(kind);
    lastKind = kind;
  }
  canvas.focus(); // so that Escape and Enter reach the chart
  refresh();
});

button('delete').onclick = () => {
  const sel = d.selected;
  if (sel != null) d.remove(sel);
};
button('lock').onclick = () => {
  const sel = d.selected;
  if (sel != null) d.setLocked(sel, !d.isLocked(sel));
  refresh();
};
button('undo').onclick = () => d.undo();
button('redo').onclick = () => d.redo();

chart.on('drawingCreated', (_e: ChartEventMap['drawingCreated']) => {
  if (sticky.checked && lastKind && d.activeTool == null) d.startTool(lastKind);
  refresh();
});
chart.on('drawingModified', refresh);
chart.on('drawingRemoved', refresh);
chart.on('drawingSelected', refresh);
// Escape disarms a tool without an event.
canvas.addEventListener('keyup', refresh);
refresh();
```

Undo and redo triggered from the buttons send `drawingCreated`,
`drawingModified` or `drawingRemoved` after the next frame, which refreshes the
buttons.

## Custom tools

`chart.drawings.register(kind, tool)` adds a drawing tool written in
JavaScript. The tool says how it is created (`anchorCount`: a number of
clicks, `{ open: min }` or `'freehand'`) and returns drawing commands from
`build(ctx)`; an optional `hitTest` decides what a press hits. The kind is
available to this chart's `add`, `startTool` and `loadState`; built-in kinds
cannot be replaced. See [Plugins](plugins.md#drawing-tools).
