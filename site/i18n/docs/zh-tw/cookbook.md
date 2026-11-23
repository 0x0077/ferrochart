<!-- source: a58b5627d3 -->
# 實用配方

面向 FerroChart 應用開發的任務範例。每個瀏覽器範例都是獨立的 TypeScript 模組，需透過瀏覽器打包工具建置，並安裝 `@ferrochart/core`；React 範例還需安裝 React。將已掛載且寬度非零的 `HTMLElement` 作為 `host` 傳入，例如 `document.body`，並在移除視圖時呼叫傳回的清理函式。價格提醒範例則傳回 `update` 和 `destroy` 方法。參見 [Web 設定](web.md)。

範例使用模擬資料。時間戳記為 UTC 毫秒，K 棒依時間遞增排序。網路範例說明了自有行情來源應提供的資料格式；套件不附帶行情服務。Python 範例註明了所需的套件，並將 PNG 檔案寫入目前目錄。

## 透過 WebSocket 推送即時 K 棒

將圖表連接到自己的 WebSocket 端點，每則文字訊息傳送一根 JSON 格式的 OHLC K 棒。

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

- 行情來源必須提供以 UTC 毫秒表示的 `time`，以及 `open`、`high`、`low`、`close`（`volume` 可選）；呼叫 `update` 前先轉換資料提供者的訊息格式。
- 相同的開盤時間會取代最後一根 K 棒；較晚的時間會新增一根；較早的時間會擲出例外。每次更新都需提供目前 K 棒的完整資料，而非單筆成交的增量。
- 卸載時呼叫傳回的清理函式；重新連線由行情轉接層負責。參見[即時更新](data.md#real-time-updates)。

## 向左捲動時載入更早的歷史資料

載入上一頁歷史資料，同時保持目前視圖不變；這個獨立範例使用記憶體中的歷史資料代替後端。

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

- 將 `loadBefore` 替換為後端請求：傳回嚴格早於 `before`、時間戳記不重複且依時間遞增排序的資料。成交量欄應與初始資料保持一致。
- 每當最早一根 K 棒改變時，`needMoreHistory` 可觸發一次。空回應會結束分頁；請求失敗後可透過按鈕主動重試。
- `prepend` 會保留現有邏輯索引和視圖；不要在每頁載入後呼叫 `fitContent`。參見[載入更早的歷史資料](data.md#loading-older-history)。

## 將指標放在獨立窗格中

為同一 K 棒系列加入疊加顯示的移動平均線和獨立的 RSI 窗格。

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

- 第四個參數指定放置位置：`'overlay'`、`'newPane'` 或既有窗格的 id。預設位置取決於指標。
- 指標會隨來源資料更新，無需單獨傳入資料。請為所選週期提供足夠的歷史資料。參見[指標位置](indicators.md#placement)。

## 將買賣紀錄關聯到標記

使用者點擊標記時，取回自己的成交物件。

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

- `id` 是自訂的非負安全整數識別碼；`target.marker` 是傳入 `setMarkers` 的物件，包含自訂的 `trade` 欄位。
- 事件中的索引對應依時間排序後的標記，並非原陣列順序。收合徽章的類型為 `'markerOverflow'`，可視需要單獨處理。
- `setMarkers` 會取代整個標記清單。參見[標記](markers.md#markers)和[點擊事件](crosshair-events.md#click)。

## 繪製價格提醒線並偵測穿越

顯示價格門檻，在傳入 K 棒的收盤價向上穿越門檻時通知應用程式。

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

- 透過傳回的 `update` 方法傳入行情 K 棒；價格線本身不會監控價格或傳送提醒。
- 這裡比較相鄰兩次收盤價，包括尚未結束的 K 棒更新。只有價格先跌破門檻、再向上穿越時才會再次觸發；這不是背景服務。
- 卸載時呼叫 `destroy`。參見[價格線](markers.md#price-lines)和[即時更新](data.md#real-time-updates)。

## 同步兩個圖表

在兩個圖表間同步十字游標時間和可見時間範圍。

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

- 同步依據時間，而非 K 棒索引；每個圖表仍需自己的資料。K 棒間距限制可能約束圖表實際顯示的範圍。
- 銷毀圖表會使其離開同步群組。先銷毀群組內的圖表，再釋放群組；繪圖同步需單獨設定，預設關閉。參見[圖表同步](sync.md#time-not-bar-numbers)。

## 儲存繪圖和視窗範圍

將使用者的視圖和繪圖儲存在 localStorage 中，圖表資料由應用程式自行重建。

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

- 呼叫 `loadState` 前，先重建資料、系列、指標及相同的窗格配置；狀態不包含行情資料或系列定義。
- 還原狀態會取代繪圖並清空復原歷程。為不相容的配置或不同商品使用不同的儲存鍵，並顯示儲存或還原時的錯誤。
- 卸載前點擊 Save。參見[狀態內容](state.md#what-a-state-contains)和[載入狀態](state.md#loading-a-state)。

## 在 React 中建立和銷毀圖表

讓 GPU 資源隨已掛載的畫布建立和釋放，同時處理初始化尚未完成就卸載的情況。

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

- 在用戶端元件中渲染 `<PriceChart />`。effect 在瀏覽器中執行；disposed 檢查也能處理 React 開發模式中的初始化與清理循環。
- 即時資料情境中，將系列儲存在 ref 中並呼叫 `update`；不要每次收到行情都重建圖表。參見 [React](frameworks.md#react)。

## 切換淺色與深色主題

使用另一個主題重建圖表，同時保留視窗範圍和繪圖。

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

- 主題在建立時指定，不能透過 `applyOptions` 更新。還原狀態前，重建相同的資料、系列和窗格，並重新訂閱事件。
- 未明確設定顏色的系列會跟隨新主題。此範例在重建期間停用切換；重建失敗時會回報錯誤，不會自動重試。參見[主題](styling.md#theme)和[狀態不包含的內容](state.md#what-it-does-not-contain)。

## 為十字游標建立 DOM 提示框

在自己的 HTML 浮層中顯示游標停留位置的 K 棒收盤價。

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

- 座標是相對於畫布的 CSS 像素，因此浮層應放在同一個已定位的容器中。顯示行情來源提供的文字時使用 `textContent`。
- 離開繪圖區時會收到 `point: null`；資料缺口處可能沒有 K 棒。同步的十字游標沒有指標座標，因此本例會隱藏其提示框。參見[十字游標事件](crosshair-events.md#crosshairmove)。

## 將 pandas DataFrame 匯出為 PNG

使用 pandas DataFrame 中的 OHLCV 資料繪圖，無需開啟視窗即可儲存圖表影像。

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

- 安裝 `ferrochart` 和 `pandas`。日期時間欄會轉換為 UTC 毫秒；此例明確提供 UTC 時間戳記。
- 資料列應依時間排序，並包含所有 OHLC 欄。`to_png(path)` 會寫入 PNG 檔案，無需呼叫 `show()`。參見 [Python 快速入門](python.md#quick-start)。

## 在僅有 CPU 的伺服器上渲染 PNG

無需視窗或 GPU 介面卡即可產生 PNG 位元組資料，用於寫入檔案或建立自己的 HTTP 回應。

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

- 安裝 `ferrochart`。`backend="cpu"` 是 `to_png` 的參數，不是 `Chart` 建構函式的參數；這條渲染路徑不會建立 GPU 介面卡。
- 不傳路徑時，`to_png` 傳回位元組資料。應根據輸出需求指定尺寸，而非依賴瀏覽器畫布。參見 [CPU 離屏渲染](offscreen.md#on-the-cpu)和 [Python](python.md#quick-start)。
