//! Rendering for C hosts: into a pixel buffer
//! (CPU renderer, any thread, no GPU needed) or into a native window the
//! host owns (wgpu surface from its raw handles).

use std::ffi::c_void;
use std::num::NonZeroIsize;
use std::ptr::NonNull;

use ferrochart_core::frame::Invalidation;
use ferrochart_core::geom::{SizeCss, SizePx};
use ferrochart_core::render::Renderer;
use ferrochart_render_cpu::CpuRenderer;
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle,
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
    Win32WindowHandle, WindowsDisplayHandle, XlibDisplayHandle, XlibWindowHandle,
};

use crate::{
    Error, FERROCHART_ERR_BUFFER_TOO_SMALL, FERROCHART_ERR_RENDER, FERROCHART_OK, FerroChart,
    Result, Sized32, out_ptr, read_sized, with_chart, write,
};

/// Window system: macOS (`window`: the `NSView*` to draw into).
pub const FERROCHART_PLATFORM_APPKIT: u32 = 1;
/// Window system: Windows (`window`: the `HWND`; `display`: the
/// `HINSTANCE`, may be null).
pub const FERROCHART_PLATFORM_WIN32: u32 = 2;
/// Window system: X11 through Xlib (`window`: the `Window` id; `display`:
/// the `Display*`; `screen`).
pub const FERROCHART_PLATFORM_XLIB: u32 = 3;
/// Window system: Wayland (`window`: the `wl_surface*`; `display`: the
/// `wl_display*`).
pub const FERROCHART_PLATFORM_WAYLAND: u32 = 4;

/// A native window to draw into (see `FERROCHART_PLATFORM_*`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FerroChartWindow {
    /// `sizeof(FerroChartWindow)`.
    pub size: u32,
    /// `FERROCHART_PLATFORM_*`.
    pub platform: u32,
    /// The window or view (see the platform).
    pub window: *mut c_void,
    /// The display connection (see the platform).
    pub display: *mut c_void,
    /// X11 screen number.
    pub screen: i32,
}

impl Default for FerroChartWindow {
    fn default() -> Self {
        Self {
            size: size_of::<Self>() as u32,
            platform: 0,
            window: std::ptr::null_mut(),
            display: std::ptr::null_mut(),
            screen: 0,
        }
    }
}

// SAFETY: repr(C), u32 size first; pointers and numbers (any bit pattern
// is a valid raw pointer).
unsafe impl Sized32 for FerroChartWindow {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

/// A display handle given by the host, for the wgpu instance.
#[derive(Debug)]
struct HostDisplay(RawDisplayHandle);

// SAFETY: a raw display handle is an address the host keeps valid while
// the chart has the window (contract of `ferrochart_attach_window`); wgpu
// only reads it.
unsafe impl Send for HostDisplay {}
// SAFETY: as above.
unsafe impl Sync for HostDisplay {}

impl HasDisplayHandle for HostDisplay {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        // SAFETY: valid while the window is attached (see above).
        Ok(unsafe { DisplayHandle::borrow_raw(self.0) })
    }
}

/// A window the chart draws into.
struct Window {
    // Field order is drop order: the renderer (with the surface) before
    // the device and instance.
    renderer: WgpuRenderer,
    _ctx: GpuContext,
    /// Nothing drawn into it yet: the next `ferrochart_render` draws even
    /// if the chart did not change.
    fresh: bool,
}

/// The renderers of a chart.
#[derive(Default)]
pub(crate) struct Renderers {
    cpu: Option<CpuRenderer>,
    window: Option<Window>,
}

fn device_size(size: SizeCss, dpr: f32) -> SizePx {
    SizePx {
        w: (size.w * dpr).round().max(1.0),
        h: (size.h * dpr).round().max(1.0),
    }
}

impl Renderers {
    pub(crate) fn resize(&mut self, size: SizeCss, dpr: f32) {
        if let Some(w) = &mut self.window {
            w.renderer.resize(device_size(size, dpr));
        }
    }
}

fn handles(w: &FerroChartWindow) -> Result<(RawDisplayHandle, RawWindowHandle)> {
    let window = NonNull::new(w.window).ok_or_else(|| Error::invalid("'window' is null"))?;
    Ok(match w.platform {
        FERROCHART_PLATFORM_APPKIT => (
            AppKitDisplayHandle::new().into(),
            AppKitWindowHandle::new(window).into(),
        ),
        FERROCHART_PLATFORM_WIN32 => {
            let hwnd = NonZeroIsize::new(w.window as isize)
                .ok_or_else(|| Error::invalid("'window' is null"))?;
            let mut h = Win32WindowHandle::new(hwnd);
            h.hinstance = NonZeroIsize::new(w.display as isize);
            (WindowsDisplayHandle::new().into(), h.into())
        }
        FERROCHART_PLATFORM_XLIB => {
            let display =
                NonNull::new(w.display).ok_or_else(|| Error::invalid("'display' is null"))?;
            (
                XlibDisplayHandle::new(Some(display), w.screen).into(),
                XlibWindowHandle::new(w.window as std::ffi::c_ulong).into(),
            )
        }
        FERROCHART_PLATFORM_WAYLAND => {
            let display =
                NonNull::new(w.display).ok_or_else(|| Error::invalid("'display' is null"))?;
            (
                WaylandDisplayHandle::new(display).into(),
                WaylandWindowHandle::new(window).into(),
            )
        }
        p => return Err(Error::invalid(format!("unknown platform {p}"))),
    })
}

fn render_error(e: impl std::fmt::Display) -> Error {
    Error::new(FERROCHART_ERR_RENDER, e.to_string())
}

/// The size of the chart's images in device pixels (CSS size × pixel ratio,
/// rounded).
///
/// # Safety
///
/// `chart` is a valid chart; `out_width`, `out_height` are valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_pixel_size(
    chart: *mut FerroChart,
    out_width: *mut u32,
    out_height: *mut u32,
) -> i32 {
    with_chart(chart, |c| {
        out_ptr(out_width, "out_width")?;
        out_ptr(out_height, "out_height")?;
        let s = device_size(c.size, c.dpr);
        // SAFETY: checked.
        unsafe {
            write(out_width, s.w as u32);
            write(out_height, s.h as u32);
        }
        Ok(FERROCHART_OK)
    })
}

/// Draws the chart with the CPU renderer into `buffer`: RGBA8 pixels, rows
/// top to bottom, `stride` bytes apart (at least width × 4), `len` bytes in
/// all (at least stride × (height − 1) + width × 4); sizes from
/// [`ferrochart_pixel_size`]. `now_ms` drives animations (kinetic
/// scrolling); any monotonic clock.
///
/// # Safety
///
/// `chart` is a valid chart; `buffer` has `len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_render_rgba(
    chart: *mut FerroChart,
    now_ms: f64,
    buffer: *mut u8,
    stride: usize,
    len: usize,
) -> i32 {
    with_chart(chart, |c| {
        if buffer.is_null() {
            return Err(Error::new(crate::FERROCHART_ERR_NULL, "'buffer' is null"));
        }
        let size = device_size(c.size, c.dpr);
        let (w, h) = (size.w as usize, size.h as usize);
        let row = w * 4;
        if stride < row || len < stride * (h - 1) + row {
            return Err(Error::new(
                FERROCHART_ERR_BUFFER_TOO_SMALL,
                format!(
                    "the image is {w} × {h} px: needs stride ≥ {row} and len ≥ stride × {} + {row}",
                    h - 1
                ),
            ));
        }
        let now = if now_ms.is_finite() { now_ms } else { 0.0 };
        let r = c.renderers.cpu.get_or_insert_with(CpuRenderer::new);
        c.chart.build_frame(now, &r.measurer());
        let rendered = r.render(c.chart.frame(), &c.chart.data_view());
        c.collect_events();
        rendered.map_err(render_error)?;
        let Some(r) = &c.renderers.cpu else {
            return Err(render_error("nothing rendered"));
        };
        let pixmap = r.pixmap().ok_or_else(|| render_error("nothing rendered"))?;
        let data = pixmap.data();
        let pw = pixmap.width() as usize * 4;
        // SAFETY: `len` bytes are writable per the contract.
        let out = unsafe { std::slice::from_raw_parts_mut(buffer, len) };
        for y in 0..h.min(pixmap.height() as usize) {
            let n = row.min(pw);
            out[y * stride..y * stride + n].copy_from_slice(&data[y * pw..y * pw + n]);
        }
        Ok(FERROCHART_OK)
    })
}

/// Makes the chart draw into a native window (replacing an attached one);
/// draw with [`ferrochart_render`]. The chart's size should match the
/// window's (`ferrochart_resize` on window resizes).
///
/// # Safety
///
/// `chart` is a valid chart; `window` is a valid `FerroChartWindow` whose
/// handles stay valid until the window is detached or the chart destroyed.
/// On macOS, call from the main thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_attach_window(
    chart: *mut FerroChart,
    window: *const FerroChartWindow,
) -> i32 {
    with_chart(chart, |c| {
        // SAFETY: valid per the contract.
        let w = unsafe { read_sized(window, FerroChartWindow::default(), "window")? };
        let (display, win) = handles(&w)?;
        c.renderers.window = None;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(HostDisplay(display)),
        ));
        // SAFETY: the host keeps the handles valid while attached (contract).
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(display),
                raw_window_handle: win,
            })
        }
        .map_err(render_error)?;
        let ctx = GpuContext::with_instance(instance, Some(&surface)).map_err(render_error)?;
        let renderer = WgpuRenderer::with_surface(&ctx, surface, device_size(c.size, c.dpr))
            .map_err(render_error)?;
        c.renderers.window = Some(Window {
            renderer,
            _ctx: ctx,
            fresh: true,
        });
        Ok(FERROCHART_OK)
    })
}

/// Stops drawing into the attached window (none: nothing).
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_detach_window(chart: *mut FerroChart) -> i32 {
    with_chart(chart, |c| {
        c.renderers.window = None;
        Ok(FERROCHART_OK)
    })
}

/// Whether the chart has changed since the last frame (1) or not (0):
/// call [`ferrochart_render`] (or `ferrochart_render_rgba`) when it has.
/// Check after input, data changes and while animations run.
///
/// # Safety
///
/// `chart` is a valid chart.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_needs_redraw(chart: *mut FerroChart) -> i32 {
    with_chart(chart, |c| {
        let fresh = c.renderers.window.as_ref().is_some_and(|w| w.fresh);
        Ok(i32::from(
            fresh || c.chart.needs_redraw() != Invalidation::None,
        ))
    })
}

/// Draws a frame into the attached window if the chart changed: returns 1
/// when it drew, 0 when nothing changed, or a negative status (a lost
/// surface is reconfigured: try again on the next frame).
///
/// # Safety
///
/// `chart` is a valid chart (on macOS, used from the main thread).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_render(chart: *mut FerroChart, now_ms: f64) -> i32 {
    with_chart(chart, |c| {
        let Some(w) = &mut c.renderers.window else {
            return Err(Error::invalid(
                "no window attached (ferrochart_attach_window)",
            ));
        };
        if !w.fresh && c.chart.needs_redraw() == Invalidation::None {
            return Ok(0);
        }
        let now = if now_ms.is_finite() { now_ms } else { 0.0 };
        c.chart.set_native_kinds(w.renderer.native_kinds());
        c.chart.build_frame(now, &w.renderer.measurer());
        let result = w.renderer.render(c.chart.frame(), &c.chart.data_view());
        if result.is_ok() {
            w.fresh = false;
        }
        c.collect_events();
        result.map_err(render_error)?;
        Ok(1)
    })
}
