# Saving and restoring

`chart.saveState()` returns the parts of a chart the user changes by working
with it: the zoom, the scroll position, pane heights, price scale settings, the
crosshair mode and the drawings. `chart.loadState(state)` puts them back. The
state is plain JSON-compatible data, so you can keep it in `localStorage`, a
file or your server.

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

declare const chart: FerroChart;

const state: ChartState = chart.saveState();
localStorage.setItem('chart-layout', JSON.stringify(state));
```

## What a state contains

| Field | Contents |
|---|---|
| `version` | Format version, currently `1` |
| `barSpacing` | Bar spacing in CSS px (the zoom level) |
| `rightOffset` | Empty space right of the last bar, in bars (the scroll position) |
| `crosshairMode` | `'normal'`, `'magnet'` or `'hidden'` |
| `panes` | One entry per pane, top to bottom (below) |
| `drawings` | Every finished drawing, bottom to top (below) |

Each `panes` entry:

| Field | Contents |
|---|---|
| `stretch` | Height relative to the other panes |
| `priceScale.mode` | `'normal'`, `'logarithmic'`, `'percentage'` or `'indexedTo100'` |
| `priceScale.inverted` | Whether higher prices are drawn lower |
| `priceScale.margins` | `{ top, bottom }`, fractions of the pane height |
| `priceScale.range` | `{ low, high }` when auto-scaling is off (the user dragged the price axis, or you set a range); `null` while auto-scaling |

Each `drawings` entry:

| Field | Contents |
|---|---|
| `pane` | Index of the pane, top to bottom (0 is the main pane), not a pane id |
| `visible` | Whether it is shown |
| `locked` | Whether it is locked against editing |
| `data` | `{ kind, anchors, style, params }`, the same as `chart.drawings.get(id)` |

Anchors are stored as time (UTC ms) and price, so drawings stay on the right
bars when the data changes or more history is loaded.

## What it does not contain

The state describes how the user arranged the chart, not what the chart shows.
These are **not** saved:

- data, series and their options;
- indicators, bands, markers and price lines;
- layer plugins and registered drawing tools;
- the theme, renderer and other `FerroChart.create` options;
- sync group membership;
- drawing ids, the selection, the armed tool, a drawing still being created,
  and the undo history.

Your application creates all of these itself, then loads the state on top.
Keep whatever you need to recreate them (the symbol, interval, indicator list
and parameters) next to the state.

## Loading a state

`loadState` needs a chart set up the same way as the one that was saved: in
particular, **the same number of panes**. Panes are matched by position, so
create the series and indicators that add panes (for example `'macd'` or `'rsi'`
with their default placement, or `addPane()`) in the same order before loading.
Load the data first as well; the restored drawings are placed by time.

```ts
import { FerroChart, randomWalk, type ChartState } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;
declare const saved: ChartState;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(randomWalk(1, 2_000, Date.UTC(2024, 0, 1), 60_000));
chart.addVolumeSeries(candles);          // same pane: no new pane
chart.addIndicator(candles, 'macd');     // adds a pane, as before saving

chart.loadState(saved);                  // two panes on both sides
```

What loading does:

- Bar spacing, right offset and crosshair mode are set.
- Each pane gets its stretch and price scale settings. A pane saved with a
  `range` keeps that range with auto-scaling off; a pane saved with
  `range: null` switches auto-scaling back on.
- All current drawings are removed and the saved ones are added. They get new
  ids, and the chart sends `drawingRemoved` and `drawingCreated` events for
  them.
- The undo history is cleared.

### Drawing tools written in JavaScript

A drawing of a kind registered with `chart.drawings.register(kind, tool)` can
only be loaded into a chart where that kind is registered. Register your tools
before calling `loadState`. See [Drawings](drawings.md) and
[Plugins](plugins.md).

## Errors

`loadState` throws an `Error` and leaves the chart unchanged when the state
does not fit. Nothing is applied partially.

| Cause | Example message |
|---|---|
| Not an object | `state must be an object from saveState()` |
| A required field is missing | `state: missing 'barSpacing'` |
| Unknown names | `state: unknown crosshair mode 'x'`, `state: unknown price scale mode 'x'` |
| A newer format version | `state version is newer than supported` |
| A different number of panes | `state has a different number of panes` |
| Bar spacing not finite or not positive, right offset not finite | `invalid bar spacing or right offset` |
| Margins outside 0 to 1 (or adding up to 1 or more); a range that is not finite and increasing, or not positive on a logarithmic scale | `invalid price scale state` |
| A drawing on a pane index that does not exist | `drawing on a missing pane` |
| A drawing the chart cannot build (unknown kind, wrong number of anchors, invalid style or parameters) | depends on the drawing |

A pane `stretch` that is not a positive number is replaced by 1 instead of
being an error.

When a saved state may be stale (for example after you changed the default
indicators of your app), catch the error and fall back to the default layout:

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

declare const chart: FerroChart;

function restore(chart: FerroChart, json: string | null): boolean {
  if (json === null) return false;
  try {
    chart.loadState(JSON.parse(json) as ChartState);
    return true;
  } catch (e) {
    console.warn('saved layout ignored:', e);
    return false;
  }
}

restore(chart, localStorage.getItem('chart-layout'));
```

## A complete example

The page below keeps one layout per symbol in `localStorage`. The setup
function creates everything that is not in the state (series, data,
indicators, their colors), and the state restores the rest. The layout is
saved whenever the user changes a drawing and when the page is hidden.

```ts
import { FerroChart, randomWalk, type ChartState, type Series } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

/** What the app needs to rebuild a chart; kept next to the chart state. */
interface Layout {
  symbol: string;
  indicators: { name: string; params: Record<string, number | string> }[];
  state: ChartState;
}

const key = (symbol: string) => `layout:${symbol}`;

function loadBars(symbol: string) {
  // Replace with your data source.
  const seed = [...symbol].reduce((s, c) => s + c.charCodeAt(0), 0);
  return randomWalk(seed, 5_000, Date.UTC(2024, 0, 1), 60_000);
}

async function openChart(symbol: string): Promise<FerroChart> {
  const chart = await FerroChart.create(canvas, { theme: 'dark' });

  // 1. Series and data (not part of the state).
  const candles: Series = chart.addCandlestickSeries();
  candles.setData(loadBars(symbol));
  chart.addVolumeSeries(candles);

  // 2. Indicators, in the order they were saved, so the panes match.
  const json = localStorage.getItem(key(symbol));
  const saved: Layout | null = json ? (JSON.parse(json) as Layout) : null;
  const indicators = saved?.indicators ?? [{ name: 'sma', params: { n: 50 } }, { name: 'rsi', params: { n: 14 } }];
  for (const { name, params } of indicators) {
    chart.addIndicator(candles, name, params);
  }

  // 3. The saved view, layout and drawings.
  if (saved) {
    try {
      chart.loadState(saved.state);
    } catch (e) {
      console.warn('saved layout ignored:', e);
    }
  }

  // Save after every drawing change and when the page is hidden.
  const save = () => {
    const layout: Layout = { symbol, indicators, state: chart.saveState() };
    localStorage.setItem(key(symbol), JSON.stringify(layout));
  };
  for (const event of ['drawingCreated', 'drawingModified', 'drawingRemoved'] as const) {
    chart.on(event, save);
  }
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') save();
  });
  return chart;
}

await openChart('BTCUSD');
```

Three details in this example matter in practice:

- The indicator list is saved with the state. If the user adds or removes an
  indicator, the number of panes changes, and an old state would no longer
  load.
- `drawingModified` is sent while the user drags a drawing, up to once per
  frame. Writing a small state to `localStorage` that often is fine; if you
  save to a server, debounce the call.
- The zoom and scroll position change without any drawing event, which is why
  the example also saves when the page is hidden. Listen to
  `visibleRangeChanged` if you need to save them sooner.

## Saving drawings only

To keep only the drawings (for example to copy them to another chart with a
different pane layout), read them from the drawings API instead of the state:

```ts
import { FerroChart, type DrawingData } from '@ferrochart/core';

declare const source: FerroChart;
declare const target: FerroChart;

const drawings: Required<DrawingData>[] = source.drawings
  .ids()
  .filter((id) => source.drawings.paneOf(id) === source.mainPane)
  .map((id) => source.drawings.get(id))
  .filter((d): d is Required<DrawingData> => d !== null);

for (const d of drawings) target.drawings.add(d);   // to the main pane
```

To keep drawings in step between open charts instead, see
[Synchronizing charts](sync.md).
