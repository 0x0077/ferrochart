//! What a Python chart or plot shares with its handles and a window showing
//! it: the state behind a lock, the "shown"
//! flag and the Python event handler. The window (`window.rs`), `to_png`
//! (`png.rs`) and the size and show methods work on any [`Scene`].

use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::frame::{Frame, Invalidation};
use ferrochart_core::geom::SizeCss;
use ferrochart_core::interaction::{CursorIcon, InputEvent, InputResult};
use ferrochart_core::text::TextMeasurer;
use ferrochart_render_wgpu::WgpuRenderer;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::err;
use crate::png::{Backend, Renderers};

/// Largest image or window side, CSS px.
pub(crate) const MAX_SIDE: f64 = 16_384.0;

/// What a window and `to_png` need of the engine object (`Chart`, `Plot`).
pub(crate) trait Draw {
    fn resize(&mut self, size: SizeCss, dpr: f32);
    fn needs_redraw(&self) -> bool;
    fn build_frame(&mut self, now_ms: f64, text: &dyn TextMeasurer);
    fn frame(&self) -> &Frame;
    fn data_view(&self) -> ChartDataView<'_>;
    fn handle_input(&mut self, event: InputEvent) -> InputResult;
}

impl Draw for ferrochart_core::Chart {
    fn resize(&mut self, size: SizeCss, dpr: f32) {
        Self::resize(self, size, dpr);
    }
    fn needs_redraw(&self) -> bool {
        Self::needs_redraw(self) != Invalidation::None
    }
    fn build_frame(&mut self, now_ms: f64, text: &dyn TextMeasurer) {
        Self::build_frame(self, now_ms, text);
    }
    fn frame(&self) -> &Frame {
        Self::frame(self)
    }
    fn data_view(&self) -> ChartDataView<'_> {
        Self::data_view(self)
    }
    fn handle_input(&mut self, event: InputEvent) -> InputResult {
        Self::handle_input(self, event)
    }
}

impl Draw for ferrochart_plot::Plot {
    fn resize(&mut self, size: SizeCss, dpr: f32) {
        Self::resize(self, size, dpr);
    }
    fn needs_redraw(&self) -> bool {
        Self::needs_redraw(self) != Invalidation::None
    }
    fn build_frame(&mut self, now_ms: f64, text: &dyn TextMeasurer) {
        Self::build_frame(self, now_ms, text);
    }
    fn frame(&self) -> &Frame {
        Self::frame(self)
    }
    fn data_view(&self) -> ChartDataView<'_> {
        ChartDataView::empty()
    }
    fn handle_input(&mut self, event: InputEvent) -> InputResult {
        Self::handle_input(self, event)
    }
}

/// The parts of a scene's state the generic code uses.
pub(crate) struct Parts<'a, C> {
    pub(crate) core: &'a mut C,
    /// Size in CSS px and device pixel ratio last set by the user or the
    /// window (`to_png` sizes the core temporarily).
    pub(crate) size: &'a mut SizeCss,
    pub(crate) dpr: &'a mut f32,
    pub(crate) renderers: &'a mut Renderers,
}

/// The state behind a [`Shared`] lock.
pub(crate) trait Scene: Send + Sized + 'static {
    type Core: Draw;
    /// An event for the Python handler.
    type Event: Send + 'static;
    fn parts(&mut self) -> Parts<'_, Self::Core>;
    /// Moves the core's events for Python to `out`.
    fn take_events(&mut self, out: &mut Vec<Self::Event>);
    /// Calls the handler of `shared` for each event (errors are printed).
    fn deliver(py: Python<'_>, shared: &Shared<Self>, events: &[Self::Event]);
}

/// A chart or plot shared by its Python objects (and a window showing it).
pub(crate) struct Shared<S: Scene> {
    state: Mutex<S>,
    /// In a window.
    pub(crate) shown: AtomicBool,
    /// `handler(name, payload)` for events.
    handler: Mutex<Option<Py<PyAny>>>,
    /// Events taken from the core, not yet given to Python.
    pending: Mutex<Vec<S::Event>>,
}

fn unpoison<T>(r: Result<T, PoisonError<T>>) -> T {
    // A panic while locked cannot leave the state half-updated in a way the
    // core does not handle (its methods do not panic), so a poisoned lock is
    // used as is.
    r.unwrap_or_else(PoisonError::into_inner)
}

impl<S: Scene> Shared<S> {
    pub(crate) fn new(state: S) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(state),
            shown: AtomicBool::new(false),
            handler: Mutex::new(None),
            pending: Mutex::new(Vec::new()),
        })
    }

    /// Locks the state (for the window).
    pub(crate) fn lock(&self) -> MutexGuard<'_, S> {
        unpoison(self.state.lock())
    }

    /// Locks the state for a Python call; a window showing it is woken when
    /// the lock is released, to draw what changed.
    pub(crate) fn py_lock(&self) -> PyGuard<'_, S> {
        PyGuard(self.lock())
    }

    /// The event handler, if set.
    pub(crate) fn handler(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        unpoison(self.handler.lock())
            .as_ref()
            .map(|h| h.clone_ref(py))
    }

    pub(crate) fn set_handler(&self, handler: Option<Py<PyAny>>) {
        *unpoison(self.handler.lock()) = handler;
    }

    pub(crate) fn is_shown(&self) -> bool {
        self.shown.load(Ordering::Acquire)
    }

    /// Size in CSS px, `(width, height)`.
    pub(crate) fn size(&self) -> (f64, f64) {
        let mut s = self.lock();
        let p = s.parts();
        (f64::from(p.size.w), f64::from(p.size.h))
    }

    /// Sizes the scene (CSS px) for `to_png` without arguments and the
    /// first window.
    pub(crate) fn resize(&self, width: f64, height: f64) -> PyResult<()> {
        let size = check_size(width, height)?;
        let mut s = self.py_lock();
        let p = s.parts();
        p.core.resize(size, *p.dpr);
        *p.size = size;
        Ok(())
    }

    fn take_core_events(&self, s: &mut S) {
        s.take_events(&mut *unpoison(self.pending.lock()));
    }

    /// Renders a PNG of `width` × `height` CSS px (default: the current
    /// size) at `scale`; the scene keeps its size. Returns `(png, backend)`.
    pub(crate) fn render_png<'py>(
        self: &Arc<Self>,
        py: Python<'py>,
        width: Option<f64>,
        height: Option<f64>,
        scale: f64,
        backend: &str,
    ) -> PyResult<(Bound<'py, PyBytes>, &'static str)> {
        let backend =
            Backend::parse(backend).ok_or_else(|| err("backend must be 'auto', 'gpu' or 'cpu'"))?;
        if !(scale.is_finite() && (0.25..=8.0).contains(&scale)) {
            return Err(err("scale must be between 0.25 and 8"));
        }
        let shared = self.clone();
        let (png, used) = py
            .detach(move || -> Result<_, String> {
                let mut guard = shared.py_lock();
                let p = guard.parts();
                let size = SizeCss {
                    w: width.map_or(p.size.w, |w| w as f32),
                    h: height.map_or(p.size.h, |h| h as f32),
                };
                if !(size.w >= 1.0 && size.h >= 1.0)
                    || f64::from(size.w.max(size.h)) * scale > MAX_SIDE
                {
                    return Err(format!(
                        "image size {} × {} at scale {scale} is out of range",
                        size.w, size.h
                    ));
                }
                let resized = size != *p.size || (scale as f32).to_bits() != p.dpr.to_bits();
                if resized {
                    p.core.resize(size, scale as f32);
                }
                let out = p.renderers.render(p.core, backend);
                if resized {
                    p.core.resize(*p.size, *p.dpr);
                }
                out
            })
            .map_err(err)?;
        Ok((PyBytes::new(py, &png), used))
    }

    /// `show`: opens a window (`window.rs`), blocking until it closes or
    /// not.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn show(
        self: &Arc<Self>,
        py: Python<'_>,
        block: bool,
        title: &str,
        width: Option<f64>,
        height: Option<f64>,
        close_after: Option<f64>,
    ) -> PyResult<()> {
        let current = self.size();
        let size = check_size(width.unwrap_or(current.0), height.unwrap_or(current.1))?;
        let close_after = close_after
            .map(|s| {
                if s.is_finite() && s >= 0.0 {
                    Ok(Duration::from_secs_f64(s))
                } else {
                    Err(err("close_after must be a non-negative number of seconds"))
                }
            })
            .transpose()?;
        let req = crate::window::Request {
            scene: self.clone(),
            title: title.to_owned(),
            size,
            close_after,
        };
        if block {
            crate::window::show_blocking(py, req)
        } else {
            crate::window::show_background(req)
        }
    }
}

/// A shared scene as the window sees it (object safe).
pub(crate) trait Shown: Send + Sync {
    fn shown(&self) -> &AtomicBool;
    /// Sizes the scene to the window.
    fn fit_window(&self, size: SizeCss, dpr: f32);
    /// The scene has something to draw.
    fn wants_frame(&self) -> bool;
    /// Draws a frame if needed; `Some(false)` if rendering failed.
    fn redraw(&self, now_ms: f64, renderer: &mut WgpuRenderer) -> Option<bool>;
    /// Handles input events; the cursor after the last one.
    fn input(&self, events: &[InputEvent]) -> Option<CursorIcon>;
    /// Hands the pending events to Python: now (blocking mode, the loop is
    /// on the main thread), or through the queue of `events.rs`. The scene
    /// is not locked, so handlers may change it.
    fn dispatch(self: Arc<Self>, blocking: bool);
}

impl<S: Scene> Shown for Shared<S> {
    fn shown(&self) -> &AtomicBool {
        &self.shown
    }

    fn fit_window(&self, size: SizeCss, dpr: f32) {
        let mut s = self.lock();
        let p = s.parts();
        p.core.resize(size, dpr);
        *p.size = size;
        *p.dpr = dpr;
    }

    fn wants_frame(&self) -> bool {
        self.lock().parts().core.needs_redraw()
    }

    fn redraw(&self, now_ms: f64, renderer: &mut WgpuRenderer) -> Option<bool> {
        use ferrochart_core::render::Renderer;
        let mut s = self.lock();
        let p = s.parts();
        let drawn = if p.core.needs_redraw() {
            p.core.build_frame(now_ms, &renderer.measurer());
            Some(renderer.render(p.core.frame(), &p.core.data_view()).is_ok())
        } else {
            None
        };
        self.take_core_events(&mut s);
        drawn
    }

    fn input(&self, events: &[InputEvent]) -> Option<CursorIcon> {
        if events.is_empty() {
            return None;
        }
        let mut s = self.lock();
        let p = s.parts();
        let mut cursor = None;
        for &e in events {
            cursor = Some(p.core.handle_input(e).cursor);
        }
        self.take_core_events(&mut s);
        cursor
    }

    fn dispatch(self: Arc<Self>, blocking: bool) {
        let mut events = std::mem::take(&mut *unpoison(self.pending.lock()));
        if events.is_empty() {
            return;
        }
        if blocking {
            Python::attach(|py| S::deliver(py, &self, &events));
            events.clear();
            // Keep the capacity.
            let mut p = unpoison(self.pending.lock());
            if p.is_empty() {
                *p = events;
            }
        } else {
            crate::events::enqueue(&self, events);
        }
    }
}

/// The state locked by a Python call.
pub(crate) struct PyGuard<'a, S>(MutexGuard<'a, S>);

impl<S> Deref for PyGuard<'_, S> {
    type Target = S;
    fn deref(&self) -> &S {
        &self.0
    }
}

impl<S> DerefMut for PyGuard<'_, S> {
    fn deref_mut(&mut self) -> &mut S {
        &mut self.0
    }
}

impl<S> Drop for PyGuard<'_, S> {
    fn drop(&mut self) {
        crate::window::wake();
    }
}

pub(crate) fn check_size(w: f64, h: f64) -> PyResult<SizeCss> {
    if !(w >= 1.0 && h >= 1.0 && w <= MAX_SIDE && h <= MAX_SIDE) {
        return Err(err(format!(
            "width and height must be between 1 and {MAX_SIDE} (got {w} × {h})"
        )));
    }
    Ok(SizeCss {
        w: w as f32,
        h: h as f32,
    })
}
