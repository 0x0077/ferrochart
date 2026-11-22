//! Behaviour of the C ABI, called as a C host would (through the extern
//! functions with raw pointers).

// Every call is an `unsafe` FFI call with pointers to locals that outlive it,
// written the way a C host would (`&mut x` as `*mut T`).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::undocumented_unsafe_blocks,
    clippy::borrow_as_ptr
)]

use std::ffi::{CStr, CString, c_char};
use std::ptr;

use ferrochart::input::*;
use ferrochart::render::*;
use ferrochart::*;

const HOUR: i64 = 3_600_000;

fn last_error() -> String {
    unsafe { CStr::from_ptr(ferrochart_last_error()) }
        .to_string_lossy()
        .into_owned()
}

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

/// Takes a string from the library.
fn take(p: *mut c_char) -> String {
    assert!(!p.is_null(), "null string: {}", last_error());
    let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_owned();
    unsafe { ferrochart_free_string(p) };
    s
}

struct Chart(*mut FerroChart);

impl Drop for Chart {
    fn drop(&mut self) {
        unsafe { ferrochart_destroy(self.0) };
    }
}

fn chart(theme: u32, w: f32, h: f32, dpr: f32) -> Chart {
    let mut o = FerroChartOptions {
        size: size_of::<FerroChartOptions>() as u32,
        theme: 0,
        width: 0.0,
        height: 0.0,
        dpr: 0.0,
    };
    assert_eq!(unsafe { ferrochart_options_default(&mut o) }, FERROCHART_OK);
    o.theme = theme;
    o.width = w;
    o.height = h;
    o.dpr = dpr;
    let mut p = ptr::null_mut();
    assert_eq!(
        unsafe { ferrochart_create(&o, &mut p) },
        FERROCHART_OK,
        "{}",
        last_error()
    );
    Chart(p)
}

/// Random-walk bars (deterministic).
fn bars(n: usize) -> [Vec<f64>; 5] {
    let mut x = 100.0_f64;
    let mut seed = 7_u64;
    let mut out: [Vec<f64>; 5] = Default::default();
    for _ in 0..n {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let r = (seed >> 33) as f64 / f64::from(1_u32 << 31) - 0.5;
        let open = x;
        x += r * 2.0;
        out[0].push(open);
        out[1].push(open.max(x) + 0.5);
        out[2].push(open.min(x) - 0.5);
        out[3].push(x);
        out[4].push(1000.0 + r * 500.0);
    }
    out
}

fn candles(ch: &Chart, n: usize) -> (u64, Vec<i64>) {
    let mut s = 0;
    let st = unsafe {
        ferrochart_add_series(
            ch.0,
            FERROCHART_MAIN_PANE,
            c("candlestick").as_ptr(),
            ptr::null(),
            &mut s,
        )
    };
    assert_eq!(st, FERROCHART_OK, "{}", last_error());
    let time: Vec<i64> = (0..n as i64).map(|i| i * HOUR).collect();
    let [o, h, l, cl, v] = bars(n);
    let st = unsafe {
        ferrochart_set_data_ohlc(
            ch.0,
            s,
            time.as_ptr(),
            o.as_ptr(),
            h.as_ptr(),
            l.as_ptr(),
            cl.as_ptr(),
            v.as_ptr(),
            n,
        )
    };
    assert_eq!(st, FERROCHART_OK, "{}", last_error());
    (s, time)
}

fn render(ch: &Chart) -> (u32, u32, Vec<u8>) {
    let (mut w, mut h) = (0, 0);
    assert_eq!(
        unsafe { ferrochart_pixel_size(ch.0, &mut w, &mut h) },
        FERROCHART_OK
    );
    let stride = w as usize * 4 + 16; // padded rows
    let mut buf = vec![0_u8; stride * h as usize];
    let st = unsafe { ferrochart_render_rgba(ch.0, 0.0, buf.as_mut_ptr(), stride, buf.len()) };
    assert_eq!(st, FERROCHART_OK, "{}", last_error());
    (w, h, buf)
}

#[test]
fn version_and_names() {
    let v = unsafe { CStr::from_ptr(ferrochart_version()) }
        .to_str()
        .unwrap();
    assert_eq!(v, env!("CARGO_PKG_VERSION"));
    let names = unsafe { CStr::from_ptr(ferrochart_indicator_names()) }
        .to_str()
        .unwrap();
    let names: Vec<&str> = names.split(',').collect();
    assert_eq!(names.len(), 30);
    assert!(names.contains(&"stoch_rsi") && names.contains(&"volume_ma"));
}

#[test]
fn null_and_invalid_arguments_are_errors() {
    let mut out = ptr::null_mut();
    unsafe {
        assert_eq!(
            ferrochart_create(ptr::null(), ptr::null_mut()),
            FERROCHART_ERR_NULL
        );
        assert!(last_error().contains("'out'"));
        assert_eq!(
            ferrochart_resize(ptr::null_mut(), 1.0, 1.0, 1.0),
            FERROCHART_ERR_NULL
        );
        ferrochart_destroy(ptr::null_mut());
        ferrochart_free_string(ptr::null_mut());
        // A struct whose size was not set.
        let o = FerroChartOptions {
            size: 0,
            theme: 0,
            width: 100.0,
            height: 100.0,
            dpr: 1.0,
        };
        assert_eq!(
            ferrochart_create(&o, &mut out),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert!(last_error().contains("size"), "{}", last_error());
        let o = FerroChartOptions {
            size: size_of::<FerroChartOptions>() as u32,
            theme: 7,
            width: 100.0,
            height: 100.0,
            dpr: 1.0,
        };
        assert_eq!(
            ferrochart_create(&o, &mut out),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert!(out.is_null());
    }
    let ch = chart(FERROCHART_THEME_LIGHT, 400.0, 300.0, 1.0);
    let mut id = 0;
    unsafe {
        assert_eq!(
            ferrochart_resize(ch.0, 0.0, 10.0, 1.0),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ferrochart_resize(ch.0, 10.0, 10.0, f32::NAN),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ferrochart_add_series(
                ch.0,
                FERROCHART_MAIN_PANE,
                c("pie").as_ptr(),
                ptr::null(),
                &mut id
            ),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert!(last_error().contains("'pie'"));
        assert_eq!(
            ferrochart_add_series(ch.0, 77, c("line").as_ptr(), ptr::null(), &mut id),
            FERROCHART_ERR_UNKNOWN_ID
        );
        // Nothing is added when the out pointer is null.
        assert_eq!(
            ferrochart_add_series(
                ch.0,
                FERROCHART_MAIN_PANE,
                c("line").as_ptr(),
                ptr::null(),
                ptr::null_mut()
            ),
            FERROCHART_ERR_NULL
        );
        assert_eq!(ferrochart_remove_series(ch.0, 0), FERROCHART_ERR_UNKNOWN_ID);
        assert_eq!(
            ferrochart_remove_series(ch.0, u64::MAX),
            FERROCHART_ERR_UNKNOWN_ID
        );
        let bad = [0xff_u8, 0];
        assert_eq!(
            ferrochart_add_series(
                ch.0,
                FERROCHART_MAIN_PANE,
                bad.as_ptr().cast(),
                ptr::null(),
                &mut id
            ),
            FERROCHART_ERR_UTF8
        );
        assert_eq!(
            ferrochart_set_crosshair_mode(ch.0, c("sideways").as_ptr()),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
    }
}

#[test]
fn data_series_options_and_errors() {
    let ch = chart(FERROCHART_THEME_DARK, 400.0, 300.0, 1.0);
    let (s, time) = candles(&ch, 100);
    unsafe {
        // Times must increase.
        let t = [5 * HOUR, 4 * HOUR];
        let p = [1.0, 1.0];
        assert_eq!(
            ferrochart_append_ohlc(
                ch.0,
                s,
                t.as_ptr(),
                p.as_ptr(),
                p.as_ptr(),
                p.as_ptr(),
                p.as_ptr(),
                ptr::null(),
                2
            ),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        // A null column with a length.
        assert_eq!(
            ferrochart_set_data_ohlc(
                ch.0,
                s,
                ptr::null(),
                p.as_ptr(),
                p.as_ptr(),
                p.as_ptr(),
                p.as_ptr(),
                ptr::null(),
                2
            ),
            FERROCHART_ERR_NULL
        );
        assert!(last_error().contains("'time'"));
        // Update the last bar, then append one.
        let last = *time.last().unwrap();
        assert_eq!(
            ferrochart_update_last(ch.0, s, last, 100.0, 103.0, 99.0, 102.0, 7.0),
            FERROCHART_OK,
            "{}",
            last_error()
        );
        // The data has volumes, so updates need one.
        assert_eq!(
            ferrochart_update_last(ch.0, s, last, 100.0, 103.0, 99.0, 102.0, f64::NAN),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ferrochart_update_last(ch.0, s, last + HOUR, 102.0, 104.0, 101.0, 103.0, 5.0),
            FERROCHART_OK
        );

        // Options round trip; an older (shorter) struct is read as far as it goes.
        let mut o: FerroChartSeriesOptions = std::mem::zeroed();
        o.size = size_of::<FerroChartSeriesOptions>() as u32;
        assert_eq!(ferrochart_series_options(ch.0, s, &mut o), FERROCHART_OK);
        assert_eq!(o.color, 0);
        assert_eq!(o.price_precision, FERROCHART_PRECISION_AUTO);
        o.color = 0x2196_F3FF;
        o.line_width = 2.0;
        o.price_precision = 3;
        assert_eq!(ferrochart_apply_series_options(ch.0, s, &o), FERROCHART_OK);
        let mut back: FerroChartSeriesOptions = std::mem::zeroed();
        back.size = size_of::<FerroChartSeriesOptions>() as u32;
        ferrochart_series_options(ch.0, s, &mut back);
        assert_eq!(
            (back.color, back.line_width, back.price_precision),
            (0x2196_F3FF, 2.0, 3)
        );
        let mut short = o;
        short.size = 8; // only `size` and `color`
        short.color = 0xFF00_00FF;
        short.line_width = -5.0; // beyond the size: ignored
        assert_eq!(
            ferrochart_apply_series_options(ch.0, s, &short),
            FERROCHART_OK,
            "{}",
            last_error()
        );
        ferrochart_series_options(ch.0, s, &mut back);
        assert_eq!((back.color, back.line_width), (0xFF00_00FF, 2.0));
        o.line_style = 9;
        assert_eq!(
            ferrochart_apply_series_options(ch.0, s, &o),
            FERROCHART_ERR_INVALID_ARGUMENT
        );

        // Value series with per-point colors.
        let mut line = 0;
        let mut lo: FerroChartSeriesOptions = std::mem::zeroed();
        lo.size = size_of::<FerroChartSeriesOptions>() as u32;
        ferrochart_series_options_default(&mut lo);
        lo.color = 0xFF98_00FF;
        assert_eq!(
            ferrochart_add_series(
                ch.0,
                FERROCHART_MAIN_PANE,
                c("line").as_ptr(),
                &lo,
                &mut line
            ),
            FERROCHART_OK
        );
        let v: Vec<f64> = (0..100).map(|i| 100.0 + f64::from(i) * 0.1).collect();
        let colors: Vec<u32> = (0..100)
            .map(|i| if i % 2 == 0 { 0 } else { 0x00FF_00FF })
            .collect();
        assert_eq!(
            ferrochart_set_data_value(ch.0, line, time.as_ptr(), v.as_ptr(), colors.as_ptr(), 100),
            FERROCHART_OK
        );
        let t = [time[99] + HOUR];
        assert_eq!(
            ferrochart_append_value(ch.0, line, t.as_ptr(), v.as_ptr(), ptr::null(), 1),
            FERROCHART_OK
        );
        // Empty data with null pointers is fine.
        assert_eq!(
            ferrochart_set_data_value(ch.0, line, ptr::null(), ptr::null(), ptr::null(), 0),
            FERROCHART_OK
        );
    }
}

#[test]
fn indicators_panes_and_transforms() {
    let ch = chart(FERROCHART_THEME_LIGHT, 600.0, 400.0, 1.0);
    let (s, _) = candles(&ch, 200);
    unsafe {
        let mut ind = 0;
        assert_eq!(
            ferrochart_add_indicator(
                ch.0,
                s,
                c("ema").as_ptr(),
                c(r#"{"n": 20}"#).as_ptr(),
                FERROCHART_PLACEMENT_DEFAULT,
                &mut ind
            ),
            FERROCHART_OK,
            "{}",
            last_error()
        );
        let mut macd = 0;
        assert_eq!(
            ferrochart_add_indicator(
                ch.0,
                s,
                c("macd").as_ptr(),
                ptr::null(),
                FERROCHART_PLACEMENT_NEW_PANE,
                &mut macd
            ),
            FERROCHART_OK
        );
        let mut count = 0;
        assert_eq!(
            ferrochart_indicator_outputs(ch.0, macd, ptr::null_mut(), 0, &mut count),
            FERROCHART_OK
        );
        assert_eq!(count, 3);
        let mut ids = [0_u64; 3];
        assert_eq!(
            ferrochart_indicator_outputs(ch.0, macd, ids.as_mut_ptr(), 3, &mut count),
            FERROCHART_OK
        );
        assert!(ids.iter().all(|&i| i != 0) && ids[0] != ids[1]);
        // Unknown and malformed parameters.
        assert_eq!(
            ferrochart_add_indicator(
                ch.0,
                s,
                c("stoch_rsi").as_ptr(),
                c(r#"{"rsi_n": 10, "colour": 1}"#).as_ptr(),
                -1,
                &mut ind
            ),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert!(
            last_error().contains("'colour'") && last_error().contains("rsi_n"),
            "{}",
            last_error()
        );
        assert_eq!(
            ferrochart_add_indicator(
                ch.0,
                s,
                c("sma").as_ptr(),
                c(r#"{"n": 2.5}"#).as_ptr(),
                -1,
                &mut ind
            ),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ferrochart_add_indicator(
                ch.0,
                s,
                c("sma").as_ptr(),
                c("{n: 2}").as_ptr(),
                -1,
                &mut ind
            ),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert!(last_error().contains("JSON"), "{}", last_error());
        assert_eq!(
            ferrochart_add_indicator(ch.0, s, c("nope").as_ptr(), ptr::null(), -1, &mut ind),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ferrochart_add_indicator(ch.0, s, c("sma").as_ptr(), ptr::null(), 99, &mut ind),
            FERROCHART_ERR_UNKNOWN_ID
        );
        assert_eq!(ferrochart_remove_indicator(ch.0, macd), FERROCHART_OK);
        assert_eq!(
            ferrochart_remove_indicator(ch.0, macd),
            FERROCHART_ERR_UNKNOWN_ID
        );

        let mut pane = 0;
        assert_eq!(ferrochart_add_pane(ch.0, 0.5, &mut pane), FERROCHART_OK);
        let mut main = 0;
        ferrochart_main_pane(ch.0, &mut main);
        assert_ne!(pane, main);
        assert_eq!(
            ferrochart_set_price_scale_mode(ch.0, pane, c("indexed_to_100").as_ptr()),
            FERROCHART_OK
        );
        assert_eq!(
            ferrochart_set_price_scale_mode(ch.0, pane, c("log").as_ptr()),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert_eq!(ferrochart_remove_pane(ch.0, pane), FERROCHART_OK);
    }
    // A transform series has a chart of its own.
    let t = chart(FERROCHART_THEME_LIGHT, 400.0, 300.0, 1.0);
    unsafe {
        let mut r = 0;
        assert_eq!(
            ferrochart_add_transform_series(
                t.0,
                FERROCHART_MAIN_PANE,
                c("renko").as_ptr(),
                c(r#"{"box_size": 2}"#).as_ptr(),
                ptr::null(),
                &mut r
            ),
            FERROCHART_OK,
            "{}",
            last_error()
        );
        assert_eq!(
            ferrochart_add_transform_series(
                t.0,
                FERROCHART_MAIN_PANE,
                c("renko").as_ptr(),
                c(r#"{"box": 2}"#).as_ptr(),
                ptr::null(),
                &mut r
            ),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
    }
}

#[test]
fn renders_pixels_offscreen() {
    let ch = chart(FERROCHART_THEME_DARK, 320.0, 200.0, 2.0);
    candles(&ch, 120);
    let (w, h, buf) = render(&ch);
    assert_eq!((w, h), (640, 400));
    let stride = w as usize * 4 + 16;
    // Opaque, and not a single color.
    let px = |x: usize, y: usize| &buf[y * stride + x * 4..y * stride + x * 4 + 4];
    assert_eq!(px(0, 0)[3], 255);
    let distinct: std::collections::HashSet<&[u8]> = (0..h as usize)
        .step_by(7)
        .flat_map(|y| (0..w as usize).step_by(7).map(move |x| (x, y)))
        .map(|(x, y)| px(x, y))
        .collect();
    assert!(distinct.len() > 10, "{} colors", distinct.len());
    // Padding bytes are not written.
    assert!(buf[w as usize * 4..stride].iter().all(|&b| b == 0));
    // A too small buffer.
    let mut small = vec![0_u8; 100];
    unsafe {
        assert_eq!(
            ferrochart_render_rgba(ch.0, 0.0, small.as_mut_ptr(), 2560, 100),
            FERROCHART_ERR_BUFFER_TOO_SMALL
        );
        assert_eq!(
            ferrochart_render_rgba(ch.0, 0.0, small.as_mut_ptr(), 4, 1 << 30),
            FERROCHART_ERR_BUFFER_TOO_SMALL
        );
        assert_eq!(
            ferrochart_render(ch.0, 0.0),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert!(last_error().contains("no window"));
    }
}

fn input(kind: u32, x: f32, y: f32) -> FerroChartInput {
    FerroChartInput {
        size: size_of::<FerroChartInput>() as u32,
        r#type: kind,
        x,
        y,
        ..FerroChartInput::default()
    }
}

fn events(ch: &Chart) -> Vec<FerroChartEvent> {
    let mut out = Vec::new();
    loop {
        let mut e = FerroChartEvent {
            size: size_of::<FerroChartEvent>() as u32,
            ..FerroChartEvent::default()
        };
        match unsafe { ferrochart_poll_event(ch.0, &mut e) } {
            1 => out.push(e),
            0 => return out,
            st => panic!("poll_event: {st} {}", last_error()),
        }
    }
}

#[test]
fn input_events_and_drawings() {
    let ch = chart(FERROCHART_THEME_LIGHT, 600.0, 400.0, 1.0);
    let (s, _) = candles(&ch, 100);
    render(&ch);
    events(&ch);
    let mut r = FerroChartInputResult::default();
    unsafe {
        // Crosshair over the plot.
        assert_eq!(
            ferrochart_handle_input(
                ch.0,
                &input(FERROCHART_INPUT_POINTER_MOVE, 300.0, 200.0),
                &mut r
            ),
            FERROCHART_OK
        );
        assert_eq!(r.cursor, FERROCHART_CURSOR_CROSSHAIR);
        assert_eq!(ferrochart_needs_redraw(ch.0), 1);
        render(&ch);
        let ev = events(&ch);
        let m = ev
            .iter()
            .find(|e| e.r#type == FERROCHART_EVENT_CROSSHAIR_MOVE)
            .expect("crosshair event");
        assert_eq!((m.has_point, m.has_bar, m.has_time), (1, 1, 1));
        assert!(m.time % HOUR == 0 && m.close > 0.0);

        // A click.
        let mut down = input(FERROCHART_INPUT_POINTER_DOWN, 300.0, 200.0);
        down.time_ms = 10.0;
        let mut up = input(FERROCHART_INPUT_POINTER_UP, 300.0, 200.0);
        up.time_ms = 50.0;
        ferrochart_handle_input(ch.0, &down, ptr::null_mut());
        ferrochart_handle_input(ch.0, &up, ptr::null_mut());
        let click = events(&ch)
            .into_iter()
            .find(|e| e.r#type == FERROCHART_EVENT_CLICK)
            .expect("click");
        assert_eq!((click.x, click.y), (300.0, 200.0));
        assert!(click.logical_index > 0.0 && click.has_time == 1);
        let _ = s;

        // Draw a horizontal line with the tool: one click.
        assert_eq!(
            ferrochart_start_tool(ch.0, c("horizontal_line").as_ptr()),
            FERROCHART_OK
        );
        assert_eq!(
            ferrochart_start_tool(ch.0, c("spiral").as_ptr()),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        ferrochart_start_tool(ch.0, c("horizontal_line").as_ptr());
        ferrochart_handle_input(ch.0, &down, ptr::null_mut());
        ferrochart_handle_input(ch.0, &up, ptr::null_mut());
        let created = events(&ch)
            .into_iter()
            .find(|e| e.r#type == FERROCHART_EVENT_DRAWING_CREATED)
            .expect("created");
        let json = take(ferrochart_drawing(ch.0, created.drawing));
        assert!(
            json.starts_with(r#"{"kind":"horizontal_line","anchors":[{"time":"#),
            "{json}"
        );
        // Escape key, undo, redo.
        let mut key = input(FERROCHART_INPUT_KEY, 0.0, 0.0);
        key.key = FERROCHART_KEY_ESCAPE;
        key.pressed = 1;
        assert_eq!(
            ferrochart_handle_input(ch.0, &key, ptr::null_mut()),
            FERROCHART_OK
        );
        key.key = 99;
        assert_eq!(
            ferrochart_handle_input(ch.0, &key, ptr::null_mut()),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert_eq!(ferrochart_undo(ch.0), 1);
        assert!(ferrochart_drawing(ch.0, created.drawing).is_null());
        assert_eq!(ferrochart_redo(ch.0), 1);
        assert_eq!(ferrochart_redo(ch.0), 0);
        // Add one from JSON, remove it.
        let mut id = 0;
        let d = c(
            r##"{"kind":"trend_line","anchors":[{"time":0,"price":100},{"time":36000000,"price":105}],"style":{"line":"#e91e63"}}"##,
        );
        assert_eq!(
            ferrochart_add_drawing(ch.0, FERROCHART_MAIN_PANE, d.as_ptr(), &mut id),
            FERROCHART_OK,
            "{}",
            last_error()
        );
        assert!(take(ferrochart_drawing(ch.0, id)).contains(r##""line":"#e91e63""##));
        assert_eq!(ferrochart_remove_drawing(ch.0, id), FERROCHART_OK);
        assert_eq!(
            ferrochart_remove_drawing(ch.0, id),
            FERROCHART_ERR_UNKNOWN_ID
        );
        let bad = c(r#"{"kind":"spiral","anchors":[]}"#);
        assert_eq!(
            ferrochart_add_drawing(ch.0, FERROCHART_MAIN_PANE, bad.as_ptr(), &mut id),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert!(last_error().contains("spiral"));

        // Wheel zoom changes the visible range.
        let (mut from, mut to) = (0.0, 0.0);
        ferrochart_visible_logical_range(ch.0, &mut from, &mut to);
        let mut wheel = input(FERROCHART_INPUT_WHEEL, 300.0, 200.0);
        wheel.delta_y = -200.0;
        ferrochart_handle_input(ch.0, &wheel, ptr::null_mut());
        render(&ch);
        let (mut f2, mut t2) = (0.0, 0.0);
        ferrochart_visible_logical_range(ch.0, &mut f2, &mut t2);
        assert!((t2 - f2) < (to - from), "{from}..{to} → {f2}..{t2}");
        assert!(
            events(&ch)
                .iter()
                .any(|e| e.r#type == FERROCHART_EVENT_VISIBLE_RANGE_CHANGED)
        );

        // An event struct without its size set is refused, the event kept.
        ferrochart_handle_input(
            ch.0,
            &input(FERROCHART_INPUT_POINTER_MOVE, 100.0, 100.0),
            ptr::null_mut(),
        );
        render(&ch);
        let mut e = FerroChartEvent::default();
        assert_eq!(
            ferrochart_poll_event(ch.0, &mut e),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert!(!events(&ch).is_empty());
    }
}

#[test]
fn state_round_trips_as_json() {
    let ch = chart(FERROCHART_THEME_LIGHT, 600.0, 400.0, 1.0);
    let (_, time) = candles(&ch, 100);
    let mut pane = 0;
    let mut id = 0;
    unsafe {
        ferrochart_add_pane(ch.0, 0.5, &mut pane);
        ferrochart_set_bar_spacing(ch.0, 9.0);
        ferrochart_set_crosshair_mode(ch.0, c("magnet").as_ptr());
        ferrochart_set_price_scale_mode(ch.0, FERROCHART_MAIN_PANE, c("logarithmic").as_ptr());
        let d = c(&format!(
            r#"{{"kind":"text","anchors":[{{"time":{},"price":101}}],"params":{{"text":"note \"1\""}}}}"#,
            time[10]
        ));
        assert_eq!(
            ferrochart_add_drawing(ch.0, pane, d.as_ptr(), &mut id),
            FERROCHART_OK,
            "{}",
            last_error()
        );
    }
    let state = take(unsafe { ferrochart_save_state(ch.0) });
    assert!(
        state.starts_with(r#"{"version":1,"bar_spacing":9,"right_offset":"#),
        "{state}"
    );
    assert!(
        state.contains(r#""crosshair_mode":"magnet""#) && state.contains(r#""mode":"logarithmic""#)
    );
    assert!(state.contains(r#""text":"note \"1\"""#));

    let other = chart(FERROCHART_THEME_DARK, 500.0, 300.0, 2.0);
    candles(&other, 100);
    unsafe {
        // Different panes: refused, unchanged.
        let before = take(ferrochart_save_state(other.0));
        assert_eq!(
            ferrochart_load_state(other.0, c(&state).as_ptr()),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert_eq!(take(ferrochart_save_state(other.0)), before);
        ferrochart_add_pane(other.0, 1.0, &mut pane);
        assert_eq!(
            ferrochart_load_state(other.0, c(&state).as_ptr()),
            FERROCHART_OK,
            "{}",
            last_error()
        );
        assert_eq!(take(ferrochart_save_state(other.0)), state);
        assert_eq!(
            ferrochart_load_state(other.0, c("[]").as_ptr()),
            FERROCHART_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ferrochart_load_state(other.0, ptr::null()),
            FERROCHART_ERR_NULL
        );
    }
    // The Python binding's form (spacing and lists as Python's json.dumps
    // writes them) loads too.
    let python = r##"{"version": 1, "bar_spacing": 9.0, "right_offset": 3.0, "crosshair_mode": "normal",
        "panes": [{"stretch": 1.0, "price_scale": {"mode": "normal", "inverted": false, "margins": [0.1, 0.2], "range": null}},
                  {"stretch": 0.5, "price_scale": {"mode": "indexed_to_100", "inverted": true, "margins": [0.1, 0.1], "range": [1.0, 2.0]}}],
        "drawings": [{"pane": 1, "visible": true, "locked": false, "data": {"kind": "horizontal_line",
            "anchors": [{"time": 0, "price": 100.0}], "style": {"line": "rgba(255, 0, 0, 0.502)", "line_width": 1.0,
            "line_dash": "solid", "fill": "#2962ff", "text": "#2962ff", "font_size": 12.0}, "params": {}}}]}"##;
    unsafe {
        assert_eq!(
            ferrochart_load_state(other.0, c(python).as_ptr()),
            FERROCHART_OK,
            "{}",
            last_error()
        );
    }
    let back = take(unsafe { ferrochart_save_state(other.0) });
    assert!(
        back.contains(r#""range":[1,2]"#) && back.contains(r#""line":"rgba(255, 0, 0, 0.502)""#),
        "{back}"
    );
}
