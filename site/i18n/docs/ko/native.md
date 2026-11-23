<!-- source: d61f00164d -->
# C, C++, C#

`ferrochart-ffi`는 헤더 하나(`ferrochart.h`)로 된 C 라이브러리입니다. C와
C++에서 바로 사용할 수 있고, C#, Swift, Go, Qt, Flutter 등 C를 호출할 수 있는
모든 언어에서 사용할 수 있습니다. 라이브러리는 사용자가 소유한 네이티브 창이나
RGBA 버퍼에 그립니다.

```bash
cargo build -p ferrochart-ffi --release
# target/release/libferrochart.{so,dylib} or ferrochart.dll, plus a static
# library; the header is crates/ferrochart-ffi/include/ferrochart.h
```

## 규칙

- 함수는 `FERROCHART_OK`(0) 또는 음수인 `FERROCHART_ERR_*` 코드를 반환합니다.
  `ferrochart_last_error()`는 호출한 스레드의 마지막 실패를 설명합니다.
- 구조체는 `size` 필드로 시작합니다. 어느 방향으로 전달하든 전달하기 전에 구조체의
  `sizeof`로 설정하세요. 이렇게 하면 새 버전의 라이브러리가 이전 버전의 구조체를
  읽을 수 있습니다.
- id는 `uint64_t`, 시간은 UTC 밀리초, 크기는 CSS 픽셀, 색상은 `0xRRGGBBAA`입니다.
- 차트는 스레드 안전하지 않습니다. 한 스레드에서만 사용하세요(macOS에서 창에 그릴
  때는 메인 스레드).
- 라이브러리가 반환한 문자열은 `ferrochart_free_string`으로 해제합니다.

## 차트

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

지표 이름과 매개변수는 다른 언어와 같습니다(`ferrochart_indicator_names()`로
목록을 얻을 수 있습니다). 매개변수는 JSON 객체이며, 기본값을 쓰려면 `NULL`을
전달합니다.

## 그리기

버퍼에 그리기(예: 이미지로 저장하거나 UI 툴킷에 표시):

```c
uint32_t w, h;
ferrochart_pixel_size(chart, &w, &h);            // device pixels
uint8_t *pixels = malloc((size_t)w * h * 4);
ferrochart_render_rgba(chart, now_ms, pixels, (size_t)w * 4, (size_t)w * h * 4);
```

창에 그리기: 창의 네이티브 핸들을 `ferrochart_attach_window`에 전달하고
(`FERROCHART_PLATFORM_WIN32`와 `HWND`, `HINSTANCE`, 또는
`FERROCHART_PLATFORM_APPKIT`과 `NSView*`), 창 크기가 바뀌면 `ferrochart_resize`를
호출하고, `ferrochart_needs_redraw`가 1을 반환할 때마다 `ferrochart_render`를
호출하세요.

## 입력과 이벤트

마우스, 휠, 키 입력을 `ferrochart_handle_input`으로 전달하세요
(`FERROCHART_INPUT_*` 이벤트, CSS 픽셀 단위). 차트가 이동, 확대·축소, 크로스헤어
이동, 활성 도구로 그리기를 처리합니다. 그런 다음 일어난 일을 읽습니다.

```c
FerroChartEvent ev = { .size = sizeof ev };
while (ferrochart_poll_event(chart, &ev) == 1) {
    if (ev.type == FERROCHART_EVENT_CLICK) printf("click at %f\n", ev.price);
}
```

차트는 `ferrochart_destroy(chart)`로 제거합니다.

## C#

저장소의 `examples/csharp`에는 헤더 전체의 P/Invoke 선언(`Native.cs`), 상태 코드를
예외로 바꾸는 작은 래퍼(`Chart.cs`), 그리고 지표, 드로잉, 이벤트, 실시간 피드를
보여 주는 데모가 있습니다.

```bash
cargo build -p ferrochart-ffi --release
dotnet run --project examples/csharp -- out/
```
