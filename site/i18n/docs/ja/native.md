<!-- source: 824d4ea76a -->
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

## マーカー、画像、フォント

マーカーには、図形でクリップした画像や、`"DB"`のような短いラベルを表示できます。画像は一度だけ追加し（ストレートRGBA8、最大256 × 256。チャートはコピーを保持します）、そのidをいくつのマーカーでも使えます。

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

1本の足に多数のマーカーがある場合は、シリーズオプションで`marker_stack_limit`を設定します。それを超えた分は「+k」バッジでまとめて表されます。`FERROCHART_EVENT_MARKER_HOVER`はポインターの下にあるマーカーまたはバッジ（`target_type`、`target_index`、`x`/`y`にその中心）を通知するので、たとえばツールチップの表示に使えます。バッジをクリックすると、`target_type`が`FERROCHART_TARGET_MARKER_OVERFLOW`になり、`count`が設定されます。

`FerroChartMarker.id`はアプリケーション側の`uint64_t`識別子です（0はなし）。マーカーのクリックとホバーのイベントは、これを`marker_id`として返します。独自のオブジェクトとの対応はアプリケーション側で保持してください。指定した価格にマーカーを置くには、`position`を`FERROCHART_MARKER_AT_PRICE`、`price`をシリーズの単位で有限の値に設定します。積み重ねやクラスタリングの対象にはなりません。他の位置では、Cの`price`フィールドは無視されます。

`FerroChartSeriesOptions.marker_cluster`は、複数の足にまたがるクラスタリングの距離をCSS pxで設定します。0で無効になります。バッジのイベントには、両端を含む足の論理インデックス範囲を示す`bars_from`と`bars_to`が含まれ、`marker_id`は描画されなかった最初のマーカーのidです。[クラスタリング](markers.md#clustering-across-bars)を参照してください。

品質フラグには、タイムスタンプとフラグの配列を渡して`ferrochart_set_bar_flags`を呼び、`FERROCHART_BAR_FILLED`と`FERROCHART_BAR_CLIPPED`のビットを使います。`FerroChartSeriesOptions.filled_bar_color`は、補完された中空の足の色を`0xRRGGBBAA`で設定します（0ではメインカラーを不透明度40%で使います）。[足の品質フラグ](data.md#bar-quality-flags)を参照してください。

同梱フォントはASCIIと一般的な記号に対応しています。中国語、日本語、韓国語やその他の文字体系には、フォントファイルを追加します。文字は追加したフォントから順に検索され、どのフォントにもない場合は`□`として描画されます。

```c
// font_data: the bytes of a .ttf, .otf or .ttc file, e.g. Noto Sans SC or
// /System/Library/Fonts/Hiragino Sans GB.ttc on macOS (copied by the call).
ferrochart_add_font(chart, font_data, font_len);
```

ネイティブではカラー絵文字フォントには対応していません。アウトライン絵文字フォントはテキストの色で描画されます。

## 入力とイベント

マウス、タッチ、ホイール、キーの入力を`ferrochart_handle_input`（CSSピクセル単位の`FERROCHART_INPUT_*`イベント）で転送すると、チャートはパン、ズーム、クロスヘアの移動、アクティブなツールでの描画を行います。タッチでは、`pointer_kind`を`FERROCHART_POINTER_TOUCH`に設定し、指ごとに固有の`pointer_id`を割り当て、システムがタッチ操作を引き継いだときは`FERROCHART_INPUT_POINTER_CANCEL`を送ります。これによりチャートは[タッチ操作](interaction.md#touch)を適用します。古いヘッダーの`FerroChartInput`（`size`が小さいもの）はマウスとして扱われます。その後、何が起きたかを読み出します。

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
