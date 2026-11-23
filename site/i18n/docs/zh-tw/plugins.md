<!-- source: 8785086f6e -->
# 外掛

FerroChart 提供五種擴充圖表的方式。它們都會把繪圖指令寫入幀中，因此能搭配所有渲染器（WebGPU、WebGL2、Vulkan/Metal/DX12 以及 CPU 渲染器）運作，並且和內建系列一樣會出現在螢幕截圖與黃金圖像中。

| 擴充方式 | 用途 | Rust | JavaScript |
|---|---|---|---|
| 圖層外掛 | 在窗格或整張圖表上額外繪圖：浮水印、交易標記、交易時段底色 | `LayerPlugin` | `chart.addLayer` |
| 自訂系列 | 自行持有並繪製資料的系列：分佈圖、熱力圖、區間方框 | `CustomSeries` | — |
| 原生 GPU 系列 | 自訂系列類型的 GPU 實作（compute shader、大量資料） | `GpuSeries` | — |
| 系列變換 | 由 K 線計算出的新類 OHLC 系列：Renko 一類的圖表 | `SeriesTransform` | — |
| 繪圖工具 | 使用者繪製的物件，具有錨點、控制點與命中測試 | `DrawingTool` | `chart.drawings.register` |

可執行的 Rust 範例是 `examples/plugins`（`cargo run -p example-plugins
--release`）；其中包含浮水印、交易標記，以及一個由模擬行情更新的自訂系列。訂單流 crate（`ferrochart-orderflow`，請見[訂單流](orderflow.md)）是最大的外掛，用到了所有 Rust 擴充點。

## 所有外掛都遵循的規則

- **每一幀都繪製，且不配置記憶體。** `build` 在每一幀都會被呼叫，包括使用者平移與縮放時。請寫入傳給你的 `FrameBuilder`（其記憶體池會保留容量），並把自己的暫存緩衝區放在外掛中，清空後重複使用。只繪製可見的部分：你會拿到可見範圍。
- **不要 panic。** 外掛在圖表的幀內執行；請改為提早返回。
- **座標在 Rust 中是實體像素**（`Px`、`PointPx`、`RectPx`），在 JavaScript 中是 CSS 像素。在 Rust 中以 `ctx.px(Css(…))` 將 CSS 尺寸轉換過來。
- **顏色來自樣式或你的選項。** 使用 `ctx.style()`（依主題解析過的系列顏色）或以選項接收的顏色；如此外掛就能跟著淺色與深色主題變化。
- **把變動告訴圖表。** 內容有變動的圖層要呼叫 `invalidate_layer`；自訂系列只能透過 `update_custom` 修改其資料。

## 圖層外掛

圖層繪製在單一窗格（其繪圖區）或整張圖表上，可位於四種深度之一：

| `LayerPosition` | 繪製位置 |
|---|---|
| `Background` | 格線之上，其他所有內容之下 |
| `BelowSeries` | 系列之下 |
| `AboveSeries` | 系列與標記之上，繪圖物件與標籤之下 |
| `Overlay` | 在疊加層上，十字游標之下 |

```rust
use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::frame::TextAlign;
use ferrochart_core::geom::{Css, PointPx, Rgba};
use ferrochart_core::plugin::{LayerCtx, LayerPlugin, LayerPosition};

struct Watermark(&'static str);

impl LayerPlugin for Watermark {
    fn position(&self) -> LayerPosition {
        LayerPosition::Background
    }

    fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
        let r = ctx.rect();
        let center = PointPx { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 };
        let label = Label {
            font: ctx.font(Css(48.0)),
            color: Rgba::hex(0x80_8080).with_alpha(0x40),
            align: TextAlign::Center,
            bg: None,
        };
        out.text(center, self.0, label);
    }
}

// None: the whole chart; Some(pane): that pane's plot.
let id = chart.add_layer(None, Box::new(Watermark("BTCUSD")))?;
```

在窗格上，`ctx.coords()` 提供該窗格的時間與價格轉換（`x_of`、`y_of`、`time_at`、`price_at`、`px`、`plot`、價格格式化）。以 `time_at(plot.x)` 與 `time_at(plot.right())` 搭配二分搜尋找出資料中可見的部分，就像範例中的 `TradeMarks` 一樣。

當圖層自己的資料有變動時，請呼叫 `chart.invalidate_layer(id)`，讓下一幀重繪它。`chart.remove_layer(id)` 會移除它。

### 在 JavaScript 中

```js
const id = chart.addLayer({
  pane: 0,                  // omit for the whole chart
  position: 'aboveSeries',  // 'background' | 'belowSeries' | 'aboveSeries' | 'overlay'
  build(ctx) {
    const x = ctx.timeToX(entryTime);
    const y = ctx.priceToY(entryPrice);
    if (x === null || y === null) return [];
    return [
      { type: 'line', x1: ctx.rect.x, y1: y, x2: x, y2: y, color: '#2962FF', dash: 'dashed' },
      { type: 'text', x: x + 4, y, text: ctx.formatPrice(entryPrice), color: '#2962FF', align: 'left' },
    ];
  },
});
chart.invalidateLayer(id); // after the plugin's own data changed
chart.removeLayer(id);
```

`build(ctx)` 回傳一個指令陣列，座標為從 canvas 左上角起算的 CSS 像素：`rect`、`line`、`polyline`、`polygon`、`text` 與 `icon`（`PluginCommand` 型別列出了它們的欄位）。`icon` 可使用與標記相同的額外欄位：`image`（以 `chart.registerIcon` 註冊的名稱）、`label`、`border` 與 `borderWidth`，因此圖層可以在任何位置繪製頭像，而不只是在 K 棒上（參見[標記](markers.md)）。`ctx` 的函式（`timeToX`、`priceToY`、`xToTime`、`yToPrice`、`textWidth`、`formatPrice`）只在呼叫期間有效。拋出例外或回傳無效指令的外掛在該幀什麼都不會畫（無效指令之前的指令會保留）；其第一個錯誤會以 `console.warn` 記錄。

JavaScript 圖層每一幀會跨越 wasm 邊界被呼叫一次，因此請將指令數量控制在可見範圍內（數百個，而不是數十萬個）。大量資料請用 Rust 撰寫自訂系列。

## 自訂系列

自訂系列自行持有資料。圖表會向它詢問時間與價格極值，把它放上時間軸，依它縮放價格軸，並讓它繪製可見的點。

```rust
use ferrochart_core::chart::SeriesOptions;
use ferrochart_core::data::{DataChange, Timestamp};
use ferrochart_core::drawing::FrameBuilder;
use ferrochart_core::geom::RectPx;
use ferrochart_core::plugin::{AxisMode, CustomSeries, MinMaxIndex, SeriesCtx};

/// A vertical bar from `lo` to `hi` per point.
#[derive(Default)]
struct Ranges {
    time: Vec<Timestamp>,
    range: Vec<(f64, f64)>,
    index: MinMaxIndex,
}

impl Ranges {
    fn push(&mut self, t: Timestamp, lo: f64, hi: f64) -> DataChange {
        self.time.push(t);
        self.range.push((lo, hi));
        let _ = self.index.push(lo, hi);
        DataChange::Append { count: 1 }
    }
}

impl CustomSeries for Ranges {
    fn kind(&self) -> &'static str {
        "example.ranges"
    }
    fn axis_mode(&self) -> AxisMode {
        AxisMode::Slots
    }
    fn times(&self) -> &[Timestamp] {
        &self.time
    }
    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        self.index.query(from, to)
    }
    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
        let w = (ctx.bar_spacing().0 * 0.6).max(1.0);
        for i in from..to {
            let (lo, hi) = self.range[i];
            let (Some(x), Some(y0), Some(y1)) =
                (ctx.x_of_point(i), ctx.coords().y_of(hi), ctx.coords().y_of(lo))
            else {
                continue;
            };
            out.rect(RectPx { x: x - w / 2.0, y: y0, w, h: (y1 - y0).max(1.0) }, ctx.style().up);
        }
    }
}

let id = chart.add_custom_series(chart.main_pane(), Box::new(Ranges::default()), SeriesOptions::default())?;
chart.update_custom(id, |r: &mut Ranges| r.push(60_000, 1.0, 2.0))?;
let ranges: Option<&Ranges> = chart.custom_series::<Ranges>(id);
```

### 約定

| 方法 | 呼叫時機 | 必須 |
|---|---|---|
| `kind` | 加入時 | 回傳唯一名稱，例如 `"myorg.profile"`；原生 GPU 實作以此比對 |
| `axis_mode` | 加入時 | 回傳 `Slots` 或 `Overlay`（見下文） |
| `times` | 每次變動後 | 回傳嚴格遞增的時間（UTC ms）；點 `i` 對應 `times()[i]` |
| `extremes(from, to)` | 每一幀 | 以 O(log n) 回傳點 `from..to` 的價格範圍：請使用 `MinMaxIndex` |
| `build(ctx, from, to, out)` | 每一幀 | 在不配置記憶體的情況下繪製點 `from..to`（可見的點，以及兩側各多一個） |
| `last_value` | 每一幀 | 最新值標籤的數值（`None`：不顯示標籤） |
| `crosshair_value(i)` | 十字游標移動時 | 點 `i` 要顯示的數值 |
| `native_params` | 每一幀，僅限原生路徑 | 為 GPU 實作寫入最多 8 個參數 |

**座標軸模式。** `Slots`：每個點占時間軸上的一格，就像內建系列的 K 線一樣（每根 K 線一個足跡圖欄位）。`Overlay`：點依其時間放在既有的格子之間，不新增格子（一分鐘圖上的一秒熱力圖欄位）。

**修改資料。** 只能透過 `chart.update_custom(id, |s: &mut MySeries| { …; change })` 修改外掛，並回傳變動的內容：

| `DataChange` | 意義 |
|---|---|
| `UpdateLast` | 最後一個點改變了，但其時間沒有變（不配置記憶體：即時更新路徑） |
| `Append { count }` | 在尾端新增了 `count` 個點（`count: 0`：沒有變動） |
| `Prepend { count }` | 在開頭新增了 `count` 個點（歷史資料） |
| `Replace` | 其他任何情況 |

與資料不符的變動（數量錯誤、最後的時間移動了）會被當作 `Replace` 套用，因此回答錯誤只會花費時間，絕不會破壞圖表。時間若不再嚴格遞增，會回傳 `TimeNotIncreasing`，且系列在出現有效變動之前什麼都不顯示。

**極值。** `MinMaxIndex` 以 O(log n) 回答區間最小值/最大值查詢，`push` / `set_last` 會讓它保持最新。每個系列使用一個，並在修改資料的同一個 closure 中更新它。

**繪圖。** `ctx.x_of_point(i)` 是點 `i` 的中心（已套用 `SeriesOptions::shift`）。`ctx.coords()` 轉換價格與時間、提供字型並格式化價格。`ctx.bar_spacing()` 是格子之間的距離，`ctx.style()` 包含系列顏色。`FrameBuilder` 可寫入矩形、線段、折線、多邊形、文字與圖示；對齊整數像素的矩形在任何裝置像素比下都很銳利。

自訂系列不能與變換系列共用一張圖表（兩者都會決定時間軸的建構方式）。

## 原生 GPU 系列

可攜的 `build` 路徑在任何地方都能運作。當系列的資料量太大時，例如有一百萬個格子的熱力圖，可以改由原生實作在 GPU 上繪製。此時幀中會為該系列放入一個 `DrawCmd::Custom`，取代其基本圖元。

```rust
use ferrochart_render_wgpu::{GpuSeries, GpuSeriesCtx};
use ferrochart_core::chart::ChartDataView;
use ferrochart_core::frame::CustomDraw;

struct MySeriesGpu { /* pipelines, per-series buffers */ }

impl GpuSeries for MySeriesGpu {
    fn kind(&self) -> &'static str {
        "myorg.profile" // the CustomSeries::kind it draws
    }
    fn prepare(&mut self, ctx: &GpuSeriesCtx<'_>, draw: &CustomDraw, data: &ChartDataView<'_>) {
        // Upload what changed since the last frame:
        //   data.changes_since(draw.series, synced_generation)
        // Read the plugin: data.custom(draw.series), downcast through &dyn Any.
        // Compute work may be submitted to ctx.queue here.
    }
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, draw: &CustomDraw) {
        // Set the pipeline and bind groups and draw; the scissor is already
        // the pane's plot. Do not change the scissor or viewport.
    }
}

renderer.register_series(Box::new(MySeriesGpu { /* … */ }));
// Before each frame (the winit and web hosts do this for you):
chart.set_native_kinds(renderer.native_kinds());
```

使用 `ferrochart-winit` 時，請在 `ChartApp::on_renderer` 中註冊，每當 GPU 渲染器建立時都會呼叫它。

原生實作的規則：

- **輸出與可攜路徑相同。** 在外掛（核心端）中計算版面配置、門檻值與色彩範圍，並透過 `native_params` 與 `data.custom` 傳遞。shader 只負責套用。請以同一張黃金圖像測試兩條路徑。
- **檢查裝置。** WebGL2 沒有 compute shader 與 storage buffer。只在裝置支援你所需功能時才註冊。未註冊的類型會自動使用可攜路徑。
- **增量上傳。** `CustomDraw::generation` 與 `data.changes_since` 會告訴你哪些部分有變動。即時更新通常只會動到最後一個點。

`ferrochart_orderflow::register_gpu` 是一個完整的實作：由 compute shader 依像素彙總熱力圖格子，再以一個四邊形為其著色。

## 系列變換

變換會把主系列的 K 線轉換成新的類 OHLC 系列，就像內建的 Renko、Range Bars、Kagi 與點數圖一樣。它對每根輸入 K 線呼叫一次；`peek` 則顯示仍在形成中的最後一根 K 線會產生什麼。

| 方法 | 必須 |
|---|---|
| `name` | 回傳名稱，例如 `"renko"` |
| `output_kind` | 回傳 `Candlestick`、`OhlcBar`、`Kagi` 或 `PointFigure` |
| `push(bar, out)` | 提交 `bar`，並把它完成的元素寫入 `out` |
| `peek(bar, out)` | 在不改變狀態的情況下，寫入 `bar` 會完成的元素以及仍在形成中的元素 |
| `reset` | 忘掉所有輸入 |
| `box_size` | 僅限點數圖：一格的價格高度 |

```rust
let id = chart.add_transform_series(pane, Box::new(MyTransform::new()), SeriesOptions::default())?;
chart.set_data(id, SeriesData::Ohlc(bars))?;   // input bars
chart.update_last(id, tick)?;                   // live input
```

變換系列的 x 軸是它自己的輸出，而非時間，因此不能與其他以時間為基礎的系列共用一張圖表。

## 繪圖工具

繪圖工具是使用者以滑鼠放置的物件：它具有錨點（時間、價格）、用於編輯的控制點、命中測試、樣式，以及供復原/重做與狀態檔使用的儲存格式。

### 在 Rust 中

實作 `DrawingTool`（`kind`、`anchors`、`anchors_mut`、`anchor_count`、`build`、`hit_test`、`handles`、`style`、`save`；`move_handle` 可選），並註冊一個能從儲存格式重建它的工廠函式：

```rust
fn my_tool(data: &DrawingData) -> ferrochart_core::Result<Box<dyn DrawingTool>> {
    Ok(Box::new(MyTool::from_data(data)?))
}

chart.drawings().register("myorg.zone", my_tool);
```

`AnchorCount` 決定它的建立方式：固定次數的點擊、以雙擊或 Enter 結束的不定數量，或手繪。內建工具列在 `ferrochart_core::drawing::BUILTIN_KINDS` 中，是很好的參考。

### 在 JavaScript 中

```js
chart.drawings.register('myorg.zone', {
  anchorCount: 2, // or { open: 3 } or 'freehand'
  build(ctx) {
    const [a, b] = ctx.anchors;
    if (a.x === null || b.x === null) return [];
    return [{
      type: 'rect',
      x: Math.min(a.x, b.x), y: Math.min(a.y, b.y),
      width: Math.abs(b.x - a.x), height: Math.abs(b.y - a.y),
      color: ctx.style.fill ?? 'rgba(41, 98, 255, 0.2)',
    }];
  },
  // Optional; default: hit inside the drawn bounds.
  hitTest(ctx, point) {
    return false; // true: the drawing; a number: that anchor's handle
  },
});
chart.drawings.startTool('myorg.zone');
```

JavaScript 類型只在呼叫了其 `drawings.register` 的那張圖表上可用（供 `add`、`startTool` 與 `loadState` 使用）；請在每張需要它的圖表上註冊。再次註冊同一類型會取代頁面上所有圖表中的實作。內建類型無法被取代。

## 測試外掛

- **黃金圖像。** 以 `ferrochart-render-cpu`（結果確定、不需 GPU）渲染固定場景，並與儲存的 PNG 比較。若有原生實作，也以 `ferrochart-render-wgpu` 渲染，並將兩條路徑與同一張圖像比較。
- **不配置記憶體。** 先預熱幾幀，然後在平移、縮放、移動十字游標（`build_frame`）以及更新最後一個點時計算記憶體配置次數。次數應為零。`ferrochart-orderflow/tests/zero_alloc.rs` 示範了計數配置器。
- **無頭幀。** `ferrochart_core::testing::MonoMeasurer`（feature `testing`）讓你在單元測試中不需字型或 GPU 就能呼叫 `chart.build_frame`。
