//! What `FerroChart` and `FerroPlot` share on a canvas: renderer selection (WebGPU, then
//! WebGL2), the
//! `requestAnimationFrame` loop, `ResizeObserver` and `devicePixelRatio`
//! tracking, DOM input and event listeners, the intro animation (`intro`)
//! and the attribution logo (`logo`). It is generic over [`Scene`],
//! implemented by the chart and by the plot.
//!
//! All state lives in one `Rc<Host<S>>`; DOM and `requestAnimationFrame`
//! callbacks hold `Weak` references to it, so dropping the object releases
//! everything and the callbacks become no-ops. JS listeners are called with
//! no Rust borrow held, so they may call back into the object.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::frame::{Frame, Invalidation, Reveal};
use ferrochart_core::geom::{PointCss, PointPx, SizeCss, SizePx};
use ferrochart_core::interaction::{GesturePhase, InputEvent, InputResult, Modifiers};
use ferrochart_core::render::Renderer;
use ferrochart_core::theme::Theme;
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};
use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    AddEventListenerOptions, Event, EventTarget, HtmlCanvasElement, KeyboardEvent, MouseEvent,
    PointerEvent, ResizeObserver, ResizeObserverEntry, WheelEvent,
};

use crate::convert;
use crate::intro::{self, Intro};
use crate::logo::Logo;

/// Consecutive render failures after which the loop waits for the next input
/// or resize instead of retrying every frame.
const MAX_RETRIES: u32 = 3;
/// A Ctrl+wheel event this long after the previous one starts a new pinch.
const PINCH_GAP_MS: f64 = 150.0;

/// What a [`Host`] draws: a chart or a plot.
pub(crate) trait Scene: Sized + 'static {
    /// Prefix of console messages.
    const NAME: &'static str;
    /// Events `on` accepts.
    const EVENTS: &'static [&'static str];

    fn resize(&mut self, size: SizeCss, dpr: f32);
    fn handle_input(&mut self, event: InputEvent) -> InputResult;
    fn needs_redraw(&self) -> Invalidation;
    fn build_frame(&mut self, now_ms: f64, renderer: &WgpuRenderer);
    fn frame(&self) -> &Frame;
    fn data_view(&self) -> ChartDataView<'_>;
    /// Gradual display, driven by `intro`.
    fn set_reveal(&mut self, reveal: Option<Reveal>);
    /// Where the attribution logo sits: the bottom-left corner of the
    /// lowest plot area, device px.
    fn logo_anchor(&self) -> PointPx;

    /// Before deciding whether to draw (state borrowed).
    fn prepare(_state: &mut State<Self>) {}

    /// After drawing (state borrowed): appends the events to dispatch as
    /// (name, payload). Returns whether the sync peers must be woken.
    fn take_events(
        host: &Rc<Host<Self>>,
        state: &mut State<Self>,
        out: &mut Vec<(&'static str, JsValue)>,
    ) -> bool;

    /// After the events were dispatched (no borrow held).
    fn after_frame(_host: &Rc<Host<Self>>, _now: f64) {}
}

pub(crate) fn js_err(msg: impl std::fmt::Display) -> JsError {
    JsError::new(&msg.to_string())
}

pub(crate) fn window() -> Result<web_sys::Window, JsError> {
    web_sys::window().ok_or_else(|| js_err("no window"))
}

pub(crate) fn set(obj: &Object, key: &str, value: &JsValue) {
    let _ = Reflect::set(obj, &JsValue::from_str(key), value);
}

/// A DOM listener removed when dropped.
pub(crate) struct Listener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}

impl Listener {
    pub(crate) fn new(
        target: &EventTarget,
        name: &'static str,
        passive: bool,
        f: impl FnMut(Event) + 'static,
    ) -> Result<Self, JsError> {
        let callback = Closure::<dyn FnMut(Event)>::new(f);
        let options = AddEventListenerOptions::new();
        options.set_passive(passive);
        target
            .add_event_listener_with_callback_and_add_event_listener_options(
                name,
                callback.as_ref().unchecked_ref(),
                &options,
            )
            .map_err(|_| js_err("addEventListener failed"))?;
        Ok(Self {
            target: target.clone(),
            name,
            callback,
        })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
    }
}

pub(crate) struct State<S> {
    pub(crate) scene: S,
    pub(crate) renderer: WgpuRenderer,
    _ctx: GpuContext,
    /// The theme the object was created with.
    pub(crate) theme: Theme,
    pub(crate) canvas: HtmlCanvasElement,
    failures: u32,
    last_pinch: f64,
    gesture_scale: f64,
    pub(crate) dpr: f64,
    css: (f64, f64),
    /// Last pointer position over the canvas (CSS px).
    pub(crate) pointer: Option<(f64, f64)>,
}

type RafCallback = Closure<dyn FnMut(f64)>;
type Observer = (ResizeObserver, Closure<dyn FnMut(Array)>);

/// Members of a sync group in this page, woken when one posts.
pub(crate) type Peers<S> = Rc<RefCell<Vec<Weak<Host<S>>>>>;

pub(crate) struct Host<S> {
    pub(crate) state: RefCell<State<S>>,
    pub(crate) backend: &'static str,
    listeners: RefCell<Vec<(String, Function)>>,
    raf_callback: RefCell<Option<RafCallback>>,
    /// The pending animation frame. Outside `state` so that scheduling and
    /// the frame itself never depend on borrowing it: a frame that finds
    /// the state busy asks for the next one instead of being lost.
    raf: Cell<Option<i32>>,
    dom: RefCell<Vec<Listener>>,
    observer: RefCell<Option<Observer>>,
    dpr_watch: RefCell<Option<(web_sys::MediaQueryList, Listener)>>,
    pub(crate) sync_peers: RefCell<Option<Peers<S>>>,
    /// `playIntro` in progress.
    pub(crate) intro: RefCell<Option<Intro>>,
    /// The attribution logo, when shown.
    pub(crate) logo: RefCell<Option<Logo>>,
}

impl<S> Drop for Host<S> {
    fn drop(&mut self) {
        if let Some((observer, _)) = self.observer.get_mut().take() {
            observer.disconnect();
        }
        if let Some(id) = self.raf.get()
            && let Some(w) = web_sys::window()
        {
            let _ = w.cancel_animation_frame(id);
        }
    }
}

/// The GPU side of a new host: `open` creates it, the caller builds its
/// scene, `Host::start` puts them together.
pub(crate) struct Surface {
    pub(crate) ctx: GpuContext,
    pub(crate) renderer: WgpuRenderer,
    backend: &'static str,
    pub(crate) theme: Theme,
    pub(crate) css: (f64, f64),
    pub(crate) dpr: f64,
    /// `attributionLogo` (default `true`).
    logo: bool,
}

/// Reads `theme`, `renderer` and `attributionLogo` of `options`, sizes the canvas backing
/// store and creates the renderer.
pub(crate) async fn open(
    canvas: &HtmlCanvasElement,
    options: &JsValue,
) -> Result<Surface, JsError> {
    console_error_panic_hook::set_once();
    let theme = match string_option(options, "theme").as_deref() {
        None | Some("light") => Theme::light(),
        Some("dark") => Theme::dark(),
        Some(other) => return Err(js_err(format!("unknown theme '{other}'"))),
    };
    let mode = string_option(options, "renderer").unwrap_or_else(|| "auto".to_owned());
    let logo = crate::js::opt_bool(options, "attributionLogo")?.unwrap_or(true);
    let dpr = window()?.device_pixel_ratio();
    let rect = canvas.get_bounding_client_rect();
    let css = (rect.width().max(1.0), rect.height().max(1.0));
    let size = SizePx {
        w: (css.0 * dpr).round() as f32,
        h: (css.1 * dpr).round() as f32,
    };
    canvas.set_width(size.w as u32);
    canvas.set_height(size.h as u32);
    #[allow(unused_mut)] // without `text-browser`
    let (ctx, mut renderer, backend) = gpu(canvas, &mode, size).await?;
    #[cfg(feature = "text-browser")]
    if let Some(glyphs) = crate::browser_text::BrowserGlyphs::new() {
        renderer
            .text_engine_mut()
            .set_glyph_source(Some(Box::new(glyphs)));
    }
    Ok(Surface {
        ctx,
        renderer,
        backend,
        theme,
        css,
        dpr,
        logo,
    })
}

impl<S: Scene> Host<S> {
    /// Sizes `scene` to the canvas, installs input and resize handling and
    /// schedules the first frame.
    pub(crate) fn start(
        surface: Surface,
        canvas: &HtmlCanvasElement,
        mut scene: S,
    ) -> Result<Rc<Self>, JsError> {
        let Surface {
            ctx,
            renderer,
            backend,
            theme,
            css,
            dpr,
            logo,
        } = surface;
        scene.resize(
            SizeCss {
                w: css.0 as f32,
                h: css.1 as f32,
            },
            dpr as f32,
        );
        if canvas.tab_index() < 0 {
            canvas.set_tab_index(0);
        }
        let style = canvas.style();
        let _ = style.set_property("touch-action", "none");
        let _ = style.set_property("outline", "none");
        let host = Rc::new(Self {
            state: RefCell::new(State {
                scene,
                renderer,
                _ctx: ctx,
                theme,
                canvas: canvas.clone(),
                failures: 0,
                last_pinch: f64::NEG_INFINITY,
                gesture_scale: 1.0,
                dpr,
                css,
                pointer: None,
            }),
            backend,
            listeners: RefCell::new(Vec::new()),
            raf_callback: RefCell::new(None),
            raf: Cell::new(None),
            dom: RefCell::new(Vec::new()),
            observer: RefCell::new(None),
            dpr_watch: RefCell::new(None),
            sync_peers: RefCell::new(None),
            intro: RefCell::new(None),
            logo: RefCell::new(None),
        });
        let weak = Rc::downgrade(&host);
        *host.raf_callback.borrow_mut() = Some(Closure::new(move |now: f64| {
            if let Some(h) = weak.upgrade() {
                frame(&h, now);
            }
        }));
        install_input(&host, canvas)?;
        install_resize(&host, canvas)?;
        if logo {
            host.set_logo(true);
        }
        schedule(&host);
        Ok(host)
    }

    /// Runs `f` on the scene and schedules a frame.
    pub(crate) fn with_scene<T>(
        self: &Rc<Self>,
        f: impl FnOnce(&mut S) -> T,
    ) -> Result<T, JsError> {
        let out = {
            let mut state = self
                .state
                .try_borrow_mut()
                .map_err(|_| js_err(format!("{} is busy (re-entrant call)", S::NAME)))?;
            f(&mut state.scene)
        };
        schedule(self);
        Ok(out)
    }

    /// Adds a listener for one of `S::EVENTS`.
    pub(crate) fn listen(&self, event: &str, callback: Function) -> Result<(), JsError> {
        if !S::EVENTS.contains(&event) {
            return Err(js_err(format!("unknown event '{event}'")));
        }
        self.listeners
            .try_borrow_mut()
            .map_err(|_| js_err(format!("{} is busy", S::NAME)))?
            .push((event.to_owned(), callback));
        Ok(())
    }

    /// Removes a listener added with `listen`.
    pub(crate) fn unlisten(&self, event: &str, callback: &Function) -> Result<(), JsError> {
        self.listeners
            .try_borrow_mut()
            .map_err(|_| js_err(format!("{} is busy", S::NAME)))?
            .retain(|(n, f)| !(n == event && f == callback));
        Ok(())
    }

    /// Removes DOM and JS listeners (the GPU resources go with the last
    /// reference).
    pub(crate) fn shut_down(&self) {
        self.dom.borrow_mut().clear();
        self.dpr_watch.borrow_mut().take();
        self.listeners.borrow_mut().clear();
        if let Some(l) = self.logo.borrow_mut().take() {
            l.remove();
        }
        // A pending `playIntro` promise resolves rather than never settling.
        if let Some(i) = self.intro.borrow_mut().take() {
            i.resolve();
        }
    }

    /// Shows or removes the attribution logo (`setAttributionLogo`).
    pub(crate) fn set_logo(self: &Rc<Self>, on: bool) {
        let Ok(mut slot) = self.logo.try_borrow_mut() else {
            return;
        };
        if !on {
            if let Some(l) = slot.take() {
                l.remove();
            }
            return;
        }
        if slot.is_some() {
            return;
        }
        let Ok(state) = self.state.try_borrow() else {
            return;
        };
        *slot = Logo::new(&state.canvas, &state.theme);
        if let Some(l) = slot.as_ref() {
            l.place(&state.canvas, state.scene.logo_anchor(), state.dpr);
        }
    }
}

/// Runs `f` on the scene of `host` (an object that may have been
/// destroyed) and schedules a frame.
pub(crate) fn with_weak<S: Scene, T>(
    host: &Weak<Host<S>>,
    f: impl FnOnce(&mut S) -> T,
) -> Result<T, JsError> {
    host.upgrade()
        .ok_or_else(|| js_err(format!("the {} was destroyed", S::NAME)))?
        .with_scene(f)
}

/// Requests an animation frame unless one is pending.
pub(crate) fn schedule<S>(host: &Rc<Host<S>>) {
    if host.raf.get().is_some() {
        return;
    }
    // While the state is borrowed (a call in progress), schedule anyway: the
    // frame checks again.
    if host
        .state
        .try_borrow()
        .is_ok_and(|s| s.failures >= MAX_RETRIES)
    {
        return;
    }
    request_frame(host);
}

fn request_frame<S>(host: &Host<S>) {
    let Ok(callback) = host.raf_callback.try_borrow() else {
        return;
    };
    let (Some(cb), Some(w)) = (callback.as_ref(), web_sys::window()) else {
        return;
    };
    host.raf
        .set(w.request_animation_frame(cb.as_ref().unchecked_ref()).ok());
}

/// Draws `host` now (within the current animation frame) instead of at the
/// next one.
pub(crate) fn frame_now<S: Scene>(host: &Rc<Host<S>>, now: f64) {
    if let Some(id) = host.raf.take()
        && let Some(w) = web_sys::window()
    {
        let _ = w.cancel_animation_frame(id);
    }
    frame(host, now);
}

/// One animation frame: build and render if needed, then dispatch events.
fn frame<S: Scene>(host: &Rc<Host<S>>, now: f64) {
    let mut events = Vec::new();
    let (wake, again, resolved) = {
        host.raf.set(None);
        let Ok(mut guard) = host.state.try_borrow_mut() else {
            // Busy (re-entered from a callback): try again next frame.
            request_frame(host);
            return;
        };
        let state = &mut *guard;
        let (animating, finished) = intro::advance(host, &mut state.scene, now);
        S::prepare(state);
        // A canvas with a CSS size of 0 (`display: none`, a collapsed
        // container) has nothing to draw into: skip the frame, keeping the
        // changes pending, instead of failing with "invalid render target
        // size". `apply_size` schedules a frame when it gets a size again.
        let drawable = drawable(state.css, state.dpr);
        if drawable && state.scene.needs_redraw() != Invalidation::None {
            state.scene.build_frame(now, &state.renderer);
            if let Ok(l) = host.logo.try_borrow()
                && let Some(l) = l.as_ref()
            {
                l.place(&state.canvas, state.scene.logo_anchor(), state.dpr);
            }
            match state
                .renderer
                .render(state.scene.frame(), &state.scene.data_view())
            {
                Ok(()) => state.failures = 0,
                Err(e) => {
                    state.failures += 1;
                    let detail = state.renderer.take_gpu_error().unwrap_or_default();
                    web_sys::console::error_1(&JsValue::from_str(&format!(
                        "{}: {e} {detail}",
                        S::NAME
                    )));
                }
            }
        }
        let wake = S::take_events(host, state, &mut events);
        (
            wake,
            drawable && (animating || state.scene.needs_redraw() != Invalidation::None),
            finished,
        )
    };
    if wake {
        wake_peers(host);
    }
    dispatch(host, &events);
    if let Some(i) = resolved {
        i.resolve();
    }
    S::after_frame(host, now);
    if again {
        schedule(host);
    }
}

/// Schedules a frame of every other member of the sync group: they apply
/// the messages this one posted.
pub(crate) fn wake_peers<S>(host: &Rc<Host<S>>) {
    let peers = host.sync_peers.borrow().clone();
    let Some(peers) = peers else {
        return;
    };
    let members: Vec<Rc<Host<S>>> = peers.borrow().iter().filter_map(Weak::upgrade).collect();
    for other in &members {
        if !Rc::ptr_eq(other, host) {
            schedule(other);
        }
    }
}

fn dispatch<S>(host: &Host<S>, events: &[(&'static str, JsValue)]) {
    if events.is_empty() {
        return;
    }
    // Clone the listener list so that callbacks may call `on`/`off`.
    let listeners = host.listeners.borrow().clone();
    for (name, payload) in events {
        for (n, f) in &listeners {
            if n == name
                && let Err(e) = f.call1(&JsValue::NULL, payload)
            {
                report_error(&e);
            }
        }
    }
}

/// Reports an exception thrown by a listener like an uncaught one
/// (`reportError`, or `console.error` where it is missing) instead of
/// dropping it; the other listeners still run.
pub(crate) fn report_error(e: &JsValue) {
    let global = js_sys::global();
    let report = Reflect::get(&global, &JsValue::from_str("reportError"))
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok());
    match report {
        Some(f) => {
            let _ = f.call1(&JsValue::NULL, e);
        }
        None => web_sys::console::error_1(e),
    }
}

/// Whether a canvas of `css` CSS px at `dpr` has at least one device pixel
/// each way (the renderer rejects smaller targets).
fn drawable((w, h): (f64, f64), dpr: f64) -> bool {
    (w * dpr).round() >= 1.0 && (h * dpr).round() >= 1.0
}

/// Sizes the canvas backing store and the scene to `css` × dpr.
fn apply_size<S: Scene>(host: &Rc<Host<S>>, css: Option<(f64, f64)>) {
    let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio());
    {
        let Ok(mut state) = host.state.try_borrow_mut() else {
            return;
        };
        let (w, h) = css.unwrap_or(state.css);
        state.css = (w, h);
        state.dpr = dpr;
        let (pw, ph) = ((w * dpr).round().max(1.0), (h * dpr).round().max(1.0));
        // Setting the backing store size, even to the same value, discards
        // what was drawn (Chromium's WebGPU canvas); a scene whose size did
        // not change need not redraw (the plot does not), so the canvas is
        // only touched when its size changes.
        if state.canvas.width() != pw as u32 || state.canvas.height() != ph as u32 {
            state.canvas.set_width(pw as u32);
            state.canvas.set_height(ph as u32);
        }
        state.scene.resize(
            SizeCss {
                w: w as f32,
                h: h as f32,
            },
            dpr as f32,
        );
        state.renderer.resize(SizePx {
            w: pw as f32,
            h: ph as f32,
        });
        state.failures = 0;
    }
    schedule(host);
}

/// Re-applies the size when `devicePixelRatio` changes (zoom, moving the
/// window to another display). A media query matches only the current ratio,
/// so it is re-armed after every change.
fn watch_dpr<S: Scene>(weak: &Weak<Host<S>>) {
    let Some(host) = weak.upgrade() else {
        return;
    };
    let Some(w) = web_sys::window() else {
        return;
    };
    let query = format!("(resolution: {}dppx)", w.device_pixel_ratio());
    let Ok(Some(mq)) = w.match_media(&query) else {
        return;
    };
    let weak2 = weak.clone();
    let Ok(listener) = Listener::new(&mq, "change", true, move |_| {
        if let Some(h) = weak2.upgrade() {
            apply_size(&h, None);
        }
        watch_dpr(&weak2);
    }) else {
        return;
    };
    *host.dpr_watch.borrow_mut() = Some((mq, listener));
}

fn mods_of(e: &MouseEvent) -> Modifiers {
    Modifiers {
        shift: e.shift_key(),
        ctrl: e.ctrl_key(),
        alt: e.alt_key(),
        meta: e.meta_key(),
    }
}

/// Position relative to the canvas in fractional CSS pixels.
fn pos_of(e: &MouseEvent) -> PointCss {
    let get = |k: &str| {
        Reflect::get(e, &JsValue::from_str(k))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32
    };
    PointCss {
        x: get("offsetX"),
        y: get("offsetY"),
    }
}

/// Feeds an input event to the scene, updates the cursor and schedules a
/// frame. Returns whether the scene consumed it.
fn input<S: Scene>(host: &Rc<Host<S>>, event: InputEvent) -> bool {
    if intro::ends_intro(&event) {
        intro::finish(host);
    }
    let consumed = {
        let Ok(mut state) = host.state.try_borrow_mut() else {
            return false;
        };
        let r = state.scene.handle_input(event);
        let _ = state
            .canvas
            .style()
            .set_property("cursor", convert::cursor_css(r.cursor));
        state.failures = 0;
        r.consumed
    };
    schedule(host);
    consumed
}

/// A DOM listener that forwards to `f` while the host is alive.
fn on<S: 'static>(
    w: &Weak<Host<S>>,
    target: &EventTarget,
    name: &'static str,
    passive: bool,
    f: impl Fn(&Rc<Host<S>>, Event) + 'static,
) -> Result<Listener, JsError> {
    let w = w.clone();
    Listener::new(target, name, passive, move |e| {
        if let Some(s) = w.upgrade() {
            f(&s, e);
        }
    })
}

#[allow(clippy::too_many_lines)]
fn install_input<S: Scene>(host: &Rc<Host<S>>, canvas: &HtmlCanvasElement) -> Result<(), JsError> {
    let target: &EventTarget = canvas.as_ref();
    let mut dom = Vec::new();
    let w = Rc::downgrade(host);
    dom.push(on(&w, target, "pointerdown", true, |s, e| {
        let Some(e) = e.dyn_ref::<PointerEvent>() else {
            return;
        };
        let Some(button) = convert::button(e.button()) else {
            return;
        };
        if let Ok(state) = s.state.try_borrow() {
            let _ = state.canvas.set_pointer_capture(e.pointer_id());
            let _ = state.canvas.focus();
        }
        input(
            s,
            InputEvent::PointerDown {
                id: u64::from(e.pointer_id().unsigned_abs()),
                pos: pos_of(e),
                button,
                mods: mods_of(e),
                time_ms: e.time_stamp(),
            },
        );
    })?);
    dom.push(on(&w, target, "pointermove", true, |s, e| {
        let Some(e) = e.dyn_ref::<PointerEvent>() else {
            return;
        };
        let pos = pos_of(e);
        if let Ok(mut state) = s.state.try_borrow_mut() {
            state.pointer = Some((f64::from(pos.x), f64::from(pos.y)));
        }
        input(
            s,
            InputEvent::PointerMove {
                id: u64::from(e.pointer_id().unsigned_abs()),
                pos: pos_of(e),
                mods: mods_of(e),
                time_ms: e.time_stamp(),
            },
        );
    })?);
    let up = |s: &Rc<Host<S>>, e: Event| {
        let Some(e) = e.dyn_ref::<PointerEvent>() else {
            return;
        };
        let Some(button) = convert::button(e.button().max(0)) else {
            return;
        };
        input(
            s,
            InputEvent::PointerUp {
                id: u64::from(e.pointer_id().unsigned_abs()),
                pos: pos_of(e),
                button,
                mods: mods_of(e),
                time_ms: e.time_stamp(),
            },
        );
    };
    dom.push(on(&w, target, "pointerup", true, up)?);
    dom.push(on(&w, target, "pointercancel", true, up)?);
    dom.push(on(&w, target, "pointerleave", true, |s, _| {
        if let Ok(mut state) = s.state.try_borrow_mut() {
            state.pointer = None;
        }
        input(s, InputEvent::PointerLeave);
    })?);
    dom.push(on(&w, target, "wheel", false, |s, e| {
        let Some(e) = e.dyn_ref::<WheelEvent>() else {
            return;
        };
        let pos = pos_of(e);
        let consumed = if e.ctrl_key() {
            let phase = match s.state.try_borrow_mut() {
                Ok(mut state) => {
                    let t = e.time_stamp();
                    let start = t - state.last_pinch > PINCH_GAP_MS;
                    state.last_pinch = t;
                    if start {
                        GesturePhase::Start
                    } else {
                        GesturePhase::Update
                    }
                }
                Err(_) => GesturePhase::Update,
            };
            input(
                s,
                InputEvent::Pinch {
                    phase,
                    center: pos,
                    scale: convert::pinch_scale(e.delta_y()),
                },
            )
        } else {
            input(
                s,
                InputEvent::Wheel {
                    pos,
                    delta: convert::wheel(e.delta_mode(), e.delta_x(), e.delta_y()),
                    mods: mods_of(e),
                },
            )
        };
        if consumed {
            e.prevent_default();
        }
    })?);
    // Safari reports trackpad pinch as non-standard gesture events with a
    // cumulative `scale`.
    let gesture = |s: &Rc<Host<S>>, e: Event, phase: GesturePhase| {
        e.prevent_default();
        let Some(m) = e.dyn_ref::<MouseEvent>() else {
            return;
        };
        let scale = Reflect::get(&e, &JsValue::from_str("scale"))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let step = match s.state.try_borrow_mut() {
            Ok(mut state) => {
                let prev = if phase == GesturePhase::Start {
                    1.0
                } else {
                    state.gesture_scale
                };
                state.gesture_scale = scale;
                if prev > 0.0 { scale / prev } else { 1.0 }
            }
            Err(_) => 1.0,
        };
        input(
            s,
            InputEvent::Pinch {
                phase,
                center: pos_of(m),
                scale: if step.is_finite() && step > 0.0 {
                    step as f32
                } else {
                    1.0
                },
            },
        );
    };
    dom.push(on(&w, target, "gesturestart", false, move |s, e| {
        gesture(s, e, GesturePhase::Start);
    })?);
    dom.push(on(&w, target, "gesturechange", false, move |s, e| {
        gesture(s, e, GesturePhase::Update);
    })?);
    dom.push(on(&w, target, "gestureend", false, move |s, e| {
        gesture(s, e, GesturePhase::End);
    })?);
    dom.push(on(&w, target, "dblclick", true, |s, e| {
        let Some(e) = e.dyn_ref::<MouseEvent>() else {
            return;
        };
        if let Some(button) = convert::button(e.button()) {
            input(
                s,
                InputEvent::DoubleClick {
                    pos: pos_of(e),
                    button,
                },
            );
        }
    })?);
    let key = |s: &Rc<Host<S>>, e: Event, pressed: bool| {
        let Some(e) = e.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        let Some(k) = convert::key(&e.key()) else {
            return;
        };
        let consumed = input(
            s,
            InputEvent::Key {
                key: k,
                pressed,
                mods: Modifiers {
                    shift: e.shift_key(),
                    ctrl: e.ctrl_key(),
                    alt: e.alt_key(),
                    meta: e.meta_key(),
                },
            },
        );
        if consumed {
            e.prevent_default();
        }
    };
    dom.push(on(&w, target, "keydown", false, move |s, e| {
        key(s, e, true);
    })?);
    dom.push(on(&w, target, "keyup", false, move |s, e| {
        key(s, e, false);
    })?);
    *host.dom.borrow_mut() = dom;
    Ok(())
}

fn install_resize<S: Scene>(host: &Rc<Host<S>>, canvas: &HtmlCanvasElement) -> Result<(), JsError> {
    let w = Rc::downgrade(host);
    let callback = Closure::<dyn FnMut(Array)>::new(move |entries: Array| {
        let Some(s) = w.upgrade() else {
            return;
        };
        let Some(entry) = entries.get(0).dyn_into::<ResizeObserverEntry>().ok() else {
            return;
        };
        let rect = entry.content_rect();
        apply_size(&s, Some((rect.width(), rect.height())));
    });
    let observer = ResizeObserver::new(callback.as_ref().unchecked_ref())
        .map_err(|_| js_err("ResizeObserver is not available"))?;
    observer.observe(canvas);
    *host.observer.borrow_mut() = Some((observer, callback));
    watch_dpr(&Rc::downgrade(host));
    Ok(())
}

pub(crate) fn string_option(options: &JsValue, key: &str) -> Option<String> {
    if !options.is_object() {
        return None;
    }
    Reflect::get(options, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
}

/// Creates the GPU context and the surface renderer: WebGPU first (an
/// adapter is requested before the canvas gets a context, because a canvas
/// that has a `webgpu` context can no longer get a `webgl2` one), then WebGL2.
async fn gpu(
    canvas: &HtmlCanvasElement,
    mode: &str,
    size: SizePx,
) -> Result<(GpuContext, WgpuRenderer, &'static str), JsError> {
    let mut errors = Vec::new();
    if mode == "auto" || mode == "webgpu" {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::BROWSER_WEBGPU;
        let instance = wgpu::Instance::new(desc);
        match instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => {
                let surface = instance
                    .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
                    .map_err(js_err)?;
                let ctx = GpuContext::from_adapter(instance, adapter)
                    .await
                    .map_err(js_err)?;
                let renderer = WgpuRenderer::with_surface(&ctx, surface, size).map_err(js_err)?;
                return Ok((ctx, renderer, "webgpu"));
            }
            Err(_) => errors.push("WebGPU is not available"),
        }
    }
    if mode == "auto" || mode == "webgl2" {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::GL;
        let instance = wgpu::Instance::new(desc);
        match instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone())) {
            Ok(surface) => match GpuContext::request(instance, Some(&surface)).await {
                Ok(ctx) => {
                    let renderer =
                        WgpuRenderer::with_surface(&ctx, surface, size).map_err(js_err)?;
                    return Ok((ctx, renderer, "webgl2"));
                }
                Err(_) => errors.push("WebGL2 is not available"),
            },
            Err(_) => errors.push("WebGL2 is not available"),
        }
    }
    if errors.is_empty() {
        return Err(js_err(format!(
            "unknown renderer '{mode}' (expected 'auto', 'webgpu' or 'webgl2')"
        )));
    }
    Err(js_err(errors.join("; ")))
}
