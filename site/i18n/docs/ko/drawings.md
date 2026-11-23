<!-- source: 58a84755b2 -->
# 드로잉 도구

드로잉은 사용자나 코드가 차트 위에 배치하는 객체입니다. 추세선, 사각형, 피보나치
도구, 포지션, 패턴, 텍스트 등이 있습니다. 각 드로잉은 데이터 공간(시간과 가격)의
앵커를 가지므로, 차트를 이동하거나 확대·축소하거나 크기를 바꿔도 봉에 붙어
있습니다. 생성, 선택, 드래그, 삭제, 실행 취소/다시 실행은 FerroChart가 직접
처리합니다. 페이지는 도구를 활성화하고, 필요하면 드로잉 이벤트에 반응하기만 하면
됩니다.

## Drawings 객체

모든 드로잉 메서드는 `chart.drawings`에 있습니다.

```ts
const drawings = chart.drawings;

drawings.startTool('trend_line');   // the next clicks draw a trend line
const id = drawings.add({           // or add one from code
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 2), price: 101.5 }],
});
drawings.setStyle(id, { line: '#ef5350', lineWidth: 2 });
drawings.undo();
```

드로잉은 숫자 id로 식별합니다. 한 차트 안에서 id는 재사용되지 않으며, 실행
취소로 드로잉이 되살아나면 이전 id가 복원됩니다.

| 메서드 / 속성 | 기능 |
|---|---|
| `kinds()` | `add`와 `startTool`이 받는 종류(내장 29종과 등록된 종류) |
| `startTool(kind)` / `cancelTool()` / `activeTool` | 대화형 생성 |
| `add(data, pane?)` | 드로잉을 추가하고 id를 반환 |
| `get(id)` | 드로잉의 `DrawingData`(종류, 앵커, 스타일, 매개변수). 알 수 없는 id면 `null` |
| `ids()` | 아래에서 위 순서의 모든 id |
| `paneOf(id)` | 드로잉이 있는 패널의 id |
| `setAnchors(id, anchors)` | 앵커 이동 |
| `setStyle(id, style)` | 일부 스타일 키 변경 |
| `setParam(id, key, value)` | 도구 매개변수(`text`, `levels`) 설정 |
| `select(id)` / `selected` | 선택 |
| `setVisible(id, visible)` / `isVisible(id)` | 표시 또는 숨기기 |
| `setLocked(id, locked)` / `isLocked(id)` | 드래그 잠금 |
| `bringToFront(id)` / `sendToBack(id)` | Z 순서 |
| `remove(id)` | 드로잉 제거 |
| `undo()` / `redo()` / `canUndo` / `canRedo` / `clearHistory()` | 히스토리 |
| `register(kind, tool)` | JavaScript로 작성한 드로잉 도구 추가 |

id를 받는 메서드는 알 수 없는 id에 대해 예외를 발생시킵니다(`null` 또는
`undefined`를 반환하는 `get`, `paneOf`, `isVisible`, `isLocked`는 제외). 드로잉은
차트 상태와 함께 저장되고 복원됩니다([상태 저장](state.md) 참고).

## 내장 종류

종류 이름은 snake_case 문자열입니다.

**선**

| 종류 | 앵커 | 설명 |
|---|---|---|
| `trend_line` | 2 | 두 앵커 사이의 선분 |
| `ray` | 2 | 첫 번째 앵커에서 두 번째 앵커를 지나 플롯 가장자리까지 |
| `extended_line` | 2 | 두 앵커를 지나 양쪽 플롯 가장자리까지 |
| `horizontal_line` | 1 | 앵커 가격에서 플롯을 가로지르는 선, 오른쪽 가장자리에 가격 레이블 |
| `horizontal_ray` | 1 | 앵커에서 오른쪽 가장자리까지, 가격 레이블 포함 |
| `vertical_line` | 1 | 앵커 시간에서 플롯을 세로로 가로지르는 선 |
| `arrow` | 2 | 두 번째 앵커에 화살촉이 있는 선분 |

**도형과 텍스트**

| 종류 | 앵커 | 설명 |
|---|---|---|
| `parallel_channel` | 3 | 처음 두 앵커를 지나는 선과 세 번째 앵커를 지나는 평행선. 내부를 채우며 가운데에 파선이 있음 |
| `rectangle` | 2 | 마주 보는 두 꼭짓점. 내부를 채움 |
| `ellipse` | 2 | 두 앵커가 이루는 사각형에 내접. 내부를 채움 |
| `text` | 1 | 앵커에서 시작하는 텍스트. 매개변수 `text`(기본값 `'Text'`). 채우기가 투명하지 않으면 `fill` 색 배경 위에 그림 |
| `callout` | 2 | 두 번째 앵커의 텍스트 상자와 첫 번째 앵커로 이어지는 지시선. 매개변수 `text`(기본값 `'Callout'`). 상자는 선 색으로 채워지고 텍스트는 흰색 |
| `path` | 2개 이상 | 폴리라인. 점마다 한 번씩 클릭하고, 더블 클릭이나 Enter로 마침 |
| `brush` | 2개 이상 | 자유 곡선: 누르고, 드래그하고, 놓기. 핸들이 없으며 전체로만 이동 |

**측정과 포지션**

| 종류 | 앵커 | 설명 |
|---|---|---|
| `price_range` | 2 | 첫 번째 앵커에서 두 번째 앵커까지의 가격 변화와 변화율을 표시하는 음영 범위 |
| `date_range` | 2 | 봉 개수와 기간을 표시하는 음영 범위 |
| `long_position` | 3(클릭 2번) | 진입(첫 번째 앵커), 목표(두 번째), 손절(세 번째). 생성 시 손절은 진입가를 기준으로 목표와 대칭으로 놓임. 수익·손실 영역, 가격과 퍼센트가 있는 레이블, 손익비 |
| `short_position` | 3(클릭 2번) | `long_position`과 같으며, 퍼센트는 매도 포지션 기준 |

**피보나치와 갠**

| 종류 | 앵커 | 설명 |
|---|---|---|
| `fib_retracement` | 2 | 각 레벨마다 `p2 + (p1 − p2) · level` 위치의 레벨선, `level (price)` 레이블, 번갈아 칠한 음영 밴드. 매개변수 `levels`(기본값 `[0, 0.236, 0.382, 0.5, 0.618, 0.786, 1]`) |
| `fib_extension` | 3 | 세 번째 앵커부터 그리는 `p3 + (p2 − p1) · level` 위치의 레벨. 매개변수 `levels`(기본값 `[0, 0.382, 0.618, 1, 1.272, 1.618, 2, 2.618]`) |
| `fib_time_zones` | 2 | 피보나치 수 F = 0, 1, 2, 3, 5, 8, …에 대해 봉 `i1 + (i2 − i1) · F` 위치의 수직선 |
| `gann_fan` | 2 | 첫 번째 앵커에서 뻗는 광선. 기울기는 두 번째 앵커로 가는 선의 1/8 … 8배이며 `1x8` … `8x1`로 레이블 |
| `pitchfork` | 3 | 앤드루스 피치포크: 첫 번째 앵커에서 나머지 두 앵커의 중점을 지나는 중앙선과, 두 앵커를 지나는 평행한 갈래선. 갈래선 사이를 채움 |
| `schiff_pitchfork` | 3 | `pitchfork`와 같으나, 원점을 가격 기준으로 두 번째 앵커 쪽으로 절반만큼 옮김 |
| `modified_schiff_pitchfork` | 3 | `pitchfork`와 같으나, 원점이 처음 두 앵커의 중점 |

**패턴과 파동**

| 종류 | 앵커 | 설명 |
|---|---|---|
| `xabcd_pattern` | 5 | 점 X, A, B, C, D와 각 구간 사이의 되돌림 비율. XAB와 BCD 삼각형을 채움 |
| `head_and_shoulders` | 7 | 왼쪽 어깨, 머리, 오른쪽 어깨 레이블과 세 번째·다섯 번째 앵커를 지나는 넥라인. 내부를 채움 |
| `elliott_impulse` | 6 | 0~5로 레이블된 파동 |
| `elliott_correction` | 4 | 0, A, B, C로 레이블된 파동 |

`p1`, `p2`, `p3`은 순서대로 각 앵커의 가격입니다. 패턴과 파동의 레이블은 고점에서는
위에, 저점에서는 아래에 표시됩니다.

## 대화형으로 드로잉 만들기

```ts
chart.drawings.startTool('rectangle');
```

`startTool(kind)`는 도구를 활성화합니다. 플롯 위의 커서가 십자 모양으로 바뀌고,
어느 패널에서든 다음에 누르는 위치에서 드로잉이 시작됩니다. 알 수 없는 종류를
지정하면 예외가 발생합니다. 도구를 활성화하면 진행 중인 드로잉은 취소됩니다.

드로잉을 완성하는 방식은 종류에 따라 다릅니다.

- **앵커 개수가 고정된 종류**(대부분): 첫 번째로 누르면 첫 앵커가 놓입니다. 다음
  앵커는 다시 클릭할 때까지 포인터를 따라다니며, 마지막 앵커가 놓일 때까지 이를
  반복합니다. 앵커가 하나인 종류(`horizontal_line`, `horizontal_ray`,
  `vertical_line`, `text`)는 첫 번째 누름으로 완성됩니다. 처음 두 앵커는 누르고,
  드래그하고, 놓는 방식으로도 배치할 수 있습니다.
- **가변 개수**(`path`): 앵커마다 한 번씩 클릭합니다. 더블 클릭이나 Enter로
  마칩니다(더블 클릭으로 중복된 점은 버립니다). 마치려면 앵커가 최소 두 개
  필요합니다.
- **자유 그리기**(`brush`): 누르고, 드래그하고, 놓습니다. 3 CSS px 움직일 때마다
  앵커가 하나 추가됩니다. 앵커가 두 개 미만인 획은 버립니다.

앵커를 배치하는 동안 x는 가장 가까운 봉에 맞춰지고(마지막 봉 이후의 시간은
외삽합니다), 크로스헤어 자석 모드에서는 y가 가장 가까운 시가, 고가, 저가, 종가에
맞춰집니다. 자유 그리기 앵커는 맞춰지지 않습니다.

드로잉이 완성되면 선택된 상태가 되고, `drawingCreated` 이벤트가 전송되며,
**도구가 비활성화됩니다**. 즉 `startTool` 한 번에 드로잉 하나가 만들어집니다. 같은
종류를 계속 그리려면 `drawingCreated` 리스너에서 다시 활성화하십시오(아래의 툴바
예제 참고).

| 키 | 생성 중 동작 |
|---|---|
| Escape | 진행 중인 드로잉을 취소하거나(앵커가 두 개 이상인 `path`는 대신 완성), 활성화된 도구를 비활성화 |
| Enter | `path` 완성 |

`cancelTool()`은 이벤트 없이 도구를 비활성화하고 진행 중인 드로잉을 버립니다.
`activeTool`은 활성화된 종류 또는 생성 중인 드로잉의 종류이며, 그 밖에는
`undefined`입니다.

드로잉을 생성하는 동안에는 `click` 이벤트가 전송되지 않습니다.

## 선택과 편집

도구가 활성화되어 있지 않을 때:

- **드로잉을 누르면** 선택됩니다(`drawingSelected`). 핸들이 나타납니다. 대부분의
  종류는 앵커마다 하나씩이고, 포지션은 진입, 목표, 손절, 오른쪽 끝에 핸들이 있으며,
  수평선과 수직선은 플롯 가운데에 하나, 브러시는 핸들이 없습니다.
- 드로잉을 **드래그하면** 이동합니다. 가로로는 봉 단위로, 세로로는 포인터를 따라
  움직입니다. 핸들을 드래그하면 해당 앵커가 이동합니다. 드래그가 끝나면
  `drawingModified`가 전송됩니다. 드래그 중에 Escape를 누르면 드로잉이 원래
  자리로 돌아갑니다.
- **빈 곳을 누르면** 선택이 해제됩니다.
- 선택된 드로잉의 핸들이 가장 먼저 히트되고, 그다음 선택된 드로잉, 그다음 나머지
  드로잉이 위에서부터 히트됩니다.
- 숨겨진 드로잉은 히트되지 않습니다. 사각형, 타원, 채널은 외곽선에서 히트되며,
  채우기가 투명하지 않으면 내부 어디서나 히트됩니다.

드로잉을 누르는 것은 이동(팬)이 아닙니다. 잠긴 드로잉을 누르면 선택된 다음 차트가
이동합니다.

### 키보드

캔버스는 포커스가 있을 때 키보드 입력을 받으며, 누르면 포커스를 받습니다.

| 키 | 효과 |
|---|---|
| Delete | 선택된 드로잉 제거(잠긴 드로잉 포함) |
| Escape | 생성 취소, 도구 비활성화 또는 현재 드래그 되돌리기 |
| Enter | 생성 중인 `path` 완성 |
| Ctrl+Z / ⌘+Z | 실행 취소 |
| Ctrl+Shift+Z / ⌘+Shift+Z, Ctrl+Y / ⌘+Y | 다시 실행 |

Backspace는 아무 동작도 하지 않습니다. 다른 키로 삭제하려면 캔버스에서 직접 이벤트를
받으십시오.

```ts
canvas.addEventListener('keydown', (e) => {
  const id = chart.drawings.selected;
  if (e.key === 'Backspace' && id != null) chart.drawings.remove(id);
});
```

## 코드로 드로잉 추가

```ts
const id = chart.drawings.add({
  kind: 'fib_retracement',
  anchors: [
    { time: Date.UTC(2024, 0, 10), price: 120 },
    { time: Date.UTC(2024, 1, 20), price: 95 },
  ],
  style: { line: '#787b86', fill: 'rgba(120, 123, 134, 0.15)' },
  params: { levels: [0, 0.382, 0.5, 0.618, 1] },
});
```

```ts
interface DrawingData {
  kind: string;
  anchors: { time: number | bigint; price: number }[];
  style?: DrawingStyle;
  params?: Record<string, number | boolean | string | number[]>;
}
```

- `time`은 UTC 밀리초(정수)입니다. 봉의 시간과 일치하지 않아도 됩니다.
- `price`는 패널의 첫 번째 가격 시리즈와 같은 단위이며, 유한한 값이어야 합니다.
- 앵커 개수는 종류에 맞아야 합니다. 대부분의 종류는 표에 적힌 개수와 정확히 같아야
  하고, 포지션은 2 또는 3개(2개면 손절이 목표의 대칭으로 추가됨), `path`와
  `brush`는 2~4096개입니다.
- `pane`의 기본값은 메인 패널입니다. 다른 패널에 그리려면 패널 id를 전달하십시오.
- 생략한 스타일 키는 아래의 기본값을 사용하며, 해당 종류가 쓰지 않는 매개변수
  키는 무시됩니다.

알 수 없는 종류, 너무 적거나 많은 앵커, 유한하지 않은 가격, 알 수 없는 패널,
잘못된 스타일이나 매개변수가 있으면 `add`는 예외를 발생시키고 아무것도 추가하지
않습니다. 코드로 추가한 드로잉은 선택되지 않습니다.

`get(id)`는 모든 키를 포함한 저장 형식을 반환합니다. `kind`, `anchors`, 전체
`style`, 그리고 종류별 매개변수가 담긴 `params`(`text`와 `callout`은 `text`,
피보나치 되돌림과 확장은 `levels`)입니다. 이를 `add`에 전달하면 드로잉을 복사할 수
있습니다.

```ts
const copy = chart.drawings.get(id);
if (copy) chart.drawings.add(copy, chart.drawings.paneOf(id));
```

`setAnchors(id, anchors)`는 드로잉을 이동합니다. 드로잉이 가진 앵커와 같은 개수가
필요합니다(완성된 포지션은 3개).

## 스타일

```ts
chart.drawings.setStyle(id, { line: '#26a69a', lineWidth: 2, lineDash: 'dashed' });
```

| 키 | 타입 | 기본값 | 용도 |
|---|---|---|---|
| `line` | `Color` | `#2962ff` | 선과 외곽선; 범위와 포지션의 레이블 배경; 콜아웃 상자 |
| `lineWidth` | `number` | `1` | CSS px, 0보다 커야 함 |
| `lineDash` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | 선의 대시 패턴 |
| `fill` | `Color` | 불투명도 16%의 `#2962ff` | 영역: 채널, 사각형, 타원, 범위, 피보나치 밴드, 피치포크, 패턴 삼각형, 텍스트 배경 |
| `text` | `Color` | `#2962ff` | 배경이 없는 텍스트와 레이블 |
| `fontSize` | `number` | `12` | CSS px, 텍스트와 레이블 |

`setStyle`은 전달한 키만 변경합니다. 색상은 `null`이 될 수 없습니다. 채우기를
없애려면 `'transparent'`를 사용하십시오. 포지션의 수익·손실 영역은 녹색과 빨간색으로
고정되어 있습니다.

## 매개변수

```ts
chart.drawings.setParam(textId, 'text', 'Breakout');
chart.drawings.setParam(fibId, 'levels', [0, 0.5, 1, 1.618]);
```

| 매개변수 | 종류 | 값 |
|---|---|---|
| `text` | `text`, `callout` | 문자열 |
| `levels` | `fib_retracement`, `fib_extension` | 최대 24개의 유한한 수 |

타입이 잘못된 값은 예외를 발생시키며 드로잉은 바뀌지 않습니다. 스타일과 매개변수
변경은 `drawingModified`를 전송하며 실행 취소할 수 있습니다.

## 표시, 잠금, 순서

```ts
chart.drawings.setVisible(id, false);   // hidden drawings are not drawn or hit
chart.drawings.setLocked(id, true);     // can be selected and deleted, not dragged
chart.drawings.bringToFront(id);
chart.drawings.sendToBack(id);
console.log(chart.drawings.isVisible(id), chart.drawings.isLocked(id), chart.drawings.ids());
```

선택된 드로잉을 숨기면 선택이 해제됩니다. 잠금은 코드에서 호출하는
`setAnchors`, `setStyle`, `remove`를 막지 않습니다. 표시, 잠금, 순서 변경은
이벤트를 보내지 않으며 실행 취소 히스토리에도 포함되지 않습니다. 이들은 차트 상태와
함께 저장됩니다.

## 선택

```ts
chart.drawings.select(id);      // selects it and shows its handles
chart.drawings.select(null);    // clears the selection
const sel = chart.drawings.selected; // the selected id, or undefined
```

## 제거

```ts
chart.drawings.remove(id);
for (const d of chart.drawings.ids()) chart.drawings.remove(d); // remove all
```

패널을 제거하면 그 패널의 드로잉도 제거되며, 드로잉이 있었다면 실행 취소
히스토리가 비워집니다.

## 실행 취소와 다시 실행

사용자가 했든 코드가 했든 다음 변경은 기록됩니다. 추가, 제거, 이동(드래그,
`setAnchors`), 스타일이나 매개변수 변경입니다. 선택, 표시, 잠금, Z 순서는 기록되지
않습니다.

```ts
chart.drawings.undo();          // false if there was nothing to undo
chart.drawings.redo();
if (chart.drawings.canUndo) { /* enable the undo button */ }
chart.drawings.clearHistory();
```

히스토리는 최근 256개의 변경을 보관합니다. 새로운 변경이 생기면 다시 실행 목록이
비워집니다. `undo`와 `redo`는 진행 중인 드로잉을 취소합니다. 상태를 불러오면
(`loadState`) 히스토리가 비워집니다. 동기화 그룹에서 받은 드로잉은 기록되지
않습니다.

## 드로잉 이벤트

| 이벤트 | 페이로드 | 발생 시점 |
|---|---|---|
| `drawingCreated` | `{ id }` | 사용자가 드로잉을 완성했거나, 코드로 추가했거나, 실행 취소/다시 실행으로 복원했거나, 불러왔을 때 |
| `drawingModified` | `{ id }` | 이동, 스타일 변경, 매개변수 변경이 있었을 때 |
| `drawingRemoved` | `{ id }` | 제거되었을 때 |
| `drawingSelected` | `{ id: number \| null }` | 선택이 바뀌었을 때 |

이벤트는 메서드 호출 중이 아니라, 변경이 일어난 프레임이 끝난 뒤에 도착합니다.
[크로스헤어와 이벤트](crosshair-events.md)를 참고하십시오.

```ts
chart.on('drawingCreated', (e: ChartEventMap['drawingCreated']) => {
  const d = chart.drawings.get(e.id);
  console.log('new', d?.kind, d?.anchors);
});
```

## 예제: 툴바

```html
<div id="toolbar">
  <button data-kind="trend_line">Trend</button>
  <button data-kind="horizontal_line">Horizontal</button>
  <button data-kind="rectangle">Rectangle</button>
  <button data-kind="fib_retracement">Fib</button>
  <button data-kind="long_position">Long</button>
  <button data-kind="path">Path</button>
  <button data-kind="brush">Brush</button>
  <button data-kind="text">Text</button>
  <label><input id="sticky" type="checkbox"> Keep drawing</label>
  <button id="lock">Lock</button>
  <button id="delete">Delete</button>
  <button id="undo">Undo</button>
  <button id="redo">Redo</button>
</div>
<canvas id="chart" style="width: 100%; height: 480px"></canvas>
```

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const canvas = document.getElementById('chart') as HTMLCanvasElement;
const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars);

const d = chart.drawings;
const toolbar = document.getElementById('toolbar') as HTMLDivElement;
const sticky = document.getElementById('sticky') as HTMLInputElement;
const button = (id: string) => document.getElementById(id) as HTMLButtonElement;
let lastKind: string | null = null;

function refresh() {
  const active = d.activeTool ?? null;
  toolbar.querySelectorAll<HTMLButtonElement>('button[data-kind]').forEach((b) => {
    b.classList.toggle('active', b.dataset.kind === active);
  });
  const sel = d.selected;
  button('delete').disabled = sel == null;
  button('lock').disabled = sel == null;
  button('lock').textContent = sel != null && d.isLocked(sel) ? 'Unlock' : 'Lock';
  button('undo').disabled = !d.canUndo;
  button('redo').disabled = !d.canRedo;
}

toolbar.addEventListener('click', (e) => {
  const kind = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-kind]')?.dataset.kind;
  if (!kind) return;
  if (d.activeTool === kind) {
    d.cancelTool();
    lastKind = null;
  } else {
    d.startTool(kind);
    lastKind = kind;
  }
  canvas.focus(); // so that Escape and Enter reach the chart
  refresh();
});

button('delete').onclick = () => {
  const sel = d.selected;
  if (sel != null) d.remove(sel);
};
button('lock').onclick = () => {
  const sel = d.selected;
  if (sel != null) d.setLocked(sel, !d.isLocked(sel));
  refresh();
};
button('undo').onclick = () => d.undo();
button('redo').onclick = () => d.redo();

chart.on('drawingCreated', (_e: ChartEventMap['drawingCreated']) => {
  if (sticky.checked && lastKind && d.activeTool == null) d.startTool(lastKind);
  refresh();
});
chart.on('drawingModified', refresh);
chart.on('drawingRemoved', refresh);
chart.on('drawingSelected', refresh);
// Escape disarms a tool without an event.
canvas.addEventListener('keyup', refresh);
refresh();
```

버튼으로 실행한 실행 취소와 다시 실행은 다음 프레임 뒤에 `drawingCreated`,
`drawingModified` 또는 `drawingRemoved`를 전송하므로, 그때 버튼이 갱신됩니다.

## 커스텀 도구

`chart.drawings.register(kind, tool)`는 JavaScript로 작성한 드로잉 도구를
추가합니다. 도구는 생성 방식(`anchorCount`: 클릭 횟수, `{ open: min }` 또는
`'freehand'`)을 지정하고 `build(ctx)`에서 그리기 명령을 반환합니다. 선택적인
`hitTest`는 누른 위치에서 무엇이 히트되는지 결정합니다. 등록한 종류는 이 차트의
`add`, `startTool`, `loadState`에서 사용할 수 있으며, 내장 종류는 교체할 수
없습니다. [플러그인](plugins.md#drawing-tools)을 참고하십시오.
