<!-- source: de9b1ca7fd -->
# 플롯 (FerroPlot)

`FerroChart`는 시간 순서의 봉을 중심으로 만들어졌습니다. 변동성 스마일, 수익률
히스토그램, 섹터별 막대 차트, 손익 워터폴, 상관관계 히트맵처럼 그 밖의 모든 것에는
`FerroPlot`을 사용하십시오. 숫자, 로그, 카테고리, 날짜 축을 지원하는 범용 x/y
플롯입니다. 차트와 같은 렌더러(WebGPU, 그다음 WebGL2), 같은 캔버스 처리, 같은
테마를 사용합니다.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const plot = await FerroPlot.create(canvas, { theme: 'dark' });

const smile = plot.addMark('line', { pointSize: 5 });
smile.setData({
  x: [80, 90, 100, 110, 120],          // strikes
  y: [0.31, 0.26, 0.24, 0.25, 0.29],   // implied volatility
});
plot.axis('right').applyOptions({ format: { type: 'percent', decimals: 0 }, title: 'IV' });
plot.axis('x').applyOptions({ title: 'Strike' });
```

플롯은 다음으로 구성됩니다.

- **마크**: 라인, 영역, 산점도, 막대, 워터폴, 셀. 각각 고유한 데이터와 옵션을
  가집니다.
- 세 개의 **축**: 아래쪽의 `'x'`, `'right'`, `'left'`(기본적으로 숨김).
- **룰**: 특정 값 위치의 수평선이나 수직선, 또는 음영 밴드.
- **레이블**: 데이터 좌표에 놓이는 텍스트나 기호.

## 플롯 만들기

`FerroPlot.create(canvas, options?)`는 `FerroChart.create`와 마찬가지로 Promise를
반환하며, 처음 사용할 때 WebAssembly 모듈을 불러옵니다. 캔버스 크기는 CSS로
정합니다. [테마와 스타일](styling.md#sizing-the-canvas)을 참고하십시오.

| 옵션 | 값 | 기본값 |
|---|---|---|
| `theme` | `'light'`, `'dark'` | `'light'` |
| `renderer` | `'auto'`, `'webgpu'`, `'webgl2'` | `'auto'` |
| `crosshair` | 마우스가 올라간 요소를 지나는 크로스헤어 선 | `false` |
| `hoverHighlight` | 마우스가 올라간 요소 강조 | `true` |
| `attributionLogo` | 플롯 영역 왼쪽 아래의 FerroChart 로고 | `true` |

`FerroPlot`의 그 밖의 멤버:

| 멤버 | 설명 |
|---|---|
| `addMark(kind, options?): Mark` | 마크를 추가합니다. 앞서 추가한 마크 위에 그려집니다 |
| `axis(name): Axis` | `'x'`, `'left'` 또는 `'right'` |
| `addRule(options): Rule` | 특정 값 위치의 선이나 밴드 |
| `addLabel(options): Label` | 특정 지점의 텍스트나 기호 |
| `fit()` | 모든 축을 설정된 범위로 되돌립니다. 자동 축은 다시 데이터를 따라갑니다 |
| `on(event, callback)`, `off(event, callback)` | 이벤트: `hover`, `click`, `viewChange` |
| `playIntro(options?)`, `finishIntro()` | 인트로 애니메이션(차트와 동일) |
| `setAttributionLogo(on)` | 로고를 표시하거나 제거합니다 |
| `renderer` | `'webgpu'` 또는 `'webgl2'` |
| `destroy()` | 리스너와 로고를 제거하고 GPU 리소스를 해제합니다(`dispose()`는 이전 이름으로 남아 있는 같은 메서드) |

사용자는 플롯을 이동하고 확대·축소할 수 있습니다. 드래그하면 이동하고, 휠이나
핀치는 포인터를 중심으로 x축을 확대·축소합니다(y축 위에서 또는 Alt를 누른 채로
하면 해당 축을 확대·축소). Shift+휠과 가로 스크롤은 이동, 축을 따라 드래그하면 그
축을 확대·축소, 더블 클릭하면 맞춤입니다. 카테고리 축은 이동은 되지만 확대·축소는
되지 않습니다.

## 마크

`plot.addMark(kind, options?)`는 `MarkOptions`를 적용한 `MarkKind` 종류의 마크를
추가하고 `Mark` 핸들을 반환합니다.

| 종류 | 그리는 내용 | 사용하는 데이터 |
|---|---|---|
| `'line'` | 데이터 순서대로 이은 점 | `x`, `y` |
| `'area'` | 선과 `y0`(또는 `baseValue`) 사이의 영역 | `x`, `y`, `y0?` |
| `'scatter'` | 점마다 하나의 기호 | `x`, `y`, `size?`, `value?`, `color?`, `label?` |
| `'bars'` | `y0`(또는 `baseValue`)에서 `y`까지의 막대 | `x`, `y`, `y0?`, `value?`, `color?`, `label?` |
| `'waterfall'` | 카테고리 축 위의 누적 합계 막대 | `x`, `y`, `step?` |
| `'cells'` | 격자의 색칠된 셀(히트맵) | `x`, `y`, `value` |

### 데이터

`mark.setData(data)`는 데이터를 교체합니다. 모든 컬럼의 길이는 같아야 하며, 타입
배열은 컬럼당 한 번의 `memcpy`로 복사됩니다.

| 컬럼 | 타입 | 의미 |
|---|---|---|
| `x` | `Float64Array \| number[]` | 숫자, UTC ms(날짜 축) 또는 카테고리 번호. 가로 막대에서는 값 |
| `y` | `Float64Array \| number[]` | 값. `NaN`은 공백(갭). 가로 막대에서는 y축 위의 위치 |
| `y0` | `Float64Array \| number[]` | 영역의 아래쪽 경계, 막대의 시작점 |
| `value` | `Float64Array \| number[]` | 색상 스케일로 색을 칠할 값(cells, bars, scatter) |
| `size` | `Float32Array \| number[]` | 산점도 지름, CSS px |
| `color` | `(string \| null)[] \| Uint32Array` | 점별 색상(`null`: 마크 색상) 또는 `0xRRGGBBAA` |
| `step` | `('delta' \| 'subtotal' \| 'total')[]` | 워터폴 역할. 기본값은 모두 `'delta'` |
| `label` | `(string \| null)[]` | 점별 텍스트: 산점도 레이블. 막대와 셀에서는 값 텍스트를 대체 |

컬럼 길이가 서로 다르거나, `x`가 유한하지 않거나, 카테고리 축의 값이 0 이상의
정수가 아니면 `setData`는 예외를 발생시키고 이전 데이터를 유지합니다.

데이터를 바꾸는 방법은 세 가지입니다.

| 메서드 | 용도 |
|---|---|
| `setData(data)` | 전체 교체 |
| `append(data)` | 끝에 점 추가. `data`는 마크의 데이터와 같은 컬럼을 가져야 합니다 |
| `update(point)` | 마지막 점 교체(예: 실시간 값). O(1), 할당 없음 |

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const pnl = plot.addMark('area', { color: '#26a69a', downColor: '#ef5350' });
plot.axis('x').applyOptions({ format: 'date' });

const t0 = Date.UTC(2024, 0, 2);
pnl.setData({ x: [t0, t0 + 60_000, t0 + 120_000], y: [0, 120, 80] });
pnl.append({ x: [t0 + 180_000], y: [150] });   // a new point
pnl.update({ x: t0 + 180_000, y: 165 });         // the last point changed
```

`update`는 `MarkPoint`(`{ x, y, y0?, value?, size?, color? }`)를 받으며, 마크에
없는 컬럼에 해당하는 필드는 무시합니다. 마크에 데이터가 없거나 `x`가 유한하지
않으면 예외가 발생합니다. 색상 스케일의 도메인과 자동 셀 크기는 마지막
`setData` 또는 `append`에서 계산한 값을 유지합니다.

### 옵션

옵션은 `addMark`에 전달하고 `mark.applyOptions(options)`로 변경합니다. 생략한
키는 기존 값을 유지합니다. `mark.options()`는 모든 옵션을 반환합니다. 색상이
`null`이면 기본값을 뜻합니다. `color`는 플롯 팔레트의 다음 색(첫 번째 마크는
테마의 선 색상), 나머지는 테마의 색상입니다.

| 옵션 | 적용 대상 | 의미 | 기본값 |
|---|---|---|---|
| `yAxis` | 전체 | `'left'` 또는 `'right'` | `'right'` |
| `orientation` | bars, waterfall | `'vertical'` 또는 `'horizontal'` | `'vertical'` |
| `color` | 전체 | 주 색상 | 팔레트 |
| `downColor` | area, waterfall | 기준값 아래의 영역, 감소하는 워터폴 항목 | 테마 |
| `totalColor` | waterfall | 소계와 합계 | 테마 |
| `fillAlpha` | area | 채우기 불투명도, 0~1 | `0.25` |
| `lineWidth` | line, area | CSS px, 0보다 큼 | `2` |
| `lineStyle` | line, area | `'solid'`, `'dashed'`, `'dotted'` | `'solid'` |
| `stepLine` | line, area | 계단형: 다음 x까지 수평, 그다음 수직 | `false` |
| `pointSize` | line, scatter | 라인: 각 점의 기호(0: 없음). 산점도: `size` 컬럼이 없을 때의 지름 | `0` |
| `icon` | line, scatter | `'circle'`, `'square'`, `'arrowUp'`, `'arrowDown'` | `'circle'` |
| `baseValue` | area, bars | `y0`가 없는 영역과 막대의 시작점 | `0` |
| `group` | bars | 같은 그룹의 막대는 한 슬롯을 나란히 나눠 씀. 없으면 `null` | `null` |
| `stack` | bars, area | 같은 스택의 마크는 누적됨(양수와 음수는 따로). 없으면 `null` | `null` |
| `band` | bars, cells | 슬롯에서 차지하는 비율, 0(제외)~1. `null`이면 기본값(막대 0.8, 셀 1) | `null` |
| `colorScale` | cells, bars, scatter | `value`를 색으로 매핑하는 방식(아래 참고). `null`이면 기본 순차 스케일 | `null` |
| `showValues` | bars, waterfall, cells | 공간이 충분하면 각 요소에 값을 표시 | `false` |
| `valueFormat` | bars, waterfall, cells | 그 값의 형식([형식](#formats) 참고) | `'auto'` |
| `cellSize` | cells | 숫자 축에서 데이터 단위의 `[width, height]`. `null`: 서로 다른 x 사이와 서로 다른 y 사이의 가장 작은 간격 | `null` |
| `hoverable` | 전체 | 호버와 클릭으로 히트됨 | `true` |
| `visible` | 전체 | 그려짐 | `true` |

잘못된 값은 예외를 발생시킵니다. 예를 들어 `'fillAlpha' must be between 0 and 1`
또는 `'icon' must be 'circle', 'square', 'arrowUp' or 'arrowDown'`입니다.

`mark.id`는 플롯 안에서 고유하고, `mark.kind`는 종류이며, `mark.remove()`는
마크를 제거합니다.

### 카테고리 축의 막대

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const x = plot.axis('x');
x.applyOptions({ type: 'category' });
x.setCategories(['Tech', 'Energy', 'Banks', 'Health']);

// Two series side by side in each slot.
const q1 = plot.addMark('bars', { group: 1, showValues: true });
const q2 = plot.addMark('bars', { group: 1, showValues: true });
q1.setData({ x: [0, 1, 2, 3], y: [4.2, -1.3, 2.1, 0.8] });
q2.setData({ x: [0, 1, 2, 3], y: [3.1, 0.6, -0.4, 1.9] });

plot.axis('right').applyOptions({ format: { type: 'percent', decimals: 1 } });
plot.addRule({ value: 0, lineStyle: 'solid' });
```

카테고리 `k`는 `k`부터 `k + 1`까지의 슬롯을 차지하며 `labels[k]`로 레이블됩니다.
`group: 1` 대신 `stack: 1`을 쓰면 막대가 쌓입니다. 가로 막대를 만들려면
`orientation: 'horizontal'`을 지정하고, 왼쪽 또는 오른쪽 축을 카테고리 축으로 만든
뒤 컬럼의 의미를 바꾸십시오. `x`에 값을, `y`에 카테고리 번호를 넣습니다.

### 워터폴

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const x = plot.axis('x');
x.applyOptions({ type: 'category' });
x.setCategories(['Revenue', 'COGS', 'Gross', 'Opex', 'Tax', 'Net']);

const pnl = plot.addMark('waterfall', {
  color: '#26a69a',       // increases
  downColor: '#ef5350',   // decreases
  totalColor: '#5d6d7e',  // subtotals and totals
  showValues: true,
  valueFormat: 'compact',
});
pnl.setData({
  x: [0, 1, 2, 3, 4, 5],
  y: [1_200_000, -450_000, 0, -380_000, -90_000, 0],
  step: ['delta', 'delta', 'subtotal', 'delta', 'delta', 'total'],
});
```

`'delta'`는 자신의 값을 누적 합계에 더합니다. `'subtotal'`이나 `'total'`은
0에서 누적 합계까지 막대를 그리고 누적 합계는 바꾸지 않으며, 그 `y`는 사용되지
않습니다. 호버와 클릭 이벤트에서 소계나 합계의 `hit.y`는 누적 합계입니다.

### 크기, 색상, 레이블이 있는 산점도

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const assets = plot.addMark('scatter', { colorScale: 'diverging' });
assets.setData({
  x: [12.1, 18.4, 25.0, 9.8],          // volatility, %
  y: [6.2, 9.1, 14.7, 3.9],            // return, %
  size: [10, 18, 24, 8],               // diameter, CSS px
  value: [0.4, -0.2, 1.1, -0.6],       // e.g. Sharpe ratio, colored
  label: ['Bonds', 'Gold', 'Tech', 'Cash'],
});
plot.axis('x').applyOptions({ title: 'Volatility %' });
plot.axis('right').applyOptions({ title: 'Return %', includeZero: true });
```

### 셀(히트맵)

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const names = ['SPX', 'NDX', 'GLD', 'TLT'];
const corr = [
  [1, 0.92, 0.05, -0.31],
  [0.92, 1, 0.02, -0.35],
  [0.05, 0.02, 1, 0.21],
  [-0.31, -0.35, 0.21, 1],
];

for (const name of ['x', 'right'] as const) {
  const a = plot.axis(name);
  a.applyOptions({ type: 'category' });
  a.setCategories(names);
}

const x: number[] = [], y: number[] = [], value: number[] = [];
corr.forEach((row, i) => row.forEach((v, j) => { x.push(j); y.push(i); value.push(v); }));

const cells = plot.addMark('cells', {
  colorScale: { center: 0, domain: [-1, 1] },
  showValues: true,
  valueFormat: { type: 'fixed', decimals: 2 },
});
cells.setData({ x, y, value });
```

숫자 축에서 셀의 너비와 높이는 데이터 단위로 `cellSize`이거나, 지정하지 않으면
서로 다른 `x` 값 사이와 서로 다른 `y` 값 사이의 가장 작은 간격만큼입니다.

## 축

`plot.axis(name)`은 `'x'`, `'left'` 또는 `'right'` 축을 반환합니다. 왼쪽 축은
기본적으로 숨겨져 있으며, 마크는 `yAxis: 'left'`로 이 축을 사용합니다.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('left').applyOptions({ visible: true, title: 'Volume', format: 'compact' });
plot.addMark('bars', { yAxis: 'left', color: 'rgba(120, 120, 120, 0.5)' });
plot.addMark('line', { yAxis: 'right' });
```

### `AxisOptions`

`axis.applyOptions(options)`는 옵션을 변경합니다. 생략한 키는 기존 값을
유지합니다. `axis.options()`는 모든 옵션을 반환합니다.

| 옵션 | 의미 | 기본값 |
|---|---|---|
| `type` | `'linear'`, `'log'` 또는 `'category'` | `'linear'` |
| `range` | 데이터를 따라가는 `'auto'` 또는 고정된 `[lo, hi]` | `'auto'` |
| `includeZero` | 자동 범위: 0을 범위 안에 포함 | `false` |
| `margin` | 자동 범위: 양 끝에 더하는 비율, 0~1 | `0.05`(카테고리: `0`) |
| `format` | 눈금 레이블의 표기 방식(아래 참고) | `'auto'` |
| `inverted` | 값이 오른쪽(x) 또는 위쪽(y)으로 갈수록 감소 | `false` |
| `visible` | 눈금, 레이블, 축선을 그림 | `true`(왼쪽: `false`) |
| `grid` | 눈금 위치의 격자선(카테고리 축: 슬롯 경계) | `true`(카테고리: `false`) |
| `title` | 축 옆의 텍스트. `null`이면 제거 | `null` |
| `locked` | 이동과 확대·축소가 이 축을 건드리지 않음 | `false` |

`type`을 바꾸면 `grid`, `margin`, `range`는 해당 타입의 기본값에서 다시
시작하며, `visible`, `title`, `format`, `inverted`, `locked`는 유지됩니다.

자동 범위는 그 축에 있는 마크의 보이는 데이터를 포함합니다. 막대, 영역,
워터폴에서는 0인 끝에 여백을 두지 않으므로 막대가 축 위에 붙어 섭니다. 카테고리
축은 모든 카테고리를 포함합니다.

오류: 고정 범위는 서로 다른 두 개의 유한한 수여야 하며, 로그 축에서는 양수여야
합니다.

### 로그 축

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('right').applyOptions({ type: 'log' });
```

0 이하의 값은 로그 축에서 위치가 없으므로 그려지지 않습니다.

### 카테고리 축

카테고리 축은 같은 크기의 슬롯으로 이루어집니다. 카테고리 `k`는 `[k, k + 1)`을
차지하고 중심은 `k + 0.5`입니다. 이 축의 데이터는 0 이상의 정수입니다.
`axis.setCategories(labels)`로 카테고리에 이름을 붙입니다(최대 1 000 000개).

### 날짜 축

날짜 축은 값이 UTC 밀리초이고 `'date'` 형식을 쓰는 선형 축입니다.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('x').applyOptions({ format: 'date' });                              // UTC
plot.axis('x').applyOptions({ format: { type: 'date', utcOffsetMinutes: 60 } }); // UTC+1
```

### 형식

`PlotFormat`은 `AxisOptions.format`과 `MarkOptions.valueFormat`에서 사용합니다.

| 형식 | 표기 |
|---|---|
| `'auto'` | 최소 소수 2자리와 유효숫자 4자리. 작은 값은 0의 개수를 아래 첨자로 표기(차트 가격 레이블과 같음) |
| `'compact'` | K, M, B 접미사 |
| `{ type: 'fixed', decimals }` | 고정 소수 자릿수, 0~12 |
| `{ type: 'percent', decimals }` | 고정 소수 자릿수와 `%` 기호. 값은 이미 퍼센트 단위(`5`는 `5%`로 표기) |
| `'date'` | 값은 UTC ms이며 UTC 기준 날짜와 시간으로 표기 |
| `{ type: 'date', utcOffsetMinutes }` | 위와 같되 UTC에서의 오프셋 적용, −1440~1440분 |

### 보이는 범위

| 메서드 | 설명 |
|---|---|
| `axis.view(): [number, number]` | 마지막 프레임 기준의 보이는 범위 |
| `axis.setView(lo, hi)` | `lo..hi`를 표시합니다(카테고리 축에서는 카테고리 단위). `plot.fit()`을 호출할 때까지 축이 데이터를 따라가지 않습니다. `viewChange` 이벤트를 보내지 않습니다 |
| `plot.fit()` | 모든 축을 설정된 범위로 되돌립니다 |

## 룰

룰은 특정 값 위치에서 플롯을 가로지르는 선이거나, 두 값 사이의 음영 밴드입니다.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const target = plot.addRule({ value: 100, label: 'target' });          // horizontal line on the right axis
plot.addRule({ axis: 'x', value: Date.UTC(2024, 5, 1), label: 'FOMC' }); // vertical line
plot.addRule({ value: 30, to: 70, color: 'rgba(120, 120, 255, 0.12)' }); // band

target.applyOptions({ value: 105 });
target.remove();
```

| 옵션 | 의미 | 기본값 |
|---|---|---|
| `axis` | `value`의 축. `'x'`이면 수직 룰 | `'right'` |
| `value` | 위치 | `0` |
| `to` | 밴드의 다른 쪽 경계. 선이면 `null` | `null` |
| `color` | 색상. `null`이면 테마의 크로스헤어 색상(밴드는 불투명도 12%) | `null` |
| `lineWidth` | CSS px | `1` |
| `lineStyle` | `'solid'`, `'dashed'`, `'dotted'` | `'dashed'` |
| `label` | 축 위 해당 값 위치의 텍스트. 없으면 `null` | `null` |
| `labelBackground` | 레이블을 룰 색상의 상자 위에 표시(그렇지 않으면 룰 색상의 글자로 표시) | `true` |

## 레이블

레이블은 데이터 좌표의 한 지점에 놓이는 텍스트, 기호 또는 둘 다입니다.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const peak = plot.addLabel({ x: 3, y: 14.7, text: 'peak', icon: 'arrowDown', align: 'center', offset: [0, -14] });
peak.remove();
```

| 옵션 | 의미 | 기본값 |
|---|---|---|
| `x`, `y` | 지점(필수) | |
| `yAxis` | `'left'` 또는 `'right'` | `'right'` |
| `text` | 텍스트(아이콘이 있으면 비워 둘 수 있음) | `''` |
| `icon` | `'circle'`, `'square'`, `'arrowUp'`, `'arrowDown'` 또는 `null` | `null` |
| `color` | `null`이면 테마의 텍스트 색상 | `null` |
| `align` | 지점 기준 텍스트 정렬: `'left'`, `'center'`, `'right'` | `'left'` |
| `offset` | 지점으로부터 텍스트의 오프셋, 오른쪽과 아래쪽으로 CSS px | `[6, 0]` |

레이블은 변경할 수 없습니다. 제거하고 새로 추가하십시오.

## 색상 스케일

셀은 항상 `value` 컬럼을 색상 스케일로 칠합니다. 막대와 산점도 마크는 `value`
컬럼이 있으면 그렇게 하고, 없으면 `color`를 사용합니다. `color` 컬럼에 있는 점
고유의 색상이 값보다 우선합니다.

| `colorScale` | 의미 |
|---|---|
| `null` 또는 `'sequential'` | 낮은 값에서 높은 값으로 짙은 파랑에서 노랑 |
| `'diverging'` | 0을 중심으로 테마의 하락 색상, 중립적인 중간색, 상승 색상 |
| `{ stops?, center?, domain?, quantile? }` | 사용자 정의 스케일 |

객체 형식:

| 키 | 의미 |
|---|---|
| `stops` | `[position, color][]`. 위치는 0~1의 오름차순(최소 하나의 정지점) |
| `center` | 이 값을 중심으로 발산형 스케일을 만듭니다. 스케일의 가운데가 `center`에 오고, 중심에서 더 먼 쪽이 양쪽의 범위를 결정합니다 |
| `domain` | 고정된 `[lo, hi]`. 범위 밖의 값은 양 끝 색상을 받습니다 |
| `quantile` | `domain`이 없을 때: 스케일이 `quantile` 분위수부터 `1 − quantile` 분위수까지의 값을 포함하므로 이상값은 포화됩니다. 0~0.5, 기본값 0.01 |

`stops`가 없으면 `center`가 있는 스케일은 발산형 색상을, 없는 스케일은 순차형
색상을 사용합니다.

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const heat = plot.addMark('cells', {
  colorScale: { stops: [[0, '#0d1b4c'], [0.5, '#219ebc'], [1, '#f4e04d']], quantile: 0.05 },
});
```

도메인은 `setData`와 `append`가 데이터로부터 계산하며, `update`는 이를
유지합니다.

## 히트 테스트와 이벤트

플롯은 포인터 아래의 요소(라인이나 산점도의 점, 막대, 셀)를 찾습니다. 그 요소를
강조하고(`hoverHighlight`), 그 요소를 지나는 크로스헤어 선을 그리며
(`crosshair`), 이를 이벤트로 알립니다.

```ts
interface PlotEventMap {
  hover: { hit: PlotHit | null };                 // the hovered element changed
  click: { hit: PlotHit | null; x: number; y: number };
  viewChange: {};                                  // an axis range changed by pan, zoom or fit()
}

interface PlotHit {
  mark: Mark;      // compare with mark.id: it is a new handle each time
  index: number;   // index of the point in the mark's data
  x: number;
  y: number;       // waterfall subtotals and totals: the running total
  position: { x: number; y: number }; // where it is drawn, CSS px from the canvas' top-left
}
```

`click`에서 `x`와 `y`는 포인터 위치의 축 값입니다(`y`는 오른쪽 축 기준이며,
오른쪽 축이 숨겨져 있으면 왼쪽 축 기준). `hoverable: false`인 마크는 히트되지
않습니다.

```ts
import { FerroPlot, type PlotHit } from '@ferrochart/core';

declare const plot: FerroPlot;
declare const tooltip: HTMLElement;

const bars = plot.addMark('bars', { showValues: true });
bars.setData({ x: [0, 1, 2], y: [5, 6, 7] });

plot.on('hover', ({ hit }: { hit: PlotHit | null }) => {
  if (!hit || hit.mark.id !== bars.id) {
    tooltip.hidden = true;
    return;
  }
  tooltip.hidden = false;
  tooltip.textContent = `#${hit.index}: ${hit.y}`;
  tooltip.style.transform = `translate(${hit.position.x}px, ${hit.position.y}px)`;
});

plot.on('click', ({ hit, x, y }: { hit: PlotHit | null; x: number; y: number }) => {
  console.log(hit ? `clicked point ${hit.index}` : `empty space at ${x}, ${y}`);
});
```

이벤트는 해당 이벤트를 발생시킨 프레임이 그려진 뒤에 전달됩니다.

## 플롯을 차트 패널에 맞춰 정렬하기

`chart.attachSidePlot(plot, pane?)`는 플롯을 차트 패널 옆에 배치하여 같은 가격이
양쪽에서 같은 높이에 오도록 합니다. 차트는 매 프레임 플롯의 오른쪽 축을, 패널이
매핑하는 대로 플롯 영역의 맨 위와 맨 아래에 해당하는 가격으로 설정합니다. 페이지
위 캔버스의 위치를 매 프레임 읽으므로, 플롯은 차트 옆 어디에 있어도 되고 플롯
영역이 패널보다 높거나 낮아도 됩니다.

- 플롯의 오른쪽 축은 잠깁니다. 차트만이 이 축을 결정합니다. 연결할 때 패널의
  가격 축이 로그 스케일이면 오른쪽 축도 로그 축이 됩니다.
- 플롯의 출처 표시 로고는 제거되며, 차트의 로고가 둘 모두를 대표합니다.
- `chart.detachSidePlot(plot)`은 따라가기를 중지합니다. 오른쪽 축은 잠긴 채로
  마지막 범위를 유지합니다.

### 예제: 볼륨 프로파일

프로파일은 보이는 봉에서 각 가격대에 거래된 거래량을 캔들 옆에 가로 막대로
보여 줍니다.

```html
<div style="display: flex; height: 520px">
  <canvas id="chart" style="flex: 1; min-width: 0"></canvas>
  <canvas id="profile" style="width: 160px"></canvas>
</div>
```

```ts
import { FerroChart, FerroPlot, randomWalk, type OhlcData } from '@ferrochart/core';

const chart = await FerroChart.create(document.getElementById('chart') as HTMLCanvasElement, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
const bars: OhlcData = randomWalk(3, 3_000, Date.UTC(2024, 0, 1), 60_000);
candles.setData(bars);
chart.addVolumeSeries(candles);

const profile = await FerroPlot.create(document.getElementById('profile') as HTMLCanvasElement, {
  theme: 'dark',
  hoverHighlight: false,
});
profile.axis('x').applyOptions({ visible: false, grid: false });
profile.axis('right').applyOptions({ visible: false, grid: false }); // the chart shows the prices
const rows = profile.addMark('bars', { orientation: 'horizontal', color: 'rgba(41, 98, 255, 0.5)', band: 0.9 });
chart.attachSidePlot(profile);

/** Volume per price row of the bars `from..=to` (indices into `bars`). */
function volumeProfile(from: number, to: number, rowSize: number) {
  const byRow = new Map<number, number>();
  for (let i = from; i <= to; i++) {
    const row = Math.floor(bars.close[i] / rowSize);
    byRow.set(row, (byRow.get(row) ?? 0) + (bars.volume?.[i] ?? 0));
  }
  const y: number[] = [], x: number[] = [];
  for (const [row, volume] of byRow) {
    y.push((row + 0.5) * rowSize); // price at the middle of the row
    x.push(volume);
  }
  return { x, y };
}

const n = bars.close.length;
const update = (from: number, to: number) => {
  const a = Math.max(0, Math.floor(from));
  const b = Math.min(n - 1, Math.ceil(to));
  if (a <= b) rows.setData(volumeProfile(a, b, 0.5));
};
update(n - 200, n - 1);
chart.on('visibleRangeChanged', ({ from, to }: { from: number; to: number }) => update(from, to));
```

숫자 축 위의 가로 막대 두께는 `y` 값 사이의 가장 작은 간격에 `band`를 곱한
값이므로, `band: 1`이면 같은 크기의 행이 서로 맞닿습니다. 사용자가 차트를
스크롤하거나 확대·축소하면 `visibleRangeChanged`가 보이는 봉에 대해 프로파일을
다시 계산하고, 가격 범위는 저절로 따라갑니다. 캔들 옆에 호가창 깊이를 표시하려면
같은 구성에서 매수·매도 호가 잔량을 `x`로, 그 가격을 `y`로 사용하고, 마지막
호가 단계의 실시간 변화에는 `rows.update`를 사용하십시오.

## 정리

`plot.destroy()`는 플롯의 리스너와 로고를 제거하고 GPU 리소스를 해제합니다. 사이드
플롯이라면 차트 따라가기도 중지합니다. 그 뒤로 `Mark`, `Axis`, `Rule`, `Label`
핸들은 예외(`the FerroPlot was destroyed`)를 발생시킵니다. 캔버스 요소는 페이지에
그대로 남습니다.
