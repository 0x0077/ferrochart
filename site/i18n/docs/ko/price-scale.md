<!-- source: 9914dfe76f -->
# 가격 축과 패널

차트는 시간 축을 공유하는 패널들을 쌓은 것입니다. 각 패널은 오른쪽에 자신의
가격 축을 가집니다. 이 장에서는 패널을 만들고 크기를 정하는 방법, 패널에 시리즈를
배치하는 방법, 가격 축 옵션을 다룹니다.

## 패널

새 차트에는 메인 패널 하나가 있습니다. `chart.mainPane`은 맨 위 패널의 id이며,
패널을 선택적으로 받는 모든 메서드의 기본값입니다.

```ts
const main = chart.mainPane;
const volumePane = chart.addPane(0.25); // below the others, a quarter of the main pane's height
const oscPane = chart.addPane(0.3);

console.log(chart.panes());             // [main, volumePane, oscPane], top to bottom
```

| 메서드 | 기능 |
|---|---|
| `addPane(stretch?)` | 다른 패널들 아래에 패널을 추가하고 그 id를 반환합니다. `stretch`는 다른 패널에 대한 상대 높이(기본값 1)이며, 0보다 큰 유한한 수여야 합니다. 그렇지 않으면 `addPane`이 예외를 던집니다. |
| `removePane(pane)` | 패널을 그 시리즈, 드로잉, 레이어 플러그인과 함께 제거합니다. 마지막 패널은 제거할 수 없습니다. |
| `panes()` | 위에서 아래 순서의 패널 id. |
| `mainPane` | 맨 위 패널의 id. |
| `setPaneStretch(pane, stretch)` | 패널의 상대 높이를 바꿉니다. `stretch`는 유한한 양수여야 합니다. |
| `paneStretch(pane)` | 패널의 상대 높이, 알 수 없는 id면 `undefined`. |

높이는 stretch 계수에 비례합니다. 계수가 1, 0.25, 0.3이면 패널들은 사용 가능한
높이의 1/1.55, 0.25/1.55, 0.3/1.55를 차지합니다. 사용자가 두 패널 사이의 구분선을
드래그하면 그 두 패널의 계수가 바뀝니다.

패널을 제거한 뒤 그 id는 재사용되지 않습니다. 맨 위 패널을 제거하면 다음 패널이
메인 패널이 됩니다. `paneStretch`를 제외하면, 알 수 없는 패널 id를 받은 메서드는
예외를 던집니다.

```ts
chart.setPaneStretch(oscPane, 0.5);
console.log(chart.paneStretch(oscPane)); // 0.5

chart.removePane(oscPane);               // its series go with it
```

패널 레이아웃(패널 수, stretch 계수, 가격 축 설정)은 저장되는 상태의 일부입니다.
[상태 저장](state.md)을 참고하세요.

## 패널에 시리즈 배치하기

시리즈를 추가하는 모든 메서드는 마지막 인수로 패널을 받으며, 생략하면 시리즈가
메인 패널에 놓입니다. `series.pane`으로 시리즈가 어디에 있는지 알 수 있습니다.

```ts
const candles = chart.addCandlestickSeries();          // main pane
const sma = chart.addSeries('line', { color: '#2962ff' });
const pane = chart.addPane(0.3);
const rsi = chart.addSeries('line', { color: '#7e57c2' }, pane);
rsi.setData(rsiValues);

console.log(rsi.pane === pane); // true
```

시리즈는 자신의 패널에 머뭅니다. 옮기려면 제거한 뒤 다른 패널에 다시 추가하세요.
지표는 배치 인수로 패널을 선택합니다. [지표](indicators.md)를 참고하세요.

한 패널의 모든 시리즈는 그 패널의 가격 축을 공유하며 함께 자동 스케일됩니다.

### 거래량 배치

`addVolumeSeries(source, pane?)`는 패널에 무엇이 함께 있는지에 따라 다르게
동작합니다.

- **가격 시리즈가 있는 패널**(기본값: 메인 패널, 캔들 옆)에서는 거래량이 별도의
  자체 스케일을 가지며 패널 아래쪽 4분의 1을 차지합니다. 가격 범위에 영향을 주지
  않으며, 가격 축은 계속 가격을 표시합니다.
- **전용 패널**에서는 거래량이 패널의 가격 축을 사용하며 패널을 채웁니다. 막대는
  아래쪽 끝에서 시작하고(아래 여백 0), 축 레이블에는 K, M, B 접미사가 붙습니다
  (`1.25M`).

```ts
// Volume under the candles, in the same pane.
chart.addVolumeSeries(candles);

// Or in a pane of its own below.
const volPane = chart.addPane(0.25);
chart.addVolumeSeries(candles, volPane);
```

## 가격 축

`applyPriceScaleOptions(options, pane?)`는 패널(기본값: 메인 패널)의 가격 축을
바꿉니다. 생략한 키는 값을 유지합니다. 전부 적용되거나 전혀 적용되지 않습니다.
값 하나라도 유효하지 않으면 예외를 던지고, 가격 축은 기존 옵션을 모두 유지합니다.
`priceScale(pane?)`는 모든 옵션을 반환합니다.

```ts
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
chart.applyPriceScaleOptions({ margins: { top: 0.2, bottom: 0.1 } }, pane);

const s = chart.priceScale();
// { mode: 'logarithmic', autoScale: true, inverted: false,
//   margins: { top: 0.1, bottom: 0.1 }, range: { low: 98.2, high: 112.7 } }
```

| 옵션 | 타입 | 기본값 | 의미 |
|---|---|---|---|
| `mode` | `'normal' \| 'logarithmic' \| 'percentage' \| 'indexedTo100'` | `'normal'` | 가격을 높이로 대응시키는 방식, [모드](#modes) 참고 |
| `autoScale` | `boolean` | `true` | 범위가 보이는 데이터를 따라가는지 여부 |
| `inverted` | `boolean` | `false` | 높은 가격을 아래쪽에 그림 |
| `margins` | `{ top, bottom }` | `{ top: 0.1, bottom: 0.1 }` | 데이터 위아래의 빈 공간, 패널 높이에 대한 비율 |
| `range` | `{ low, high }` | 데이터를 따름 | 표시되는 가격 범위(여백 제외). 설정하면 자동 스케일이 꺼집니다. |

`priceScale().range`는 자동 스케일 중에도 항상 현재 표시 중인 범위입니다.

옵션은 `mode`, `margins`, `inverted`, `range`, `autoScale` 순서로 적용됩니다.
유효하지 않은 값은 예외를 던지며, 이 순서에서 그보다 앞선 옵션은 이미 적용된
상태입니다.

### 모드

| 모드 | 축 표시 내용 |
|---|---|
| `'normal'` | 선형 스케일의 가격 |
| `'logarithmic'` | 로그 스케일의 가격. 같은 거리가 같은 비율을 나타냅니다. 가격 변화가 큰 장기 데이터에 유용합니다. |
| `'percentage'` | 첫 번째 표시 값 대비 변화율: (p − base) / \|base\| × 100, 소수점 2자리와 `%`로 표시 |
| `'indexedTo100'` | 첫 번째 표시 값을 100으로 한 지수: p / \|base\| × 100, 소수점 2자리로 표시 |

`'percentage'`와 `'indexedTo100'` 모드에서는 패널의 모든 시리즈가 각자의 첫 번째
표시 값, 즉 첫 번째로 보이는 봉의 종가나 값 시리즈의 첫 번째로 보이는 값을
기준으로 정규화됩니다. 그래서 가격 수준이 크게 다른 시리즈를 한 축에서 비교할 수
있습니다. 사용자가 이동하면 기준도 첫 번째로 보이는 봉을 따라 움직입니다. 범위,
축 레이블, 크로스헤어 가격은 퍼센트 또는 지수 포인트 단위입니다.

가격 모드(`'normal'`, `'logarithmic'`)와 상대 모드(`'percentage'`,
`'indexedTo100'`) 사이를 전환하면, 이전 범위가 다른 단위이므로 자동 스케일이
켜집니다.

### 자동 스케일과 고정 범위

기본적으로 범위는 패널에 있는 모든 시리즈의 보이는 데이터의 고가와 저가(여기에
여백을 더함)를 따르며, 이동이나 확대·축소, 새 데이터 수신에 따라 갱신됩니다.
`range`를 설정하면 범위가 고정됩니다.

```ts
chart.applyPriceScaleOptions({ range: { low: 90, high: 110 } }); // autoScale is now false
chart.applyPriceScaleOptions({ autoScale: true });               // follow the data again
```

`low`는 `high`보다 작아야 하고 둘 다 유한해야 합니다. 로그 모드에서는 `low`가
0보다 커야 합니다. 상대 모드에서 범위는 퍼센트 또는 지수 포인트 단위입니다.

`range`는 자동 스케일을 끄므로 `{ range, autoScale: true }`는 모순이며 예외를
던집니다. `{ range, autoScale: false }`는 괜찮습니다.

사용자도 범위를 바꿀 수 있습니다. 가격 축을 드래그하거나 그 위에서 휠을 돌리면
(또는 플롯에서 Alt + 휠) 가격이 확대·축소되고 자동 스케일이 꺼집니다. 가격 축을
더블클릭하면 다시 켜집니다.
[제스처와 키보드](interaction.md#gestures)를 참고하세요.

### 여백

`margins`는 데이터가 패널의 위아래 끝에 붙지 않게 합니다. 두 값 모두 0 이상
1 미만이어야 하며, 합이 1보다 작아야 합니다. 두 키 모두 필수입니다.

```ts
// Leave the top 30 % free for a legend.
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0.05 } });
```

### 반전

`inverted: true`는 축을 뒤집어 높은 가격을 아래쪽에 그립니다. 채권 수익률을
가격과 대비해 볼 때나 역방향 통화쌍에 쓰이기도 합니다.

```ts
chart.applyPriceScaleOptions({ inverted: true });
```

## 예제: 두 종목 비교

한 패널에 두 라인 시리즈를 두고 퍼센트 모드로 설정하면 뷰의 왼쪽 끝 기준 상대
성과를 보여 줍니다.

```ts
const a = chart.addSeries('line', { color: '#2962ff' });
const b = chart.addSeries('line', { color: '#ff6d00' });
a.setData({ time, value: closesA }); // around 180
b.setData({ time, value: closesB }); // around 0.95

chart.applyPriceScaleOptions({ mode: 'percentage' });
```

## 예제: 3패널 레이아웃

메인 패널에는 캔들과 이동평균, 그 아래에 거래량, 맨 아래에 오실레이터를 둡니다.

```ts
const candles = chart.addCandlestickSeries();
candles.setData(bars);

const volPane = chart.addPane(0.2);
chart.addVolumeSeries(candles, volPane);

const oscPane = chart.addPane(0.25);
const osc = chart.addSeries('histogram', { color: '#26a69a' }, oscPane);
osc.setData({ time: bars.time, value: oscillator });

// Fixed range for the oscillator, a little room at the top of the main pane.
chart.applyPriceScaleOptions({ range: { low: -1, high: 1 } }, oscPane);
chart.applyPriceScaleOptions({ margins: { top: 0.15, bottom: 0.1 } });
```
