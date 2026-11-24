<!-- source: 80fd5ef585 -->
# 시간 축과 뷰포트

시간 축은 어떤 봉이 보이고 그 봉이 얼마나 넓은지를 결정합니다. 모든 패널이 시간
축을 공유합니다. 이 장에서는 봉 간격, 오른쪽 여백, 표시 범위, 픽셀·논리
인덱스·시간 사이의 변환, 그리고 뷰 변경을 알리는 이벤트를 다룹니다.

## 논리 인덱스

시간 축은 시간이 아니라 논리 인덱스로 동작합니다. `setData` 후 첫 번째 봉의
인덱스는 0, 다음은 1과 같은 식입니다. 봉 `i`는 `i`를 중심으로 `i − 0.5`부터
`i + 0.5`까지를 차지합니다. 봉 사이나 데이터 범위 밖의 위치는 소수이거나 범위를
벗어난 인덱스를 가집니다. 시간과 상호 변환하려면 `timeAtIndex`와 `indexOfTime`을
사용하세요([데이터](data.md#reading-data-back) 참고).

인덱스는 시간이 아니라 봉을 셉니다. 봉이 없는 주말이나 거래 정지 구간은 공간을
차지하지 않습니다.

## 봉 간격

봉 간격은 이웃한 두 봉의 중심 사이 거리이며 CSS 픽셀 단위입니다. 확대·축소하면
이 값이 바뀝니다.

```ts
console.log(chart.barSpacing); // 6 on a new chart
chart.setBarSpacing(12);        // zoom in
```

| | 값 |
|---|---|
| 기본값 | 6 CSS px |
| 최솟값 | 0.5 CSS px, 모든 데이터를 한 번에 보여 주는 데 필요하면 더 작게(플롯 폭의 90 %를 봉 개수로 나눈 값) |
| 최댓값 | `maxBarSpacing`, 기본값 50 CSS px |

`setBarSpacing`은 값을 이 범위로 제한합니다. 유한한 양수가 아닌 값은 무시됩니다.
오른쪽 끝의 봉은 제자리에 머뭅니다.

최대 간격은 차트를 만들 때 정합니다.

```ts
const chart = await FerroChart.create(canvas, { maxBarSpacing: 200 });
```

`maxBarSpacing`은 1 이상의 수여야 하며, 그렇지 않으면 `create`가 reject됩니다.
봉 하나에 공간이 많이 필요할 때 올리세요. 풋프린트 숫자에는 60 px 이상, TPO
문자에는 수백 px가 필요합니다([오더플로우](orderflow.md) 참고).

## 오른쪽 여백

오른쪽 여백은 마지막 봉과 플롯 오른쪽 끝 사이의 빈 공간이며 봉 단위입니다. 소수나
음수일 수도 있습니다(음수면 마지막 봉이 오른쪽 끝 너머에 있으며, 과거 데이터를
보는 뷰는 이렇게 표현됩니다).

```ts
console.log(chart.rightOffset); // 3 on a new chart
chart.setRightOffset(10);        // more room for the forming bar
chart.scrollToRealtime();        // back to the latest bar, with the default offset of 3
```

`setRightOffset`은 화면에 최소 한 개의 봉을 남깁니다. 그 범위를 벗어난 값은
제한되고, 유한하지 않은 값은 무시됩니다.

뷰는 오른쪽 여백을 통해 최신 봉에 고정됩니다. `update`나 `setData`로 끝에 봉이
추가되어도 오른쪽 여백은 그대로이므로, 새 봉 하나마다 내용이 한 봉씩 왼쪽으로
이동합니다. 사용자가 최신 데이터를 보고 있으면 새 봉이 화면 안으로 스크롤되어
들어오고, 뒤로 스크롤해 둔 상태라면 보고 있는 봉들도 왼쪽으로 이동합니다.
`scrollToRealtime()`은 최신 봉을 다시 보여 줍니다.

## 표시 범위

`visibleLogicalRange()`는 플롯 왼쪽과 오른쪽 끝의 논리 인덱스를 소수로
반환합니다.

```ts
const { from, to } = chart.visibleLogicalRange();
const visibleBars = to - from;
```

`setVisibleLogicalRange(from, to)`는 간격 제한이 허용하는 한 정확히 그 범위를
플롯 전체에 걸쳐 보여 줍니다(허용하지 않으면 오른쪽 끝을 유지합니다).

```ts
// The last 100 bars.
const data = chart.dataRange();
if (data) chart.setVisibleLogicalRange(data.to - 99.5, data.to + 0.5);
```

`from`은 `to`보다 작아야 하고 둘 다 유한해야 합니다. 그렇지 않으면 예외를 던지고
뷰는 바뀌지 않습니다. `create` 직후 첫 프레임 전에 호출해도 됩니다. 플롯 폭을
알게 되는 즉시 범위가 적용됩니다. 그 전에 이동하거나 확대·축소하면 취소됩니다.

`fitContent()`는 오른쪽 여백 없이 모든 봉을 왼쪽 끝부터 오른쪽 끝까지 보여
줍니다. 봉이 적으면 최대 봉 간격 때문에 왼쪽에 공간이 남을 수 있습니다.
`setVisibleLogicalRange`처럼 플롯 폭을 알 때까지 기다립니다.

`dataRange()`는 데이터 기준 범위를 계산할 수 있도록 첫 번째와 마지막 봉의
인덱스를 반환합니다(데이터가 없으면 `null`).

```ts
const data = chart.dataRange();
if (data) {
  // The first 200 bars.
  chart.setVisibleLogicalRange(data.from - 0.5, data.from + 199.5);
}
```

### 시간으로 범위 지정하기

`indexOfTime`으로 시간을 인덱스로 변환합니다. 정확히 그 시간인 봉이 있어야 하므로,
임의의 시간에 가장 가까운 봉을 찾으려면 직접 보관한 시간 배열에서 검색하세요.

```ts
function showTimes(fromMs: number, toMs: number) {
  const a = chart.indexOfTime(fromMs);
  const b = chart.indexOfTime(toMs);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a - 0.5, b + 0.5);
}
```

## 픽셀, 인덱스, 시간

| 메서드 | 변환 |
|---|---|
| `logicalToCoordinate(index)` | 논리 인덱스 → 봉 중심의 x, 캔버스 왼쪽 끝으로부터의 CSS px |
| `coordinateToLogical(x)` | x(캔버스 왼쪽 끝으로부터의 CSS px) → 논리 인덱스, 소수 |
| `timeAtIndex(index)` | 논리 인덱스 → 그 봉의 시간(UTC ms), 데이터 범위 밖이면 `null`, 가장 가까운 봉으로 반올림 |
| `indexOfTime(time)` | 시간 → 정확히 그 시간인 봉의 논리 인덱스, 없으면 `null` |

좌표는 마지막 프레임의 레이아웃을 기준으로 하므로 첫 프레임 이후(예: 이벤트
핸들러 안)에 사용하세요. x 좌표는 모든 패널에서 같습니다.

```ts
// The bar under a point of the canvas.
canvas.addEventListener('dblclick', (e) => {
  const rect = canvas.getBoundingClientRect();
  const index = Math.round(chart.coordinateToLogical(e.clientX - rect.left));
  const time = chart.timeAtIndex(index);
  if (time !== null) console.log(new Date(time).toISOString(), candles.dataAt(index));
});

// Position an HTML element over a bar.
function placeOverBar(el: HTMLElement, time: number) {
  const index = chart.indexOfTime(time);
  if (index === null) return;
  el.style.left = `${chart.logicalToCoordinate(index)}px`;
}
```

크로스헤어와 클릭의 경우 이벤트가 이미 논리 인덱스, 시간, 가격을 담고 있습니다.
[크로스헤어와 이벤트](crosshair-events.md)를 참고하세요.

## visibleRangeChanged 이벤트

`visibleRangeChanged`는 표시 논리 범위가 바뀔 때마다 전송됩니다. 제스처, 위의
메서드들, 크기 조정, 뷰를 움직이는 데이터 변경이 모두 해당됩니다.

```ts
chart.on('visibleRangeChanged', ({ from, to }) => {
  const a = chart.timeAtIndex(Math.ceil(from));
  const b = chart.timeAtIndex(Math.floor(to));
  label.textContent = a !== null && b !== null
    ? `${new Date(a).toISOString()} – ${new Date(b).toISOString()}`
    : '';
});
```

이벤트는 그것을 만든 프레임 이후에 프레임당 최대 한 번 전달됩니다. 빠르게 이동하는
동안에는 중간 범위가 모두 오는 것이 아니라 최신 범위만 받습니다. 차트에 데이터가
없는 동안에는 아무것도 전송되지 않습니다. 리스너는
`chart.off('visibleRangeChanged', callback)`으로 제거하세요.

뷰가 첫 번째 봉으로부터 한 화면 이내로 들어오면 같은 프레임에서
`needMoreHistory`도 전송될 수 있습니다.
[과거 데이터 불러오기](data.md#loading-older-history)를 참고하세요.

### 다시 불러올 때 뷰 유지하기

`prepend`로 오래된 봉을 불러오면 모든 논리 인덱스와 뷰가 유지됩니다. `setData`는
봉 번호를 다시 0부터 매기므로 논리 인덱스는 바뀌지만 시간은 바뀌지 않습니다.
데이터를 교체한 뒤(예: 타임스탬프가 같은 다른 종목) 뷰를 복원하려면 시간으로
저장하세요.

```ts
function saveView() {
  const { from, to } = chart.visibleLogicalRange();
  return { from: chart.timeAtIndex(from), to: chart.timeAtIndex(to) };
}

function restoreView(view: { from: number | null; to: number | null }) {
  if (view.from === null || view.to === null) return chart.fitContent();
  const a = chart.indexOfTime(view.from);
  const b = chart.indexOfTime(view.to);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a, b);
}
```

봉 간격과 오른쪽 여백을 드로잉 및 패널 레이아웃과 함께 저장하려면 `saveState`를
사용하세요([상태 저장](state.md) 참고). 여러 차트를 같은 범위로 유지하려면
[차트 동기화](sync.md)를 참고하세요.

## 사용자 제스처

다음 입력은 시간 축을 바꿉니다(전체 표는 [제스처와 키보드](interaction.md)
참고).

| 입력 | 효과 |
|---|---|
| 플롯에서 드래그 | 이동(움직이는 중에 놓으면 관성 스크롤) |
| 휠 / 트랙패드 스크롤 | 포인터를 중심으로 시간 확대·축소 |
| 가로 스크롤 또는 Shift + 휠 | 이동 |
| 핀치(트랙패드, 두 손가락) | 확대·축소 |
| 시간 축 드래그 | 봉 간격 변경 |
| 시간 축 더블클릭 | 봉 간격 초기화 |
| ← → | 10 %씩 이동 |
| ↑ ↓ | 확대·축소 |
| Home / End | 첫 번째 / 최신 봉으로 이동 |

확대·축소할 때 포인터 아래의 봉은 제자리에 머뭅니다. 모든 변경은
`visibleRangeChanged`를 전송합니다.
