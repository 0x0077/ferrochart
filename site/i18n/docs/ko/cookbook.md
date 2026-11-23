<!-- source: a58b5627d3 -->
# 쿡북

FerroChart를 사용하는 애플리케이션을 위한 작업별 예제입니다. 각 브라우저 예제는
`@ferrochart/core`가 설치된 브라우저 번들러에서 사용하는 독립적인 TypeScript
모듈이며, React 예제에는 React도 필요합니다. DOM에 마운트되어 있고 너비가 0이
아닌 `HTMLElement`를 `host`로 전달하세요. 예를 들어 `document.body`를 사용할 수
있습니다. 뷰를 제거할 때는 반환된 정리 함수를 호출하세요. 가격 알림 예제는
정리 함수 대신 `update`와 `destroy` 메서드를 반환합니다. [웹 설정](web.md)을 참고하세요.

예제는 합성 데이터를 사용합니다. 타임스탬프는 UTC 밀리초이며, 봉은 시간 오름차순으로
정렬되어 있습니다. 네트워크 예제는 자체 데이터 피드가 따라야 하는 입력 규약을
설명하며, 시장 데이터 서비스는 포함되어 있지 않습니다. Python 예제에는 필요한
패키지가 명시되어 있으며, PNG 파일을 현재 디렉터리에 저장합니다.

## WebSocket으로 캔들 스트리밍하기

텍스트 메시지마다 JSON OHLC 봉 하나를 보내는 자체 WebSocket 엔드포인트에 차트를 연결합니다.

```ts
import { FerroChart, type Bar } from '@ferrochart/core';

export async function mountLiveChart(host: HTMLElement, feedUrl: string) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const candles = chart.addCandlestickSeries();
  const socket = new WebSocket(feedUrl);
  const receive = (event: MessageEvent<string>) => {
    try {
      const bar = JSON.parse(event.data) as Bar;
      candles.update(bar);
    } catch (error) {
      console.error('Invalid or out-of-order bar', error);
    }
  };
  socket.addEventListener('message', receive);
  socket.addEventListener('error', () => console.error('WebSocket error'));
  return () => {
    socket.removeEventListener('message', receive);
    socket.close();
    chart.destroy();
    canvas.remove();
  };
}
```

- 피드는 UTC 밀리초 단위의 `time`과 `open`, `high`, `low`, `close`를 보내야 합니다(`volume`은 선택 사항). `update`를 호출하기 전에 데이터 제공자의 페이로드를 이 형식으로 변환하세요.
- 시작 시간이 같으면 마지막 캔들을 교체하고, 더 늦으면 새 캔들을 추가합니다. 더 이른 시간은 예외를 던집니다. 매번 개별 체결 데이터의 증분이 아니라 현재 캔들의 전체 데이터를 전달해야 합니다.
- 언마운트할 때 반환된 정리 함수를 호출하세요. 연결 복구는 데이터 피드 어댑터에서 처리합니다. [실시간 업데이트](data.md#real-time-updates)를 참고하세요.

## 왼쪽으로 스크롤할 때 과거 데이터 불러오기

현재 뷰를 유지하면서 더 오래된 데이터 페이지를 불러옵니다. 이 독립 실행 예제는 백엔드 대신 메모리 내 데이터 저장소를 사용합니다.

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountHistory(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const archive = Array.from({ length: 120 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const initial = archive.slice(-40);
  const candles = chart.addCandlestickSeries();
  let oldest = initial[0].time;
  let loading = false, exhausted = false, disposed = false;
  const button = document.createElement('button');
  button.textContent = 'Load earlier / retry';
  host.append(button);
  async function loadBefore(before: number) {
    return archive.filter(bar => bar.time < before).slice(-40);
  }
  async function loadOlder() {
    if (loading || exhausted || disposed) return;
    loading = true;
    button.disabled = true;
    try {
      const older = await loadBefore(oldest);
      if (disposed) return;
      if (!older.length) { exhausted = true; return; }
      candles.prepend(older);
      oldest = older[0].time;
    } catch (error) {
      console.error('History load failed; use the retry button', error);
    } finally {
      loading = false;
      button.disabled = exhausted;
    }
  }
  chart.on('needMoreHistory', loadOlder);
  button.onclick = () => { void loadOlder(); };
  candles.setData(initial);
  chart.fitContent();
  return () => {
    disposed = true;
    chart.destroy();
    canvas.remove();
    button.remove();
  };
}
```

- `loadBefore`를 자체 백엔드 요청으로 바꾸세요. `before`보다 엄격하게 이른 타임스탬프를 중복 없이 오름차순으로 반환해야 합니다. 거래량 열의 유무도 초기 데이터와 일치시켜야 합니다.
- `needMoreHistory`는 시리즈의 맨 앞 봉 하나당 한 번 발생합니다. 빈 응답을 받으면 페이지 불러오기를 끝내며, 요청이 실패하면 버튼으로 명시적으로 재시도할 수 있습니다.
- `prepend`는 기존 논리 인덱스와 뷰를 유지합니다. 페이지를 불러올 때마다 `fitContent`를 호출하지 마세요. [과거 데이터 불러오기](data.md#loading-older-history)를 참고하세요.

## 지표를 별도 패널에 배치하기

같은 캔들 시리즈에 이동평균 오버레이와 RSI 패널을 추가합니다.

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountIndicators(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  chart.addIndicator(candles, 'ema', { n: 20 }, 'overlay');
  chart.addIndicator(candles, 'rsi', { n: 14 }, 'newPane');
  return () => { chart.destroy(); canvas.remove(); };
}
```

- 배치는 네 번째 인수로 지정합니다. `'overlay'`, `'newPane'` 또는 기존 패널 id를 사용할 수 있습니다. 기본값은 지표에 따라 다릅니다.
- 지표는 원본 데이터를 따라가므로 별도로 데이터를 넣지 마세요. 선택한 기간에 충분한 과거 데이터를 제공해야 합니다. [지표 배치](indicators.md#placement)를 참고하세요.

## 매수·매도 기록을 마커에 연결하기

사용자가 마커를 클릭하면 해당하는 애플리케이션의 거래 객체를 찾습니다.

```ts
import { FerroChart, type Marker } from '@ferrochart/core';

type Trade = { id: number; time: number; side: 'buy' | 'sell'; quantity: number };

export async function mountTradeMarkers(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  const trades: Trade[] = [
    { id: 1, time: bars[10].time, side: 'buy', quantity: 2 },
    { id: 2, time: bars[30].time, side: 'sell', quantity: 1 },
  ];
  const markers: (Marker & { trade: Trade })[] = trades.map(trade => ({
    id: trade.id, time: trade.time, trade,
    position: trade.side === 'buy' ? 'belowBar' : 'aboveBar',
    shape: trade.side === 'buy' ? 'arrowUp' : 'arrowDown',
    text: trade.side,
  }));
  candles.setMarkers(markers);
  const info = document.createElement('p');
  info.textContent = 'Click a buy or sell marker';
  host.append(info);
  chart.on('click', ({ target }) => {
    if (target?.type !== 'marker' || target.series.id !== candles.id) return;
    const marker = markers.find(candidate => candidate === target.marker);
    if (marker) info.textContent = `Trade ${target.id}: ${marker.trade.side} ${marker.trade.quantity}`;
  });
  return () => { chart.destroy(); canvas.remove(); info.remove(); };
}
```

- `id`에는 애플리케이션에서 정한 0 이상의 안전한 정수 식별자를 사용합니다. `target.marker`는 `setMarkers`에 전달한 객체이며, 사용자 정의 `trade` 필드도 그대로 포함합니다.
- 이벤트의 인덱스는 원래 배열 순서가 아니라 시간순으로 정렬된 마커를 기준으로 합니다. 접힌 배지는 타입이 `'markerOverflow'`이므로 필요한 경우 따로 처리하세요.
- `setMarkers`는 마커 목록을 교체합니다. [마커](markers.md#markers)와 [클릭 이벤트](crosshair-events.md#click)를 참고하세요.

## 가격 알림선을 그리고 돌파 감지하기

임계 가격을 표시하고, 들어오는 캔들의 종가가 이를 상향 돌파하면 애플리케이션에 알립니다.

```ts
import { FerroChart, type Bar } from '@ferrochart/core';

export async function mountPriceAlert(host: HTMLElement, notify: (price: number) => void) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const candles = chart.addCandlestickSeries();
  const first = { time: Date.UTC(2025, 0, 1), open: 100, high: 102, low: 99, close: 101 };
  candles.setData([first]);
  const threshold = 105;
  candles.createPriceLine({ price: threshold, title: 'Alert', color: '#ef5350', lineStyle: 'dashed' });
  let previous = first.close;
  return {
    update(bar: Bar) {
      candles.update(bar);
      const crossed = previous < threshold && bar.close >= threshold;
      previous = bar.close;
      if (crossed) notify(bar.close);
    },
    destroy() { chart.destroy(); canvas.remove(); },
  };
}
```

- 반환된 `update` 메서드에 피드의 봉을 전달하세요. 가격선 자체는 가격을 감시하거나 알림을 보내지 않습니다.
- 이 예제는 형성 중인 캔들의 업데이트를 포함해 연속된 종가 값 사이의 돌파를 감지합니다. 가격이 임계값 아래로 내려갔다가 다시 상향 돌파해야 재발생하며, 백그라운드 서비스로 동작하지는 않습니다.
- 언마운트할 때 `destroy`를 호출하세요. [가격선](markers.md#price-lines)과 [실시간 업데이트](data.md#real-time-updates)를 참고하세요.

## 두 차트 동기화하기

두 차트의 크로스헤어 시간과 보이는 시간 범위를 공유합니다.

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

export async function mountSyncedCharts(host: HTMLElement) {
  const canvases = [document.createElement('canvas'), document.createElement('canvas')];
  for (const canvas of canvases) {
    canvas.style.cssText = 'display:block;width:100%;height:260px';
    host.append(canvas);
  }
  const first = await FerroChart.create(canvases[0]);
  let second: FerroChart;
  try { second = await FerroChart.create(canvases[1]); }
  catch (error) { first.destroy(); canvases.forEach(canvas => canvas.remove()); throw error; }
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  first.addCandlestickSeries().setData(bars);
  second.addSeries('line').setData(bars.map(bar => ({ time: bar.time, value: bar.close })));
  first.fitContent();
  second.fitContent();
  const group = new SyncGroup();
  first.joinSync(group, { crosshair: true, visibleRange: true });
  second.joinSync(group, { crosshair: true, visibleRange: true });
  return () => {
    first.destroy(); second.destroy();
    group.free();
    canvases.forEach(canvas => canvas.remove());
  };
}
```

- 동기화는 봉 인덱스가 아니라 시간을 기준으로 하며, 각 차트에는 여전히 자체 데이터가 필요합니다. 봉 간격 제한에 따라 차트에 표시되는 범위가 제한될 수 있습니다.
- 차트를 제거하면 그룹에서 나갑니다. 차트들을 제거한 뒤 그룹을 해제하세요. 드로잉 동기화는 별도 기능이며 기본적으로 꺼져 있습니다. [차트 동기화](sync.md#time-not-bar-numbers)를 참고하세요.

## 드로잉과 뷰포트 저장하기

차트 데이터는 애플리케이션에서 다시 구성하고, 사용자의 뷰와 드로잉은 localStorage에 보관합니다.

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

export async function mountSavedChart(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  const key = 'ferrochart-cookbook-view';
  const status = document.createElement('p');
  const draw = document.createElement('button');
  draw.textContent = 'Draw trend line';
  draw.onclick = () => chart.drawings.startTool('trend_line');
  const save = document.createElement('button');
  save.textContent = 'Save view and drawings';
  save.onclick = () => {
    try {
      localStorage.setItem(key, JSON.stringify(chart.saveState()));
      status.textContent = 'Saved';
    } catch (error) { status.textContent = String(error); }
  };
  host.append(draw, save, status);
  try {
    const stored = localStorage.getItem(key);
    if (stored) chart.loadState(JSON.parse(stored) as ChartState);
  } catch (error) { status.textContent = `Could not restore: ${String(error)}`; }
  return () => { chart.destroy(); canvas.remove(); draw.remove(); save.remove(); status.remove(); };
}
```

- `loadState` 전에 데이터, 시리즈, 지표와 동일한 패널 구성을 다시 만드세요. 상태에는 시장 데이터나 시리즈 정의가 포함되지 않습니다.
- 복원하면 드로잉이 교체되고 실행 취소 기록이 지워집니다. 서로 호환되지 않는 레이아웃이나 종목에는 다른 저장 키를 사용하고, 저장 및 복원 오류를 사용자에게 표시하세요.
- 언마운트하기 전에 저장 버튼을 누르세요. [상태에 포함되는 내용](state.md#what-a-state-contains)과 [상태 불러오기](state.md#loading-a-state)를 참고하세요.

## React에서 차트 생성하고 제거하기

초기화가 끝나기 전에 언마운트되는 경우까지 고려하여 GPU 리소스의 수명을 마운트된 캔버스에 맞춥니다.

```tsx
'use client';

import { useEffect, useRef } from 'react';
import { FerroChart, type OhlcRow } from '@ferrochart/core';

const bars: OhlcRow[] = Array.from({ length: 60 }, (_, i) => ({
  time: Date.UTC(2025, 0, 1) + i * 60_000,
  open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
}));

export function PriceChart() {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    let disposed = false;
    let chart: FerroChart | undefined;
    void FerroChart.create(element).then(created => {
      if (disposed) { created.destroy(); return; }
      chart = created;
      created.addCandlestickSeries().setData(bars);
      created.fitContent();
    }).catch(error => console.error('Chart initialization failed', error));
    return () => { disposed = true; chart?.destroy(); };
  }, []);
  return <canvas ref={canvas} style={{ display: 'block', width: '100%', height: 360 }} />;
}
```

- 클라이언트 컴포넌트에서 `<PriceChart />`를 렌더링하세요. effect는 브라우저에서 실행됩니다. disposed 확인은 React 개발 모드의 설정/정리 주기도 처리합니다.
- 실시간 데이터는 시리즈를 ref에 보관하고 `update`로 갱신하세요. 틱마다 차트를 다시 만들지 마세요. [React](frameworks.md#react)를 참고하세요.

## 밝은 테마와 어두운 테마 전환하기

뷰포트와 드로잉을 유지하면서 다른 테마로 차트를 다시 만듭니다.

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountThemeSwitch(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  let theme: 'light' | 'dark' = 'light';
  const create = async () => {
    const result = await FerroChart.create(canvas, { theme });
    result.addCandlestickSeries().setData(bars);
    return result;
  };
  let chart: FerroChart | undefined = await create();
  chart.fitContent();
  let disposed = false;
  const button = document.createElement('button');
  button.textContent = 'Switch theme';
  host.append(button);
  button.onclick = async () => {
    if (!chart) return;
    button.disabled = true;
    const state = chart.saveState();
    chart.destroy();
    chart = undefined;
    theme = theme === 'light' ? 'dark' : 'light';
    try {
      const next = await create();
      if (disposed) { next.destroy(); return; }
      chart = next;
      chart.loadState(state);
      button.disabled = false;
    } catch (error) { console.error('Theme rebuild failed', error); }
  };
  return () => {
    disposed = true;
    chart?.destroy();
    canvas.remove(); button.remove();
  };
}
```

- 테마는 생성 옵션이며, `applyOptions`로 갱신할 수 없습니다. 상태를 복원하기 전에 동일한 데이터, 시리즈와 패널을 다시 만들고 이벤트 구독도 다시 연결하세요.
- 색상을 지정하지 않은 시리즈는 새 테마를 따릅니다. 이 예제는 다시 만드는 동안 전환을 비활성화하며, 실패하면 자동으로 재시도하지 않고 오류를 알립니다. [테마](styling.md#theme)와 [상태에 포함되지 않는 내용](state.md#what-it-does-not-contain)을 참고하세요.

## 크로스헤어용 DOM 툴팁 만들기

마우스가 가리키는 캔들의 종가를 자체 HTML 오버레이에 표시합니다.

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountTooltip(host: HTMLElement) {
  const wrapper = document.createElement('div');
  wrapper.style.position = 'relative';
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  const tip = document.createElement('div');
  tip.hidden = true;
  tip.style.cssText = 'position:absolute;pointer-events:none;background:#111;color:#fff;padding:6px';
  wrapper.append(canvas, tip);
  host.append(wrapper);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  chart.on('crosshairMove', ({ point }) => {
    if (!point?.bar || point.time === null || point.x === undefined || point.y === undefined) {
      tip.hidden = true;
      return;
    }
    tip.textContent = `${new Date(point.time).toISOString()} · ${point.bar.close.toFixed(2)}`;
    tip.style.left = `${point.x + 12}px`;
    tip.style.top = `${point.y + 12}px`;
    tip.hidden = false;
  });
  return () => { chart.destroy(); wrapper.remove(); };
}
```

- 좌표는 캔버스 기준 CSS 픽셀이므로 오버레이를 같은 위치 지정 래퍼 안에 두세요. 피드에서 얻은 텍스트에는 `textContent`를 사용하세요.
- 플롯 영역을 벗어나면 `point: null`이 전달되며, 공백에는 봉이 없을 수 있습니다. 동기화된 크로스헤어에는 포인터 좌표가 없으므로 이 예제에서는 숨깁니다. [크로스헤어 이벤트](crosshair-events.md#crosshairmove)를 참고하세요.

## pandas DataFrame을 PNG로 내보내기

pandas DataFrame의 OHLCV 데이터를 그리고 창을 열지 않고 차트 이미지로 저장합니다.

```python
import pandas as pd
import ferrochart as fc

df = pd.DataFrame({
    "time": pd.to_datetime([
        "2026-01-01T00:00:00Z", "2026-01-01T00:01:00Z", "2026-01-01T00:02:00Z"
    ]),
    "open": [100.0, 101.0, 100.5],
    "high": [102.0, 102.5, 103.0],
    "low": [99.0, 100.0, 100.0],
    "close": [101.0, 100.5, 102.0],
    "volume": [1200.0, 950.0, 1400.0],
})
chart = fc.Chart(theme="dark", width=800, height=450)
candles = chart.add_series("candlestick")
candles.set_data(df)
chart.add_volume_series(candles)
chart.fit_content()
chart.to_png("pandas-chart.png")
```

- `ferrochart`와 `pandas`를 설치하세요. datetime 열은 UTC 밀리초로 변환됩니다. 이 예제는 UTC 타임스탬프를 명시적으로 지정합니다.
- 행을 시간순으로 정렬하고 모든 OHLC 열을 제공하세요. `to_png(path)`는 PNG를 저장하며, `show()`를 호출할 필요는 없습니다. [Python 빠른 시작](python.md#quick-start)을 참고하세요.

## CPU 전용 서버에서 PNG 렌더링하기

창이나 GPU 어댑터 없이 PNG 바이트를 생성하여 파일로 저장하거나 자체 HTTP 응답에 사용합니다.

```python
from pathlib import Path
import ferrochart as fc

chart = fc.Chart(width=800, height=450)
chart.add_series("line", color="#2196f3").set_data({
    "time": [1767225600000, 1767225660000, 1767225720000],
    "value": [100.0, 101.5, 100.8],
})
chart.fit_content()
png = chart.to_png(backend="cpu")
Path("server-chart.png").write_bytes(png)
```

- `ferrochart`를 설치하세요. `backend="cpu"`는 `Chart` 생성자가 아니라 `to_png`의 인수이며, 이 경로에서는 GPU 어댑터를 만들지 않습니다.
- 경로를 전달하지 않으면 `to_png`는 바이트를 반환합니다. 브라우저 캔버스에 의존하지 말고 출력에 맞는 크기를 지정하세요. [CPU 오프스크린 렌더링](offscreen.md#on-the-cpu)과 [Python](python.md#quick-start)을 참고하세요.
