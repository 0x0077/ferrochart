<!-- source: 7f51672705 -->
# 主題與樣式

本章說明圖表的外觀以及它如何融入頁面：主題、顏色、座標軸與留白、互動、十字游標、大量小圖、匯出圖片、署名標誌、開場動畫、GPU 渲染器、尺寸與清理。除非另有說明，這裡的內容同樣適用於 `FerroPlot`。

## 主題

主題在建立圖表時選定：

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' }); // 'light' (default) or 'dark'
```

主題決定背景、格線、文字、座標軸與十字游標的顏色、K 線預設的漲跌顏色，以及系列、指標與繪圖的預設顏色。

主題也可以是一個物件：在 `base` 主題（預設 `'light'`）之上覆寫的 CSS 顏色，未給出的鍵取自 `base`。用 `applyOptions` 可以變更既有圖表的主題，檢視與資料保持不變。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  theme: { base: 'dark', background: '#0b0d12', up: '#30a0f0', down: '#f09030', fontSize: 13 },
});
chart.applyOptions({ theme: 'light' });
```

可用的鍵有 `background`、`text`、`grid`、`axisLine`、`up`、`down`、`upWick`、`downWick`、`volumeUp`、`volumeDown`、`crosshair`、`crosshairLabelBg`、`crosshairLabelText`、`line`、`fontSize` 與 `axisPadding`；未知的鍵會產生錯誤。`chart.options().theme` 會讀回目前的主題。

任何未設定或設為 `null` 的顏色都會跟隨主題。只在需要的地方明確設定顏色；其餘部分在兩種主題下都能保持一致。

## 系列顏色

系列的顏色來自 `SeriesOptions`，可在建立時設定，或之後以 `series.applyOptions` 設定：

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const candles = chart.addCandlestickSeries({ color: '#26a69a', downColor: '#ef5350' });
const line = chart.addSeries('line', { color: '#2962ff', lineWidth: 2, lineStyle: 'dashed' });
const area = chart.addSeries('area', {
  color: '#2962ff',
  topFill: 'rgba(41, 98, 255, 0.4)',
  bottomFill: 'rgba(41, 98, 255, 0)',
});

line.applyOptions({ color: null }); // back to the theme's color
```

| 選項 | 用途 |
|---|---|
| `color` | 折線、面積與基準線（基準以上）系列的線條顏色；上漲的 K 線與棒；直方圖柱 |
| `downColor` | 下跌的 K 線與棒；基準線系列在基準以下的線條 |
| `topFill` | 面積圖頂端的填色；基準線在基準以上的填色 |
| `bottomFill` | 面積圖底端的填色；基準線在基準以下的填色 |
| `lineWidth` | 線寬，單位為 CSS px |
| `lineStyle` | `'solid'`、`'dashed'` 或 `'dotted'` |

面積填色是從 `topFill` 到 `bottomFill` 的漸層。直方圖系列也接受逐柱顏色（`setData` 的 `color` 欄）。所有系列選項以及各種系列繪製的內容，請參見[系列](series.md)。

其他物件有各自的顏色選項：價格線（`createPriceLine({ color })`）、標記（每個標記的 `color`）、兩個系列之間的通道帶（`addBand(upper, lower, { fill, fillCrossed })`）以及繪圖（`style: { line, fill, text }`）。參見[系列](series.md)、[標記](markers.md)、[指標](indicators.md)與[繪圖](drawings.md)。

## 顏色

顏色是下列其中一種形式的 CSS 顏色字串（型別 `Color`）：

| 形式 | 範例 |
|---|---|
| `#rgb`, `#rgba` | `'#f80'`, `'#f808'` |
| `#rrggbb`, `#rrggbbaa` | `'#ff8800'`, `'#ff880080'` |
| `rgb(r, g, b)` | `'rgb(255, 136, 0)'` |
| `rgba(r, g, b, a)` | `'rgba(255, 136, 0, 0.5)'`，alpha 介於 0 到 1 |
| `transparent` | `'transparent'` |

各通道為 0 到 255；以逗號、空格或 `/` 分隔。具名顏色（`'red'`）、`hsl()` 與百分比**不**被接受：呼叫會擲出例外，例如 `'color': invalid color 'red'`。

從 `options()` 讀回的顏色會寫成 `#rrggbb`，不透明度未滿時則寫成 `rgba(r, g, b, a)`；跟隨主題的顏色讀回為 `null`。

逐點顏色欄（直方圖資料、plot 標記）也接受 `0xRRGGBBAA` 值的 `Uint32Array`，處理大量資料時可避免建立字串。

## 座標軸、格線與留白

App 與表格中的圖表常常完全沒有座標軸。`priceAxis` 與 `timeAxis` 顯示或隱藏座標軸，`grid` 控制格線（`true`、`false` 或 `{ horizontal, vertical }`），`padding` 在繪圖區周圍留出空白（CSS px，一個數字或 `{ top, right, bottom, left }`）。隱藏的座標軸會把空間讓給繪圖區。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  priceAxis: false,
  grid: { vertical: false },
  padding: { top: 72, bottom: 40 },
});
chart.applyOptions({ timeAxis: false });
```

這些選項以及接下來兩節的選項都能用在 `create` 與 `applyOptions` 中，`chart.options()` 會讀回它們。在 Python 中它們是 snake_case 的關鍵字引數（`Chart(preset="sparkline")`、`chart.apply_options(price_axis=False)`）；在 C 中是 `FerroChartAppearance`、`FerroChartInteraction`、`FerroChartCrosshairStyle` 與 `FerroChartTheme` 結構。

## 互動

`interaction` 開關平移、縮放與十字游標：`{ pan, zoom, crosshair }`，或用 `false` 全部關閉。被關閉的輸入圖表不會使用，因此在觸控螢幕上頁面會照常捲動與縮放。點擊仍會回報。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { interaction: { pan: false, zoom: false } });
```

## 十字游標樣式

`crosshairStyle` 決定十字游標要畫哪些部分：`vertical` 與 `horizontal` 兩條線、座標軸上的 `labels`，以及第一個系列數值處的 `dot`（`{ radius, color }`，`null` 表示不畫；顏色預設為該系列的顏色）。一條垂直線加一個圓點是行情卡片常見的十字游標。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  crosshairStyle: { horizontal: false, labels: false, dot: { radius: 5 } },
});
```

## 大量小圖

`preset: 'sparkline'` 是放在表格儲存格中的圖表：沒有座標軸、沒有格線、留白 2、不能互動，十字游標是一條垂直線加一個圓點。與它一起給出的選項會疊加在上面。

瀏覽器每個頁面只保留約 16 個 WebGL 情境，而每個圖表都有自己的 GPU 裝置。頁面上有大量小圖時，請從池中建立它們：它們共用一個裝置（在 WebGL2 上共用一個情境）。視埠之外的圖表不會繪製，捲動回來時再畫。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvases: HTMLCanvasElement[];

const pool = await FerroChart.createPool();
const charts = await Promise.all(canvases.map((c) => pool.create(c, { preset: 'sparkline' })));
// …
pool.destroy(); // destroys every chart of the pool
```

在 WebGL2 上，池中的圖表以 2D 情境繪製自己的畫布，因此畫布不能已有其他情境。

## 匯出圖片

`chart.toBlob(type?, quality?)` 繪製一幀，回傳 PNG（預設）、WebP 或 JPEG 的 `Blob`；`chart.toImageBitmap()` 回傳 `ImageBitmap`。圖片的像素尺寸與畫布相同，不含署名標誌（它不在畫布中）。`FerroPlot` 有相同的方法。

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const png = await chart.toBlob();
const jpeg = await chart.toBlob('image/jpeg', 0.9);
```

## 署名標誌

預設情況下，最下方窗格的左下角、時間軸正上方會顯示一個小小的 FerroChart 標誌。它連結到 ferrochart.com，指標移到上方時會顯示名稱。

此標誌只是一種禮貌性的署名，並非授權要求：你可以把它關閉。未設定 `attributionLogo` 時，只有畫布不小於 320 × 160 CSS px 才會顯示標誌，畫布尺寸跨過這個界線時會隨之出現或消失；設為 `true` 或 `false`（或呼叫 `setAttributionLogo`）後就固定不變。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { attributionLogo: false });

chart.setAttributionLogo(true);  // show it
chart.setAttributionLogo(false); // remove it
```

它的放置方式：

- 它是一個插入在畫布正後方的 `<a>` 元素，以絕對定位緊貼畫布；它不會繪製到畫布中，因此不會出現在螢幕截圖或引擎渲染的影像裡。
- 版面變更時會更新其位置。它與畫布本身一樣，相對於畫布的 offset parent 定位。
- 若加入標誌時畫布沒有父元素，就不會加入標誌。

`FerroPlot` 也有相同的選項與方法；其標誌位於繪圖區的左下角。以 `attachSidePlot` 附加到圖表上的 plot 不會顯示自己的標誌，因為圖表的標誌已代表兩者（參見[Plots](plots.md)）。

## 開場動畫

`playIntro` 會先顯示空白的圖表（格線與座標軸），再由左至右繪出資料。可在圖表首次出現時使用，例如在登陸頁面上，或使用者開啟某個商品時。

```ts
import { FerroChart, randomWalk } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' });
chart.addCandlestickSeries().setData(randomWalk(1, 300, Date.UTC(2024, 0, 1), 3_600_000));

await chart.playIntro({ duration: 1200, delay: 200, pen: '#4cc9f0' });
// The animation has ended (or was ended early).
```

`IntroOptions`：

| 選項 | 意義 | 預設值 |
|---|---|---|
| `duration` | 由左至右繪出資料所需時間，單位 ms | `900` |
| `delay` | 先顯示空白圖表的時間，單位 ms | `0` |
| `pen` | 繪製邊緣發光線條的顏色；`null` 表示不顯示 | `null` |

行為：

- 圖表會立即（在下一影格之前）以空白狀態顯示，因此請在 `setData` 之後立刻呼叫 `playIntro`，以免完整圖表一閃而過。
- 價格線、數值標籤與十字游標會在最後出現。
- 在畫布上按下、滾動滾輪、雙指縮放、雙擊或按鍵，都會提前結束動畫。在圖表上移動指標則不會。
- `chart.finishIntro()` 會立即結束動畫。
- 再次呼叫 `playIntro` 會先結束正在執行的動畫。
- 傳回的 promise 會在動畫結束時 resolve，無論以何種方式結束（包括圖表被銷毀時）。它永遠不會 reject；無效的選項（負數或非有限的 `duration` 或 `delay`、無效的 `pen` 顏色）會直接由呼叫本身擲出例外。

對於 `FerroPlot`，`playIntro` 會由左至右繪出標記；規則線標籤與懸停醒目提示會在最後出現。

## 渲染器

FerroChart 使用 GPU 繪製：瀏覽器支援時使用 WebGPU，否則使用 WebGL2。沒有 Canvas 2D 備援。

| `renderer` 選項 | 行為 |
|---|---|
| `'auto'`（預設） | 先 WebGPU，再 WebGL2 |
| `'webgpu'` | 僅 WebGPU |
| `'webgl2'` | 僅 WebGL2 |

當所要求的 API 都無法使用（`'WebGPU is not
available'`、`'WebGL2 is not available'`，或兩者以 `;` 連接），或選項值未知時，`create` 會 reject。`renderer` 屬性會告訴你正在使用哪個 API：

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

try {
  const chart = await FerroChart.create(canvas);
  console.log(chart.renderer); // 'webgpu' or 'webgl2'
} catch (e) {
  canvas.replaceWith(Object.assign(document.createElement('p'), {
    textContent: 'This chart needs WebGPU or WebGL2.',
  }));
}
```

強制使用 `'webgl2'`，是在支援 WebGPU 的瀏覽器上測試備援的簡單方法。

## 設定畫布尺寸

畫布的尺寸以 CSS 設定。FerroChart 會自行把畫布的後備儲存區（`canvas.width` 與 `canvas.height`）設為 CSS 尺寸乘以 `devicePixelRatio`，讓圖表在高密度螢幕上保持清晰。請不要自行設定畫布的 `width` 與 `height`。

```html
<div style="height: 60vh">
  <canvas id="chart" style="display: block; width: 100%; height: 100%"></canvas>
</div>
```

- `ResizeObserver` 會監看畫布的內容方塊。當其 CSS 尺寸改變時，後備儲存區與版面會隨之調整，圖表也會重繪。你永遠不需要呼叫調整大小的方法。
- 當 `devicePixelRatio` 改變（瀏覽器縮放、把視窗移到另一個螢幕）時，後備儲存區也會調整大小。
- 後備儲存區至少為 1 × 1 px。若建立圖表時它位於隱藏的元素中，一旦該元素取得尺寸，圖表就會開始繪製。
- `display: block` 可避免行內畫布下方多出的幾個像素空白。
- 在 flex 或 grid 版面中，請為畫布設定 `min-height: 0`（或 `min-width: 0`），讓容器能夠縮小它。

建立圖表時，畫布還會被設定 `tabindex="0"`（除非它已有 `tabindex`）以便接收按鍵，設定 `touch-action: none` 讓觸控手勢平移與縮放的是圖表而非頁面（設為 `touchScroll: 'page'` 時使用 `pan-y`，將垂直滑動留給頁面；參見[觸控螢幕](web.md#touch-screens)），並設定 `outline: none`。指標位於圖表上時會設定游標樣式。

## 清理

```ts
import { FerroChart, FerroPlot } from '@ferrochart/core';

declare const chart: FerroChart;
declare const plot: FerroPlot;

chart.destroy();
plot.destroy(); // FerroPlot's equivalent
```

`destroy()`：

- 移除畫布上的 DOM 事件監聽器，以及所有以 `on` 加入的回呼函式；
- 離開圖表的同步群組；
- 移除署名標誌；
- resolve 尚未完成的 `playIntro` promise；
- 釋放 GPU 資源。

畫布元素會留在頁面上；若不再需要，請自行移除。`destroy()` 之後，圖表物件便無法使用，對其 `Series`、`Indicator` 與其他控制代碼的呼叫都會擲出例外（`the FerroChart was
destroyed`）。不再顯示的圖表請將其銷毀：每個圖表都占用 GPU 記憶體，而且瀏覽器會限制單一頁面可擁有的 WebGL context 數量。

在 React 或 Vue 中，請在元件掛載時建立圖表，在卸載時銷毀；參見[React 與 Vue](frameworks.md)。
