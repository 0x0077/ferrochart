//! Multi-chart sync: crosshair and visible
//! range across charts of different intervals, no echoes, options, leaving,
//! and host-forwarded messages.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::option_option)]

use ferrochart_core::chart::{ChartEvent, CrosshairPoint, SeriesData, SeriesOptions};
use ferrochart_core::data::DrawingId;
use ferrochart_core::drawing::{Anchor, DrawingData, DrawingStyle};
use ferrochart_core::frame::{Invalidation, SeriesKind};
use ferrochart_core::geom::{PointCss, SizeCss};
use ferrochart_core::interaction::{Button, InputEvent, Modifiers};
use ferrochart_core::pane::PaneOptions;
use ferrochart_core::sync::{DrawingSyncOp, SyncGroup, SyncKey, SyncMessage, SyncOptions};
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions};

const START: i64 = 1_704_067_200_000;
const MIN: i64 = 60_000;
const DPR: f32 = 2.0;
const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
    meta: false,
};

/// A chart of `n` bars `step` ms apart.
fn chart(n: usize, step: i64, width: f32) -> Chart {
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: width, h: 400.0 }, DPR);
    let id = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    let data = random_walk(5, n, START, step).unwrap();
    chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
    frame(&mut chart);
    chart
}

/// Builds a frame and returns its events.
fn frame(chart: &mut Chart) -> Vec<ChartEvent> {
    chart.build_frame(0.0, &MonoMeasurer);
    chart.drain_events().collect()
}

fn crosshair(events: &[ChartEvent]) -> Option<Option<CrosshairPoint>> {
    events.iter().rev().find_map(|e| match e {
        ChartEvent::CrosshairMove { point } => Some(*point),
        _ => None,
    })
}

fn sync_events(events: &[ChartEvent]) -> Vec<SyncMessage> {
    events
        .iter()
        .filter_map(|e| match e {
            ChartEvent::Sync(m) => Some(*m),
            _ => None,
        })
        .collect()
}

/// Moves the pointer to fraction `fx` of the main plot's width.
fn hover(chart: &mut Chart, fx: f32) {
    let p = chart.layout().plot;
    let pos = PointCss {
        x: (p.x + p.w * fx) / DPR,
        y: (p.y + p.h * 0.5) / DPR,
    };
    chart.handle_input(InputEvent::PointerMove {
        id: 0,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos,
        mods: NONE,
        time_ms: 0.0,
    });
}

fn drag(chart: &mut Chart, dx: f32) {
    let p = chart.layout().plot;
    let at = |x: f32| PointCss {
        x: (p.x + p.w * 0.5) / DPR + x,
        y: (p.y + p.h * 0.5) / DPR,
    };
    let button = Button::Left;
    chart.handle_input(InputEvent::PointerDown {
        id: 0,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos: at(0.0),
        button,
        mods: NONE,
        time_ms: 0.0,
    });
    chart.handle_input(InputEvent::PointerMove {
        id: 0,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos: at(dx),
        mods: NONE,
        time_ms: 10.0,
    });
    // Released long after the last move: no kinetic scrolling.
    chart.handle_input(InputEvent::PointerUp {
        id: 0,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos: at(dx),
        button,
        mods: NONE,
        time_ms: 1000.0,
    });
}

/// Times at the plot edges of `chart`, interpolated.
fn visible_times(chart: &mut Chart) -> (f64, f64) {
    let (from, to) = chart.time_scale().visible_range();
    let t = |i: f64, chart: &Chart| {
        let view = chart.data_view();
        let index = view.time_index();
        let first = index.first_index() as f64;
        let t0 = index.times()[0] as f64;
        let step = (index.times()[1] - index.times()[0]) as f64;
        t0 + (i - first) * step
    };
    (t(from, chart), t(to, chart))
}

fn pair(opts_b: SyncOptions) -> (Chart, Chart) {
    let group = SyncGroup::new();
    // `a`: 1-minute bars; `b`: 5-minute bars over the same period, narrower.
    let mut a = chart(1000, MIN, 800.0);
    let mut b = chart(200, 5 * MIN, 600.0);
    a.join_sync(&group, SyncOptions::default());
    b.join_sync(&group, opts_b);
    frame(&mut a);
    frame(&mut b);
    (a, b)
}

#[test]
fn crosshair_follows_time_across_intervals() {
    let (mut a, mut b) = pair(SyncOptions::default());
    hover(&mut a, 0.5);
    let ea = frame(&mut a);
    let pa = crosshair(&ea).unwrap().unwrap();
    let t = pa.time.unwrap();
    assert_eq!(sync_events(&ea), [SyncMessage::Crosshair { time: Some(t) }]);

    assert_eq!(b.needs_redraw(), Invalidation::Cursor);
    let eb = frame(&mut b);
    let pb = crosshair(&eb).unwrap().unwrap();
    // The 5-minute bar containing `t`.
    let tb = pb.time.unwrap();
    assert!(tb <= t && t < tb + 5 * MIN, "{tb} {t}");
    assert!(pb.price.is_nan());
    assert!(pb.bar.is_some());
    assert!(
        sync_events(&eb).is_empty(),
        "applied messages are not posted"
    );

    // Nothing bounces back.
    assert_eq!(a.needs_redraw(), Invalidation::None);
    assert_eq!(b.needs_redraw(), Invalidation::None);
}

#[test]
fn leaving_the_plot_clears_the_crosshair_of_the_others() {
    let (mut a, mut b) = pair(SyncOptions::default());
    hover(&mut a, 0.3);
    frame(&mut a);
    frame(&mut b);
    a.handle_input(InputEvent::PointerLeave);
    let ea = frame(&mut a);
    assert_eq!(sync_events(&ea), [SyncMessage::Crosshair { time: None }]);
    assert_eq!(crosshair(&frame(&mut b)), Some(None));
}

#[test]
fn moving_to_another_chart_keeps_its_crosshair() {
    let (mut a, mut b) = pair(SyncOptions::default());
    hover(&mut a, 0.3);
    frame(&mut a);
    frame(&mut b);
    // The pointer moves from `a` to `b`; `b` builds its frame first.
    a.handle_input(InputEvent::PointerLeave);
    hover(&mut b, 0.6);
    let pb = crosshair(&frame(&mut b)).unwrap().unwrap();
    let ea = frame(&mut a);
    // `a`'s removal does not clear `b`'s crosshair; `a` shows `b`'s.
    assert!(sync_events(&ea).is_empty());
    let pa = crosshair(&ea).unwrap().unwrap();
    assert_eq!(pa.time.unwrap(), pb.time.unwrap());
    assert!(frame(&mut b).is_empty());
}

#[test]
fn pan_and_zoom_follow_time() {
    let (mut a, mut b) = pair(SyncOptions::default());
    drag(&mut a, -150.0);
    a.time_scale().set_bar_spacing(4.0);
    let ea = frame(&mut a);
    assert!(
        sync_events(&ea)
            .iter()
            .any(|m| matches!(m, SyncMessage::VisibleRange { .. }))
    );
    assert_eq!(b.needs_redraw(), Invalidation::Light);
    let eb = frame(&mut b);
    assert!(sync_events(&eb).is_empty());
    let (fa, ta) = visible_times(&mut a);
    let (fb, tb) = visible_times(&mut b);
    // Within a millisecond (messages carry whole milliseconds) and the
    // spacing rounding of `b`.
    assert!((fa - fb).abs() < 1.0 + 1e-6 * fa.abs(), "{fa} {fb}");
    assert!((ta - tb).abs() < 1.0 + 1e-6 * ta.abs(), "{ta} {tb}");
    // 4 px per minute on `a` is 20 px per 5-minute bar, scaled by the ratio
    // of the plot widths.
    let ratio = f64::from(b.layout().plot.w / a.layout().plot.w);
    let spacing = b.time_scale().bar_spacing();
    assert!((spacing - 20.0 * ratio).abs() < 1e-3, "{spacing}");

    assert_eq!(a.needs_redraw(), Invalidation::None);
    assert_eq!(b.needs_redraw(), Invalidation::None);
}

#[test]
fn new_data_does_not_move_the_others() {
    let (mut a, mut b) = pair(SyncOptions::default());
    let before = b.time_scale().visible_range();
    let next = random_walk(9, 1, START + 1000 * MIN, MIN).unwrap();
    a.append(ferrochart_core::data::SeriesId(0), SeriesData::Ohlc(next))
        .unwrap();
    let ea = frame(&mut a);
    assert!(sync_events(&ea).is_empty());
    assert_eq!(b.needs_redraw(), Invalidation::None);
    assert_eq!(b.time_scale().visible_range(), before);
}

#[test]
fn options_select_what_is_shared() {
    let (mut a, mut b) = pair(SyncOptions {
        crosshair: false,
        visible_range: true,
        ..SyncOptions::default()
    });
    hover(&mut a, 0.5);
    frame(&mut a);
    assert_eq!(b.needs_redraw(), Invalidation::None);
    assert_eq!(crosshair(&frame(&mut b)), None);
    // `b` does not post its crosshair either.
    hover(&mut b, 0.5);
    assert!(sync_events(&frame(&mut b)).is_empty());
    assert_eq!(a.needs_redraw(), Invalidation::None);
}

#[test]
fn leaving_the_group_removes_the_crosshair() {
    let (mut a, mut b) = pair(SyncOptions::default());
    hover(&mut a, 0.5);
    frame(&mut a);
    frame(&mut b);
    a.leave_sync();
    assert_eq!(a.sync_options(), None);
    assert_eq!(crosshair(&frame(&mut b)), Some(None));
    // `a` keeps its own crosshair, and no longer posts.
    hover(&mut a, 0.6);
    assert!(sync_events(&frame(&mut a)).is_empty());
    assert_eq!(b.needs_redraw(), Invalidation::None);
}

#[test]
fn joining_takes_over_the_group_state() {
    let group = SyncGroup::new();
    let mut a = chart(1000, MIN, 800.0);
    a.join_sync(&group, SyncOptions::default());
    drag(&mut a, -100.0);
    hover(&mut a, 0.5);
    let t = crosshair(&frame(&mut a)).unwrap().unwrap().time;

    let mut c = chart(1000, MIN, 800.0);
    c.join_sync(&group, SyncOptions::default());
    let ec = frame(&mut c);
    assert_eq!(crosshair(&ec).unwrap().unwrap().time, t);
    assert_eq!(
        c.time_scale().visible_range(),
        a.time_scale().visible_range()
    );
    assert!(sync_events(&ec).is_empty());
}

#[test]
fn hosts_can_forward_messages() {
    // `a` and `b` in separate groups (e.g. separate workers); the host
    // forwards `a`'s messages to `b`.
    let mut a = chart(1000, MIN, 800.0);
    let mut b = chart(1000, MIN, 800.0);
    a.join_sync(&SyncGroup::new(), SyncOptions::default());
    drag(&mut a, 80.0);
    hover(&mut a, 0.25);
    let ea = frame(&mut a);
    let msgs = sync_events(&ea);
    assert_eq!(msgs.len(), 2);
    for m in msgs {
        b.apply_sync(m);
    }
    let eb = frame(&mut b);
    assert!(sync_events(&eb).is_empty());
    assert_eq!(
        crosshair(&eb).unwrap().unwrap().time,
        crosshair(&ea).unwrap().unwrap().time
    );
    let (ra, rb) = (
        a.time_scale().visible_range(),
        b.time_scale().visible_range(),
    );
    assert!((ra.0 - rb.0).abs() < 1e-6 && (ra.1 - rb.1).abs() < 1e-6);
}

#[test]
fn pending_events_are_coalesced() {
    let (mut a, _b) = pair(SyncOptions::default());
    for k in 0..10 {
        hover(&mut a, 0.1 + 0.05 * k as f32);
        a.build_frame(0.0, &MonoMeasurer);
    }
    let events: Vec<_> = a.drain_events().collect();
    assert_eq!(sync_events(&events).len(), 1);
}

const DRAWINGS: SyncOptions = SyncOptions {
    crosshair: false,
    visible_range: false,
    drawings: true,
};

fn line(t0: i64, t1: i64, price: f64) -> DrawingData {
    DrawingData::new(
        "trend_line",
        vec![
            Anchor {
                time: START + t0 * MIN,
                price,
            },
            Anchor {
                time: START + t1 * MIN,
                price: price + 1.0,
            },
        ],
    )
}

/// Saved forms of a chart's drawings, bottom to top, with visibility and lock.
fn drawings(chart: &mut Chart) -> Vec<(DrawingData, bool, bool)> {
    let d = chart.drawings();
    let ids: Vec<DrawingId> = d.ids().collect();
    ids.iter()
        .map(|&id| {
            (
                d.data(id).unwrap(),
                d.is_visible(id).unwrap(),
                d.is_locked(id).unwrap(),
            )
        })
        .collect()
}

fn only(chart: &mut Chart) -> DrawingId {
    let ids: Vec<_> = chart.drawings().ids().collect();
    assert_eq!(ids.len(), 1, "{ids:?}");
    ids[0]
}

/// Adding, moving, restyling, hiding, locking and removing a drawing
/// reach the other member; received changes are not undoable there, and
/// local undo is sent like any other change.
#[test]
fn drawings_follow_across_charts() {
    let group = SyncGroup::new();
    let mut a = chart(300, MIN, 800.0);
    let mut b = chart(300, MIN, 800.0);
    a.join_sync(&group, DRAWINGS);
    b.join_sync(&group, DRAWINGS);
    let pane = a.main_pane();
    let id = a.drawings().add(pane, &line(100, 200, 100.0)).unwrap();
    assert_eq!(
        b.needs_redraw(),
        Invalidation::None,
        "sent at a's next frame"
    );
    let ea = frame(&mut a);
    assert_eq!(
        ea.iter()
            .filter(|e| matches!(e, ChartEvent::DrawingSync))
            .count(),
        1
    );
    assert_eq!(b.needs_redraw(), Invalidation::Light);
    let eb = frame(&mut b);
    let rid = only(&mut b);
    assert!(eb.contains(&ChartEvent::DrawingCreated(rid)));
    assert_eq!(drawings(&mut b), drawings(&mut a));
    assert!(!b.drawings().can_undo());
    // Nothing comes back to `a`.
    assert_eq!(a.needs_redraw(), Invalidation::None);

    let moved = [
        Anchor {
            time: START + 110 * MIN,
            price: 99.0,
        },
        Anchor {
            time: START + 210 * MIN,
            price: 101.0,
        },
    ];
    a.drawings().set_anchors(id, &moved).unwrap();
    let style = DrawingStyle {
        line_width: ferrochart_core::geom::Css(3.0),
        ..DrawingStyle::default()
    };
    a.drawings().set_style(id, style).unwrap();
    a.drawings().set_visible(id, false).unwrap();
    a.drawings().set_locked(id, true).unwrap();
    frame(&mut a);
    let eb = frame(&mut b);
    assert!(eb.contains(&ChartEvent::DrawingModified(rid)));
    assert_eq!(drawings(&mut b), drawings(&mut a));
    assert_eq!(b.drawings().data(rid).unwrap().anchors, moved);
    assert_eq!(b.drawings().is_visible(rid), Some(false));
    assert_eq!(b.drawings().is_locked(rid), Some(true));

    // A change made on `b` goes back to `a`.
    b.drawings().set_visible(rid, true).unwrap();
    frame(&mut b);
    frame(&mut a);
    assert_eq!(a.drawings().is_visible(id), Some(true));

    a.drawings().remove(id).unwrap();
    frame(&mut a);
    let eb = frame(&mut b);
    assert!(eb.contains(&ChartEvent::DrawingRemoved(rid)));
    assert_eq!(b.drawings().ids().count(), 0);
    // Undoing the removal on `a` adds it back on `b`.
    assert!(a.drawings().undo());
    frame(&mut a);
    frame(&mut b);
    assert_eq!(drawings(&mut b), drawings(&mut a));
    assert_eq!(b.drawings().ids().count(), 1);
}

/// Joining takes over the group's drawings and shares the chart's own (of
/// the main pane only); members without `drawings` are left alone; a
/// drawing being created is sent once it is finished.
#[test]
fn joining_shares_existing_drawings() {
    let group = SyncGroup::new();
    let mut a = chart(300, MIN, 800.0);
    let mut b = chart(300, MIN, 800.0);
    let mut c = chart(300, MIN, 800.0);
    let pa = a.main_pane();
    a.drawings().add(pa, &line(10, 20, 100.0)).unwrap();
    let other = a.add_pane(PaneOptions::default());
    a.drawings().add(other, &line(30, 40, 50.0)).unwrap();
    let pb = b.main_pane();
    b.drawings().add(pb, &line(50, 60, 101.0)).unwrap();
    a.join_sync(&group, DRAWINGS);
    c.join_sync(&group, SyncOptions::default());
    b.join_sync(&group, DRAWINGS);
    // `b` took over `a`'s main-pane drawing at once, below its own.
    assert_eq!(b.drawings().ids().count(), 2);
    frame(&mut b);
    frame(&mut a);
    assert_eq!(a.drawings().ids().count(), 3);
    let main_a: Vec<_> = {
        let d = a.drawings();
        let ids: Vec<_> = d.ids().collect();
        ids.into_iter()
            .filter(|&id| d.pane_of(id) == Some(pa))
            .map(|id| d.data(id).unwrap())
            .collect()
    };
    let all_b: Vec<_> = drawings(&mut b).into_iter().map(|d| d.0).collect();
    assert_eq!(main_a.len(), 2);
    assert!(all_b.iter().all(|d| main_a.contains(d)));
    frame(&mut c);
    assert_eq!(c.drawings().ids().count(), 0);

    // Interactive creation: nothing is sent until the drawing is finished.
    b.drawings().start_tool("horizontal_line").unwrap();
    let plot = b.layout().plot;
    let at = PointCss {
        x: (plot.x + plot.w * 0.5) / DPR,
        y: (plot.y + plot.h * 0.5) / DPR,
    };
    for e in [
        InputEvent::PointerDown {
            id: 0,
            kind: ferrochart_core::interaction::PointerKind::Mouse,
            pos: at,
            button: Button::Left,
            mods: NONE,
            time_ms: 0.0,
        },
        InputEvent::PointerUp {
            id: 0,
            kind: ferrochart_core::interaction::PointerKind::Mouse,
            pos: at,
            button: Button::Left,
            mods: NONE,
            time_ms: 0.0,
        },
    ] {
        b.handle_input(e);
    }
    frame(&mut b);
    frame(&mut a);
    assert_eq!(a.drawings().ids().count(), 4);
}

/// A member that misses more operations than the queue holds is rebuilt
/// from the group's table.
#[test]
fn a_member_far_behind_is_rebuilt() {
    let group = SyncGroup::new();
    let mut a = chart(300, MIN, 800.0);
    let mut b = chart(300, MIN, 800.0);
    a.join_sync(&group, DRAWINGS);
    b.join_sync(&group, DRAWINGS);
    let pane = a.main_pane();
    let gone = a.drawings().add(pane, &line(1, 2, 90.0)).unwrap();
    frame(&mut a);
    frame(&mut b);
    assert_eq!(b.drawings().ids().count(), 1);
    a.drawings().remove(gone).unwrap();
    let id = a.drawings().add(pane, &line(5, 9, 95.0)).unwrap();
    for k in 0..1100 {
        a.drawings().set_locked(id, k % 2 == 0).unwrap();
        frame(&mut a);
    }
    assert_eq!(b.needs_redraw(), Invalidation::Light);
    frame(&mut b);
    assert_eq!(drawings(&mut b), drawings(&mut a));
}

/// Hosts forward operations between workers: `DrawingSync` is reported
/// once until taken, `take_drawing_sync_ops`/`apply_drawing_sync` and the
/// full state round-trip, and a bad state leaves the chart unchanged.
#[test]
fn hosts_can_forward_drawing_ops() {
    let mut a = chart(300, MIN, 800.0);
    let mut b = chart(300, MIN, 800.0);
    a.join_sync(&SyncGroup::with_origin(7), DRAWINGS);
    let pane = a.main_pane();
    let id = a.drawings().add(pane, &line(10, 20, 100.0)).unwrap();
    a.drawings().set_locked(id, true).unwrap();
    let events = frame(&mut a);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ChartEvent::DrawingSync))
            .count(),
        1
    );
    a.drawings().set_visible(id, false).unwrap();
    assert!(
        !frame(&mut a).contains(&ChartEvent::DrawingSync),
        "not taken yet"
    );
    let mut ops = Vec::new();
    a.take_drawing_sync_ops(&mut ops);
    assert_eq!(ops.len(), 3);
    assert_eq!(ops[0].key().0 >> 32, 7);
    for op in ops.drain(..) {
        b.apply_drawing_sync(op).unwrap();
    }
    assert_eq!(drawings(&mut b), drawings(&mut a));
    assert!(!b.drawings().can_undo());
    assert_eq!(
        b.apply_drawing_sync(DrawingSyncOp::Remove { key: SyncKey(1) }),
        Err(ferrochart_core::Error::UnknownId)
    );

    // Full state: a new chart catches up; stale synced drawings go away.
    let state = a.drawing_sync_state();
    assert_eq!(state.len(), 1);
    let mut c = chart(300, MIN, 800.0);
    c.load_drawing_sync_state(&state).unwrap();
    assert_eq!(drawings(&mut c), drawings(&mut a));
    b.load_drawing_sync_state(&[]).unwrap();
    assert_eq!(b.drawings().ids().count(), 0);
    let bad = [DrawingSyncOp::Remove {
        key: state[0].key(),
    }];
    assert!(c.load_drawing_sync_state(&bad).is_err());
    assert_eq!(c.drawings().ids().count(), 1);

    #[cfg(feature = "serde")]
    {
        let json = serde_json::to_string(&state).unwrap();
        let back: Vec<DrawingSyncOp> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, state);
    }
}

/// Leaving and joining the same group again keeps the keys: no duplicates,
/// and drawings removed by others meanwhile go away.
#[test]
fn rejoining_does_not_duplicate_drawings() {
    let group = SyncGroup::new();
    let mut a = chart(300, MIN, 800.0);
    let mut b = chart(300, MIN, 800.0);
    a.join_sync(&group, DRAWINGS);
    b.join_sync(&group, DRAWINGS);
    let pane = a.main_pane();
    let keep = a.drawings().add(pane, &line(10, 20, 100.0)).unwrap();
    let drop = a.drawings().add(pane, &line(30, 40, 100.0)).unwrap();
    frame(&mut a);
    frame(&mut b);
    b.leave_sync();
    a.drawings().remove(drop).unwrap();
    frame(&mut a);
    b.join_sync(&group, DRAWINGS);
    assert_eq!(drawings(&mut b), drawings(&mut a));
    frame(&mut b);
    frame(&mut a);
    assert_eq!(a.drawings().ids().collect::<Vec<_>>(), [keep]);
}
