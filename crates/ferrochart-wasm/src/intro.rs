//! `playIntro` / `finishIntro`: the host
//! moves the scene's reveal from 0 to 1 in its animation frames. The chart
//! is empty at once (grid and axes only); after `delay` ms the data is drawn
//! from left to right over `duration` ms; then the reveal is removed and the
//! promise resolves. Pointer presses, wheel, pinch and keys end it first.

use std::rc::Rc;

use ferrochart_core::frame::Reveal;
use ferrochart_core::geom::Rgba;
use ferrochart_core::interaction::InputEvent;
use js_sys::{Function, Promise};
use wasm_bindgen::prelude::*;

use crate::host::{Host, Scene, js_err, schedule};

/// Default `duration`, ms.
const DURATION_MS: f64 = 900.0;

/// An intro in progress.
pub(crate) struct Intro {
    delay: f64,
    duration: f64,
    pen: Option<Rgba>,
    /// Time of the first animation frame (set by that frame).
    start: Option<f64>,
    resolve: Function,
}

impl Intro {
    /// Settles the `playIntro` promise.
    pub(crate) fn resolve(self) {
        let _ = self.resolve.call0(&JsValue::UNDEFINED);
    }
}

/// Input that ends the intro: a deliberate action, not a pointer passing by.
pub(crate) fn ends_intro(event: &InputEvent) -> bool {
    matches!(
        event,
        InputEvent::PointerDown { .. }
            | InputEvent::Wheel { .. }
            | InputEvent::Pinch { .. }
            | InputEvent::Key { .. }
            | InputEvent::DoubleClick { .. }
    )
}

/// `playIntro(options)`: `{ duration = 900, delay = 0, pen = null }`.
pub(crate) fn play<S: Scene>(host: &Rc<Host<S>>, options: &JsValue) -> Result<Promise, JsError> {
    let duration = crate::js::opt_f64(options, "duration")?.unwrap_or(DURATION_MS);
    let delay = crate::js::opt_f64(options, "delay")?.unwrap_or(0.0);
    if !duration.is_finite() || duration < 0.0 || !delay.is_finite() || delay < 0.0 {
        return Err(js_err("'duration' and 'delay' must be finite numbers >= 0"));
    }
    let pen = crate::js::opt_color(options, "pen")?.flatten();
    // A running intro ends (its promise resolves) before the new one.
    finish(host);
    let mut resolve = None;
    let promise = Promise::new(&mut |res, _rej| resolve = Some(res));
    let resolve = resolve.ok_or_else(|| js_err("Promise executor did not run"))?;
    let intro = Intro {
        delay,
        duration,
        pen,
        start: None,
        resolve,
    };
    {
        let mut slot = host
            .intro
            .try_borrow_mut()
            .map_err(|_| js_err(format!("{} is busy", S::NAME)))?;
        *slot = Some(intro);
    }
    // Empty at once, before the first animation frame.
    host.with_scene(|s| s.set_reveal(Some(Reveal { progress: 0.0, pen })))?;
    Ok(promise)
}

/// `finishIntro()` and input during the intro: full display now, the
/// promise resolves.
pub(crate) fn finish<S: Scene>(host: &Rc<Host<S>>) {
    let Some(intro) = host.intro.try_borrow_mut().ok().and_then(|mut i| i.take()) else {
        return;
    };
    if let Ok(mut state) = host.state.try_borrow_mut() {
        state.scene.set_reveal(None);
    }
    schedule(host);
    intro.resolve();
}

/// One animation frame: sets the reveal for `now`. Returns whether the
/// intro is still running (another frame is needed) and, when it has just
/// finished, the intro whose promise to resolve after the frame.
pub(crate) fn advance<S: Scene>(host: &Host<S>, scene: &mut S, now: f64) -> (bool, Option<Intro>) {
    let Ok(mut slot) = host.intro.try_borrow_mut() else {
        return (false, None);
    };
    let Some(intro) = slot.as_mut() else {
        return (false, None);
    };
    let start = *intro.start.get_or_insert(now);
    if let Some(progress) = crate::convert::intro_progress(now - start, intro.delay, intro.duration)
    {
        scene.set_reveal(Some(Reveal {
            progress,
            pen: intro.pen,
        }));
        return (true, None);
    }
    scene.set_reveal(None);
    (false, slot.take())
}
