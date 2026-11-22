# Themes and styling

This chapter covers how a chart looks and how it fits into a page: the theme,
colors, the attribution logo, the intro animation, the GPU renderer, sizing
and cleanup. Everything here applies to `FerroPlot` as well unless noted.

## Theme

The theme is chosen when the chart is created:

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' }); // 'light' (default) or 'dark'
```

The theme sets the background, grid, text, axis and crosshair colors, the
default up and down colors of candles, and the default colors of series,
indicators and drawings. It cannot be changed on an existing chart; to switch
themes, save the state (see [Saving and restoring](state.md)), destroy the
chart and create a new one.

Any color you leave unset, or set to `null`, follows the theme. Set colors
explicitly only where you need them; the rest then stays consistent in both
themes.

## Series colors

Series take their colors from `SeriesOptions`, at creation or later with
`series.applyOptions`:

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const candles = chart.addCandlestickSeries({ color: '#26a69a', downColor: '#ef5350' });
const line = chart.addSeries('line', { color: '#2962ff', lineWidth: 2, lineStyle: 'dashed' });
const area = chart.addSeries('area', {
  color: '#2962ff',
  topFill: 'rgba(41, 98, 255, 0.4)',
  bottomFill: 'rgba(41, 98, 255, 0)',
});

line.applyOptions({ color: null }); // back to the theme's color
```

| Option | Used for |
|---|---|
| `color` | Line color of line, area and baseline (above the base) series; up candles and bars; histogram bars |
| `downColor` | Down candles and bars; the baseline series' line below the base |
| `topFill` | Area fill at the top; baseline fill above the base |
| `bottomFill` | Area fill at the bottom; baseline fill below the base |
| `lineWidth` | Line width in CSS px |
| `lineStyle` | `'solid'`, `'dashed'` or `'dotted'` |

Area fills are a gradient from `topFill` to `bottomFill`. Histogram series also
accept a color per bar (the `color` column of `setData`). All series options,
and what each series kind draws, are in [Series](series.md).

Other objects have their own color options: price lines
(`createPriceLine({ color })`), markers (`color` per marker), bands between
two series (`addBand(upper, lower, { fill, fillCrossed })`) and drawings
(`style: { line, fill, text }`). See [Series](series.md),
[Markers](markers.md), [Indicators](indicators.md) and
[Drawings](drawings.md).

## Colors

Colors are CSS color strings in one of these forms (type `Color`):

| Form | Example |
|---|---|
| `#rgb`, `#rgba` | `'#f80'`, `'#f808'` |
| `#rrggbb`, `#rrggbbaa` | `'#ff8800'`, `'#ff880080'` |
| `rgb(r, g, b)` | `'rgb(255, 136, 0)'` |
| `rgba(r, g, b, a)` | `'rgba(255, 136, 0, 0.5)'`, alpha from 0 to 1 |
| `transparent` | `'transparent'` |

Channels are 0 to 255; commas, spaces or a `/` separate them. Named colors
(`'red'`), `hsl()` and percentages are **not** accepted: the call throws, for
example `'color': invalid color 'red'`.

Colors read back from `options()` are written as `#rrggbb`, or
`rgba(r, g, b, a)` when they are not opaque; a color that follows the theme
reads back as `null`.

Per-point color columns (histogram data, plot marks) also accept a
`Uint32Array` of `0xRRGGBBAA` values, which avoids creating strings for large
data.

## Attribution logo

By default a small FerroChart logo is shown at the bottom-left corner of the
lowest pane, just above the time axis. It links to ferrochart.com and shows
the name when the pointer is over it.

The logo is a courtesy, not a license requirement: you may turn it off.

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { attributionLogo: false });

chart.setAttributionLogo(true);  // show it
chart.setAttributionLogo(false); // remove it
```

How it is placed:

- It is an `<a>` element inserted right after the canvas, positioned
  absolutely next to it; it is not drawn into the canvas, so it does not
  appear in screenshots or images rendered by the engine.
- Its position is updated when the layout changes. It is positioned relative
  to the canvas's offset parent, like the canvas itself.
- If the canvas has no parent element when the logo would be added, it is
  not added.

A `FerroPlot` has the same option and method; its logo sits at the
bottom-left of the plot area. A plot attached to a chart with
`attachSidePlot` loses its logo, since the chart's stands for both (see
[Plots](plots.md)).

## Intro animation

`playIntro` shows the chart empty (grid and axes) and then draws the data from
left to right. Use it when a chart first appears, for example on a landing
page or when the user opens a symbol.

```ts
import { FerroChart, randomWalk } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' });
chart.addCandlestickSeries().setData(randomWalk(1, 300, Date.UTC(2024, 0, 1), 3_600_000));

await chart.playIntro({ duration: 1200, delay: 200, pen: '#4cc9f0' });
// The animation has ended (or was ended early).
```

`IntroOptions`:

| Option | Meaning | Default |
|---|---|---|
| `duration` | Time to draw the data from left to right, ms | `900` |
| `delay` | Time the empty chart is shown first, ms | `0` |
| `pen` | Color of a glowing line at the drawing edge; `null` for none | `null` |

Behavior:

- The chart is shown empty at once, before the next frame, so call
  `playIntro` right after `setData` to avoid a flash of the full chart.
- Price lines, value labels and the crosshair appear at the end.
- A press, wheel, pinch, double-click or key on the canvas ends it early.
  Moving the pointer over the chart does not.
- `chart.finishIntro()` ends it at once.
- Calling `playIntro` again ends the running animation first.
- The returned promise resolves when the animation ends, however it ends
  (also when the chart is destroyed). It never rejects; invalid options
  (a negative or non-finite `duration` or `delay`, an invalid `pen` color)
  throw from the call itself.

For a `FerroPlot`, `playIntro` draws the marks from left to right; rule labels
and the hover highlight appear at the end.

## Renderer

FerroChart draws with the GPU: WebGPU where the browser supports it, WebGL2
otherwise. There is no Canvas 2D fallback.

| `renderer` option | Behavior |
|---|---|
| `'auto'` (default) | WebGPU, then WebGL2 |
| `'webgpu'` | WebGPU only |
| `'webgl2'` | WebGL2 only |

`create` rejects when no requested API is available (`'WebGPU is not
available'`, `'WebGL2 is not available'`, or both joined by `;`), or for an
unknown option value. The `renderer` property tells which API is in use:

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

try {
  const chart = await FerroChart.create(canvas);
  console.log(chart.renderer); // 'webgpu' or 'webgl2'
} catch (e) {
  canvas.replaceWith(Object.assign(document.createElement('p'), {
    textContent: 'This chart needs WebGPU or WebGL2.',
  }));
}
```

Forcing `'webgl2'` is a simple way to test the fallback on a browser that
has WebGPU.

## Sizing the canvas

The canvas is sized with CSS. FerroChart sets the canvas's backing store
(`canvas.width` and `canvas.height`) itself, to the CSS size times
`devicePixelRatio`, so the chart is sharp on high-density screens. Do not set
`width` and `height` on the canvas yourself.

```html
<div style="height: 60vh">
  <canvas id="chart" style="display: block; width: 100%; height: 100%"></canvas>
</div>
```

- A `ResizeObserver` watches the canvas's content box. When its CSS size
  changes, the backing store and the layout follow and the chart redraws.
  You never call a resize method.
- When `devicePixelRatio` changes (browser zoom, moving the window to another
  screen), the backing store is resized too.
- The backing store is at least 1 × 1 px. If the chart is in a hidden element
  when it is created, it draws as soon as the element gets a size.
- `display: block` avoids the few pixels of space below an inline canvas.
- Inside flex or grid layouts, give the canvas `min-height: 0` (or
  `min-width: 0`) so the container can shrink it.

When the chart is created, the canvas also gets `tabindex="0"` (unless it has
a `tabindex`) so it can receive keys, `touch-action: none` so touch gestures
pan and zoom the chart instead of the page, and `outline: none`. The cursor
style is set while the pointer is over the chart.

## Cleaning up

```ts
import { FerroChart, FerroPlot } from '@ferrochart/core';

declare const chart: FerroChart;
declare const plot: FerroPlot;

chart.destroy();
plot.destroy(); // FerroPlot's equivalent
```

`destroy()`:

- removes the DOM event listeners on the canvas and every callback added with
  `on`;
- leaves the chart's sync group;
- removes the attribution logo;
- resolves a pending `playIntro` promise;
- releases the GPU resources.

The canvas element stays in the page; remove it yourself if you no longer
need it. After `destroy()`, the chart object cannot be used, and calls on its
`Series`, `Indicator` and other handles throw (`the FerroChart was
destroyed`). Destroy charts you no longer show: each one holds GPU memory,
and browsers limit the number of WebGL contexts a page may have.

In React or Vue, create the chart when the component mounts and destroy it
when it unmounts; see [React and Vue](frameworks.md).
