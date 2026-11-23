<!-- source: d3368c1d0b -->
# 儲存與還原

`chart.saveState()` 會傳回使用者在操作過程中改變的圖表部分：縮放、捲動位置、窗格高度、價格軸設定、十字游標模式與繪圖。`chart.loadState(state)` 則把它們還原回去。狀態是與 JSON 相容的純資料，因此可以存放在 `localStorage`、檔案或你的伺服器上。

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

declare const chart: FerroChart;

const state: ChartState = chart.saveState();
localStorage.setItem('chart-layout', JSON.stringify(state));
```

## 狀態包含的內容

| 欄位 | 內容 |
|---|---|
| `version` | 格式版本，目前為 `1` |
| `barSpacing` | K 棒間距，單位為 CSS px（縮放程度） |
| `rightOffset` | 最後一根 K 棒右側的空白，以 K 棒數計（捲動位置） |
| `crosshairMode` | `'normal'`、`'magnet'` 或 `'hidden'` |
| `panes` | 每個窗格一筆，由上而下（見下方） |
| `drawings` | 每個已完成的繪圖，由下層到上層（見下方） |

每筆 `panes`：

| 欄位 | 內容 |
|---|---|
| `stretch` | 相對於其他窗格的高度 |
| `priceScale.mode` | `'normal'`、`'logarithmic'`、`'percentage'` 或 `'indexedTo100'` |
| `priceScale.inverted` | 是否把較高的價格畫在較低處 |
| `priceScale.margins` | `{ top, bottom }`，以窗格高度的比例表示 |
| `priceScale.range` | 自動縮放關閉時（使用者拖曳了價格軸，或你設定了範圍）為 `{ low, high }`；自動縮放期間為 `null` |

每筆 `drawings`：

| 欄位 | 內容 |
|---|---|
| `pane` | 窗格的索引，由上而下（0 是主窗格），而非窗格 id |
| `visible` | 是否顯示 |
| `locked` | 是否鎖定以防止編輯 |
| `data` | `{ kind, anchors, style, params }`，與 `chart.drawings.get(id)` 相同 |

錨點以時間（UTC ms）與價格儲存，因此在資料變更或載入更多歷史資料後，繪圖仍會留在正確的 K 棒上。

## 狀態不包含的內容

狀態描述的是使用者如何安排圖表，而不是圖表顯示什麼。以下內容**不會**儲存：

- 資料、系列及其選項；
- 指標、通道帶、標記與價格線；
- 圖層外掛與已註冊的繪圖工具；
- 主題、渲染器與其他 `FerroChart.create` 選項；
- 同步群組成員資格；
- 繪圖 id、選取狀態、待命中的工具、仍在建立中的繪圖，以及復原歷程。

這些都由你的應用程式自行建立，再在其上載入狀態。請把重建它們所需的資訊（商品、週期、指標清單與參數）和狀態放在一起保存。

## 載入狀態

`loadState` 需要一個設定方式與儲存時相同的圖表：尤其是**窗格數量必須相同**。窗格依位置對應，因此在載入之前，請以相同順序建立會新增窗格的系列與指標（例如採用預設位置的 `'macd'` 或 `'rsi'`，或 `addPane()`）。也請先載入資料；還原的繪圖是依時間放置的。

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

載入會做的事：

- 設定 K 棒間距、右側偏移與十字游標模式。
- 每個窗格取得其 stretch 與價格軸設定。以 `range` 儲存的窗格會保留該範圍並關閉自動縮放；以 `range: null` 儲存的窗格會重新開啟自動縮放。
- 移除目前所有繪圖，並加入已儲存的繪圖。它們會取得新的 id，圖表會為它們傳送 `drawingRemoved` 與 `drawingCreated` 事件。
- 清除復原歷程。

### 以 JavaScript 撰寫的繪圖工具

以 `chart.drawings.register(kind, tool)` 註冊之種類的繪圖，只能載入到已註冊該種類的圖表中。請在呼叫 `loadState` 之前註冊你的工具。參見[繪圖](drawings.md)與[外掛](plugins.md)。

## 錯誤

當狀態不相符時，`loadState` 會擲出 `Error`，圖表保持不變。不會只套用一部分。

| 原因 | 訊息範例 |
|---|---|
| 不是物件 | `state must be an object from saveState()` |
| 缺少必要欄位 | `state: missing 'barSpacing'` |
| 未知的名稱 | `state: unknown crosshair mode 'x'`、`state: unknown price scale mode 'x'` |
| 較新的格式版本 | `state version is newer than supported` |
| 窗格數量不同 | `state has a different number of panes` |
| K 棒間距非有限值或非正數、右側偏移非有限值 | `invalid bar spacing or right offset` |
| 邊距超出 0 到 1（或總和達 1 以上）；範圍不是有限且遞增，或在對數座標下不是正數 | `invalid price scale state` |
| 繪圖位於不存在的窗格索引上 | `drawing on a missing pane` |
| 圖表無法建立的繪圖（未知種類、錨點數量錯誤、無效的樣式或參數） | 視繪圖而定 |

窗格的 `stretch` 若不是正數，會以 1 取代，而不視為錯誤。

當已儲存的狀態可能過時（例如你變更了應用程式的預設指標之後），請捕捉錯誤並退回預設版面：

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

## 完整範例

下方頁面在 `localStorage` 中為每個商品保存一份版面。設定函式會建立狀態中沒有的所有東西（系列、資料、指標及其顏色），其餘部分則由狀態還原。每當使用者變更繪圖，以及頁面被隱藏時，都會儲存版面。

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

這個範例中有三個細節在實務上很重要：

- 指標清單與狀態一起儲存。若使用者新增或移除指標，窗格數量會改變，舊的狀態將無法再載入。
- 使用者拖曳繪圖時會傳送 `drawingModified`，最多每影格一次。這麼頻繁地把小型狀態寫入 `localStorage` 沒有問題；若儲存到伺服器，請對呼叫做防抖（debounce）處理。
- 縮放與捲動位置的改變不會觸發任何繪圖事件，因此範例也會在頁面隱藏時儲存。若需要更早儲存它們，請監聽 `visibleRangeChanged`。

## 只儲存繪圖

若只想保存繪圖（例如要把它們複製到窗格配置不同的另一個圖表），請從繪圖 API 讀取，而不是從狀態讀取：

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

若想讓開啟中的多個圖表之間的繪圖保持同步，請參見[同步多個圖表](sync.md)。
