<!-- source: d61f00164d -->
# C、C++、C#

`ferrochart-ffi`は、ヘッダー1つ（`ferrochart.h`）のCライブラリです。CとC++ からは直接、C#、Swift、Go、Qt、FlutterなどCを呼べる言語からも使えます。ライブラリは、あなたが所有するネイティブウィンドウ、またはRGBAバッファに描画します。

```bash
cargo build -p ferrochart-ffi --release
# target/release/libferrochart.{so,dylib} or ferrochart.dll, plus a static
# library; the header is crates/ferrochart-ffi/include/ferrochart.h
```

## 規約

- 関数は`FERROCHART_OK`（0）または負の`FERROCHART_ERR_*`コードを返します。`ferrochart_last_error()`は、呼び出し元スレッドでの直前の失敗を説明します。
- 構造体は`size`フィールドで始まります。どちらの方向に渡す場合も、渡す前にこれを構造体の`sizeof`に設定してください。これにより、新しいライブラリが古い構造体を読めます。
- idは`uint64_t`、時刻はUTCミリ秒、サイズはCSSピクセル、色は`0xRRGGBBAA`です。
- チャートはスレッドセーフではありません。1つのスレッドから使ってください（macOSでウィンドウに描画する場合はメインスレッド）。
- ライブラリが返す文字列は`ferrochart_free_string`で解放します。

## チャート

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

インジケーターの名前とパラメーターは他の言語と同じです（`ferrochart_indicator_names()`で一覧を取得できます）。パラメーターはJSONオブジェクトで、デフォルトを使う場合は`NULL`です。

## 描画

バッファへの描画。たとえば画像として保存したり、UIツールキットで表示したりする場合：

```c
uint32_t w, h;
ferrochart_pixel_size(chart, &w, &h);            // device pixels
uint8_t *pixels = malloc((size_t)w * h * 4);
ferrochart_render_rgba(chart, now_ms, pixels, (size_t)w * 4, (size_t)w * h * 4);
```

ウィンドウへの描画：ウィンドウのネイティブハンドルを`ferrochart_attach_window`に渡し（`HWND`と`HINSTANCE`を持つ`FERROCHART_PLATFORM_WIN32`、または`NSView*`を持つ`FERROCHART_PLATFORM_APPKIT`）、ウィンドウのリサイズ時に`ferrochart_resize`を呼び、`ferrochart_needs_redraw`が1を返すたびに`ferrochart_render`を呼びます。

## 入力とイベント

マウス、ホイール、キーの入力を`ferrochart_handle_input`（CSSピクセル単位の`FERROCHART_INPUT_*`イベント）で転送すると、チャートはパン、ズーム、クロスヘアの移動、アクティブなツールでの描画を行います。その後、何が起きたかを読み出します。

```c
FerroChartEvent ev = { .size = sizeof ev };
while (ferrochart_poll_event(chart, &ev) == 1) {
    if (ev.type == FERROCHART_EVENT_CLICK) printf("click at %f\n", ev.price);
}
```

チャートは`ferrochart_destroy(chart)`で破棄します。

## C#

リポジトリの`examples/csharp`には、ヘッダー全体のP/Invoke宣言（`Native.cs`）、ステータスコードを例外に変換する小さなラッパー（`Chart.cs`）、インジケーター、描画、イベント、リアルタイムフィードを使ったデモがあります。

```bash
cargo build -p ferrochart-ffi --release
dotnet run --project examples/csharp -- out/
```
