# Web (JavaScript / TypeScript)

The npm package `@ferrochart/core` contains the WebAssembly build and TypeScript
types. It renders with WebGPU where available and falls back to WebGL2.

```bash
npm install @ferrochart/core
```

## A chart in a page

```html
<canvas id="chart" style="width: 100%; height: 480px"></canvas>
<script type="module">
  import { FerroChart } from '@ferrochart/core';

  // create() loads the .wasm file the first time.
  const chart = await FerroChart.create(document.getElementById('chart'), {
    theme: 'dark',     // 'light' (default) or 'dark'
    renderer: 'auto',  // 'auto' (WebGPU, then WebGL2), 'webgpu' or 'webgl2'
  });

  const candles = chart.addCandlestickSeries();
  chart.addVolumeSeries(candles); // volume histogram from the same data

  candles.setData({
    time: new BigInt64Array([...]),  // UTC milliseconds; Float64Array or number[] also work
    open: new Float64Array([...]),
    high: new Float64Array([...]),
    low: new Float64Array([...]),
    close: new Float64Array([...]),
    volume: new Float64Array([...]), // optional
  });
</script>
```

The canvas is sized by CSS. FerroChart watches its size and the device pixel
ratio and keeps the backing store sharp; you never set `canvas.width` yourself.

Typed arrays are copied into the engine with one `memcpy` per column, so
`setData` with a million bars takes a few tens of milliseconds.

## Real-time updates

```js
// Same time as the last bar: replaces it. Later time: appends a new bar.
candles.update({ time: Date.now(), open: 101.2, high: 101.9, low: 100.8, close: 101.5, volume: 1234 });
```

Calling `update` a hundred times per second is fine: each call is cheap, and
the chart redraws at most once per animation frame.

## Events

```js
chart.on('crosshairMove', ({ point }) => {
  if (point?.bar) console.log(point.time, point.bar.close);
});
chart.on('visibleRangeChanged', ({ from, to }) => { /* logical indices */ });
chart.on('needMoreHistory', ({ barsBeforeStart }) => { /* load older bars */ });
```

Events are delivered after the frame that produced them.

## Touch screens

On phones and tablets the chart pans with one finger, zooms with two and shows
the crosshair after a 500 ms long press (the full list is in
[Gestures and keyboard](interaction.md#touch)). The `touchScroll` option says
who gets a vertical swipe:

```js
const chart = await FerroChart.create(canvas, { touchScroll: 'page' });
chart.setTouchScroll('chart'); // change it later
```

| `touchScroll` | Canvas `touch-action` | Vertical swipe | Use it for |
|---|---|---|---|
| `'chart'` (default) | `none` | Pans the chart | Full-screen charts, app-like pages |
| `'page'` | `pan-y` | Scrolls the page | Charts inside articles, docs and scrolling panels |

With `'page'`, horizontal swipes, two fingers and the long press still go to
the chart, and once the long-press crosshair is showing, moving the finger up
or down moves the crosshair, not the page. The cost: with the price axis'
auto-scale off, one finger cannot pan prices; use two fingers.
`FerroPlot` takes the same option.

## Cleaning up

```js
chart.destroy(); // removes listeners and releases GPU resources
```

## Browser support

WebGPU or WebGL2 is required; there is no Canvas2D fallback. Tested in Chrome,
Firefox and WebKit (Safari's engine) with both renderers. See
`examples/web` for a complete demo and the [JavaScript API](js-api.md) for
every method.
