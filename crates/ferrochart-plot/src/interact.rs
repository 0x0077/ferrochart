//! Input handling (pan, zoom, kinetic panning, hover, click) and the events
//! a plot reports to the host.
//!
//! - Dragging in the plot pans every unlocked axis that can move (category
//!   axes only when not every category is visible); releasing keeps the
//!   motion going with the same decay as on charts.
//! - The wheel or a pinch zooms the x axis around the pointer (pans a
//!   category x axis); over a y axis, or with Alt, it zooms that y axis.
//!   Shift and horizontal scrolls pan. Dragging along an axis zooms it.
//! - Double click fits.
//! - Touch: a press shows the hover at the finger (no hover otherwise);
//!   moving more than 8 CSS px pans and clears it; two fingers zoom x and
//!   the unlocked y axes by the change of their distance along each
//!   direction around their midpoint, and pan with it; hits reach 8 CSS px
//!   further. The host's pinch is ignored during a two-finger pinch.
//!

use ferrochart_core::frame::Invalidation;
use ferrochart_core::geom::{PointCss, PointPx};
use ferrochart_core::interaction::{
    Button, CursorIcon, GesturePhase, InputEvent, InputResult, KINETIC_MIN_SPEED, KINETIC_TAU_MS,
    Modifiers, PointerKind, VelocityTracker, WheelDelta,
};

use crate::axis::{AxisId, AxisKind};
use crate::draw::Geometry;
use crate::hit::{Shape, hit_at};
use crate::mark::MarkId;
use crate::plot::{Plot, geometry};

/// Pixels per wheel line, wheel zoom per pixel and axis-drag zoom per pixel
/// (CSS px), as for charts.
const LINE_PX: f32 = 40.0;
const WHEEL_ZOOM: f64 = 0.002;
const AXIS_DRAG_ZOOM: f64 = 0.005;
/// A press becomes a drag beyond this distance (CSS px).
const DRAG_THRESHOLD: f32 = 3.0;
/// The same for a finger.
const TOUCH_DRAG_THRESHOLD: f32 = 8.0;
/// A finger hits points this much further away (CSS px).
pub(crate) const TOUCH_SLOP_CSS: f32 = 8.0;
/// Two fingers closer than this along a direction do not zoom that
/// direction (CSS px): the ratio would jump.
const PINCH_MIN_SPAN_CSS: f32 = 16.0;

/// An element under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// The mark.
    pub mark: MarkId,
    /// Index of the point in the mark's data.
    pub index: usize,
    /// The point's x.
    pub x: f64,
    /// The point's y (waterfall subtotals and totals: the running total).
    pub y: f64,
    /// Where the element is drawn (a point, or the center of a bar or
    /// cell), in CSS pixels.
    pub pos: PointCss,
}

/// Something the host may react to; see [`Plot::drain_events`](crate::Plot::drain_events).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlotEvent {
    /// The hovered element changed (after a pan or zoom under a still
    /// pointer, [`Plot::hit_test`](crate::Plot::hit_test) gives its new
    /// position).
    Hover(Option<Hit>),
    /// A click, with the element under it and the axis values at the pointer.
    Click {
        /// The element clicked, if any.
        hit: Option<Hit>,
        /// x at the pointer.
        x: f64,
        /// Value of the right axis (the left one if the right is hidden) at
        /// the pointer.
        y: f64,
    },
    /// The visible range of an axis changed.
    ViewChanged,
}

/// The gesture in progress.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Gesture {
    #[default]
    Idle,
    /// Pressed in the plot, not moved far enough to pan yet.
    Press {
        id: u64,
        start: PointPx,
    },
    Pan {
        id: u64,
        last: PointPx,
    },
    /// Two fingers: `(id, position)` of each.
    Pinch {
        a: (u64, PointPx),
        b: (u64, PointPx),
    },
    /// Dragging along an axis zooms it.
    Scale {
        id: u64,
        axis: AxisId,
        last: PointPx,
    },
}

/// Kinetic panning: velocity in px/ms and the time of the last step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Kinetic {
    pub vx: f64,
    pub vy: f64,
    pub last_ms: f64,
}

/// Interaction state.
#[derive(Clone, Debug, Default)]
pub(crate) struct Interaction {
    pub gesture: Gesture,
    tx: VelocityTracker,
    ty: VelocityTracker,
    pub kinetic: Option<Kinetic>,
    /// Pointer position over the plot (physical px).
    pub pointer: Option<PointPx>,
    /// The hovered element and what to highlight.
    pub hover: Option<(Hit, Shape)>,
    /// The last pointer event came from a finger.
    pub touch: bool,
}

/// Where a point is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Region {
    Plot,
    Axis(AxisId),
    Outside,
}

fn consumed(cursor: CursorIcon) -> InputResult {
    InputResult {
        consumed: true,
        cursor,
    }
}

impl Plot {
    /// Handles an input event; see the module documentation of
    /// `interact`. Events for the host are queued
    /// for [`drain_events`](Self::drain_events).
    pub fn handle_input(&mut self, event: InputEvent) -> InputResult {
        let dpr = self.dpr;
        match event {
            InputEvent::PointerDown {
                id,
                kind,
                pos,
                button: Button::Left,
                time_ms,
                ..
            } => {
                let p = pos.to_px(dpr);
                let touch = kind == PointerKind::Touch;
                self.input.touch = touch;
                if touch && let Some(result) = self.second_finger(id, p) {
                    return result;
                }
                if !matches!(self.input.gesture, Gesture::Idle) {
                    return InputResult::default();
                }
                self.input.kinetic = None;
                match self.region(p) {
                    Region::Plot => {
                        self.input.gesture = Gesture::Press { id, start: p };
                        self.input.tx.clear();
                        self.input.ty.clear();
                        self.input.tx.record(time_ms, p.x);
                        self.input.ty.record(time_ms, p.y);
                        if touch {
                            // A finger has no hover: show it at the press.
                            self.set_pointer(Some(p));
                        }
                        consumed(self.hover_cursor())
                    }
                    Region::Axis(axis) if self.scalable(axis) => {
                        self.input.gesture = Gesture::Scale { id, axis, last: p };
                        consumed(axis_cursor(axis))
                    }
                    _ => InputResult::default(),
                }
            }
            InputEvent::PointerMove {
                id,
                kind,
                pos,
                time_ms,
                ..
            } => {
                let p = pos.to_px(dpr);
                self.input.touch = kind == PointerKind::Touch;
                match self.input.gesture {
                    Gesture::Pinch { a, b } if a.0 == id || b.0 == id => self.pinch_move(id, p),
                    Gesture::Press { id: d, start } if d == id => {
                        if (p.x - start.x).hypot(p.y - start.y) > self.drag_threshold() {
                            self.input.gesture = Gesture::Pan { id, last: start };
                            self.set_pointer(None);
                            return self.drag_to(p, time_ms);
                        }
                        consumed(self.hover_cursor())
                    }
                    Gesture::Pan { id: d, .. } if d == id => self.drag_to(p, time_ms),
                    Gesture::Scale { id: d, axis, last } if d == id => {
                        self.scale_drag(axis, last, p);
                        self.input.gesture = Gesture::Scale { id, axis, last: p };
                        consumed(axis_cursor(axis))
                    }
                    _ => {
                        let region = self.region(p);
                        self.set_pointer((region == Region::Plot).then_some(p));
                        match region {
                            Region::Plot => InputResult {
                                consumed: false,
                                cursor: self.hover_cursor(),
                            },
                            Region::Axis(a) if self.scalable(a) => InputResult {
                                consumed: false,
                                cursor: axis_cursor(a),
                            },
                            _ => InputResult::default(),
                        }
                    }
                }
            }
            InputEvent::PointerUp {
                id,
                kind,
                pos,
                time_ms,
                ..
            } => {
                let p = pos.to_px(dpr);
                self.input.touch = kind == PointerKind::Touch;
                if self.end_pinch_finger(id) {
                    return consumed(CursorIcon::Grabbing);
                }
                match self.input.gesture {
                    Gesture::Press { id: d, .. } if d == id => {
                        self.input.gesture = Gesture::Idle;
                        self.click(p);
                        consumed(self.hover_cursor())
                    }
                    Gesture::Pan { id: d, .. } if d == id => {
                        self.input.gesture = Gesture::Idle;
                        let vx = self.input.tx.release_velocity(time_ms);
                        let vy = self.input.ty.release_velocity(time_ms);
                        if vx.hypot(vy) >= KINETIC_MIN_SPEED {
                            self.input.kinetic = Some(Kinetic {
                                vx,
                                vy,
                                last_ms: time_ms,
                            });
                            self.invalidation = self.invalidation.max(Invalidation::Light);
                        }
                        // A finger that panned leaves no hover behind.
                        let hover = self.layout.plot.contains(p) && !self.input.touch;
                        self.set_pointer(hover.then_some(p));
                        consumed(self.hover_cursor())
                    }
                    Gesture::Scale { id: d, axis, .. } if d == id => {
                        self.input.gesture = Gesture::Idle;
                        consumed(axis_cursor(axis))
                    }
                    _ => InputResult::default(),
                }
            }
            InputEvent::PointerCancel { id } => {
                if self.end_pinch_finger(id) {
                    return consumed(CursorIcon::Grabbing);
                }
                match self.input.gesture {
                    Gesture::Press { id: d, .. }
                    | Gesture::Pan { id: d, .. }
                    | Gesture::Scale { id: d, .. }
                        if d == id =>
                    {
                        self.input.gesture = Gesture::Idle;
                        consumed(CursorIcon::Default)
                    }
                    _ => InputResult::default(),
                }
            }
            InputEvent::PointerLeave => {
                if !matches!(self.input.gesture, Gesture::Idle) {
                    self.input.gesture = Gesture::Idle;
                }
                self.set_pointer(None);
                InputResult::default()
            }
            InputEvent::Wheel { pos, delta, mods } => self.wheel(pos.to_px(dpr), delta, mods),
            InputEvent::Pinch {
                phase,
                center,
                scale,
            } => {
                let p = center.to_px(dpr);
                // The same gesture as the two fingers tracked here.
                if matches!(self.input.gesture, Gesture::Pinch { .. }) {
                    return consumed(CursorIcon::Grabbing);
                }
                if self.region(p) != Region::Plot {
                    return InputResult::default();
                }
                if phase == GesturePhase::Start {
                    self.input.kinetic = None;
                }
                if self.x.zoom_px(f64::from(p.x), f64::from(scale)) {
                    self.view_changed();
                }
                consumed(CursorIcon::Default)
            }
            InputEvent::DoubleClick {
                pos,
                button: Button::Left,
            } if self.region(pos.to_px(dpr)) == Region::Plot => {
                self.fit();
                consumed(self.hover_cursor())
            }
            _ => InputResult::default(),
        }
    }

    /// Movement (physical px) before a press of the last pointer kind
    /// becomes a drag.
    fn drag_threshold(&self) -> f32 {
        let css = if self.input.touch {
            TOUCH_DRAG_THRESHOLD
        } else {
            DRAG_THRESHOLD
        };
        css * self.dpr
    }

    /// A finger pressed while another one presses or pans: a pinch (a
    /// third finger is ignored). `None`: not a second finger.
    fn second_finger(&mut self, id: u64, p: PointPx) -> Option<InputResult> {
        let first = match self.input.gesture {
            Gesture::Press { id: d, start } if d != id => (d, start),
            Gesture::Pan { id: d, last } if d != id => (d, last),
            Gesture::Pinch { .. } => return Some(consumed(CursorIcon::Grabbing)),
            _ => return None,
        };
        self.input.gesture = Gesture::Pinch {
            a: first,
            b: (id, p),
        };
        self.input.kinetic = None;
        self.set_pointer(None);
        Some(consumed(CursorIcon::Grabbing))
    }

    /// A finger of a pinch moved: each direction zooms by the change of the
    /// fingers' distance along it (when they are far enough apart) around
    /// their midpoint, then everything pans with the midpoint.
    fn pinch_move(&mut self, id: u64, p: PointPx) -> InputResult {
        let Gesture::Pinch { mut a, mut b } = self.input.gesture else {
            return InputResult::default();
        };
        let (oa, ob) = (a.1, b.1);
        if a.0 == id {
            a.1 = p;
        } else if b.0 == id {
            b.1 = p;
        } else {
            return InputResult::default();
        }
        let min = PINCH_MIN_SPAN_CSS * self.dpr;
        let ratio = |d0: f32, d1: f32| {
            (d0.abs() >= min && d1.abs() >= min).then(|| f64::from(d1.abs() / d0.abs()))
        };
        let (mx0, my0) = (f64::from(oa.x + ob.x) * 0.5, f64::from(oa.y + ob.y) * 0.5);
        let (mx1, my1) = (
            f64::from(a.1.x + b.1.x) * 0.5,
            f64::from(a.1.y + b.1.y) * 0.5,
        );
        let mut changed = false;
        if let Some(f) = ratio(oa.x - ob.x, a.1.x - b.1.x) {
            changed |= self.x.zoom_px(mx0, f);
        }
        if let Some(f) = ratio(oa.y - ob.y, a.1.y - b.1.y) {
            changed |= self.left.zoom_px(my0, f);
            changed |= self.right.zoom_px(my0, f);
        }
        changed |= self.pan_by(mx1 - mx0, my1 - my0);
        if changed {
            self.view_changed();
        }
        self.input.gesture = Gesture::Pinch { a, b };
        consumed(CursorIcon::Grabbing)
    }

    /// Finger `id` of a pinch lifted: the other one pans on from where it
    /// is. Whether `id` was part of the pinch.
    fn end_pinch_finger(&mut self, id: u64) -> bool {
        let Gesture::Pinch { a, b } = self.input.gesture else {
            return false;
        };
        let (rest_id, rest) = if a.0 == id {
            b
        } else if b.0 == id {
            a
        } else {
            return false;
        };
        self.input.gesture = Gesture::Pan {
            id: rest_id,
            last: rest,
        };
        self.input.tx.clear();
        self.input.ty.clear();
        true
    }

    /// The element under `pos` in the last layout.
    pub fn hit_test(&self, pos: PointCss) -> Option<Hit> {
        self.hit_shape(pos.to_px(self.dpr)).map(|h| h.0)
    }

    /// Whether a touch gesture the page must not take over is running (a
    /// two-finger pinch): a web host keeps the page from scrolling meanwhile
    /// (`touchScroll`). A finger's pan is not one (see
    /// `Chart::captures_touch`).
    pub fn captures_touch(&self) -> bool {
        matches!(self.input.gesture, Gesture::Pinch { .. })
    }

    /// Whether kinetic panning is running (keep requesting frames).
    pub fn is_animating(&self) -> bool {
        self.input.kinetic.is_some()
    }

    pub(crate) fn hit_shape(&self, p: PointPx) -> Option<(Hit, Shape)> {
        hit_at(&geometry!(self), &self.marks, p)
    }

    fn region(&self, p: PointPx) -> Region {
        let l = self.layout;
        if l.plot.contains(p) {
            Region::Plot
        } else if self.x.opts.visible && l.x_axis.contains(p) {
            Region::Axis(AxisId::X)
        } else if self.left.opts.visible && l.left_axis.contains(p) {
            Region::Axis(AxisId::Left)
        } else if self.right.opts.visible && l.right_axis.contains(p) {
            Region::Axis(AxisId::Right)
        } else {
            Region::Outside
        }
    }

    /// Whether dragging along axis `id` zooms it.
    fn scalable(&self, id: AxisId) -> bool {
        let a = self.axis_ref(id);
        !a.opts.locked && a.opts.kind != AxisKind::Category
    }

    fn hover_cursor(&self) -> CursorIcon {
        if self.input.hover.is_some() {
            CursorIcon::Pointer
        } else if self.crosshair {
            CursorIcon::Crosshair
        } else {
            CursorIcon::Default
        }
    }

    /// Moves the pointer (or takes it away) and updates the hovered element.
    pub(crate) fn set_pointer(&mut self, p: Option<PointPx>) {
        self.input.pointer = p;
        self.update_hover();
    }

    /// Hit-tests the pointer in the current layout; a change is reported and
    /// redraws the overlay.
    pub(crate) fn update_hover(&mut self) {
        let hover = self.input.pointer.and_then(|p| self.hit_shape(p));
        let key = |h: Option<(Hit, Shape)>| h.map(|h| (h.0.mark, h.0.index));
        if key(hover) != key(self.input.hover) {
            self.push_event(PlotEvent::Hover(hover.map(|h| h.0)));
        }
        if hover != self.input.hover {
            self.input.hover = hover;
            self.invalidation = self.invalidation.max(Invalidation::Cursor);
        }
    }

    fn push_event(&mut self, e: PlotEvent) {
        // Consecutive view changes are one event.
        if e == PlotEvent::ViewChanged && self.events.last() == Some(&PlotEvent::ViewChanged) {
            return;
        }
        self.events.push(e);
    }

    pub(crate) fn view_changed(&mut self) {
        self.push_event(PlotEvent::ViewChanged);
        self.invalidation = self.invalidation.max(Invalidation::Light);
    }

    fn click(&mut self, p: PointPx) {
        let hit = self.hit_shape(p).map(|h| h.0);
        let x = self.x.value_at(f64::from(p.x));
        let y = self.axis_ref(self.grid_axis()).value_at(f64::from(p.y));
        self.push_event(PlotEvent::Click { hit, x, y });
    }

    fn drag_to(&mut self, p: PointPx, time_ms: f64) -> InputResult {
        if let Gesture::Pan { id, last } = self.input.gesture {
            if self.pan_by(f64::from(p.x - last.x), f64::from(p.y - last.y)) {
                self.view_changed();
            }
            self.input.tx.record(time_ms, p.x);
            self.input.ty.record(time_ms, p.y);
            self.input.gesture = Gesture::Pan { id, last: p };
        }
        consumed(CursorIcon::Grabbing)
    }

    fn scale_drag(&mut self, axis: AxisId, last: PointPx, p: PointPx) {
        let dpr = f64::from(self.dpr);
        let a = self.axis_mut(axis);
        let center = a.start + a.len * 0.5;
        // Right zooms x in; down zooms y out.
        let factor = if axis == AxisId::X {
            (f64::from(p.x - last.x) / dpr * AXIS_DRAG_ZOOM).exp()
        } else {
            (-f64::from(p.y - last.y) / dpr * AXIS_DRAG_ZOOM).exp()
        };
        if a.zoom_px(center, factor) {
            self.view_changed();
        }
    }

    fn wheel(&mut self, p: PointPx, delta: WheelDelta, mods: Modifiers) -> InputResult {
        let (dx, dy) = match delta {
            WheelDelta::Lines { x, y } => (x * LINE_PX, y * LINE_PX),
            WheelDelta::Pixels { x, y } => (x.0, y.0),
        };
        let region = self.region(p);
        if region == Region::Outside {
            return InputResult::default();
        }
        self.input.kinetic = None;
        let dpr = self.dpr;
        let zoom = (-f64::from(dy) * WHEEL_ZOOM).exp();
        let nx = self.category_count(AxisId::X);
        let changed = match region {
            Region::Axis(id @ (AxisId::Left | AxisId::Right)) => {
                self.axis_mut(id).zoom_px(f64::from(p.y), zoom)
            }
            Region::Plot if mods.alt => {
                let l = self.left.zoom_px(f64::from(p.y), zoom);
                self.right.zoom_px(f64::from(p.y), zoom) | l
            }
            _ if mods.shift => {
                let d = if dx.abs() > dy.abs() { dx } else { dy };
                self.x.pan_px(f64::from(-d * dpr), nx)
            }
            _ if dx.abs() > dy.abs() => self.x.pan_px(f64::from(-dx * dpr), nx),
            _ => {
                // Category x axes do not zoom: the wheel scrolls them.
                self.x.zoom_px(f64::from(p.x), zoom) || self.x.pan_px(f64::from(-dy * dpr), nx)
            }
        };
        if changed {
            self.view_changed();
        }
        consumed(match region {
            Region::Axis(a) => axis_cursor(a),
            _ => self.hover_cursor(),
        })
    }

    /// Number of categories of axis `id` (labels or data, whichever is more).
    pub(crate) fn category_count(&self, id: AxisId) -> usize {
        let used = self
            .marks
            .iter()
            .filter(|m| m.opts.visible)
            .map(|m| match id {
                AxisId::X => m.cat_x,
                _ if m.opts.y_axis == id => m.cat_y,
                _ => 0,
            })
            .max()
            .unwrap_or(0);
        self.axis_ref(id).category_count(used)
    }

    /// Pans the axes by `dx`, `dy` physical pixels; whether any moved.
    pub(crate) fn pan_by(&mut self, dx: f64, dy: f64) -> bool {
        let nx = self.category_count(AxisId::X);
        let nl = self.category_count(AxisId::Left);
        let nr = self.category_count(AxisId::Right);
        let a = self.x.pan_px(dx, nx);
        let b = self.left.pan_px(dy, nl);
        let c = self.right.pan_px(dy, nr);
        a | b | c
    }

    /// Steps kinetic panning to `now_ms`.
    pub(crate) fn step_kinetic(&mut self, now_ms: f64) {
        let Some(k) = self.input.kinetic else {
            return;
        };
        let dt = now_ms - k.last_ms;
        if dt.is_nan() || dt <= 0.0 {
            return;
        }
        let decay = (-dt / KINETIC_TAU_MS).exp();
        let travel = KINETIC_TAU_MS * (1.0 - decay);
        let moved = self.pan_by(k.vx * travel, k.vy * travel);
        let (vx, vy) = (k.vx * decay, k.vy * decay);
        self.input.kinetic = (moved && vx.hypot(vy) >= KINETIC_MIN_SPEED).then_some(Kinetic {
            vx,
            vy,
            last_ms: now_ms,
        });
        if moved {
            self.view_changed();
        }
    }
}

fn axis_cursor(a: AxisId) -> CursorIcon {
    if a == AxisId::X {
        CursorIcon::EwResize
    } else {
        CursorIcon::NsResize
    }
}
