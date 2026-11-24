//! Touch interaction: the touch rules,
//! the two-finger pinch, the long-press crosshair, the touch drag threshold and
//! hit tolerances, `PointerCancel`, and the host's pinch during a pinch.
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::chart::{ChartEvent, ClickTarget, SeriesData, SeriesOptions};
use ferrochart_core::drawing::{Anchor, DrawingData};
use ferrochart_core::frame::{Invalidation, SeriesKind};
use ferrochart_core::geom::{PointCss, Px, SizeCss};
use ferrochart_core::interaction::{
    Button, CrosshairMode, CursorIcon, GesturePhase, InputEvent, Modifiers, PointerKind,
};
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions};
use proptest::prelude::*;

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
    meta: false,
};
const TOUCH: PointerKind = PointerKind::Touch;
const MOUSE: PointerKind = PointerKind::Mouse;

fn chart() -> Chart {
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: 800.0, h: 400.0 }, 1.0);
    let id = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    let data = random_walk(3, 5000, 0, 60_000).unwrap();
    chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
    frame(&mut chart, 0.0);
    events(&mut chart);
    chart
}

fn frame(chart: &mut Chart, now: f64) {
    chart.build_frame(now, &MonoMeasurer);
}

fn events(chart: &mut Chart) -> Vec<ChartEvent> {
    chart.drain_events().collect()
}

fn at(x: f32, y: f32) -> PointCss {
    PointCss { x, y }
}

fn down(chart: &mut Chart, id: u64, kind: PointerKind, pos: PointCss, t: f64) {
    chart.handle_input(InputEvent::PointerDown {
        id,
        kind,
        pos,
        button: Button::Left,
        mods: NONE,
        time_ms: t,
    });
}

fn mv(chart: &mut Chart, id: u64, kind: PointerKind, pos: PointCss, t: f64) {
    chart.handle_input(InputEvent::PointerMove {
        id,
        kind,
        pos,
        mods: NONE,
        time_ms: t,
    });
}

fn up(chart: &mut Chart, id: u64, kind: PointerKind, pos: PointCss, t: f64) {
    chart.handle_input(InputEvent::PointerUp {
        id,
        kind,
        pos,
        button: Button::Left,
        mods: NONE,
        time_ms: t,
    });
}

/// Logical index under CSS x.
fn index_at(chart: &mut Chart, x: f32) -> f64 {
    let plot = chart.layout().plot;
    chart.time_scale().x_to_index(Px(x - plot.x))
}

fn clicks(ev: &[ChartEvent]) -> Vec<Option<ClickTarget>> {
    ev.iter()
        .filter_map(|e| match e {
            ChartEvent::Click { target, .. } => Some(*target),
            _ => None,
        })
        .collect()
}

/// The last crosshair position reported: `Some(shown)`, or `None` if the
/// crosshair did not change.
fn crosshair(ev: &[ChartEvent]) -> Option<bool> {
    ev.iter().rev().find_map(|e| match e {
        ChartEvent::CrosshairMove { point } => Some(point.is_some()),
        _ => None,
    })
}

#[test]
fn pinch_zooms_around_the_midpoint_and_pans_with_it() {
    let mut c = chart();
    let spacing = c.time_scale().bar_spacing();
    let under = index_at(&mut c, 400.0);
    down(&mut c, 1, TOUCH, at(300.0, 200.0), 0.0);
    down(&mut c, 2, TOUCH, at(500.0, 200.0), 10.0);
    mv(&mut c, 1, TOUCH, at(250.0, 200.0), 20.0);
    mv(&mut c, 2, TOUCH, at(550.0, 200.0), 30.0);
    assert!((c.time_scale().bar_spacing() / spacing - 1.5).abs() < 1e-6);
    assert!((index_at(&mut c, 400.0) - under).abs() < 1e-6);
    assert!(c.captures_touch());
    // Both fingers move right: the bars under them follow.
    mv(&mut c, 1, TOUCH, at(300.0, 200.0), 40.0);
    mv(&mut c, 2, TOUCH, at(600.0, 200.0), 50.0);
    assert!((index_at(&mut c, 450.0) - under).abs() < 1e-6);
    // The host's pinch of the same gesture is ignored.
    let s = c.time_scale().bar_spacing();
    c.handle_input(InputEvent::Pinch {
        phase: GesturePhase::Update,
        center: at(450.0, 200.0),
        scale: 2.0,
    });
    assert_eq!(c.time_scale().bar_spacing(), s);
    // No crosshair while pinching.
    frame(&mut c, 60.0);
    assert_ne!(crosshair(&events(&mut c)), Some(true));
}

#[test]
fn the_remaining_finger_pans_on_without_a_jump() {
    let mut c = chart();
    down(&mut c, 1, TOUCH, at(300.0, 200.0), 0.0);
    down(&mut c, 2, TOUCH, at(500.0, 200.0), 10.0);
    mv(&mut c, 2, TOUCH, at(550.0, 200.0), 20.0);
    up(&mut c, 2, TOUCH, at(550.0, 200.0), 30.0);
    let before = c.time_scale().visible_range();
    mv(&mut c, 1, TOUCH, at(300.0, 200.0), 40.0);
    assert_eq!(c.time_scale().visible_range(), before);
    let spacing = c.time_scale().bar_spacing();
    mv(&mut c, 1, TOUCH, at(360.0, 200.0), 50.0);
    let after = c.time_scale().visible_range();
    assert!((before.0 - after.0 - 60.0 / spacing).abs() < 1e-6);
    up(&mut c, 1, TOUCH, at(360.0, 200.0), 500.0);
    assert!(clicks(&events(&mut c)).is_empty());
}

#[test]
fn a_long_press_shows_a_crosshair_that_stays_until_the_next_tap() {
    let mut c = chart();
    down(&mut c, 1, TOUCH, at(400.0, 200.0), 1000.0);
    assert!(c.needs_redraw() >= Invalidation::Cursor);
    frame(&mut c, 1499.0);
    assert_ne!(crosshair(&events(&mut c)), Some(true));
    frame(&mut c, 1500.0);
    assert_eq!(crosshair(&events(&mut c)), Some(true));
    assert!(c.captures_touch());
    // It follows the finger instead of panning.
    let range = c.time_scale().visible_range();
    mv(&mut c, 1, TOUCH, at(300.0, 150.0), 1600.0);
    frame(&mut c, 1600.0);
    assert_eq!(c.time_scale().visible_range(), range);
    assert_eq!(crosshair(&events(&mut c)), Some(true));
    // Lifting keeps it and is no click.
    up(&mut c, 1, TOUCH, at(300.0, 150.0), 1700.0);
    frame(&mut c, 1700.0);
    let ev = events(&mut c);
    assert!(clicks(&ev).is_empty());
    assert_eq!(crosshair(&ev), None);
    assert_eq!(c.needs_redraw(), Invalidation::None);
    // A tap hides it and is a click.
    down(&mut c, 1, TOUCH, at(500.0, 200.0), 2000.0);
    up(&mut c, 1, TOUCH, at(502.0, 200.0), 2050.0);
    frame(&mut c, 2050.0);
    let ev = events(&mut c);
    assert_eq!(clicks(&ev).len(), 1);
    assert_eq!(crosshair(&ev), Some(false));
}

#[test]
fn a_long_press_is_decided_by_event_time_without_a_frame() {
    let mut c = chart();
    down(&mut c, 1, TOUCH, at(400.0, 200.0), 0.0);
    up(&mut c, 1, TOUCH, at(400.0, 200.0), 499.0);
    assert_eq!(clicks(&events(&mut c)).len(), 1);
    down(&mut c, 1, TOUCH, at(400.0, 200.0), 1000.0);
    up(&mut c, 1, TOUCH, at(400.0, 200.0), 1500.0);
    frame(&mut c, 1500.0);
    let ev = events(&mut c);
    assert!(clicks(&ev).is_empty());
    assert_eq!(crosshair(&ev), Some(true));
}

#[test]
fn with_the_crosshair_hidden_a_long_press_still_pans() {
    let mut c = chart();
    c.set_crosshair_mode(CrosshairMode::Hidden);
    down(&mut c, 1, TOUCH, at(400.0, 200.0), 0.0);
    frame(&mut c, 800.0);
    let before = c.time_scale().visible_range();
    mv(&mut c, 1, TOUCH, at(460.0, 200.0), 900.0);
    assert_ne!(c.time_scale().visible_range(), before);
}

#[test]
fn a_finger_pans_without_a_crosshair_and_has_a_wider_threshold() {
    let mut c = chart();
    // 5 px is a tap for a finger, a pan for a mouse.
    down(&mut c, 1, TOUCH, at(400.0, 200.0), 0.0);
    mv(&mut c, 1, TOUCH, at(405.0, 200.0), 10.0);
    up(&mut c, 1, TOUCH, at(405.0, 200.0), 20.0);
    assert_eq!(clicks(&events(&mut c)).len(), 1);
    down(&mut c, 1, MOUSE, at(400.0, 200.0), 100.0);
    mv(&mut c, 1, MOUSE, at(405.0, 200.0), 110.0);
    up(&mut c, 1, MOUSE, at(405.0, 200.0), 300.0);
    assert!(clicks(&events(&mut c)).is_empty());
    c.handle_input(InputEvent::PointerLeave);
    frame(&mut c, 400.0);
    events(&mut c);
    // A finger pan shows no crosshair.
    down(&mut c, 1, TOUCH, at(400.0, 200.0), 500.0);
    mv(&mut c, 1, TOUCH, at(460.0, 200.0), 510.0);
    // A pan leaves vertical swipes to the page (`touchScroll`).
    assert!(!c.captures_touch());
    frame(&mut c, 510.0);
    assert_ne!(crosshair(&events(&mut c)), Some(true));
}

#[test]
fn cancel_ends_the_gesture_without_click_or_kinetic() {
    let mut c = chart();
    down(&mut c, 1, TOUCH, at(400.0, 200.0), 0.0);
    c.handle_input(InputEvent::PointerCancel { id: 1 });
    assert!(clicks(&events(&mut c)).is_empty());
    assert_eq!(c.needs_redraw(), Invalidation::None);
    // A fast pan cancelled mid-flight does not coast.
    down(&mut c, 1, TOUCH, at(400.0, 200.0), 100.0);
    for i in 1..=5 {
        mv(
            &mut c,
            1,
            TOUCH,
            at(400.0 + 30.0 * i as f32, 200.0),
            100.0 + 10.0 * f64::from(i),
        );
    }
    c.handle_input(InputEvent::PointerCancel { id: 1 });
    assert!(!c.is_animating());
    // Cancelling one finger of a pinch leaves the other panning.
    down(&mut c, 1, TOUCH, at(300.0, 200.0), 1000.0);
    down(&mut c, 2, TOUCH, at(500.0, 200.0), 1010.0);
    c.handle_input(InputEvent::PointerCancel { id: 2 });
    let before = c.time_scale().visible_range();
    mv(&mut c, 1, TOUCH, at(340.0, 200.0), 1020.0);
    assert_ne!(c.time_scale().visible_range(), before);
}

/// A horizontal line through (400, 200), returned with its id.
fn with_line(c: &mut Chart) -> ferrochart_core::data::DrawingId {
    down(c, 9, MOUSE, at(400.0, 200.0), 0.0);
    up(c, 9, MOUSE, at(400.0, 200.0), 10.0);
    let (time, price) = events(c)
        .iter()
        .find_map(|e| match e {
            ChartEvent::Click { time, price, .. } => Some(((*time).unwrap(), *price)),
            _ => None,
        })
        .unwrap();
    let pane = c.main_pane();
    let id = c
        .drawings()
        .add(
            pane,
            &DrawingData::new("horizontal_line", vec![Anchor { time, price }]),
        )
        .unwrap();
    frame(c, 20.0);
    events(c);
    id
}

#[test]
fn a_finger_hits_drawings_from_further_away() {
    let mut c = chart();
    let line = with_line(&mut c);
    // 8 px below the line: a miss for a mouse, a hit for a finger.
    down(&mut c, 1, MOUSE, at(300.0, 208.0), 100.0);
    up(&mut c, 1, MOUSE, at(300.0, 208.0), 110.0);
    assert_eq!(clicks(&events(&mut c)), vec![None]);
    down(&mut c, 1, TOUCH, at(300.0, 208.0), 200.0);
    up(&mut c, 1, TOUCH, at(300.0, 208.0), 210.0);
    assert_eq!(
        clicks(&events(&mut c)),
        vec![Some(ClickTarget::Drawing(line))]
    );
}

#[test]
fn a_second_finger_is_ignored_while_dragging_a_drawing() {
    let mut c = chart();
    with_line(&mut c);
    let spacing = c.time_scale().bar_spacing();
    down(&mut c, 1, TOUCH, at(300.0, 202.0), 100.0);
    down(&mut c, 2, TOUCH, at(600.0, 300.0), 110.0);
    mv(&mut c, 2, TOUCH, at(700.0, 300.0), 120.0);
    mv(&mut c, 1, TOUCH, at(300.0, 250.0), 130.0);
    assert_eq!(c.time_scale().bar_spacing(), spacing);
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Down(u64, f32, f32),
    Move(u64, f32, f32),
    Up(u64, f32, f32),
    Cancel(u64),
    Frame,
}

fn op() -> impl Strategy<Value = Op> {
    let id = 1u64..4;
    let x = -50.0f32..850.0;
    let y = -50.0f32..450.0;
    prop_oneof![
        (id.clone(), x.clone(), y.clone()).prop_map(|(i, x, y)| Op::Down(i, x, y)),
        (id.clone(), x.clone(), y.clone()).prop_map(|(i, x, y)| Op::Move(i, x, y)),
        (id.clone(), x, y).prop_map(|(i, x, y)| Op::Up(i, x, y)),
        id.prop_map(Op::Cancel),
        Just(Op::Frame),
    ]
}

proptest! {
    /// Any order of presses, moves, releases and cancels of up to three
    /// fingers keeps the chart consistent, and lifting every finger ends
    /// the gesture: a fresh pan works.
    #[test]
    fn any_touch_sequence_ends_idle(ops in prop::collection::vec(op(), 1..60)) {
        let mut c = chart();
        let mut t = 0.0;
        for o in ops {
            t += 40.0;
            match o {
                Op::Down(i, x, y) => down(&mut c, i, TOUCH, at(x, y), t),
                Op::Move(i, x, y) => mv(&mut c, i, TOUCH, at(x, y), t),
                Op::Up(i, x, y) => up(&mut c, i, TOUCH, at(x, y), t),
                Op::Cancel(i) => {
                    c.handle_input(InputEvent::PointerCancel { id: i });
                }
                Op::Frame => frame(&mut c, t),
            }
            let s = c.time_scale().bar_spacing();
            prop_assert!(s.is_finite() && s > 0.0);
        }
        for i in 1..4 {
            c.handle_input(InputEvent::PointerCancel { id: i });
        }
        frame(&mut c, t + 10_000.0);
        down(&mut c, 7, TOUCH, at(400.0, 200.0), t + 20_000.0);
        let r = c.handle_input(InputEvent::PointerMove {
            id: 7,
            kind: TOUCH,
            pos: at(460.0, 200.0),
            mods: NONE,
            time_ms: t + 20_010.0,
        });
        prop_assert_eq!(r.cursor, CursorIcon::Grabbing);
    }
}
