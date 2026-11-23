//! C ABI for FerroChart.
//!
//! A chart is an opaque `FerroChart*` from [`ferrochart_create`], freed with
//! [`ferrochart_destroy`]. Functions return a status: [`FERROCHART_OK`] (0)
//! or a negative `FERROCHART_ERR_*` code, with a message from
//! [`ferrochart_last_error`] (per thread, valid until the next failing call
//! on that thread). Null pointers give [`FERROCHART_ERR_NULL`]; nothing
//! panics across the boundary (a panic becomes [`FERROCHART_ERR_PANIC`] and
//! the chart refuses further calls except `ferrochart_destroy`).
//!
//! Conventions:
//! - ids are `uint64_t`; [`FERROCHART_MAIN_PANE`] stands for the main pane;
//! - times are UTC milliseconds (`int64_t`); sizes are CSS px with a device
//!   pixel ratio;
//! - names (series kinds, indicators, transforms, modes, drawing kinds) are
//!   the `snake_case` names of the Python binding (`heikin_ashi`, `stoch_rsi`,
//!   `indexed_to_100`, `trend_line`); parameters, drawings and states are JSON
//!   in the forms of the Python dicts (`convert.rs`);
//! - colors are `0xRRGGBBAA`; in series options 0 means the theme color;
//! - structs start with a `size` field that the caller sets to
//!   `sizeof(struct)` before passing them in either direction: fields are
//!   only ever appended, so a struct from an older header is read and
//!   written as far as its `size` (the `*_default` functions fill the
//!   rest);
//! - a chart is not thread-safe: use it from one thread at a time (a chart
//!   with a native window: from the thread that owns the window).
//!
//! The header `include/ferrochart.h` is generated with
//! `cbindgen --config cbindgen.toml --output include/ferrochart.h` (checked
//! by `scripts/check.sh`).

mod convert;
pub mod input;
mod json;
mod params;
pub mod render;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use ferrochart_core::chart::{
    ChartEvent, Marker, MarkerPosition, MarkerShape, SeriesData, SeriesOptions,
};
use ferrochart_core::data::{
    BarUpdate, DrawingId, IndicatorId, OhlcColumns, PaneId, SeriesId, ValueColumns,
};
use ferrochart_core::frame::{LineDash, SeriesKind};
use ferrochart_core::geom::{Css, SizeCss};
use ferrochart_core::icon::IconId;
use ferrochart_core::pane::PaneOptions;
use ferrochart_core::plugin::{Indicator as _, Placement};
use ferrochart_core::price_scale::ScaleSide;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};

// Status codes ---------------------------------------------------------------

/// Success.
pub const FERROCHART_OK: i32 = 0;
/// A required pointer argument is null.
pub const FERROCHART_ERR_NULL: i32 = -1;
/// An argument is out of range or malformed (see `ferrochart_last_error`).
pub const FERROCHART_ERR_INVALID_ARGUMENT: i32 = -2;
/// No series, pane, indicator or drawing has this id.
pub const FERROCHART_ERR_UNKNOWN_ID: i32 = -3;
/// A string is not valid UTF-8.
pub const FERROCHART_ERR_UTF8: i32 = -4;
/// Rendering failed (no GPU adapter, lost surface, …).
pub const FERROCHART_ERR_RENDER: i32 = -5;
/// An internal error; the chart can only be destroyed.
pub const FERROCHART_ERR_PANIC: i32 = -6;
/// An output buffer is too small.
pub const FERROCHART_ERR_BUFFER_TOO_SMALL: i32 = -7;

// Other constants ----------------------------------------------------------

/// The main pane, wherever a pane id is expected.
pub const FERROCHART_MAIN_PANE: u64 = u64::MAX;

/// Light theme.
pub const FERROCHART_THEME_LIGHT: u32 = 0;
/// Dark theme.
pub const FERROCHART_THEME_DARK: u32 = 1;

/// Solid lines.
pub const FERROCHART_LINE_SOLID: u32 = 0;
/// Dashed lines.
pub const FERROCHART_LINE_DASHED: u32 = 1;
/// Dotted lines.
pub const FERROCHART_LINE_DOTTED: u32 = 2;

/// `price_precision` for automatic decimals (the default): at least 2
/// decimals and 4 significant digits, with a subscript count of zeros for
/// small prices (`0.0₄1234` is 0.00001234).
pub const FERROCHART_PRECISION_AUTO: u32 = u32::MAX;

/// Placement of an indicator: its default.
pub const FERROCHART_PLACEMENT_DEFAULT: i64 = -1;
/// Placement of an indicator: over its source series.
pub const FERROCHART_PLACEMENT_OVERLAY: i64 = -2;
/// Placement of an indicator: a new pane below the others.
pub const FERROCHART_PLACEMENT_NEW_PANE: i64 = -3;
/// Placement of an indicator: the volume area of the source's pane.
pub const FERROCHART_PLACEMENT_VOLUME: i64 = -4;

/// Most events kept for `ferrochart_poll_event`; older ones are dropped.
const MAX_EVENTS: usize = 10_000;
/// Largest side of a chart, CSS px times the pixel ratio.
const MAX_SIDE: f32 = 16_384.0;

// Errors -----------------------------------------------------------------------

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

/// A failed call: status code and message.
pub(crate) struct Error {
    code: i32,
    message: String,
}

impl Error {
    pub(crate) fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::new(FERROCHART_ERR_INVALID_ARGUMENT, message)
    }

    fn null(what: &str) -> Self {
        Self::new(FERROCHART_ERR_NULL, format!("'{what}' is null"))
    }
}

impl From<ferrochart_core::Error> for Error {
    fn from(e: ferrochart_core::Error) -> Self {
        let code = match e {
            ferrochart_core::Error::UnknownId => FERROCHART_ERR_UNKNOWN_ID,
            _ => FERROCHART_ERR_INVALID_ARGUMENT,
        };
        Self::new(code, e.to_string())
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::invalid(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::invalid(message)
    }
}

type Result<T> = std::result::Result<T, Error>;

fn set_error(message: &str) {
    // Interior NULs cannot occur in our messages, but be safe.
    let c = CString::new(message.replace('\0', " ")).unwrap_or_default();
    LAST_ERROR.with(|e| *e.borrow_mut() = c);
}

/// Runs `f`, turning errors and panics into status codes.
fn call(f: impl FnOnce() -> Result<i32>) -> i32 {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            set_error(&e.message);
            e.code
        }
        Err(_) => {
            set_error("internal error");
            FERROCHART_ERR_PANIC
        }
    }
}

/// Runs `f` on the chart behind `chart`; a panic poisons the chart.
fn with_chart(chart: *mut FerroChart, f: impl FnOnce(&mut FerroChart) -> Result<i32>) -> i32 {
    if chart.is_null() {
        set_error("'chart' is null");
        return FERROCHART_ERR_NULL;
    }
    // SAFETY: the caller passes a chart from `ferrochart_create` that is not
    // destroyed and not used by another thread (documented contract).
    let c = unsafe { &mut *chart };
    if c.poisoned {
        set_error("the chart failed internally earlier; destroy it");
        return FERROCHART_ERR_PANIC;
    }
    match catch_unwind(AssertUnwindSafe(|| f(c))) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            set_error(&e.message);
            e.code
        }
        Err(_) => {
            // SAFETY: as above; the closure's borrow ended with the unwind.
            unsafe { (*chart).poisoned = true };
            set_error("internal error");
            FERROCHART_ERR_PANIC
        }
    }
}

/// The message of the last failed call on this thread (empty if none). The
/// string stays valid until the next failing call on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn ferrochart_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

/// The library version, e.g. `"0.4.0"` (static string).
#[unsafe(no_mangle)]
pub extern "C" fn ferrochart_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

// Argument helpers ---------------------------------------------------------------

/// A string argument.
///
/// # Safety
///
/// `p` is null or a NUL-terminated string valid for the call.
pub(crate) unsafe fn text<'a>(p: *const c_char, what: &str) -> Result<&'a str> {
    if p.is_null() {
        return Err(Error::null(what));
    }
    // SAFETY: non-null and NUL-terminated per the contract.
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|_| Error::new(FERROCHART_ERR_UTF8, format!("'{what}' is not UTF-8")))
}

/// An optional string argument (null: `None`).
///
/// # Safety
///
/// As [`text`].
unsafe fn opt_text<'a>(p: *const c_char, what: &str) -> Result<Option<&'a str>> {
    if p.is_null() {
        Ok(None)
    } else {
        // SAFETY: forwarded contract.
        unsafe { text(p, what) }.map(Some)
    }
}

/// An array argument of `len` elements (null allowed when `len` is 0).
///
/// # Safety
///
/// `p` points to `len` initialized elements valid for the call.
pub(crate) unsafe fn slice<'a, T>(p: *const T, len: usize, what: &str) -> Result<&'a [T]> {
    if len == 0 {
        return Ok(&[]);
    }
    if p.is_null() {
        return Err(Error::null(what));
    }
    if !p.is_aligned() {
        return Err(Error::invalid(format!("'{what}' is not aligned")));
    }
    if len
        .checked_mul(size_of::<T>())
        .is_none_or(|n| n > isize::MAX as usize)
    {
        return Err(Error::invalid(format!("'{what}' is too long")));
    }
    // SAFETY: non-null, aligned, `len` elements per the contract.
    Ok(unsafe { std::slice::from_raw_parts(p, len) })
}

/// Checks an output pointer before the call changes anything.
fn out_ptr<T>(p: *mut T, what: &str) -> Result<()> {
    if p.is_null() {
        Err(Error::null(what))
    } else if !p.is_aligned() {
        Err(Error::invalid(format!("'{what}' is not aligned")))
    } else {
        Ok(())
    }
}

/// Writes through an output pointer checked with [`out_ptr`].
///
/// # Safety
///
/// `p` is valid for writes of `T`.
unsafe fn write<T>(p: *mut T, v: T) {
    // SAFETY: checked non-null and aligned; valid per the contract.
    unsafe { p.write(v) };
}

/// A struct whose first field is its `size` in bytes.
///
/// # Safety
///
/// `#[repr(C)]`, `Copy`, with a `u32` `size` first and only fields for which
/// every bit pattern is valid.
pub(crate) unsafe trait Sized32: Copy {
    fn set_size(&mut self);
}

/// Reads a versioned struct: as many bytes as its `size` says (at most
/// ours), the rest from `default`.
///
/// # Safety
///
/// `p` is null or points to at least `size` readable bytes.
pub(crate) unsafe fn read_sized<T: Sized32>(p: *const T, default: T, what: &str) -> Result<T> {
    if p.is_null() {
        return Err(Error::null(what));
    }
    // SAFETY: the first field is a u32 per the contract; read unaligned to
    // be lenient.
    let size = unsafe { ptr::read_unaligned(p.cast::<u32>()) } as usize;
    if size < size_of::<u32>() {
        return Err(Error::invalid(format!(
            "'{what}->size' is not set (use the *_default function)"
        )));
    }
    let mut out = default;
    let n = size.min(size_of::<T>());
    // SAFETY: `n` bytes are readable (`n` ≤ size), `out` is a plain struct
    // of at least `n` bytes for which every bit pattern is valid.
    unsafe { ptr::copy_nonoverlapping(p.cast::<u8>(), (&raw mut out).cast::<u8>(), n) };
    out.set_size();
    Ok(out)
}

/// Writes a versioned struct: as many bytes as the caller's `size` says
/// (at most ours); `size` then holds the number of bytes written.
///
/// # Safety
///
/// `p` is null or points to a struct whose `size` field is set and that has
/// that many writable bytes.
pub(crate) unsafe fn write_sized<T: Sized32>(p: *mut T, v: T, what: &str) -> Result<()> {
    if p.is_null() {
        return Err(Error::null(what));
    }
    // SAFETY: the first field is a u32 per the contract.
    let size = unsafe { ptr::read_unaligned(p.cast::<u32>()) } as usize;
    if size < size_of::<u32>() {
        return Err(Error::invalid(format!(
            "'{what}->size' is not set (set it to sizeof the struct)"
        )));
    }
    let n = size.min(size_of::<T>());
    // SAFETY: `n` bytes are writable per the contract; `v` has at least `n`.
    unsafe {
        ptr::copy_nonoverlapping((&raw const v).cast::<u8>(), p.cast::<u8>(), n);
        ptr::write_unaligned(p.cast::<u32>(), n as u32);
    }
    Ok(())
}

fn pane_id(chart: &Chart, pane: u64) -> Result<PaneId> {
    if pane == FERROCHART_MAIN_PANE {
        return Ok(chart.main_pane());
    }
    let p =
        u32::try_from(pane).map_err(|_| Error::new(FERROCHART_ERR_UNKNOWN_ID, "unknown pane"))?;
    let p = PaneId(p);
    if chart.panes().any(|x| x == p) {
        Ok(p)
    } else {
        Err(Error::new(
            FERROCHART_ERR_UNKNOWN_ID,
            format!("unknown pane {pane}"),
        ))
    }
}

fn series_id(id: u64) -> Result<SeriesId> {
    u32::try_from(id)
        .map(SeriesId)
        .map_err(|_| Error::new(FERROCHART_ERR_UNKNOWN_ID, format!("unknown series {id}")))
}

fn indicator_id(id: u64) -> Result<IndicatorId> {
    u32::try_from(id)
        .map(IndicatorId)
        .map_err(|_| Error::new(FERROCHART_ERR_UNKNOWN_ID, format!("unknown indicator {id}")))
}

/// A string for the caller, freed with [`ferrochart_free_string`].
fn owned_string(s: String) -> *mut c_char {
    CString::new(s).map_or(ptr::null_mut(), CString::into_raw)
}

// Charts -------------------------------------------------------------------------------

/// Options of [`ferrochart_create`]; fill with [`ferrochart_options_default`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FerroChartOptions {
    /// `sizeof(FerroChartOptions)`.
    pub size: u32,
    /// `FERROCHART_THEME_LIGHT` or `FERROCHART_THEME_DARK`.
    pub theme: u32,
    /// Width, CSS px.
    pub width: f32,
    /// Height, CSS px.
    pub height: f32,
    /// Device pixels per CSS px.
    pub dpr: f32,
}

// SAFETY: repr(C), u32 size first, plain numbers.
unsafe impl Sized32 for FerroChartOptions {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

impl Default for FerroChartOptions {
    fn default() -> Self {
        Self {
            size: size_of::<Self>() as u32,
            theme: FERROCHART_THEME_LIGHT,
            width: 800.0,
            height: 450.0,
            dpr: 1.0,
        }
    }
}

/// A chart (opaque to C).
pub struct FerroChart {
    pub(crate) chart: Chart,
    pub(crate) size: SizeCss,
    pub(crate) dpr: f32,
    pub(crate) renderers: render::Renderers,
    events: VecDeque<ChartEvent>,
    poisoned: bool,
}

impl FerroChart {
    /// The oldest queued event.
    pub(crate) fn pop_event(&mut self) -> Option<ChartEvent> {
        self.events.pop_front()
    }

    /// Puts back an event that could not be delivered.
    pub(crate) fn unpop_event(&mut self, e: ChartEvent) {
        self.events.push_front(e);
    }

    /// Moves the chart's new events to the queue.
    pub(crate) fn collect_events(&mut self) {
        for e in self.chart.drain_events() {
            if matches!(e, ChartEvent::Sync(_) | ChartEvent::DrawingSync) {
                continue;
            }
            if self.events.len() == MAX_EVENTS {
                self.events.pop_front();
            }
            self.events.push_back(e);
        }
    }
}

fn check_size(w: f32, h: f32, dpr: f32) -> Result<SizeCss> {
    if !(dpr.is_finite() && (0.25..=8.0).contains(&dpr)) {
        return Err(Error::invalid(format!(
            "dpr must be between 0.25 and 8 (got {dpr})"
        )));
    }
    if !(w >= 1.0 && h >= 1.0 && w * dpr <= MAX_SIDE && h * dpr <= MAX_SIDE) {
        return Err(Error::invalid(format!(
            "size {w} × {h} at dpr {dpr} is out of range (1 CSS px to {MAX_SIDE} device px)"
        )));
    }
    Ok(SizeCss { w, h })
}

/// Fills `out` with the default options. Set `out->size =
/// sizeof(FerroChartOptions)` first.
///
/// # Safety
///
/// `out` is null or a struct with `size` set, valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_options_default(out: *mut FerroChartOptions) -> i32 {
    call(|| {
        // SAFETY: valid per the contract.
        unsafe { write_sized(out, FerroChartOptions::default(), "out")? };
        Ok(FERROCHART_OK)
    })
}

/// Creates a chart (`options` may be null: the defaults) and stores it in
/// `*out`.
///
/// # Safety
///
/// `options` is null or a valid `FerroChartOptions`; `out` is valid for
/// writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_create(
    options: *const FerroChartOptions,
    out: *mut *mut FerroChart,
) -> i32 {
    call(|| {
        out_ptr(out, "out")?;
        let o = if options.is_null() {
            FerroChartOptions::default()
        } else {
            // SAFETY: valid per the contract.
            unsafe { read_sized(options, FerroChartOptions::default(), "options")? }
        };
        let theme = match o.theme {
            FERROCHART_THEME_LIGHT => Theme::light(),
            FERROCHART_THEME_DARK => Theme::dark(),
            t => return Err(Error::invalid(format!("unknown theme {t}"))),
        };
        let size = check_size(o.width, o.height, o.dpr)?;
        let mut chart = Chart::new(ChartOptions {
            theme,
            ..ChartOptions::default()
        });
        chart.resize(size, o.dpr);
        let handle = Box::new(FerroChart {
            chart,
            size,
            dpr: o.dpr,
            renderers: render::Renderers::default(),
            events: VecDeque::new(),
            poisoned: false,
        });
        // SAFETY: checked; valid per the contract.
        unsafe { write(out, Box::into_raw(handle)) };
        Ok(FERROCHART_OK)
    })
}

/// Destroys a chart (null: nothing).
///
/// # Safety
///
/// `chart` is null or from `ferrochart_create` and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_destroy(chart: *mut FerroChart) {
    if chart.is_null() {
        return;
    }
    // A panic in a destructor must not cross the boundary; the memory is
    // then leaked.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: from `Box::into_raw` in `ferrochart_create`.
        drop(unsafe { Box::from_raw(chart) });
    }));
}

/// Resizes the chart: `width` × `height` CSS px at `dpr` device pixels per
/// CSS px (also the surface of an attached window).
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_resize(
    chart: *mut FerroChart,
    width: f32,
    height: f32,
    dpr: f32,
) -> i32 {
    with_chart(chart, |c| {
        let size = check_size(width, height, dpr)?;
        c.chart.resize(size, dpr);
        c.size = size;
        c.dpr = dpr;
        c.renderers.resize(size, dpr);
        Ok(FERROCHART_OK)
    })
}

// Panes -----------------------------------------------------------------------------------

/// The id of the main (top) pane.
///
/// # Safety
///
/// `chart` is a valid chart; `out` is valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_main_pane(chart: *mut FerroChart, out: *mut u64) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out, "out")?;
        // SAFETY: checked.
        unsafe { write(out, u64::from(c.chart.main_pane().0)) };
        Ok(FERROCHART_OK)
    })
}

/// Adds a pane below the others (`stretch`: height relative to the other
/// panes, default 1).
///
/// # Safety
///
/// `chart` is a valid chart; `out_id` is valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_add_pane(
    chart: *mut FerroChart,
    stretch: f32,
    out_id: *mut u64,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_id, "out_id")?;
        if !(stretch.is_finite() && stretch > 0.0) {
            return Err(Error::invalid("'stretch' must be positive"));
        }
        let id = c.chart.add_pane(PaneOptions {
            stretch_factor: stretch,
        });
        // SAFETY: checked.
        unsafe { write(out_id, u64::from(id.0)) };
        Ok(FERROCHART_OK)
    })
}

/// Removes a pane (not the last one) with its series and drawings.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_remove_pane(chart: *mut FerroChart, pane: u64) -> i32 {
    with_chart(chart, |c| {
        let p = pane_id(&c.chart, pane)?;
        c.chart.remove_pane(p)?;
        Ok(FERROCHART_OK)
    })
}

// Series ----------------------------------------------------------------------------------

/// Options of a series; fill with [`ferrochart_series_options_default`] or
/// read with [`ferrochart_series_options`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FerroChartSeriesOptions {
    /// `sizeof(FerroChartSeriesOptions)`.
    pub size: u32,
    /// Main color (up color of candles), `0xRRGGBBAA`; 0: theme.
    pub color: u32,
    /// Down color of candles and bars; 0: theme.
    pub down_color: u32,
    /// Fill above the line (area, baseline); 0: theme.
    pub top_fill: u32,
    /// Fill below the line (area, baseline); 0: theme.
    pub bottom_fill: u32,
    /// Line width, CSS px.
    pub line_width: f32,
    /// `FERROCHART_LINE_*`.
    pub line_style: u32,
    /// Lines as steps (0 or 1).
    pub step: u8,
    /// Last value label on the price axis (0 or 1).
    pub last_value_visible: u8,
    /// Horizontal line at the last value (0 or 1).
    pub price_line_visible: u8,
    /// Reserved, 0.
    pub reserved: u8,
    /// Base value of baseline series.
    pub base_value: f64,
    /// Shift in bars (positive: to the right).
    pub shift: i32,
    /// Decimals of price labels (0–12), or [`FERROCHART_PRECISION_AUTO`].
    pub price_precision: u32,
    /// Most markers drawn per bar and side (1–65535); beyond it a "+k"
    /// badge stands for the rest. 0: no limit (the default; r21).
    pub marker_stack_limit: u32,
}

// SAFETY: repr(C), u32 size first, plain numbers.
unsafe impl Sized32 for FerroChartSeriesOptions {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

fn color_u32(c: Option<ferrochart_core::geom::Rgba>) -> u32 {
    c.map_or(0, convert::rgba_u32)
}

fn color_opt(c: u32) -> Option<ferrochart_core::geom::Rgba> {
    (c != 0).then(|| convert::rgba(c))
}

impl FerroChartSeriesOptions {
    fn from_core(o: &SeriesOptions) -> Self {
        Self {
            size: size_of::<Self>() as u32,
            color: color_u32(o.color),
            down_color: color_u32(o.down_color),
            top_fill: color_u32(o.top_fill),
            bottom_fill: color_u32(o.bottom_fill),
            line_width: o.line_width.0,
            line_style: match o.line_style {
                LineDash::Solid => FERROCHART_LINE_SOLID,
                LineDash::Dashed => FERROCHART_LINE_DASHED,
                LineDash::Dotted => FERROCHART_LINE_DOTTED,
            },
            step: u8::from(o.step),
            last_value_visible: u8::from(o.last_value_visible),
            price_line_visible: u8::from(o.price_line_visible),
            reserved: 0,
            base_value: o.base_value,
            shift: o.shift,
            price_precision: o
                .price_precision
                .map_or(FERROCHART_PRECISION_AUTO, u32::from),
            marker_stack_limit: o.marker_stack_limit.map_or(0, u32::from),
        }
    }

    /// `base` with these options.
    fn apply(&self, base: SeriesOptions) -> Result<SeriesOptions> {
        if !(self.line_width.is_finite() && self.line_width > 0.0) {
            return Err(Error::invalid("'line_width' must be positive"));
        }
        if !self.base_value.is_finite() {
            return Err(Error::invalid("'base_value' must be finite"));
        }
        if self.price_precision > 12 && self.price_precision != FERROCHART_PRECISION_AUTO {
            return Err(Error::invalid(
                "'price_precision' must be 0–12 or FERROCHART_PRECISION_AUTO",
            ));
        }
        let marker_stack_limit = match self.marker_stack_limit {
            0 => None,
            n => Some(u16::try_from(n).map_err(|_| {
                Error::invalid("'marker_stack_limit' must be 0 (no limit) to 65535")
            })?),
        };
        if self.shift.unsigned_abs() > 1_000_000 {
            return Err(Error::invalid("'shift' is out of range"));
        }
        let flag = |v: u8, name: &str| match v {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::invalid(format!("'{name}' must be 0 or 1"))),
        };
        Ok(SeriesOptions {
            color: color_opt(self.color),
            down_color: color_opt(self.down_color),
            top_fill: color_opt(self.top_fill),
            bottom_fill: color_opt(self.bottom_fill),
            line_width: Css(self.line_width),
            line_style: match self.line_style {
                FERROCHART_LINE_SOLID => LineDash::Solid,
                FERROCHART_LINE_DASHED => LineDash::Dashed,
                FERROCHART_LINE_DOTTED => LineDash::Dotted,
                s => return Err(Error::invalid(format!("unknown line style {s}"))),
            },
            step: flag(self.step, "step")?,
            last_value_visible: flag(self.last_value_visible, "last_value_visible")?,
            price_line_visible: flag(self.price_line_visible, "price_line_visible")?,
            base_value: self.base_value,
            shift: self.shift,
            price_precision: u8::try_from(self.price_precision).ok(),
            marker_stack_limit,
            ..base
        })
    }
}

/// Fills `out` with the default series options. Set `out->size =
/// sizeof(FerroChartSeriesOptions)` first.
///
/// # Safety
///
/// `out` is null or a struct with `size` set, valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_series_options_default(
    out: *mut FerroChartSeriesOptions,
) -> i32 {
    call(|| {
        // SAFETY: valid per the contract.
        unsafe {
            write_sized(
                out,
                FerroChartSeriesOptions::from_core(&SeriesOptions::default()),
                "out",
            )?;
        }
        Ok(FERROCHART_OK)
    })
}

/// Reads options from `options` (null: defaults) on top of `base`.
///
/// # Safety
///
/// `options` is null or a valid `FerroChartSeriesOptions`.
unsafe fn series_options(
    options: *const FerroChartSeriesOptions,
    base: SeriesOptions,
) -> Result<SeriesOptions> {
    if options.is_null() {
        return Ok(base);
    }
    // SAFETY: valid per the contract.
    let o = unsafe {
        read_sized(
            options,
            FerroChartSeriesOptions::from_core(&base),
            "options",
        )?
    };
    o.apply(base)
}

/// Adds a series of `kind` (`candlestick`, `ohlc`, `heikin_ashi`, `line`,
/// `area`, `baseline`, `histogram`, `points`) to `pane`; `options` may be
/// null.
///
/// # Safety
///
/// `chart` is a valid chart, `kind` a string, `options` null or valid,
/// `out_id` valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_add_series(
    chart: *mut FerroChart,
    pane: u64,
    kind: *const c_char,
    options: *const FerroChartSeriesOptions,
    out_id: *mut u64,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_id, "out_id")?;
        // SAFETY: valid per the contract.
        let kind = unsafe { text(kind, "kind")? };
        let k = params::series_kind(&convert::camel(kind)).ok_or_else(|| {
            Error::invalid(format!(
                "unknown series kind '{kind}' (expected candlestick, ohlc, heikin_ashi, line, \
                 area, baseline, histogram or points)"
            ))
        })?;
        // SAFETY: valid per the contract.
        let o = unsafe { series_options(options, SeriesOptions::default())? };
        let p = pane_id(&c.chart, pane)?;
        let id = c.chart.add_series(p, k, o)?;
        // SAFETY: checked.
        unsafe { write(out_id, u64::from(id.0)) };
        Ok(FERROCHART_OK)
    })
}

/// Adds a volume histogram of the volume column of `source` (by default in
/// the bottom quarter of the main pane).
///
/// # Safety
///
/// `chart` is a valid chart; `out_id` is valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_add_volume_series(
    chart: *mut FerroChart,
    pane: u64,
    source: u64,
    out_id: *mut u64,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_id, "out_id")?;
        let p = pane_id(&c.chart, pane)?;
        let id = c.chart.add_series(
            p,
            SeriesKind::Volume,
            SeriesOptions::volume_of(series_id(source)?),
        )?;
        // SAFETY: checked.
        unsafe { write(out_id, u64::from(id.0)) };
        Ok(FERROCHART_OK)
    })
}

/// Adds a series that draws a transform of the OHLC bars it is given:
/// `renko`, `range_bars`, `kagi` or `point_figure`, with parameters as a JSON
/// object (null: defaults). It must be the only series of the chart.
///
/// # Safety
///
/// `chart` is a valid chart, `name` a string, `params_json` null or a string,
/// `options` null or valid, `out_id` valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_add_transform_series(
    chart: *mut FerroChart,
    pane: u64,
    name: *const c_char,
    params_json: *const c_char,
    options: *const FerroChartSeriesOptions,
    out_id: *mut u64,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_id, "out_id")?;
        // SAFETY: valid per the contract.
        let name = unsafe { text(name, "name")? };
        // SAFETY: valid per the contract.
        let p = unsafe { opt_text(params_json, "params_json")? }
            .map(json::parse)
            .transpose()?;
        let t = convert::build_params(p.as_ref(), &format!("transform '{name}'"), |get| {
            params::transform(&convert::camel(name), get)
        })?;
        // SAFETY: valid per the contract.
        let o = unsafe { series_options(options, SeriesOptions::default())? };
        let pane = pane_id(&c.chart, pane)?;
        let id = c.chart.add_transform_series(pane, t, o)?;
        // SAFETY: checked.
        unsafe { write(out_id, u64::from(id.0)) };
        Ok(FERROCHART_OK)
    })
}

/// Removes a series.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_remove_series(chart: *mut FerroChart, series: u64) -> i32 {
    with_chart(chart, |c| {
        c.chart.remove_series(series_id(series)?)?;
        Ok(FERROCHART_OK)
    })
}

/// The current options of a series (set `out->size` first).
///
/// # Safety
///
/// `chart` is a valid chart; `out` is a struct with `size` set, valid for
/// writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_series_options(
    chart: *mut FerroChart,
    series: u64,
    out: *mut FerroChartSeriesOptions,
) -> i32 {
    with_chart(chart, |c| {
        let o = c.chart.series_options(series_id(series)?).ok_or_else(|| {
            Error::new(
                FERROCHART_ERR_UNKNOWN_ID,
                format!("unknown series {series}"),
            )
        })?;
        // SAFETY: valid per the contract.
        unsafe { write_sized(out, FerroChartSeriesOptions::from_core(o), "out")? };
        Ok(FERROCHART_OK)
    })
}

/// Replaces the options of a series.
///
/// # Safety
///
/// `chart` is a valid chart; `options` is a valid `FerroChartSeriesOptions`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_apply_series_options(
    chart: *mut FerroChart,
    series: u64,
    options: *const FerroChartSeriesOptions,
) -> i32 {
    with_chart(chart, |c| {
        if options.is_null() {
            return Err(Error::null("options"));
        }
        let id = series_id(series)?;
        let base = *c.chart.series_options(id).ok_or_else(|| {
            Error::new(
                FERROCHART_ERR_UNKNOWN_ID,
                format!("unknown series {series}"),
            )
        })?;
        // SAFETY: valid per the contract.
        let o = unsafe { series_options(options, base)? };
        c.chart.set_series_options(id, o)?;
        Ok(FERROCHART_OK)
    })
}

// Markers and images (r21) -------------------------------------------------------------

/// Marker position: above the bar (the default).
pub const FERROCHART_MARKER_ABOVE_BAR: u32 = 0;
/// Marker position: below the bar.
pub const FERROCHART_MARKER_BELOW_BAR: u32 = 1;

/// Marker shape: filled circle (the default).
pub const FERROCHART_SHAPE_CIRCLE: u32 = 0;
/// Marker shape: filled square.
pub const FERROCHART_SHAPE_SQUARE: u32 = 1;
/// Marker shape: arrow pointing up.
pub const FERROCHART_SHAPE_ARROW_UP: u32 = 2;
/// Marker shape: arrow pointing down.
pub const FERROCHART_SHAPE_ARROW_DOWN: u32 = 3;

/// A marker for [`ferrochart_set_markers`]. Zero-initialize it, set `size`
/// and the fields you need: zero fields take their defaults.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FerroChartMarker {
    /// `sizeof(FerroChartMarker)`.
    pub size: u32,
    /// Reserved, 0.
    pub reserved: u32,
    /// Time of the bar, UTC ms; a time between bars snaps to the next bar.
    pub time: i64,
    /// `FERROCHART_MARKER_ABOVE_BAR` or `FERROCHART_MARKER_BELOW_BAR`.
    pub position: u32,
    /// `FERROCHART_SHAPE_*`.
    pub shape: u32,
    /// Shape and text color, `0xRRGGBBAA`; 0: the series' color.
    pub color: u32,
    /// Diameter (side of a square), 4–64 CSS px; 0: 10.
    pub marker_size: f32,
    /// Image clipped to the shape (circles and squares), an id from
    /// [`ferrochart_add_icon`]; 0: none.
    pub icon: u64,
    /// Border color inside the shape's edge, `0xRRGGBBAA`; 0: none.
    pub border: u32,
    /// Border width, CSS px; 0: 2.
    pub border_width: f32,
    /// Text next to the shape (UTF-8); null: none.
    pub text: *const c_char,
    /// Text centered in the shape (circles and squares, at most 8
    /// characters, not with `icon`), e.g. `"DB"` (UTF-8); null: none.
    pub label: *const c_char,
}

impl Default for FerroChartMarker {
    fn default() -> Self {
        Self {
            size: size_of::<Self>() as u32,
            reserved: 0,
            time: 0,
            position: FERROCHART_MARKER_ABOVE_BAR,
            shape: FERROCHART_SHAPE_CIRCLE,
            color: 0,
            marker_size: 0.0,
            icon: 0,
            border: 0,
            border_width: 0.0,
            text: ptr::null(),
            label: ptr::null(),
        }
    }
}

// SAFETY: repr(C), u32 size first; numbers and pointers (any bit pattern is
// a valid raw pointer).
unsafe impl Sized32 for FerroChartMarker {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

/// The C id of an image (core ids start at 0; C ids at 1, 0 means none).
fn icon_c(id: IconId) -> u64 {
    u64::from(id.0) + 1
}

fn icon_id(id: u64) -> Result<IconId> {
    id.checked_sub(1)
        .and_then(|i| u32::try_from(i).ok())
        .map(IconId)
        .ok_or_else(|| Error::new(FERROCHART_ERR_UNKNOWN_ID, format!("unknown icon {id}")))
}

/// A core marker from a C one.
///
/// # Safety
///
/// `text` and `label` are null or NUL-terminated strings.
unsafe fn marker(
    m: &FerroChartMarker,
    icons: ferrochart_core::icon::IconsView<'_>,
) -> Result<Marker> {
    let mut out = Marker {
        time: m.time,
        position: match m.position {
            FERROCHART_MARKER_ABOVE_BAR => MarkerPosition::AboveBar,
            FERROCHART_MARKER_BELOW_BAR => MarkerPosition::BelowBar,
            p => return Err(Error::invalid(format!("unknown marker position {p}"))),
        },
        shape: match m.shape {
            FERROCHART_SHAPE_CIRCLE => MarkerShape::Circle,
            FERROCHART_SHAPE_SQUARE => MarkerShape::Square,
            FERROCHART_SHAPE_ARROW_UP => MarkerShape::ArrowUp,
            FERROCHART_SHAPE_ARROW_DOWN => MarkerShape::ArrowDown,
            s => return Err(Error::invalid(format!("unknown marker shape {s}"))),
        },
        color: color_opt(m.color),
        border: color_opt(m.border),
        ..Marker::default()
    };
    if m.marker_size != 0.0 {
        out.size = Some(Css(m.marker_size));
    }
    if m.border_width != 0.0 {
        out.border_width = Css(m.border_width);
    }
    if m.icon != 0 {
        let id = icon_id(m.icon)?;
        if icons.get(id).is_none() {
            return Err(Error::new(
                FERROCHART_ERR_UNKNOWN_ID,
                format!("unknown icon {} (add it with ferrochart_add_icon)", m.icon),
            ));
        }
        out.icon = Some(id);
    }
    // SAFETY: forwarded contract.
    unsafe {
        opt_text(m.text, "text")?
            .unwrap_or_default()
            .clone_into(&mut out.text);
        opt_text(m.label, "label")?
            .unwrap_or_default()
            .clone_into(&mut out.label);
    }
    Ok(out)
}

/// Replaces the markers of a series with `len` markers (`len` 0 removes
/// them). Each element has `size` set; elements are `markers[0].size`
/// bytes apart (`sizeof(FerroChartMarker)` of the caller's header).
///
/// # Safety
///
/// `chart` is a valid chart; `markers` is null (with `len` 0) or points to
/// `len` markers whose strings are null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_markers(
    chart: *mut FerroChart,
    series: u64,
    markers: *const FerroChartMarker,
    len: usize,
) -> i32 {
    with_chart(chart, |c| {
        let id = series_id(series)?;
        let mut out = Vec::new();
        if len > 0 {
            if markers.is_null() {
                return Err(Error::null("markers"));
            }
            // SAFETY: the first field is a u32 per the contract.
            let stride = unsafe { ptr::read_unaligned(markers.cast::<u32>()) } as usize;
            if stride < size_of::<u32>() {
                return Err(Error::invalid(
                    "'markers->size' is not set (set it to sizeof(FerroChartMarker))",
                ));
            }
            if len
                .checked_mul(stride)
                .is_none_or(|n| n > isize::MAX as usize)
            {
                return Err(Error::invalid("'markers' is too long"));
            }
            out.try_reserve(len)
                .map_err(|_| Error::invalid("'markers' is too long"))?;
            let icons = c.chart.data_view().icons();
            for i in 0..len {
                // SAFETY: `len` elements `stride` bytes apart per the
                // contract (checked not to overflow).
                let m = unsafe {
                    // Unaligned when `stride` is: `read_sized` copies bytes.
                    let p = markers.byte_add(i * stride);
                    // Each element as long as the first, so none reads past
                    // the array.
                    if ptr::read_unaligned(p.cast::<u32>()) as usize != stride {
                        return Err(Error::invalid(format!(
                            "marker {i}: 'size' differs from markers[0].size"
                        )));
                    }
                    read_sized(p, FerroChartMarker::default(), "markers")?
                };
                // SAFETY: forwarded contract.
                out.push(unsafe { marker(&m, icons) }.map_err(|e| Error {
                    message: format!("marker {i}: {}", e.message),
                    ..e
                })?);
            }
        }
        c.chart.set_markers(id, out)?;
        Ok(FERROCHART_OK)
    })
}

/// Adds an image for markers: `width` × `height` pixels (1–256 each) of
/// straight (not premultiplied) RGBA8, rows `stride` bytes apart (at least
/// `width` × 4). Stores its id (never 0) in `*out_id`. The chart keeps a
/// resampled copy; `rgba` can be freed after the call.
///
/// # Safety
///
/// `chart` is a valid chart; `rgba` has `stride` × (`height` − 1) +
/// `width` × 4 readable bytes; `out_id` is valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_add_icon(
    chart: *mut FerroChart,
    rgba: *const u8,
    width: u32,
    height: u32,
    stride: usize,
    out_id: *mut u64,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_id, "out_id")?;
        let side = 1..=ferrochart_core::icon::MAX_IMAGE_SIDE;
        if !side.contains(&width) || !side.contains(&height) {
            return Err(Error::invalid(format!(
                "icon size {width} × {height} is out of range (1–256 px each)"
            )));
        }
        let (w, h) = (width as usize, height as usize);
        let row = w * 4;
        if stride < row {
            return Err(Error::invalid(format!("'stride' must be at least {row}")));
        }
        let len = stride
            .checked_mul(h - 1)
            .and_then(|n| n.checked_add(row))
            .ok_or_else(|| Error::invalid("'stride' is too large"))?;
        // SAFETY: `len` readable bytes per the contract.
        let data = unsafe { slice(rgba, len, "rgba")? };
        let mut pixels = Vec::with_capacity(row * h);
        for y in 0..h {
            pixels.extend_from_slice(&data[y * stride..y * stride + row]);
        }
        let id = c.chart.add_icon(ferrochart_core::icon::IconImage {
            width,
            height,
            rgba: &pixels,
        })?;
        // SAFETY: checked.
        unsafe { write(out_id, icon_c(id)) };
        Ok(FERROCHART_OK)
    })
}

/// Removes an image; markers that use it draw their shape alone.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_remove_icon(chart: *mut FerroChart, icon: u64) -> i32 {
    with_chart(chart, |c| {
        c.chart.remove_icon(icon_id(icon)?)?;
        Ok(FERROCHART_OK)
    })
}

// Data --------------------------------------------------------------------------------------

/// OHLC columns from C arrays (copied).
///
/// # Safety
///
/// Each non-null pointer has `len` elements.
#[allow(clippy::too_many_arguments)]
unsafe fn ohlc(
    time: *const i64,
    open: *const f64,
    high: *const f64,
    low: *const f64,
    close: *const f64,
    volume: *const f64,
    len: usize,
) -> Result<SeriesData> {
    // SAFETY: forwarded contract.
    unsafe {
        Ok(SeriesData::Ohlc(OhlcColumns {
            time: slice(time, len, "time")?.to_vec(),
            open: slice(open, len, "open")?.to_vec(),
            high: slice(high, len, "high")?.to_vec(),
            low: slice(low, len, "low")?.to_vec(),
            close: slice(close, len, "close")?.to_vec(),
            volume: if volume.is_null() {
                None
            } else {
                Some(slice(volume, len, "volume")?.to_vec())
            },
        }))
    }
}

/// Value columns from C arrays (copied).
///
/// # Safety
///
/// Each non-null pointer has `len` elements.
unsafe fn values(
    time: *const i64,
    value: *const f64,
    color: *const u32,
    len: usize,
) -> Result<SeriesData> {
    // SAFETY: forwarded contract.
    unsafe {
        Ok(SeriesData::Value(ValueColumns {
            time: slice(time, len, "time")?.to_vec(),
            value: slice(value, len, "value")?.to_vec(),
            color: if color.is_null() {
                None
            } else {
                Some(slice(color, len, "color")?.to_vec())
            },
        }))
    }
}

/// Replaces the data of an OHLC series (candlestick, OHLC, Heikin-Ashi,
/// transform) with `len` bars: strictly increasing UTC ms times and
/// prices; `volume` may be null.
///
/// # Safety
///
/// `chart` is a valid chart; each non-null array has `len` elements.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ferrochart_set_data_ohlc(
    chart: *mut FerroChart,
    series: u64,
    time: *const i64,
    open: *const f64,
    high: *const f64,
    low: *const f64,
    close: *const f64,
    volume: *const f64,
    len: usize,
) -> i32 {
    with_chart(chart, |c| {
        // SAFETY: forwarded contract.
        let d = unsafe { ohlc(time, open, high, low, close, volume, len)? };
        c.chart.set_data(series_id(series)?, d)?;
        Ok(FERROCHART_OK)
    })
}

/// Appends bars after the last one (same arguments as
/// [`ferrochart_set_data_ohlc`]).
///
/// # Safety
///
/// As [`ferrochart_set_data_ohlc`].
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ferrochart_append_ohlc(
    chart: *mut FerroChart,
    series: u64,
    time: *const i64,
    open: *const f64,
    high: *const f64,
    low: *const f64,
    close: *const f64,
    volume: *const f64,
    len: usize,
) -> i32 {
    with_chart(chart, |c| {
        // SAFETY: forwarded contract.
        let d = unsafe { ohlc(time, open, high, low, close, volume, len)? };
        c.chart.append(series_id(series)?, d)?;
        Ok(FERROCHART_OK)
    })
}

/// Replaces the data of a line, area, baseline, histogram or points series:
/// `len` times and values, `color` (per point `0xRRGGBBAA`, 0: the series
/// color) may be null. Only histogram series draw per-point colors, and only
/// at full detail (not once bars are merged when zoomed out).
///
/// # Safety
///
/// `chart` is a valid chart; each non-null array has `len` elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_data_value(
    chart: *mut FerroChart,
    series: u64,
    time: *const i64,
    value: *const f64,
    color: *const u32,
    len: usize,
) -> i32 {
    with_chart(chart, |c| {
        // SAFETY: forwarded contract.
        let d = unsafe { values(time, value, color, len)? };
        c.chart.set_data(series_id(series)?, d)?;
        Ok(FERROCHART_OK)
    })
}

/// Appends points after the last one (same arguments as
/// [`ferrochart_set_data_value`]).
///
/// # Safety
///
/// As [`ferrochart_set_data_value`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_append_value(
    chart: *mut FerroChart,
    series: u64,
    time: *const i64,
    value: *const f64,
    color: *const u32,
    len: usize,
) -> i32 {
    with_chart(chart, |c| {
        // SAFETY: forwarded contract.
        let d = unsafe { values(time, value, color, len)? };
        c.chart.append(series_id(series)?, d)?;
        Ok(FERROCHART_OK)
    })
}

/// Updates the last bar (same `time`) or appends one (later `time`).
/// Single-value series take `close`. `volume`: NaN for none (only for
/// series whose data has no volume).
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ferrochart_update_last(
    chart: *mut FerroChart,
    series: u64,
    time: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
) -> i32 {
    with_chart(chart, |c| {
        let bar = BarUpdate {
            time,
            open,
            high,
            low,
            close,
            volume: (!volume.is_nan()).then_some(volume),
        };
        c.chart.update_last(series_id(series)?, bar)?;
        Ok(FERROCHART_OK)
    })
}

// Indicators ------------------------------------------------------------------------------

/// Indicator names accepted by [`ferrochart_add_indicator`], comma
/// separated, in the order of the documentation (static string).
#[unsafe(no_mangle)]
pub extern "C" fn ferrochart_indicator_names() -> *const c_char {
    static NAMES: std::sync::OnceLock<CString> = std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| {
            let names: Vec<String> = params::INDICATORS
                .iter()
                .map(|n| convert::snake(n))
                .collect();
            CString::new(names.join(",")).unwrap_or_default()
        })
        .as_ptr()
}

/// Attaches indicator `name` computed from `source`, with parameters as a
/// JSON object (null: the defaults), e.g. `"{\"n\": 20}"`. `placement`: a
/// `FERROCHART_PLACEMENT_*` constant or a pane id.
///
/// # Safety
///
/// `chart` is a valid chart, `name` a string, `params_json` null or a string,
/// `out_id` valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_add_indicator(
    chart: *mut FerroChart,
    source: u64,
    name: *const c_char,
    params_json: *const c_char,
    placement: i64,
    out_id: *mut u64,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_id, "out_id")?;
        // SAFETY: valid per the contract.
        let name = unsafe { text(name, "name")? };
        // SAFETY: valid per the contract.
        let p = unsafe { opt_text(params_json, "params_json")? }
            .map(json::parse)
            .transpose()?;
        let ind = convert::build_params(p.as_ref(), &format!("indicator '{name}'"), |get| {
            params::indicator(&convert::camel(name), get)
        })?;
        let placement = match placement {
            FERROCHART_PLACEMENT_DEFAULT => ind.default_placement(),
            FERROCHART_PLACEMENT_OVERLAY => Placement::Overlay,
            FERROCHART_PLACEMENT_NEW_PANE => Placement::NewPane,
            FERROCHART_PLACEMENT_VOLUME => Placement::Volume,
            p => Placement::Pane(pane_id(
                &c.chart,
                u64::try_from(p).map_err(|_| Error::invalid(format!("invalid placement {p}")))?,
            )?),
        };
        let id = c
            .chart
            .add_indicator(series_id(source)?, Box::new(ind), placement)?;
        // SAFETY: checked.
        unsafe { write(out_id, u64::from(id.0)) };
        Ok(FERROCHART_OK)
    })
}

/// Removes an indicator with its outputs (and the pane it created).
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_remove_indicator(
    chart: *mut FerroChart,
    indicator: u64,
) -> i32 {
    with_chart(chart, |c| {
        c.chart.remove_indicator(indicator_id(indicator)?)?;
        Ok(FERROCHART_OK)
    })
}

/// The series an indicator draws: up to `capacity` ids in `out`, their
/// number in `*out_count` (which may exceed `capacity`: call again with a
/// larger buffer; `out` may be null when `capacity` is 0).
///
/// # Safety
///
/// `chart` is a valid chart; `out` has `capacity` writable elements;
/// `out_count` is valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_indicator_outputs(
    chart: *mut FerroChart,
    indicator: u64,
    out: *mut u64,
    capacity: usize,
    out_count: *mut usize,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_count, "out_count")?;
        if capacity > 0 {
            out_ptr(out, "out")?;
        }
        let ids = c
            .chart
            .indicator_series(indicator_id(indicator)?)
            .ok_or_else(|| {
                Error::new(
                    FERROCHART_ERR_UNKNOWN_ID,
                    format!("unknown indicator {indicator}"),
                )
            })?;
        for (i, id) in ids.iter().take(capacity).enumerate() {
            // SAFETY: `i` < capacity elements per the contract.
            unsafe { write(out.add(i), u64::from(id.0)) };
        }
        // SAFETY: checked.
        unsafe { write(out_count, ids.len()) };
        Ok(FERROCHART_OK)
    })
}

// View ------------------------------------------------------------------------------------

/// Shows all data.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_fit_content(chart: *mut FerroChart) -> i32 {
    with_chart(chart, |c| {
        c.chart.fit_content();
        Ok(FERROCHART_OK)
    })
}

/// Scrolls to the last bar.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_scroll_to_realtime(chart: *mut FerroChart) -> i32 {
    with_chart(chart, |c| {
        c.chart.scroll_to_realtime();
        Ok(FERROCHART_OK)
    })
}

/// Shows logical indices `from`..`to` across the plot (applied when the
/// plot width is known, i.e. at the next frame).
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_visible_logical_range(
    chart: *mut FerroChart,
    from: f64,
    to: f64,
) -> i32 {
    with_chart(chart, |c| {
        c.chart.time_scale().set_visible_range(from, to)?;
        Ok(FERROCHART_OK)
    })
}

/// The logical indices at the plot edges (fractional).
///
/// # Safety
///
/// `chart` is a valid chart; `out_from` and `out_to` are valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_visible_logical_range(
    chart: *mut FerroChart,
    out_from: *mut f64,
    out_to: *mut f64,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_from, "out_from")?;
        out_ptr(out_to, "out_to")?;
        let (from, to) = c.chart.time_scale().visible_range();
        // SAFETY: checked.
        unsafe {
            write(out_from, from);
            write(out_to, to);
        }
        Ok(FERROCHART_OK)
    })
}

/// Sets the distance between bars, CSS px.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_bar_spacing(chart: *mut FerroChart, spacing: f64) -> i32 {
    with_chart(chart, |c| {
        if !(spacing.is_finite() && spacing > 0.0) {
            return Err(Error::invalid("the bar spacing must be positive"));
        }
        let dpr = f64::from(c.dpr);
        c.chart.time_scale().set_bar_spacing(spacing * dpr);
        Ok(FERROCHART_OK)
    })
}

/// Sets the crosshair mode: `normal`, `magnet` or `hidden`.
///
/// # Safety
///
/// `chart` is a valid chart; `mode` is a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_crosshair_mode(
    chart: *mut FerroChart,
    mode: *const c_char,
) -> i32 {
    with_chart(chart, |c| {
        // SAFETY: valid per the contract.
        let mode = unsafe { text(mode, "mode")? };
        let m = params::crosshair_mode(mode).ok_or_else(|| {
            Error::invalid("crosshair mode must be 'normal', 'magnet' or 'hidden'")
        })?;
        c.chart.set_crosshair_mode(m);
        Ok(FERROCHART_OK)
    })
}

/// Sets the mode of the price scale of `pane`: `normal`, `logarithmic`,
/// `percentage` or `indexed_to_100`.
///
/// # Safety
///
/// `chart` is a valid chart; `mode` is a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_price_scale_mode(
    chart: *mut FerroChart,
    pane: u64,
    mode: *const c_char,
) -> i32 {
    with_chart(chart, |c| {
        // SAFETY: valid per the contract.
        let mode = unsafe { text(mode, "mode")? };
        let m = convert::scale_mode(mode).ok_or_else(|| {
            Error::invalid("mode must be 'normal', 'logarithmic', 'percentage' or 'indexed_to_100'")
        })?;
        let p = pane_id(&c.chart, pane)?;
        let s = c
            .chart
            .price_scale(p, ScaleSide::Right)
            .ok_or_else(|| Error::new(FERROCHART_ERR_UNKNOWN_ID, "unknown pane"))?;
        s.set_mode(m)?;
        Ok(FERROCHART_OK)
    })
}

// State and drawings ------------------------------------------------------------------------

/// Frees a string returned by this library (null: nothing).
///
/// # Safety
///
/// `s` is null or a string from this library, not freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_free_string(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: from `CString::into_raw` in `owned_string`.
        drop(unsafe { CString::from_raw(s) });
    }
}

/// The view, layout and drawings as JSON (the form of the Python binding's
/// `save_state`), to free with [`ferrochart_free_string`]; null on error.
/// Data, series and indicators are not included.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_save_state(chart: *mut FerroChart) -> *mut c_char {
    let mut out = ptr::null_mut();
    with_chart(chart, |c| {
        out = owned_string(convert::state_json(&c.chart.save_state()).to_json());
        Ok(FERROCHART_OK)
    });
    out
}

/// Loads a state from [`ferrochart_save_state`] into a chart with the same
/// panes. The drawings replace the current ones; the undo history is
/// cleared. The chart is unchanged if the state does not fit.
///
/// # Safety
///
/// `chart` is a valid chart; `json` is a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_load_state(chart: *mut FerroChart, json: *const c_char) -> i32 {
    with_chart(chart, |c| {
        // SAFETY: valid per the contract.
        let text = unsafe { text(json, "json")? };
        let s = convert::state(&json::parse(text)?)?;
        c.chart.load_state(s)?;
        Ok(FERROCHART_OK)
    })
}

/// Adds a drawing from its JSON form (`{"kind", "anchors": [{"time",
/// "price"}], "style"?, "params"?}`) on `pane`.
///
/// # Safety
///
/// `chart` is a valid chart, `json` a string, `out_id` valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_add_drawing(
    chart: *mut FerroChart,
    pane: u64,
    json: *const c_char,
    out_id: *mut u64,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_id, "out_id")?;
        // SAFETY: valid per the contract.
        let text = unsafe { text(json, "json")? };
        let d = convert::drawing(&json::parse(text)?)?;
        let p = pane_id(&c.chart, pane)?;
        let id = c.chart.drawings().add(p, &d).map_err(|e| match e {
            ferrochart_core::Error::InvalidArgument(m) if m.contains("kind") => {
                Error::invalid(format!("unknown drawing kind '{}'", d.kind))
            }
            e => e.into(),
        })?;
        // SAFETY: checked.
        unsafe { write(out_id, id.0) };
        Ok(FERROCHART_OK)
    })
}

/// The JSON form of a drawing, to free with [`ferrochart_free_string`];
/// null for an unknown id.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_drawing(chart: *mut FerroChart, drawing: u64) -> *mut c_char {
    let mut out = ptr::null_mut();
    with_chart(chart, |c| {
        let d = c.chart.drawings().data(DrawingId(drawing)).ok_or_else(|| {
            Error::new(
                FERROCHART_ERR_UNKNOWN_ID,
                format!("unknown drawing {drawing}"),
            )
        })?;
        out = owned_string(convert::drawing_json(&d).to_json());
        Ok(FERROCHART_OK)
    });
    out
}

/// Removes a drawing.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_remove_drawing(chart: *mut FerroChart, drawing: u64) -> i32 {
    with_chart(chart, |c| {
        c.chart
            .drawings()
            .remove(DrawingId(drawing))
            .map_err(|e| match e {
                ferrochart_core::Error::UnknownId => Error::new(
                    FERROCHART_ERR_UNKNOWN_ID,
                    format!("unknown drawing {drawing}"),
                ),
                e => e.into(),
            })?;
        Ok(FERROCHART_OK)
    })
}

/// Arms a drawing tool: the next clicks (`ferrochart_handle_input`) create a
/// drawing of `kind` (e.g. `trend_line`).
///
/// # Safety
///
/// `chart` is a valid chart; `kind` is a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_start_tool(chart: *mut FerroChart, kind: *const c_char) -> i32 {
    with_chart(chart, |c| {
        // SAFETY: valid per the contract.
        let kind = unsafe { text(kind, "kind")? };
        c.chart
            .drawings()
            .start_tool(kind)
            .map_err(|_| Error::invalid(format!("unknown drawing kind '{kind}'")))?;
        Ok(FERROCHART_OK)
    })
}

/// Cancels the armed tool or the drawing being created.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_cancel_tool(chart: *mut FerroChart) -> i32 {
    with_chart(chart, |c| {
        c.chart.drawings().cancel_tool();
        Ok(FERROCHART_OK)
    })
}

/// Undoes the last drawing change: 1 if there was one, 0 if not.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_undo(chart: *mut FerroChart) -> i32 {
    with_chart(chart, |c| Ok(i32::from(c.chart.drawings().undo())))
}

/// Redoes the last undone drawing change: 1 if there was one, 0 if not.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_redo(chart: *mut FerroChart) -> i32 {
    with_chart(chart, |c| Ok(i32::from(c.chart.drawings().redo())))
}
