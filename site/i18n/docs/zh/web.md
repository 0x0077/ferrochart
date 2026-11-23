<!-- source: c694d739d5 -->
# Web（JavaScript / TypeScript）

npm 包 `@ferrochart/core` 包含 WebAssembly 构建产物和 TypeScript
类型定义。在支持的环境中使用 WebGPU 渲染，否则回退到 WebGL2。

```bash
npm install @ferrochart/core
```

## 在页面中放一个图表

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

canvas 的尺寸由 CSS 决定。FerroChart 会监听它的尺寸和设备像素比，保持后备缓冲区清晰；
你永远不需要自己设置 `canvas.width`。

类型化数组以每列一次 `memcpy` 的方式复制进引擎，因此用一百万根 K 线调用
`setData` 只需几十毫秒。

## 实时更新

```js
// Same time as the last bar: replaces it. Later time: appends a new bar.
candles.update({ time: Date.now(), open: 101.2, high: 101.9, low: 100.8, close: 101.5, volume: 1234 });
```

每秒调用一百次 `update` 也没问题：每次调用开销很小，而且图表每个动画帧最多只重绘一次。

## 事件

```js
chart.on('crosshairMove', ({ point }) => {
  if (point?.bar) console.log(point.time, point.bar.close);
});
chart.on('visibleRangeChanged', ({ from, to }) => { /* logical indices */ });
chart.on('needMoreHistory', ({ barsBeforeStart }) => { /* load older bars */ });
```

事件会在产生它们的那一帧之后派发。

## 清理

```js
chart.destroy(); // removes listeners and releases GPU resources
```

## 浏览器支持

需要 WebGPU 或 WebGL2；没有 Canvas2D 回退。已在 Chrome、Firefox 和 WebKit
（Safari 的引擎）中用两种渲染器测试过。完整演示见 `examples/web`，全部方法见
[JavaScript API](js-api.md)。
