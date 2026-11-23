<!-- source: e853ddc392 -->
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

## 标记、图片与字体

标记可以显示裁剪成其形状的图片，或 `"DB"` 这样的简短文字徽标。图片只需添加一次（非预乘 RGBA8，最大 256 × 256；图表会保留一份副本），之后可以在任意数量的标记中使用它的 id：

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

一根 K 线上标记很多时，请在系列选项中设置 `marker_stack_limit`：超出部分由一个“+k”徽标代表。`FERROCHART_EVENT_MARKER_HOVER` 报告指针下方的标记或徽标（`target_type`、`target_index`，以及 `x`/`y` 中的中心坐标），例如用于显示提示框；点击徽标时，`target_type` 为 `FERROCHART_TARGET_MARKER_OVERFLOW`，并带有 `count`。

内置字体包含 ASCII 和常用符号。要使用中文、日文、韩文或其他文字，请添加字体文件；字符按添加顺序在这些字体中查找，都没有时画成 `□`：

```c
// font_data: the bytes of a .ttf, .otf or .ttc file, e.g. Noto Sans SC or
// /System/Library/Fonts/Hiragino Sans GB.ttc on macOS (copied by the call).
ferrochart_add_font(chart, font_data, font_len);
```

原生端不支持彩色 emoji 字体；轮廓 emoji 字体会以文字颜色绘制。

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
