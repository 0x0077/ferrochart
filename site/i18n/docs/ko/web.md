<!-- source: c694d739d5 -->
# 웹 (JavaScript / TypeScript)

npm 패키지 `@ferrochart/core`에는 WebAssembly 빌드와 TypeScript 타입이
들어 있습니다. 가능하면 WebGPU로 렌더링하고, 그렇지 않으면 WebGL2로 대체합니다.

```bash
npm install @ferrochart/core
```

## 페이지에 차트 넣기

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

캔버스 크기는 CSS로 정합니다. FerroChart가 캔버스 크기와 장치 픽셀 비율을
감시하여 백킹 스토어를 선명하게 유지하므로, `canvas.width`를 직접 설정할 필요가
없습니다.

타입 배열은 컬럼당 한 번의 `memcpy`로 엔진에 복사되므로, 100만 개의 봉으로
`setData`를 호출해도 수십 밀리초면 끝납니다.

## 실시간 업데이트

```js
// Same time as the last bar: replaces it. Later time: appends a new bar.
candles.update({ time: Date.now(), open: 101.2, high: 101.9, low: 100.8, close: 101.5, volume: 1234 });
```

`update`를 초당 100번 호출해도 괜찮습니다. 각 호출은 비용이 적고, 차트는
애니메이션 프레임당 최대 한 번만 다시 그립니다.

## 이벤트

```js
chart.on('crosshairMove', ({ point }) => {
  if (point?.bar) console.log(point.time, point.bar.close);
});
chart.on('visibleRangeChanged', ({ from, to }) => { /* logical indices */ });
chart.on('needMoreHistory', ({ barsBeforeStart }) => { /* load older bars */ });
```

이벤트는 해당 이벤트를 발생시킨 프레임이 그려진 뒤에 전달됩니다.

## 정리

```js
chart.destroy(); // removes listeners and releases GPU resources
```

## 브라우저 지원

WebGPU 또는 WebGL2가 필요하며, Canvas2D 대체 경로는 없습니다. Chrome,
Firefox, WebKit(Safari의 엔진)에서 두 렌더러 모두로 테스트했습니다. 전체 데모는
`examples/web`을, 모든 메서드는 [JavaScript API](js-api.md)를 참고하십시오.
