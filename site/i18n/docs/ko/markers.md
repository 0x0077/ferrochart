<!-- source: b0ef03b731 -->
# 마커와 가격선

마커는 봉에 붙는 작은 아이콘이며 텍스트를 함께 달 수 있습니다. 매수·매도 신호나
뉴스 표시에 씁니다. 가격선은 주문, 손절, 목표가처럼 고정된 가격에 그리는 수평선입니다.
둘 다 시리즈에 속합니다. 또한 각 시리즈에는 최근 값 레이블과 최근 값 선이 있으며,
시리즈 옵션으로 켜고 끕니다.

## 마커

### 마커 설정

```ts
candles.setMarkers([
  { time: Date.UTC(2024, 0, 15), position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: Date.UTC(2024, 1, 2), position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
  { time: Date.UTC(2024, 1, 20), shape: 'circle', text: 'Earnings' },
]);
```

`setMarkers`는 시리즈의 모든 마커를 교체합니다. 하나를 추가하려면 현재 목록을
읽어 덧붙인 뒤 다시 설정하고, 모두 지우려면 빈 배열을 전달하세요.

```ts
candles.setMarkers([...candles.markers(), { time: Date.UTC(2024, 2, 1), text: 'Split' }]);
candles.setMarkers([]);
```

### 마커 필드

| 필드 | 타입 | 기본값 | 의미 |
|---|---|---|---|
| `time` | `number \| bigint \| Date` | 필수 | UTC 밀리초(정수) 또는 `Date`, `markers()`는 숫자를 반환 |
| `position` | `'aboveBar' \| 'belowBar'` | `'aboveBar'` | 봉의 고가 위 또는 저가 아래. 단일 값 시리즈에서는 값의 위 또는 아래. |
| `shape` | `'circle' \| 'square' \| 'arrowUp' \| 'arrowDown'` | `'circle'` | 채워진 아이콘 |
| `color` | `Color \| null` | `null` | 아이콘과 텍스트 색상, `null`이면 시리즈의 주 색상(캔들과 봉은 상승 색상) 사용 |
| `text` | `string` | `''` | 아이콘 옆, 봉에서 먼 쪽에 표시되는 텍스트, 없으면 빈 문자열 |

알 수 없는 `position`이나 `shape`, 없거나 정수가 아닌 `time`, 유효하지 않은
색상은 예외를 던지며, 마커는 이전 상태로 유지됩니다.

### 마커 읽기

`series.markers()`는 모든 필드가 채워진 마커를 시간순으로 정렬해 반환합니다(기본
색상을 쓰는 경우 `color`는 `null`). 안정 정렬이므로 시간이 같은 마커는 전달한
순서를 유지합니다.

이 목록에서 마커의 인덱스가 `click` 이벤트가 `target.index`로 보고하는 값입니다
([크로스헤어와 이벤트](crosshair-events.md) 참고). 목록이 정렬되어 있으므로
인덱스는 `setMarkers`에 전달한 배열에서의 위치와 다를 수 있습니다.

### 배치와 맞춤

- 마커는 시간이 `time`과 같은 봉에 그려집니다. 두 봉 사이의 시간은 **다음** 봉에
  맞춰지고, 마지막 봉 이후의 시간은 마지막 봉에 맞춰집니다. 첫 번째 봉 이전의
  시간은 첫 번째 봉에 맞춰집니다.
- 그 봉에 시리즈 값이 없으면(공백) 마커는 그려지지 않습니다.
- 마커는 시리즈의 `shift` 옵션을 따릅니다.
- 아이콘 크기는 10 CSS px이며 봉에서 4 CSS px 떨어져 있습니다.
- 같은 봉의 같은 쪽에 있는 여러 마커는 `markers()` 순서대로 봉에서 멀어지는
  방향으로 쌓입니다. 텍스트가 있는 마커는 아이콘에 텍스트 한 줄을 더한 공간을
  차지합니다.
- 텍스트는 봉을 중심으로, `aboveBar`면 아이콘 위에, `belowBar`면 아이콘 아래에
  마커 색상으로 표시됩니다.
- 마커는 플롯 영역 밖으로 나가는 부분이 잘립니다.

```ts
// Two signals on the same bar: the first sits next to the low, the second below it.
candles.setMarkers([
  { time: t, position: 'belowBar', shape: 'arrowUp', text: 'Entry' },
  { time: t, position: 'belowBar', shape: 'circle', text: 'Alert' },
]);
```

### 마커를 가질 수 있는 시리즈

거래량 시리즈를 제외한 모든 시리즈 종류입니다. 거래량 시리즈에서 `setMarkers`를
호출하면 예외가 발생합니다. 지표 출력에도 마커를 붙일 수 있습니다(`Indicator`의
`outputs()`는 일반 시리즈입니다).

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];
rsi.setMarkers([{ time: t, position: 'aboveBar', shape: 'square', text: 'OB' }]);
```

### 예제: 신호로 마커 만들기

```ts
import type { Marker, OhlcData } from '@ferrochart/core';

// Mark crossings of the close over a 20-bar SMA.
function crossMarkers(bars: OhlcData, sma: number[]): Marker[] {
  const out: Marker[] = [];
  const close = bars.close;
  for (let i = 1; i < close.length; i++) {
    const was = close[i - 1] - sma[i - 1];
    const now = close[i] - sma[i];
    if (Number.isNaN(was) || Number.isNaN(now)) continue;
    if (was <= 0 && now > 0) {
      out.push({ time: bars.time[i], position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' });
    } else if (was >= 0 && now < 0) {
      out.push({ time: bars.time[i], position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' });
    }
  }
  return out;
}

candles.setMarkers(crossMarkers(bars, smaValues));
```

## 가격선

### 가격선 만들기

```ts
const entry = candles.createPriceLine({
  price: 101.25,
  color: '#2962ff',
  lineWidth: 1,
  lineStyle: 'solid',
  title: 'Entry',
});
const stop = candles.createPriceLine({ price: 98.5, color: '#ef5350', title: 'Stop' });
```

`createPriceLine`은 `PriceLine`을 반환합니다. 가격은 시리즈의 단위이므로, 퍼센트
또는 지수 가격 축에서는 선이 시리즈와 함께 움직입니다
([가격 축](price-scale.md) 참고).

### 옵션

| 옵션 | 타입 | 기본값 | 의미 |
|---|---|---|---|
| `price` | `number` | `0` | 시리즈 단위의 가격, 유한해야 함 |
| `color` | `Color \| null` | `null` | 선, 제목, 축 레이블 색상, `null`이면 시리즈의 주 색상 사용 |
| `lineWidth` | `number` | `1` | CSS px, 0보다 커야 함 |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'dashed'` | 대시 패턴 |
| `axisLabelVisible` | `boolean` | `true` | 가격 축에 가격 레이블 표시 |
| `title` | `string` | `''` | 플롯 오른쪽 끝에서 선 위에 표시되는 텍스트, 없으면 빈 문자열 |

축 레이블은 시리즈의 가격 형식(`pricePrecision`)을 사용합니다. 가격 축의
레이블이 겹치면 서로 떨어지도록 옮겨집니다. 보이는 가격 범위 밖에 있는 가격선은
그려지지 않습니다.

### 변경과 제거

```ts
entry.applyOptions({ price: 101.5 });         // only the keys you pass change
console.log(entry.options());                   // every option: { price, color, lineWidth, ... }
stop.applyOptions({ title: 'Stop (moved)', lineStyle: 'dotted' });

stop.remove();
```

`remove()` 후 그 객체에서 `options`, `applyOptions`, `remove`를 호출하면 예외가
발생합니다. 시리즈를 제거하면 그 가격선도 제거됩니다.

`createPriceLine`은 거래량 시리즈, 유한하지 않은 가격, 객체가 아닌 옵션 인수,
유효하지 않은 옵션 값에 대해 예외를 던집니다.

### 예제: 드래그할 수 있는 주문선

가격선은 스스로 입력을 처리하지 않습니다. 차트의 이벤트와 좌표 변환을 조합하거나,
사용자가 드래그할 수 있는 `horizontal_line` 드로잉을 사용하세요
([드로잉 도구](drawings.md) 참고).

```ts
const id = chart.drawings.add({
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 1), price: 101.25 }],
  style: { line: '#2962ff' },
});

chart.on('drawingModified', (e: ChartEventMap['drawingModified']) => {
  if (e.id !== id) return;
  const price = chart.drawings.get(id)?.anchors[0].price;
  if (price !== undefined) console.log('order moved to', price);
});
```

## 최근 값 레이블과 선

각 시리즈는 최신 값을 가격 축의 레이블과 플롯을 가로지르는 점선 수평선으로
보여 줍니다. 마지막 봉의 종가가 시가 이상이면(베이스라인 시리즈는 기준값 이상이면)
상승 색상을, 그렇지 않으면 하락 색상을 사용합니다. 라인, 영역, 히스토그램, 포인트
시리즈는 주 색상을 사용합니다.

| 시리즈 옵션 | 기본값 | 의미 |
|---|---|---|
| `lastValueVisible` | `true` | 가격 축에 최근 값 레이블 표시 |
| `priceLineVisible` | `true` | 최근 값에 점선 표시 |
| `pricePrecision` | `null` | 레이블의 소수 자릿수, `null`이면 자동 |

```ts
const line = chart.addSeries('line', { lastValueVisible: true, priceLineVisible: false });
candles.applyOptions({ priceLineVisible: false });
```

거래량 시리즈는 둘 다 그리지 않습니다. 지표 출력은 `priceLineVisible: false`로
만들어지며 최근 값 레이블은 표시됩니다. 출력마다 끌 수 있습니다.

```ts
for (const s of chart.addIndicator(candles, 'boll').outputs()) {
  s.applyOptions({ lastValueVisible: false });
}
```

시리즈 옵션은 [시리즈](series.md)에 설명되어 있습니다.
