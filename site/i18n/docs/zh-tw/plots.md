<!-- source: 0b3cf203ce -->
# 繪圖（FerroPlot）

`FerroChart` 是圍繞時間序列 K 線設計的。其他所有用途，例如波動率微笑、報酬率分佈直方圖、產業別長條圖、損益瀑布圖或相關係數熱力圖，請使用 `FerroPlot`：一種通用的 x/y 繪圖，支援數值、對數、類別與日期座標軸。它與圖表使用相同的渲染器（WebGPU，其次 WebGL2）、相同的 canvas 處理方式與相同的主題。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const plot = await FerroPlot.create(canvas, { theme: 'dark' });

const smile = plot.addMark('line', { pointSize: 5 });
smile.setData({
  x: [80, 90, 100, 110, 120],          // strikes
  y: [0.31, 0.26, 0.24, 0.25, 0.29],   // implied volatility
});
plot.axis('right').applyOptions({ format: { type: 'percent', decimals: 0 }, title: 'IV' });
plot.axis('x').applyOptions({ title: 'Strike' });
```

一張繪圖由以下部分組成：

- **圖形（mark）**：折線、面積、散佈點、長條、瀑布與格子，各自擁有自己的資料與選項；
- 三條**座標軸**：位於底部的 `'x'`、`'right'`，以及 `'left'`（預設隱藏）；
- **參考線（rule）**：在某個數值處的水平線或垂直線，或帶底色的區帶；
- **標籤（label）**：位於資料座標上的文字或符號。

## 建立繪圖

`FerroPlot.create(canvas, options?)` 和 `FerroChart.create` 一樣回傳 promise，並在第一次使用時載入 WebAssembly 模組。canvas 的尺寸由 CSS 決定；請見[主題與樣式](styling.md#sizing-the-canvas)。

| 選項 | 值 | 預設值 |
|---|---|---|
| `theme` | `'light'`、`'dark'` | `'light'` |
| `renderer` | `'auto'`、`'webgpu'`、`'webgl2'` | `'auto'` |
| `crosshair` | 穿過滑鼠所指元素的十字游標線 | `false` |
| `hoverHighlight` | 醒目標示滑鼠所指的元素 | `true` |
| `attributionLogo` | 繪圖區左下角的 FerroChart 標誌 | `true` |
| `touchScroll` | `'chart'`、`'page'`：由誰處理垂直滑動；參見[觸控螢幕](web.md#touch-screens) | `'chart'` |

`FerroPlot` 的其他成員：

| 成員 | 說明 |
|---|---|
| `addMark(kind, options?): Mark` | 加入一個圖形，繪製在先前的圖形之上 |
| `axis(name): Axis` | `'x'`、`'left'` 或 `'right'` |
| `addRule(options): Rule` | 位於某個數值的線或區帶 |
| `addLabel(options): Label` | 位於某個點的文字或符號 |
| `fit()` | 讓每條座標軸回到其設定的範圍；自動座標軸重新跟隨資料 |
| `on(event, callback)`、`off(event, callback)` | 事件：`hover`、`click`、`viewChange` |
| `playIntro(options?)`、`finishIntro()` | 開場動畫，與圖表相同 |
| `setAttributionLogo(on)` | 顯示或移除標誌 |
| `renderer` | `'webgpu'` 或 `'webgl2'` |
| `destroy()` | 移除監聽器與標誌，釋放 GPU 資源（`dispose()` 是同一個方法，為其舊名稱） |

使用者可以平移與縮放繪圖：拖曳會平移；滾輪或雙指縮放會以指標為中心縮放 x 軸（在 y 軸上方或按住 Alt 時，則縮放該座標軸）；Shift+滾輪與水平捲動會平移；沿著座標軸拖曳會縮放該座標軸；雙擊則還原至完整範圍。類別座標軸可平移但不能縮放。

在觸控螢幕上，單指可平移，雙指則以中點為中心縮放；每個方向的縮放幅度取決於兩指在該方向上的距離變化（若兩指在某方向的距離小於 16 CSS px，該方向就不會縮放）。手指按在元素上會顯示其懸停狀態，抬起後仍會保留，除非手指曾平移圖表；手指對資料點的命中範圍比滑鼠多出 8 CSS px。

## 圖形

`plot.addMark(kind, options?)` 以 `MarkOptions` 加入一個 `MarkKind` 類型的圖形，並回傳一個 `Mark` handle。

| 類型 | 繪製方式 | 使用的資料 |
|---|---|---|
| `'line'` | 依資料順序連接的點 | `x`、`y` |
| `'area'` | 折線與 `y0`（或 `baseValue`）之間的區域 | `x`、`y`、`y0?` |
| `'scatter'` | 每個點一個符號 | `x`、`y`、`size?`、`value?`、`color?`、`label?` |
| `'bars'` | 從 `y0`（或 `baseValue`）延伸到 `y` 的長條 | `x`、`y`、`y0?`、`value?`、`color?`、`label?` |
| `'waterfall'` | 類別座標軸上的累計長條 | `x`、`y`、`step?` |
| `'cells'` | 網格中著色的格子（熱力圖） | `x`、`y`、`value` |

### 資料

`mark.setData(data)` 會取代資料。所有欄的長度相同；型別陣列以每欄一次 `memcpy` 的方式複製。

| 欄 | 型別 | 意義 |
|---|---|---|
| `x` | `Float64Array \| number[]` | 數字、UTC ms（日期座標軸）或類別編號。水平長條則為數值 |
| `y` | `Float64Array \| number[]` | 數值；`NaN` 代表缺口。水平長條則為在 y 軸上的位置 |
| `y0` | `Float64Array \| number[]` | 面積的下緣、長條的起點 |
| `value` | `Float64Array \| number[]` | 依色階著色的數值（cells、bars、scatter） |
| `size` | `Float32Array \| number[]` | 散佈點直徑，CSS px |
| `color` | `(string \| null)[] \| Uint32Array` | 逐點顏色（`null`：圖形顏色），或 `0xRRGGBBAA` |
| `step` | `('delta' \| 'subtotal' \| 'total')[]` | 瀑布圖的角色；預設全部為 `'delta'` |
| `label` | `(string \| null)[]` | 逐點文字：散佈點的標籤；取代長條與格子的數值文字 |

當各欄長度不同、某個 `x` 不是有限值，或類別座標軸上的值不是從 0 起的整數時，`setData` 會拋出錯誤並保留舊資料。

修改資料有三種方式：

| 方法 | 用途 |
|---|---|
| `setData(data)` | 全部取代 |
| `append(data)` | 在尾端加入點；`data` 必須與圖形的資料具有相同的欄 |
| `update(point)` | 取代最後一個點，例如即時數值；O(1)，不配置記憶體 |

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const pnl = plot.addMark('area', { color: '#26a69a', downColor: '#ef5350' });
plot.axis('x').applyOptions({ format: 'date' });

const t0 = Date.UTC(2024, 0, 2);
pnl.setData({ x: [t0, t0 + 60_000, t0 + 120_000], y: [0, 120, 80] });
pnl.append({ x: [t0 + 180_000], y: [150] });   // a new point
pnl.update({ x: t0 + 180_000, y: 165 });         // the last point changed
```

`update` 接受一個 `MarkPoint`（`{ x, y, y0?, value?, size?, color? }`）；圖形沒有的欄所對應的欄位會被忽略。若圖形沒有資料或 `x` 不是有限值，則會拋出錯誤。色階的值域與自動格子大小會沿用上一次 `setData` 或 `append` 計算出的值。

### 選項

選項在 `addMark` 時傳入，並以 `mark.applyOptions(options)` 修改；省略的鍵會保留原值。`mark.options()` 回傳所有選項。顏色為 `null` 代表使用預設值：`color` 取繪圖調色盤中的下一個顏色（第一個圖形使用主題的折線顏色），其他顏色則取主題的顏色。

| 選項 | 適用於 | 意義 | 預設值 |
|---|---|---|---|
| `yAxis` | 全部 | `'left'` 或 `'right'` | `'right'` |
| `orientation` | bars、waterfall | `'vertical'` 或 `'horizontal'` | `'vertical'` |
| `color` | 全部 | 主要顏色 | 調色盤 |
| `downColor` | area、waterfall | 低於基準值的區域；瀑布圖中減少的項目 | 主題 |
| `totalColor` | waterfall | 小計與總計 | 主題 |
| `fillAlpha` | area | 填色的不透明度，0 到 1 | `0.25` |
| `lineWidth` | line、area | CSS px，> 0 | `2` |
| `lineStyle` | line、area | `'solid'`、`'dashed'`、`'dotted'` | `'solid'` |
| `stepLine` | line、area | 階梯：先水平延伸到下一個 x，再垂直移動 | `false` |
| `pointSize` | line、scatter | 折線：在每個點畫一個符號（0：不畫）。散佈點：沒有 `size` 欄時的直徑 | `0` |
| `icon` | line、scatter | `'circle'`、`'square'`、`'arrowUp'`、`'arrowDown'` | `'circle'` |
| `baseValue` | area、bars | 沒有 `y0` 時面積與長條的起點 | `0` |
| `group` | bars | 同一群組的長條在同一格中並排；`null` 表示不分組 | `null` |
| `stack` | bars、area | 同一堆疊的圖形會相加（正值與負值分開）；`null` 表示不堆疊 | `null` |
| `band` | bars、cells | 占格子寬度的比例，0（不含）到 1；`null` 為預設值（長條 0.8，格子 1） | `null` |
| `colorScale` | cells、bars、scatter | `value` 如何對應到顏色（見下文）；`null` 為預設的連續色階 | `null` |
| `showValues` | bars、waterfall、cells | 空間足夠時，在每個元素上寫出數值 | `false` |
| `valueFormat` | bars、waterfall、cells | 這些數值的格式（請見[格式](#formats)） | `'auto'` |
| `cellSize` | cells | 數值座標軸上的 `[width, height]`，以資料單位計；`null`：相異 x 之間與相異 y 之間的最小間距 | `null` |
| `hoverable` | 全部 | 可被滑鼠懸停與點擊命中 | `true` |
| `visible` | 全部 | 是否繪製 | `true` |

無效的值會拋出錯誤，例如 `'fillAlpha' must be between 0 and 1` 或 `'icon' must be 'circle', 'square', 'arrowUp' or 'arrowDown'`。

`mark.id` 在其所屬繪圖中是唯一的，`mark.kind` 是其類型，`mark.remove()` 會移除該圖形。

### 類別座標軸上的長條

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const x = plot.axis('x');
x.applyOptions({ type: 'category' });
x.setCategories(['Tech', 'Energy', 'Banks', 'Health']);

// Two series side by side in each slot.
const q1 = plot.addMark('bars', { group: 1, showValues: true });
const q2 = plot.addMark('bars', { group: 1, showValues: true });
q1.setData({ x: [0, 1, 2, 3], y: [4.2, -1.3, 2.1, 0.8] });
q2.setData({ x: [0, 1, 2, 3], y: [3.1, 0.6, -0.4, 1.9] });

plot.axis('right').applyOptions({ format: { type: 'percent', decimals: 1 } });
plot.addRule({ value: 0, lineStyle: 'solid' });
```

類別 `k` 占據從 `k` 到 `k + 1` 的格子，標示為 `labels[k]`。若以 `stack: 1` 取代 `group: 1`，長條會改為堆疊。若要水平長條，請設定 `orientation: 'horizontal'`，將左側或右側座標軸設為類別座標軸，並對調各欄的意義：`x` 存放數值，`y` 存放類別編號。

### 瀑布圖

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const x = plot.axis('x');
x.applyOptions({ type: 'category' });
x.setCategories(['Revenue', 'COGS', 'Gross', 'Opex', 'Tax', 'Net']);

const pnl = plot.addMark('waterfall', {
  color: '#26a69a',       // increases
  downColor: '#ef5350',   // decreases
  totalColor: '#5d6d7e',  // subtotals and totals
  showValues: true,
  valueFormat: 'compact',
});
pnl.setData({
  x: [0, 1, 2, 3, 4, 5],
  y: [1_200_000, -450_000, 0, -380_000, -90_000, 0],
  step: ['delta', 'delta', 'subtotal', 'delta', 'delta', 'total'],
});
```

`'delta'` 會將其數值加到累計值上。`'subtotal'` 或 `'total'` 會畫出從 0 到累計值的長條，且不改變累計值；其 `y` 不會被使用。在 hover 與 click 事件中，小計或總計的 `hit.y` 為累計值。

### 帶有大小、顏色與標籤的散佈圖

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const assets = plot.addMark('scatter', { colorScale: 'diverging' });
assets.setData({
  x: [12.1, 18.4, 25.0, 9.8],          // volatility, %
  y: [6.2, 9.1, 14.7, 3.9],            // return, %
  size: [10, 18, 24, 8],               // diameter, CSS px
  value: [0.4, -0.2, 1.1, -0.6],       // e.g. Sharpe ratio, colored
  label: ['Bonds', 'Gold', 'Tech', 'Cash'],
});
plot.axis('x').applyOptions({ title: 'Volatility %' });
plot.axis('right').applyOptions({ title: 'Return %', includeZero: true });
```

### 格子（熱力圖）

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const names = ['SPX', 'NDX', 'GLD', 'TLT'];
const corr = [
  [1, 0.92, 0.05, -0.31],
  [0.92, 1, 0.02, -0.35],
  [0.05, 0.02, 1, 0.21],
  [-0.31, -0.35, 0.21, 1],
];

for (const name of ['x', 'right'] as const) {
  const a = plot.axis(name);
  a.applyOptions({ type: 'category' });
  a.setCategories(names);
}

const x: number[] = [], y: number[] = [], value: number[] = [];
corr.forEach((row, i) => row.forEach((v, j) => { x.push(j); y.push(i); value.push(v); }));

const cells = plot.addMark('cells', {
  colorScale: { center: 0, domain: [-1, 1] },
  showValues: true,
  valueFormat: { type: 'fixed', decimals: 2 },
});
cells.setData({ x, y, value });
```

在數值座標軸上，格子的寬與高為 `cellSize` 個資料單位，否則等於相異 `x` 值之間與相異 `y` 值之間的最小間距。

## 座標軸

`plot.axis(name)` 回傳 `'x'`、`'left'` 或 `'right'` 座標軸。左側座標軸預設隱藏；圖形以 `yAxis: 'left'` 使用它。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('left').applyOptions({ visible: true, title: 'Volume', format: 'compact' });
plot.addMark('bars', { yAxis: 'left', color: 'rgba(120, 120, 120, 0.5)' });
plot.addMark('line', { yAxis: 'right' });
```

### `AxisOptions`

`axis.applyOptions(options)` 會修改選項；省略的鍵會保留原值。`axis.options()` 回傳所有選項。

| 選項 | 意義 | 預設值 |
|---|---|---|
| `type` | `'linear'`、`'log'` 或 `'category'` | `'linear'` |
| `range` | `'auto'`（跟隨資料），或固定的 `[lo, hi]` | `'auto'` |
| `includeZero` | 自動範圍：讓 0 保持在範圍內 | `false` |
| `margin` | 自動範圍：在兩端各加上的比例，0 到 1 | `0.05`（類別：`0`） |
| `format` | 刻度標籤的書寫方式（見下文） | `'auto'` |
| `inverted` | 數值向右（x）或向上（y）遞減 | `false` |
| `visible` | 繪製刻度、標籤與座標軸線 | `true`（左側：`false`） |
| `grid` | 在刻度處畫格線（類別座標軸：在格子邊界處） | `true`（類別：`false`） |
| `title` | 座標軸旁的文字；`null` 會將其移除 | `null` |
| `locked` | 平移與縮放不影響此座標軸 | `false` |

修改 `type` 時，`grid`、`margin` 與 `range` 會從該類型的預設值開始；`visible`、`title`、`format`、`inverted` 與 `locked` 則保留。

自動範圍涵蓋該座標軸上各圖形的可見資料。對於長條、面積與瀑布圖，位於零的那一端不加邊距，讓長條立在座標軸上。類別座標軸涵蓋所有類別。

錯誤：固定範圍必須是兩個相異的有限數，在對數座標軸上還必須為正數。

### 對數座標軸

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('right').applyOptions({ type: 'log' });
```

0 或負值在對數座標軸上沒有位置，不會被繪製。

### 類別座標軸

類別座標軸由等寬的格子組成；類別 `k` 占據 `[k, k + 1)`，其中心為 `k + 0.5`。其上的資料為從 0 起的整數。以 `axis.setCategories(labels)` 為類別命名（最多 1 000 000 個）。

### 日期座標軸

日期座標軸是數值為 UTC 毫秒、並使用 `'date'` 格式的線性座標軸：

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('x').applyOptions({ format: 'date' });                              // UTC
plot.axis('x').applyOptions({ format: { type: 'date', utcOffsetMinutes: 60 } }); // UTC+1
```

### 格式

`PlotFormat` 用於 `AxisOptions.format` 與 `MarkOptions.valueFormat`：

| 格式 | 輸出 |
|---|---|
| `'auto'` | 至少 2 位小數與 4 位有效數字，較小的值以下標表示零的個數（與圖表的價格標籤相同） |
| `'compact'` | K、M 與 B 後綴 |
| `{ type: 'fixed', decimals }` | 固定小數位數，0 到 12 |
| `{ type: 'percent', decimals }` | 固定小數位數並加上 `%` 符號；數值本身已是百分比（`5` 寫成 `5%`） |
| `'date'` | 數值為 UTC ms，以 UTC 的日期與時間寫出 |
| `{ type: 'date', utcOffsetMinutes }` | 同上，但相對 UTC 有偏移，−1440 到 1440 分鐘 |

### 可見範圍

| 方法 | 說明 |
|---|---|
| `axis.view(): [number, number]` | 截至上一幀的可見範圍 |
| `axis.setView(lo, hi)` | 顯示 `lo..hi`（在類別座標軸上以類別為單位）；座標軸會停止跟隨資料，直到呼叫 `plot.fit()`。不會送出 `viewChange` 事件 |
| `plot.fit()` | 讓每條座標軸回到其設定的範圍 |

## 參考線

參考線是在某個數值處橫跨繪圖的線，或兩個數值之間帶底色的區帶。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const target = plot.addRule({ value: 100, label: 'target' });          // horizontal line on the right axis
plot.addRule({ axis: 'x', value: Date.UTC(2024, 5, 1), label: 'FOMC' }); // vertical line
plot.addRule({ value: 30, to: 70, color: 'rgba(120, 120, 255, 0.12)' }); // band

target.applyOptions({ value: 105 });
target.remove();
```

| 選項 | 意義 | 預設值 |
|---|---|---|
| `axis` | `value` 所屬的座標軸：`'x'` 會產生垂直參考線 | `'right'` |
| `value` | 位置 | `0` |
| `to` | 區帶的另一邊；`null` 表示線 | `null` |
| `color` | 顏色；`null` 為主題的十字游標顏色（區帶的不透明度為 12%） | `null` |
| `lineWidth` | CSS px | `1` |
| `lineStyle` | `'solid'`、`'dashed'`、`'dotted'` | `'dashed'` |
| `label` | 座標軸上該數值處的文字；`null` 表示無 | `null` |
| `labelBackground` | 標籤放在參考線顏色的方框上（否則以參考線顏色書寫） | `true` |

## 標籤

標籤是位於資料座標某一點上的文字、符號或兩者兼具。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const peak = plot.addLabel({ x: 3, y: 14.7, text: 'peak', icon: 'arrowDown', align: 'center', offset: [0, -14] });
peak.remove();
```

| 選項 | 意義 | 預設值 |
|---|---|---|
| `x`、`y` | 該點（必填） | |
| `yAxis` | `'left'` 或 `'right'` | `'right'` |
| `text` | 文字（有圖示時可為空） | `''` |
| `icon` | `'circle'`、`'square'`、`'arrowUp'`、`'arrowDown'` 或 `null` | `null` |
| `color` | `null` 為主題的文字顏色 | `null` |
| `align` | 文字相對於該點的對齊方式：`'left'`、`'center'`、`'right'` | `'left'` |
| `offset` | 文字相對於該點的偏移，以 CSS px 計，向右與向下 | `[6, 0]` |

標籤無法修改；請移除後再新增一個。

## 色階

格子一律以色階為其 `value` 欄著色。長條與散佈圖形在有 `value` 欄時也會這麼做；否則使用 `color`。點在 `color` 欄中自己的顏色優先於其數值。

| `colorScale` | 意義 |
|---|---|
| `null` 或 `'sequential'` | 由深藍到黃色，從低到高 |
| `'diverging'` | 主題的下跌色、中性的中間色與上漲色，以 0 為中心 |
| `{ stops?, center?, domain?, quantile? }` | 你自己的色階 |

物件形式：

| 鍵 | 意義 |
|---|---|
| `stops` | `[position, color][]`，位置為 0 到 1 並依遞增排列（至少一個色標） |
| `center` | 讓色階以此值為中心發散：色階的中間位於 `center`，離它較遠的那一側決定兩側的範圍 |
| `domain` | 固定的 `[lo, hi]`；超出範圍的值使用兩端的顏色 |
| `quantile` | 未指定 `domain` 時：色階涵蓋從 `quantile` 分位數到 `1 − quantile` 分位數的值，讓極端值達到飽和。0 到 0.5，預設 0.01 |

未指定 `stops` 時，有 `center` 的色階使用發散色，沒有的則使用連續色。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const heat = plot.addMark('cells', {
  colorScale: { stops: [[0, '#0d1b4c'], [0.5, '#219ebc'], [1, '#f4e04d']], quantile: 0.05 },
});
```

值域由 `setData` 與 `append` 從資料計算而得；`update` 會保留它。

## 命中測試與事件

繪圖會找出指標下方的元素：折線或散佈圖的某個點、某根長條或某個格子。它會醒目標示該元素（`hoverHighlight`）、畫出穿過它的十字游標線（`crosshair`），並回報它。

```ts
interface PlotEventMap {
  hover: { hit: PlotHit | null };                 // the hovered element changed
  click: { hit: PlotHit | null; x: number; y: number };
  viewChange: {};                                  // an axis range changed by pan, zoom or fit()
}

interface PlotHit {
  mark: Mark;      // compare with mark.id: it is a new handle each time
  index: number;   // index of the point in the mark's data
  x: number;
  y: number;       // waterfall subtotals and totals: the running total
  position: { x: number; y: number }; // where it is drawn, CSS px from the canvas' top-left
}
```

在 `click` 中，`x` 與 `y` 是指標位置的座標軸數值（`y` 取自右側座標軸，若右側座標軸隱藏則取自左側）。設定 `hoverable: false` 的圖形永遠不會被命中。

```ts
import { FerroPlot, type PlotHit } from '@ferrochart/core';

declare const plot: FerroPlot;
declare const tooltip: HTMLElement;

const bars = plot.addMark('bars', { showValues: true });
bars.setData({ x: [0, 1, 2], y: [5, 6, 7] });

plot.on('hover', ({ hit }: { hit: PlotHit | null }) => {
  if (!hit || hit.mark.id !== bars.id) {
    tooltip.hidden = true;
    return;
  }
  tooltip.hidden = false;
  tooltip.textContent = `#${hit.index}: ${hit.y}`;
  tooltip.style.transform = `translate(${hit.position.x}px, ${hit.position.y}px)`;
});

plot.on('click', ({ hit, x, y }: { hit: PlotHit | null; x: number; y: number }) => {
  console.log(hit ? `clicked point ${hit.index}` : `empty space at ${x}, ${y}`);
});
```

事件會在產生它們的那一幀之後送出。

## 讓繪圖與圖表窗格對齊

`chart.attachSidePlot(plot, pane?)` 會把繪圖放在圖表窗格旁邊，讓同一價格在兩者中位於相同高度。每一幀，圖表都會依照窗格的對應方式，將繪圖的右側座標軸設為繪圖區頂部與底部的價格。canvas 在頁面上的位置每一幀都會重新讀取，因此繪圖可以放在圖表旁的任何位置，其繪圖區也可以比窗格高或矮。

- 繪圖的右側座標軸會被鎖定：圖表是它唯一的來源。若附加時窗格的價格軸為對數刻度，右側座標軸會變成對數座標軸。
- 繪圖的標誌會被移除；圖表的標誌同時代表兩者。
- `chart.detachSidePlot(plot)` 會停止跟隨；右側座標軸保持鎖定，並保留最後的範圍。

### 範例：成交量分佈圖

分佈圖以 K 線旁的水平長條，顯示可見 K 線中各價位成交了多少量。

```html
<div style="display: flex; height: 520px">
  <canvas id="chart" style="flex: 1; min-width: 0"></canvas>
  <canvas id="profile" style="width: 160px"></canvas>
</div>
```

```ts
import { FerroChart, FerroPlot, randomWalk, type OhlcData } from '@ferrochart/core';

const chart = await FerroChart.create(document.getElementById('chart') as HTMLCanvasElement, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
const bars: OhlcData = randomWalk(3, 3_000, Date.UTC(2024, 0, 1), 60_000);
candles.setData(bars);
chart.addVolumeSeries(candles);

const profile = await FerroPlot.create(document.getElementById('profile') as HTMLCanvasElement, {
  theme: 'dark',
  hoverHighlight: false,
});
profile.axis('x').applyOptions({ visible: false, grid: false });
profile.axis('right').applyOptions({ visible: false, grid: false }); // the chart shows the prices
const rows = profile.addMark('bars', { orientation: 'horizontal', color: 'rgba(41, 98, 255, 0.5)', band: 0.9 });
chart.attachSidePlot(profile);

/** Volume per price row of the bars `from..=to` (indices into `bars`). */
function volumeProfile(from: number, to: number, rowSize: number) {
  const byRow = new Map<number, number>();
  for (let i = from; i <= to; i++) {
    const row = Math.floor(bars.close[i] / rowSize);
    byRow.set(row, (byRow.get(row) ?? 0) + (bars.volume?.[i] ?? 0));
  }
  const y: number[] = [], x: number[] = [];
  for (const [row, volume] of byRow) {
    y.push((row + 0.5) * rowSize); // price at the middle of the row
    x.push(volume);
  }
  return { x, y };
}

const n = bars.close.length;
const update = (from: number, to: number) => {
  const a = Math.max(0, Math.floor(from));
  const b = Math.min(n - 1, Math.ceil(to));
  if (a <= b) rows.setData(volumeProfile(a, b, 0.5));
};
update(n - 200, n - 1);
chart.on('visibleRangeChanged', ({ from, to }: { from: number; to: number }) => update(from, to));
```

數值座標軸上的水平長條，其粗細為 `band` 乘以各 `y` 值之間的最小間距，因此大小相同的列在 `band: 1` 時會彼此相接。當使用者捲動或縮放圖表時，`visibleRangeChanged` 會為可見的 K 線重新計算分佈圖，價格範圍則會自動跟隨。若要在 K 線旁顯示委託簿深度，可使用相同的設定，以買賣掛單量作為 `x`、其價格作為 `y`，並以 `rows.update` 處理最後一檔的即時變動。

## 清理

`plot.destroy()` 會移除繪圖的監聽器與標誌，並釋放其 GPU 資源；側邊繪圖也會停止跟隨其圖表。之後 `Mark`、`Axis`、`Rule` 與 `Label` handle 都會拋出錯誤（`the FerroPlot was destroyed`）。canvas 元素會保留在頁面中。
