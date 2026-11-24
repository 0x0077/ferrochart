<!-- source: 610fb1bc72 -->
# 마커와 가격선

마커는 봉에 붙는 작은 도형이며 텍스트를 함께 달 수 있습니다. 매수·매도 신호나
뉴스 표시에 씁니다. 마커는 이미지(지갑이나 계정 아바타)나 `DB` 같은 짧은 배지를
표시할 수도 있고, 마커가 몰린 봉은 마커를 "+k" 배지로 접을 수 있습니다. 가격선은 주문, 손절, 목표가처럼 고정된 가격에 그리는 수평선입니다.
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
| `id` | `number` | `0` | 사용자 지정 식별자: 0부터 2^53−1까지의 정수이며 0은 없음을 뜻합니다. 클릭과 호버 이벤트가 보고합니다. |
| `position` | `'aboveBar' \| 'belowBar' \| 'atPrice'` | `'aboveBar'` | 고가 위, 저가 아래 또는 `price`를 중심으로 배치합니다. 단일 값 시리즈에서는 값의 위 또는 아래. |
| `price` | `number` | 없음 | `'atPrice'`에서는 필수이며 유한해야 합니다. 다른 위치에서는 허용되지 않습니다. 시리즈의 가격 단위입니다. |
| `shape` | `'circle' \| 'square' \| 'arrowUp' \| 'arrowDown'` | `'circle'` | 채워진 아이콘 |
| `color` | `Color \| null` | `null` | 아이콘과 텍스트 색상, `null`이면 시리즈의 주 색상(캔들과 봉은 상승 색상) 사용 |
| `text` | `string` | `''` | 아이콘 옆, 봉에서 먼 쪽에 표시되는 텍스트, 없으면 빈 문자열 |
| `size` | `number \| null` | `null` | 지름(사각형은 한 변), 4–64 CSS px, `null`이면 10 |
| `icon` | `string \| null` | `null` | `chart.registerIcon`으로 추가한 이미지의 이름, 원이나 사각형 안에 그려짐 |
| `label` | `string` | `''` | 원이나 사각형 안에 그리는 최대 8자의 텍스트(예: `'DB'`), `icon`과 함께 쓸 수 없음 |
| `border` | `Color \| null` | `null` | 도형 가장자리 안쪽의 테두리 링, `null`이면 없음 |
| `borderWidth` | `number` | `2` | 링 두께, CSS px |

알 수 없는 `position`이나 `shape`, 없거나 정수가 아닌 `time`, 유효하지 않은
색상, 4–64 범위를 벗어난 `size`, 8자를 넘는 `label`, 화살표에 지정한 `icon`이나
`label`, 또는 한 마커에 둘 다 지정한 경우 예외를 던지며, 마커는 이전 상태로
유지됩니다. 안전한 음이 아닌 정수 범위를 벗어난 `id`, `atPrice`에서 없거나
유한하지 않은 `price`, 다른 위치에 지정한 `price`도 예외를 발생시킵니다.

### 마커 읽기

`series.markers()`는 모든 필드가 채워진 마커를 시간순으로 정렬해 반환합니다(기본
색상을 쓰는 경우 `color`는 `null`). 안정 정렬이므로 시간이 같은 마커는 전달한
순서를 유지합니다.

이 목록에서 마커의 인덱스가 `click` 이벤트가 `target.index`로 보고하는 값입니다
([크로스헤어와 이벤트](crosshair-events.md) 참고). 목록이 정렬되어 있으므로
인덱스는 `setMarkers`에 전달한 배열에서의 위치와 다를 수 있습니다.

### 마커에 사용자 데이터 담기

각 마커에 직접 정한 `id`를 부여하고 같은 객체에 애플리케이션 데이터를 붙이세요.
JavaScript의 `click`과 `markerHover` 이벤트는 사용자 정의 필드를 포함한 원래
객체를 `marker`로 반환합니다.

```ts
import type { Marker } from '@ferrochart/core';

const trades: (Marker & { trade: { wallet: string; quantity: number } })[] = [
  { id: 101, time: Date.UTC(2024, 0, 15), position: 'belowBar', text: 'Buy',
    trade: { wallet: 'wallet-a', quantity: 2 } },
];
candles.setMarkers(trades);

chart.on('click', ({ target }) => {
  if (target?.type !== 'marker' || target.series.id !== candles.id) return;
  const marker = target.marker as (typeof trades)[number];
  console.log(target.id, marker.trade.wallet, marker.trade.quantity);
});
chart.on('markerHover', (e) => {
  if (!e || e.series.id !== candles.id || e.overflow > 0) return;
  const marker = e.marker as (typeof trades)[number];
  console.log(e.id, marker.trade.wallet);
});
```

위 타입 단언은 이 시리즈의 마커에 있는 사용자 정의 필드를 나타냅니다.
`index`는 시간순 정렬 후의 위치이며 애플리케이션 식별자가 아닙니다. 이벤트를
사용자 데이터와 연결할 때는 `id`나 원래 `marker`를 사용하세요.
"+k" 배지의 `id`와 `marker`는 별도의 배지 객체가 아니라 숨겨진 첫 번째 마커를
가리킵니다. `markers()`는 원래 객체가 아닌 정규화된 마커 필드를 반환합니다.
`setMarkers`로 마커를 교체할 때 사용자 정의 필드를 유지하려면 목록을 직접
보관하세요.

### 배치와 맞춤

- 마커는 시간이 `time`과 같은 봉에 그려집니다. 두 봉 사이의 시간은 **다음** 봉에
  맞춰지고, 마지막 봉 이후의 시간은 마지막 봉에 맞춰집니다. 첫 번째 봉 이전의
  시간은 첫 번째 봉에 맞춰집니다.
- 그 봉에 시리즈 값이 없으면(공백) 마커는 그려지지 않습니다.
- 마커는 시리즈의 `shift` 옵션을 따릅니다.
- 아이콘 크기는 `size` CSS px(기본값 10)입니다. 위/아래 마커는 봉에서
  4 CSS px 떨어져 있습니다.
- 같은 봉의 같은 쪽에 있는 여러 위/아래 마커는 `markers()` 순서대로 봉에서 멀어지는
  방향으로 쌓입니다. 텍스트가 있는 마커는 아이콘에 텍스트 한 줄을 더한 공간을
  차지합니다.
- 텍스트는 봉을 중심으로, `aboveBar`면 아이콘 위에, `belowBar`면 아이콘 아래에
  마커 색상으로 표시됩니다.
- `'atPrice'` 마커는 유한한 `price`를 중심으로 배치되며 텍스트는 오른쪽에
  표시됩니다. 시간은 여전히 봉에 맞춰지지만 쌓이거나 군집화되지 않습니다.
  시간과 가격이 같은 마커가 여러 개면 겹칠 수 있습니다.
- 마커는 플롯 영역 밖으로 나가는 부분이 잘립니다.

```ts
// Two signals on the same bar: the first sits next to the low, the second below it.
candles.setMarkers([
  { time: t, position: 'belowBar', shape: 'arrowUp', text: 'Entry' },
  { time: t, position: 'belowBar', shape: 'circle', text: 'Alert' },
]);
```

정확한 가격에서 발생한 이벤트를 표시하려면 다음과 같이 설정합니다.

```ts
candles.setMarkers([
  { id: 102, time: t, position: 'atPrice', price: 101.25, shape: 'circle', text: 'Fill' },
]);
```

### 이미지, 배지, 테두리

이미지를 한 번 이름으로 등록한 뒤, 그 이름을 여러 마커에서 사용합니다. 소스는
URL(가져오며, 다른 출처의 이미지에는 CORS 헤더가 필요합니다), `HTMLImageElement`,
`ImageBitmap`, `ImageData` 또는 캔버스일 수 있습니다. 차트는 64 × 64 픽셀에
맞춰 축소한 사본을 보관합니다.

```ts
await chart.registerIcon('whale', '/avatars/whale.png');
await chart.registerIcon('dev', document.querySelector('img#dev') as HTMLImageElement);

candles.setMarkers([
  { time: t1, icon: 'whale', size: 24, border: '#2fbf71' },             // a buy
  { time: t2, icon: 'whale', size: 24, border: '#ef5350' },             // a sell
  { time: t2, position: 'belowBar', label: 'DB', color: '#2962ff', size: 20 },
  { time: t3, position: 'belowBar', label: '狙', color: '#ff9800', size: 20 },
]);
```

- 이미지는 마커 색상 위에 원(둥글게 잘림)이나 사각형을 채우므로, 투명한 이미지는
  뒤의 색상이 비쳐 보입니다. 화살표에는 이미지나 레이블을 넣을 수 없습니다.
- `registerIcon`은 프로미스를 반환합니다. 이름을 사용하기 전에 완료를 기다리세요.
  등록되지 않은 이름으로 `setMarkers`를 호출하면 예외가 발생합니다. 이미지를
  불러오거나 읽을 수 없으면(가져오기 실패, CORS 헤더 없는 다른 출처의 이미지)
  프로미스가 거부되므로, 그 마커는 `label`이나 일반 도형으로 대체하세요.
- 같은 이름을 다시 등록하면 그 이름을 쓰는 모든 마커의 이미지가 교체됩니다.
  `chart.unregisterIcon(name)`은 이미지를 제거하며(이미지가 있었는지를 반환),
  그 이미지를 쓰던 마커는 도형만 그리고 `markers()`는 해당 마커의 `icon`을
  `null`로 보고합니다. 차트 하나는 최대 1024개의 이미지를 보관합니다.
- `label`은 마커 색상과 대비되는 색으로 가운데에 크기를 맞춰 그립니다. 라틴
  문자는 지름의 약 0.45, 한중일 문자는 약 0.6 크기이며, 텍스트가 더 넓으면 더
  작아집니다.
- `border` 링은 가장자리 안쪽에 그려지므로 마커 크기가 바뀌지 않습니다. 예를 들어
  매수는 초록색 링, 매도는 빨간색 링으로 표시할 수 있습니다.

### 몰린 봉 접기

한 봉에 마커가 많이 몰릴 때(예: 같은 분에 트레이더 열두 명이 매수) 시리즈에
`markerStackLimit`을 설정하세요. 봉의 각 쪽에서 최대 그 개수만큼의 자리만
사용하며, 마지막 자리에는 그려지지 않은 k개의 마커를 나타내는 "+k"가 표시됩니다.

```ts
candles.applyOptions({ markerStackLimit: 4 });   // 3 markers, then "+k"
candles.applyOptions({ markerStackLimit: null }); // no limit (the default)
```

배지는 시리즈의 주 색상으로 그린 원이며, 대신하는 마커와 크기가 같습니다. 배지를
클릭하면 target이 `{ type: 'markerOverflow', series, first, count, id, marker, bars }`인
`click`이 전송됩니다. `first`는 숨겨진 첫 번째 마커의 정렬된 인덱스이며, `count`는
해당 쪽에서 숨겨진 개수이고, `id`/`marker`는 그 첫 번째 마커를 식별합니다.
`bars`는 첫 번째와 마지막 논리 봉 인덱스를 담으며, 한 봉에서 접힌 경우 둘이
같습니다. 정렬된 목록에는 다른 쪽의 마커가 사이에 끼어 있을 수 있으므로,
`count`가 목록의 연속된 구간을 나타내지는 않습니다.

### 여러 봉의 마커 군집화

가까운 여러 봉에 걸쳐 마커를 묶으려면 `markerCluster`에 CSS 픽셀 단위의 유한한
양수를 설정하세요. `null`(기본값)은 군집화를 끕니다.

```ts
candles.applyOptions({ markerCluster: 28, markerStackLimit: 4 });
// Later, keep per-bar folding but turn off cross-bar clustering:
candles.applyOptions({ markerCluster: null });
```

버킷에는 `ceil(markerCluster / bar spacing)`개의 논리 인덱스가 들어갑니다
(최소 하나). 논리 인덱스에 맞춰 정렬되므로 이동해도 마커가 다시 묶이지 않으며,
확대·축소하면 버킷 크기가 바뀔 수 있습니다. 버킷의 같은 쪽에서 두 개 이상의
봉에 마커가 있으면 첫 번째 마커와 "+k" 배지만 표시하며, 그쪽에 마커가 가장
많은 봉에 그립니다. `atPrice` 마커는 참여하지 않습니다.

이런 그룹에서는 군집화가 `markerStackLimit`보다 우선합니다. 그 밖의 경우에는
`markerStackLimit`이 한 봉의 마커 접기를 계속 제어합니다. 배지를 클릭하면
`markerOverflow` target이 전달되고, 호버하면 `overflow > 0`입니다.
`bars: { from, to }`는 배지가 나타내는 첫 번째와 마지막 논리 인덱스를 양 끝을
포함해 담습니다. 상세 목록을 만들 때는 이 범위, `marker`의 위/아래 위치, 직접
보관한 마커 데이터를 사용하세요. `first`/`index`와 개수만으로는 구성원의 연속된
목록을 얻을 수 없습니다. [논리 인덱스](data.md#logical-indices)와
[클릭 이벤트](crosshair-events.md#click)를 참고하세요.

### 마커 호버

`markerHover` 이벤트는 포인터가 마커나 "+k" 배지 위로 올라가면 발생하고, 벗어나면
`null`과 함께 발생합니다. 마커의 인덱스와 CSS px 단위의 중심과 지름을 담고 있어
툴팁에 필요한 정보를 모두 제공합니다.

```ts
chart.on('markerHover', (e) => {
  if (!e) return hideTooltip();
  if (e.overflow > 0) {
    showTooltip(e.x, e.y - e.size / 2, `${e.overflow} more · bars ${e.bars.from}–${e.bars.to}`);
  } else {
    showTooltip(e.x, e.y - e.size / 2, e.marker.text ?? '');
  }
});
```

`index`는 `series.markers()`에서의 위치입니다. 배지의 경우 그려지지 않은 첫 번째
마커이며 `overflow`는 k입니다(마커는 0). `id`와 `marker`는 그 마커를 식별하며,
`bars`는 마커나 배지가 나타내는 양 끝을 포함한 논리 범위입니다.
호버된 마커는 프레임을 그릴 때 찾으므로
이벤트는 프레임당 최대 한 번 전송됩니다. 인트로 애니메이션이 재생되는 동안에는
어떤 마커도 호버된 것으로 보지 않습니다.
[크로스헤어와 이벤트](crosshair-events.md)를 참고하세요.

### 중국어, 일본어, 한국어와 이모지

마커 텍스트와 레이블에는 어떤 문자든 쓸 수 있습니다. 내장 폰트는 라틴 문자, 숫자,
일반 기호만 지원하며, 그 밖의 문자는 다음에서 가져옵니다.

- **웹**: 브라우저 자체 폰트(시스템의 CJK 폰트와 컬러 이모지)를 사용하며, 설정할
  것도 없고 패키지에 추가되는 것도 없습니다.
- **데스크톱, Python, C**: 직접 추가한 폰트를 순서대로 검색합니다. winit에서는
  `WindowOptions { fonts, .. }`, Python에서는 `fc.Chart(fonts=[...])`, C에서는
  `ferrochart_add_font`를 사용합니다. 시스템 폰트는 검색하지 않습니다. 보통
  설치되어 있는 폰트: `/System/Library/Fonts/Hiragino Sans GB.ttc`(macOS),
  `C:/Windows/Fonts/msyh.ttc`(Windows),
  `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`(Linux, Noto CJK
  패키지 설치 시). 또는 애플리케이션에 Noto Sans SC/JP/KR을 포함해 배포하세요.

어떤 폰트에도 없는 문자는 `□`로 그려집니다. 네이티브에서는 컬러 이모지 폰트를
지원하지 않으며, 윤곽선 이모지 폰트(Noto Emoji)는 텍스트 색상으로 그려집니다.
텍스트는 문자소 클러스터 단위로 배치됩니다. 웹에서는 피부색이 적용된 이모지,
가족 등의 ZWJ 시퀀스, 국기, 결합 악센트가 붙은 글자를 브라우저가 하나의 글리프로
측정하고 그립니다. 네이티브에서는 이러한 시퀀스를 문자 단위로 그립니다(폰트
셰이핑을 하지 않음). 셰이핑과 오른쪽에서 왼쪽으로 쓰는 배치는 지원하지 않으므로
아랍어와 히브리어는 올바르게 표시되지 않습니다.

### Python, C, Rust

바인딩마다 고유한 이름 규칙을 사용합니다.

```python
red = chart.add_icon(avatar)          # a (height, width, 4) or (height, width, 3) uint8 array
candles.set_markers([
    {"time": t, "icon": red, "size": 24, "border": "#2fbf71"},
    {"time": t, "position": "below_bar", "label": "DB", "color": "#2962ff"},
])
candles.apply_options(marker_stack_limit=4, marker_cluster=28)
chart.on("marker_hover", lambda e: print(e))   # None when the pointer leaves
```

Python의 마커 딕셔너리는 `id`와 `price`를 동반한 `position="at_price"`를
받으며, 이벤트는 `id`와 배지의 `bars`를 보고합니다. 원래 객체를 `marker`로
돌려받는 것은 JavaScript 기능입니다. 네이티브 Python에서는 id를 사용자 객체와
연결하는 매핑을 직접 보관하세요. Rust에서는 `Marker::id`, `Marker::price`,
`MarkerPosition::AtPrice`를 쓰며, `SeriesOptions::marker_cluster`로 군집화를
제어합니다. C에서는 `FerroChartMarker.id`, `.price`,
`FERROCHART_MARKER_AT_PRICE`를 쓰며, 시리즈 옵션은 `marker_cluster`입니다.

Python에서 `add_icon`은 정수 id를 반환하고 `remove_icon(id)`는 이미지를
제거합니다. C에서 `ferrochart_add_icon`과 `ferrochart_set_markers`는 같은 필드를
받습니다([C, C++, C#](native.md) 참고). Rust에서 `Chart::add_icon`은
`IconImage`(straight RGBA, 최대 256 × 256)를 받아 `Marker::icon`에 쓸 `IconId`를
반환하며, `ChartEvent::MarkerHover`와 `ClickTarget::MarkerOverflow`가 각각
이벤트와 클릭 target입니다.

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
