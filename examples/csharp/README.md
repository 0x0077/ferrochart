# FerroChart from C#

Uses the FerroChart C library (`ferrochart.dll` / `libferrochart.dylib` /
`libferrochart.so`) through P/Invoke:

- `Native.cs`: the declarations, one per function and struct of
  `crates/ferrochart-ffi/include/ferrochart.h`;
- `Chart.cs`: a thin wrapper that turns status codes into
  `FerroChartException`s (with the library's message) and copies and frees
  returned strings;
- `Png.cs`: a small PNG writer;
- `Program.cs`: the demo. It builds candlesticks with volume, EMA,
  Bollinger bands and an RSI pane, sends a click, draws a trend line with the
  tool and zooms with the wheel. Then it reads the events, runs a live feed
  that redraws only when the chart changed, round-trips the saved state, and
  writes `ferrochart-csharp.png`.

```sh
cargo build -p ferrochart-ffi --release
dotnet run --project examples/csharp -- out/
```

The library is loaded from `$FERROCHART_LIB` if set, otherwise from the
nearest `target/release` or `target/debug` above the program. For an app,
ship the library next to it (the default search path).

Drawing into a window: pass the native handle to `ferrochart_attach_window`
(`FERROCHART_PLATFORM_WIN32` with the `HWND` and `HINSTANCE`, or `APPKIT`
with an `NSView*`). Resize the chart with the window. Forward mouse,
wheel and key input with `ferrochart_handle_input`, and call
`ferrochart_render` when `ferrochart_needs_redraw` returns 1. The rules:

- A chart is not thread-safe; use it from one thread (on macOS, the main
  thread).
- Struct `Size` fields must be set to `Marshal.SizeOf<T>()`; the wrapper
  does this.
