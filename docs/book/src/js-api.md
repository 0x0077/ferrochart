# JavaScript API overview

Package `@ferrochart/core`. This page covers loading the module and the
conventions every part of the API follows. The full reference, every class,
method and type, is generated from the package's TypeScript declarations:
[JavaScript API reference](https://ferrochart.com/docs/api).

## Loading

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
```

`FerroChart.create` and `FerroPlot.create` load the WebAssembly module the
first time they are called; concurrent calls share one load. Call the default
export `init` yourself when you need the module without creating a chart
first (`randomWalk`, `new SyncGroup()`, `indicatorNames()`), or to load the
`.wasm` file from another URL:

```ts
import init, { SyncGroup } from '@ferrochart/core';

await init();                                     // next to the module (default)
await init({ module_or_path: '/assets/ferrochart_wasm_bg.wasm' });
const group = new SyncGroup();
```

`init` can be called any number of times; after the first load it returns at
once.

### TypeScript

Declarations are included. They use `Symbol.dispose`, so add
`"ESNext.Disposable"` to `compilerOptions.lib`, or keep the common
`skipLibCheck: true`.

### Bundlers and frameworks

The package is an ES module that fetches its `.wasm` file relative to its own
URL (`new URL('…_bg.wasm', import.meta.url)`), which Vite, webpack 5, Rollup
and esbuild understand. Create charts in the browser only: in React use an
effect, in Vue `onMounted`, and skip server rendering for the component (see
[React and Vue](frameworks.md)).

## Classes

| Class | Created by | What it is |
|---|---|---|
| [`FerroChart`](https://ferrochart.com/docs/api/FerroChart) | `FerroChart.create(canvas, options?)` | A financial chart on a `<canvas>`: panes, series, scales, crosshair, drawings |
| [`Series`](https://ferrochart.com/docs/api/Series) | `chart.addSeries`, `addCandlestickSeries`, `addVolumeSeries`, `addTransformSeries`, order-flow `add*` | One data series |
| [`Indicator`](https://ferrochart.com/docs/api/Indicator) | `chart.addIndicator` | An indicator and its output series |
| [`PriceLine`](https://ferrochart.com/docs/api/PriceLine) | `series.createPriceLine` | A horizontal line at a price |
| [`Drawings`](https://ferrochart.com/docs/api/Drawings) | `chart.drawings` | The chart's drawings and drawing tools |
| [`SyncGroup`](https://ferrochart.com/docs/api/SyncGroup) | `new SyncGroup()` | Links crosshair, visible range and drawings of several charts |
| [`FerroPlot`](https://ferrochart.com/docs/api/FerroPlot) | `FerroPlot.create(canvas, options?)` | A general x/y plot (numbers, categories, dates) |
| [`Mark`](https://ferrochart.com/docs/api/Mark), [`Axis`](https://ferrochart.com/docs/api/Axis), [`Rule`](https://ferrochart.com/docs/api/Rule), [`Label`](https://ferrochart.com/docs/api/Label) | methods of `FerroPlot` | The parts of a plot |

## Conventions

### Time

Times are **UTC milliseconds**. Column data (`time` in `setData`) accepts
`number[]`, `Float64Array` or `BigInt64Array`; everywhere else a single time
(rows, `update`, markers, drawing anchors, `indexOfTime`) can be a `number`, a
`bigint` or a `Date`. Times must be strictly
increasing. Events and getters return times as `number` (or `null` beyond the
data). The time axis labels are in UTC.

### Logical indices

Bars are addressed by a **logical index**: the first bar of the initial data is
0, the next 1, and so on. Bars prepended later get negative indices, so an
index never changes once given. Visible ranges, the crosshair and the
conversion methods (`coordinateToLogical`, `timeAtIndex`, `indexOfTime`)
use them. Fractional values are positions between bar centers.

### Coordinates

Positions are **CSS pixels** from the canvas' top-left corner, as in DOM
events. The canvas is sized by CSS; the chart sets its backing store to the
CSS size × `devicePixelRatio` and follows size changes by itself.

### Colors

A `Color` is a CSS color string: `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`,
`rgba()` or `'transparent'`. In options, `null` means the theme's default.

### Options objects

Options are plain objects. `applyOptions` and similar methods change only the
keys you pass; absent keys keep their value. Getters such as
`series.options()` return every key (`Required<…>`).

### Errors

Methods throw an `Error` with a message that names the problem (an unknown
indicator name, an invalid parameter, columns of different lengths, a bar
whose time is not increasing, an unknown id). The chart is left unchanged.
`FerroChart.create` rejects when the requested GPU API is not available:
there is no Canvas 2D fallback; `renderer: 'auto'` tries WebGPU, then WebGL2.

### Events

`chart.on(name, callback)` subscribes, `chart.off(name, callback)`
unsubscribes. The callback's parameter is typed from the event name. Events
are delivered after the frame that produced them, not inside the input
handler. An exception thrown by a callback is reported to the console (through
`reportError`) and the other callbacks still run. See
[Crosshair and events](crosshair-events.md).

### Lifetime and memory

A chart listens to its canvas and the window and redraws on its own. Keep a
reference to it while it is on the page, and call `chart.destroy()` when you
remove it: that removes the listeners and releases GPU memory. A chart that is
dropped without `destroy` may be garbage-collected and stop responding.

Every object of the API holds WebAssembly memory. Objects the chart returns
(`Series`, `Indicator`, `PriceLine`) are freed with the chart; `free()`, or a
`using` declaration, releases one earlier. Removing a series (`remove()`) is a
different thing: it takes it off the chart.

### Input handled on the canvas

Pointer (mouse, pen, touch), wheel, keyboard (the canvas gets
`tabindex="0"`), double-click and trackpad pinch (Ctrl+wheel in Chrome and
Firefox, gesture events in Safari), and two-finger touch, which the chart
recognizes itself. The canvas style gets `touch-action: none`, or `pan-y` with
`touchScroll: 'page'` (see [Touch screens](web.md#touch-screens)). See
[Gestures and keyboard](interaction.md) for what each gesture does.

## Test data

`randomWalk(seed, n, startMs, intervalMs)` returns deterministic OHLC columns,
identical to the Rust generator `ferrochart_core::testing::random_walk`. The
examples and demos use it.

```ts
import init, { randomWalk } from '@ferrochart/core';

await init();
const bars = randomWalk(42, 10_000, Date.UTC(2024, 0, 1), 60_000);
```
