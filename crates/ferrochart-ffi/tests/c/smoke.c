/* Smoke test of the C API through the generated header (scripts/test-ffi-c.sh
 * compiles it as C99 and as C++ and runs it against the shared library). */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ferrochart.h"

#define CHECK(call)                                                             \
    do {                                                                        \
        int32_t st_ = (call);                                                   \
        if (st_ < 0) {                                                          \
            fprintf(stderr, "%s:%d: %s -> %d: %s\n", __FILE__, __LINE__, #call, \
                    st_, ferrochart_last_error());                              \
            return 1;                                                           \
        }                                                                       \
    } while (0)

#define EXPECT(cond)                                                            \
    do {                                                                        \
        if (!(cond)) {                                                          \
            fprintf(stderr, "%s:%d: expected %s\n", __FILE__, __LINE__, #cond); \
            return 1;                                                           \
        }                                                                       \
    } while (0)

enum { N = 500 };

int main(void) {
    FerroChartOptions opts;
    FerroChart *chart = NULL;
    uint64_t candles = 0, ema = 0, drawing = 0;
    static int64_t t[N];
    static double o[N], h[N], l[N], c[N], v[N];
    double price = 100.0;
    uint32_t width = 0, height = 0;
    size_t stride, len;
    uint8_t *pixels;
    FerroChartInput in;
    FerroChartInputResult res;
    FerroChartEvent ev;
    int clicks = 0, i;
    char *state;
    static uint8_t avatar[16 * 16 * 4];
    uint64_t icon = 0;
    FerroChartMarker marks[2];
    FerroChartSeriesOptions so;
    FerroChartTheme theme;
    FerroChartAppearance look;
    FerroChartCrosshairStyle cross;
    const uint8_t junk[3] = {1, 2, 3};

    printf("FerroChart %s\n", ferrochart_version());

    /* Errors are codes with a message. */
    EXPECT(ferrochart_create(NULL, NULL) == FERROCHART_ERR_NULL);
    EXPECT(strlen(ferrochart_last_error()) > 0);

    memset(&opts, 0, sizeof opts);
    opts.size = sizeof opts;
    CHECK(ferrochart_options_default(&opts));
    opts.theme = FERROCHART_THEME_DARK;
    opts.width = 640;
    opts.height = 360;
    opts.dpr = 1;
    CHECK(ferrochart_create(&opts, &chart));

    for (i = 0; i < N; i++) {
        double next = price + sin(i * 0.1) * 1.5 + ((i * 7919) % 13 - 6) * 0.1;
        t[i] = 1700000000000LL + (int64_t)i * 60000;
        o[i] = price;
        c[i] = next;
        h[i] = (price > next ? price : next) + 0.4;
        l[i] = (price < next ? price : next) - 0.4;
        v[i] = 1000 + (i % 17) * 50;
        price = next;
    }
    CHECK(ferrochart_add_series(chart, FERROCHART_MAIN_PANE, "candlestick", NULL, &candles));
    CHECK(ferrochart_set_data_ohlc(chart, candles, t, o, h, l, c, v, N));
    CHECK(ferrochart_add_indicator(chart, candles, "ema", "{\"n\": 20}", FERROCHART_PLACEMENT_DEFAULT, &ema));
    CHECK(ferrochart_add_indicator(chart, candles, "rsi", NULL, FERROCHART_PLACEMENT_NEW_PANE, &ema));
    EXPECT(ferrochart_add_indicator(chart, candles, "ema", "{\"m\": 1}", -1, &ema) == FERROCHART_ERR_INVALID_ARGUMENT);
    CHECK(ferrochart_update_last(chart, candles, t[N - 1] + 60000, price, price + 1, price - 1, price + 0.5, 900));

    CHECK(ferrochart_pixel_size(chart, &width, &height));
    EXPECT(width == 640 && height == 360);
    stride = (size_t)width * 4;
    len = stride * height;
    pixels = (uint8_t *)malloc(len);
    EXPECT(pixels != NULL);
    CHECK(ferrochart_render_rgba(chart, 0, pixels, stride, len));
    EXPECT(pixels[3] == 255);

    /* A click on the plot. */
    memset(&in, 0, sizeof in);
    in.size = sizeof in;
    in.type = FERROCHART_INPUT_POINTER_DOWN;
    in.x = 300;
    in.y = 150;
    in.time_ms = 1;
    CHECK(ferrochart_handle_input(chart, &in, &res));
    in.type = FERROCHART_INPUT_POINTER_UP;
    in.time_ms = 40;
    CHECK(ferrochart_handle_input(chart, &in, &res));
    memset(&ev, 0, sizeof ev);
    ev.size = sizeof ev;
    while (ferrochart_poll_event(chart, &ev) == 1) {
        if (ev.type == FERROCHART_EVENT_CLICK) {
            clicks++;
            EXPECT(ev.has_time && ev.time >= t[0]);
        }
        ev.size = sizeof ev;
    }
    EXPECT(clicks == 1);

    CHECK(ferrochart_add_drawing(chart, FERROCHART_MAIN_PANE,
                                 "{\"kind\":\"horizontal_line\",\"anchors\":[{\"time\":1700000000000,\"price\":101}]}",
                                 &drawing));
    state = ferrochart_save_state(chart);
    EXPECT(state != NULL && strstr(state, "horizontal_line") != NULL);
    CHECK(ferrochart_load_state(chart, state));
    ferrochart_free_string(state);

    /* Image markers and a badge (r21). */
    for (i = 0; i < 16 * 16; i++) {
        avatar[i * 4] = 230;
        avatar[i * 4 + 1] = 20;
        avatar[i * 4 + 2] = 20;
        avatar[i * 4 + 3] = 255;
    }
    CHECK(ferrochart_add_icon(chart, avatar, 16, 16, 16 * 4, &icon));
    EXPECT(icon != 0);
    memset(marks, 0, sizeof marks);
    marks[0].size = sizeof marks[0];
    marks[0].time = t[N - 10];
    marks[0].icon = icon;
    marks[0].marker_size = 24;
    marks[0].border = 0x00ff00ffu;
    marks[1] = marks[0];
    marks[1].icon = 0;
    marks[1].label = "DB";
    marks[1].position = FERROCHART_MARKER_BELOW_BAR;
    CHECK(ferrochart_set_markers(chart, candles, marks, 2));
    marks[1].icon = icon;
    EXPECT(ferrochart_set_markers(chart, candles, marks, 2) == FERROCHART_ERR_INVALID_ARGUMENT);
    memset(&so, 0, sizeof so);
    so.size = sizeof so;
    CHECK(ferrochart_series_options(chart, candles, &so));
    so.marker_stack_limit = 3;
    CHECK(ferrochart_apply_series_options(chart, candles, &so));
    EXPECT(ferrochart_add_font(chart, junk, sizeof junk) == FERROCHART_ERR_INVALID_ARGUMENT);
    CHECK(ferrochart_render_rgba(chart, 8, pixels, stride, len));
    CHECK(ferrochart_remove_icon(chart, icon));

    /* Look (r25): a sparkline on a custom dark theme. */
    memset(&theme, 0, sizeof theme);
    theme.size = sizeof theme;
    CHECK(ferrochart_theme_default(1, &theme));
    theme.background = 0x000000ffu;
    CHECK(ferrochart_set_theme(chart, &theme));
    memset(&look, 0, sizeof look);
    look.size = sizeof look;
    CHECK(ferrochart_appearance(chart, &look));
    look.price_axis = look.time_axis = look.grid_horizontal = look.grid_vertical = 0;
    look.padding_top = look.padding_right = look.padding_bottom = look.padding_left = 2;
    CHECK(ferrochart_set_appearance(chart, &look));
    memset(&cross, 0, sizeof cross);
    cross.size = sizeof cross;
    CHECK(ferrochart_crosshair_style(chart, &cross));
    cross.horizontal = cross.labels = 0;
    cross.dot = 1;
    cross.dot_radius = 4;
    CHECK(ferrochart_set_crosshair_style(chart, &cross));
    look.price_axis = 7;
    EXPECT(ferrochart_set_appearance(chart, &look) == FERROCHART_ERR_INVALID_ARGUMENT);
    CHECK(ferrochart_render_rgba(chart, 12, pixels, stride, len));
    EXPECT(pixels[0] == 0 && pixels[1] == 0 && pixels[2] == 0 && pixels[3] == 255);

    if (ferrochart_needs_redraw(chart) == 1) {
        CHECK(ferrochart_render_rgba(chart, 16, pixels, stride, len));
    }
    free(pixels);
    ferrochart_destroy(chart);
    printf("ok\n");
    return 0;
}
