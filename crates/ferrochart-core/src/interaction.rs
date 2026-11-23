//! Platform-independent input events and the interaction state machine:
//! panning with kinetic scrolling, wheel and pinch zoom, axis dragging,
//! keyboard navigation and the crosshair.
//!
//! Bindings (winit, DOM, native) translate their events into [`InputEvent`]s
//! in CSS pixels; the chart multiplies by the device pixel ratio. Input never
//! allocates: the state machine keeps a fixed-size velocity history.
//!
//! **Clock**: kinetic scrolling compares `time_ms` of pointer events with the
//! `now_ms` passed to `Chart::build_frame`, so both must come from the same
//! clock (e.g. `performance.now()` or `Instant` since start).
//!
//! With several panes, price-axis drags, wheel zoom over the axis and vertical
//! panning act on the pane under the pointer; dragging a separator resizes
//! the two panes next to it.
//!
//! Drawings: with a tool armed, clicks create a drawing
//! (click per anchor, or press-drag-release for the first two); otherwise a
//! press on a drawing selects it and drags it (or the handle under the
//! pointer) unless it is locked. Escape cancels, Delete removes the
//! selection, Ctrl/⌘+Z undoes, Ctrl/⌘+Shift+Z and Ctrl/⌘+Y redo.
//!

use smallvec::SmallVec;

use crate::chart::{Chart, HANDLE_CSS};
use crate::data::{DrawingId, PaneId};
use crate::drawing::{
    Anchor, AnchorCount, FREEHAND_STEP_CSS, HitPart, index_of_time, time_of_index,
};
use crate::frame::Invalidation;
use crate::geom::{Css, PointCss, PointPx, Px};
use crate::pane::{MIN_PANE_CSS, SEPARATOR_HIT_CSS, SEPARATOR_PX, split_stretch};

/// Mouse button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Button {
    /// Primary button (also touch and pen contact).
    #[default]
    Left,
    /// Middle button.
    Middle,
    /// Secondary button.
    Right,
}

/// Keyboard modifiers. Plain flags mirror the DOM/winit events they come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[allow(clippy::struct_excessive_bools)]
pub struct Modifiers {
    /// Shift.
    pub shift: bool,
    /// Control.
    pub ctrl: bool,
    /// Alt / Option.
    pub alt: bool,
    /// Meta / Command / Windows.
    pub meta: bool,
}

/// Scroll amount of a wheel event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelDelta {
    /// Lines (mouse wheel notches). Positive `y` scrolls down.
    Lines {
        /// Horizontal lines.
        x: f32,
        /// Vertical lines.
        y: f32,
    },
    /// Pixels (touchpads, precise wheels). Positive `y` scrolls down.
    Pixels {
        /// Horizontal distance.
        x: Css,
        /// Vertical distance.
        y: Css,
    },
}

/// Phase of a continuous gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GesturePhase {
    /// The gesture begins.
    Start,
    /// The gesture continues.
    Update,
    /// The gesture ends.
    End,
}

/// Keys the chart reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// Left arrow: pan towards earlier bars.
    ArrowLeft,
    /// Right arrow: pan towards later bars.
    ArrowRight,
    /// Up arrow: zoom in.
    ArrowUp,
    /// Down arrow: zoom out.
    ArrowDown,
    /// Home: scroll to the first bar.
    Home,
    /// End: scroll to the latest bar.
    End,
    /// Escape: cancel the current drag or the drawing being created.
    Escape,
    /// Delete: delete the selected drawing.
    Delete,
    /// Enter: finish a drawing with an open number of anchors (a path).
    Enter,
    /// Any other key, by character.
    Char(char),
}

/// A platform-independent input event. Positions are in CSS pixels relative to
/// the chart's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum InputEvent {
    /// A button was pressed or a touch began.
    PointerDown {
        /// Pointer id (mouse = 0).
        id: u64,
        /// Position.
        pos: PointCss,
        /// Button.
        button: Button,
        /// Modifiers.
        mods: Modifiers,
        /// Event time, same clock as `build_frame`'s `now_ms`.
        time_ms: f64,
    },
    /// The pointer moved.
    PointerMove {
        /// Pointer id.
        id: u64,
        /// Position.
        pos: PointCss,
        /// Modifiers.
        mods: Modifiers,
        /// Event time.
        time_ms: f64,
    },
    /// A button was released or a touch ended.
    PointerUp {
        /// Pointer id.
        id: u64,
        /// Position.
        pos: PointCss,
        /// Button.
        button: Button,
        /// Modifiers.
        mods: Modifiers,
        /// Event time.
        time_ms: f64,
    },
    /// The pointer left the chart.
    PointerLeave,
    /// Wheel or touchpad scroll.
    Wheel {
        /// Pointer position.
        pos: PointCss,
        /// Scroll amount.
        delta: WheelDelta,
        /// Modifiers.
        mods: Modifiers,
    },
    /// Touchpad or touch-screen pinch. `scale` is relative to the previous event.
    Pinch {
        /// Phase.
        phase: GesturePhase,
        /// Center of the gesture.
        center: PointCss,
        /// Scale factor since the previous event (> 1 zooms in).
        scale: f32,
    },
    /// A key was pressed or released.
    Key {
        /// The key.
        key: Key,
        /// Pressed (`true`) or released.
        pressed: bool,
        /// Modifiers.
        mods: Modifiers,
    },
    /// Double click / double tap.
    DoubleClick {
        /// Position.
        pos: PointCss,
        /// Button.
        button: Button,
    },
}

/// Mouse cursor the host should show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CursorIcon {
    /// Arrow.
    #[default]
    Default,
    /// Crosshair (over the plot).
    Crosshair,
    /// Open hand.
    Grab,
    /// Closed hand (panning).
    Grabbing,
    /// Horizontal resize (time axis).
    EwResize,
    /// Vertical resize (price axis).
    NsResize,
    /// Pointing hand (over a drawing).
    Pointer,
    /// Move (over a drawing's handle, or dragging a drawing).
    Move,
}

/// Result of [`Chart::handle_input`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputResult {
    /// The chart used the event; the host should not handle it further (e.g.
    /// prevent page scrolling on the web).
    pub consumed: bool,
    /// Cursor to show.
    pub cursor: CursorIcon,
}

/// Crosshair behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CrosshairMode {
    /// Follows the pointer; x snaps to bar centers.
    #[default]
    Normal,
    /// y snaps to the nearest of the bar's open, high, low and close.
    Magnet,
    /// No crosshair.
    Hidden,
}

/// Pointer movement (CSS px) before a press becomes a pan.
const DRAG_THRESHOLD: f32 = 3.0;
/// Kinetic velocity decay time constant: after a release at speed `v`, the
/// speed is `v · exp(-t / KINETIC_TAU_MS)`.
pub const KINETIC_TAU_MS: f64 = 325.0;
/// Kinetic scrolling starts above / stops below this speed (physical px per ms).
pub const KINETIC_MIN_SPEED: f64 = 0.05;
/// Velocity is measured over the pointer moves of the last ... ms.
const VELOCITY_WINDOW_MS: f64 = 100.0;
/// A pan released after a pause longer than this has no kinetic phase.
const RELEASE_PAUSE_MS: f64 = 50.0;
/// CSS px per wheel line.
const LINE_PX: f32 = 40.0;
/// Zoom factor per CSS px of wheel scroll.
const WHEEL_ZOOM: f64 = 0.002;
/// Zoom factor per CSS px of axis drag.
const AXIS_DRAG_ZOOM: f64 = 0.005;

/// Where a pointer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Region {
    Plot(PaneId),
    PriceAxis(PaneId),
    /// The separator above pane `k` (an index, `k >= 1`).
    Separator(usize),
    TimeAxis,
    Outside,
}

#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    t: f64,
    x: f32,
}

/// Release velocity of a drag along one axis, from a fixed number of recent
/// pointer samples (no allocation). Used by the chart's kinetic panning and
/// by `ferrochart-plot`.
///
/// ```
/// use ferrochart_core::interaction::VelocityTracker;
///
/// let mut v = VelocityTracker::default();
/// for i in 0..10 {
///     v.record(f64::from(i) * 10.0, i as f32 * 5.0); // 5 px every 10 ms
/// }
/// assert!((v.release_velocity(95.0) - 0.5).abs() < 1e-9);
/// assert_eq!(v.release_velocity(200.0), 0.0); // paused before release
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct VelocityTracker {
    samples: [Sample; 8],
    n_samples: usize,
}

impl VelocityTracker {
    /// Forgets all samples (a new drag).
    pub fn clear(&mut self) {
        self.n_samples = 0;
    }

    /// Records the pointer position `x` (px) at time `t` (ms).
    pub fn record(&mut self, t: f64, x: f32) {
        self.samples.copy_within(1.., 0);
        let last = self.samples.len() - 1;
        self.samples[last] = Sample { t, x };
        self.n_samples = (self.n_samples + 1).min(self.samples.len());
    }

    /// Release velocity in px/ms from the samples of the last 100 ms; 0 if
    /// the pointer paused for more than 50 ms before `now`.
    pub fn release_velocity(&self, now: f64) -> f64 {
        let recent = &self.samples[self.samples.len() - self.n_samples..];
        let Some(last) = recent.last() else {
            return 0.0;
        };
        if now - last.t > RELEASE_PAUSE_MS {
            return 0.0;
        }
        let Some(first) = recent.iter().find(|s| last.t - s.t <= VELOCITY_WINDOW_MS) else {
            return 0.0;
        };
        let dt = last.t - first.t;
        if dt <= 0.0 {
            return 0.0;
        }
        f64::from(last.x - first.x) / dt
    }
}

/// Interaction state.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum Drag {
    #[default]
    Idle,
    PressPending {
        id: u64,
        start: PointPx,
        pane: PaneId,
    },
    Panning {
        id: u64,
        last: PointPx,
        pane: PaneId,
    },
    ScalingPrice {
        id: u64,
        last_y: f32,
        pane: PaneId,
    },
    ScalingTime {
        id: u64,
        last_x: f32,
    },
    /// Dragging the separator between `upper` and `lower`, which together span
    /// `top..bottom` (separator included) when the drag started.
    ResizingPanes {
        id: u64,
        upper: PaneId,
        lower: PaneId,
        top: f32,
        bottom: f32,
    },
    /// Dragging a drawing or one of its handles from `start`.
    EditingDrawing {
        id: u64,
        drawing: DrawingId,
        part: HitPart,
        start: PointPx,
        pane: PaneId,
    },
}

impl Drag {
    /// The pane the drag acts on, if any.
    pub(crate) fn pane(self) -> Option<PaneId> {
        match self {
            Self::PressPending { pane, .. }
            | Self::Panning { pane, .. }
            | Self::ScalingPrice { pane, .. }
            | Self::EditingDrawing { pane, .. } => Some(pane),
            Self::ResizingPanes { upper, .. } => Some(upper),
            Self::Idle | Self::ScalingTime { .. } => None,
        }
    }

    /// Whether the drag involves `pane`.
    pub(crate) fn involves(self, pane: PaneId) -> bool {
        match self {
            Self::ResizingPanes { upper, lower, .. } => upper == pane || lower == pane,
            _ => self.pane() == Some(pane),
        }
    }
}

/// State machine data stored in the chart.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Interaction {
    pub(crate) drag: Drag,
    /// Kinetic scrolling velocity (physical px per ms) and the time of the last
    /// step (`None` until the first frame after release).
    pub(crate) kinetic: Option<(f64, Option<f64>)>,
    velocity: VelocityTracker,
    /// Pointer position in physical px, `None` when outside.
    pub(crate) pointer: Option<PointPx>,
    pub(crate) crosshair: CrosshairMode,
}

impl Default for Interaction {
    fn default() -> Self {
        Self {
            drag: Drag::Idle,
            kinetic: None,
            velocity: VelocityTracker::default(),
            pointer: None,
            crosshair: CrosshairMode::Normal,
        }
    }
}

impl Chart {
    pub(crate) fn region(&self, p: PointPx) -> Region {
        let l = self.layout;
        let in_width = p.x >= 0.0 && p.x < l.time_axis.right();
        if in_width && p.y < l.time_axis.y {
            let hit = SEPARATOR_HIT_CSS * self.dpr;
            for k in 1..self.panes.len() {
                let center = self.panes[k].plot.y - SEPARATOR_PX * 0.5;
                if (p.y - center).abs() <= hit {
                    return Region::Separator(k);
                }
            }
        }
        if let Some(i) = self.pane_at(p) {
            return Region::Plot(self.panes[i].id);
        }
        if l.price_axis.contains(p) {
            return self
                .panes
                .iter()
                .find(|pane| p.y >= pane.plot.y && p.y < pane.plot.bottom())
                .map_or(Region::Outside, |pane| Region::PriceAxis(pane.id));
        }
        if l.time_axis.contains(p) {
            Region::TimeAxis
        } else {
            Region::Outside
        }
    }

    fn hover_cursor(&self, p: PointPx) -> CursorIcon {
        match self.region(p) {
            Region::Plot(pane) => {
                if self.drawings.armed.is_some() || self.drawings.creating.is_some() {
                    return CursorIcon::Crosshair;
                }
                match self.hit_drawing(pane, p) {
                    Some((_, HitPart::Handle(_))) => CursorIcon::Move,
                    Some((_, HitPart::Body)) => CursorIcon::Pointer,
                    None => CursorIcon::Crosshair,
                }
            }
            Region::PriceAxis(_) | Region::Separator(_) => CursorIcon::NsResize,
            Region::TimeAxis => CursorIcon::EwResize,
            Region::Outside => CursorIcon::Default,
        }
    }

    fn set_pointer(&mut self, p: Option<PointPx>) {
        if self.input.pointer != p {
            self.input.pointer = p;
            // A frame decides the hovered marker (`MarkerHover`), so one is
            // needed when markers were drawn, even without a crosshair.
            if self.input.crosshair != CrosshairMode::Hidden || self.tracks_marker_hover() {
                self.invalidation = self.invalidation.max(Invalidation::Cursor);
            }
        }
    }

    /// Plot-relative x of a physical point.
    fn plot_x(&self, p: PointPx) -> f32 {
        p.x - self.layout.plot.x
    }

    fn zoom_time(&mut self, anchor_x: f32, factor: f64) {
        self.time_scale.zoom(Px(anchor_x), factor);
    }

    fn zoom_price_at(&mut self, pane: PaneId, y: f32, factor: f64) {
        let Some(pane) = self.pane_mut(pane) else {
            return;
        };
        let plot = pane.plot;
        let center =
            pane.price_scale
                .y_to_price(f64::from(y), f64::from(plot.y), f64::from(plot.h));
        pane.price_scale.zoom_around(factor, center);
    }

    /// Moves the separator of a `ResizingPanes` drag to `y`.
    fn resize_panes(&mut self, y: f32) {
        let Drag::ResizingPanes {
            upper,
            lower,
            top,
            bottom,
            ..
        } = self.input.drag
        else {
            return;
        };
        let min = MIN_PANE_CSS * self.dpr;
        let (lo, hi) = (top + min, bottom - min - SEPARATOR_PX);
        if lo > hi {
            return;
        }
        let y = y.round().clamp(lo, hi);
        let (Some(su), Some(sl)) = (self.pane_stretch(upper), self.pane_stretch(lower)) else {
            return;
        };
        if let Some((a, b)) = split_stretch(su + sl, y - top, bottom - y - SEPARATOR_PX) {
            for (id, s) in [(upper, a), (lower, b)] {
                if let Some(p) = self.pane_mut(id) {
                    p.stretch = s;
                }
            }
            self.invalidation = Invalidation::Full;
        }
    }

    /// The anchor under `p` on `pane`: x snaps to the nearest bar (times
    /// past the data are extrapolated), y snaps to the bar's OHLC in magnet
    /// mode. `None` without data.
    pub(crate) fn anchor_at(&self, pane: PaneId, p: PointPx) -> Option<Anchor> {
        let i = self.pane_index(pane)?;
        let ctx = self.drawing_ctx(i, &self.text_cache)?;
        let plot = ctx.plot;
        let idx = self.time_scale.x_to_index(Px(p.x - plot.x)).round();
        let time = time_of_index(&self.time_index, idx)?;
        let mut y = p.y;
        if self.input.crosshair == CrosshairMode::Magnet
            && let Some(bar) = self.magnet_bar(pane, idx as i64)
        {
            let mut best = f32::INFINITY;
            for v in bar {
                if let Some(vy) = ctx.y_of(v)
                    && (vy - p.y).abs() < (best - p.y).abs()
                {
                    best = vy;
                }
            }
            if best.is_finite() {
                y = best;
            }
        }
        let price = ctx.price_at(y);
        price.is_finite().then_some(Anchor { time, price })
    }

    /// The anchor exactly at `p` (no snapping to bars or the magnet): for
    /// freehand drawings.
    pub(crate) fn free_anchor_at(&self, pane: PaneId, p: PointPx) -> Option<Anchor> {
        let i = self.pane_index(pane)?;
        let ctx = self.drawing_ctx(i, &self.text_cache)?;
        let time = ctx.time_at(p.x)?;
        let price = ctx.price_at(p.y);
        price.is_finite().then_some(Anchor { time, price })
    }

    /// What `p` hits among the drawings of `pane`: the selected drawing's
    /// handles first, then the selected drawing, then the others from the
    /// top.
    pub(crate) fn hit_drawing(&self, pane: PaneId, p: PointPx) -> Option<(DrawingId, HitPart)> {
        let i = self.pane_index(pane)?;
        let ctx = self.drawing_ctx(i, &self.text_cache)?;
        let tol = ctx.hit_tolerance();
        let entries = &self.drawings.entries;
        let selected = self
            .drawings
            .selected()
            .and_then(|s| self.drawings.entry(s))
            .filter(|e| e.pane == pane && e.visible);
        if let Some(e) = selected {
            let r = HANDLE_CSS * 0.5 * self.dpr + tol * 0.5;
            let hit = self
                .handle_buf
                .try_borrow_mut()
                .ok()
                .and_then(|mut handles| {
                    handles.clear();
                    e.tool.handles(&ctx, &mut handles);
                    handles
                        .iter()
                        .position(|h| (p.x - h.x).hypot(p.y - h.y) <= r)
                });
            if let Some(k) = hit {
                return Some((e.id, HitPart::Handle(k)));
            }
            if let Some(part) = e.tool.hit_test(&ctx, p) {
                return Some((e.id, part));
            }
        }
        entries.iter().rev().find_map(|e| {
            let b = e.bbox;
            let near = b.w + b.h > 0.0
                && p.x >= b.x - tol
                && p.x <= b.right() + tol
                && p.y >= b.y - tol
                && p.y <= b.bottom() + tol;
            if e.pane != pane || !e.visible || !near {
                return None;
            }
            Some((e.id, e.tool.hit_test(&ctx, p)?))
        })
    }

    /// A press on the plot of `pane` when drawings are involved: creating
    /// one, or selecting and starting to drag one. `None` lets the press pan.
    fn drawing_press(&mut self, id: u64, pane: PaneId, p: PointPx) -> Option<InputResult> {
        let take = |cursor| {
            Some(InputResult {
                consumed: true,
                cursor,
            })
        };
        if let Some(c) = self.drawings.creating {
            if let Some(a) = self.anchor_at(c.pane, p) {
                self.drawings.creation_click(a, Some(p));
            }
            return take(CursorIcon::Crosshair);
        }
        if self.drawings.armed.is_some() {
            if let Some(a) = self.anchor_at(pane, p) {
                self.drawings.begin_creation(pane, a, p);
            }
            // A freehand drawing starts exactly under the pointer.
            if self.drawings.creating_count() == Some(AnchorCount::Freehand)
                && let Some(a) = self.free_anchor_at(pane, p)
                && let Some(c) = self.drawings.creating
                && let Some(e) = self.drawings.entry_mut(c.id)
                && let Some(first) = e.tool.anchors_mut().first_mut()
            {
                *first = a;
            }
            return take(CursorIcon::Crosshair);
        }
        let Some((drawing, part)) = self.hit_drawing(pane, p) else {
            let _ = self.drawings.select(None);
            return None;
        };
        let _ = self.drawings.select(Some(drawing));
        let e = self.drawings.entry(drawing)?;
        if e.locked {
            return None;
        }
        let before = e.tool.anchors().to_vec();
        self.drawings.edit_before = before;
        self.input.drag = Drag::EditingDrawing {
            id,
            drawing,
            part,
            start: p,
            pane,
        };
        take(CursorIcon::Move)
    }

    /// Moves the dragged drawing (or its handle) to follow the pointer: the
    /// body moves by whole bars and by the pointer's y in display space.
    fn drag_drawing(&mut self, p: PointPx) {
        let Drag::EditingDrawing {
            drawing,
            part,
            start,
            pane,
            ..
        } = self.input.drag
        else {
            return;
        };
        match part {
            HitPart::Handle(k) => {
                let Some(a) = self.anchor_at(pane, p) else {
                    return;
                };
                if let Some(e) = self.drawings.entry_mut(drawing) {
                    e.tool.move_handle(k, a);
                }
            }
            HitPart::Body => {
                let Some(i) = self.pane_index(pane) else {
                    return;
                };
                let Some(ctx) = self.drawing_ctx(i, &self.text_cache) else {
                    return;
                };
                let spacing = self.time_scale.bar_spacing();
                let di = if spacing > 0.0 {
                    (f64::from(p.x - start.x) / spacing).round()
                } else {
                    0.0
                };
                let dy = p.y - start.y;
                let moved: SmallVec<[Anchor; 4]> = self
                    .drawings
                    .edit_before
                    .iter()
                    .map(|a| {
                        let time = if di == 0.0 {
                            Some(a.time)
                        } else {
                            index_of_time(&self.time_index, a.time)
                                .and_then(|j| time_of_index(&self.time_index, j + di))
                        };
                        let price = ctx.y_of(a.price).map(|y| ctx.price_at(y + dy));
                        match (time, price) {
                            (Some(time), Some(price)) if price.is_finite() => {
                                Anchor { time, price }
                            }
                            _ => *a,
                        }
                    })
                    .collect();
                if let Some(e) = self.drawings.entry_mut(drawing) {
                    let anchors = e.tool.anchors_mut();
                    anchors.clear();
                    anchors.extend_from_slice(&moved);
                }
            }
        }
        self.drawings.dirty = true;
    }

    /// The current crosshair mode.
    pub fn crosshair_mode(&self) -> CrosshairMode {
        self.input.crosshair
    }

    /// Sets the crosshair mode.
    pub fn set_crosshair_mode(&mut self, mode: CrosshairMode) {
        self.input.crosshair = mode;
        self.invalidation = self.invalidation.max(Invalidation::Cursor);
    }

    /// Whether kinetic scrolling is running (the host keeps requesting frames).
    pub fn is_animating(&self) -> bool {
        self.input.kinetic.is_some()
    }

    /// Advances kinetic scrolling to `now_ms` (called by `build_frame`).
    pub(crate) fn step_kinetic(&mut self, now_ms: f64) {
        let Some((v, last)) = self.input.kinetic else {
            return;
        };
        let Some(last) = last else {
            self.input.kinetic = Some((v, Some(now_ms)));
            self.time_scale.changed = true;
            return;
        };
        let dt = now_ms - last;
        if !(dt.is_finite() && dt > 0.0) {
            self.input.kinetic = Some((v, Some(now_ms)));
            return;
        }
        let decay = (-dt / KINETIC_TAU_MS).exp();
        let distance = v * KINETIC_TAU_MS * (1.0 - decay);
        let before = self.time_scale.right_offset();
        self.time_scale.scroll_by(Px(distance as f32));
        let v = v * decay;
        let stuck = (self.time_scale.right_offset() - before).abs() < 1e-9;
        self.input.kinetic = (v.abs() >= KINETIC_MIN_SPEED && !stuck).then_some((v, Some(now_ms)));
    }

    /// Handles an input event. Coordinates are CSS pixels; regions refer
    /// to the layout of the last built frame.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::chart::{SeriesData, SeriesOptions};
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::geom::{PointCss, SizeCss};
    /// use ferrochart_core::interaction::{Button, InputEvent, Modifiers};
    /// use ferrochart_core::testing::{random_walk, MonoMeasurer};
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// chart.resize(SizeCss { w: 800.0, h: 400.0 }, 1.0);
    /// let id = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
    /// chart.set_data(id, SeriesData::Ohlc(random_walk(1, 1000, 0, 60_000)?))?;
    /// chart.build_frame(0.0, &MonoMeasurer);
    ///
    /// let before = chart.time_scale().visible_range();
    /// let mods = Modifiers::default();
    /// let at = |x| PointCss { x, y: 200.0 };
    /// chart.handle_input(InputEvent::PointerDown { id: 0, pos: at(400.0), button: Button::Left, mods, time_ms: 0.0 });
    /// chart.handle_input(InputEvent::PointerMove { id: 0, pos: at(460.0), mods, time_ms: 16.0 });
    /// chart.handle_input(InputEvent::PointerUp { id: 0, pos: at(460.0), button: Button::Left, mods, time_ms: 500.0 });
    /// // Dragging right by 60 px shows bars 10 bars earlier (6 px spacing).
    /// let after = chart.time_scale().visible_range();
    /// assert!((before.0 - after.0 - 10.0).abs() < 1e-9);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn handle_input(&mut self, event: InputEvent) -> InputResult {
        let dpr = self.dpr;
        let consumed = |cursor| InputResult {
            consumed: true,
            cursor,
        };
        match event {
            InputEvent::PointerDown {
                id,
                pos,
                button,
                time_ms,
                ..
            } => {
                let p = pos.to_px(dpr);
                self.input.kinetic = None;
                if button != Button::Left || !matches!(self.input.drag, Drag::Idle) {
                    return InputResult {
                        consumed: false,
                        cursor: self.hover_cursor(p),
                    };
                }
                match self.region(p) {
                    Region::Plot(pane) => {
                        if let Some(result) = self.drawing_press(id, pane, p) {
                            self.set_pointer(Some(p));
                            return result;
                        }
                        self.input.drag = Drag::PressPending { id, start: p, pane };
                        self.input.velocity.clear();
                        self.input.velocity.record(time_ms, p.x);
                        self.set_pointer(Some(p));
                        consumed(CursorIcon::Crosshair)
                    }
                    Region::PriceAxis(pane) => {
                        self.input.drag = Drag::ScalingPrice {
                            id,
                            last_y: p.y,
                            pane,
                        };
                        consumed(CursorIcon::NsResize)
                    }
                    Region::Separator(k) => {
                        let (Some(up), Some(down)) = (self.panes.get(k - 1), self.panes.get(k))
                        else {
                            return InputResult::default();
                        };
                        self.input.drag = Drag::ResizingPanes {
                            id,
                            upper: up.id,
                            lower: down.id,
                            top: up.plot.y,
                            bottom: down.plot.bottom(),
                        };
                        consumed(CursorIcon::NsResize)
                    }
                    Region::TimeAxis => {
                        self.input.drag = Drag::ScalingTime { id, last_x: p.x };
                        consumed(CursorIcon::EwResize)
                    }
                    Region::Outside => InputResult::default(),
                }
            }
            InputEvent::PointerMove {
                id, pos, time_ms, ..
            } => {
                let p = pos.to_px(dpr);
                let in_plot = matches!(self.region(p), Region::Plot(_));
                match self.input.drag {
                    Drag::Idle if self.drawings.creating.is_some() => {
                        if let Some(c) = self.drawings.creating {
                            if c.count == AnchorCount::Freehand {
                                if let Some(a) = self.free_anchor_at(c.pane, p) {
                                    let step = FREEHAND_STEP_CSS * dpr;
                                    self.drawings.freehand_move(a, p, step);
                                }
                            } else if let Some(a) = self.anchor_at(c.pane, p) {
                                self.drawings.creation_move(a);
                            }
                        }
                        self.set_pointer(in_plot.then_some(p));
                        consumed(CursorIcon::Crosshair)
                    }
                    Drag::EditingDrawing { id: d, .. } if d == id => {
                        self.drag_drawing(p);
                        self.set_pointer(in_plot.then_some(p));
                        consumed(CursorIcon::Move)
                    }
                    Drag::Idle => {
                        self.set_pointer(in_plot.then_some(p));
                        InputResult {
                            consumed: false,
                            cursor: self.hover_cursor(p),
                        }
                    }
                    Drag::PressPending {
                        id: d, start, pane, ..
                    } if d == id => {
                        let moved = (p.x - start.x).hypot(p.y - start.y);
                        if moved > DRAG_THRESHOLD * dpr {
                            self.input.drag = Drag::Panning {
                                id,
                                last: start,
                                pane,
                            };
                            return self.pan_to(p, time_ms);
                        }
                        self.set_pointer(in_plot.then_some(p));
                        consumed(CursorIcon::Crosshair)
                    }
                    Drag::Panning { id: d, .. } if d == id => self.pan_to(p, time_ms),
                    Drag::ScalingPrice {
                        id: d,
                        last_y,
                        pane,
                    } if d == id => {
                        let dy = f64::from((p.y - last_y) / dpr);
                        if let Some(scale) = self.pane_mut(pane).map(|p| &mut p.price_scale) {
                            let center = scale.center_price();
                            scale.zoom_around((dy * AXIS_DRAG_ZOOM).exp(), center);
                        }
                        self.input.drag = Drag::ScalingPrice {
                            id,
                            last_y: p.y,
                            pane,
                        };
                        consumed(CursorIcon::NsResize)
                    }
                    Drag::ResizingPanes { id: d, .. } if d == id => {
                        self.resize_panes(p.y);
                        consumed(CursorIcon::NsResize)
                    }
                    Drag::ScalingTime { id: d, last_x } if d == id => {
                        let dx = f64::from((p.x - last_x) / dpr);
                        let right = self.layout.plot.w;
                        self.zoom_time(right, (dx * AXIS_DRAG_ZOOM).exp());
                        self.input.drag = Drag::ScalingTime { id, last_x: p.x };
                        consumed(CursorIcon::EwResize)
                    }
                    _ => InputResult::default(),
                }
            }
            InputEvent::PointerUp {
                id, pos, time_ms, ..
            } => {
                let p = pos.to_px(dpr);
                let drag = self.input.drag;
                if let Some(c) = self.drawings.creating
                    && matches!(drag, Drag::Idle)
                {
                    // Releasing far from the press is a click (drag creation).
                    let far = c
                        .down
                        .is_some_and(|d| (p.x - d.x).hypot(p.y - d.y) > DRAG_THRESHOLD * dpr);
                    if c.count == AnchorCount::Freehand {
                        if let Some(a) = self.free_anchor_at(c.pane, p) {
                            // The release point, unless the last anchor is close.
                            self.drawings.freehand_move(a, p, 1.0);
                        }
                        self.drawings.freehand_release();
                        return consumed(self.hover_cursor(p));
                    }
                    if let Some(c) = self.drawings.creating.as_mut() {
                        c.down = None;
                    }
                    if far && let Some(a) = self.anchor_at(c.pane, p) {
                        self.drawings.creation_click(a, None);
                    }
                    return consumed(CursorIcon::Crosshair);
                }
                if let Drag::EditingDrawing {
                    id: d,
                    drawing,
                    start,
                    pane,
                    ..
                } = drag
                {
                    if d != id {
                        return InputResult::default();
                    }
                    self.input.drag = Drag::Idle;
                    let before = core::mem::take(&mut self.drawings.edit_before);
                    self.drawings.moved(drawing, before);
                    // A press on a drawing that did not move it is a click.
                    if (p.x - start.x).hypot(p.y - start.y) <= DRAG_THRESHOLD * dpr {
                        self.emit_click(pane, p, Some(drawing));
                    }
                    return consumed(self.hover_cursor(p));
                }
                let (Drag::PressPending { id: d, .. }
                | Drag::Panning { id: d, .. }
                | Drag::ScalingPrice { id: d, .. }
                | Drag::ScalingTime { id: d, .. }
                | Drag::ResizingPanes { id: d, .. }) = drag
                else {
                    return InputResult {
                        consumed: false,
                        cursor: self.hover_cursor(p),
                    };
                };
                if d != id {
                    return InputResult::default();
                }
                self.input.drag = Drag::Idle;
                if let Drag::PressPending { pane, .. } = drag {
                    self.emit_click(pane, p, None);
                }
                if let Drag::Panning { .. } = drag {
                    let v = self.input.velocity.release_velocity(time_ms);
                    if v.abs() >= KINETIC_MIN_SPEED {
                        // Start counting from the release.
                        self.input.kinetic = Some((v, Some(time_ms)));
                        self.time_scale.changed = true;
                    }
                }
                consumed(self.hover_cursor(p))
            }
            InputEvent::PointerLeave => {
                self.set_pointer(None);
                InputResult::default()
            }
            InputEvent::Wheel { pos, delta, mods } => self.wheel(pos.to_px(dpr), delta, mods),
            InputEvent::Pinch {
                phase,
                center,
                scale,
            } => {
                if phase == GesturePhase::Start {
                    self.input.kinetic = None;
                }
                let p = center.to_px(dpr);
                if scale.is_finite() && scale > 0.0 {
                    let x = self.plot_x(p);
                    self.zoom_time(x, f64::from(scale));
                }
                consumed(self.hover_cursor(p))
            }
            InputEvent::Key { key, pressed, mods } => {
                if !pressed {
                    return InputResult::default();
                }
                self.key(key, mods)
            }
            InputEvent::DoubleClick { pos, button } => {
                let p = pos.to_px(dpr);
                if button != Button::Left {
                    return InputResult::default();
                }
                if self.drawings.finish_open() {
                    return consumed(self.hover_cursor(p));
                }
                match self.region(p) {
                    Region::PriceAxis(pane) => {
                        if let Some(pane) = self.pane_mut(pane) {
                            pane.price_scale.set_auto_scale(true);
                        }
                        consumed(CursorIcon::NsResize)
                    }
                    Region::TimeAxis => {
                        let spacing = self.default_bar_spacing.to_px(dpr).0;
                        self.time_scale.set_bar_spacing(f64::from(spacing));
                        consumed(CursorIcon::EwResize)
                    }
                    _ => InputResult::default(),
                }
            }
        }
    }

    fn pan_to(&mut self, p: PointPx, time_ms: f64) -> InputResult {
        let Drag::Panning { id, last, pane } = self.input.drag else {
            return InputResult::default();
        };
        self.time_scale.scroll_by(Px(p.x - last.x));
        if let Some(pane) = self.pane_mut(pane)
            && !pane.price_scale.auto_scale()
        {
            let h = f64::from(pane.plot.h);
            pane.price_scale.pan_px(f64::from(p.y - last.y), h);
        }
        self.input.velocity.record(time_ms, p.x);
        self.input.drag = Drag::Panning { id, last: p, pane };
        let in_plot = matches!(self.region(p), Region::Plot(_));
        self.set_pointer(in_plot.then_some(p));
        InputResult {
            consumed: true,
            cursor: CursorIcon::Grabbing,
        }
    }

    fn wheel(&mut self, p: PointPx, delta: WheelDelta, mods: Modifiers) -> InputResult {
        let (dx, dy) = match delta {
            WheelDelta::Lines { x, y } => (x * LINE_PX, y * LINE_PX),
            WheelDelta::Pixels { x, y } => (x.0, y.0),
        };
        if !(dx.is_finite() && dy.is_finite()) {
            return InputResult::default();
        }
        let region = self.region(p);
        if region == Region::Outside {
            return InputResult::default();
        }
        self.input.kinetic = None;
        let dpr = self.dpr;
        if mods.shift {
            // Shift + wheel pans; vertical wheels pan horizontally.
            let d = if dx.abs() > dy.abs() { dx } else { dy };
            self.time_scale.scroll_by(Px(-d * dpr));
        } else if let (Region::PriceAxis(pane), _) | (Region::Plot(pane), true) = (region, mods.alt)
        {
            self.zoom_price_at(pane, p.y, (f64::from(dy) * WHEEL_ZOOM).exp());
        } else if dx.abs() > dy.abs() {
            // Horizontal touchpad scroll pans.
            self.time_scale.scroll_by(Px(-dx * dpr));
        } else {
            let anchor = if region == Region::TimeAxis {
                self.layout.plot.w
            } else {
                self.plot_x(p)
            };
            self.zoom_time(anchor, (-f64::from(dy) * WHEEL_ZOOM).exp());
        }
        InputResult {
            consumed: true,
            cursor: self.hover_cursor(p),
        }
    }

    fn key(&mut self, key: Key, mods: Modifiers) -> InputResult {
        let w = self.layout.plot.w;
        let command = mods.ctrl || mods.meta;
        let done = |consumed| InputResult {
            consumed,
            cursor: CursorIcon::Default,
        };
        match key {
            Key::Char('z' | 'Z') if command => {
                return done(if mods.shift {
                    self.drawings.redo()
                } else {
                    self.drawings.undo()
                });
            }
            Key::Char('y' | 'Y') if command => return done(self.drawings.redo()),
            Key::Escape if self.drawings.creating.is_some() => {
                self.drawings.escape_creation();
                return done(true);
            }
            Key::Escape if self.drawings.armed.is_some() => {
                self.drawings.cancel_tool();
                return done(true);
            }
            Key::Enter => return done(self.drawings.finish_open()),
            Key::Escape if matches!(self.input.drag, Drag::EditingDrawing { .. }) => {
                let before = core::mem::take(&mut self.drawings.edit_before);
                if let Drag::EditingDrawing { drawing, .. } = self.input.drag
                    && let Some(e) = self.drawings.entry_mut(drawing)
                {
                    let anchors = e.tool.anchors_mut();
                    anchors.clear();
                    anchors.extend_from_slice(&before);
                }
                self.drawings.dirty = true;
                self.input.drag = Drag::Idle;
                return done(true);
            }
            Key::Delete => {
                let Some(sel) = self.drawings.selected() else {
                    return done(false);
                };
                if matches!(self.input.drag, Drag::EditingDrawing { .. }) {
                    self.input.drag = Drag::Idle;
                }
                return done(self.drawings.remove(sel).is_ok());
            }
            Key::ArrowLeft => self.time_scale.scroll_by(Px(w * 0.1)),
            Key::ArrowRight => self.time_scale.scroll_by(Px(-w * 0.1)),
            Key::ArrowUp => self.zoom_time(w, 1.25),
            Key::ArrowDown => self.zoom_time(w, 0.8),
            Key::Home => self.time_scale.scroll_to_first(),
            Key::End => self.scroll_to_realtime(),
            Key::Escape => {
                let was_dragging = !matches!(self.input.drag, Drag::Idle);
                self.input.drag = Drag::Idle;
                self.input.kinetic = None;
                return InputResult {
                    consumed: was_dragging,
                    cursor: CursorIcon::Default,
                };
            }
            Key::Char(_) => return InputResult::default(),
        }
        InputResult {
            consumed: true,
            cursor: CursorIcon::Default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_uses_recent_samples() {
        let mut s = VelocityTracker::default();
        for i in 0..10 {
            s.record(f64::from(i) * 10.0, i as f32 * 5.0);
        }
        assert!((s.release_velocity(95.0) - 0.5).abs() < 1e-9);
        // A pause before release: no kinetic phase.
        assert!(s.release_velocity(200.0).abs() < 1e-12);
    }
}
