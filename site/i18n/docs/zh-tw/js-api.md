<!-- source: 29952e0395 -->
# JavaScript API 概覽

套件 `@ferrochart/core`。本頁說明如何載入模組，以及 API 各部分共同遵循的慣例。完整參考文件（所有類別、方法與型別）由套件的 TypeScript 宣告自動產生：[JavaScript API 參考](https://ferrochart.com/docs/api)。

## 載入

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
```

`FerroChart.create` 與 `FerroPlot.create` 在第一次被呼叫時載入 WebAssembly 模組；同時進行的呼叫會共用同一次載入。若你需要在不先建立圖表的情況下使用模組（`randomWalk`、`new SyncGroup()`、`indicatorNames()`），或要從其他 URL 載入 `.wasm` 檔案，請自行呼叫預設匯出的 `init`：

```ts
import init, { SyncGroup } from '@ferrochart/core';

await init();                                     // next to the module (default)
await init({ module_or_path: '/assets/ferrochart_wasm_bg.wasm' });
const group = new SyncGroup();
```

`init` 可以呼叫任意次數；第一次載入完成後，它會立即傳回。

### TypeScript

已內含型別宣告。宣告中使用了 `Symbol.dispose`，因此請在 `compilerOptions.lib` 中加入 `"ESNext.Disposable"`，或沿用常見的 `skipLibCheck: true`。

### 打包工具與框架

此套件是 ES 模組，會相對於自身 URL 取得其 `.wasm` 檔案（`new URL('…_bg.wasm', import.meta.url)`），Vite、webpack 5、Rollup 與 esbuild 都能理解這種寫法。請只在瀏覽器中建立圖表：在 React 中使用 effect，在 Vue 中使用 `onMounted`，並讓該元件略過伺服器端渲染（參見[React 與 Vue](frameworks.md)）。

## 類別

| 類別 | 建立方式 | 說明 |
|---|---|---|
| [`FerroChart`](https://ferrochart.com/docs/api/FerroChart) | `FerroChart.create(canvas, options?)` | `<canvas>` 上的金融圖表：窗格、系列、刻度、十字游標、繪圖 |
| [`Series`](https://ferrochart.com/docs/api/Series) | `chart.addSeries`、`addCandlestickSeries`、`addVolumeSeries`、`addTransformSeries`、訂單流的 `add*` | 一個資料系列 |
| [`Indicator`](https://ferrochart.com/docs/api/Indicator) | `chart.addIndicator` | 一個指標及其輸出系列 |
| [`PriceLine`](https://ferrochart.com/docs/api/PriceLine) | `series.createPriceLine` | 位於某個價格的水平線 |
| [`Drawings`](https://ferrochart.com/docs/api/Drawings) | `chart.drawings` | 圖表的繪圖與繪圖工具 |
| [`SyncGroup`](https://ferrochart.com/docs/api/SyncGroup) | `new SyncGroup()` | 連結多個圖表的十字游標、可見範圍與繪圖 |
| [`FerroPlot`](https://ferrochart.com/docs/api/FerroPlot) | `FerroPlot.create(canvas, options?)` | 通用的 x/y 圖（數值、類別、日期） |
| [`Mark`](https://ferrochart.com/docs/api/Mark), [`Axis`](https://ferrochart.com/docs/api/Axis), [`Rule`](https://ferrochart.com/docs/api/Rule), [`Label`](https://ferrochart.com/docs/api/Label) | `FerroPlot` 的方法 | plot 的各個組成部分 |

## 慣例

### 時間

時間為 **UTC 毫秒**。欄資料（`setData` 中的 `time`）接受 `number[]`、`Float64Array` 或 `BigInt64Array`；其他地方的單一時間（列、`update`、標記、繪圖錨點、`indexOfTime`）可以是 `number`、`bigint` 或 `Date`。時間必須嚴格遞增。事件與取值方法以 `number` 傳回時間（超出資料範圍時為 `null`）。時間軸標籤以 UTC 顯示。

### 邏輯索引

K 棒以**邏輯索引**定址：初始資料的第一根 K 棒為 0，下一根為 1，依此類推。之後以 prepend 加入的 K 棒取得負索引，因此索引一旦給定就不會改變。可見範圍、十字游標與轉換方法（`coordinateToLogical`、`timeAtIndex`、`indexOfTime`）都使用邏輯索引。小數值表示 K 棒中心之間的位置。

### 座標

位置以距畫布左上角的 **CSS 像素**表示，與 DOM 事件相同。畫布尺寸由 CSS 決定；圖表會把後備儲存區設為 CSS 尺寸 × `devicePixelRatio`，並自行跟隨尺寸變化。

### 顏色

`Color` 是 CSS 顏色字串：`#rgb`、`#rrggbb`、`#rrggbbaa`、`rgb()`、`rgba()` 或 `'transparent'`。在選項中，`null` 表示使用主題的預設值。

### 選項物件

選項是純物件。`applyOptions` 及類似方法只會變更你傳入的鍵；未傳入的鍵保留原值。`series.options()` 等取值方法會傳回所有鍵（`Required<…>`）。

### 錯誤

方法會擲出 `Error`，訊息中會指明問題（未知的指標名稱、無效的參數、長度不同的欄、時間未遞增的 K 棒、未知的 id）。圖表保持不變。當所要求的 GPU API 無法使用時，`FerroChart.create` 會 reject：沒有 Canvas 2D 備援；`renderer: 'auto'` 會先嘗試 WebGPU，再嘗試 WebGL2。

### 事件

`chart.on(name, callback)` 訂閱，`chart.off(name, callback)` 取消訂閱。回呼參數的型別由事件名稱推導。事件在產生它的影格之後傳遞，而不是在輸入處理常式中。回呼函式擲出的例外會回報到主控台（透過 `reportError`），其他回呼函式仍會照常執行。參見[十字游標與事件](crosshair-events.md)。

### 生命週期與記憶體

圖表會監聽其畫布與 window，並自行重繪。只要它還在頁面上，就要保留對它的參照；移除時請呼叫 `chart.destroy()`：這會移除監聽器並釋放 GPU 記憶體。未經 `destroy` 就被丟棄的圖表可能會被垃圾回收而停止回應。

API 的每個物件都持有 WebAssembly 記憶體。圖表傳回的物件（`Series`、`Indicator`、`PriceLine`）會隨圖表一起釋放；`free()` 或 `using` 宣告可以提早釋放單一物件。移除系列（`remove()`）是另一回事：它會把系列從圖表上拿掉。

### 畫布處理的輸入

指標（滑鼠、觸控筆、觸控）、滾輪、鍵盤（畫布會取得 `tabindex="0"`）、雙擊、觸控板雙指縮放（Chrome 與 Firefox 中為 Ctrl+滾輪，Safari 中為 gesture 事件），以及由圖表自行辨識的雙指觸控。畫布樣式會設定 `touch-action: none`；設為 `touchScroll: 'page'` 時則使用 `pan-y`（參見[觸控螢幕](web.md#touch-screens)）。各手勢的作用請參見[手勢與鍵盤](interaction.md)。

## 測試資料

`randomWalk(seed, n, startMs, intervalMs)` 傳回確定性的 OHLC 欄，與 Rust 產生器 `ferrochart_core::testing::random_walk` 完全相同。範例與示範都使用它。

```ts
import init, { randomWalk } from '@ferrochart/core';

await init();
const bars = randomWalk(42, 10_000, Date.UTC(2024, 0, 1), 60_000);
```
