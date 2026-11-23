<!-- source: 32f28d0f34 -->
# 指標

FerroChart 內建 30 種技術指標。指標讀取一個來源系列，計算出一個或多個輸出系列，並在來源變動時讓它們保持最新。輸出都是一般的系列：你可以像對待其他系列一樣設定它們的樣式、讀取數值，並在上面加上標記。

## 加入指標

```ts
const sma = chart.addIndicator(candles, 'sma', { n: 50 });
const boll = chart.addIndicator(candles, 'boll', { n: 20, k: 2 });
const macd = chart.addIndicator(candles, 'macd', { fast: 12, slow: 26, signal: 9 });
const rsi = chart.addIndicator(candles, 'rsi');        // all defaults
```

`chart.addIndicator(source, name, params?, placement?)` 回傳一個 `Indicator`。

| 參數 | 意義 |
|---|---|
| `source` | 用來計算的系列。成交量系列與自訂系列以外的任何系列皆可。 |
| `name` | 下表中的名稱之一，例如 `'sma'`、`'stochRsi'`。也接受 `snake_case` 寫法（`'stoch_rsi'`、`'volume_ma'`）。 |
| `params` | 以名稱指定的參數，使用 camelCase。未指定的參數採用預設值。 |
| `placement` | 輸出放置的位置；預設為 `'default'`。 |

指標會在 `addIndicator` 返回之前，一次對所有既有資料完成計算。

### 放置位置

```ts
type Placement = 'default' | 'overlay' | 'newPane' | 'volume' | number;
```

| 值 | 輸出放置於 |
|---|---|
| `'default'` | 指標的慣常位置：移動平均線與通道為 `'overlay'`，震盪指標為 `'newPane'`，成交量均線為 `'volume'`（見下表） |
| `'overlay'` | 來源所在的窗格，使用其價格軸 |
| `'newPane'` | 在其他窗格下方，為此指標新建的窗格 |
| `'volume'` | 來源的成交量系列所在的窗格，使用成交量座標軸。來源沒有成交量系列時，則為新窗格 |
| 窗格 id | 該窗格（取自 `chart.panes()`、`chart.mainPane`、`chart.addPane()` 或 `series.pane`） |

```ts
// Two oscillators in one pane.
const rsi = chart.addIndicator(candles, 'rsi');
const pane = rsi.outputs()[0].pane;
chart.addIndicator(candles, 'stochRsi', {}, pane);
```

由 `'newPane'`（或在沒有成交量系列時由 `'volume'`）建立的窗格，若上面已沒有其他內容，會隨指標一起移除。

### 來源系列

OHLC 系列（K 線、美國線、Heikin-Ashi）會提供指標開盤、最高、最低、收盤與成交量。單值系列（line、area、baseline、histogram、points）則提供 `open = high = low = close = value`，且沒有成交量。因此你可以計算折線系列的 RSI，但不能計算 OBV。

需要成交量的指標（VWMA、成交量均線、VWAP、MFI、OBV、CMF）在來源為單值系列，或來源 OHLC 系列的資料沒有成交量欄時，會拋出錯誤。

來源沒有數值的 K 線（例如只有其他系列才有的時間點）會被略過：該處的輸出為 `NaN`，且該 K 線不計入指標的視窗。

### 指標的指標

來源可以是另一個指標的輸出。例如在 RSI 的窗格中畫出 RSI 的 9 根 SMA：

```ts
const rsiLine = chart.addIndicator(candles, 'rsi').outputs()[0];
const rsiAvg = chart.addIndicator(rsiLine, 'sma', { n: 9 }, rsiLine.pane);
```

移除 RSI 時也會移除該 SMA。

## 內建指標

`indicatorNames()` 依本表的順序回傳 30 個名稱。輸出依 `outputs()` 回傳的順序列出。除非標示為（直方圖）或（點），所有輸出都是折線。

| 名稱 | 指標 | 參數（預設值） | 輸出 | 預設位置 |
|---|---|---|---|---|
| `sma` | 簡單移動平均線 | `n`（20）、`source` | sma | 疊加 |
| `ema` | 指數移動平均線 | `n`（20）、`source` | ema | 疊加 |
| `wma` | 加權移動平均線 | `n`（20）、`source` | wma | 疊加 |
| `hma` | 赫爾移動平均線 | `n`（20，至少 2）、`source` | hma | 疊加 |
| `vwma` | 成交量加權移動平均線 | `n`（20）、`source` | vwma | 疊加 |
| `dema` | 雙重 EMA | `n`（20）、`source` | dema | 疊加 |
| `tema` | 三重 EMA | `n`（20）、`source` | tema | 疊加 |
| `volumeMa` | 成交量移動平均線 | `n`（20） | vma | 成交量 |
| `boll` | 布林通道 | `n`（20）、`k`（2）、`source` | upper、mid、lower；帶狀區域 upper–lower | 疊加 |
| `keltner` | 肯特納通道 | `n`（20）、`m`（2）、`atrN`（10） | upper、mid、lower；帶狀區域 upper–lower | 疊加 |
| `donchian` | 唐奇安通道 | `n`（20） | upper、mid、lower | 疊加 |
| `ichimoku` | 一目均衡表 | `tenkan`（9）、`kijun`（26）、`senkou`（52）、`shift`（26） | 轉換線、基準線、先行帶 A、先行帶 B、遲行線；帶狀區域 先行帶 A–先行帶 B | 疊加 |
| `sar` | 拋物線 SAR | `afStart`（0.02）、`afStep`（0.02）、`afMax`（0.2） | sar（點） | 疊加 |
| `supertrend` | 超級趨勢（SuperTrend） | `atrN`（10）、`m`（3） | long、short | 疊加 |
| `vwap` | 成交量加權平均價 | `anchor`（`'day'`）、`utcOffsetMs`（0） | vwap | 疊加 |
| `macd` | MACD | `fast`（12）、`slow`（26）、`signal`（9）、`source` | macd、signal、hist（直方圖） | 新窗格 |
| `rsi` | 相對強弱指標 | `n`（14）、`source` | rsi | 新窗格 |
| `stoch` | 隨機指標 | `k`（14）、`smoothK`（3）、`d`（3） | %K、%D | 新窗格 |
| `stochRsi` | 隨機 RSI | `rsiN`（14）、`stochN`（14）、`smoothK`（3）、`d`（3）、`source` | %K、%D | 新窗格 |
| `cci` | 順勢指標 | `n`（20） | cci | 新窗格 |
| `willr` | 威廉指標 %R | `n`（14） | %R | 新窗格 |
| `mfi` | 資金流量指標 | `n`（14） | mfi | 新窗格 |
| `adx` | ADX 與趨向指標 | `n`（14） | adx、+DI、−DI | 新窗格 |
| `atr` | 平均真實區間 | `n`（14） | atr | 新窗格 |
| `obv` | 能量潮 | 無 | obv | 新窗格 |
| `cmf` | 蔡金資金流量 | `n`（20） | cmf | 新窗格 |
| `roc` | 變動率（%） | `n`（10）、`source` | roc | 新窗格 |
| `momentum` | 動量 | `n`（10）、`source` | momentum | 新窗格 |
| `trix` | TRIX | `n`（15）、`source` | trix | 新窗格 |
| `aroon` | 阿隆指標 | `n`（14） | up、down | 新窗格 |

### 參數值

| 參數 | 值 |
|---|---|
| 週期（`n`、`fast`、`slow`、`signal`、`stoch` 的 `k`、`smoothK`、`d`、`rsiN`、`stochN`、`atrN`、`tenkan`、`kijun`、`senkou`、`shift`） | 整數，至少為 1（`hma`：`n` 至少為 2） |
| 乘數（`boll` 的 `k`、`m`） | 大於 0 的有限數 |
| `afStart`、`afStep`、`afMax` | 大於 0 的有限數，且 `afStart` 不大於 `afMax` |
| `source` | `'close'`（預設）、`'open'`、`'high'`、`'low'`、`'hl2'` (H+L)/2、`'hlc3'` (H+L+C)/3、`'ohlc4'` (O+H+L+C)/4 |
| `anchor`（`vwap`） | `'day'`（預設）、`'week'`（每週從星期一開始）、`'month'`，或開始累計的時間（UTC ms） |
| `utcOffsetMs`（`vwap`） | 在切分日、週、月之前加到 UTC 上的毫秒整數，例如紐約標準時間為 `-5 * 3600_000` |

```ts
chart.addIndicator(candles, 'ema', { n: 21, source: 'hlc3' });
chart.addIndicator(candles, 'vwap', { anchor: 'week' });
chart.addIndicator(candles, 'vwap', { anchor: Date.UTC(2024, 2, 1) }); // anchored VWAP
chart.addIndicator(candles, 'vwap', { utcOffsetMs: 9 * 3600_000 });   // sessions in UTC+9
```

參數名稱必須與表中的寫法完全一致。指標不使用的鍵，包括 `atr_n` 這類 snake_case 寫法，都會拋出錯誤，並列出可接受的鍵：

```ts
chart.addIndicator(candles, 'sma', { length: 20 });
// Error: unknown parameter 'length' for indicator 'sma' (expected: n, source)
```

`addTransformSeries` 也以相同方式檢查參數。

### 公式

簡要說明；`P` 為所選 `source` 的價格。「暖機期」是指開頭輸出為 `NaN` 的 K 線數量。

**移動平均線與通道**

- **SMA**：`P` 最近 `n` 個值的平均。暖機期 `n − 1`。
- **EMA**：`e = e + α(P − e)`，`α = 2/(n + 1)`，以前 `n` 個值的 SMA 作為初始值。暖機期 `n − 1`。
- **WMA**：從最新值起權重依序為 `n, n−1, …, 1`。暖機期 `n − 1`。
- **HMA**：`WMA(2·WMA(P, n/2) − WMA(P, n), √n)`（減半與開根號皆無條件捨去）。暖機期 `n + ⌊√n⌋ − 2`。
- **VWMA**：`n` 根 K 線的 `Σ(P·V) / ΣV`（成交量總和為 0 時為 `NaN`）。暖機期 `n − 1`。
- **DEMA**：`2·e1 − e2`，其中 `e1 = EMA(P, n)`、`e2 = EMA(e1, n)`。暖機期 `2(n − 1)`。
- **TEMA**：`3·e1 − 3·e2 + e3`，`e3 = EMA(e2, n)`。暖機期 `3(n − 1)`。
- **成交量均線**：成交量的 SMA。暖機期 `n − 1`。
- **BOLL**：中軌 = `SMA(P, n)`，上軌與下軌 = 中軌 ± `k` × `n` 根 K 線的母體標準差。暖機期 `n − 1`。
- **Keltner**：中軌 = `EMA(close, n)`，上軌與下軌 = 中軌 ± `m` × `ATR(atrN)`。暖機期 `max(n − 1, atrN)`。
- **Donchian**：上軌 = `n` 根 K 線的最高價，下軌 = 最低價，中軌 = 兩者平均。暖機期 `n − 1`。
- **Ichimoku**：轉換線與基準線分別是 `tenkan` 與 `kijun` 根 K 線最高最低區間的中點。先行帶 A = (轉換線 + 基準線)/2，先行帶 B = `senkou` 根 K 線區間的中點，兩者皆往前繪製 `shift` 根 K 線（最後 `shift` 個值出現在最後一根 K 線的右側）。遲行線是往回繪製 `shift` 根 K 線的收盤價。
- **SAR**：拋物線 SAR，加速因子從 `afStart` 開始，每次增加 `afStep`，最大到 `afMax`，計算方式與 TA-Lib 相同。暖機期 1。
- **SuperTrend**：帶狀區間 `HL2 ± m·ATR(atrN)`，在趨勢持續期間只會收緊；收盤價穿越作用中的帶時趨勢反轉。`long`（下軌）在上升趨勢中有值，`short`（上軌）在下降趨勢中有值，其餘為 `NaN`。暖機期 `atrN`。
- **VWAP**：從錨點起算的 `Σ(HLC3·V) / ΣV`，每天、每週或每月（以 UTC 加上 `utcOffsetMs`）重新起算，或從指定時間起算（在此之前為 `NaN`）。暖機期 0。

**震盪指標**

- **MACD**：macd = `EMA(P, fast) − EMA(P, slow)`，signal = `EMA(macd, signal)`，hist = macd − signal。macd 的暖機期為 `slow − 1`，signal 與 hist 為 `slow + signal − 2`。
- **RSI**：對漲幅與跌幅做 Wilder 平滑（`r = r + (x − r)/n`）；`100·gain/(gain + loss)`，兩者皆為 0 時為 50。暖機期 `n`。
- **Stoch**：raw = `k` 根 K 線的 `100·(C − lowest low)/(highest high − lowest low)`（區間為 0 時為 50）；%K = `SMA(raw, smoothK)`，%D = `SMA(%K, d)`。%K 的暖機期為 `k + smoothK − 2`，%D 再多 `d − 1`。
- **StochRSI**：將隨機指標公式套用於 `stochN` 根 K 線的 `RSI(P, rsiN)`，再像 Stoch 一樣平滑。%K 的暖機期為 `rsiN + stochN + smoothK − 2`，%D 再多 `d − 1`。
- **CCI**：`(TP − SMA(TP, n)) / (0.015 · mean deviation)`，`TP` = HLC3；平均偏差為 0 時為 0。暖機期 `n − 1`。
- **Williams %R**：`n` 根 K 線的 `−100·(highest high − C)/(highest high − lowest low)`；區間為 0 時為 −50。暖機期 `n − 1`。
- **MFI**：`n` 根 K 線的 `100·positive flow/(positive + negative flow)`，其中 flow = HLC3 × 成交量，其正負號依 HLC3 的變化而定；兩者皆為 0 時為 50。暖機期 `n`。
- **ADX**：+DI 與 −DI 由經 Wilder 平滑的趨向變動與真實區間計算；adx = `100·|+DI − −DI|/(+DI + −DI)` 的 Wilder 平均。DI 的暖機期為 `n`，adx 為 `2n − 1`。
- **ATR**：真實區間的 Wilder 平均。暖機期 `n`。
- **OBV**：成交量的累計值，收盤上漲時加上、收盤下跌時減去。暖機期 0。
- **CMF**：`n` 根 K 線的 `Σ(MFM·V)/ΣV`，`MFM = ((C − L) − (H − C))/(H − L)`。暖機期 `n − 1`。
- **ROC**：`100·(P / P[n bars ago] − 1)`。暖機期 `n`。
- **Momentum**：`P − P[n bars ago]`。暖機期 `n`。
- **TRIX**：`EMA(EMA(EMA(P, n), n), n)` 相對前一根 K 線的百分比變化。暖機期 `3(n − 1) + 1`。
- **Aroon**：在最近 `n + 1` 根 K 線中，`100·(n − bars since the highest high)/n`（up），最低價同理（down）。暖機期 `n`。

## 設定輸出樣式

`indicator.outputs()` 回傳輸出系列。和任何系列一樣以 `applyOptions` 設定樣式（請見[系列](series.md)）：

```ts
const [macdLine, signalLine, hist] = macd.outputs();
macdLine.applyOptions({ color: '#2962ff', lineWidth: 2 });
signalLine.applyOptions({ color: '#ff9800', lineStyle: 'dashed' });
hist.applyOptions({ color: '#787b86' });

const [upper, mid, lower] = boll.outputs();
mid.applyOptions({ lineStyle: 'dotted' });
```

輸出建立時的線寬為 1 CSS px（點為 3，即點的直徑），每個輸出有固定的顏色，`priceLineVisible: false`，並顯示最新值標籤。每次呼叫 `outputs()` 都會回傳指向相同系列的新 handle；請以 `id` 比較它們。

輸出無法單獨移除：對它呼叫 `series.remove()` 會拋出錯誤。請改為移除指標。

### 帶狀區域

BOLL 與 Keltner 會填滿上軌與下軌之間的區域；Ichimoku 會填滿先行帶 A 與先行帶 B 之間的雲帶（A 在 B 之上時為綠色，之下時為紅色）。`indicator.bands()` 回傳這些填色的 id；以 `chart.setBandOptions` 修改其顏色：

```ts
const [band] = boll.bands();
chart.setBandOptions(band, { fill: 'rgba(41, 98, 255, 0.08)' });

const ichimoku = chart.addIndicator(candles, 'ichimoku');
const [cloud] = ichimoku.bands();
chart.setBandOptions(cloud, { fill: 'rgba(38, 166, 154, 0.25)', fillCrossed: 'rgba(239, 83, 80, 0.25)' });
console.log(chart.bandOptions(cloud)); // { fill, fillCrossed }
```

| 帶狀區域選項 | 意義 |
|---|---|
| `fill` | 上方輸出位於下方輸出之上或相等時 |
| `fillCrossed` | 位於其下時 |

`null` 會還原預設值：對 BOLL 與 Keltner 而言，是上軌顏色、不透明度 15%（`fillCrossed`：與 `fill` 相同）。傳入 `'transparent'` 可隱藏填色。其他指標沒有帶狀區域；`bands()` 回傳空陣列。

## 移除

```ts
macd.remove();
```

`remove()` 會移除指標、其輸出系列、這些系列的價格線與標記、其帶狀區域、以其輸出計算的所有指標，以及它所建立且現在已空的窗格。之後對該物件呼叫 `outputs()`、`bands()` 與 `remove()` 都會拋出錯誤。

移除來源系列會一併移除其指標。

## 修改參數

參數無法就地修改。請移除指標再重新加入。若要保留自訂樣式，請先讀取它們：

```ts
import type { FerroChart, Indicator, Series, Placement } from '@ferrochart/core';

function replaceIndicator(
  chart: FerroChart,
  old: Indicator,
  source: Series,
  name: string,
  params: Record<string, number | string>,
  placement?: Placement,
): Indicator {
  // `shift` is set by the indicator (Ichimoku), so it is not copied.
  const styles = old.outputs().map((s) => {
    const { shift, ...style } = s.options();
    return style;
  });
  old.remove();
  const next = chart.addIndicator(source, name, params, placement);
  next.outputs().forEach((s, i) => {
    if (styles[i]) s.applyOptions(styles[i]);
  });
  return next;
}

let sma = chart.addIndicator(candles, 'sma', { n: 20 });
sma = replaceIndicator(chart, sma, candles, 'sma', { n: 50 });
```

移除震盪指標也會移除它所建立的窗格（若上面沒有其他內容），因此替換後的指標會在底部獲得新窗格。若要讓震盪指標保持在固定位置，請自行以 `chart.addPane()` 建立窗格，並將其 id 作為放置位置傳入：你自己建立的窗格不會隨指標移除。

## 錯誤

在下列情況中，`addIndicator` 會拋出 `Error` 且不做任何變更：

| 問題 | 訊息（範例） |
|---|---|
| 未知的名稱 | `unknown indicator 'smaa'` |
| 週期不是 ≥ 0 的整數 | `'n' must be a non-negative integer` |
| 週期為 0 | `invalid argument: periods must be at least 1` |
| `hma` 的 `n` 小於 2 | `invalid argument: the period is too short` |
| 乘數 ≤ 0 或不是有限數 | `invalid argument: multipliers must be finite and > 0` |
| `afStart` 大於 `afMax` | `invalid argument: af_start must not exceed af_max` |
| 應為數字之處不是數字 | `'k' must be a finite number` |
| 未知的 `source` | `unknown source 'median'` |
| 無效的 VWAP `anchor` | `'anchor' must be 'day', 'week', 'month' or a time in ms` |
| `utcOffsetMs` 不是整數 | `'utcOffsetMs' must be an integer` |
| 無效的 `placement` | `placement must be 'default', 'overlay', 'newPane', 'volume' or a pane id` |
| 未知的窗格 id | `unknown id` |
| 以成交量系列作為來源 | `invalid argument: volume series have no data of their own; use the source series` |
| 在沒有成交量的來源上使用成交量指標 | `invalid argument: the indicator needs volume` |

```ts
try {
  chart.addIndicator(candles, 'rsi', { n: 0 });
} catch (e) {
  console.error((e as Error).message); // invalid argument: periods must be at least 1
}
```

## 即時更新

指標會自行跟隨其來源：

- 以最後一根 K 線的時間呼叫 `series.update(bar)` 會更新每個輸出的最後一個值；以更晚的時間呼叫則會附加一個值。只計算新增或變動的 K 線，而非整個系列。
- `series.setData(...)` 會以新資料重新計算指標。

增量更新的結果與對整個系列重新計算相同。輸出會與來源在同一幀中變動。

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];

feed.onTick((bar) => {
  candles.update(bar);
  const last = chart.dataRange();
  if (last) console.log('RSI', rsi.dataAt(last.to)?.value);
});
```

## 列出名稱

```ts
import init, { indicatorNames } from '@ferrochart/core';

await init(); // not needed once a chart has been created
console.log(indicatorNames()); // ['sma', 'ema', ..., 'aroon']
```
