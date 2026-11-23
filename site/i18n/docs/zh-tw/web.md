<!-- source: c694d739d5 -->
# Web（JavaScript / TypeScript）

npm 套件 `@ferrochart/core` 包含 WebAssembly 建置與 TypeScript 型別。在支援的環境中以 WebGPU 渲染，否則退回 WebGL2。

```bash
npm install @ferrochart/core
```

## 在網頁中放一張圖表

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

canvas 的尺寸由 CSS 決定。FerroChart 會監看它的尺寸與裝置像素比，讓後台緩衝區保持清晰；你永遠不需要自己設定 `canvas.width`。

型別陣列（typed array）以每個欄位一次 `memcpy` 的方式複製進引擎，因此對一百萬根 K 線呼叫 `setData` 只需數十毫秒。

## 即時更新

```js
// Same time as the last bar: replaces it. Later time: appends a new bar.
candles.update({ time: Date.now(), open: 101.2, high: 101.9, low: 100.8, close: 101.5, volume: 1234 });
```

每秒呼叫 `update` 一百次也沒問題：每次呼叫的成本都很低，而且圖表每個動畫幀最多只重繪一次。

## 事件

```js
chart.on('crosshairMove', ({ point }) => {
  if (point?.bar) console.log(point.time, point.bar.close);
});
chart.on('visibleRangeChanged', ({ from, to }) => { /* logical indices */ });
chart.on('needMoreHistory', ({ barsBeforeStart }) => { /* load older bars */ });
```

事件會在產生它們的那一幀之後送出。

## 清理

```js
chart.destroy(); // removes listeners and releases GPU resources
```

## 瀏覽器支援

需要 WebGPU 或 WebGL2；不提供 Canvas2D 退回方案。已在 Chrome、Firefox 與 WebKit（Safari 的引擎）上以兩種渲染器測試。完整範例請見 `examples/web`，所有方法請見 [JavaScript API](js-api.md)。
