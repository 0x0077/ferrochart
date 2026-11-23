//! Desktop host for FerroChart on winit + wgpu.
//!
//! - [`show`] opens a window with the defaults and shows a chart until it is
//!   closed.
//! - [`run`] opens a window with a wgpu surface and drives the chart: winit
//!   events become [`InputEvent`]s, frames are built and rendered only when
//!   [`Chart::needs_redraw`] says so (which includes kinetic scrolling), chart
//!   events are handed to your [`ChartApp`], and [`ChartApp::update`] can ask to
//!   be woken at a given time (e.g. for a real-time feed).
//! - [`InputConverter`] is the event conversion on its own, for hosts that own
//!   the event loop themselves (an egui app, a custom window).
//!
//! All times passed to the chart (`InputEvent::time_ms`, `build_frame`) come
//! from one monotonic clock: milliseconds since [`run`] started.
//!
//! # Examples
//!
//! ```no_run
//! use ferrochart_core::{Chart, ChartOptions};
//! use ferrochart_winit::{ChartApp, WindowOptions, run};
//!
//! struct App;
//! impl ChartApp for App {}
//!
//! run(WindowOptions::default(), Chart::new(ChartOptions::default()), App)?;
//! # Ok::<(), ferrochart_winit::HostError>(())
//! ```

pub use {wgpu, winit};

use std::sync::Arc;
use std::time::{Duration, Instant};

use ferrochart_core::Chart;
use ferrochart_core::chart::ChartEvent;
use ferrochart_core::frame::Invalidation;
use ferrochart_core::geom::{Css, PointCss, SizeCss, SizePx};
use ferrochart_core::interaction::{
    Button, CursorIcon, GesturePhase, InputEvent, Key, Modifiers, WheelDelta,
};
use ferrochart_core::render::{RenderError, Renderer};
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

/// Two presses closer than this (and [`DOUBLE_CLICK_DISTANCE`]) are a double click.
const DOUBLE_CLICK_MS: f64 = 400.0;
/// Maximum distance between the presses of a double click, CSS pixels.
const DOUBLE_CLICK_DISTANCE: f32 = 4.0;
/// Consecutive render failures after which the host stops retrying until the
/// next input or resize (e.g. while the window is occluded).
const MAX_RETRIES: u32 = 3;

/// Errors from [`run`].
#[derive(Debug)]
#[non_exhaustive]
pub enum HostError {
    /// The winit event loop could not be created or failed.
    EventLoop(winit::error::EventLoopError),
    /// The window could not be created.
    Window(winit::error::OsError),
    /// The wgpu surface could not be created.
    Surface(wgpu::CreateSurfaceError),
    /// No GPU adapter/device, or the surface is unsupported.
    Render(RenderError),
    /// `WindowOptions::fonts[index]` is not a usable TrueType/OpenType font.
    Font {
        /// Index in [`WindowOptions::fonts`].
        index: usize,
        /// Why it was rejected.
        message: String,
    },
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventLoop(e) => write!(f, "event loop: {e}"),
            Self::Window(e) => write!(f, "window: {e}"),
            Self::Surface(e) => write!(f, "surface: {e}"),
            Self::Render(e) => write!(f, "renderer: {e}"),
            Self::Font { index, message } => write!(f, "font {index}: {message}"),
        }
    }
}

impl std::error::Error for HostError {}

/// Window settings for [`run`].
#[derive(Clone, Debug)]
pub struct WindowOptions {
    /// Window title.
    pub title: String,
    /// Initial inner size.
    pub size: SizeCss,
    /// Fallback fonts (TrueType/OpenType/collection data), searched in order
    /// for characters the bundled font lacks: CJK and other scripts (r21). Without one
    /// they are drawn as `□`. No system fonts are
    /// scanned; for example on macOS
    /// `std::fs::read("/System/Library/Fonts/Hiragino Sans GB.ttc")`.
    pub fonts: Vec<Vec<u8>>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "FerroChart".to_owned(),
            size: SizeCss {
                w: 1200.0,
                h: 720.0,
            },
            fonts: Vec::new(),
        }
    }
}

/// Application callbacks of [`run`]. All methods have empty defaults.
pub trait ChartApp {
    /// Called once per event-loop iteration before the loop sleeps. Change
    /// the chart here (append data, `update_last`, …). Return the time
    /// (same clock as `now_ms`) at which you want to be called again, or
    /// `None` to sleep until the next input.
    fn update(&mut self, chart: &mut Chart, now_ms: f64) -> Option<f64> {
        let _ = (chart, now_ms);
        None
    }

    /// A chart event (crosshair moved, visible range changed, need more
    /// history, …), delivered after the frame that produced it.
    fn on_event(&mut self, chart: &mut Chart, event: ChartEvent) {
        let _ = (chart, event);
    }

    /// Called once the GPU renderer exists (and again if it is recreated):
    /// register native custom series here with
    /// [`WgpuRenderer::register_series`]. The chart learns their kinds
    /// before the next frame.
    fn on_renderer(&mut self, renderer: &mut WgpuRenderer) {
        let _ = renderer;
    }

    /// Called after each rendered frame with the CPU time spent building and
    /// submitting it, for statistics.
    fn on_frame(&mut self, window: &Window, cpu_ms: f64) {
        let _ = (window, cpu_ms);
    }
}

/// Converts winit window events into [`InputEvent`]s. Keeps the state winit
/// does not put on each event: modifiers, the pointer position, and the last
/// press (to synthesize [`InputEvent::DoubleClick`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct InputConverter {
    mods: Modifiers,
    pos: Option<PointCss>,
    last_press: Option<(Button, PointCss, f64)>,
}

fn button(b: MouseButton) -> Option<Button> {
    match b {
        MouseButton::Left => Some(Button::Left),
        MouseButton::Middle => Some(Button::Middle),
        MouseButton::Right => Some(Button::Right),
        _ => None,
    }
}

fn phase(p: TouchPhase) -> GesturePhase {
    match p {
        TouchPhase::Started => GesturePhase::Start,
        TouchPhase::Moved => GesturePhase::Update,
        TouchPhase::Ended | TouchPhase::Cancelled => GesturePhase::End,
    }
}

fn key(k: &WKey) -> Option<Key> {
    match k {
        WKey::Named(NamedKey::ArrowLeft) => Some(Key::ArrowLeft),
        WKey::Named(NamedKey::ArrowRight) => Some(Key::ArrowRight),
        WKey::Named(NamedKey::ArrowUp) => Some(Key::ArrowUp),
        WKey::Named(NamedKey::ArrowDown) => Some(Key::ArrowDown),
        WKey::Named(NamedKey::Home) => Some(Key::Home),
        WKey::Named(NamedKey::End) => Some(Key::End),
        WKey::Named(NamedKey::Escape) => Some(Key::Escape),
        WKey::Named(NamedKey::Delete) => Some(Key::Delete),
        WKey::Named(NamedKey::Enter) => Some(Key::Enter),
        WKey::Character(s) => s.chars().next().map(Key::Char),
        _ => None,
    }
}

/// winit's positive y scrolls up (content moves down); the chart, like the
/// DOM, uses positive y for scrolling down. Pixel deltas are physical.
fn wheel_delta(delta: MouseScrollDelta, scale_factor: f64) -> WheelDelta {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => WheelDelta::Lines { x: -x, y: -y },
        MouseScrollDelta::PixelDelta(p) => WheelDelta::Pixels {
            x: Css(-(p.x / scale_factor) as f32),
            y: Css(-(p.y / scale_factor) as f32),
        },
    }
}

/// The winit cursor for a chart cursor.
pub fn cursor_icon(c: CursorIcon) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as W;
    match c {
        CursorIcon::Crosshair => W::Crosshair,
        CursorIcon::Grab => W::Grab,
        CursorIcon::Grabbing => W::Grabbing,
        CursorIcon::EwResize => W::EwResize,
        CursorIcon::NsResize => W::NsResize,
        CursorIcon::Pointer => W::Pointer,
        CursorIcon::Move => W::Move,
        _ => W::Default,
    }
}

impl InputConverter {
    /// A converter with no modifiers held and the pointer outside.
    pub fn new() -> Self {
        Self::default()
    }

    fn moved(&mut self, pos: PointCss, time_ms: f64, mut emit: impl FnMut(InputEvent)) {
        self.pos = Some(pos);
        emit(InputEvent::PointerMove {
            id: 0,
            pos,
            mods: self.mods,
            time_ms,
        });
    }

    fn mouse(&mut self, b: Button, pressed: bool, time_ms: f64, mut emit: impl FnMut(InputEvent)) {
        let Some(pos) = self.pos else {
            return;
        };
        let mods = self.mods;
        if !pressed {
            emit(InputEvent::PointerUp {
                id: 0,
                pos,
                button: b,
                mods,
                time_ms,
            });
            return;
        }
        emit(InputEvent::PointerDown {
            id: 0,
            pos,
            button: b,
            mods,
            time_ms,
        });
        let double = self.last_press.is_some_and(|(lb, lp, lt)| {
            lb == b
                && time_ms - lt <= DOUBLE_CLICK_MS
                && (pos.x - lp.x).abs() <= DOUBLE_CLICK_DISTANCE
                && (pos.y - lp.y).abs() <= DOUBLE_CLICK_DISTANCE
        });
        if double {
            self.last_press = None;
            emit(InputEvent::DoubleClick { pos, button: b });
        } else {
            self.last_press = Some((b, pos, time_ms));
        }
    }

    /// Converts `event` and passes the resulting chart events (zero, one or
    /// two) to `emit`. `scale_factor` is the window's dpr; `time_ms` is the
    /// host clock shared with `build_frame`.
    pub fn convert(
        &mut self,
        event: &WindowEvent,
        scale_factor: f64,
        time_ms: f64,
        mut emit: impl FnMut(InputEvent),
    ) {
        let mods = self.mods;
        let css = |v: f64| (v / scale_factor) as f32;
        match event {
            WindowEvent::ModifiersChanged(m) => {
                let s: ModifiersState = m.state();
                self.mods = Modifiers {
                    shift: s.shift_key(),
                    ctrl: s.control_key(),
                    alt: s.alt_key(),
                    meta: s.super_key(),
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.moved(
                    PointCss {
                        x: css(position.x),
                        y: css(position.y),
                    },
                    time_ms,
                    emit,
                );
            }
            WindowEvent::CursorLeft { .. } => {
                self.pos = None;
                emit(InputEvent::PointerLeave);
            }
            WindowEvent::MouseInput {
                state, button: b, ..
            } => {
                if let Some(b) = button(*b) {
                    self.mouse(b, *state == ElementState::Pressed, time_ms, emit);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let Some(pos) = self.pos else {
                    return;
                };
                emit(InputEvent::Wheel {
                    pos,
                    delta: wheel_delta(*delta, scale_factor),
                    mods,
                });
            }
            WindowEvent::PinchGesture {
                delta, phase: p, ..
            } => {
                let Some(center) = self.pos else {
                    return;
                };
                let scale = (1.0 + delta) as f32;
                emit(InputEvent::Pinch {
                    phase: phase(*p),
                    center,
                    scale: if scale.is_finite() && scale > 0.0 {
                        scale
                    } else {
                        1.0
                    },
                });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(k) = key(&event.logical_key) {
                    emit(InputEvent::Key {
                        key: k,
                        pressed: event.state == ElementState::Pressed,
                        mods,
                    });
                }
            }
            WindowEvent::Focused(false) => {
                self.mods = Modifiers::default();
            }
            _ => {}
        }
    }
}

/// Opens a window with the default options and shows `chart` until the
/// window is closed: [`run`] with [`WindowOptions::default`] and no
/// callbacks.
///
/// # Errors
///
/// As [`run`].
///
/// # Examples
///
/// ```no_run
/// use ferrochart_core::testing::random_walk;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let candles = chart.add_candlestick()?;
/// chart.set_data(candles, random_walk(1, 500, 0, 60_000)?)?;
/// ferrochart_winit::show(chart)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn show(chart: Chart) -> Result<(), HostError> {
    struct NoApp;
    impl ChartApp for NoApp {}
    run(WindowOptions::default(), chart, NoApp)
}

/// Opens a window showing `chart` and runs until it is closed.
///
/// The chart is sized to the window; `app` is called back as described on
/// [`ChartApp`].
///
/// # Errors
///
/// [`HostError`] if the event loop, window, surface or GPU device cannot be
/// created, or a font of [`WindowOptions::fonts`] is not usable.
pub fn run(options: WindowOptions, chart: Chart, app: impl ChartApp) -> Result<(), HostError> {
    let event_loop = EventLoop::new().map_err(HostError::EventLoop)?;
    let mut host = Host {
        options,
        chart,
        app,
        clock: Instant::now(),
        input: InputConverter::new(),
        gpu: None,
        error: None,
        events: Vec::new(),
        failures: 0,
        wake: None,
    };
    event_loop
        .run_app(&mut host)
        .map_err(HostError::EventLoop)?;
    host.error.map_or(Ok(()), Err)
}

struct Gpu {
    window: Arc<Window>,
    // Keeps the device alive alongside the renderer.
    _ctx: GpuContext,
    renderer: WgpuRenderer,
}

struct Host<A> {
    options: WindowOptions,
    chart: Chart,
    app: A,
    clock: Instant,
    input: InputConverter,
    gpu: Option<Gpu>,
    error: Option<HostError>,
    events: Vec<ChartEvent>,
    failures: u32,
    wake: Option<f64>,
}

fn physical(size: PhysicalSize<u32>) -> SizePx {
    SizePx {
        w: size.width as f32,
        h: size.height as f32,
    }
}

impl<A: ChartApp> Host<A> {
    fn now_ms(&self) -> f64 {
        self.clock.elapsed().as_secs_f64() * 1000.0
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, e: HostError) {
        self.error = Some(e);
        event_loop.exit();
    }

    fn create(&mut self, event_loop: &ActiveEventLoop) -> Result<Gpu, HostError> {
        let attrs = Window::default_attributes()
            .with_title(self.options.title.clone())
            .with_inner_size(LogicalSize::new(
                f64::from(self.options.size.w),
                f64::from(self.options.size.h),
            ));
        let window = Arc::new(event_loop.create_window(attrs).map_err(HostError::Window)?);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(window.clone()),
        ));
        let surface = instance
            .create_surface(window.clone())
            .map_err(HostError::Surface)?;
        let ctx = GpuContext::with_instance(instance, Some(&surface)).map_err(HostError::Render)?;
        let size = window.inner_size();
        let mut renderer =
            WgpuRenderer::with_surface(&ctx, surface, physical(size)).map_err(HostError::Render)?;
        for (index, font) in self.options.fonts.iter().enumerate() {
            renderer
                .text_engine_mut()
                .add_fallback_font(font.clone())
                .map_err(|e| HostError::Font {
                    index,
                    message: e.to_string(),
                })?;
        }
        self.app.on_renderer(&mut renderer);
        self.resize_chart(&window);
        Ok(Gpu {
            window,
            _ctx: ctx,
            renderer,
        })
    }

    fn resize_chart(&mut self, window: &Window) {
        let dpr = window.scale_factor();
        let size = window.inner_size().to_logical::<f64>(dpr);
        self.chart.resize(
            SizeCss {
                w: size.width as f32,
                h: size.height as f32,
            },
            dpr as f32,
        );
    }

    fn redraw(&mut self) {
        let now = self.now_ms();
        let Some(gpu) = &mut self.gpu else {
            return;
        };
        // Native custom series of the renderer; no-op when unchanged.
        self.chart.set_native_kinds(gpu.renderer.native_kinds());
        if self.chart.needs_redraw() != Invalidation::None {
            let start = Instant::now();
            self.chart.build_frame(now, &gpu.renderer.measurer());
            match gpu
                .renderer
                .render(self.chart.frame(), &self.chart.data_view())
            {
                Ok(()) => {
                    self.failures = 0;
                    let cpu_ms = start.elapsed().as_secs_f64() * 1000.0;
                    self.app.on_frame(&gpu.window, cpu_ms);
                }
                // The frame is still pending in the chart; try again (the
                // surface was reconfigured), but not forever.
                Err(_) => self.failures += 1,
            }
        }
        self.events.extend(self.chart.drain_events());
        for event in self.events.drain(..) {
            self.app.on_event(&mut self.chart, event);
        }
    }

    fn wants_frame(&self) -> bool {
        self.chart.needs_redraw() != Invalidation::None && self.failures < MAX_RETRIES
    }
}

impl<A: ChartApp> ApplicationHandler for Host<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        match self.create(event_loop) {
            Ok(gpu) => {
                gpu.window.request_redraw();
                self.gpu = Some(gpu);
            }
            Err(e) => self.fail(event_loop, e),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = self.gpu.as_ref().map(|g| g.window.clone()) else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.renderer.resize(physical(size));
                }
                self.resize_chart(&window);
                self.failures = 0;
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.resize_chart(&window);
                self.failures = 0;
            }
            WindowEvent::RedrawRequested => self.redraw(),
            other => {
                let now = self.now_ms();
                let (chart, input) = (&mut self.chart, &mut self.input);
                let mut cursor = None;
                input.convert(&other, window.scale_factor(), now, |e| {
                    let r = chart.handle_input(e);
                    cursor = Some(r.cursor);
                });
                if let Some(c) = cursor {
                    window.set_cursor(cursor_icon(c));
                    self.failures = 0;
                }
            }
        }
        if self.wants_frame() {
            window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = self.now_ms();
        if self.wake.is_none_or(|t| t <= now) {
            self.wake = self.app.update(&mut self.chart, now);
        }
        if let Some(gpu) = &self.gpu
            && self.wants_frame()
        {
            gpu.window.request_redraw();
        }
        event_loop.set_control_flow(match self.wake {
            Some(t) if t.is_finite() => ControlFlow::WaitUntil(
                self.clock + Duration::from_secs_f64((t / 1000.0).clamp(0.0, 1e9)),
            ),
            _ => ControlFlow::Wait,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(f: impl FnOnce(&mut dyn FnMut(InputEvent))) -> Vec<InputEvent> {
        let mut out = Vec::new();
        f(&mut |e| out.push(e));
        out
    }

    #[test]
    fn wheel_direction_and_dpr_follow_the_dom_convention() {
        assert_eq!(
            wheel_delta(MouseScrollDelta::LineDelta(0.0, 1.0), 2.0),
            WheelDelta::Lines { x: -0.0, y: -1.0 }
        );
        let px = wheel_delta(
            MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(4.0, -20.0)),
            2.0,
        );
        assert_eq!(
            px,
            WheelDelta::Pixels {
                x: Css(-2.0),
                y: Css(10.0)
            }
        );
    }

    #[test]
    fn two_quick_presses_make_a_double_click() {
        let mut c = InputConverter::new();
        let pos = PointCss { x: 10.0, y: 10.0 };
        let mut events = collect(|e| c.moved(pos, 0.0, e));
        events.extend(collect(|e| c.mouse(Button::Left, true, 0.0, e)));
        events.extend(collect(|e| c.mouse(Button::Left, false, 50.0, e)));
        events.extend(collect(|e| c.mouse(Button::Left, true, 200.0, e)));
        assert!(matches!(
            events.last(),
            Some(InputEvent::DoubleClick { .. })
        ));
        // A third press starts over.
        let third = collect(|e| c.mouse(Button::Left, true, 300.0, e));
        assert_eq!(third.len(), 1);
        // Slow presses do not.
        let slow = collect(|e| c.mouse(Button::Left, true, 2000.0, e));
        assert_eq!(slow.len(), 1);
    }

    #[test]
    fn buttons_need_a_known_pointer_position() {
        let mut c = InputConverter::new();
        assert!(collect(|e| c.mouse(Button::Left, true, 0.0, e)).is_empty());
    }
}
