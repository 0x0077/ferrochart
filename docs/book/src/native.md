# C, C++ and C#

`ferrochart-ffi` is a C library with one header, `ferrochart.h`. Use it from C
and C++ directly, and from C#, Swift, Go, Qt, Flutter or any language that can
call C. The library draws into a native window you own, or into an RGBA buffer.

```bash
cargo build -p ferrochart-ffi --release
# target/release/libferrochart.{so,dylib} or ferrochart.dll, plus a static
# library; the header is crates/ferrochart-ffi/include/ferrochart.h
```

## Conventions

- Functions return `FERROCHART_OK` (0) or a negative `FERROCHART_ERR_*` code.
  `ferrochart_last_error()` describes the last failure on the calling thread.
- Structs start with a `size` field: set it to `sizeof` the struct before
  passing it in either direction. This lets a newer library read an older
  struct.
- Ids are `uint64_t`, times are UTC milliseconds, sizes are CSS pixels and
  colors are `0xRRGGBBAA`.
- A chart is not thread-safe: use it from one thread (on macOS, the main
  thread when it draws into a window).
- Strings the library returns are freed with `ferrochart_free_string`.

## A chart

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

Indicator names and parameters are those of the other languages
(`ferrochart_indicator_names()` lists them); parameters are a JSON object, or
`NULL` for the defaults.

## Drawing

Into a buffer, for example to save an image or to show it in a UI toolkit:

```c
uint32_t w, h;
ferrochart_pixel_size(chart, &w, &h);            // device pixels
uint8_t *pixels = malloc((size_t)w * h * 4);
ferrochart_render_rgba(chart, now_ms, pixels, (size_t)w * 4, (size_t)w * h * 4);
```

Into a window: pass its native handle to `ferrochart_attach_window`
(`FERROCHART_PLATFORM_WIN32` with the `HWND` and `HINSTANCE`, or
`FERROCHART_PLATFORM_APPKIT` with an `NSView*`), call `ferrochart_resize` when
the window is resized, and call `ferrochart_render` whenever
`ferrochart_needs_redraw` returns 1.

## Input and events

Forward mouse, wheel and key input with `ferrochart_handle_input`
(`FERROCHART_INPUT_*` events in CSS pixels); the chart pans, zooms, moves the
crosshair and draws with the active tool. Then read what happened:

```c
FerroChartEvent ev = { .size = sizeof ev };
while (ferrochart_poll_event(chart, &ev) == 1) {
    if (ev.type == FERROCHART_EVENT_CLICK) printf("click at %f\n", ev.price);
}
```

Destroy the chart with `ferrochart_destroy(chart)`.

## C#

`examples/csharp` in the repository has the P/Invoke declarations of the whole
header (`Native.cs`), a small wrapper that turns status codes into exceptions
(`Chart.cs`) and a demo with indicators, drawings, events and a live feed:

```bash
cargo build -p ferrochart-ffi --release
dotnet run --project examples/csharp -- out/
```
