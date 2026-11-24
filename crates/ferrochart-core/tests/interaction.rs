//! Interaction tests: scripted `InputEvent` sequences drive a `Chart`; the
//! tests assert visible range, scale state, frame layers and emitted events.
//! No window system involved.
//!
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::chart::{ChartEvent, SeriesData, SeriesOptions};
use ferrochart_core::frame::{Invalidation, LayerId, SeriesKind};
use ferrochart_core::geom::{Css, PointCss, SizeCss};
use ferrochart_core::interaction::{
    Button, CrosshairMode, CursorIcon, InputEvent, Key, Modifiers, WheelDelta,
};
use ferrochart_core::price_scale::ScaleSide;
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions};

const START: i64 = 1_704_067_200_000;
/// Device pixel ratio of the test chart.
const DPR: f32 = 2.0;

/// A physical-pixel point of the layout, as CSS px.
fn css(x: f32, y: f32) -> PointCss {
    PointCss {
        x: x / DPR,
        y: y / DPR,
    }
}
const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
    meta: false,
};

fn chart() -> Chart {
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: 800.0, h: 400.0 }, DPR);
    let pane = chart.main_pane();
    let id = chart
        .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    chart
        .add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(id))
        .unwrap();
    let data = random_walk(3, 5000, START, 60_000).unwrap();
    chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
    frame(&mut chart, 0.0);
    chart.drain_events().for_each(drop);
    chart
}

fn frame(chart: &mut Chart, now: f64) {
    chart.build_frame(now, &MonoMeasurer);
}

fn at(x: f32, y: f32) -> PointCss {
    PointCss { x, y }
}

fn down(chart: &mut Chart, p: PointCss, t: f64) -> CursorIcon {
    chart
        .handle_input(InputEvent::PointerDown {
            id: 1,
            kind: ferrochart_core::interaction::PointerKind::Mouse,
            pos: p,
            button: Button::Left,
            mods: NONE,
            time_ms: t,
        })
        .cursor
}

fn mv(chart: &mut Chart, p: PointCss, t: f64) -> CursorIcon {
    chart
        .handle_input(InputEvent::PointerMove {
            id: 1,
            kind: ferrochart_core::interaction::PointerKind::Mouse,
            pos: p,
            mods: NONE,
            time_ms: t,
        })
        .cursor
}

fn up(chart: &mut Chart, p: PointCss, t: f64) {
    chart.handle_input(InputEvent::PointerUp {
        id: 1,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos: p,
        button: Button::Left,
        mods: NONE,
        time_ms: t,
    });
}

/// A plot point in CSS px (the price axis is on the right, the time axis below).
fn plot_point(chart: &Chart, fx: f32, fy: f32) -> PointCss {
    let p = chart.layout().plot;
    css(p.x + p.w * fx, p.y + p.h * fy)
}

#[test]
fn click_without_drag_does_not_pan() {
    let mut chart = chart();
    let before = chart.time_scale().visible_range();
    let p = plot_point(&chart, 0.5, 0.5);
    down(&mut chart, p, 0.0);
    mv(&mut chart, at(p.x + 1.0, p.y), 10.0);
    up(&mut chart, at(p.x + 1.0, p.y), 20.0);
    assert_eq!(chart.time_scale().visible_range(), before);
    assert!(!chart.is_animating());
}

#[test]
fn fling_starts_kinetic_scrolling_that_decays_and_stops() {
    let mut chart = chart();
    chart
        .time_scale()
        .scroll_by(ferrochart_core::geom::Px(-3000.0));
    frame(&mut chart, 0.0);
    let p = plot_point(&chart, 0.5, 0.5);
    assert_eq!(down(&mut chart, p, 0.0), CursorIcon::Crosshair);
    let mut t = 0.0;
    for i in 1..=10 {
        t = f64::from(i) * 10.0;
        // 20 CSS px per 10 ms = 4 physical px/ms at dpr 2.
        assert_eq!(
            mv(&mut chart, at(p.x + 20.0 * i as f32, p.y), t),
            CursorIcon::Grabbing
        );
    }
    up(&mut chart, at(p.x + 200.0, p.y), t + 5.0);
    assert!(chart.is_animating());
    assert!(chart.needs_redraw() >= Invalidation::Light);

    // The right edge depends only on the right offset (the left edge also moves
    // when the price axis width changes).
    let mut last = chart.time_scale().visible_range().1;
    let mut steps = Vec::new();
    let mut now = t + 5.0;
    while chart.is_animating() && now < 10_000.0 {
        now += 16.0;
        frame(&mut chart, now);
        let to = chart.time_scale().visible_range().1;
        steps.push(last - to);
        last = to;
    }
    assert!(!chart.is_animating(), "kinetic scrolling must stop");
    assert!(steps.len() > 10);
    // Moves towards earlier bars, slower and slower.
    assert!(steps.iter().all(|&d| d >= 0.0));
    assert!(steps.windows(2).all(|w| w[1] <= w[0] + 1e-9));
    assert_eq!(chart.needs_redraw(), Invalidation::None);
}

#[test]
fn a_new_press_stops_kinetic_scrolling() {
    let mut chart = chart();
    // Leave room on the right so that dragging left can move.
    chart
        .time_scale()
        .scroll_by(ferrochart_core::geom::Px(3000.0));
    frame(&mut chart, 0.0);
    let p = plot_point(&chart, 0.5, 0.5);
    down(&mut chart, p, 0.0);
    for i in 1..=5 {
        mv(
            &mut chart,
            at(p.x - 30.0 * i as f32, p.y),
            f64::from(i) * 10.0,
        );
    }
    up(&mut chart, at(p.x - 150.0, p.y), 52.0);
    assert!(chart.is_animating());
    down(&mut chart, p, 60.0);
    assert!(!chart.is_animating());
}

#[test]
fn wheel_zoom_keeps_the_bar_under_the_pointer() {
    let mut chart = chart();
    let p = plot_point(&chart, 0.3, 0.5);
    let x = ferrochart_core::geom::Px(p.x * DPR - chart.layout().plot.x);
    let before = chart.time_scale().x_to_index(x);
    let spacing = chart.time_scale().bar_spacing();
    let r = chart.handle_input(InputEvent::Wheel {
        pos: p,
        delta: WheelDelta::Lines { x: 0.0, y: -2.0 },
        mods: NONE,
    });
    assert!(r.consumed);
    assert!(
        chart.time_scale().bar_spacing() > spacing,
        "scrolling up zooms in"
    );
    assert!((chart.time_scale().x_to_index(x) - before).abs() < 1e-6);
}

#[test]
fn shift_wheel_pans_and_alt_wheel_scales_price() {
    let mut chart = chart();
    let p = plot_point(&chart, 0.5, 0.5);
    let spacing = chart.time_scale().bar_spacing();
    let before = chart.time_scale().visible_range();
    chart.handle_input(InputEvent::Wheel {
        pos: p,
        delta: WheelDelta::Pixels {
            x: Css(0.0),
            y: Css(-30.0),
        },
        mods: Modifiers {
            shift: true,
            ..NONE
        },
    });
    assert_eq!(chart.time_scale().bar_spacing(), spacing);
    // Scrolling up moves towards earlier bars, like scrolling a page up.
    assert!(chart.time_scale().visible_range().1 < before.1);

    let pane = chart.main_pane();
    let range = chart.price_scale(pane, ScaleSide::Right).unwrap().range();
    chart.handle_input(InputEvent::Wheel {
        pos: p,
        delta: WheelDelta::Lines { x: 0.0, y: 1.0 },
        mods: Modifiers { alt: true, ..NONE },
    });
    let scale = chart.price_scale(pane, ScaleSide::Right).unwrap();
    assert!(!scale.auto_scale());
    let r = scale.range();
    assert!(
        r.1 - r.0 > range.1 - range.0,
        "scrolling down shows more prices"
    );
}

#[test]
fn price_axis_drag_and_double_click() {
    let mut chart = chart();
    let axis = chart.layout().price_axis;
    let p = css(axis.x + axis.w * 0.5, axis.y + axis.h * 0.5);
    let pane = chart.main_pane();
    let range = chart.price_scale(pane, ScaleSide::Right).unwrap().range();
    assert_eq!(down(&mut chart, p, 0.0), CursorIcon::NsResize);
    mv(&mut chart, at(p.x, p.y + 50.0), 10.0);
    up(&mut chart, at(p.x, p.y + 50.0), 20.0);
    let scale = chart.price_scale(pane, ScaleSide::Right).unwrap();
    assert!(!scale.auto_scale());
    let r = scale.range();
    // Same center, wider range.
    assert!(((r.0 + r.1) - (range.0 + range.1)).abs() < 1e-6 * range.1);
    assert!(r.1 - r.0 > (range.1 - range.0) * 1.2);

    chart.handle_input(InputEvent::DoubleClick {
        pos: p,
        button: Button::Left,
    });
    assert!(
        chart
            .price_scale(pane, ScaleSide::Right)
            .unwrap()
            .auto_scale()
    );
}

#[test]
fn time_axis_drag_and_double_click() {
    let mut chart = chart();
    let axis = chart.layout().time_axis;
    let p = css(axis.x + axis.w * 0.5, axis.y + axis.h * 0.5);
    let spacing = chart.time_scale().bar_spacing();
    assert_eq!(down(&mut chart, p, 0.0), CursorIcon::EwResize);
    mv(&mut chart, at(p.x + 40.0, p.y), 10.0);
    up(&mut chart, at(p.x + 40.0, p.y), 20.0);
    assert!(
        chart.time_scale().bar_spacing() > spacing,
        "dragging right zooms in"
    );
    chart.handle_input(InputEvent::DoubleClick {
        pos: p,
        button: Button::Left,
    });
    assert_eq!(chart.time_scale().bar_spacing(), spacing);
}

#[test]
fn keys_pan_zoom_and_jump() {
    let mut chart = chart();
    let spacing = chart.time_scale().bar_spacing();
    let key = |chart: &mut Chart, key| {
        chart.handle_input(InputEvent::Key {
            key,
            pressed: true,
            mods: NONE,
        })
    };
    let before = chart.time_scale().visible_range();
    assert!(key(&mut chart, Key::ArrowLeft).consumed);
    assert!(chart.time_scale().visible_range().0 < before.0);
    key(&mut chart, Key::ArrowUp);
    assert!(chart.time_scale().bar_spacing() > spacing);
    key(&mut chart, Key::Home);
    assert!(chart.time_scale().visible_range().0.abs() <= 0.5 + 1e-9);
    key(&mut chart, Key::End);
    let (_, to) = chart.time_scale().visible_range();
    assert!((to - (4999.0 + 3.0 + 0.5)).abs() < 1e-9);
    assert!(!key(&mut chart, Key::Char('x')).consumed);
}

#[test]
fn escape_cancels_a_pan() {
    let mut chart = chart();
    let p = plot_point(&chart, 0.5, 0.5);
    down(&mut chart, p, 0.0);
    mv(&mut chart, at(p.x + 50.0, p.y), 10.0);
    let r = chart.handle_input(InputEvent::Key {
        key: Key::Escape,
        pressed: true,
        mods: NONE,
    });
    assert!(r.consumed);
    let range = chart.time_scale().visible_range();
    mv(&mut chart, at(p.x + 150.0, p.y), 20.0);
    assert_eq!(chart.time_scale().visible_range(), range);
}

#[test]
fn crosshair_only_redraws_the_overlay() {
    let mut chart = chart();
    let revs = |chart: &Chart| [0, 1, 2].map(|i| chart.frame().layers[i].revision);
    let before = revs(&chart);
    let p = plot_point(&chart, 0.4, 0.4);
    assert_eq!(mv(&mut chart, p, 0.0), CursorIcon::Crosshair);
    assert_eq!(chart.needs_redraw(), Invalidation::Cursor);
    frame(&mut chart, 0.0);
    let after = revs(&chart);
    assert_eq!(after[0], before[0]);
    assert_eq!(after[1], before[1]);
    assert_eq!(after[2], before[2] + 1);
    let overlay = &chart.frame().layers[LayerId::Overlay as usize].cmds;
    assert!(!overlay.is_empty());

    let events: Vec<_> = chart.drain_events().collect();
    let Some(ChartEvent::CrosshairMove { point: Some(c) }) = events.first() else {
        panic!("expected a crosshair event, got {events:?}");
    };
    assert_eq!(c.time, Some(START + c.logical_index * 60_000));
    assert!(c.bar.is_some());

    chart.handle_input(InputEvent::PointerLeave);
    frame(&mut chart, 0.0);
    assert!(
        chart.frame().layers[LayerId::Overlay as usize]
            .cmds
            .is_empty()
    );
    let events: Vec<_> = chart.drain_events().collect();
    assert_eq!(events, [ChartEvent::CrosshairMove { point: None }]);
}

#[test]
fn magnet_snaps_to_ohlc_and_hidden_draws_nothing() {
    let mut chart = chart();
    chart.set_crosshair_mode(CrosshairMode::Magnet);
    let p = plot_point(&chart, 0.6, 0.5);
    mv(&mut chart, p, 0.0);
    frame(&mut chart, 0.0);
    let events: Vec<_> = chart.drain_events().collect();
    let Some(ChartEvent::CrosshairMove { point: Some(c) }) = events.first() else {
        panic!("expected a crosshair event, got {events:?}");
    };
    let b = c.bar.unwrap();
    let nearest = [b.open, b.high, b.low, b.close]
        .into_iter()
        .map(|v| (v - c.price).abs())
        .fold(f64::INFINITY, f64::min);
    // Within a pixel's worth of price.
    let pane = chart.main_pane();
    let (lo, hi) = chart.price_scale(pane, ScaleSide::Right).unwrap().range();
    assert!(nearest < (hi - lo) / 400.0, "{nearest}");

    chart.set_crosshair_mode(CrosshairMode::Hidden);
    frame(&mut chart, 0.0);
    assert!(
        chart.frame().layers[LayerId::Overlay as usize]
            .cmds
            .is_empty()
    );
}

#[test]
fn events_are_coalesced_and_history_is_requested_once() {
    let mut chart = chart();
    let p = plot_point(&chart, 0.5, 0.5);
    for i in 0..20 {
        mv(&mut chart, at(p.x + i as f32, p.y), 0.0);
        chart.time_scale().scroll_by(ferrochart_core::geom::Px(5.0));
        frame(&mut chart, 0.0);
    }
    let events: Vec<_> = chart.drain_events().collect();
    assert_eq!(events.len(), 2, "{events:?}");

    let key = |chart: &mut Chart, key| {
        chart.handle_input(InputEvent::Key {
            key,
            pressed: true,
            mods: NONE,
        });
    };
    key(&mut chart, Key::Home);
    frame(&mut chart, 0.0);
    key(&mut chart, Key::ArrowRight);
    frame(&mut chart, 0.0);
    key(&mut chart, Key::ArrowLeft);
    frame(&mut chart, 0.0);
    let history = chart
        .drain_events()
        .filter(|e| matches!(e, ChartEvent::NeedMoreHistory { .. }))
        .count();
    assert_eq!(history, 1);
}

/// A chart with candles in the main pane, volume in a second pane and an empty
/// third pane, stretch 2 : 1 : 1.
fn three_panes() -> (Chart, [ferrochart_core::data::PaneId; 3]) {
    use ferrochart_core::pane::PaneOptions;
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: 800.0, h: 600.0 }, DPR);
    let main = chart.main_pane();
    let id = chart
        .add_series(main, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    let vol = chart.add_pane(PaneOptions {
        stretch_factor: 0.5,
    });
    let empty = chart.add_pane(PaneOptions {
        stretch_factor: 0.5,
    });
    chart
        .add_series(vol, SeriesKind::Volume, SeriesOptions::volume_of(id))
        .unwrap();
    let data = random_walk(3, 5000, START, 60_000).unwrap();
    chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
    frame(&mut chart, 0.0);
    chart.drain_events().for_each(drop);
    (chart, [main, vol, empty])
}

#[test]
fn panes_stack_and_share_the_price_axis_width() {
    let (chart, [main, vol, empty]) = three_panes();
    let [a, b, c] = [main, vol, empty].map(|p| chart.pane_layout(p).unwrap());
    assert_eq!(a.plot.y, 0.0);
    assert_eq!(a.plot.bottom() + 1.0, b.plot.y);
    assert_eq!(b.plot.bottom() + 1.0, c.plot.y);
    assert_eq!(c.plot.bottom(), chart.layout().time_axis.y);
    assert!((a.plot.h / b.plot.h - 2.0).abs() < 0.02);
    assert_eq!(a.plot.w, b.plot.w);
    assert_eq!(a.price_axis.x, c.price_axis.x);
    assert_eq!(chart.layout().plot, a.plot);
    // The volume pane has its own auto-scaled range starting at zero.
    let mut chart = chart;
    let (lo, _) = chart.price_scale(vol, ScaleSide::Right).unwrap().range();
    assert_eq!(lo, 0.0);
}

#[test]
fn dragging_a_separator_resizes_the_neighbours() {
    let (mut chart, [main, vol, empty]) = three_panes();
    let b = chart.pane_layout(vol).unwrap().plot;
    let sep = b.y - 0.5;
    assert_eq!(mv(&mut chart, css(300.0, sep), 0.0), CursorIcon::NsResize);
    down(&mut chart, css(300.0, sep), 0.0);
    mv(&mut chart, css(300.0, sep - 100.0), 10.0);
    up(&mut chart, css(300.0, sep - 100.0), 20.0);
    assert_eq!(chart.needs_redraw(), Invalidation::Full);
    frame(&mut chart, 20.0);
    let b2 = chart.pane_layout(vol).unwrap().plot;
    assert!((b2.y - (b.y - 100.0)).abs() <= 1.0, "{} vs {}", b2.y, b.y);
    assert!((b2.h - (b.h + 100.0)).abs() <= 1.0);
    // The third pane keeps its height (up to rounding); the stretch sum of
    // the two is kept.
    assert!((chart.pane_layout(empty).unwrap().plot.h - b.h).abs() <= 1.0);
    let sum = chart.pane_stretch(main).unwrap() + chart.pane_stretch(vol).unwrap();
    assert!((sum - 1.5).abs() < 1e-5);

    // Dragging far past the neighbour keeps a minimum height.
    let sep = b2.y - 0.5;
    down(&mut chart, css(300.0, sep), 30.0);
    mv(&mut chart, css(300.0, -500.0), 40.0);
    up(&mut chart, css(300.0, -500.0), 50.0);
    frame(&mut chart, 50.0);
    let a = chart.pane_layout(main).unwrap().plot;
    assert!(a.h >= 24.0 * DPR - 1.0, "{}", a.h);
}

#[test]
fn price_axis_drag_scales_only_the_pane_under_the_pointer() {
    let (mut chart, [main, vol, _]) = three_panes();
    let before_main = chart.price_scale(main, ScaleSide::Right).unwrap().range();
    let before_vol = chart.price_scale(vol, ScaleSide::Right).unwrap().range();
    let axis = chart.pane_layout(vol).unwrap().price_axis;
    let x = axis.x + axis.w * 0.5;
    let y = axis.y + axis.h * 0.5;
    assert_eq!(down(&mut chart, css(x, y), 0.0), CursorIcon::NsResize);
    mv(&mut chart, css(x, y + 60.0), 10.0);
    up(&mut chart, css(x, y + 60.0), 20.0);
    let scale = chart.price_scale(vol, ScaleSide::Right).unwrap();
    assert!(!scale.auto_scale());
    assert_ne!(scale.range(), before_vol);
    let scale = chart.price_scale(main, ScaleSide::Right).unwrap();
    assert!(scale.auto_scale());
    assert_eq!(scale.range(), before_main);
}

#[test]
fn crosshair_reports_the_hovered_pane() {
    let (mut chart, [_, vol, empty]) = three_panes();
    for (pane, magnet_ok) in [(vol, false), (empty, false)] {
        let p = chart.pane_layout(pane).unwrap().plot;
        mv(&mut chart, css(p.x + p.w * 0.5, p.y + p.h * 0.5), 0.0);
        frame(&mut chart, 0.0);
        let point = chart
            .drain_events()
            .find_map(|e| match e {
                ChartEvent::CrosshairMove { point } => point,
                _ => None,
            })
            .unwrap();
        assert_eq!(point.pane, pane);
        assert!(point.bar.is_some() || magnet_ok);
        // One vertical line per pane plus the horizontal line.
        let lines: u32 = chart.frame().layers[LayerId::Overlay as usize]
            .cmds
            .iter()
            .map(|c| match c {
                ferrochart_core::frame::DrawCmd::Lines(s) => s.len,
                _ => 0,
            })
            .sum();
        assert_eq!(lines, 4);
    }
}

#[test]
fn removing_a_pane_removes_its_series() {
    let (mut chart, [main, vol, empty]) = three_panes();
    chart.remove_pane(empty).unwrap();
    assert_eq!(chart.panes().count(), 2);
    assert!(chart.pane_layout(empty).is_none());
    // The volume series (id 1) lives in `vol`.
    chart.remove_pane(vol).unwrap();
    assert_eq!(
        chart.remove_series(ferrochart_core::data::SeriesId(1)),
        Err(ferrochart_core::Error::UnknownId)
    );
    assert!(matches!(
        chart.remove_pane(main),
        Err(ferrochart_core::Error::InvalidArgument(_))
    ));
    frame(&mut chart, 0.0);
    assert_eq!(chart.layout().plot.bottom(), chart.layout().time_axis.y);
}
