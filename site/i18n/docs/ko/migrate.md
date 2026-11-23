<!-- source: 5a2a4a05ee -->
# lightweight-charts에서 마이그레이션

lightweight-charts용으로 작성한 코드를 FerroChart로 옮기는 방법은 두 가지입니다.

1. **호환 엔트리** `@ferrochart/core/lightweight-charts`. lightweight-charts v4와
   v5 API에서 자주 쓰이는 부분을 FerroChart 위에 구현한 것으로, 기존 차트는
   import만 바꿔도 동작하는 경우가 많습니다. 기존 페이지에서 FerroChart를
   시험해 보거나 단계적으로 마이그레이션할 때 사용합니다.
2. **네이티브 API** `@ferrochart/core`. 이 가이드의 나머지 부분에서 설명하는
   API로, 모든 기능을 사용할 수 있습니다. 대용량 데이터를 위한 컬럼형 데이터,
   지표, 드로잉, 동기화, 오더플로우, 플롯 등이 여기에 해당합니다.

## 호환 엔트리

```ts
// Before:
// import { createChart, CandlestickSeries, LineSeries } from 'lightweight-charts';
import { createChart, CandlestickSeries, LineSeries, type UTCTimestamp } from '@ferrochart/core/lightweight-charts';

const container = document.getElementById('chart') as HTMLElement;
const chart = createChart(container, { layout: { background: { color: '#131722' } } });

const candles = chart.addSeries(CandlestickSeries, { upColor: '#26a69a', downColor: '#ef5350' });
candles.setData([
  { time: 1700000000 as UTCTimestamp, open: 10, high: 12, low: 9, close: 11 },
  { time: 1700000060 as UTCTimestamp, open: 11, high: 13, low: 10, close: 12 },
]);

const sma = chart.addSeries(LineSeries, { color: '#2962ff', lineWidth: 2 });
sma.setData([{ time: 1700000060 as UTCTimestamp, value: 11.5 }]);

chart.timeScale().fitContent();
```

### 지원 범위

| 영역 | 지원 항목 |
|---|---|
| 생성 | `createChart(container, options)`, 요소 또는 요소의 id 사용 |
| 시리즈 | `CandlestickSeries`, `BarSeries`, `LineSeries`, `AreaSeries`, `BaselineSeries`, `HistogramSeries`를 사용하는 `addSeries(definition, options?, paneIndex?)`; v4 메서드 `addCandlestickSeries`, `addBarSeries`, `addLineSeries`, `addAreaSeries`, `addBaselineSeries`, `addHistogramSeries`; `removeSeries` |
| 데이터 | `setData`, `update`, `dataByIndex`; 시간은 `UTCTimestamp` 초, `'YYYY-MM-DD'` 문자열 또는 `BusinessDay` 객체; 히스토그램 데이터의 봉별 `color` |
| 시리즈 API | `applyOptions`, `options`, `seriesType`, `priceScale`, `createPriceLine`, `removePriceLine` |
| 마커 | v4 `series.setMarkers` / `series.markers`; v5 `createSeriesMarkers(series, markers)`와 `setMarkers`, `markers`, `detach` |
| 시간 축 | `fitContent`, `scrollToRealTime`, `scrollToPosition`, `scrollPosition`, `getVisibleLogicalRange`, `setVisibleLogicalRange`, `getVisibleRange`, `setVisibleRange`, `logicalToCoordinate`, `coordinateToLogical`, `timeToCoordinate`, `coordinateToTime`, 보이는 논리 범위·시간 범위 구독, `applyOptions` / `options`(`barSpacing`, `rightOffset`), `width` |
| 가격 축 | `chart.priceScale('right', paneIndex)`와 `series.priceScale()`: `scaleMargins`, `mode`, `invertScale`, `autoScale`를 지정하는 `applyOptions` / `options` |
| 크로스헤어 | `time`, `logical`, `point`, `paneIndex`, `seriesData`를 제공하는 `subscribeCrosshairMove` / `unsubscribeCrosshairMove` |
| 차트 | `applyOptions` / `options`(`width`, `height`, `layout.background.color`, `crosshair.mode`, `timeScale.barSpacing`, `timeScale.rightOffset`), `resize`, `remove`, `timeScale`, `priceScale` |
| 상수 | `ColorType`, `CrosshairMode`, `LineStyle`, `LineType`, `PriceScaleMode`, `isUTCTimestamp`, `isBusinessDay` |

TypeScript 선언(`ISeriesApi`, `IChartApi`, `Time`, `UTCTimestamp`,
`SeriesMarker` 등)이 포함되어 있습니다.

### 비동기 시작

GPU 설정은 비동기로 이루어지지만, `createChart`는 lightweight-charts와
마찬가지로 즉시 반환됩니다. 차트가 준비되기 전에 한 호출은 큐에 쌓였다가 준비가
끝나면 순서대로 실행됩니다. 이 점이 중요한 경우를 위해 두 가지가 추가되어 있습니다.

| 멤버 | 설명 |
|---|---|
| `chart.whenReady(): Promise<void>` | GPU 설정이 끝나고 큐에 쌓인 호출이 실행되면 resolve됩니다 |
| `chart.ferrochart: FerroChart \| null` | 내부의 네이티브 차트. 준비될 때까지는 `null` |

`getVisibleLogicalRange()`, `getVisibleRange()`, `coordinateToTime()`,
`dataByIndex()` 같은 getter는 차트가 준비될 때까지 `null`을 반환합니다
(`priceScale().options()`는 `{}`를 반환합니다). 값을 읽기 전에
`whenReady()`를 await하십시오.

```ts
import { createChart } from '@ferrochart/core/lightweight-charts';

declare const container: HTMLElement;

const chart = createChart(container);
// ... add series and data ...
await chart.whenReady();
console.log(chart.timeScale().getVisibleLogicalRange());
```

큐에 쌓인 호출에서 발생한 오류는 throw되지 않고 `console.error`로 콘솔에
기록됩니다. 오류를 일으킨 호출은 이미 반환되었기 때문입니다. 차트가 준비된
뒤에는 평소처럼 오류가 throw됩니다.

### 차이점

**렌더링과 크기**

- 차트는 `createChart`가 컨테이너에 추가하는 `<canvas>`에 그려집니다.
  `width`/`height` 옵션이 없으면 캔버스가 컨테이너를 가득 채우고
  (`100%` × `100%`) 컨테이너 크기를 따라가므로, 컨테이너에 높이를 지정하십시오.
  `autoSize`는 필요 없으며 무시됩니다.
- `width`, `height`, `resize(width, height)`는 캔버스의 CSS 크기를 설정합니다.
- `remove()`는 차트를 파괴하고 캔버스를 제거합니다.
- WebGPU 또는 WebGL2가 필요합니다. [테마와 스타일](styling.md#renderer)을 참고하십시오.

**테마와 색상**

- `layout.background.color`로 테마가 결정됩니다. `#rrggbb`(또는
  `#rrggbbaa`)로 쓴 어두운 색이면 다크 테마, 그 밖에는 라이트 테마가 됩니다.
  그 밖의 `layout` 옵션(`textColor`, `fontSize`, `fontFamily`, 그라디언트)은
  무시됩니다. `layout`은 GPU 설정이 끝날 때 한 번만 읽으며, 그 뒤에 바꾸면
  경고가 기록되고 아무 효과가 없습니다.
- `crosshair.mode`: `Normal`, `Magnet`, `Hidden`이 동작하며, `MagnetOHLC`는
  `Magnet`처럼 동작합니다.
- `grid`, `watermark`, `handleScroll`, `handleScale`, `localization`,
  `kineticScroll` 등 대응하는 기능이 없는 차트 옵션은 받기만 하고 무시합니다.
- 색상은 FerroChart가 받아들이는 형식이어야 합니다. `#rgb`, `#rrggbb`, `#rrggbbaa`,
  `rgb()`, `rgba()`, `transparent` 중 하나입니다. `'red'` 같은 색상 이름은
  오류가 됩니다. [색상](styling.md#colors)을 참고하십시오.

**시리즈 옵션.** 다음 옵션만 사용되며, 나머지는 받기만 하고 무시합니다.

| 시리즈 | 사용되는 옵션 |
|---|---|
| Candlestick, Bar | `upColor`, `downColor` |
| Line | `color` |
| Area | `lineColor`, `topColor`, `bottomColor` |
| Baseline | `topLineColor`, `bottomLineColor`, `topFillColor1`, `bottomFillColor1`, `baseValue: { type: 'price', price }` |
| Histogram | `color`, `base` |
| 전체 | `lineWidth`, `lineStyle`, `lineType`, `lastValueVisible`, `priceLineVisible`, `priceFormat.precision` |

따라서 `wickUpColor`, `borderUpColor` 같은 캔들 색상, `crosshairMarker*`,
`priceFormat.minMove`, `priceFormat.type`, `title`, `visible`은 효과가
없습니다. `LineStyle.LargeDashed`는 파선으로, `LineStyle.SparseDotted`는
점선으로 그립니다. `LineType.WithSteps`는 계단형으로, `LineType.Curved`는
직선 구간으로 그립니다.

**가격 축**

- 각 패널에는 오른쪽에 가격 축이 하나 있습니다. 왼쪽 가격 축이나 오버레이 가격
  축은 없습니다. `'right'`가 아닌 `priceScaleId`로 만든 시리즈(거래량 오버레이에
  흔히 쓰는 `''` 포함)는 기본 패널 높이의 30%인 별도 패널에 배치되고 경고가
  기록됩니다. 거래량은 아래에 나오는 네이티브 `addVolumeSeries`를 쓰면 메인
  패널 하단에 그려집니다.
- `'right'`가 아닌 id로 `chart.priceScale(id)`를 호출하면 경고가 기록되고
  오른쪽 축이 반환됩니다.

**시간**

- 시간은 UTC 밀리초로 변환됩니다. `'YYYY-MM-DD'` 문자열과 `BusinessDay` 객체는
  UTC 자정이 되며, 그 밖의 문자열 형식은 예외를 던집니다.
- 차트가 (`setData` 또는 `update`에서) 처음 받은 시간의 형식이 차트가 반환하는
  시간의 형식을 결정합니다. 이벤트, `getVisibleRange`, `coordinateToTime`,
  `dataByIndex`, 크로스헤어의 `time`은 그에 따라 초, 문자열 또는
  `BusinessDay` 객체를 사용합니다.
- 논리 인덱스는 lightweight-charts와 마찬가지로 첫 번째 봉이 0입니다.

**데이터**

- 공백 항목(`{ time }`만 있는 항목)은 캔들스틱과 바 데이터에서는 제거되고,
  라인, 영역, 베이스라인, 히스토그램 데이터에서는 공백(갭)이 됩니다.
- 이 API에서 OHLC 데이터에는 거래량이 없습니다. 거래량 시리즈는 네이티브 API를
  사용하십시오.

**마커**

- `position: 'inBar'`는 봉 위에 그려지고, 알 수 없는 `shape`는 원으로
  그려집니다. `size`는 기본 크기의 배수이며(1은 10 CSS px, 4–64 px 범위로
  제한), `id`는 무시됩니다.

**아직 지원하지 않는 기능**

- `subscribeClick`은 아무 동작도 하지 않고 경고를 기록합니다. 대신
  `chart.ferrochart`를 통해 네이티브 `click` 이벤트를 사용하십시오.
- `series.priceToCoordinate`와 `series.coordinateToPrice`는 `null`을 반환하고
  경고를 기록합니다.
- 커스텀 시리즈, 시리즈 프리미티브, 패널 프리미티브. FerroChart 자체의 확장
  기능은 [플러그인](plugins.md)을 참고하십시오.
- `scrollToPosition(position, animated)`는 `animated`를 무시합니다.
- `IPriceLine.options()`는 모든 옵션이 아니라 전달한 옵션만 반환합니다.

### 호환 엔트리에서 네이티브 기능 사용하기

`chart.ferrochart`는 네이티브 `FerroChart`이므로, 페이지는 기존
lightweight-charts 호출을 그대로 두고 네이티브 기능을 함께 쓸 수 있습니다.

```ts
import { createChart, CandlestickSeries } from '@ferrochart/core/lightweight-charts';

declare const container: HTMLElement;

const chart = createChart(container);
chart.addSeries(CandlestickSeries);
await chart.whenReady();

const fc = chart.ferrochart;
if (fc) {
  fc.drawings.startTool('trend_line');
  fc.on('click', (e) => console.log('clicked at', e.time, e.price));
}
```

네이티브 객체를 통해 추가한 시리즈는 호환 레이어가 알지 못합니다(예를 들어
크로스헤어 이벤트의 `seriesData`에 포함되지 않습니다).

## 네이티브 API로 옮기기

네이티브 API는 몇 가지 큰 부분에서 lightweight-charts와 다릅니다.

- **생성은 비동기입니다.** `FerroChart.create(canvas, options)`는 Promise를
  반환하며, 컨테이너 대신 CSS로 크기를 정한 `<canvas>` 요소를 받습니다.
- **시간은 UTC 밀리초입니다.** `number`, `bigint` 또는 (행 형식에서는) `Date`로
  지정합니다. lightweight-charts의 초 값에 1000을 곱하십시오.
- **데이터는 컬럼일 수 있습니다.** `setData`는 행 배열도 받지만, 대용량
  데이터에서는 타입 배열로 된 컬럼이 훨씬 빠릅니다.
- **종류는 문자열입니다.** `chart.addSeries('candlestick', options, pane)`처럼
  쓰며, 패널은 인덱스가 아니라 id로 지정합니다.
- **옵션은 평평합니다.** 시리즈 색상은 `color`, `downColor`, `topFill`,
  `bottomFill`이며, 테마는 `'light'` 또는 `'dark'`입니다.
- **이벤트는 `on`/`off`를 사용합니다.** `subscribeCrosshairMove(...)` 대신
  `chart.on('crosshairMove', ...)`를 씁니다.

### 나란히 비교

lightweight-charts:

```js
import { createChart, CandlestickSeries, HistogramSeries } from 'lightweight-charts';

const chart = createChart(document.getElementById('chart'), {
  layout: { background: { color: '#131722' }, textColor: '#d1d4dc' },
});
const candles = chart.addSeries(CandlestickSeries, { upColor: '#26a69a', downColor: '#ef5350' });
candles.setData(rows); // [{ time: 1700000000, open, high, low, close }]

const volume = chart.addSeries(HistogramSeries, { priceScaleId: '' });
volume.priceScale().applyOptions({ scaleMargins: { top: 0.8, bottom: 0 } });
volume.setData(rows.map((r) => ({ time: r.time, value: r.volume })));

chart.subscribeCrosshairMove((param) => {
  const bar = param.seriesData.get(candles);
  if (bar) legend.textContent = `C ${bar.close}`;
});
chart.timeScale().fitContent();
```

FerroChart:

```ts
import { FerroChart } from '@ferrochart/core';

declare const rows: { time: number; open: number; high: number; low: number; close: number; volume: number }[];
declare const legend: HTMLElement;

const canvas = document.getElementById('chart') as HTMLCanvasElement; // style="width: 100%; height: 480px"
const chart = await FerroChart.create(canvas, { theme: 'dark' });

const candles = chart.addCandlestickSeries({ color: '#26a69a', downColor: '#ef5350' });
candles.setData(rows.map((r) => ({ ...r, time: r.time * 1000 }))); // seconds to ms; volume included
chart.addVolumeSeries(candles); // reads the volume column, bottom of the main pane

chart.on('crosshairMove', ({ point }) => {
  if (point?.bar) legend.textContent = `C ${point.bar.close}`;
});
chart.fitContent();
```

### 자주 쓰는 호출 대응표

| lightweight-charts | FerroChart |
|---|---|
| `createChart(container, options)` | `await FerroChart.create(canvas, { theme, renderer })` |
| `chart.addSeries(CandlestickSeries, o)` / `addCandlestickSeries(o)` | `chart.addCandlestickSeries(o)` 또는 `chart.addSeries('candlestick', o)` |
| `BarSeries` / `addBarSeries` | `chart.addSeries('ohlc', o)` |
| `LineSeries`, `AreaSeries`, `BaselineSeries`, `HistogramSeries` | `chart.addSeries('line' \| 'area' \| 'baseline' \| 'histogram', o)` |
| `chart.addSeries(def, o, paneIndex)` | `chart.addSeries(kind, o, chart.panes()[paneIndex])`; 새 패널은 `chart.addPane()`으로 추가 |
| `priceScaleId: ''`를 쓴 거래량 히스토그램 | `chart.addVolumeSeries(candles)` |
| `chart.removeSeries(series)` | `series.remove()` |
| `series.setData(items)` | `series.setData(rows)`(시간은 ms) 또는 `series.setData({ time, open, high, low, close, volume })` 컬럼 |
| `series.update(item)` | `series.update(bar)`(시간은 ms) |
| `series.applyOptions({ upColor, downColor })` | `series.applyOptions({ color, downColor })` |
| `series.applyOptions({ lineColor, topColor, bottomColor })` | `series.applyOptions({ color, topFill, bottomFill })` |
| `series.applyOptions({ priceFormat: { precision } })` | `series.applyOptions({ pricePrecision })` |
| `series.createPriceLine(o)` / `removePriceLine(line)` | `series.createPriceLine(o)` / `line.remove()` |
| `series.setMarkers(m)` / `createSeriesMarkers(series, m)` | `series.setMarkers(m)`(시간은 ms) |
| `series.dataByIndex(i)` | `series.dataAt(logicalIndex)` |
| `chart.timeScale().fitContent()` | `chart.fitContent()` |
| `chart.timeScale().scrollToRealTime()` | `chart.scrollToRealtime()` |
| `timeScale().scrollToPosition(n)` / `scrollPosition()` | `chart.setRightOffset(n)` / `chart.rightOffset` |
| `timeScale().getVisibleLogicalRange()` | `chart.visibleLogicalRange()` |
| `timeScale().setVisibleLogicalRange({ from, to })` | `chart.setVisibleLogicalRange(from, to)` |
| `timeScale().setVisibleRange({ from, to })` | `chart.applySync({ type: 'visibleRange', from, to })`(ms) |
| `timeScale().applyOptions({ barSpacing, rightOffset })` | `chart.setBarSpacing(px)`, `chart.setRightOffset(bars)` |
| `timeScale().logicalToCoordinate(i)` / `coordinateToLogical(x)` | `chart.logicalToCoordinate(i)` / `chart.coordinateToLogical(x)` |
| `timeScale().timeToCoordinate(t)` | `chart.logicalToCoordinate(chart.indexOfTime(ms))` |
| `timeScale().coordinateToTime(x)` | `chart.timeAtIndex(chart.coordinateToLogical(x))` |
| `timeScale().subscribeVisibleLogicalRangeChange(h)` | `chart.on('visibleRangeChanged', h)` |
| `chart.priceScale('right').applyOptions({ scaleMargins, mode, invertScale, autoScale })` | `chart.applyPriceScaleOptions({ margins, mode, inverted, autoScale }, pane)` |
| `PriceScaleMode.Logarithmic` | `mode: 'logarithmic'`(`'normal'`, `'percentage'`, `'indexedTo100'`도 있음) |
| `chart.applyOptions({ crosshair: { mode: CrosshairMode.Magnet } })` | `chart.setCrosshairMode('magnet')` |
| `chart.subscribeCrosshairMove(h)` | `chart.on('crosshairMove', h)` |
| `chart.subscribeClick(h)` | `chart.on('click', h)` |
| `chart.applyOptions({ width, height })` / `resize(w, h)` | 캔버스의 CSS 크기를 설정하면 차트가 따라감 |
| `chart.remove()` | `chart.destroy()` |

논리 인덱스는 한 가지 점에서 다릅니다. FerroChart에서는 과거 데이터를 앞에
추가해도 봉이 자신의 논리 인덱스를 유지하므로, 첫 번째 봉의 인덱스가 음수일 수
있습니다. `chart.dataRange()`는 첫 번째 봉과 마지막 봉의 인덱스를 반환합니다.

각 네이티브 호출의 자세한 내용은 [시리즈](series.md), [데이터](data.md),
[시간 축](time-scale.md), [가격 축](price-scale.md),
[크로스헤어와 이벤트](crosshair-events.md), [마커](markers.md)를 참고하십시오.
