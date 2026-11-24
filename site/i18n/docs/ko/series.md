<!-- source: bec9dd6da7 -->
# 시리즈 유형

시리즈는 한 가지 방식으로 그려지는 하나의 데이터 집합입니다. 캔들, 라인,
히스토그램 등이 있습니다. 차트는 여러 패널에 걸쳐 원하는 만큼 시리즈를 담을 수
있습니다. 차트의 모든 시리즈는 하나의 시간 축을 공유합니다([데이터](data.md) 참고).

## 시리즈 추가

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas);

const candles = chart.addSeries('candlestick');           // main pane
const sma = chart.addSeries('line', { color: '#2962ff' }); // with options
const pane = chart.addPane(0.3);
const osc = chart.addSeries('histogram', {}, pane);        // on another pane
```

`addSeries(kind, options?, pane?)`는 `Series` 핸들을 반환합니다. `pane`을
생략하면 메인(맨 위) 패널에 시리즈가 추가됩니다. `addCandlestickSeries(options?,
pane?)`는 `addSeries('candlestick', ...)`의 축약형입니다. 알 수 없는 종류나
패널 id를 지정하면 예외가 발생합니다.

`Series`에는 읽기 전용 속성이 세 가지 있습니다.

| 속성 | 의미 |
|---|---|
| `id` | 차트 안에서 고유한 숫자. 같은 시리즈의 핸들(예: `click` 이벤트에서 받은 핸들)은 같은 id를 갖습니다. |
| `kind` | `addSeries`에 전달한 종류. 거래량 시리즈는 `'volume'`, 해당 변환은 `'kagi'` / `'pointFigure'`(Renko와 레인지 바는 `'candlestick'` 또는 `'ohlc'`), 플러그인 시리즈는 `'custom'`입니다. |
| `pane` | 시리즈가 있는 패널의 id. |

## 종류

| 종류 | 데이터 | 그리는 내용 |
|---|---|---|
| `'candlestick'` | OHLC | 캔들: 시가에서 종가까지의 몸통, 저가에서 고가까지의 꼬리 |
| `'ohlc'` | OHLC | 왼쪽에 시가 눈금, 오른쪽에 종가 눈금이 있는 바 |
| `'heikinAshi'` | OHLC | 전달한 봉으로 계산한 Heikin-Ashi 캔들 |
| `'line'` | 값 | 값을 잇는 선 |
| `'area'` | 값 | 패널 하단까지 그라디언트로 채운 선 |
| `'baseline'` | 값 | `baseValue` 위아래를 서로 다른 색으로 채운 선 |
| `'histogram'` | 값 | `baseValue`에서 각 값까지의 막대, 포인트별 색상 지정 가능 |
| `'points'` | 값 | 각 값 위치의 점 |

거래량 히스토그램은 `addVolumeSeries`로 추가하고([거래량](#volume) 참고),
Renko, 레인지 바, Kagi, Point & Figure는 `addTransformSeries`로 추가합니다
([봉 변환](#bar-transforms) 참고).

OHLC 시리즈는 `{ time, open, high, low, close, volume? }`를, 값 시리즈는
`{ time, value, color? }`를 받습니다. 두 형식과 행 배열, 검증, 실시간
업데이트는 [데이터](data.md)에서 설명합니다.

### 캔들스틱

```ts
const candles = chart.addCandlestickSeries({
  color: '#26a69a',      // up candles (close >= open)
  downColor: '#ef5350',  // down candles
});
candles.setData(randomWalk(1, 500, Date.UTC(2024, 0, 1), 60_000));
```

`color`와 `downColor`를 모두 지정하지 않으면 꼬리는 테마의 꼬리 색상을
사용합니다. 둘 중 하나라도 지정하면 꼬리는 몸통과 같은 색이 됩니다.

### OHLC 바

`'ohlc'`는 `'candlestick'`과 같은 데이터와 옵션을 받으며, 전통적인 바 차트로
그립니다.

### Heikin-Ashi

`'heikinAshi'`는 일반 OHLC 봉을 받으며, 차트가 실시간 업데이트를 포함해
Heikin-Ashi 값을 계산합니다. 색상은 캔들스틱과 같은 방식으로 동작합니다.

### 라인

```ts
const line = chart.addSeries('line', { color: '#2962ff', lineWidth: 1, lineStyle: 'dashed' });
line.setData({ time: [0, 60_000, 120_000, 180_000], value: [10, 11, NaN, 12] });
```

`NaN` 값은 공백(갭)으로, 그 지점을 지나는 선은 그려지지 않습니다.

### 영역

영역은 아래쪽에 세로 그라디언트가 있는 선입니다. `topFill`은 선 위치의 색상,
`bottomFill`은 패널 하단의 색상입니다.

```ts
chart.addSeries('area', {
  color: '#2962ff',
  topFill: 'rgba(41, 98, 255, 0.4)',
  bottomFill: 'rgba(41, 98, 255, 0)',
});
```

기본적으로 두 채우기 색 모두 `color`이며, 불투명도는 위쪽이 40 %, 아래쪽이 2 %입니다.

### 베이스라인

베이스라인 시리즈는 `baseValue`를 기준으로 나뉩니다. 그 위에서는 선이 `color`,
채우기가 `topFill`이고, 그 아래에서는 선이 `downColor`, 채우기가
`bottomFill`입니다.

```ts
chart.addSeries('baseline', {
  baseValue: 100,
  color: '#26a69a',
  downColor: '#ef5350',
});
```

기본적으로 채우기 색은 두 선 색상에 불투명도 20 %를 적용한 것입니다.

### 히스토그램

막대는 `baseValue`(기본값 0)에서 각 값까지 그려집니다. 자동 스케일 범위에는
항상 `baseValue`가 포함됩니다.

포인트마다 고유한 색을 지정할 수 있습니다. `value` 옆에 `color` 컬럼을
전달하십시오. 항목은 CSS 색상(시리즈 색상은 `null`)이거나, `0`이 시리즈 색상을
뜻하는 `0xRRGGBBAA` 값의 `Uint32Array`입니다.

```ts
const hist = chart.addSeries('histogram', { color: '#26a69a' }, pane);
hist.setData({
  time: [0, 60_000, 120_000],
  value: [1.5, -0.7, 0.4],
  color: [null, '#ef5350', null],
});

// The same colors as packed integers.
hist.setData({
  time: [0, 60_000, 120_000],
  value: [1.5, -0.7, 0.4],
  color: new Uint32Array([0, 0xef5350ff, 0]),
});
```

포인트별 색상은 최대 상세 수준에서만 그려집니다. 여러 막대가 한 픽셀을 공유해
병합될 만큼 축소하면 시리즈 색상이 사용됩니다. 다른 종류도 `color` 컬럼을 받지만
무시합니다. `update`로는 색상을 지정할 수 없으며, 추가된 포인트는 시리즈 색상을
갖습니다.

### 포인트

`'points'`는 각 값 위치에 점을 그립니다. `lineWidth`는 점의 지름(CSS px)입니다.
공백(`NaN`)에는 점이 없습니다.

```ts
const dots = chart.addSeries('points', { color: '#ff9800', lineWidth: 4 });
```

### 거래량

거래량 시리즈는 OHLC 시리즈의 `volume` 컬럼을 그립니다. 자체 데이터가 없으며,
원본 시리즈의 `setData`와 `update`가 거래량 시리즈도 업데이트합니다.

```ts
const candles = chart.addCandlestickSeries();
const volume = chart.addVolumeSeries(candles);          // main pane
// or: chart.addVolumeSeries(candles, chart.addPane(0.25));
```

원본은 OHLC 시리즈(캔들스틱, OHLC 바, Heikin-Ashi) 또는 변환 시리즈여야 합니다.
막대는 원본 봉의 상승·하락 여부에 따라 테마의 거래량 색상으로 칠해집니다. 가격도
표시하는 패널에서는 거래량이 패널 하단 4분의 1 영역에 자체 스케일로 그려지고,
전용 패널에서는 패널 전체를 채웁니다.
[가격 축과 패널](price-scale.md#volume-placement)을 참고하십시오.

원본 시리즈를 제거하면 해당 거래량 시리즈도 함께 제거됩니다.

## 시리즈 옵션

옵션은 `addSeries`에 전달하고, 나중에 `applyOptions`로 변경합니다. 생략한 키는
기존 값을 유지합니다. `options()`는 모든 키를 반환합니다.

```ts
const s = chart.addSeries('line', { color: '#2962ff' });
s.applyOptions({ lineWidth: 3 });       // color stays '#2962ff'
console.log(s.options().lineWidth);     // 3
s.applyOptions({ color: null });        // back to the theme color
```

| 옵션 | 타입 | 기본값 | 사용하는 시리즈 | 의미 |
|---|---|---|---|---|
| `color` | `Color \| null` | 테마 | 전체 | 선 색상(line, area, points); 상승 캔들과 바; 히스토그램 막대; 기준값 위의 베이스라인 선; Kagi 양선과 Point & Figure X 열 |
| `downColor` | `Color \| null` | 테마 | candlestick, ohlc, heikinAshi, baseline, 변환 | 하락 캔들과 바; 기준값 아래의 베이스라인 선; Kagi 음선과 Point & Figure O 열 |
| `topFill` | `Color \| null` | [영역](#area), [베이스라인](#baseline) 참고 | area, baseline | 선 위치의 영역 채우기; 기준값 위의 베이스라인 채우기 |
| `bottomFill` | `Color \| null` | [영역](#area), [베이스라인](#baseline) 참고 | area, baseline | 하단의 영역 채우기; 기준값 아래의 베이스라인 채우기 |
| `lineWidth` | `number` | `2` | line, area, baseline, points, Kagi, Point & Figure | 선 두께(CSS px). points에서는 점의 지름. 양수여야 합니다. |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | line, area, baseline | 선의 대시 패턴 |
| `step` | `boolean` | `false` | line, area, baseline | 계단선. [계단선](#step-lines) 참고 |
| `baseValue` | `number` | `0` | baseline, histogram | 베이스라인 채우기가 나뉘는 위치, 히스토그램 막대가 시작하는 위치. 유한한 값이어야 합니다. |
| `lastValueVisible` | `boolean` | `true` | volume 제외 전체 | 가격 축의 마지막 값 레이블 |
| `priceLineVisible` | `boolean` | `true` | volume 제외 전체 | 마지막 값 위치의 점선 수평선 |
| `shift` | `number` | `0` | 전체 | 봉 단위 표시 이동. [이동](#shift) 참고 |
| `pricePrecision` | `number \| null` | `null` | 전체 | 가격 레이블의 소수 자릿수. [가격 정밀도](#price-precision) 참고 |
| `markerStackLimit` | `number \| null` | `null` | 마커가 있는 시리즈 | 정수 1–65535: "+k" 배지를 포함한 봉의 각 쪽 자리 수. `null`이면 제한 없음. [접기](markers.md#folding-crowded-bars) 참고. |
| `markerCluster` | `number \| null` | `null` | 마커가 있는 시리즈 | 여러 봉의 군집화를 위한 CSS 픽셀 단위의 유한한 양수 거리. `null`이면 끔. `atPrice`는 제외. [군집화](markers.md#clustering-across-bars) 참고. |
| `filledBarColor` | `Color \| null` | `null` | candlestick, ohlc, heikinAshi | `filled`로 표시된 속이 빈 봉의 색상. `null`이면 주 색상을 40% 불투명도로 사용. [봉 품질 플래그](data.md#bar-quality-flags) 참고. |

잘못된 값(알 수 없는 선 스타일, 형식이 잘못된 색상, 음수 두께)은 예외를
발생시키며, 시리즈는 이전 옵션을 유지합니다.

거래량 시리즈는 항상 테마의 거래량 색상을 사용합니다.

### 색상

색상은 CSS 문자열입니다. `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
`rgb(r, g, b)`, `rgba(r, g, b, a)`(알파는 0~1) 또는 `'transparent'`를 쓸 수
있습니다. `'red'` 같은 색상 이름은 허용되지 않습니다. `null`은 테마 기본값을
뜻합니다. `options()`는 색상을 `#rrggbb`로, 반투명이면 `rgba(...)`로 반환하며,
테마 색상은 `null`로 반환합니다.

### 계단선

`step: true`를 지정하면 선이 다음 포인트의 시간까지 수평으로 이어진 다음 그 값까지
수직으로 이동합니다. 금리나 포지션 크기처럼 바뀔 때까지 유지되는 값에 사용하십시오.

```ts
const rate = chart.addSeries('line', { step: true, color: '#7e57c2' }, pane);
rate.setData({
  time: [Date.UTC(2024, 0, 1), Date.UTC(2024, 2, 20), Date.UTC(2024, 6, 31)],
  value: [5.5, 5.25, 5.0],
});
```

### 이동

`shift`는 논리 인덱스 `j`의 포인트를 `j + shift` 위치에 그립니다. 양수 값은
시리즈를 오른쪽(미래)으로, 음수 값은 왼쪽으로 옮깁니다. 정수여야 합니다. 자동
스케일, 크로스헤어, `dataAt`은 그려진 위치를 따릅니다. 일목균형표(Ichimoku)의
선행 스팬과 후행 스팬이 이 옵션을 사용합니다.

```ts
// A moving average drawn 10 bars ahead (a "displaced" moving average).
const dma = chart.addSeries('line', { shift: 10 });
```

### 가격 정밀도

`pricePrecision`은 가격 축 레이블, 마지막 값 레이블, 크로스헤어 레이블의 소수
자릿수를 0에서 12 사이로 설정합니다. `null`(기본값)은 자동으로, 보이는 가격에
대해 최소 소수 2자리와 유효숫자 4자리를 사용합니다. 아주 작은 가격은 0의 개수를
아래 첨자로 표기합니다. `0.0₄1234`는 0.00001234입니다.

패널의 레이블은 그 패널의 첫 번째 가격 시리즈의 정밀도를 사용합니다.
`'percentage'`와 `'indexedTo100'` 가격 축 모드에서는 축이 항상 소수 2자리를
사용합니다. 0에서 12 사이의 정수나 `null` 이외의 값은 예외를 발생시킵니다.

```ts
candles.applyOptions({ pricePrecision: 5 }); // FX quotes
candles.applyOptions({ pricePrecision: null }); // automatic again
```

## 봉 변환

변환 시리즈는 일반 봉(또는 틱)을 받아 특정 차트 기법으로 만든 결과를 그립니다.
Renko 벽돌, 레인지 바, Kagi 선, Point & Figure 열이 있습니다. 이러한 기법은
시간을 무시하며, 각 요소가 시간 축에서 자신만의 슬롯을 차지합니다.

```ts
const renko = chart.addTransformSeries('renko', { boxSize: 0.5 });
renko.setData(randomWalk(1, 5_000, Date.UTC(2024, 0, 1), 60_000));
chart.addVolumeSeries(renko); // optional
chart.fitContent();
```

`addTransformSeries(name, params, options?, pane?)`는 `Series`를 반환합니다.
캔들스틱 시리즈와 똑같이 `setData`와 `update`로 데이터를 공급합니다. 틱은 네
가격이 모두 같은 봉입니다.

```ts
renko.update({ time: tick.time, open: tick.price, high: tick.price, low: tick.price, close: tick.price, volume: tick.size });
```

### 매개변수

가격은 데이터와 같은 단위입니다.

| 변환 | 매개변수 | 그리는 형태 |
|---|---|---|
| `'renko'` | `boxSize`(가격) **또는** `atr`(ATR 기간, 정수); `kind?` | 캔들스틱(기본값) 또는 OHLC 바 |
| `'rangeBars'` | `range`(가격); `kind?` | 캔들스틱(기본값) 또는 OHLC 바 |
| `'kagi'` | `reversal`(가격) **또는** `reversalPercent` | Kagi 선 |
| `'pointFigure'` | `boxSize`(가격); `reversal?`(박스 수, 정수, 기본값 3) | X와 O 열 |

`kind`는 `'candlestick'`(기본값) 또는 `'ohlc'`입니다. 숫자는 유한한 양수여야
합니다. 두 대안을 모두 지정하거나(`boxSize`와 `atr`, 또는 `reversal`과
`reversalPercent`), 둘 다 지정하지 않거나, 알 수 없는 이름을 쓰면 예외가 발생합니다.

```ts
chart.addTransformSeries('renko', { atr: 14, kind: 'ohlc' });
chart.addTransformSeries('rangeBars', { range: 1 });
chart.addTransformSeries('kagi', { reversalPercent: 2 });
chart.addTransformSeries('pointFigure', { boxSize: 1, reversal: 3 });
```

(위의 각 줄은 별도의 차트가 필요합니다.
[제약 사항](#constraints)을 참고하십시오.)

### Renko

Renko는 종가만 사용합니다. 종가가 마지막 벽돌보다 `boxSize`만큼 온전히 더
움직일 때마다 벽돌이 추가됩니다. 같은 방향으로 이어갈 때는 박스 하나, 방향을
바꿀 때는 박스 두 개가 필요합니다. 첫 번째 벽돌은 첫 종가를 기준으로 합니다.

`atr: n`을 지정하면 박스 크기는 처음 `n`개 봉의 평균 실제 범위(ATR)가 되며,
이후 고정됩니다. 그 전까지는 아무것도 그려지지 않습니다.

Renko에는 형성 중인 벽돌이 없습니다. 벽돌은 완성된 뒤에만 나타납니다.

### 레인지 바

모든 바는 저가에서 고가까지 정확히 `range`만큼의 폭을 갖습니다. 입력 봉 안에서
가격은 상승 봉이면 시가 → 저가 → 고가 → 종가, 하락 봉이면 시가 → 고가 → 저가 →
종가 순으로 움직인다고 가정합니다. 가격이 현재 바의 범위를 벗어나면 그 바가
완성되고, 다음 바가 그 경계에서 시작합니다. 마지막 바는 형성 중이며 업데이트에
따라 바뀝니다.

### Kagi

Kagi 선은 종가를 그 방향으로 따라가다가, 종가가 최소 `reversal`(또는 선의 끝
가격의 `reversalPercent` 퍼센트)만큼 반전하면 방향을 바꿉니다. 선이 직전
어깨(shoulder)를 넘어 오르면 굵은 선(양선, `color`로 그림)이 되고, 직전
허리(waist) 아래로 떨어지면 가는 선(음선, `downColor`)이 됩니다. 마지막 선은
형성 중입니다.

### Point & Figure

가격은 `boxSize` 크기의 박스로 나뉩니다. 종가가 위쪽으로 새 박스를 채우는 동안
X 열이 올라가고, `reversal`개 박스만큼 반전하면 O 열이 시작되며, 그 반대도
마찬가지입니다. X 열은 `color`, O 열은 `downColor`를 사용합니다. 마지막 열은
형성 중입니다.

### 제약 사항

- 차트에는 변환 시리즈가 최대 하나만 있을 수 있으며, 거래량 외에는 다른 시리즈를
  함께 둘 수 없습니다. 두 번째 변환이나 다른 시리즈를 추가하면 예외가 발생하고,
  이미 시리즈가 있는 차트에 변환을 추가해도 마찬가지입니다. 원래 봉을 보여 주려면
  별도의 차트를 사용하고, 필요하면 [동기화](sync.md)로 두 차트를 맞추십시오.
- 각 요소의 시간은 그 요소를 완성한 입력 봉의 시간입니다(그 시간이 증가하지
  않는다면 이전 요소보다 1밀리초 뒤). `timeAtIndex`와 시간 축은 이 시간을
  표시합니다.
- 각 봉의 입력 거래량은 그 뒤에 형성되는 요소에 더해집니다. 입력 거래량이 없으면
  거래량은 비어 있습니다.
- 하나의 입력 봉이 만들 수 있는 요소는 최대 65536개입니다. 박스나 레인지가 가격
  변동에 비해 너무 작아 한 봉이 이를 넘으면, 호출이 예외를 발생시키고 시리즈가
  비워집니다.
- 공백 봉(모든 가격이 `NaN`)은 건너뜁니다.

## 두 시리즈 사이의 밴드

`addBand(upper, lower, options?)`는 같은 패널에 있는 두 단일 값 시리즈(line,
area, baseline, histogram, points) 사이의 영역을 채웁니다. `upper`가 `lower`
이상인 곳에는 `fill`이, 아래인 곳에는 `fillCrossed`가 사용됩니다. 밴드는 패널의
시리즈 뒤(거래량 위)에 그려집니다.

```ts
const upper = chart.addSeries('line', { color: '#26a69a' });
const lower = chart.addSeries('line', { color: '#ef5350' });
upper.setData({ time, value: highs });
lower.setData({ time, value: lows });

const band = chart.addBand(upper, lower, {
  fill: 'rgba(38, 166, 154, 0.15)',
  fillCrossed: 'rgba(239, 83, 80, 0.15)',
});

chart.setBandOptions(band, { fillCrossed: null }); // keys left out keep their value
console.log(chart.bandOptions(band));               // { fill: 'rgba(...)', fillCrossed: null }
chart.removeBand(band);
```

`addBand`는 밴드 id를 반환합니다. `fill`이 `null`이거나 없으면 위쪽 시리즈의
색상에 불투명도 15 %를 적용한 색이 되며, `fillCrossed`가 `null`이면 `fill`과
같습니다. `bandOptions`는 알 수 없는 id에 대해 `null`을 반환합니다. 두 시리즈 중
하나가 제거되면 밴드도 자동으로 제거됩니다. OHLC 시리즈 두 개, 서로 다른 패널의
시리즈, 또는 같은 시리즈를 두 번 지정하면 예외가 발생합니다.

밴드가 있는 지표(Bollinger, Keltner, Ichimoku)는 밴드를 자동으로 만들어 줍니다.
[지표](indicators.md)를 참고하십시오.

## 시리즈 제거

```ts
line.remove();
```

`remove()`는 시리즈와 그 데이터, 마커, 가격선, 그리고 해당 시리즈를 사용하는
밴드를 삭제합니다. OHLC 시리즈를 제거하면 그 거래량 시리즈도 제거됩니다. 그 뒤로는
핸들의 모든 메서드가 예외(`'the series was removed'`)를 발생시킵니다. 데이터가
있는 마지막 시리즈를 제거하면 시간 축이 초기화됩니다.

지표 출력은 하나씩 제거할 수 없습니다. 출력 하나에 `remove()`를 호출하면 예외가
발생하므로, 대신 지표를 제거하십시오(`indicator.remove()`).

패널 전체를 모든 시리즈와 함께 제거하려면 `chart.removePane(id)`를 사용하십시오
([가격 축과 패널](price-scale.md) 참고).
