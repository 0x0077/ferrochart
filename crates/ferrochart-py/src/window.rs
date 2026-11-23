//! `show()` of charts and plots: native windows on winit + wgpu.
//!
//! winit allows one `EventLoop` per process, so it is a process singleton,
//! created by the first `show` and reused after. The first `show` also fixes
//! the mode:
//!
//! - blocking (default, every platform): the loop lives on the main thread
//!   and each `show` runs it with `run_app_on_demand` until its window
//!   closes, without the GIL. Python signals (Ctrl+C) are checked every
//!   100 ms and raised from `show`;
//! - non-blocking (`block=False`, Linux and Windows): the loop runs on its
//!   own thread until the process ends; `show` sends it a message to open a
//!   window and returns.
//!
//! A window holds its chart or plot as a [`Shown`] trait object. It locks it
//! for each frame and each batch of input events. Python calls that touch a
//! chart or plot wake the loop (`wake`), so changes made from another thread
//! or between `show(block=False)` calls are drawn. A chart or plot is in at
//! most one window at a time.
//!
//! Events go to the Python handler (`events.rs`): directly in
//! the blocking mode (the loop runs on the main thread), through a queue
//! delivered on the Python main thread otherwise.

use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use ferrochart_core::geom::{SizeCss, SizePx};
use ferrochart_core::interaction::InputEvent;
use ferrochart_core::render::Renderer;
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};
use ferrochart_winit::{InputConverter, cursor_icon, wgpu, winit};
use pyo3::prelude::*;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

use crate::err;
use crate::shared::Shown;

/// How often Python signals are checked while `show` blocks.
const SIGNAL_CHECK: Duration = Duration::from_millis(100);
/// Consecutive render failures after which a window waits for input.
const MAX_RETRIES: u32 = 3;

/// A message to the event loop.
pub(crate) enum Msg {
    /// Open a window.
    Open(Request),
    /// A chart or plot may have changed: redraw the windows that need it.
    Wake,
}

/// A window to open.
pub(crate) struct Request {
    pub(crate) scene: Arc<dyn Shown>,
    pub(crate) title: String,
    pub(crate) size: SizeCss,
    /// Close the window by itself after this long (tests).
    pub(crate) close_after: Option<Duration>,
}

#[derive(Clone)]
enum Mode {
    Blocking,
    Background(EventLoopProxy<Msg>),
}

/// The mode fixed by the first `show`.
static MODE: Mutex<Option<Mode>> = Mutex::new(None);
/// Where `wake` sends to; set while a loop runs.
static PROXY: Mutex<Option<EventLoopProxy<Msg>>> = Mutex::new(None);

thread_local! {
    /// The blocking mode's event loop (main thread).
    static LOOP: RefCell<Option<EventLoop<Msg>>> = const { RefCell::new(None) };
}

fn mode() -> std::sync::MutexGuard<'static, Option<Mode>> {
    MODE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Asks the running loop (if any) to redraw charts and plots that changed.
pub(crate) fn wake() {
    let proxy = PROXY.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(p) = proxy.as_ref() {
        let _ = p.send_event(Msg::Wake);
    }
}

fn set_proxy(p: Option<EventLoopProxy<Msg>>) {
    *PROXY.lock().unwrap_or_else(PoisonError::into_inner) = p;
}

/// Marks the chart or plot of `req` as shown; fails if it already is.
fn claim(req: &Request) -> PyResult<()> {
    if req.scene.shown().swap(true, Ordering::AcqRel) {
        return Err(err("already shown in a window"));
    }
    Ok(())
}

/// The event loop moved into `Python::detach`, which runs its closure on
/// the calling thread.
struct OnThisThread<T>(T);

// SAFETY: `Python::detach` requires `Send` only to keep Python objects out
// of the closure; it runs the closure on the calling thread, so the event
// loop (which must stay on its thread) never moves. The host holds no
// Python objects; callbacks take the GIL with `Python::attach`.
unsafe impl<T> Send for OnThisThread<T> {}

impl<T> OnThisThread<T> {
    /// The value (a method, so that closures capture the whole wrapper).
    fn into_inner(self) -> T {
        self.0
    }
}

/// Shows `req` and blocks until its window closes (main thread).
pub(crate) fn show_blocking(py: Python<'_>, req: Request) -> PyResult<()> {
    {
        let mut m = mode();
        match m.as_ref() {
            Some(Mode::Background(_)) => {
                return Err(err(
                    "show(block=False) was used in this process; every later show() must use \
                     block=False too (winit allows one event loop per process)",
                ));
            }
            Some(Mode::Blocking) => {}
            None => *m = Some(Mode::Blocking),
        }
    }
    claim(&req)?;
    let scene = req.scene.clone();
    let result = run_blocking(py, req);
    scene.shown().store(false, Ordering::Release);
    result
}

fn run_blocking(py: Python<'_>, req: Request) -> PyResult<()> {
    let event_loop = LOOP.with(|l| l.borrow_mut().take());
    let event_loop = match event_loop {
        Some(l) => l,
        None => EventLoop::<Msg>::with_user_event()
            .build()
            .map_err(|e| err(format!("cannot create the event loop: {e}")))?,
    };
    set_proxy(Some(event_loop.create_proxy()));
    let mut host = Host::new(true);
    host.pending.push(req);
    let job = OnThisThread((event_loop, host));
    let (event_loop, host, run) = py
        .detach(move || {
            use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
            let (mut event_loop, mut host) = job.into_inner();
            let run = event_loop.run_app_on_demand(&mut host);
            OnThisThread((event_loop, host, run))
        })
        .into_inner();
    set_proxy(None);
    LOOP.with(|l| *l.borrow_mut() = Some(event_loop));
    run.map_err(|e| err(format!("event loop: {e}")))?;
    host.error.map_or(Ok(()), Err)
}

/// Shows `req` from the background UI thread (started on first use) and
/// returns at once. Linux and Windows only.
pub(crate) fn show_background(req: Request) -> PyResult<()> {
    if cfg!(target_os = "macos") {
        return Err(err(
            "show(block=False) is not available on macOS: windows must run on the main thread",
        ));
    }
    let proxy = {
        let mut m = mode();
        match m.as_ref() {
            Some(Mode::Blocking) => {
                return Err(err(
                    "show() (blocking) was used in this process; every later show() must block \
                     too (winit allows one event loop per process)",
                ));
            }
            Some(Mode::Background(p)) => p.clone(),
            None => {
                let p = start_background()?;
                *m = Some(Mode::Background(p.clone()));
                p
            }
        }
    };
    claim(&req)?;
    let scene = req.scene.clone();
    proxy.send_event(Msg::Open(req)).map_err(|_| {
        scene.shown().store(false, Ordering::Release);
        err("the window thread has stopped")
    })
}

/// Starts the UI thread and returns its proxy.
fn start_background() -> PyResult<EventLoopProxy<Msg>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("ferrochart-ui".into())
        .spawn(move || {
            let event_loop = match build_any_thread() {
                Ok(l) => l,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            let _ = tx.send(Ok(event_loop.create_proxy()));
            let mut host = Host::new(false);
            let _ = event_loop.run_app(&mut host);
        })
        .map_err(|e| err(format!("cannot start the window thread: {e}")))?;
    let proxy = rx
        .recv()
        .map_err(|_| err("the window thread stopped"))?
        .map_err(err)?;
    set_proxy(Some(proxy.clone()));
    Ok(proxy)
}

/// An event loop that may run off the main thread.
fn build_any_thread() -> Result<EventLoop<Msg>, String> {
    let mut builder = EventLoop::<Msg>::with_user_event();
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    #[cfg(all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    ))]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    }
    builder
        .build()
        .map_err(|e| format!("cannot create the event loop: {e}"))
}

/// An open window.
struct Win {
    window: Arc<Window>,
    // Keeps the device alive alongside the renderer.
    _ctx: GpuContext,
    renderer: WgpuRenderer,
    scene: Arc<dyn Shown>,
    input: InputConverter,
    failures: u32,
    close_at: Option<Instant>,
}

struct Host {
    /// Blocking mode: leave the loop when the last window closes, check
    /// Python signals.
    blocking: bool,
    windows: Vec<Win>,
    pending: Vec<Request>,
    clock: Instant,
    error: Option<PyErr>,
    next_signal_check: Instant,
    /// Input events of one window event (reused).
    inputs: Vec<InputEvent>,
}

fn physical(size: PhysicalSize<u32>) -> SizePx {
    SizePx {
        w: size.width as f32,
        h: size.height as f32,
    }
}

impl Host {
    fn new(blocking: bool) -> Self {
        let now = Instant::now();
        Self {
            blocking,
            windows: Vec::new(),
            pending: Vec::new(),
            clock: now,
            error: None,
            next_signal_check: now + SIGNAL_CHECK,
            inputs: Vec::new(),
        }
    }

    fn now_ms(&self) -> f64 {
        self.clock.elapsed().as_secs_f64() * 1000.0
    }

    fn open_pending(&mut self, event_loop: &ActiveEventLoop) {
        for req in std::mem::take(&mut self.pending) {
            let scene = req.scene.clone();
            match open(event_loop, req) {
                Ok(win) => {
                    win.window.request_redraw();
                    self.windows.push(win);
                }
                Err(e) => {
                    scene.shown().store(false, Ordering::Release);
                    if self.blocking {
                        self.error = Some(err(e));
                    } else {
                        eprintln!("ferrochart: cannot open a window: {e}");
                    }
                }
            }
        }
        if self.blocking && self.windows.is_empty() {
            event_loop.exit();
        }
    }

    fn close(&mut self, event_loop: &ActiveEventLoop, i: usize) {
        let win = self.windows.swap_remove(i);
        win.scene.shown().store(false, Ordering::Release);
        drop(win);
        if self.blocking && self.windows.is_empty() {
            event_loop.exit();
        }
    }

    fn redraw(&mut self, i: usize) {
        let now = self.now_ms();
        let Some(win) = self.windows.get_mut(i) else {
            return;
        };
        match win.scene.redraw(now, &mut win.renderer) {
            Some(true) => win.failures = 0,
            Some(false) => win.failures += 1,
            None => {}
        }
        win.scene.clone().dispatch(self.blocking);
    }

    fn wants_frame(win: &Win) -> bool {
        win.failures < MAX_RETRIES && win.scene.wants_frame()
    }
}

fn open(event_loop: &ActiveEventLoop, req: Request) -> Result<Win, String> {
    let attrs = Window::default_attributes()
        .with_title(req.title)
        .with_inner_size(LogicalSize::new(
            f64::from(req.size.w),
            f64::from(req.size.h),
        ));
    let window = Arc::new(event_loop.create_window(attrs).map_err(|e| e.to_string())?);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
        Box::new(window.clone()),
    ));
    let surface = instance
        .create_surface(window.clone())
        .map_err(|e| e.to_string())?;
    let ctx = GpuContext::with_instance(instance, Some(&surface)).map_err(|e| e.to_string())?;
    let mut renderer = WgpuRenderer::with_surface(&ctx, surface, physical(window.inner_size()))
        .map_err(|e| e.to_string())?;
    crate::png::add_fonts(renderer.text_engine_mut(), &req.scene.fonts());
    let win = Win {
        window,
        _ctx: ctx,
        renderer,
        scene: req.scene,
        input: InputConverter::new(),
        failures: 0,
        close_at: req.close_after.map(|d| Instant::now() + d),
    };
    fit_window(&win);
    Ok(win)
}

/// Sizes the chart or plot to the window.
fn fit_window(win: &Win) {
    let dpr = win.window.scale_factor();
    let size = win.window.inner_size().to_logical::<f64>(dpr);
    let css = SizeCss {
        w: size.width as f32,
        h: size.height as f32,
    };
    win.scene.fit_window(css, dpr as f32);
}

impl ApplicationHandler<Msg> for Host {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        // Each `run_app_on_demand` starts with `Init`; open the window of
        // this `show` (also done in `resumed`).
        if cause == StartCause::Init {
            self.open_pending(event_loop);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.open_pending(event_loop);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, msg: Msg) {
        match msg {
            Msg::Open(req) => {
                self.pending.push(req);
                self.open_pending(event_loop);
            }
            Msg::Wake => {
                for win in &self.windows {
                    if Self::wants_frame(win) {
                        win.window.request_redraw();
                    }
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(i) = self.windows.iter().position(|w| w.window.id() == id) else {
            return;
        };
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                self.close(event_loop, i);
                return;
            }
            WindowEvent::Resized(size) => {
                let win = &mut self.windows[i];
                win.renderer.resize(physical(size));
                fit_window(win);
                win.failures = 0;
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let win = &mut self.windows[i];
                fit_window(win);
                win.failures = 0;
            }
            WindowEvent::RedrawRequested => self.redraw(i),
            other => {
                let now = self.now_ms();
                let win = &mut self.windows[i];
                let scale = win.window.scale_factor();
                let inputs = &mut self.inputs;
                inputs.clear();
                win.input.convert(&other, scale, now, |e| inputs.push(e));
                if let Some(c) = win.scene.input(inputs) {
                    win.window.set_cursor(cursor_icon(c));
                    win.failures = 0;
                }
                win.scene.clone().dispatch(self.blocking);
            }
        }
        if let Some(win) = self.windows.get(i)
            && Self::wants_frame(win)
        {
            win.window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        while let Some(i) = self
            .windows
            .iter()
            .position(|w| w.close_at.is_some_and(|t| t <= now))
        {
            self.close(event_loop, i);
        }
        for win in &self.windows {
            if Self::wants_frame(win) {
                win.window.request_redraw();
            }
        }
        let close_at = self.windows.iter().filter_map(|w| w.close_at).min();
        if !self.blocking {
            event_loop.set_control_flow(close_at.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
            return;
        }
        if now >= self.next_signal_check {
            self.next_signal_check = now + SIGNAL_CHECK;
            // A closure: the method alone is not general over the lifetime.
            #[allow(clippy::redundant_closure_for_method_calls)]
            let signals = Python::attach(|py| py.check_signals());
            if let Err(e) = signals {
                self.error = Some(e);
                // Close the windows and leave `show`.
                for win in self.windows.drain(..) {
                    win.scene.shown().store(false, Ordering::Release);
                }
                event_loop.exit();
                return;
            }
        }
        let wake = close_at.map_or(self.next_signal_check, |t| t.min(self.next_signal_check));
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}
