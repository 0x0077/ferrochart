<!-- source: d61f00164d -->
# C、C++ 与 C#

`ferrochart-ffi` 是一个只有一个头文件 `ferrochart.h` 的 C 库。C 和 C++ 可以直接使用它，C#、Swift、Go、Qt、Flutter 或任何能调用 C 的语言也都可以使用。该库可以绘制到你自己的原生窗口中，也可以绘制到 RGBA 缓冲区中。

```bash
cargo build -p ferrochart-ffi --release
# target/release/libferrochart.{so,dylib} or ferrochart.dll, plus a static
# library; the header is crates/ferrochart-ffi/include/ferrochart.h
```

## 约定

- 函数返回 `FERROCHART_OK`（0）或负数的 `FERROCHART_ERR_*` 错误码。`ferrochart_last_error()` 描述调用线程上最近一次失败。
- 结构体以 `size` 字段开头：无论数据朝哪个方向传递，传入之前都要把它设为该结构体的 `sizeof`。这使较新的库能够读取较旧的结构体。
- id 为 `uint64_t`，时间为 UTC 毫秒，尺寸为 CSS 像素，颜色为 `0xRRGGBBAA`。
- 图表不是线程安全的：请只在一个线程中使用它（在 macOS 上，绘制到窗口时须在主线程）。
- 库返回的字符串用 `ferrochart_free_string` 释放。

## 创建图表

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

指标名称和参数与其他语言相同（`ferrochart_indicator_names()` 会列出它们）；参数是一个 JSON 对象，传 `NULL` 表示使用默认值。

## 绘制

绘制到缓冲区，例如用于保存图像或在 UI 工具包中显示：

```c
uint32_t w, h;
ferrochart_pixel_size(chart, &w, &h);            // device pixels
uint8_t *pixels = malloc((size_t)w * h * 4);
ferrochart_render_rgba(chart, now_ms, pixels, (size_t)w * 4, (size_t)w * h * 4);
```

绘制到窗口：把窗口的原生句柄传给 `ferrochart_attach_window`（`FERROCHART_PLATFORM_WIN32` 配合 `HWND` 和 `HINSTANCE`，或 `FERROCHART_PLATFORM_APPKIT` 配合 `NSView*`），窗口尺寸改变时调用 `ferrochart_resize`，并在 `ferrochart_needs_redraw` 返回 1 时调用 `ferrochart_render`。

## 输入与事件

用 `ferrochart_handle_input` 转发鼠标、滚轮和键盘输入（`FERROCHART_INPUT_*` 事件，以 CSS 像素为单位）；图表会据此平移、缩放、移动十字光标，并用当前激活的工具绘制。然后读取发生了什么：

```c
FerroChartEvent ev = { .size = sizeof ev };
while (ferrochart_poll_event(chart, &ev) == 1) {
    if (ev.type == FERROCHART_EVENT_CLICK) printf("click at %f\n", ev.price);
}
```

用 `ferrochart_destroy(chart)` 销毁图表。

## C#

仓库中的 `examples/csharp` 包含整个头文件的 P/Invoke 声明（`Native.cs`）、一个把状态码转换为异常的小型封装（`Chart.cs`），以及一个包含指标、画线、事件和实时行情的演示：

```bash
cargo build -p ferrochart-ffi --release
dotnet run --project examples/csharp -- out/
```
