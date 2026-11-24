<!-- source: fbce18a0e1 -->
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

## 触屏

在手机和平板上，单指平移图表，双指缩放，长按 500 ms 后显示十字光标（完整列表见[手势与键盘](interaction.md#touch)）。`touchScroll` 选项决定由谁处理竖向滑动：

```js
const chart = await FerroChart.create(canvas, { touchScroll: 'page' });
chart.setTouchScroll('chart'); // change it later
```

| `touchScroll` | Canvas `touch-action` | 竖向滑动 | 适用场景 |
|---|---|---|---|
| `'chart'`（默认） | `none` | 平移图表 | 全屏图表、应用式页面 |
| `'page'` | `pan-y` | 滚动页面 | 文章、文档和滚动面板中的图表 |

使用 `'page'` 时，水平滑动、双指操作和长按仍由图表处理；长按十字光标显示后，上下移动手指会移动十字光标，而不是滚动页面。代价是：价格轴关闭自动缩放时，单指无法平移价格，需使用双指。
`FerroPlot` 接受相同选项。

## 清理

```js
chart.destroy(); // removes listeners and releases GPU resources
```

## 浏览器支持

需要 WebGPU 或 WebGL2；没有 Canvas2D 回退。已在 Chrome、Firefox 和 WebKit
（Safari 的引擎）中用两种渲染器测试过。完整演示见 `examples/web`，全部方法见
[JavaScript API](js-api.md)。
