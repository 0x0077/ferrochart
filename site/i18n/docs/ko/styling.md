<!-- source: 7f51672705 -->
# 테마와 스타일

이 장에서는 차트의 모양과 페이지에 차트를 배치하는 방법을 다룹니다. 테마, 색상,
축과 여백, 상호작용, 크로스헤어, 많은 작은 차트, 이미지 내보내기, 출처 표시 로고,
인트로 애니메이션, GPU 렌더러, 크기 지정, 정리를 설명합니다.
별도로 언급하지 않는 한 여기 나오는 모든 내용은 `FerroPlot`에도 적용됩니다.

## 테마

테마는 차트를 만들 때 정합니다.

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' }); // 'light' (default) or 'dark'
```

테마는 배경, 그리드, 텍스트, 축, 크로스헤어 색상과 캔들의 기본 상승·하락 색상,
그리고 시리즈, 지표, 드로잉의 기본 색상을 정합니다.

테마는 객체로도 지정할 수 있습니다. `base` 테마(기본값 `'light'`) 위에 덮어쓰는 CSS
색상이며, 지정하지 않은 키는 `base`에서 가져옵니다. 이미 만든 차트의 테마는
`applyOptions`로 바꿀 수 있고, 보기와 데이터는 그대로 유지됩니다.

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  theme: { base: 'dark', background: '#0b0d12', up: '#30a0f0', down: '#f09030', fontSize: 13 },
});
chart.applyOptions({ theme: 'light' });
```

키는 `background`, `text`, `grid`, `axisLine`, `up`, `down`, `upWick`, `downWick`, `volumeUp`, `volumeDown`, `crosshair`, `crosshairLabelBg`, `crosshairLabelText`, `line`, `fontSize`, `axisPadding`입니다. 알 수 없는 키는 오류입니다.
`chart.options().theme`으로 현재 테마를 읽을 수 있습니다.

설정하지 않았거나 `null`로 설정한 색상은 테마를 따릅니다. 필요한 곳에만 색상을
명시적으로 지정하면 나머지는 두 테마 모두에서 일관되게 유지됩니다.

## 시리즈 색상

시리즈는 생성 시 또는 나중에 `series.applyOptions`로 지정한 `SeriesOptions`에서
색상을 가져옵니다.

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

| 옵션 | 용도 |
|---|---|
| `color` | 라인, 영역, 베이스라인(기준선 위) 시리즈의 선 색상, 상승 캔들과 봉, 히스토그램 막대 |
| `downColor` | 하락 캔들과 봉, 베이스라인 시리즈의 기준선 아래 선 |
| `topFill` | 영역 위쪽 채우기, 베이스라인의 기준선 위 채우기 |
| `bottomFill` | 영역 아래쪽 채우기, 베이스라인의 기준선 아래 채우기 |
| `lineWidth` | CSS px 단위의 선 두께 |
| `lineStyle` | `'solid'`, `'dashed'` 또는 `'dotted'` |

영역 채우기는 `topFill`에서 `bottomFill`로 이어지는 그라디언트입니다. 히스토그램
시리즈는 막대별 색상(`setData`의 `color` 열)도 받습니다. 모든 시리즈 옵션과 각
시리즈 종류가 그리는 내용은 [시리즈](series.md)에 있습니다.

다른 객체에는 각자의 색상 옵션이 있습니다. 가격선(`createPriceLine({ color })`),
마커(마커별 `color`), 두 시리즈 사이의 밴드(`addBand(upper, lower, { fill,
fillCrossed })`), 드로잉(`style: { line, fill, text }`)입니다.
[시리즈](series.md), [마커](markers.md), [지표](indicators.md),
[드로잉](drawings.md)을 참고하세요.

## 색상

색상은 다음 형태 중 하나의 CSS 색상 문자열입니다(타입 `Color`).

| 형태 | 예 |
|---|---|
| `#rgb`, `#rgba` | `'#f80'`, `'#f808'` |
| `#rrggbb`, `#rrggbbaa` | `'#ff8800'`, `'#ff880080'` |
| `rgb(r, g, b)` | `'rgb(255, 136, 0)'` |
| `rgba(r, g, b, a)` | `'rgba(255, 136, 0, 0.5)'`, 알파는 0~1 |
| `transparent` | `'transparent'` |

채널 값은 0~255이며 쉼표, 공백 또는 `/`로 구분합니다. 색상 이름(`'red'`),
`hsl()`, 백분율은 받지 **않습니다**. 호출 시 예외가 발생합니다(예:
`'color': invalid color 'red'`).

`options()`에서 다시 읽은 색상은 `#rrggbb`로, 불투명하지 않으면
`rgba(r, g, b, a)`로 표기됩니다. 테마를 따르는 색상은 `null`로 읽힙니다.

점별 색상 열(히스토그램 데이터, 플롯 마크)은 `0xRRGGBBAA` 값의 `Uint32Array`도
받으므로, 대용량 데이터에서 문자열을 만들지 않아도 됩니다.

## 축, 격자, 여백

앱과 표 안의 차트에는 축이 아예 없는 경우가 많습니다. `priceAxis`와 `timeAxis`로
축을 보이거나 숨기고, `grid`로 격자선(`true`, `false` 또는
`{ horizontal, vertical }`)을, `padding`으로 플롯 주위 여백(CSS px, 숫자 또는
`{ top, right, bottom, left }`)을 정합니다. 숨긴 축의 공간은 플롯이 씁니다.

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  priceAxis: false,
  grid: { vertical: false },
  padding: { top: 72, bottom: 40 },
});
chart.applyOptions({ timeAxis: false });
```

이 옵션들과 다음 두 절의 옵션은 `create`와 `applyOptions` 모두에서 쓸 수 있고
`chart.options()`로 읽을 수 있습니다. Python에서는 snake_case 키워드 인수
(`Chart(preset="sparkline")`, `chart.apply_options(price_axis=False)`), C에서는
`FerroChartAppearance`, `FerroChartInteraction`, `FerroChartCrosshairStyle`,
`FerroChartTheme` 구조체입니다.

## 상호작용

`interaction`은 이동, 확대/축소, 크로스헤어를 켜고 끕니다. `{ pan, zoom, crosshair }`
또는 모두 끄는 `false`입니다. 꺼진 입력은 차트가 쓰지 않으므로 터치스크린에서는
페이지가 평소처럼 스크롤되고 확대됩니다. 클릭은 계속 보고됩니다.

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { interaction: { pan: false, zoom: false } });
```

## 크로스헤어 스타일

`crosshairStyle`은 크로스헤어의 어느 부분을 그릴지 정합니다. `vertical`과 `horizontal`
선, 축의 `labels`, 첫 시리즈 값 위치의 `dot`(`{ radius, color }`, 그리지 않으려면
`null`, 색상 기본값은 그 시리즈의 색)입니다. 세로선과 점은 시세 카드에서 흔히 쓰는
크로스헤어입니다.

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  crosshairStyle: { horizontal: false, labels: false, dot: { radius: 5 } },
});
```

## 많은 작은 차트

`preset: 'sparkline'`은 표 셀에 넣는 차트입니다. 축과 격자가 없고 여백은 2이며
상호작용이 없고, 크로스헤어는 세로선과 점입니다. 함께 지정한 옵션은 그 위에 적용됩니다.

브라우저는 페이지당 WebGL 컨텍스트를 16개 정도만 유지하며, 차트마다 자기 GPU
장치를 가집니다. 작은 차트가 많은 페이지에서는 풀에서 차트를 만드세요. 장치
하나(WebGL2에서는 컨텍스트 하나)를 공유합니다. 뷰포트 밖의 차트는 그리지 않고,
다시 스크롤되어 들어올 때 그립니다.

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvases: HTMLCanvasElement[];

const pool = await FerroChart.createPool();
const charts = await Promise.all(canvases.map((c) => pool.create(c, { preset: 'sparkline' })));
// …
pool.destroy(); // destroys every chart of the pool
```

WebGL2에서는 풀의 차트가 2D 컨텍스트로 자기 캔버스를 그리므로, 캔버스에 다른
컨텍스트가 있으면 안 됩니다.

## 이미지 내보내기

`chart.toBlob(type?, quality?)`는 한 프레임을 그려 PNG(기본값), WebP 또는 JPEG
`Blob`으로 돌려줍니다. `chart.toImageBitmap()`은 `ImageBitmap`을 돌려줍니다.
이미지의 픽셀 크기는 캔버스와 같고, 캔버스 밖에 있는 출처 표시 로고는 들어가지
않습니다. `FerroPlot`에도 같은 메서드가 있습니다.

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const png = await chart.toBlob();
const jpeg = await chart.toBlob('image/jpeg', 0.9);
```

## 출처 표시 로고

기본적으로 가장 아래 패널의 왼쪽 아래 모서리, 시간 축 바로 위에 작은 FerroChart
로고가 표시됩니다. ferrochart.com으로 연결되며 포인터를 올리면 이름이 표시됩니다.

로고는 라이선스 요구 사항이 아니라 선의의 표시이므로 끌 수 있습니다.
`attributionLogo`를 지정하지 않으면 로고는 320 × 160 CSS px 이상인 캔버스에서만
표시되며, 캔버스 크기가 이 경계를 넘나들 때 나타나거나 사라집니다. `true`나
`false`를 지정하거나 `setAttributionLogo`를 호출하면 고정됩니다.

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { attributionLogo: false });

chart.setAttributionLogo(true);  // show it
chart.setAttributionLogo(false); // remove it
```

배치 방식:

- 캔버스 바로 뒤에 삽입되어 캔버스 옆에 절대 위치로 놓이는 `<a>` 요소입니다.
  캔버스 안에 그려지지 않으므로 스크린샷이나 엔진이 렌더링한 이미지에는 나타나지
  않습니다.
- 레이아웃이 바뀌면 위치가 갱신됩니다. 캔버스와 마찬가지로 캔버스의 offset
  parent를 기준으로 배치됩니다.
- 로고를 추가할 시점에 캔버스에 부모 요소가 없으면 로고는 추가되지 않습니다.

`FerroPlot`에도 같은 옵션과 메서드가 있으며, 로고는 플롯 영역의 왼쪽 아래에
놓입니다. `attachSidePlot`으로 차트에 붙인 플롯은 차트의 로고가 둘 다를 대표하므로
자신의 로고를 잃습니다([플롯](plots.md) 참고).

## 인트로 애니메이션

`playIntro`는 차트를 빈 상태(그리드와 축)로 보여 준 다음 데이터를 왼쪽에서
오른쪽으로 그립니다. 랜딩 페이지나 사용자가 종목을 열 때처럼 차트가 처음 나타날
때 사용하세요.

```ts
import { FerroChart, randomWalk } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' });
chart.addCandlestickSeries().setData(randomWalk(1, 300, Date.UTC(2024, 0, 1), 3_600_000));

await chart.playIntro({ duration: 1200, delay: 200, pen: '#4cc9f0' });
// The animation has ended (or was ended early).
```

`IntroOptions`:

| 옵션 | 의미 | 기본값 |
|---|---|---|
| `duration` | 데이터를 왼쪽에서 오른쪽으로 그리는 시간, ms | `900` |
| `delay` | 먼저 빈 차트를 보여 주는 시간, ms | `0` |
| `pen` | 그리는 가장자리에 빛나는 선의 색상, 없으면 `null` | `null` |

동작:

- 차트는 다음 프레임 전에 즉시 빈 상태로 표시되므로, 전체 차트가 잠깐 보이는
  것을 피하려면 `setData` 직후에 `playIntro`를 호출하세요.
- 가격선, 값 레이블, 크로스헤어는 마지막에 나타납니다.
- 캔버스에서 누르기, 휠, 핀치, 더블클릭, 키 입력이 있으면 일찍 끝납니다. 차트
  위에서 포인터를 움직이는 것만으로는 끝나지 않습니다.
- `chart.finishIntro()`는 즉시 끝냅니다.
- `playIntro`를 다시 호출하면 실행 중인 애니메이션을 먼저 끝냅니다.
- 반환된 promise는 애니메이션이 어떤 식으로 끝나든(차트가 제거될 때도 포함)
  끝나면 resolve됩니다. reject되는 일은 없으며, 유효하지 않은 옵션(음수이거나
  유한하지 않은 `duration` 또는 `delay`, 유효하지 않은 `pen` 색상)은 호출 자체에서
  예외를 던집니다.

`FerroPlot`에서 `playIntro`는 마크를 왼쪽에서 오른쪽으로 그리며, 규칙선 레이블과
호버 강조는 마지막에 나타납니다.

## 렌더러

FerroChart는 GPU로 그립니다. 브라우저가 지원하면 WebGPU를, 그렇지 않으면
WebGL2를 사용합니다. Canvas 2D 대체 경로는 없습니다.

| `renderer` 옵션 | 동작 |
|---|---|
| `'auto'` (기본값) | WebGPU, 그다음 WebGL2 |
| `'webgpu'` | WebGPU만 |
| `'webgl2'` | WebGL2만 |

요청한 API를 쓸 수 없으면(`'WebGPU is not
available'`, `'WebGL2 is not available'`, 또는 둘을 `;`로 이은 메시지) 또는
알 수 없는 옵션 값이면 `create`가 reject됩니다. `renderer` 속성으로 사용 중인
API를 알 수 있습니다.

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

`'webgl2'`를 강제하면 WebGPU가 있는 브라우저에서 대체 경로를 간단히 테스트할 수
있습니다.

## 캔버스 크기 지정

캔버스 크기는 CSS로 정합니다. FerroChart는 고밀도 화면에서도 선명하도록 캔버스의
백킹 스토어(`canvas.width`와 `canvas.height`)를 CSS 크기에 `devicePixelRatio`를
곱한 값으로 직접 설정합니다. 캔버스에 `width`와 `height`를 직접 설정하지 마세요.

```html
<div style="height: 60vh">
  <canvas id="chart" style="display: block; width: 100%; height: 100%"></canvas>
</div>
```

- `ResizeObserver`가 캔버스의 content box를 감시합니다. CSS 크기가 바뀌면 백킹
  스토어와 레이아웃이 따라 바뀌고 차트가 다시 그려집니다. 크기 조정 메서드를
  호출할 일은 없습니다.
- `devicePixelRatio`가 바뀌면(브라우저 확대·축소, 창을 다른 화면으로 이동) 백킹
  스토어도 크기가 조정됩니다.
- 백킹 스토어는 최소 1 × 1 px입니다. 차트를 만들 때 숨겨진 요소 안에 있었다면,
  요소에 크기가 생기는 즉시 그려집니다.
- `display: block`은 인라인 캔버스 아래에 생기는 몇 픽셀의 공간을 없애 줍니다.
- flex나 grid 레이아웃 안에서는 컨테이너가 캔버스를 줄일 수 있도록 캔버스에
  `min-height: 0`(또는 `min-width: 0`)을 지정하세요.

차트를 만들면 캔버스에 키 입력을 받을 수 있도록 `tabindex="0"`(이미 `tabindex`가
있으면 제외), 터치 제스처가 페이지 대신 차트를 이동·확대·축소하도록
`touch-action: none`(세로 스와이프를 페이지에 맡기는 `touchScroll: 'page'`에서는
`pan-y`, [터치스크린](web.md#touch-screens) 참고), 그리고 `outline: none`이
설정됩니다. 포인터가 차트 위에 있는 동안에는 커서 스타일이 설정됩니다.

## 정리

```ts
import { FerroChart, FerroPlot } from '@ferrochart/core';

declare const chart: FerroChart;
declare const plot: FerroPlot;

chart.destroy();
plot.destroy(); // FerroPlot's equivalent
```

`destroy()`는 다음을 수행합니다.

- 캔버스의 DOM 이벤트 리스너와 `on`으로 추가한 모든 콜백을 제거합니다.
- 차트의 동기화 그룹에서 탈퇴합니다.
- 출처 표시 로고를 제거합니다.
- 대기 중인 `playIntro` promise를 resolve합니다.
- GPU 리소스를 해제합니다.

캔버스 요소는 페이지에 남으므로, 더 이상 필요 없다면 직접 제거하세요.
`destroy()` 후에는 차트 객체를 사용할 수 없으며, 그 `Series`, `Indicator` 등의
핸들에 대한 호출은 예외를 던집니다(`the FerroChart was
destroyed`). 더 이상 표시하지 않는 차트는 제거하세요. 차트마다 GPU 메모리를
차지하며, 브라우저는 한 페이지가 가질 수 있는 WebGL 컨텍스트 수를 제한합니다.

React나 Vue에서는 컴포넌트가 마운트될 때 차트를 만들고 언마운트될 때
제거하세요. [React와 Vue](frameworks.md)를 참고하세요.
