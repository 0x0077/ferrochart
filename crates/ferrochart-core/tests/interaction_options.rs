//! Switching panning, zooming and the crosshair off (r25).
//!
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::appearance::InteractionOptions;
use ferrochart_core::chart::{ChartEvent, SeriesData, SeriesOptions};
use ferrochart_core::frame::{DrawCmd, LayerId, SeriesKind};
use ferrochart_core::geom::{PointCss, SizeCss};
use ferrochart_core::interaction::{
    Button, GesturePhase, InputEvent, InputResult, Key, Modifiers, PointerKind, WheelDelta,
};
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
    meta: false,
};

fn chart(interaction: InteractionOptions) -> Chart {
    let mut chart = Chart::new(ChartOptions {
        interaction,
        ..ChartOptions::default()
    });
    chart.resize(SizeCss { w: 800.0, h: 400.0 }, 1.0);
    let id = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    chart
        .set_data(
            id,
            SeriesData::Ohlc(random_walk(3, 5000, 0, 60_000).unwrap()),
        )
        .unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    let _ = chart.drain_events().count();
    chart
}

fn at(x: f32, y: f32) -> PointCss {
    PointCss { x, y }
}

fn pointer(chart: &mut Chart, kind: PointerKind, id: u64, path: &[PointCss]) -> Vec<InputResult> {
    let mut out = vec![chart.handle_input(InputEvent::PointerDown {
        id,
        kind,
        pos: path[0],
        button: Button::Left,
        mods: NONE,
        time_ms: 0.0,
    })];
    for (i, &pos) in path.iter().enumerate().skip(1) {
        out.push(chart.handle_input(InputEvent::PointerMove {
            id,
            kind,
            pos,
            mods: NONE,
            time_ms: 16.0 * i as f64,
        }));
    }
    out.push(chart.handle_input(InputEvent::PointerUp {
        id,
        kind,
        pos: path[path.len() - 1],
        button: Button::Left,
        mods: NONE,
        time_ms: 16.0 * path.len() as f64,
    }));
    out
}

fn range(chart: &mut Chart) -> (f64, f64) {
    chart.build_frame(1000.0, &MonoMeasurer);
    chart.time_scale().visible_range()
}

fn clicks(chart: &mut Chart) -> usize {
    chart
        .drain_events()
        .filter(|e| matches!(e, ChartEvent::Click { .. }))
        .count()
}

fn wheel(chart: &mut Chart, y: f32, shift: bool) -> bool {
    chart
        .handle_input(InputEvent::Wheel {
            pos: at(300.0, 200.0),
            delta: WheelDelta::Lines { x: 0.0, y },
            mods: Modifiers { shift, ..NONE },
        })
        .consumed
}

#[test]
fn without_pan_a_drag_is_not_consumed_and_moves_nothing() {
    let mut c = chart(InteractionOptions {
        pan: false,
        ..InteractionOptions::default()
    });
    let before = range(&mut c);
    let r = pointer(
        &mut c,
        PointerKind::Mouse,
        0,
        &[at(300.0, 200.0), at(340.0, 200.0), at(400.0, 200.0)],
    );
    assert!(!r[1].consumed && !r[2].consumed);
    assert_eq!(range(&mut c), before);
    assert_eq!(clicks(&mut c), 0, "a drag is not a click");

    // A press without movement still clicks.
    pointer(&mut c, PointerKind::Touch, 1, &[at(300.0, 200.0)]);
    assert_eq!(clicks(&mut c), 1);
    // Shift + wheel and the arrow keys do not pan.
    assert!(!wheel(&mut c, 3.0, true));
    let key = c.handle_input(InputEvent::Key {
        key: Key::ArrowLeft,
        pressed: true,
        mods: NONE,
    });
    assert!(!key.consumed);
    assert_eq!(range(&mut c), before);
    // Zooming still works.
    assert!(wheel(&mut c, -3.0, false));
    assert_ne!(range(&mut c), before);
}

#[test]
fn without_zoom_wheel_pinch_and_axes_are_not_consumed() {
    let mut c = chart(InteractionOptions {
        zoom: false,
        ..InteractionOptions::default()
    });
    let before = range(&mut c);
    assert!(!wheel(&mut c, -3.0, false));
    let pinch = c.handle_input(InputEvent::Pinch {
        phase: GesturePhase::Update,
        center: at(300.0, 200.0),
        scale: 1.5,
    });
    assert!(!pinch.consumed);
    // Two fingers: the second is ignored, the first pans.
    c.handle_input(InputEvent::PointerDown {
        id: 1,
        kind: PointerKind::Touch,
        pos: at(300.0, 200.0),
        button: Button::Left,
        mods: NONE,
        time_ms: 0.0,
    });
    let second = c.handle_input(InputEvent::PointerDown {
        id: 2,
        kind: PointerKind::Touch,
        pos: at(400.0, 200.0),
        button: Button::Left,
        mods: NONE,
        time_ms: 0.0,
    });
    assert!(!second.consumed);
    c.handle_input(InputEvent::PointerMove {
        id: 2,
        kind: PointerKind::Touch,
        pos: at(500.0, 200.0),
        mods: NONE,
        time_ms: 16.0,
    });
    assert_eq!(range(&mut c), before);
    c.handle_input(InputEvent::PointerCancel { id: 1 });
    // Dragging the price axis (right edge) does not scale.
    let axis = c.pane_layout(c.main_pane()).unwrap().price_axis;
    let x = axis.x + axis.w * 0.5;
    let r = pointer(&mut c, PointerKind::Mouse, 3, &[at(x, 100.0), at(x, 200.0)]);
    assert!(!r[0].consumed);
    assert_eq!(range(&mut c), before);
    // Shift + wheel still pans.
    assert!(wheel(&mut c, 3.0, true));
    assert_ne!(range(&mut c), before);
}

fn overlay_lines(chart: &Chart) -> usize {
    chart.frame().layers[LayerId::Overlay as usize]
        .cmds
        .iter()
        .map(|c| match c {
            DrawCmd::Lines(s) => s.len as usize,
            _ => 0,
        })
        .sum()
}

#[test]
fn without_crosshair_the_pointer_draws_nothing() {
    let mut on = chart(InteractionOptions::default());
    let mut off = chart(InteractionOptions {
        crosshair: false,
        ..InteractionOptions::default()
    });
    for c in [&mut on, &mut off] {
        c.handle_input(InputEvent::PointerMove {
            id: 0,
            kind: PointerKind::Mouse,
            pos: at(300.0, 200.0),
            mods: NONE,
            time_ms: 0.0,
        });
        c.build_frame(16.0, &MonoMeasurer);
    }
    assert!(overlay_lines(&on) > 0);
    assert_eq!(overlay_lines(&off), 0);
    let moved = off
        .drain_events()
        .any(|e| matches!(e, ChartEvent::CrosshairMove { point } if point.is_some()));
    assert!(!moved);

    // A long press shows nothing either, and does not pan.
    let before = range(&mut off);
    off.handle_input(InputEvent::PointerDown {
        id: 5,
        kind: PointerKind::Touch,
        pos: at(300.0, 200.0),
        button: Button::Left,
        mods: NONE,
        time_ms: 2000.0,
    });
    off.build_frame(2600.0, &MonoMeasurer);
    assert_eq!(overlay_lines(&off), 0);
    off.handle_input(InputEvent::PointerUp {
        id: 5,
        kind: PointerKind::Touch,
        pos: at(300.0, 200.0),
        button: Button::Left,
        mods: NONE,
        time_ms: 2700.0,
    });
    assert_eq!(range(&mut off), before);
}

#[test]
fn switching_pan_off_ends_a_pan() {
    let mut c = chart(InteractionOptions::default());
    c.handle_input(InputEvent::PointerDown {
        id: 0,
        kind: PointerKind::Mouse,
        pos: at(300.0, 200.0),
        button: Button::Left,
        mods: NONE,
        time_ms: 0.0,
    });
    c.handle_input(InputEvent::PointerMove {
        id: 0,
        kind: PointerKind::Mouse,
        pos: at(350.0, 200.0),
        mods: NONE,
        time_ms: 16.0,
    });
    c.set_interaction(InteractionOptions::none());
    let before = range(&mut c);
    let r = c.handle_input(InputEvent::PointerMove {
        id: 0,
        kind: PointerKind::Mouse,
        pos: at(450.0, 200.0),
        mods: NONE,
        time_ms: 32.0,
    });
    assert!(!r.consumed);
    assert_eq!(range(&mut c), before);
}
