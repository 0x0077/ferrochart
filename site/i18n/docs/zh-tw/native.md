<!-- source: 824d4ea76a -->
# C、C++ 與 C#

`ferrochart-ffi` 是一個只有單一標頭檔 `ferrochart.h` 的 C 函式庫。可以直接從 C 與 C++ 使用，也可以從 C#、Swift、Go、Qt、Flutter 或任何能呼叫 C 的語言使用。此函式庫可以繪製到你擁有的原生視窗中，或繪製到 RGBA 緩衝區。

```bash
cargo build -p ferrochart-ffi --release
# target/release/libferrochart.{so,dylib} or ferrochart.dll, plus a static
# library; the header is crates/ferrochart-ffi/include/ferrochart.h
```

## 慣例

- 函式傳回 `FERROCHART_OK`（0）或負數的 `FERROCHART_ERR_*` 代碼。`ferrochart_last_error()` 會說明呼叫端執行緒上最近一次的失敗。
- 結構以 `size` 欄位開頭：無論傳入或傳出，都請先把它設為該結構的 `sizeof`。這讓較新的函式庫能夠讀取較舊的結構。
- id 為 `uint64_t`，時間為 UTC 毫秒，尺寸為 CSS 像素，顏色為 `0xRRGGBBAA`。
- 圖表不是執行緒安全的：請只在一個執行緒上使用它（在 macOS 上，繪製到視窗時必須是主執行緒）。
- 函式庫傳回的字串以 `ferrochart_free_string` 釋放。

## 一個圖表

```c
#include "ferrochart.h"

FerroChartOptions o = { .size = sizeof o };
ferrochart_options_default(&o);
o.theme = FERROCHART_THEME_DARK;
o.width = 1200; o.height = 700; o.dpr = 2;

FerroChart *chart;
if (ferrochart_create(&o, &chart) != FERROCHART_OK) {
    fprintf(stderr, "%s\n", ferrochart_last_error());
    return 1;
}

uint64_t candles, volume, ema;
ferrochart_add_series(chart, FERROCHART_MAIN_PANE, "candlestick", NULL, &candles);
ferrochart_add_volume_series(chart, FERROCHART_MAIN_PANE, candles, &volume);
ferrochart_set_data_ohlc(chart, candles, time, open, high, low, close, vol, n);
ferrochart_add_indicator(chart, candles, "ema", "{\"n\": 20}", FERROCHART_PLACEMENT_DEFAULT, &ema);
```

指標名稱與參數與其他語言相同（`ferrochart_indicator_names()` 會列出它們）；參數是一個 JSON 物件，傳入 `NULL` 則使用預設值。

## 繪製

繪製到緩衝區，例如用於儲存影像或在 UI 工具組中顯示：

```c
uint32_t w, h;
ferrochart_pixel_size(chart, &w, &h);            // device pixels
uint8_t *pixels = malloc((size_t)w * h * 4);
ferrochart_render_rgba(chart, now_ms, pixels, (size_t)w * 4, (size_t)w * h * 4);
```

繪製到視窗：把視窗的原生控制代碼傳給 `ferrochart_attach_window`（`FERROCHART_PLATFORM_WIN32` 搭配 `HWND` 與 `HINSTANCE`，或 `FERROCHART_PLATFORM_APPKIT` 搭配 `NSView*`），在視窗調整大小時呼叫 `ferrochart_resize`，並在 `ferrochart_needs_redraw` 傳回 1 時呼叫 `ferrochart_render`。

## 標記、圖片與字型

標記可以顯示裁切成其形狀的圖片，或簡短的標籤，例如 `"DB"`。圖片只需加入一次（非預乘 RGBA8，最大 256 × 256；圖表會保留一份副本），即可在任意數量的標記中使用其 id：

```c
uint64_t avatar;
ferrochart_add_icon(chart, rgba, 64, 64, 64 * 4, &avatar);

FerroChartMarker m[2] = {0};                  // zero fields are defaults
m[0].size = sizeof m[0];
m[0].time = t;
m[0].icon = avatar;
m[0].marker_size = 24;                        // CSS px, 4–64
m[0].border = 0x2fbf71ff;
m[1] = m[0];
m[1].icon = 0;
m[1].label = "DB";                            // an image or a label, not both
m[1].position = FERROCHART_MARKER_BELOW_BAR;
ferrochart_set_markers(chart, candles, m, 2); // replaces the series' markers
```

同一根 K 棒上有很多標記時，請在系列選項中設定 `marker_stack_limit`：超過上限的部分由一個「+k」徽章代表。`FERROCHART_EVENT_MARKER_HOVER` 會回報指標下方的標記或徽章（`target_type`、`target_index`，以及其中心 `x`/`y`），例如用來顯示提示框；點擊徽章時，`target_type` 為 `FERROCHART_TARGET_MARKER_OVERFLOW`，並帶有 `count`。

`FerroChartMarker.id` 是自訂的 `uint64_t` 識別碼（0 表示無）；標記點擊和懸停事件透過 `marker_id` 傳回它。請在宿主端維護自有物件的查找表。要將標記放在某個價格處，請將 `position` 設為 `FERROCHART_MARKER_AT_PRICE`，並將 `price` 設為以系列單位表示的有限值；此標記不參與堆疊或聚合。其他位置會忽略 C 的 `price` 欄位。

`FerroChartSeriesOptions.marker_cluster` 設定以 CSS px 表示的跨 K 棒聚合距離；0 停用聚合。徽章事件包含 `bars_from` 和 `bars_to`，表示包含兩端的邏輯 K 棒範圍；`marker_id` 是第一個隱藏標記的 id。參見[聚合](markers.md#clustering-across-bars)。

要設定品質旗標，請向 `ferrochart_set_bar_flags` 傳入時間戳記和旗標陣列，使用 `FERROCHART_BAR_FILLED` 和 `FERROCHART_BAR_CLIPPED` 位元。`FerroChartSeriesOptions.filled_bar_color` 以 `0xRRGGBBAA` 設定補齊 K 棒的空心顏色（0 使用主色，不透明度為 40%）。參見 [K 棒品質旗標](data.md#bar-quality-flags)。

內建字型涵蓋 ASCII 與常用符號。若要顯示中文、日文、韓文或其他文字，請加入字型檔案；字元會依序在已加入的字型中查找，所有字型都沒有時畫成 `□`：

```c
// font_data: the bytes of a .ttf, .otf or .ttc file, e.g. Noto Sans SC or
// /System/Library/Fonts/Hiragino Sans GB.ttc on macOS (copied by the call).
ferrochart_add_font(chart, font_data, font_len);
```

原生環境不支援彩色 emoji 字型；輪廓 emoji 字型會以文字顏色繪製。

## 輸入與事件

以 `ferrochart_handle_input` 轉發滑鼠、觸控、滾輪與按鍵輸入（`FERROCHART_INPUT_*` 事件，以 CSS 像素表示）；圖表會平移、縮放、移動十字游標，並以啟用中的工具繪圖。觸控輸入需將 `pointer_kind` 設為 `FERROCHART_POINTER_TOUCH`，為每根手指指定各自的 `pointer_id`，並在系統接管觸控時傳送 `FERROCHART_INPUT_POINTER_CANCEL`；圖表就會套用[觸控手勢](interaction.md#touch)。來自舊版標頭檔的 `FerroChartInput`（`size` 較小）會被視為滑鼠輸入。接著讀取發生了什麼：

```c
FerroChartEvent ev = { .size = sizeof ev };
while (ferrochart_poll_event(chart, &ev) == 1) {
    if (ev.type == FERROCHART_EVENT_CLICK) printf("click at %f\n", ev.price);
}
```

以 `ferrochart_destroy(chart)` 銷毀圖表。

## C#

儲存庫中的 `examples/csharp` 包含整個標頭檔的 P/Invoke 宣告（`Native.cs`）、一個把狀態碼轉為例外的小型包裝（`Chart.cs`），以及一個含有指標、繪圖、事件與即時資料源的示範：

```bash
cargo build -p ferrochart-ffi --release
dotnet run --project examples/csharp -- out/
```
