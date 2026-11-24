<!-- source: 8ace0a0ae7 -->
# 資料

本章說明如何把 K 棒與數值放進系列：資料格式、驗證規則、即時更新、載入更早的歷史資料，以及讀回資料。

## 欄與列

`setData` 會取代系列的所有資料。它接受兩種形式：欄物件（每個欄位一個陣列），或列物件陣列。

### 欄

OHLC 系列（K 線、OHLC 棒、平均 K 線（Heikin-Ashi）與各種轉換）接受 `OhlcData`：

| 欄 | 型別 | 說明 |
|---|---|---|
| `time` | `BigInt64Array`、`Float64Array` 或 `number[]` | K 棒開盤時間，UTC 毫秒，嚴格遞增 |
| `open`, `high`, `low`, `close` | `Float64Array` 或 `number[]` | |
| `volume` | `Float64Array` 或 `number[]` | 選用 |

數值系列（折線、面積、基準線、直方圖、點）接受 `ValueData`：

| 欄 | 型別 | 說明 |
|---|---|---|
| `time` | `BigInt64Array`、`Float64Array` 或 `number[]` | UTC 毫秒，嚴格遞增 |
| `value` | `Float64Array` 或 `number[]` | `NaN` 表示缺口 |
| `color` | `(string \| null)[]` 或 `Uint32Array` | 選用的逐點顏色，僅直方圖會使用（參見[系列類型](series.md#histogram)） |

所有欄的長度必須相同。

```ts
candles.setData({
  time: new BigInt64Array([1704067200000n, 1704067260000n, 1704067320000n]),
  open: new Float64Array([100.0, 100.4, 100.1]),
  high: new Float64Array([100.6, 100.5, 100.9]),
  low: new Float64Array([99.8, 99.9, 100.0]),
  close: new Float64Array([100.4, 100.1, 100.8]),
  volume: new Float64Array([1200, 900, 1500]),
});

line.setData({ time: [1704067200000, 1704067260000], value: [100.4, 100.1] });
```

### 列

也接受物件陣列：OHLC 系列使用 `OhlcRow`（`{ time, open, high, low,
close, volume? }`），數值系列使用 `ValueRow`（`{ time, value?, color? }`）。在列中，`time` 也可以是 `Date`。

```ts
candles.setData([
  { time: new Date('2024-01-01T00:00:00Z'), open: 100.0, high: 100.6, low: 99.8, close: 100.4 },
  { time: new Date('2024-01-01T00:01:00Z'), open: 100.4, high: 100.5, low: 99.9, close: 100.1 },
]);

line.setData([
  { time: 1704067200000, value: 100.4 },
  { time: 1704067260000 },                 // no value: a gap
  { time: 1704067320000, value: 100.8 },
]);
```

沒有 `value`（或值為 `NaN`）的列是缺口。缺少 OHLC 欄位時會擲出指明該欄位的錯誤（`data[3] has no 'open'`）。若部分列有 `volume` 而其他列沒有，缺少的成交量視為 0。

### 該用哪一種

列在任何處理之前都會先轉換成欄，因此兩者結果相同。若你的資料本來就是物件（JSON API、幾百根 K 棒），用列寫起來較簡單，就用列。大量資料請用欄：型別陣列每欄只需一次 `memcpy` 就能複製進引擎，不必逐根讀取物件，因此對一百萬根 K 棒呼叫 `setData` 只需數十毫秒。若你收到的是二進位資料（例如來自 WebSocket 或檔案），請直接建立型別陣列，完全略過物件。

### 時間

時間是自 Unix 紀元起算的 UTC 毫秒數，必須為整數。

- `BigInt64Array` 的時間會直接使用。
- `Float64Array` 與 `number[]` 的時間會轉換為整數；非有限值、帶有小數部分或超過 ±9·10¹⁵ 的值會擲出例外（`time[5] is not an integer millisecond timestamp`）。
- 在列與 `update` 中，`time` 可以是 number、bigint 或 `Date`。

若你的資料來源使用秒，請乘以 1000。

圖表以 UTC 顯示時間。

## 驗證

`setData` 與 `update` 會先檢查資料。只要有任何錯誤就會擲出例外，系列則保留原本的資料。

| 規則 | 錯誤訊息 |
|---|---|
| 時間嚴格遞增（不可重複） | `time is not strictly increasing at index 7` |
| 所有欄與 `time` 等長 | ``column `open` has a different length than `time` `` |
| 價格與成交量為有限值（缺口 K 棒除外） | `invalid OHLC values in bar 12` |
| `high` ≥ max(`open`, `close`) 且 `low` ≤ min(`open`, `close`) | `invalid OHLC values in bar 12` |
| 欄為數字陣列或型別陣列 | `column 'high' must be a Float64Array or an array of numbers` |

訊息中的索引是該筆資料在你傳入的資料中的位置。

四個價格全為 `NaN` 的 K 棒是缺口 K 棒（參見[缺口](#gaps)）；它是合法的，其成交量會設為 0。

## 缺口

- 在數值系列中，`NaN` 值（或沒有 `value` 的列）表示缺口。折線、面積與基準線會在該處中斷；直方圖與點則不繪製任何東西。
- 在 OHLC 系列中，開、高、低、收皆為 `NaN` 的 K 棒是缺口，不會繪製。

`dataAt` 對缺口傳回 `null`。

### 同一時間軸上的多個系列

圖表的所有系列共用一條時間軸：邏輯索引 42 在每個系列中都是同一個時間。當系列的時間不同時，時間軸是它們時間的聯集。若某個時間只有其他系列才有，該系列在此處就沒有資料點：OHLC 系列、直方圖與點在該處不顯示任何東西；折線、面積與基準線則連接相鄰的點。若新的時間落在時間軸既有的 K 棒之間，時間軸會重建，圖表並會傳送 `warning` 事件。

## K 棒品質旗標

在 K 線、OHLC 棒或 Heikin-Ashi 系列上使用 `setBarFlags`，描述應用程式補齊或裁切過的資料。旗標不會改變價格；請先傳入處理好的 OHLC 資料：

```ts
const start = Date.UTC(2024, 0, 1);
candles.setData([
  { time: start, open: 100, high: 102, low: 99, close: 101 },
  { time: start + 60_000, open: 101, high: 101, low: 101, close: 101 },
  { time: start + 120_000, open: 101, high: 104, low: 100, close: 103 },
]);
candles.applyOptions({ filledBarColor: '#78909c' });
candles.setBarFlags([
  { time: start + 60_000, filled: true },
  { time: start + 120_000, clipped: true },
]);
// Clear all quality flags when they are no longer needed:
// candles.setBarFlags([]);
```

- `filled` 標識資料來源在沒有真實成交時合成的 K 棒。它以 `filledBarColor` 繪製為空心（預設使用系列主色，不透明度為 40%）。真正缺失的 K 棒也可表示為[缺口](#gaps)；此旗標不會自動補齊缺失資料。
- `clipped` 標識已裁切異常影線的 K 棒。影線兩端各顯示一條短刻度線。請在應用程式中依時間保留原始最高價和最低價，並在提示框中顯示；圖表只有你傳入的裁切後數值。同一根 K 棒可以同時設定這兩個旗標。
- 每次呼叫都會取代全部旗標；`[]` 清空旗標。時間為 UTC 毫秒（也可使用 `Date`/`bigint`，與資料相同）；時間重複時最後一項生效。縮小時若多根 K 棒合併，僅當所有原始 K 棒都為 filled 時，合併後的 K 棒才為 filled；只要任意一根為 clipped，合併後的 K 棒就為 clipped。

Python 使用 `candles.set_bar_flags(times, filled=..., clipped=...)`，布林陣列的長度需與 `times` 相同，並使用 `candles.apply_options(filled_bar_color="#78909c")`。參見 [Python](python.md#options-indicators-and-drawings)和[系列選項](series.md#series-options)。

## 即時更新

`update` 會改變最後一根 K 棒或新增一根：

- 時間與最後一根 K 棒相同：取代它（形成中的 K 棒）。
- 時間較晚：附加一根新 K 棒。
- 時間較早：擲出例外。

```ts
// OHLC series
candles.update({ time: 1704067380000, open: 100.8, high: 101.2, low: 100.7, close: 101.0, volume: 300 });
// Value series
line.update({ time: 1704067380000, value: 101.0 });
```

該 K 棒會以與 `setData` 資料相同的方式驗證。取代最後一根 K 棒不會配置記憶體，且圖表每個動畫影格最多重繪一次，因此每筆成交都呼叫 `update` 也沒有問題。系列上的指標會以增量方式更新。成交量系列跟隨其來源系列，本身沒有 `update`。

對於轉換系列（Renko、Kagi 等），每次 `update` 是一根輸入 K 棒或一筆 tick，繪製的元素會隨之變化；參見[K 棒轉換](series.md#bar-transforms)。

### 批次附加

要在尾端加入多根 K 棒（例如重新連線後資料源送來的一批），請以 `setData` 接受的任一形式（欄或列）呼叫 `append`。時間必須晚於最後一根 K 棒，且只有在系列本身有成交量欄時才必須提供成交量欄（反之則不可提供）；否則會擲出例外，資料維持原狀。

```ts
candles.append([
  { time: 1704067440000, open: 101.0, high: 101.4, low: 100.9, close: 101.3, volume: 250 },
  { time: 1704067500000, open: 101.3, high: 101.5, low: 101.1, close: 101.2, volume: 180 },
]);
```

形成中的 K 棒用 `update`，一起到達的已完成 K 棒用 `append`。

## 載入更早的歷史資料

`prepend` 會在第一根 K 棒之前插入更舊的 K 棒，接受 `setData` 的任一形式。既有 K 棒保留原本的邏輯索引；新 K 棒取得負索引（索引 0 之前的 K 棒是 -1）。視圖不會移動，系列上的指標會重新計算。時間必須早於第一根 K 棒；成交量規則與 `append` 相同。

圖表會告訴你何時該載入：當視圖左緣之前剩下的 K 棒少於一個畫面寬時，會傳送 `needMoreHistory` 事件。`barsBeforeStart` 是剩餘的 K 棒數（若資料之前的空白已經可見，則為負數）。每個第一根 K 棒只傳送一次：`prepend` 之後第一根 K 棒換成了新的，因此當使用者捲動到接近它時，事件會再次傳送。

### 完整範例

```ts
import { FerroChart, type OhlcRow } from '@ferrochart/core';

// Your backend: up to `count` bars that end before `beforeMs`, oldest first.
declare function fetchBars(symbol: string, beforeMs: number, count: number): Promise<OhlcRow[]>;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const latest = await fetchBars('BTCUSD', Date.now(), 1000);
candles.setData(latest);
let first = Number(latest[0]?.time ?? Date.now());
let loading = false;
let exhausted = false;

chart.on('needMoreHistory', async () => {
  if (loading || exhausted) return;
  loading = true;
  try {
    // Only bars before the first one: an overlapping bar would make prepend throw.
    const older = (await fetchBars('BTCUSD', first, 1000)).filter((b) => Number(b.time) < first);
    if (older.length === 0) {
      exhausted = true; // the backend has nothing older
      return;
    }
    candles.prepend(older);
    first = Number(older[0].time);
  } finally {
    loading = false;
  }
});
```

`loading` 旗標用來防止你自己的程式碼發出重疊的請求（事件本身對同一根第一根 K 棒不會重複）。在此期間，即時 K 棒照常透過 `update` 進來：`prepend` 與 `update` 作用在相反的兩端。

## 讀回資料

| 方法 | 傳回值 |
|---|---|
| `series.dataAt(index)` | OHLC 系列傳回 `{ open, high, low, close, volume }`（沒有成交量欄時 `volume` 為 `null`），數值系列傳回 `{ value }`；缺口或超出資料範圍時為 `null` |
| `chart.timeAtIndex(index)` | 某邏輯索引處 K 棒的時間（UTC ms），超出資料範圍時為 `null` |
| `chart.indexOfTime(time)` | 時間恰好等於該值的 K 棒之邏輯索引，不存在時為 `null` |
| `chart.dataRange()` | `{ from, to }`：第一根與最後一根 K 棒的邏輯索引；沒有資料時為 `null` |

小數索引會四捨五入到最近的 K 棒。

```ts
const range = chart.dataRange();
if (range) {
  const last = candles.dataAt(range.to);              // the forming bar
  const t = chart.timeAtIndex(range.to);              // its time
  console.log(t, last);
}

const i = chart.indexOfTime(Date.UTC(2024, 0, 2, 9, 30));
if (i !== null) console.log(candles.dataAt(i));
```

對於設有 `shift` 的系列，`dataAt(index)` 傳回繪製在 `index` 處的點。

## 邏輯索引

每根 K 棒都有一個邏輯索引。可見範圍、十字游標位置與點擊都以這些索引回報，而 `setVisibleLogicalRange` 與 `logicalToCoordinate` 接受的也是它們；參見[時間軸與視窗](time-scale.md)。所有系列共用時間軸，因此索引 `i` 在每個系列中都是同一個時間。

`setData` 之後，第一根 K 棒的索引為 0。在 K 棒之間的位置有意義之處（視圖邊緣、指標位置），索引可為小數：K 棒 `i` 涵蓋 `i - 0.5` 到 `i + 0.5`。

## 細節層級

K 棒以欄的形式儲存：一個時間陣列、一個開盤價陣列，依此類推。這正是載入快速、記憶體占用小的原因：每根 K 棒約 58 位元組，已包含時間索引與細節層級金字塔。

當你縮小到多根 K 棒共用一個像素時，FerroChart 會從預先計算的金字塔繪製彙總後的 K 棒（每一層數量減半，並保留真實的最高價與最低價）。因此無論資料量多大，每影格繪製的物件數都受螢幕寬度限制——約每個實體像素四個。

所有資料變更在內部都是增量的：渲染器會得知哪個範圍改變了，並只上傳那部分。

## 成交量

成交量系列讀取 OHLC 系列的成交量欄：`chart.addVolumeSeries(candles)`。它沒有自己的資料；請把 `volume` 傳給 K 線系列的 `setData` 與 `update`。參見[系列類型](series.md#volume)。

## 範例：模擬的即時資料源

一個帶有 500 根歷史 K 棒、每 250 ms 產生一個新價格的圖表。每當時鐘跨過分鐘邊界，就開始一根新的一分鐘 K 棒。

```ts
import { FerroChart, randomWalk, type Bar } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const interval = 60_000;
const now = Date.now();
const start = Math.floor(now / interval) * interval - 500 * interval;
const history = randomWalk(42, 500, start, interval); // deterministic demo data
candles.setData(history);

const n = history.close.length;
let bar: Bar = {
  time: Number(history.time[n - 1]),
  open: history.open[n - 1],
  high: history.high[n - 1],
  low: history.low[n - 1],
  close: history.close[n - 1],
  volume: history.volume?.[n - 1] ?? 0,
};

const timer = setInterval(() => {
  const t = Math.floor(Date.now() / interval) * interval;
  const price = bar.close * (1 + (Math.random() - 0.5) * 0.002);
  const size = Math.round(Math.random() * 10);
  if (t > Number(bar.time)) {
    // A new minute: open a new bar at the last close.
    bar = { time: t, open: bar.close, high: Math.max(bar.close, price), low: Math.min(bar.close, price), close: price, volume: size };
  } else {
    bar = { ...bar, high: Math.max(bar.high, price), low: Math.min(bar.low, price), close: price, volume: (bar.volume ?? 0) + size };
  }
  candles.update(bar); // same time: replaces, later time: appends
}, 250);

// When the chart goes away:
// clearInterval(timer); chart.destroy();
```

視圖會與最新 K 棒保持固定距離（右側偏移），因此無論使用者正在看最新資料還是已經往回捲動，每附加一根 K 棒，內容都會向左捲動一根。`chart.scrollToRealtime()` 會把最新 K 棒帶回視野中（參見[時間軸與視窗](time-scale.md#right-offset)）。
