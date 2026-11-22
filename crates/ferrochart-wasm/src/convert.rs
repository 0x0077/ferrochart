//! DOM → chart input conversion that does not need a browser: key names, wheel deltas,
//! trackpad pinch, cursor names, and
//! the column checks of `setData`, and the timing of `playIntro`.
// Used only by the wasm32 binding; compiled everywhere so it is tested natively.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use ferrochart_core::geom::Css;
use ferrochart_core::interaction::{Button, CursorIcon, Key, WheelDelta};

/// `WheelEvent.deltaMode` values.
const DOM_DELTA_LINE: u32 = 1;
const DOM_DELTA_PAGE: u32 = 2;
/// Lines per page for `DOM_DELTA_PAGE` (rare; Firefox with some settings).
const LINES_PER_PAGE: f32 = 10.0;
/// Ctrl+wheel (Chrome/Firefox trackpad pinch) zoom per pixel of `deltaY`.
const PINCH_PER_PX: f64 = 0.01;

/// `KeyboardEvent.key` → chart key.
pub(crate) fn key(name: &str) -> Option<Key> {
    Some(match name {
        "ArrowLeft" => Key::ArrowLeft,
        "ArrowRight" => Key::ArrowRight,
        "ArrowUp" => Key::ArrowUp,
        "ArrowDown" => Key::ArrowDown,
        "Home" => Key::Home,
        "End" => Key::End,
        "Escape" => Key::Escape,
        "Delete" => Key::Delete,
        "Enter" => Key::Enter,
        _ => {
            let mut chars = name.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Key::Char(c)
        }
    })
}

/// `MouseEvent.button` → chart button.
pub(crate) fn button(b: i16) -> Option<Button> {
    match b {
        0 => Some(Button::Left),
        1 => Some(Button::Middle),
        2 => Some(Button::Right),
        _ => None,
    }
}

/// `WheelEvent` deltas → chart wheel delta (same sign convention as the DOM).
pub(crate) fn wheel(mode: u32, dx: f64, dy: f64) -> WheelDelta {
    let (x, y) = (dx as f32, dy as f32);
    match mode {
        DOM_DELTA_LINE => WheelDelta::Lines { x, y },
        DOM_DELTA_PAGE => WheelDelta::Lines {
            x: x * LINES_PER_PAGE,
            y: y * LINES_PER_PAGE,
        },
        _ => WheelDelta::Pixels {
            x: Css(x),
            y: Css(y),
        },
    }
}

/// Zoom factor of a Ctrl+wheel event, which is how Chrome and Firefox report
/// a trackpad pinch: scrolling up (negative `deltaY`) zooms in.
pub(crate) fn pinch_scale(dy: f64) -> f32 {
    let s = (-dy * PINCH_PER_PX).exp() as f32;
    if s.is_finite() && s > 0.0 { s } else { 1.0 }
}

/// CSS `cursor` value for a chart cursor.
pub(crate) fn cursor_css(c: CursorIcon) -> &'static str {
    match c {
        CursorIcon::Crosshair => "crosshair",
        CursorIcon::Grab => "grab",
        CursorIcon::Grabbing => "grabbing",
        CursorIcon::EwResize => "ew-resize",
        CursorIcon::NsResize => "ns-resize",
        CursorIcon::Pointer => "pointer",
        CursorIcon::Move => "move",
        _ => "default",
    }
}

/// Float64 times (UTC ms) → i64, rejecting non-finite or fractional values.
pub(crate) fn times_from_f64(values: &[f64]) -> Result<Vec<i64>, usize> {
    values
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            if t.is_finite() && t.fract() == 0.0 && t.abs() < 9.0e15 {
                Ok(t as i64)
            } else {
                Err(i)
            }
        })
        .collect()
}

/// Reveal progress of `playIntro` at `elapsed` ms after it started: 0
/// during `delay`, then an ease-in-out cubic over `duration`; `None` once
/// finished (also for a zero or invalid duration).
pub(crate) fn intro_progress(elapsed: f64, delay: f64, duration: f64) -> Option<f32> {
    if !(duration.is_finite() && duration > 0.0) {
        return None;
    }
    let t = (elapsed - delay.max(0.0)) / duration;
    if t.is_nan() || t >= 1.0 {
        return None;
    }
    let t = t.max(0.0);
    let eased = if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) * 0.5
    };
    Some(eased as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intro_timing() {
        assert_eq!(intro_progress(0.0, 200.0, 900.0), Some(0.0));
        assert_eq!(intro_progress(199.0, 200.0, 900.0), Some(0.0));
        assert_eq!(intro_progress(650.0, 200.0, 900.0), Some(0.5));
        let a = intro_progress(300.0, 0.0, 900.0).unwrap();
        let b = intro_progress(600.0, 0.0, 900.0).unwrap();
        assert!(0.0 < a && a < b && b < 1.0);
        assert_eq!(intro_progress(900.0, 0.0, 900.0), None);
        assert_eq!(intro_progress(0.0, 0.0, 0.0), None);
        assert_eq!(intro_progress(0.0, 0.0, f64::NAN), None);
        assert_eq!(intro_progress(f64::NAN, 0.0, 900.0), None);
    }

    #[test]
    fn keys() {
        assert_eq!(key("ArrowLeft"), Some(Key::ArrowLeft));
        assert_eq!(key("+"), Some(Key::Char('+')));
        assert_eq!(key("é"), Some(Key::Char('é')));
        assert_eq!(key("Shift"), None);
        assert_eq!(key(""), None);
    }

    #[test]
    fn wheel_modes() {
        assert_eq!(
            wheel(0, 1.0, -2.0),
            WheelDelta::Pixels {
                x: Css(1.0),
                y: Css(-2.0)
            }
        );
        assert_eq!(wheel(1, 0.0, 3.0), WheelDelta::Lines { x: 0.0, y: 3.0 });
        assert_eq!(wheel(2, 0.0, 1.0), WheelDelta::Lines { x: 0.0, y: 10.0 });
    }

    #[test]
    fn pinch_up_zooms_in() {
        assert!(pinch_scale(-10.0) > 1.0);
        assert!(pinch_scale(10.0) < 1.0);
        assert!(pinch_scale(f64::NAN).to_bits() == 1.0_f32.to_bits());
        assert!(pinch_scale(-1e9).to_bits() == 1.0_f32.to_bits());
    }

    #[test]
    fn float_times() {
        assert_eq!(times_from_f64(&[1.0, 2.0]), Ok(vec![1, 2]));
        assert_eq!(times_from_f64(&[1.0, 2.5]), Err(1));
        assert_eq!(times_from_f64(&[f64::NAN]), Err(0));
    }
}
