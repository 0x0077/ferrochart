//! Saving and loading chart state: round
//! trips (also through JSON with the `serde` feature), drawings replacing the
//! current ones, device-pixel independence, and errors that leave the chart
//! unchanged.
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::chart::{ChartEvent, SeriesData, SeriesOptions};
use ferrochart_core::drawing::{Anchor, DrawingData, DrawingStyle, Param};
use ferrochart_core::frame::{LineDash, SeriesKind};
use ferrochart_core::geom::{Css, Rgba, SizeCss};
use ferrochart_core::interaction::CrosshairMode;
use ferrochart_core::pane::PaneOptions;
use ferrochart_core::price_scale::{PriceScaleMode, ScaleSide};
use ferrochart_core::state::{ChartState, STATE_VERSION};
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions, Error};

const START: i64 = 1_704_067_200_000;
const MIN: i64 = 60_000;

/// What the host sets up: candles, and the volume in a second pane.
fn setup(dpr: f32) -> Chart {
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: 800.0, h: 400.0 }, dpr);
    let candles = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    let pane = chart.add_pane(PaneOptions {
        stretch_factor: 0.3,
    });
    chart
        .add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(candles))
        .unwrap();
    chart
        .set_data(
            candles,
            SeriesData::Ohlc(random_walk(3, 500, START, MIN).unwrap()),
        )
        .unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    chart.drain_events().for_each(drop);
    chart
}

fn anchor(bar: i64, price: f64) -> Anchor {
    Anchor {
        time: START + bar * MIN,
        price,
    }
}

/// A chart with changed view, scales and drawings.
fn edited(dpr: f32) -> Chart {
    let mut chart = setup(dpr);
    let main = chart.main_pane();
    let lower = chart.panes().nth(1).unwrap();
    chart.time_scale().set_bar_spacing(9.0 * f64::from(dpr));
    chart.time_scale().set_right_offset(-20.5);
    chart.set_crosshair_mode(CrosshairMode::Magnet);
    chart.set_pane_stretch(lower, 0.6).unwrap();
    let scale = chart.price_scale(main, ScaleSide::Right).unwrap();
    scale.set_mode(PriceScaleMode::Logarithmic).unwrap();
    scale.set_range(80.0, 130.0).unwrap();
    scale.set_inverted(true);
    scale.set_margins(0.05, 0.2).unwrap();

    let d = chart.drawings();
    let mut fib = DrawingData::new("fib_retracement", vec![anchor(10, 90.0), anchor(40, 110.0)]);
    fib.set("levels", Param::Numbers(vec![0.0, 0.5, 1.0]));
    fib.style = DrawingStyle {
        line: Rgba::hex(0x00FF_9800),
        line_width: Css(2.0),
        line_dash: LineDash::Dashed,
        ..DrawingStyle::default()
    };
    d.add(main, &fib).unwrap();
    let mut text = DrawingData::new("text", vec![anchor(100, 100.0)]);
    text.set("text", Param::Text("note «1»".into()));
    let t = d.add(main, &text).unwrap();
    d.set_locked(t, true).unwrap();
    let v = d
        .add(
            lower,
            &DrawingData::new("vertical_line", vec![anchor(200, 0.0)]),
        )
        .unwrap();
    d.set_visible(v, false).unwrap();
    d.add(
        main,
        &DrawingData::new(
            "long_position",
            vec![anchor(300, 100.0), anchor(320, 104.0)],
        ),
    )
    .unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    chart.drain_events().for_each(drop);
    chart
}

#[test]
fn round_trip() {
    let mut a = edited(2.0);
    let state = a.save_state();
    assert_eq!(state.version, STATE_VERSION);
    assert_eq!(state.panes.len(), 2);
    assert_eq!(state.drawings.len(), 4);
    assert_eq!(state.bar_spacing, Css(9.0));

    let mut b = setup(2.0);
    b.load_state(state.clone()).unwrap();
    assert_eq!(b.save_state(), state);
    b.build_frame(0.0, &MonoMeasurer);
    assert_eq!(
        b.time_scale().visible_range(),
        a.time_scale().visible_range()
    );
    assert_eq!(b.layout(), a.layout());
    // The same frame: same commands in every layer.
    for (la, lb) in a.frame().layers.iter().zip(&b.frame().layers) {
        assert_eq!(la.cmds, lb.cmds);
    }
}

#[test]
fn spacing_is_saved_in_css_pixels() {
    let state = edited(2.0).save_state();
    let mut b = setup(1.0);
    b.load_state(state).unwrap();
    assert_eq!(b.time_scale().bar_spacing(), 9.0);
}

#[test]
fn loading_replaces_drawings_and_clears_history() {
    let state = edited(1.0).save_state();
    let mut b = setup(1.0);
    let main = b.main_pane();
    let old = b
        .drawings()
        .add(
            main,
            &DrawingData::new("horizontal_line", vec![anchor(0, 95.0)]),
        )
        .unwrap();
    b.drawings().select(Some(old)).unwrap();
    assert!(b.drawings().can_undo());
    b.drain_events().for_each(drop);
    b.load_state(state).unwrap();

    let lower = b.panes().nth(1);
    let d = b.drawings();
    assert!(d.data(old).is_none());
    assert_eq!(d.ids().count(), 4);
    assert!(!d.can_undo() && !d.can_redo());
    assert_eq!(d.selected(), None);
    let ids: Vec<_> = d.ids().collect();
    assert_eq!(d.is_locked(ids[1]), Some(true));
    assert_eq!(d.is_visible(ids[2]), Some(false));
    assert_eq!(d.pane_of(ids[2]), lower);

    b.build_frame(0.0, &MonoMeasurer);
    let events: Vec<_> = b.drain_events().collect();
    assert!(events.contains(&ChartEvent::DrawingRemoved(old)));
    assert!(events.contains(&ChartEvent::DrawingSelected(None)));
    let created = events
        .iter()
        .filter(|e| matches!(e, ChartEvent::DrawingCreated(_)))
        .count();
    assert_eq!(created, 4);
}

#[test]
fn auto_scaled_ranges_are_not_saved() {
    let state = edited(1.0).save_state();
    assert_eq!(state.panes[0].price_scale.range, Some((80.0, 130.0)));
    assert_eq!(state.panes[1].price_scale.range, None);
    // Loading `None` turns auto-scaling back on.
    let mut b = setup(1.0);
    let main = b.main_pane();
    b.price_scale(main, ScaleSide::Right)
        .unwrap()
        .set_range(1.0, 2.0)
        .unwrap();
    b.load_state(setup(1.0).save_state()).unwrap();
    assert!(b.price_scale(main, ScaleSide::Right).unwrap().auto_scale());
}

/// A change to a saved state.
type Edit = Box<dyn Fn(&mut ChartState)>;

#[test]
fn errors_leave_the_chart_unchanged() {
    let mut chart = edited(1.0);
    let before = chart.save_state();
    let good = before.clone();
    let cases: Vec<(&str, Edit)> = vec![
        ("newer version", Box::new(|s| s.version = STATE_VERSION + 1)),
        (
            "fewer panes",
            Box::new(|s| {
                s.panes.pop();
            }),
        ),
        ("spacing", Box::new(|s| s.bar_spacing = Css(0.0))),
        ("offset", Box::new(|s| s.right_offset = f64::INFINITY)),
        (
            "margins",
            Box::new(|s| s.panes[0].price_scale.margins = (0.6, 0.5)),
        ),
        (
            "log range",
            Box::new(|s| s.panes[0].price_scale.range = Some((-1.0, 10.0))),
        ),
        ("drawing pane", Box::new(|s| s.drawings[0].pane = 2)),
        (
            "drawing kind",
            Box::new(|s| s.drawings[3].data.kind = "nope".into()),
        ),
        (
            "drawing anchors",
            Box::new(|s| s.drawings[1].data.anchors.clear()),
        ),
    ];
    for (name, edit) in cases {
        let mut s = good.clone();
        edit(&mut s);
        let err = chart.load_state(s);
        assert!(matches!(err, Err(Error::InvalidArgument(_))), "{name}");
        assert_eq!(chart.save_state(), before, "{name}");
        assert!(chart.drain_events().next().is_none(), "{name}");
    }
}

#[test]
fn unfinished_drawing_is_not_saved() {
    use ferrochart_core::geom::PointCss;
    use ferrochart_core::interaction::{Button, InputEvent, Modifiers};
    let mut chart = setup(1.0);
    chart.drawings().start_tool("trend_line").unwrap();
    let p = chart.layout().plot;
    let pos = PointCss {
        x: p.x + p.w * 0.5,
        y: p.y + p.h * 0.5,
    };
    let mods = Modifiers::default();
    chart.handle_input(InputEvent::PointerDown {
        id: 0,
        pos,
        button: Button::Left,
        mods,
        time_ms: 0.0,
    });
    chart.handle_input(InputEvent::PointerUp {
        id: 0,
        pos,
        button: Button::Left,
        mods,
        time_ms: 10.0,
    });
    assert_eq!(chart.drawings().ids().count(), 1);
    assert!(chart.save_state().drawings.is_empty());
}

#[cfg(feature = "serde")]
#[test]
fn json_round_trip() {
    let state = edited(2.0).save_state();
    let json = serde_json::to_string(&state).unwrap();
    let back: ChartState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, state);
    let mut b = setup(2.0);
    b.load_state(back).unwrap();
    assert_eq!(b.save_state(), state);
    // Readable field names, e.g. for hosts that edit saved layouts.
    assert!(json.contains("\"bar_spacing\":9.0"), "{json}");
    assert!(json.contains("\"kind\":\"fib_retracement\""), "{json}");
}
