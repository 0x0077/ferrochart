<!-- source: d3368c1d0b -->
# 저장과 복원

`chart.saveState()`는 사용자가 차트를 다루면서 바꾸는 부분을 반환합니다.
확대·축소 수준, 스크롤 위치, 패널 높이, 가격 축 설정, 크로스헤어 모드, 드로잉이
해당됩니다. `chart.loadState(state)`는 이를 되돌려 놓습니다. 상태는 JSON과
호환되는 단순한 데이터이므로 `localStorage`, 파일, 서버 어디에든 보관할 수
있습니다.

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

declare const chart: FerroChart;

const state: ChartState = chart.saveState();
localStorage.setItem('chart-layout', JSON.stringify(state));
```

## 상태에 들어 있는 것

| 필드 | 내용 |
|---|---|
| `version` | 형식 버전, 현재 `1` |
| `barSpacing` | CSS px 단위의 봉 간격(확대·축소 수준) |
| `rightOffset` | 마지막 봉 오른쪽의 빈 공간, 봉 단위(스크롤 위치) |
| `crosshairMode` | `'normal'`, `'magnet'` 또는 `'hidden'` |
| `panes` | 패널마다 하나씩, 위에서 아래 순서(아래 참고) |
| `drawings` | 완성된 모든 드로잉, 아래에서 위 순서(아래 참고) |

각 `panes` 항목:

| 필드 | 내용 |
|---|---|
| `stretch` | 다른 패널에 대한 상대 높이 |
| `priceScale.mode` | `'normal'`, `'logarithmic'`, `'percentage'` 또는 `'indexedTo100'` |
| `priceScale.inverted` | 높은 가격을 아래쪽에 그리는지 여부 |
| `priceScale.margins` | `{ top, bottom }`, 패널 높이에 대한 비율 |
| `priceScale.range` | 자동 스케일이 꺼져 있으면(사용자가 가격 축을 드래그했거나 코드에서 범위를 설정한 경우) `{ low, high }`, 자동 스케일 중에는 `null` |

각 `drawings` 항목:

| 필드 | 내용 |
|---|---|
| `pane` | 위에서 아래로 센 패널의 인덱스(0은 메인 패널)이며, 패널 id가 아닙니다 |
| `visible` | 표시 여부 |
| `locked` | 편집 잠금 여부 |
| `data` | `{ kind, anchors, style, params }`, `chart.drawings.get(id)`와 같은 형태 |

앵커는 시간(UTC ms)과 가격으로 저장되므로, 데이터가 바뀌거나 과거 데이터가 더
로드되어도 드로잉은 올바른 봉에 머뭅니다.

## 상태에 들어 있지 않은 것

상태는 차트가 무엇을 보여 주는지가 아니라 사용자가 차트를 어떻게 배치했는지를
기술합니다. 다음은 저장되지 **않습니다**.

- 데이터, 시리즈와 그 옵션
- 지표, 밴드, 마커, 가격선
- 레이어 플러그인과 등록된 드로잉 도구
- 테마, 렌더러 및 그 밖의 `FerroChart.create` 옵션
- 동기화 그룹 참여 여부
- 드로잉 id, 선택 상태, 활성화된 도구, 생성 중인 드로잉, 실행 취소 기록

애플리케이션이 이 모든 것을 직접 만든 다음, 그 위에 상태를 불러옵니다. 이것들을
다시 만드는 데 필요한 정보(종목, 주기, 지표 목록과 매개변수)는 상태와 함께
보관하세요.

## 상태 불러오기

`loadState`에는 저장할 때와 같은 방식으로 구성된 차트가 필요합니다. 특히
**패널 수가 같아야** 합니다. 패널은 위치로 대응되므로, 불러오기 전에 패널을
추가하는 시리즈와 지표(예: 기본 배치의 `'macd'`나 `'rsi'`, 또는 `addPane()`)를
같은 순서로 만드세요. 데이터도 먼저 불러오세요. 복원된 드로잉은 시간을 기준으로
배치됩니다.

```ts
import { FerroChart, randomWalk, type ChartState } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;
declare const saved: ChartState;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(randomWalk(1, 2_000, Date.UTC(2024, 0, 1), 60_000));
chart.addVolumeSeries(candles);          // same pane: no new pane
chart.addIndicator(candles, 'macd');     // adds a pane, as before saving

chart.loadState(saved);                  // two panes on both sides
```

불러오기가 하는 일:

- 봉 간격, 오른쪽 여백, 크로스헤어 모드를 설정합니다.
- 각 패널에 stretch와 가격 축 설정을 적용합니다. `range`와 함께 저장된 패널은
  자동 스케일이 꺼진 채 그 범위를 유지하고, `range: null`로 저장된 패널은 자동
  스케일을 다시 켭니다.
- 현재 드로잉을 모두 제거하고 저장된 드로잉을 추가합니다. 새 id가 부여되며,
  차트는 이들에 대해 `drawingRemoved`와 `drawingCreated` 이벤트를 보냅니다.
- 실행 취소 기록을 지웁니다.

### JavaScript로 작성한 드로잉 도구

`chart.drawings.register(kind, tool)`로 등록한 종류의 드로잉은 그 종류가 등록된
차트에만 불러올 수 있습니다. `loadState`를 호출하기 전에 도구를 등록하세요.
[드로잉](drawings.md)과 [플러그인](plugins.md)을 참고하세요.

## 오류

상태가 맞지 않으면 `loadState`는 `Error`를 던지고 차트를 바꾸지 않습니다. 일부만
적용되는 일은 없습니다.

| 원인 | 메시지 예 |
|---|---|
| 객체가 아님 | `state must be an object from saveState()` |
| 필수 필드가 없음 | `state: missing 'barSpacing'` |
| 알 수 없는 이름 | `state: unknown crosshair mode 'x'`, `state: unknown price scale mode 'x'` |
| 더 새로운 형식 버전 | `state version is newer than supported` |
| 패널 수가 다름 | `state has a different number of panes` |
| 봉 간격이 유한하지 않거나 양수가 아님, 오른쪽 여백이 유한하지 않음 | `invalid bar spacing or right offset` |
| 여백이 0~1 범위를 벗어남(또는 합이 1 이상), 범위가 유한하고 증가하지 않거나 로그 스케일에서 양수가 아님 | `invalid price scale state` |
| 존재하지 않는 패널 인덱스의 드로잉 | `drawing on a missing pane` |
| 차트가 만들 수 없는 드로잉(알 수 없는 종류, 잘못된 앵커 수, 유효하지 않은 스타일이나 매개변수) | 드로잉에 따라 다름 |

패널 `stretch`가 양수가 아니면 오류 대신 1로 대체됩니다.

저장된 상태가 오래되었을 수 있다면(예: 앱의 기본 지표를 바꾼 뒤) 오류를 잡아서
기본 레이아웃으로 대체하세요.

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

declare const chart: FerroChart;

function restore(chart: FerroChart, json: string | null): boolean {
  if (json === null) return false;
  try {
    chart.loadState(JSON.parse(json) as ChartState);
    return true;
  } catch (e) {
    console.warn('saved layout ignored:', e);
    return false;
  }
}

restore(chart, localStorage.getItem('chart-layout'));
```

## 전체 예제

아래 페이지는 종목마다 레이아웃 하나를 `localStorage`에 보관합니다. 설정
함수가 상태에 없는 모든 것(시리즈, 데이터, 지표, 그 색상)을 만들고, 나머지는
상태가 복원합니다. 레이아웃은 사용자가 드로잉을 바꿀 때마다, 그리고 페이지가
숨겨질 때 저장됩니다.

```ts
import { FerroChart, randomWalk, type ChartState, type Series } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

/** What the app needs to rebuild a chart; kept next to the chart state. */
interface Layout {
  symbol: string;
  indicators: { name: string; params: Record<string, number | string> }[];
  state: ChartState;
}

const key = (symbol: string) => `layout:${symbol}`;

function loadBars(symbol: string) {
  // Replace with your data source.
  const seed = [...symbol].reduce((s, c) => s + c.charCodeAt(0), 0);
  return randomWalk(seed, 5_000, Date.UTC(2024, 0, 1), 60_000);
}

async function openChart(symbol: string): Promise<FerroChart> {
  const chart = await FerroChart.create(canvas, { theme: 'dark' });

  // 1. Series and data (not part of the state).
  const candles: Series = chart.addCandlestickSeries();
  candles.setData(loadBars(symbol));
  chart.addVolumeSeries(candles);

  // 2. Indicators, in the order they were saved, so the panes match.
  const json = localStorage.getItem(key(symbol));
  const saved: Layout | null = json ? (JSON.parse(json) as Layout) : null;
  const indicators = saved?.indicators ?? [{ name: 'sma', params: { n: 50 } }, { name: 'rsi', params: { n: 14 } }];
  for (const { name, params } of indicators) {
    chart.addIndicator(candles, name, params);
  }

  // 3. The saved view, layout and drawings.
  if (saved) {
    try {
      chart.loadState(saved.state);
    } catch (e) {
      console.warn('saved layout ignored:', e);
    }
  }

  // Save after every drawing change and when the page is hidden.
  const save = () => {
    const layout: Layout = { symbol, indicators, state: chart.saveState() };
    localStorage.setItem(key(symbol), JSON.stringify(layout));
  };
  for (const event of ['drawingCreated', 'drawingModified', 'drawingRemoved'] as const) {
    chart.on(event, save);
  }
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') save();
  });
  return chart;
}

await openChart('BTCUSD');
```

이 예제에서 실무상 중요한 세부 사항은 다음과 같습니다.

- 지표 목록을 상태와 함께 저장합니다. 사용자가 지표를 추가하거나 제거하면 패널
  수가 바뀌어 이전 상태를 더 이상 불러올 수 없기 때문입니다.
- `drawingModified`는 사용자가 드로잉을 드래그하는 동안 프레임당 최대 한 번
  전송됩니다. 작은 상태를 그 정도로 자주 `localStorage`에 쓰는 것은 괜찮지만,
  서버에 저장한다면 호출에 디바운스를 거세요.
- 확대·축소 수준과 스크롤 위치는 드로잉 이벤트 없이 바뀌므로, 예제는 페이지가
  숨겨질 때도 저장합니다. 더 빨리 저장해야 한다면 `visibleRangeChanged`를
  구독하세요.

## 드로잉만 저장하기

드로잉만 보관하려면(예: 패널 구성이 다른 차트로 복사하려면) 상태 대신 드로잉
API에서 읽으세요.

```ts
import { FerroChart, type DrawingData } from '@ferrochart/core';

declare const source: FerroChart;
declare const target: FerroChart;

const drawings: Required<DrawingData>[] = source.drawings
  .ids()
  .filter((id) => source.drawings.paneOf(id) === source.mainPane)
  .map((id) => source.drawings.get(id))
  .filter((d): d is Required<DrawingData> => d !== null);

for (const d of drawings) target.drawings.add(d);   // to the main pane
```

열려 있는 차트 사이에서 드로잉을 계속 일치시키려면 [차트 동기화](sync.md)를
참고하세요.
