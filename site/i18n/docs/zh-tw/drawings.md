<!-- source: 58a84755b2 -->
# 繪圖工具

繪圖物件是由使用者或程式碼放置在圖表上的物件：趨勢線、矩形、斐波那契工具、部位、型態、文字等。每個繪圖物件都有位於資料空間（時間與價格）的錨點，因此在平移、縮放或調整圖表大小時，它會固定在 K 線上。FerroChart 自行處理建立、選取、拖曳、刪除與復原/重做；你的頁面只需啟用工具，並視需要回應繪圖事件。

## Drawings 物件

所有繪圖方法都在 `chart.drawings` 上：

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

繪圖物件以數字 id 識別。同一張圖表內的 id 永不重複使用；以復原找回的繪圖物件會恢復其原本的 id。

| 方法 / 屬性 | 作用 |
|---|---|
| `kinds()` | `add` 與 `startTool` 接受的類型（29 種內建類型加上已註冊的類型） |
| `startTool(kind)` / `cancelTool()` / `activeTool` | 互動式建立 |
| `add(data, pane?)` | 加入一個繪圖物件；回傳其 id |
| `get(id)` | 該繪圖物件的 `DrawingData`（類型、錨點、樣式、參數），未知 id 回傳 `null` |
| `ids()` | 所有 id，由下而上 |
| `paneOf(id)` | 繪圖物件所在窗格的 id |
| `setAnchors(id, anchors)` | 移動錨點 |
| `setStyle(id, style)` | 修改部分樣式鍵 |
| `setParam(id, key, value)` | 設定工具參數（`text`、`levels`） |
| `select(id)` / `selected` | 選取 |
| `setVisible(id, visible)` / `isVisible(id)` | 顯示或隱藏 |
| `setLocked(id, locked)` / `isLocked(id)` | 鎖定以防止拖曳 |
| `bringToFront(id)` / `sendToBack(id)` | 疊放順序 |
| `remove(id)` | 移除繪圖物件 |
| `undo()` / `redo()` / `canUndo` / `canRedo` / `clearHistory()` | 歷史紀錄 |
| `register(kind, tool)` | 加入以 JavaScript 撰寫的繪圖工具 |

接受 id 的方法遇到未知 id 時會拋出錯誤（`get`、`paneOf`、`isVisible` 與 `isLocked` 除外，它們回傳 `null` 或 `undefined`）。繪圖物件會隨圖表狀態一起儲存與還原（請見[儲存狀態](state.md)）。

## 內建類型

類型名稱是 snake_case 字串。

**線條**

| 類型 | 錨點 | 說明 |
|---|---|---|
| `trend_line` | 2 | 兩錨點之間的線段 |
| `ray` | 2 | 從第一個錨點經過第二個錨點延伸到繪圖區邊緣 |
| `extended_line` | 2 | 穿過兩個錨點，向兩側延伸到繪圖區邊緣 |
| `horizontal_line` | 1 | 在錨點價格處橫跨繪圖區，右側邊緣有價格標籤 |
| `horizontal_ray` | 1 | 從錨點延伸到右側邊緣，附價格標籤 |
| `vertical_line` | 1 | 在錨點時間處縱貫繪圖區 |
| `arrow` | 2 | 在第二個錨點處帶有箭頭的線段 |

**形狀與文字**

| 類型 | 錨點 | 說明 |
|---|---|---|
| `parallel_channel` | 3 | 穿過前兩個錨點的直線，以及穿過第三個錨點的平行線；有填色，並有一條虛線中線 |
| `rectangle` | 2 | 對角兩點；有填色 |
| `ellipse` | 2 | 內切於兩錨點構成的矩形；有填色 |
| `text` | 1 | 從錨點開始的文字。參數 `text`（預設 `'Text'`）。填色不透明時，繪製在 `fill` 顏色的背景上 |
| `callout` | 2 | 位於第二個錨點的文字框，以引線連到第一個錨點。參數 `text`（預設 `'Callout'`）。文字框以線條顏色填滿，文字為白色 |
| `path` | 2 個以上 | 折線；每點擊一次加一個點，以雙擊或 Enter 結束 |
| `brush` | 2 個以上 | 手繪線：按下、拖曳後放開。沒有控制點；只能整體移動 |

**測量與部位**

| 類型 | 錨點 | 說明 |
|---|---|---|
| `price_range` | 2 | 帶底色的區間，顯示從第一個錨點到第二個錨點的價格變化與百分比變化 |
| `date_range` | 2 | 帶底色的區間，顯示 K 線數量與持續時間 |
| `long_position` | 3（點擊 2 次） | 進場（第一個錨點）、目標（第二個）與停損（第三個）。建立時，停損會以進場價為中心與目標對稱。顯示獲利與虧損區域、含價格與百分比的標籤，以及報酬風險比 |
| `short_position` | 3（點擊 2 次） | 與 `long_position` 相同，百分比以空單計算 |

**斐波那契與江恩**

| 類型 | 錨點 | 說明 |
|---|---|---|
| `fib_retracement` | 2 | 對每個水準在 `p2 + (p1 − p2) · level` 處畫一條水準線，標示為 `level (price)`，並以交替的底色帶區隔。參數 `levels`（預設 `[0, 0.236, 0.382, 0.5, 0.618, 0.786, 1]`） |
| `fib_extension` | 3 | 水準位於 `p3 + (p2 − p1) · level`，從第三個錨點開始繪製。參數 `levels`（預設 `[0, 0.382, 0.618, 1, 1.272, 1.618, 2, 2.618]`） |
| `fib_time_zones` | 2 | 在 K 線 `i1 + (i2 − i1) · F` 處的垂直線，F 為斐波那契數 0、1、2、3、5、8、… |
| `gann_fan` | 2 | 從第一個錨點發出的射線，斜率為連到第二個錨點之直線的 1/8 … 8 倍，標示為 `1x8` … `8x1` |
| `pitchfork` | 3 | 安德魯音叉線：從第一個錨點穿過另兩點中點的中線，以及穿過那兩點的平行叉齒；叉齒之間有填色 |
| `schiff_pitchfork` | 3 | 與 `pitchfork` 相同，但原點在價格上往第二個錨點移動一半 |
| `modified_schiff_pitchfork` | 3 | 與 `pitchfork` 相同，但原點位於前兩個錨點的中點 |

**型態與波浪**

| 類型 | 錨點 | 說明 |
|---|---|---|
| `xabcd_pattern` | 5 | X、A、B、C、D 五點，並標示各段之間的回撤比率；XAB 與 BCD 三角形有填色 |
| `head_and_shoulders` | 7 | 左肩、頭部、右肩的標籤，以及穿過第三與第五個錨點的頸線；有填色 |
| `elliott_impulse` | 6 | 標示為 0 到 5 的波浪 |
| `elliott_correction` | 4 | 標示為 0、A、B、C 的波浪 |

`p1`、`p2`、`p3` 依序為各錨點的價格。型態與波浪的標籤在波峰時位於上方，在波谷時位於下方。

## 以互動方式建立繪圖物件

```ts
chart.drawings.startTool('rectangle');
```

`startTool(kind)` 會啟用一個工具。游標在繪圖區上方變成十字形，下一次在任一窗格上按下時就會在那裡開始繪製。未知的類型會拋出錯誤。啟用工具會取消正在進行中的繪製。

繪圖物件如何完成取決於類型：

- **固定數量的錨點**（大多數類型）：第一次按下放置第一個錨點；下一個錨點會跟著指標移動，直到你再次點擊，依此類推直到放置最後一個錨點。單一錨點的類型（`horizontal_line`、`horizontal_ray`、`vertical_line`、`text`）在第一次按下時即完成。前兩個錨點也可以透過按下、拖曳、放開來放置。
- **不定數量**（`path`）：每點擊一次放一個錨點。雙擊或 Enter 結束繪製（雙擊產生的重複點會被捨棄）。結束時至少需要兩個錨點。
- **手繪**（`brush`）：按下、拖曳後放開。每移動 3 CSS px 就新增一個錨點。少於兩個錨點的筆畫會被捨棄。

放置錨點時，x 會吸附到最近的 K 線（超過最後一根 K 線的時間會以外插計算），在磁吸十字游標模式下，y 會吸附到最近的開盤、最高、最低或收盤價。手繪錨點不會吸附。

繪圖物件完成時，它會被選取、送出 `drawingCreated` 事件，並且**解除工具的啟用**：一次 `startTool` 建立一個繪圖物件。若要持續繪製同一類型，請在 `drawingCreated` 監聽器中再次啟用它（請見下方的工具列範例）。

| 按鍵 | 建立期間 |
|---|---|
| Escape | 取消正在進行中的繪製（至少有兩個錨點的 `path` 則改為完成繪製），或解除已啟用的工具 |
| Enter | 完成 `path` |

`cancelTool()` 會解除工具的啟用並捨棄進行中的繪製，不送出事件。`activeTool` 是已啟用的類型，或正在建立的繪圖物件類型；其他情況下為 `undefined`。

建立繪圖物件期間不會送出 `click` 事件。

## 選取與編輯

沒有啟用工具時：

- **按下繪圖物件**會選取它（`drawingSelected`），並顯示其控制點：大多數類型每個錨點一個；部位在進場、目標、停損與右側邊緣各有一個控制點；水平線與垂直線在繪圖區中央有一個；筆刷則沒有。
- **拖曳**繪圖物件會移動它：水平方向以整根 K 線為單位，垂直方向跟隨指標。拖曳控制點會移動該錨點。拖曳完成時送出 `drawingModified`。拖曳期間按 Escape 會將繪圖物件放回原處。
- **按下空白處**會清除選取。
- 命中判定的順序是：先是已選取繪圖物件的控制點，接著是已選取的繪圖物件，然後是其他繪圖物件（由上而下）。
- 隱藏的繪圖物件無法被命中。矩形、橢圓與通道在其外框上可被命中，填色不透明時則在內部任何位置皆可。

在繪圖物件上按下不會造成平移。在鎖定的繪圖物件上按下會選取它，然後平移圖表。

### 鍵盤

canvas 取得焦點時才會接收鍵盤輸入；按下 canvas 即會使其取得焦點。

| 按鍵 | 效果 |
|---|---|
| Delete | 移除已選取的繪圖物件（鎖定的也會移除） |
| Escape | 取消建立、解除工具，或還原目前的拖曳 |
| Enter | 完成正在建立的 `path` |
| Ctrl+Z / ⌘+Z | 復原 |
| Ctrl+Shift+Z / ⌘+Shift+Z、Ctrl+Y / ⌘+Y | 重做 |

Backspace 沒有作用。若要以其他按鍵刪除，請自行在 canvas 上監聽：

```ts
canvas.addEventListener('keydown', (e) => {
  const id = chart.drawings.selected;
  if (e.key === 'Backspace' && id != null) chart.drawings.remove(id);
});
```

## 從程式碼加入繪圖物件

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

- `time` 是 UTC 毫秒（整數）。不必是某根 K 線的時間。
- `price` 以窗格第一個價格系列的單位表示，且必須為有限值。
- 錨點數量必須符合類型：大多數類型須與表中完全一致；部位為 2 或 3 個（給 2 個時，停損會以目標的對稱點加入）；`path` 與 `brush` 為 2 到 4096 個。
- `pane` 預設為主窗格。傳入窗格 id 可在其他窗格上繪製。
- 未指定的樣式鍵採用下方的預設值；類型不使用的參數鍵會被忽略。

遇到未知類型、錨點過少或過多、非有限的價格、未知窗格、無效樣式或無效參數時，`add` 會拋出錯誤且不加入任何東西。從程式碼加入的繪圖物件不會被選取。

`get(id)` 回傳包含所有鍵的儲存格式：`kind`、`anchors`、完整的 `style`，以及含有該類型參數的 `params`（`text` 與 `callout` 為 `text`，斐波那契回撤與延伸為 `levels`）。將它傳給 `add` 即可複製一個繪圖物件：

```ts
const copy = chart.drawings.get(id);
if (copy) chart.drawings.add(copy, chart.drawings.paneOf(id));
```

`setAnchors(id, anchors)` 會移動繪圖物件。它需要與該繪圖物件相同數量的錨點（已完成的部位為 3 個）。

## 樣式

```ts
chart.drawings.setStyle(id, { line: '#26a69a', lineWidth: 2, lineDash: 'dashed' });
```

| 鍵 | 型別 | 預設值 | 用於 |
|---|---|---|---|
| `line` | `Color` | `#2962ff` | 線條與外框；區間與部位的標籤背景；callout 文字框 |
| `lineWidth` | `number` | `1` | CSS px，大於 0 |
| `lineDash` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | 線條的虛線樣式 |
| `fill` | `Color` | `#2962ff`，不透明度 16% | 區域：通道、矩形、橢圓、區間、斐波那契色帶、音叉線、型態三角形、文字背景 |
| `text` | `Color` | `#2962ff` | 沒有背景的文字與標籤 |
| `fontSize` | `number` | `12` | CSS px，文字與標籤 |

`setStyle` 只會修改你傳入的鍵。顏色不可為 `null`；不要填色時請使用 `'transparent'`。部位的獲利與虧損區域使用固定的綠色與紅色。

## 參數

```ts
chart.drawings.setParam(textId, 'text', 'Breakout');
chart.drawings.setParam(fibId, 'levels', [0, 0.5, 1, 1.618]);
```

| 參數 | 類型 | 值 |
|---|---|---|
| `text` | `text`、`callout` | 字串 |
| `levels` | `fib_retracement`、`fib_extension` | 最多 24 個有限數 |

型別錯誤的值會拋出錯誤，且繪圖物件保持不變。樣式與參數的變更會送出 `drawingModified`，並且可以復原。

## 可見性、鎖定與順序

```ts
chart.drawings.setVisible(id, false);   // hidden drawings are not drawn or hit
chart.drawings.setLocked(id, true);     // can be selected and deleted, not dragged
chart.drawings.bringToFront(id);
chart.drawings.sendToBack(id);
console.log(chart.drawings.isVisible(id), chart.drawings.isLocked(id), chart.drawings.ids());
```

隱藏已選取的繪圖物件會清除選取。鎖定不會阻止程式碼呼叫 `setAnchors`、`setStyle` 或 `remove`。可見性、鎖定與順序不會送出事件，也不屬於復原歷史；它們會隨圖表狀態一起儲存。

## 選取

```ts
chart.drawings.select(id);      // selects it and shows its handles
chart.drawings.select(null);    // clears the selection
const sel = chart.drawings.selected; // the selected id, or undefined
```

## 移除

```ts
chart.drawings.remove(id);
for (const d of chart.drawings.ids()) chart.drawings.remove(d); // remove all
```

移除窗格會移除其上的繪圖物件；若其中有任何繪圖物件，復原歷史會被清除。

## 復原與重做

下列變更無論由使用者或程式碼發起都會被記錄：加入、移除、移動（拖曳、`setAnchors`），以及修改樣式或參數。選取、可見性、鎖定與疊放順序則不會。

```ts
chart.drawings.undo();          // false if there was nothing to undo
chart.drawings.redo();
if (chart.drawings.canUndo) { /* enable the undo button */ }
chart.drawings.clearHistory();
```

歷史紀錄保留最近 256 次變更。新的變更會清除重做清單。`undo` 與 `redo` 會取消進行中的繪製。載入狀態（`loadState`）會清除歷史紀錄。從同步群組收到的繪圖物件不會被記錄。

## 繪圖事件

| 事件 | 內容 | 時機 |
|---|---|---|
| `drawingCreated` | `{ id }` | 繪圖物件由使用者完成、從程式碼加入、經復原/重做還原或被載入 |
| `drawingModified` | `{ id }` | 繪圖物件被移動、變更樣式或修改了參數 |
| `drawingRemoved` | `{ id }` | 繪圖物件被移除 |
| `drawingSelected` | `{ id: number \| null }` | 選取狀態改變 |

事件在發生變更的那一幀之後送達，而不是在方法呼叫內部。請見[十字游標與事件](crosshair-events.md)。

```ts
chart.on('drawingCreated', (e: ChartEventMap['drawingCreated']) => {
  const d = chart.drawings.get(e.id);
  console.log('new', d?.kind, d?.anchors);
});
```

## 範例：工具列

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

由按鈕觸發的復原與重做會在下一幀之後送出 `drawingCreated`、`drawingModified` 或 `drawingRemoved`，進而更新按鈕狀態。

## 自訂工具

`chart.drawings.register(kind, tool)` 會加入一個以 JavaScript 撰寫的繪圖工具。工具會說明自己的建立方式（`anchorCount`：點擊次數、`{ open: min }` 或 `'freehand'`），並從 `build(ctx)` 回傳繪圖指令；可選的 `hitTest` 決定按下時命中什麼。此類型可供這張圖表的 `add`、`startTool` 與 `loadState` 使用；內建類型無法被取代。請見[外掛](plugins.md#drawing-tools)。
