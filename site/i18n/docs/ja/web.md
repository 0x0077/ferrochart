<!-- source: fbce18a0e1 -->
# Web（JavaScript / TypeScript）

npmパッケージ`@ferrochart/core`には、WebAssemblyビルドとTypeScriptの型定義が含まれています。利用可能な場合はWebGPUで描画し、そうでなければWebGL2にフォールバックします。

```bash
npm install @ferrochart/core
```

## ページにチャートを置く

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

canvasのサイズはCSSで決まります。FerroChartはそのサイズとデバイスピクセル比を監視し、バッキングストアを常にシャープに保ちます。`canvas.width`を自分で設定する必要はありません。

型付き配列はカラムごとに1回の`memcpy`でエンジンにコピーされるため、100万本の足に対する`setData`は数十ミリ秒で完了します。

## リアルタイム更新

```js
// Same time as the last bar: replaces it. Later time: appends a new bar.
candles.update({ time: Date.now(), open: 101.2, high: 101.9, low: 100.8, close: 101.5, volume: 1234 });
```

`update`を1秒間に100回呼び出しても問題ありません。各呼び出しは軽量で、チャートの再描画はアニメーションフレームごとに最大1回です。

## イベント

```js
chart.on('crosshairMove', ({ point }) => {
  if (point?.bar) console.log(point.time, point.bar.close);
});
chart.on('visibleRangeChanged', ({ from, to }) => { /* logical indices */ });
chart.on('needMoreHistory', ({ barsBeforeStart }) => { /* load older bars */ });
```

イベントは、それを発生させたフレームの後に配信されます。

## タッチスクリーン

スマートフォンやタブレットでは、1本指でパン、2本指でズームし、500 msの長押しでクロスヘアを表示します（一覧は[ジェスチャーとキーボード](interaction.md#touch)を参照）。`touchScroll`オプションは、縦スワイプをどちらで処理するかを指定します。

```js
const chart = await FerroChart.create(canvas, { touchScroll: 'page' });
chart.setTouchScroll('chart'); // change it later
```

| `touchScroll` | canvasの`touch-action` | 縦スワイプ | 用途 |
|---|---|---|---|
| `'chart'`（デフォルト） | `none` | チャートをパン | 全画面チャート、アプリのようなページ |
| `'page'` | `pan-y` | ページをスクロール | 記事、ドキュメント、スクロールするパネル内のチャート |

`'page'`でも、横スワイプ、2本指、長押しはチャートで処理します。長押しでクロスヘアが表示された後は、指を上下に動かすとページではなくクロスヘアが動きます。ただし、価格軸の自動スケールがオフの場合、1本指では価格方向にパンできません。2本指を使ってください。`FerroPlot`も同じオプションを受け取ります。

## 後片付け

```js
chart.destroy(); // removes listeners and releases GPU resources
```

## ブラウザ対応

WebGPUまたはWebGL2が必要です。Canvas2Dへのフォールバックはありません。Chrome、Firefox、WebKit（Safariのエンジン）で両方のレンダラーをテストしています。完全なデモは`examples/web`を、すべてのメソッドについては[JavaScript API](js-api.md)を参照してください。
