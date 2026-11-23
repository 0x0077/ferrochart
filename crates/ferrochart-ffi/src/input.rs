//! Input from the host and chart events for it:
//! `ferrochart_handle_input` and `ferrochart_poll_event`.

use ferrochart_core::chart::{ChartEvent, ClickTarget, WarningKind};
use ferrochart_core::geom::{Css, PointCss};
use ferrochart_core::interaction::{
    Button, CursorIcon, GesturePhase, InputEvent, Key, Modifiers, WheelDelta,
};

use crate::{
    Error, FERROCHART_OK, FerroChart, Result, Sized32, out_ptr, read_sized, with_chart, write,
    write_sized,
};

/// Input: pointer pressed.
pub const FERROCHART_INPUT_POINTER_DOWN: u32 = 1;
/// Input: pointer moved.
pub const FERROCHART_INPUT_POINTER_MOVE: u32 = 2;
/// Input: pointer released.
pub const FERROCHART_INPUT_POINTER_UP: u32 = 3;
/// Input: pointer left the chart.
pub const FERROCHART_INPUT_POINTER_LEAVE: u32 = 4;
/// Input: wheel or trackpad scroll (`delta_x`, `delta_y`, `delta_mode`).
pub const FERROCHART_INPUT_WHEEL: u32 = 5;
/// Input: pinch gesture (`phase`, `scale`, `x`/`y` the center).
pub const FERROCHART_INPUT_PINCH: u32 = 6;
/// Input: key pressed or released (`key`, `key_char`, `pressed`).
pub const FERROCHART_INPUT_KEY: u32 = 7;
/// Input: double click.
pub const FERROCHART_INPUT_DOUBLE_CLICK: u32 = 8;

/// Left mouse button (or touch, pen).
pub const FERROCHART_BUTTON_LEFT: u32 = 0;
/// Middle button.
pub const FERROCHART_BUTTON_MIDDLE: u32 = 1;
/// Right button.
pub const FERROCHART_BUTTON_RIGHT: u32 = 2;

/// Modifier flag: Shift.
pub const FERROCHART_MOD_SHIFT: u32 = 1;
/// Modifier flag: Control.
pub const FERROCHART_MOD_CTRL: u32 = 2;
/// Modifier flag: Alt / Option.
pub const FERROCHART_MOD_ALT: u32 = 4;
/// Modifier flag: Command / Windows key.
pub const FERROCHART_MOD_META: u32 = 8;

/// Wheel deltas in pixels.
pub const FERROCHART_DELTA_PIXELS: u32 = 0;
/// Wheel deltas in lines.
pub const FERROCHART_DELTA_LINES: u32 = 1;

/// Gesture phase: start.
pub const FERROCHART_PHASE_START: u32 = 0;
/// Gesture phase: update.
pub const FERROCHART_PHASE_UPDATE: u32 = 1;
/// Gesture phase: end.
pub const FERROCHART_PHASE_END: u32 = 2;

/// Key: a character (`key_char`).
pub const FERROCHART_KEY_CHAR: u32 = 1;
/// Key: ←.
pub const FERROCHART_KEY_LEFT: u32 = 2;
/// Key: →.
pub const FERROCHART_KEY_RIGHT: u32 = 3;
/// Key: ↑.
pub const FERROCHART_KEY_UP: u32 = 4;
/// Key: ↓.
pub const FERROCHART_KEY_DOWN: u32 = 5;
/// Key: Home.
pub const FERROCHART_KEY_HOME: u32 = 6;
/// Key: End.
pub const FERROCHART_KEY_END: u32 = 7;
/// Key: Escape.
pub const FERROCHART_KEY_ESCAPE: u32 = 8;
/// Key: Delete or Backspace.
pub const FERROCHART_KEY_DELETE: u32 = 9;
/// Key: Enter.
pub const FERROCHART_KEY_ENTER: u32 = 10;

/// Cursor: default arrow.
pub const FERROCHART_CURSOR_DEFAULT: u32 = 0;
/// Cursor: crosshair.
pub const FERROCHART_CURSOR_CROSSHAIR: u32 = 1;
/// Cursor: open hand.
pub const FERROCHART_CURSOR_GRAB: u32 = 2;
/// Cursor: closed hand.
pub const FERROCHART_CURSOR_GRABBING: u32 = 3;
/// Cursor: horizontal resize.
pub const FERROCHART_CURSOR_EW_RESIZE: u32 = 4;
/// Cursor: vertical resize.
pub const FERROCHART_CURSOR_NS_RESIZE: u32 = 5;
/// Cursor: pointing hand.
pub const FERROCHART_CURSOR_POINTER: u32 = 6;
/// Cursor: move.
pub const FERROCHART_CURSOR_MOVE: u32 = 7;

/// An input event; zero it, set `size = sizeof(FerroChartInput)` and the
/// fields its `type` uses.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FerroChartInput {
    /// `sizeof(FerroChartInput)`.
    pub size: u32,
    /// `FERROCHART_INPUT_*`.
    pub r#type: u32,
    /// Pointer id (mouse: 0; touches: distinct ids).
    pub pointer_id: u64,
    /// Position (pointer events, double click, wheel, pinch center), CSS px
    /// from the top left of the chart.
    pub x: f32,
    /// See `x`.
    pub y: f32,
    /// `FERROCHART_BUTTON_*` (down, up, double click).
    pub button: u32,
    /// `FERROCHART_MOD_*` flags.
    pub mods: u32,
    /// Event time, ms (any monotonic clock; pointer events).
    pub time_ms: f64,
    /// Wheel deltas, as the DOM's `deltaX`/`deltaY`: positive scrolls
    /// right/down (a wheel turned toward the user, which zooms out);
    /// negative `delta_y` zooms in.
    pub delta_x: f32,
    /// See `delta_x`.
    pub delta_y: f32,
    /// `FERROCHART_DELTA_*`.
    pub delta_mode: u32,
    /// `FERROCHART_PHASE_*` (pinch).
    pub phase: u32,
    /// Scale factor since the last pinch event (pinch).
    pub scale: f32,
    /// `FERROCHART_KEY_*` (key).
    pub key: u32,
    /// Unicode scalar value of a `FERROCHART_KEY_CHAR` key.
    pub key_char: u32,
    /// 1 when pressed, 0 when released (key).
    pub pressed: u8,
    /// Reserved, 0.
    pub reserved: [u8; 3],
}

// SAFETY: repr(C), u32 size first, plain numbers.
unsafe impl Sized32 for FerroChartInput {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

// SAFETY: repr(C), u32 size first, plain numbers.
unsafe impl Sized32 for FerroChartEvent {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

/// What the chart did with an input event (fixed layout).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FerroChartInputResult {
    /// 1 if the chart used the event (the host should not handle it too).
    pub consumed: u8,
    /// Reserved.
    pub reserved: [u8; 3],
    /// The cursor to show, `FERROCHART_CURSOR_*`.
    pub cursor: u32,
}

fn mods(m: u32) -> Modifiers {
    Modifiers {
        shift: m & crate::input::FERROCHART_MOD_SHIFT != 0,
        ctrl: m & FERROCHART_MOD_CTRL != 0,
        alt: m & FERROCHART_MOD_ALT != 0,
        meta: m & FERROCHART_MOD_META != 0,
    }
}

fn button(b: u32) -> Result<Button> {
    Ok(match b {
        FERROCHART_BUTTON_LEFT => Button::Left,
        FERROCHART_BUTTON_MIDDLE => Button::Middle,
        FERROCHART_BUTTON_RIGHT => Button::Right,
        b => return Err(Error::invalid(format!("unknown button {b}"))),
    })
}

fn finite(v: f32, what: &str) -> Result<f32> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(Error::invalid(format!("'{what}' must be finite")))
    }
}

fn convert(e: &FerroChartInput) -> Result<InputEvent> {
    let pos = || -> Result<PointCss> {
        Ok(PointCss {
            x: finite(e.x, "x")?,
            y: finite(e.y, "y")?,
        })
    };
    let time_ms = if e.time_ms.is_finite() {
        e.time_ms
    } else {
        0.0
    };
    Ok(match e.r#type {
        FERROCHART_INPUT_POINTER_DOWN => InputEvent::PointerDown {
            id: e.pointer_id,
            pos: pos()?,
            button: button(e.button)?,
            mods: mods(e.mods),
            time_ms,
        },
        FERROCHART_INPUT_POINTER_MOVE => InputEvent::PointerMove {
            id: e.pointer_id,
            pos: pos()?,
            mods: mods(e.mods),
            time_ms,
        },
        FERROCHART_INPUT_POINTER_UP => InputEvent::PointerUp {
            id: e.pointer_id,
            pos: pos()?,
            button: button(e.button)?,
            mods: mods(e.mods),
            time_ms,
        },
        FERROCHART_INPUT_POINTER_LEAVE => InputEvent::PointerLeave,
        FERROCHART_INPUT_WHEEL => {
            let (x, y) = (finite(e.delta_x, "delta_x")?, finite(e.delta_y, "delta_y")?);
            InputEvent::Wheel {
                pos: pos()?,
                delta: match e.delta_mode {
                    FERROCHART_DELTA_PIXELS => WheelDelta::Pixels {
                        x: Css(x),
                        y: Css(y),
                    },
                    FERROCHART_DELTA_LINES => WheelDelta::Lines { x, y },
                    m => return Err(Error::invalid(format!("unknown delta mode {m}"))),
                },
                mods: mods(e.mods),
            }
        }
        FERROCHART_INPUT_PINCH => InputEvent::Pinch {
            phase: match e.phase {
                FERROCHART_PHASE_START => GesturePhase::Start,
                FERROCHART_PHASE_UPDATE => GesturePhase::Update,
                FERROCHART_PHASE_END => GesturePhase::End,
                p => return Err(Error::invalid(format!("unknown phase {p}"))),
            },
            center: pos()?,
            scale: {
                let s = finite(e.scale, "scale")?;
                if s <= 0.0 {
                    return Err(Error::invalid("'scale' must be positive"));
                }
                s
            },
        },
        FERROCHART_INPUT_KEY => InputEvent::Key {
            key: match e.key {
                FERROCHART_KEY_CHAR => Key::Char(
                    char::from_u32(e.key_char)
                        .ok_or_else(|| Error::invalid("invalid 'key_char'"))?,
                ),
                FERROCHART_KEY_LEFT => Key::ArrowLeft,
                FERROCHART_KEY_RIGHT => Key::ArrowRight,
                FERROCHART_KEY_UP => Key::ArrowUp,
                FERROCHART_KEY_DOWN => Key::ArrowDown,
                FERROCHART_KEY_HOME => Key::Home,
                FERROCHART_KEY_END => Key::End,
                FERROCHART_KEY_ESCAPE => Key::Escape,
                FERROCHART_KEY_DELETE => Key::Delete,
                FERROCHART_KEY_ENTER => Key::Enter,
                k => return Err(Error::invalid(format!("unknown key {k}"))),
            },
            pressed: e.pressed != 0,
            mods: mods(e.mods),
        },
        FERROCHART_INPUT_DOUBLE_CLICK => InputEvent::DoubleClick {
            pos: pos()?,
            button: button(e.button)?,
        },
        t => return Err(Error::invalid(format!("unknown input type {t}"))),
    })
}

fn cursor(c: CursorIcon) -> u32 {
    match c {
        CursorIcon::Crosshair => FERROCHART_CURSOR_CROSSHAIR,
        CursorIcon::Grab => FERROCHART_CURSOR_GRAB,
        CursorIcon::Grabbing => FERROCHART_CURSOR_GRABBING,
        CursorIcon::EwResize => FERROCHART_CURSOR_EW_RESIZE,
        CursorIcon::NsResize => FERROCHART_CURSOR_NS_RESIZE,
        CursorIcon::Pointer => FERROCHART_CURSOR_POINTER,
        CursorIcon::Move => FERROCHART_CURSOR_MOVE,
        _ => FERROCHART_CURSOR_DEFAULT,
    }
}

/// Passes an input event to the chart; `out` (may be null) receives whether
/// it was used and the cursor to show. Redraw afterwards if
/// `ferrochart_needs_redraw` says so.
///
/// # Safety
///
/// `chart` is a valid chart, `event` a valid `FerroChartInput`, `out` null
/// or valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_handle_input(
    chart: *mut FerroChart,
    event: *const FerroChartInput,
    out: *mut FerroChartInputResult,
) -> i32 {
    with_chart(chart, |c| {
        if !out.is_null() {
            out_ptr(out, "out")?;
        }
        // SAFETY: valid per the contract.
        let e = unsafe { read_sized(event, FerroChartInput::default(), "event")? };
        let r = c.chart.handle_input(convert(&e)?);
        c.collect_events();
        if !out.is_null() {
            // SAFETY: checked.
            unsafe {
                write(
                    out,
                    FerroChartInputResult {
                        consumed: u8::from(r.consumed),
                        reserved: [0; 3],
                        cursor: cursor(r.cursor),
                    },
                );
            }
        }
        Ok(FERROCHART_OK)
    })
}

/// Event: the crosshair moved (`has_point`: 0 when it left the plot;
/// `logical_index`, `time`/`has_time`, `pane`, `price`, and the bar under it
/// when `has_bar`).
pub const FERROCHART_EVENT_CROSSHAIR_MOVE: u32 = 1;
/// Event: a click on a plot (`x`, `y`, `pane`, `logical_index`,
/// `time`/`has_time`, `price`, `target_type` with `target_id`/`target_index`).
pub const FERROCHART_EVENT_CLICK: u32 = 2;
/// Event: the visible logical range changed (`from`, `to`).
pub const FERROCHART_EVENT_VISIBLE_RANGE_CHANGED: u32 = 3;
/// Event: the view reached the start of the data (`bars_before_start`).
pub const FERROCHART_EVENT_NEED_MORE_HISTORY: u32 = 4;
/// Event: a drawing was created (`drawing`).
pub const FERROCHART_EVENT_DRAWING_CREATED: u32 = 5;
/// Event: a drawing was changed (`drawing`).
pub const FERROCHART_EVENT_DRAWING_MODIFIED: u32 = 6;
/// Event: a drawing was removed (`drawing`).
pub const FERROCHART_EVENT_DRAWING_REMOVED: u32 = 7;
/// Event: the selection changed (`drawing`, `has_drawing` 0 for none).
pub const FERROCHART_EVENT_DRAWING_SELECTED: u32 = 8;
/// Event: a warning (`warning`, `target_id` the series, `count`).
pub const FERROCHART_EVENT_WARNING: u32 = 9;
/// Event: the marker under the pointer changed (r21). On a marker:
/// `target_type` `FERROCHART_TARGET_MARKER`, `target_id` the series,
/// `target_index` the marker, `x`/`y` its center (CSS px); on a "+k" badge:
/// `FERROCHART_TARGET_MARKER_OVERFLOW` with `target_index` the first marker
/// not drawn and `count` = k. `FERROCHART_TARGET_NONE` when the pointer left
/// the markers.
pub const FERROCHART_EVENT_MARKER_HOVER: u32 = 10;

/// Click target: nothing.
pub const FERROCHART_TARGET_NONE: u32 = 0;
/// Click target: a series (`target_id`).
pub const FERROCHART_TARGET_SERIES: u32 = 1;
/// Click target: a marker (`target_id` the series, `target_index`).
pub const FERROCHART_TARGET_MARKER: u32 = 2;
/// Click target: a drawing (`target_id`).
pub const FERROCHART_TARGET_DRAWING: u32 = 3;
/// Click target: a "+k" badge of folded markers (r21; `target_id` the
/// series, `target_index` the first marker not drawn, `count` = k).
pub const FERROCHART_TARGET_MARKER_OVERFLOW: u32 = 4;

/// Warning: invalid OHLC bars were repaired (`target_id`, `count`).
pub const FERROCHART_WARNING_OHLC_REPAIRED: u32 = 1;
/// Warning: series with different times were merged on one axis.
pub const FERROCHART_WARNING_TIME_AXIS_MERGED: u32 = 2;
/// Warning: another kind.
pub const FERROCHART_WARNING_OTHER: u32 = 99;

/// A chart event from [`ferrochart_poll_event`]; fields not used by its
/// `type` are zero.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FerroChartEvent {
    /// `sizeof(FerroChartEvent)`.
    pub size: u32,
    /// `FERROCHART_EVENT_*`.
    pub r#type: u32,
    /// Pane id.
    pub pane: u64,
    /// Position of a click, CSS px.
    pub x: f32,
    /// See `x`.
    pub y: f32,
    /// Logical index (fractional for clicks).
    pub logical_index: f64,
    /// Time, UTC ms (valid when `has_time`).
    pub time: i64,
    /// Price at the pointer.
    pub price: f64,
    /// Open, high, low, close, volume of the bar under the crosshair (valid
    /// when `has_bar`; NaN volume: none).
    pub open: f64,
    /// See `open`.
    pub high: f64,
    /// See `open`.
    pub low: f64,
    /// See `open`.
    pub close: f64,
    /// See `open`.
    pub volume: f64,
    /// Visible range.
    pub from: f64,
    /// See `from`.
    pub to: f64,
    /// Bars before the first visible one (negative: empty space).
    pub bars_before_start: i64,
    /// Drawing id.
    pub drawing: u64,
    /// Series or drawing id of a click target or warning.
    pub target_id: u64,
    /// Marker index of a click or hover target.
    pub target_index: u64,
    /// Bars of an `FERROCHART_WARNING_OHLC_REPAIRED` warning; markers of a
    /// `FERROCHART_TARGET_MARKER_OVERFLOW` target.
    pub count: u64,
    /// `FERROCHART_TARGET_*`.
    pub target_type: u32,
    /// `FERROCHART_WARNING_*`.
    pub warning: u32,
    /// 1 if `time` is set.
    pub has_time: u8,
    /// 1 if the crosshair is on a plot.
    pub has_point: u8,
    /// 1 if a bar is under the crosshair.
    pub has_bar: u8,
    /// 1 if a drawing is selected.
    pub has_drawing: u8,
    /// Reserved.
    pub reserved: [u8; 4],
    /// `id` of the marker of a marker click or hover target (for a "+k"
    /// badge, of the first marker not drawn); 0: none (r22).
    pub marker_id: u64,
    /// First and last logical index of the bars of a marker hover, or of
    /// the bars whose markers a `FERROCHART_TARGET_MARKER_OVERFLOW` badge
    /// hides on its side (r22); 0 otherwise.
    pub bars_from: i64,
    /// See `bars_from`.
    pub bars_to: i64,
}

fn event(e: &ChartEvent) -> FerroChartEvent {
    let mut out = FerroChartEvent {
        size: size_of::<FerroChartEvent>() as u32,
        ..FerroChartEvent::default()
    };
    match e {
        ChartEvent::CrosshairMove { point } => {
            out.r#type = FERROCHART_EVENT_CROSSHAIR_MOVE;
            if let Some(p) = point {
                out.has_point = 1;
                out.logical_index = p.logical_index as f64;
                out.has_time = u8::from(p.time.is_some());
                out.time = p.time.unwrap_or_default();
                out.pane = u64::from(p.pane.0);
                out.price = p.price;
                if let Some(b) = p.bar {
                    out.has_bar = 1;
                    out.open = b.open;
                    out.high = b.high;
                    out.low = b.low;
                    out.close = b.close;
                    out.volume = b.volume.unwrap_or(f64::NAN);
                }
            }
        }
        ChartEvent::Click {
            pos,
            pane,
            logical_index,
            time,
            price,
            target,
        } => {
            out.r#type = FERROCHART_EVENT_CLICK;
            out.x = pos.x;
            out.y = pos.y;
            out.pane = u64::from(pane.0);
            out.logical_index = *logical_index;
            out.has_time = u8::from(time.is_some());
            out.time = time.unwrap_or_default();
            out.price = *price;
            match target {
                Some(ClickTarget::Series(s)) => {
                    out.target_type = FERROCHART_TARGET_SERIES;
                    out.target_id = u64::from(s.0);
                }
                Some(ClickTarget::Marker { series, index, id }) => {
                    out.target_type = FERROCHART_TARGET_MARKER;
                    out.target_id = u64::from(series.0);
                    out.target_index = *index as u64;
                    out.marker_id = *id;
                }
                Some(ClickTarget::Drawing(d)) => {
                    out.target_type = FERROCHART_TARGET_DRAWING;
                    out.target_id = d.0;
                }
                Some(ClickTarget::MarkerOverflow {
                    series,
                    first,
                    count,
                    id,
                    bars,
                }) => {
                    out.target_type = FERROCHART_TARGET_MARKER_OVERFLOW;
                    out.target_id = u64::from(series.0);
                    out.target_index = *first as u64;
                    out.count = u64::from(*count);
                    out.marker_id = *id;
                    (out.bars_from, out.bars_to) = (bars.start, bars.end - 1);
                }
                None | Some(_) => {}
            }
        }
        ChartEvent::VisibleRangeChanged { from, to } => {
            out.r#type = FERROCHART_EVENT_VISIBLE_RANGE_CHANGED;
            out.from = *from;
            out.to = *to;
        }
        ChartEvent::NeedMoreHistory { bars_before_start } => {
            out.r#type = FERROCHART_EVENT_NEED_MORE_HISTORY;
            out.bars_before_start = *bars_before_start;
        }
        ChartEvent::DrawingCreated(d) => {
            out.r#type = FERROCHART_EVENT_DRAWING_CREATED;
            out.drawing = d.0;
        }
        ChartEvent::DrawingModified(d) => {
            out.r#type = FERROCHART_EVENT_DRAWING_MODIFIED;
            out.drawing = d.0;
        }
        ChartEvent::DrawingRemoved(d) => {
            out.r#type = FERROCHART_EVENT_DRAWING_REMOVED;
            out.drawing = d.0;
        }
        ChartEvent::DrawingSelected(d) => {
            out.r#type = FERROCHART_EVENT_DRAWING_SELECTED;
            out.has_drawing = u8::from(d.is_some());
            out.drawing = d.map_or(0, |d| d.0);
        }
        ChartEvent::Warning(w) => {
            out.r#type = FERROCHART_EVENT_WARNING;
            match w {
                WarningKind::OhlcRepaired { series, bars } => {
                    out.warning = FERROCHART_WARNING_OHLC_REPAIRED;
                    out.target_id = u64::from(series.0);
                    out.count = *bars as u64;
                }
                WarningKind::TimeAxisMerged { .. } => {
                    out.warning = FERROCHART_WARNING_TIME_AXIS_MERGED;
                }
                _ => out.warning = FERROCHART_WARNING_OTHER,
            }
        }
        ChartEvent::MarkerHover(hit) => {
            out.r#type = FERROCHART_EVENT_MARKER_HOVER;
            if let Some(h) = hit {
                out.target_type = if h.overflow > 0 {
                    FERROCHART_TARGET_MARKER_OVERFLOW
                } else {
                    FERROCHART_TARGET_MARKER
                };
                out.target_id = u64::from(h.series.0);
                out.target_index = h.index as u64;
                out.count = u64::from(h.overflow);
                out.marker_id = h.id;
                (out.bars_from, out.bars_to) = (h.bars.start, h.bars.end - 1);
                out.x = h.center.x;
                out.y = h.center.y;
            }
        }
        _ => {
            out.r#type = FERROCHART_EVENT_WARNING;
            out.warning = FERROCHART_WARNING_OTHER;
        }
    }
    out
}

/// Takes the oldest pending event (set `out->size` first): returns 1 and
/// fills `*out`, or 0 when
/// there is none (or a negative status). Events come from input, frames
/// and API calls; at most 10 000 are kept (older ones are dropped).
///
/// # Safety
///
/// `chart` is a valid chart; `out` is a `FerroChartEvent` with `size` set,
/// valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_poll_event(
    chart: *mut FerroChart,
    out: *mut FerroChartEvent,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out, "out")?;
        c.collect_events();
        match c.pop_event() {
            Some(e) => {
                // SAFETY: valid per the contract.
                if let Err(err) = unsafe { write_sized(out, event(&e), "out") } {
                    c.unpop_event(e); // not delivered: keep it
                    return Err(err);
                }
                Ok(1)
            }
            None => Ok(0),
        }
    })
}
