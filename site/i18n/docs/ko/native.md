<!-- source: e853ddc392 -->
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

## 마커, 이미지, 폰트

마커는 도형 모양으로 잘린 이미지나 `"DB"` 같은 짧은 레이블을 표시할 수 있습니다.
이미지를 한 번 추가하고(straight RGBA8, 최대 256 × 256, 차트가 사본을 보관합니다)
그 id를 여러 마커에서 사용하세요.

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

한 봉에 마커가 많으면 시리즈 옵션에 `marker_stack_limit`을 설정하세요. 그 개수를
넘는 나머지는 "+k" 배지로 표시됩니다. `FERROCHART_EVENT_MARKER_HOVER`는 포인터
아래의 마커나 배지(`target_type`, `target_index`, `x`/`y`에 담긴 중심)를 보고하며,
예를 들어 툴팁을 표시하는 데 쓸 수 있습니다. 배지를 클릭하면 `target_type`이
`FERROCHART_TARGET_MARKER_OVERFLOW`이고 `count`가 함께 전달됩니다.

내장 폰트는 ASCII와 일반 기호를 지원합니다. 중국어, 일본어, 한국어 또는 다른
문자를 쓰려면 폰트 파일을 추가하세요. 문자는 추가한 폰트에서 순서대로 찾으며,
어떤 폰트에도 없으면 `□`로 그려집니다.

```c
// font_data: the bytes of a .ttf, .otf or .ttc file, e.g. Noto Sans SC or
// /System/Library/Fonts/Hiragino Sans GB.ttc on macOS (copied by the call).
ferrochart_add_font(chart, font_data, font_len);
```

네이티브에서는 컬러 이모지 폰트를 지원하지 않습니다. 윤곽선 이모지 폰트는 텍스트
색상으로 그려집니다.

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
