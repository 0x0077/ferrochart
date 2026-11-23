//! Drawing tools through the chart:
//! creation by clicks and by dragging, selection, dragging bodies and
//! handles, keyboard shortcuts, undo/redo round trips, saved forms, and
//! anchors that follow time rather than logical indices.
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::chart::{
    ChartEvent, ClickTarget, Marker, MarkerPosition, MarkerShape, SeriesData, SeriesOptions,
};
use ferrochart_core::data::{DrawingId, OhlcColumns, SeriesId};
use ferrochart_core::drawing::{Anchor, BUILTIN_KINDS, DrawingData, DrawingStyle, Param};
use ferrochart_core::frame::{DrawCmd, LayerId, SeriesKind};
use ferrochart_core::geom::{PointCss, SizeCss};
use ferrochart_core::interaction::{Button, CursorIcon, InputEvent, Key, Modifiers};
use ferrochart_core::pane::PaneOptions;
use ferrochart_core::price_scale::ScaleSide;
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions, Error};

const START: i64 = 1_704_067_200_000;
const MIN: i64 = 60_000;
const DPR: f32 = 2.0;
const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
    meta: false,
};

fn data(n: usize) -> OhlcColumns {
    random_walk(11, n, START, MIN).unwrap()
}

fn chart() -> Chart {
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: 800.0, h: 400.0 }, DPR);
    let candles = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    chart
        .set_data(candles, SeriesData::Ohlc(data(300)))
        .unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    chart.drain_events().for_each(drop);
    chart
}

fn events(chart: &mut Chart) -> Vec<ChartEvent> {
    chart.build_frame(0.0, &MonoMeasurer);
    chart
        .drain_events()
        .filter(|e| {
            matches!(
                e,
                ChartEvent::DrawingCreated(_)
                    | ChartEvent::DrawingModified(_)
                    | ChartEvent::DrawingRemoved(_)
                    | ChartEvent::DrawingSelected(_)
            )
        })
        .collect()
}

/// A point of the main plot at fractions `(fx, fy)` of its size, in CSS px.
fn at(chart: &Chart, fx: f32, fy: f32) -> PointCss {
    let p = chart.layout().plot;
    PointCss {
        x: (p.x + p.w * fx) / DPR,
        y: (p.y + p.h * fy) / DPR,
    }
}

fn down(chart: &mut Chart, pos: PointCss) -> CursorIcon {
    chart
        .handle_input(InputEvent::PointerDown {
            id: 0,
            pos,
            button: Button::Left,
            mods: NONE,
            time_ms: 0.0,
        })
        .cursor
}

fn mv(chart: &mut Chart, pos: PointCss) -> CursorIcon {
    chart
        .handle_input(InputEvent::PointerMove {
            id: 0,
            pos,
            mods: NONE,
            time_ms: 0.0,
        })
        .cursor
}

fn up(chart: &mut Chart, pos: PointCss) {
    chart.handle_input(InputEvent::PointerUp {
        id: 0,
        pos,
        button: Button::Left,
        mods: NONE,
        time_ms: 0.0,
    });
}

fn click(chart: &mut Chart, pos: PointCss) {
    down(chart, pos);
    up(chart, pos);
}

fn key(chart: &mut Chart, key: Key, mods: Modifiers) -> bool {
    chart
        .handle_input(InputEvent::Key {
            key,
            pressed: true,
            mods,
        })
        .consumed
}

fn ctrl(shift: bool) -> Modifiers {
    Modifiers {
        ctrl: true,
        shift,
        ..NONE
    }
}

fn anchors(chart: &mut Chart, id: DrawingId) -> Vec<Anchor> {
    chart.drawings().data(id).unwrap().anchors
}

fn only_drawing(chart: &mut Chart) -> DrawingId {
    let ids: Vec<_> = chart.drawings().ids().collect();
    assert_eq!(ids.len(), 1);
    ids[0]
}

/// An armed tool creates a drawing with one click per anchor; the
/// drawing is selected and reported, and anchors snap to bar times.
#[test]
fn clicks_create_a_drawing() {
    let mut chart = chart();
    chart.drawings().start_tool("trend_line").unwrap();
    assert_eq!(chart.drawings().active_tool(), Some("trend_line"));
    let (a, b) = (at(&chart, 0.2, 0.3), at(&chart, 0.6, 0.7));
    click(&mut chart, a);
    assert!(events(&mut chart).is_empty(), "not created yet");
    mv(&mut chart, b);
    click(&mut chart, b);
    let id = only_drawing(&mut chart);
    assert_eq!(
        events(&mut chart),
        [
            ChartEvent::DrawingCreated(id),
            ChartEvent::DrawingSelected(Some(id))
        ]
    );
    assert_eq!(chart.drawings().active_tool(), None);
    let anchors = anchors(&mut chart, id);
    assert_eq!(anchors.len(), 2);
    assert!(anchors.iter().all(|a| (a.time - START) % MIN == 0));
    assert!(anchors[0].time < anchors[1].time);
    assert!(anchors[0].price > anchors[1].price);
    // Drawn: lines in the main layer, handles as icons.
    let cmds = &chart.frame().layers[LayerId::Main as usize].cmds;
    assert!(cmds.iter().any(|c| matches!(c, DrawCmd::Icon(_))));
}

/// Press, drag and release creates a two-anchor drawing in one gesture; a
/// one-anchor tool is created by a single click.
#[test]
fn dragging_creates_a_drawing() {
    let mut chart = chart();
    chart.drawings().start_tool("rectangle").unwrap();
    let (a, b) = (at(&chart, 0.3, 0.3), at(&chart, 0.5, 0.6));
    down(&mut chart, a);
    mv(&mut chart, b);
    up(&mut chart, b);
    let rect = only_drawing(&mut chart);
    let r = anchors(&mut chart, rect);
    assert_ne!(r[0], r[1]);
    chart.drawings().start_tool("horizontal_line").unwrap();
    let p = at(&chart, 0.5, 0.5);
    click(&mut chart, p);
    assert_eq!(chart.drawings().ids().count(), 2);
    assert_eq!(chart.drawings().active_tool(), None);
}

/// Escape drops a drawing being created; nothing is recorded.
#[test]
fn escape_cancels_creation() {
    let mut chart = chart();
    chart.drawings().start_tool("fib_extension").unwrap();
    let p = at(&chart, 0.2, 0.2);
    click(&mut chart, p);
    let p = at(&chart, 0.4, 0.6);
    click(&mut chart, p);
    assert_eq!(chart.drawings().ids().count(), 1);
    assert!(key(&mut chart, Key::Escape, NONE));
    assert_eq!(chart.drawings().ids().count(), 0);
    assert!(!chart.drawings().can_undo());
    assert!(events(&mut chart).is_empty());
}

fn with_line(chart: &mut Chart) -> DrawingId {
    let pane = chart.main_pane();
    let line = DrawingData::new(
        "trend_line",
        vec![
            Anchor {
                time: START + 200 * MIN,
                price: 100.0,
            },
            Anchor {
                time: START + 260 * MIN,
                price: 100.0,
            },
        ],
    );
    let id = chart.drawings().add(pane, &line).unwrap();
    chart.time_scale().fit_content();
    chart
        .price_scale(pane, ScaleSide::Right)
        .unwrap()
        .set_range(90.0, 110.0)
        .unwrap();
    events(chart);
    id
}

/// Screen position (CSS px) of a time and price on the main pane.
fn screen(chart: &mut Chart, time: i64, price: f64) -> PointCss {
    let plot = chart.layout().plot;
    let j = (time - START) / MIN;
    let x = plot.x + chart.time_scale().index_to_x(j as f64).0;
    let (lo, hi) = chart
        .price_scale(chart.main_pane(), ScaleSide::Right)
        .unwrap()
        .range();
    // Default margins 10% top and bottom.
    let usable = plot.h * 0.8;
    let y = plot.y + plot.h * 0.1 + usable * (1.0 - ((price - lo) / (hi - lo)) as f32);
    PointCss {
        x: x / DPR,
        y: y / DPR,
    }
}

/// A press on a drawing selects it; dragging moves it by whole bars;
/// the move is undoable and redoable; a click elsewhere deselects.
#[test]
fn dragging_moves_and_undo_restores() {
    let mut chart = chart();
    let id = with_line(&mut chart);
    let before = anchors(&mut chart, id);
    let on = screen(&mut chart, START + 230 * MIN, 100.0);
    assert_eq!(mv(&mut chart, on), CursorIcon::Pointer);
    assert_eq!(down(&mut chart, on), CursorIcon::Move);
    assert_eq!(chart.drawings().selected(), Some(id));
    let spacing = chart.time_scale().bar_spacing() as f32 / DPR;
    let to = PointCss {
        x: on.x + 10.0 * spacing,
        y: on.y - 30.0,
    };
    mv(&mut chart, to);
    up(&mut chart, to);
    let after = anchors(&mut chart, id);
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(b.time - a.time, 10 * MIN);
        assert!(b.price > a.price);
    }
    assert_eq!(
        events(&mut chart),
        [
            ChartEvent::DrawingSelected(Some(id)),
            ChartEvent::DrawingModified(id)
        ]
    );
    assert!(key(&mut chart, Key::Char('z'), ctrl(false)));
    assert_eq!(anchors(&mut chart, id), before);
    assert!(key(&mut chart, Key::Char('z'), ctrl(true)));
    assert_eq!(anchors(&mut chart, id), after);
    assert!(key(&mut chart, Key::Char('z'), ctrl(false)));
    assert!(key(&mut chart, Key::Char('y'), ctrl(false)));
    assert_eq!(anchors(&mut chart, id), after);
    // A click on empty space deselects.
    let p = at(&chart, 0.05, 0.05);
    click(&mut chart, p);
    assert_eq!(chart.drawings().selected(), None);
}

/// Dragging a handle moves that anchor only; Escape during a drag restores.
#[test]
fn handles_move_single_anchors() {
    let mut chart = chart();
    let id = with_line(&mut chart);
    chart.drawings().select(Some(id)).unwrap();
    events(&mut chart);
    let before = anchors(&mut chart, id);
    let h = screen(&mut chart, before[1].time, before[1].price);
    assert_eq!(mv(&mut chart, h), CursorIcon::Move);
    down(&mut chart, h);
    let to = screen(&mut chart, START + 280 * MIN, 105.0);
    mv(&mut chart, to);
    up(&mut chart, to);
    let after = anchors(&mut chart, id);
    assert_eq!(after[0], before[0]);
    assert_eq!(after[1].time, START + 280 * MIN);
    assert!((after[1].price - 105.0).abs() < 0.1);
    // Escape in the middle of a drag puts the drawing back.
    down(&mut chart, to);
    mv(&mut chart, h);
    assert!(key(&mut chart, Key::Escape, NONE));
    assert_eq!(anchors(&mut chart, id), after);
}

/// Locked drawings are selected but not dragged (the press pans instead);
/// hidden drawings are not hit.
#[test]
fn locked_and_hidden_drawings() {
    let mut chart = chart();
    let id = with_line(&mut chart);
    chart.drawings().set_locked(id, true).unwrap();
    let before = anchors(&mut chart, id);
    let on = screen(&mut chart, START + 230 * MIN, 100.0);
    down(&mut chart, on);
    let to = PointCss {
        x: on.x + 40.0,
        y: on.y,
    };
    mv(&mut chart, to);
    up(&mut chart, to);
    assert_eq!(chart.drawings().selected(), Some(id));
    assert_eq!(anchors(&mut chart, id), before);
    chart.drawings().set_visible(id, false).unwrap();
    assert_eq!(chart.drawings().selected(), None);
    events(&mut chart);
    let on = screen(&mut chart, START + 230 * MIN, 100.0);
    assert_eq!(mv(&mut chart, on), CursorIcon::Crosshair);
}

/// Delete removes the selection; undo brings it back with the same id.
#[test]
fn delete_and_undo() {
    let mut chart = chart();
    let id = with_line(&mut chart);
    assert!(!key(&mut chart, Key::Delete, NONE), "nothing selected");
    chart.drawings().select(Some(id)).unwrap();
    assert!(key(&mut chart, Key::Delete, NONE));
    assert_eq!(chart.drawings().ids().count(), 0);
    assert_eq!(
        events(&mut chart),
        [
            ChartEvent::DrawingSelected(Some(id)),
            ChartEvent::DrawingSelected(None),
            ChartEvent::DrawingRemoved(id)
        ]
    );
    assert!(chart.drawings().undo());
    assert_eq!(only_drawing(&mut chart), id);
    assert!(chart.drawings().redo());
    assert_eq!(chart.drawings().ids().count(), 0);
    assert!(!chart.drawings().redo());
}

/// Undo/redo round trips of every command, and saved forms.
#[test]
fn undo_redo_round_trips() {
    let mut chart = chart();
    let id = with_line(&mut chart);
    let pane = chart.main_pane();
    let saved = chart.drawings().data(id).unwrap();
    let style = DrawingStyle {
        line_width: ferrochart_core::geom::Css(3.0),
        ..DrawingStyle::default()
    };
    chart.drawings().set_style(id, style).unwrap();
    let text = chart
        .drawings()
        .add(
            pane,
            &DrawingData::new(
                "text",
                vec![Anchor {
                    time: START,
                    price: 100.0,
                }],
            ),
        )
        .unwrap();
    chart
        .drawings()
        .set_param(text, "text", Param::Text("note".into()))
        .unwrap();
    assert_eq!(
        chart.drawings().data(text).unwrap().text("text"),
        Some("note")
    );
    assert!(
        chart
            .drawings()
            .set_param(text, "text", Param::Number(1.0))
            .is_err()
    );
    let moved = [
        Anchor {
            time: START,
            price: 1.0,
        },
        Anchor {
            time: START + MIN,
            price: 2.0,
        },
    ];
    chart.drawings().set_anchors(id, &moved).unwrap();
    chart.drawings().remove(text).unwrap();
    let full = (
        chart.drawings().data(id),
        chart.drawings().ids().collect::<Vec<_>>(),
    );
    // Back to the start…
    while chart.drawings().undo() {}
    assert_eq!(chart.drawings().ids().count(), 0);
    // …and forward again.
    let mut steps = 0;
    while chart.drawings().redo() {
        steps += 1;
        if steps == 1 {
            assert_eq!(chart.drawings().data(id).as_ref(), Some(&saved));
        }
    }
    assert_eq!(steps, 6);
    assert_eq!(
        (
            chart.drawings().data(id),
            chart.drawings().ids().collect::<Vec<_>>()
        ),
        full
    );
    // A new change clears the redo stack.
    chart.drawings().undo();
    chart
        .drawings()
        .set_style(id, DrawingStyle::default())
        .unwrap();
    assert!(!chart.drawings().can_redo());
}

/// Every built-in tool draws, can be hit where it draws, and saves to a form
/// that rebuilds the same drawing.
#[test]
fn every_tool_draws_and_is_hit() {
    let mut chart = chart();
    let pane = chart.main_pane();
    for kind in BUILTIN_KINDS {
        let n: i64 = match kind {
            "horizontal_line" | "horizontal_ray" | "vertical_line" | "text" => 1,
            "parallel_channel"
            | "fib_extension"
            | "pitchfork"
            | "schiff_pitchfork"
            | "modified_schiff_pitchfork" => 3,
            "elliott_correction" => 4,
            "xabcd_pattern" => 5,
            "elliott_impulse" => 6,
            "head_and_shoulders" => 7,
            _ => 2,
        };
        // Up to 60 bars wide; patterns zigzag.
        let step = if n > 3 { 60 / n } else { 30 };
        let anchors = (0..n)
            .map(|k| Anchor {
                time: START + (200 + step * k) * MIN,
                price: if n > 3 {
                    95.0 + 8.0 * (k % 2) as f64 + k as f64
                } else {
                    95.0 + 5.0 * k as f64
                },
            })
            .collect();
        let id = chart
            .drawings()
            .add(pane, &DrawingData::new(kind, anchors))
            .unwrap();
        let saved = chart.drawings().data(id).unwrap();
        chart.drawings().remove(id).unwrap();
        let again = chart.drawings().add(pane, &saved).unwrap();
        assert_eq!(chart.drawings().data(again).unwrap(), saved, "{kind}");
    }
    chart.time_scale().fit_content();
    chart
        .price_scale(pane, ScaleSide::Right)
        .unwrap()
        .set_range(80.0, 120.0)
        .unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    let ids: Vec<_> = chart.drawings().ids().collect();
    assert_eq!(ids.len(), BUILTIN_KINDS.len());
    // Show only one drawing at a time and hover over its first anchor
    // (every tool draws something there or next to it).
    for &id in &ids {
        for &other in &ids {
            chart.drawings().set_visible(other, other == id).unwrap();
        }
        chart.build_frame(0.0, &MonoMeasurer);
        let a = chart.drawings().data(id).unwrap();
        let kind = a.kind.clone();
        let p = match kind.as_str() {
            // Hit the line at mid-plot rather than off-screen x.
            "horizontal_line" | "vertical_line" => {
                let s = screen(&mut chart, a.anchors[0].time, a.anchors[0].price);
                let mid = at(&chart, 0.5, 0.5);
                if kind == "horizontal_line" {
                    PointCss { x: mid.x, y: s.y }
                } else {
                    PointCss { x: s.x, y: mid.y }
                }
            }
            "text" => {
                let s = screen(&mut chart, a.anchors[0].time, a.anchors[0].price);
                PointCss {
                    x: s.x + 3.0,
                    y: s.y,
                }
            }
            "fib_retracement" | "fib_extension" | "price_range" | "date_range" | "rectangle"
            | "ellipse" | "long_position" | "short_position" => {
                // Inside, between the first two anchors.
                let (s, t) = (
                    screen(&mut chart, a.anchors[0].time, a.anchors[0].price),
                    screen(&mut chart, a.anchors[1].time, a.anchors[1].price),
                );
                let i = if kind == "fib_extension" { 2 } else { 0 };
                let c = screen(&mut chart, a.anchors[i].time, a.anchors[i].price);
                // The extension's levels start at the third anchor's price.
                let y = if kind == "fib_extension" {
                    let mid = a.anchors[2].price + (a.anchors[1].price - a.anchors[0].price) * 0.5;
                    screen(&mut chart, a.anchors[2].time, mid).y
                } else {
                    (s.y + t.y) * 0.5
                };
                PointCss {
                    x: c.x + (t.x - s.x).abs() * 0.25,
                    y,
                }
            }
            _ => screen(&mut chart, a.anchors[0].time, a.anchors[0].price),
        };
        assert_eq!(mv(&mut chart, p), CursorIcon::Pointer, "{kind}");
    }
}

/// Anchors are times; prepending history keeps drawings in place.
#[test]
fn anchors_follow_time_through_prepend() {
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: 800.0, h: 400.0 }, DPR);
    let pane = chart.main_pane();
    let candles = chart
        .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    let all = data(400);
    let tail = OhlcColumns {
        time: all.time[100..].to_vec(),
        open: all.open[100..].to_vec(),
        high: all.high[100..].to_vec(),
        low: all.low[100..].to_vec(),
        close: all.close[100..].to_vec(),
        volume: None,
    };
    let head = OhlcColumns {
        time: all.time[..100].to_vec(),
        open: all.open[..100].to_vec(),
        high: all.high[..100].to_vec(),
        low: all.low[..100].to_vec(),
        close: all.close[..100].to_vec(),
        volume: None,
    };
    chart.set_data(candles, SeriesData::Ohlc(tail)).unwrap();
    let t = START + 350 * MIN;
    let v = chart
        .drawings()
        .add(
            pane,
            &DrawingData::new(
                "vertical_line",
                vec![Anchor {
                    time: t,
                    price: 100.0,
                }],
            ),
        )
        .unwrap();
    let x_of = |chart: &mut Chart| {
        chart.build_frame(0.0, &MonoMeasurer);
        let f = chart.frame();
        f.pools
            .lines
            .iter()
            .find(|l| l.from.x == l.to.x && l.to.y - l.from.y > 100.0)
            .map(|l| l.from.x)
    };
    let x = x_of(&mut chart).unwrap();
    chart.prepend(candles, SeriesData::Ohlc(head)).unwrap();
    assert_eq!(x_of(&mut chart), Some(x));
    assert_eq!(anchors(&mut chart, v)[0].time, t);
    // Times past the data are extrapolated with the bar spacing.
    let future = chart
        .drawings()
        .add(
            pane,
            &DrawingData::new(
                "vertical_line",
                vec![Anchor {
                    time: START + 410 * MIN,
                    price: 100.0,
                }],
            ),
        )
        .unwrap();
    assert!(chart.drawings().get(future).is_some());
}

/// Errors and the removal of a pane's drawings.
#[test]
fn ids_panes_and_errors() {
    let mut chart = chart();
    let lower = chart.add_pane(PaneOptions::default());
    let a = Anchor {
        time: START,
        price: 1.0,
    };
    assert_eq!(
        chart.drawings().add(
            ferrochart_core::data::PaneId(99),
            &DrawingData::new("text", vec![a])
        ),
        Err(Error::UnknownId)
    );
    assert!(matches!(
        chart
            .drawings()
            .add(lower, &DrawingData::new("nope", vec![a])),
        Err(Error::InvalidArgument(_))
    ));
    assert!(matches!(
        chart
            .drawings()
            .add(lower, &DrawingData::new("trend_line", vec![a])),
        Err(Error::InvalidArgument(_))
    ));
    let nan = Anchor {
        time: START,
        price: f64::NAN,
    };
    assert!(
        chart
            .drawings()
            .add(lower, &DrawingData::new("text", vec![nan]))
            .is_err()
    );
    assert!(chart.drawings().start_tool("nope").is_err());
    let id = chart
        .drawings()
        .add(lower, &DrawingData::new("text", vec![a]))
        .unwrap();
    assert_eq!(chart.drawings().pane_of(id), Some(lower));
    events(&mut chart);
    chart.remove_pane(lower).unwrap();
    assert!(chart.drawings().get(id).is_none());
    assert_eq!(events(&mut chart), [ChartEvent::DrawingRemoved(id)]);
    assert!(!chart.drawings().can_undo());
    let bogus = DrawingId(12345);
    assert_eq!(chart.drawings().remove(bogus), Err(Error::UnknownId));
    assert_eq!(chart.drawings().select(Some(bogus)), Err(Error::UnknownId));
    assert_eq!(
        chart.drawings().set_anchors(bogus, &[a]),
        Err(Error::UnknownId)
    );
}

/// Click events of the last frame.
fn clicks(chart: &mut Chart) -> Vec<ChartEvent> {
    chart.build_frame(0.0, &MonoMeasurer);
    chart
        .drain_events()
        .filter(|e| matches!(e, ChartEvent::Click { .. }))
        .collect()
}

fn click_target(chart: &mut Chart, pos: PointCss) -> Option<ClickTarget> {
    click(chart, pos);
    let events = clicks(chart);
    assert_eq!(events.len(), 1, "{events:?}");
    let ChartEvent::Click { target, .. } = events[0] else {
        unreachable!()
    };
    target
}

/// A press and release reports a click with its position, bar time and
/// price; what was hit follows the priority order (drawing, marker,
/// series).
#[test]
fn clicks_report_position_and_target() {
    let mut chart = chart();
    let candles = SeriesId(0);
    let line = with_line(&mut chart);
    let pane = chart.main_pane();

    // Empty space above everything.
    let p = screen(&mut chart, START + 100 * MIN, 109.0);
    click(&mut chart, p);
    let events = clicks(&mut chart);
    let [
        ChartEvent::Click {
            pos,
            pane: clicked,
            logical_index,
            time,
            price,
            target,
        },
    ] = events[..]
    else {
        panic!("{events:?}")
    };
    assert_eq!((pos, clicked, target), (p, pane, None));
    assert!((logical_index - 100.0).abs() < 0.5, "{logical_index}");
    assert_eq!(time, Some(START + 100 * MIN));
    assert!((price - 109.0).abs() < 0.05, "{price}");

    // On the drawing (selected by the press): the drawing.
    let p = screen(&mut chart, START + 230 * MIN, 100.0);
    assert_eq!(
        click_target(&mut chart, p),
        Some(ClickTarget::Drawing(line))
    );
    // A locked drawing is still a click target.
    chart.drawings().set_locked(line, true).unwrap();
    assert_eq!(
        click_target(&mut chart, p),
        Some(ClickTarget::Drawing(line))
    );
    chart.drawings().remove(line).unwrap();

    // On a candle body, halfway between open and close.
    let bars = data(300);
    let k = 120;
    let mid = (bars.open[k] + bars.close[k]) * 0.5;
    chart
        .price_scale(pane, ScaleSide::Right)
        .unwrap()
        .set_range(mid - 5.0, mid + 5.0)
        .unwrap();
    let p = screen(&mut chart, START + k as i64 * MIN, mid);
    assert_eq!(
        click_target(&mut chart, p),
        Some(ClickTarget::Series(candles))
    );

    // On a marker drawn above a bar.
    chart
        .set_markers(
            candles,
            vec![Marker {
                time: START + 140 * MIN,
                position: MarkerPosition::AboveBar,
                shape: MarkerShape::Circle,
                color: None,
                text: String::new(),
                ..Marker::default()
            }],
        )
        .unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    // The only icon of the frame: no drawings are left.
    assert_eq!(chart.frame().pools.icons.len(), 1);
    let icon = chart.frame().pools.icons[0].pos;
    let p = PointCss {
        x: icon.x / DPR,
        y: icon.y / DPR,
    };
    assert_eq!(
        click_target(&mut chart, p),
        Some(ClickTarget::Marker {
            series: candles,
            index: 0,
            id: 0,
        })
    );
}

/// A drag, or a click that creates a drawing, is not a click.
#[test]
fn drags_and_creation_are_not_clicks() {
    let mut chart = chart();
    let (a, b) = (at(&chart, 0.3, 0.3), at(&chart, 0.5, 0.6));
    down(&mut chart, a);
    mv(&mut chart, b);
    up(&mut chart, b);
    assert!(clicks(&mut chart).is_empty());
    // Within the 3 CSS px threshold: still a click.
    down(&mut chart, a);
    let near = PointCss {
        x: a.x + 2.0,
        y: a.y,
    };
    mv(&mut chart, near);
    up(&mut chart, near);
    assert_eq!(clicks(&mut chart).len(), 1);

    chart.drawings().start_tool("trend_line").unwrap();
    click(&mut chart, a);
    mv(&mut chart, b);
    click(&mut chart, b);
    assert_eq!(chart.drawings().ids().count(), 1);
    assert!(clicks(&mut chart).is_empty());
}

fn double_click(chart: &mut Chart, pos: PointCss) {
    click(chart, pos);
    chart.handle_input(InputEvent::DoubleClick {
        pos,
        button: Button::Left,
    });
}

/// A path takes one anchor per click until a double click (or Enter);
/// the double click's second click does not add an anchor. Escape finishes
/// it with enough anchors and drops it otherwise.
#[test]
fn open_drawings_end_on_double_click_enter_or_escape() {
    let mut chart = chart();
    chart.drawings().start_tool("path").unwrap();
    let pts = [
        at(&chart, 0.2, 0.2),
        at(&chart, 0.4, 0.6),
        at(&chart, 0.6, 0.3),
    ];
    for &p in &pts[..2] {
        mv(&mut chart, p);
        click(&mut chart, p);
    }
    mv(&mut chart, pts[2]);
    double_click(&mut chart, pts[2]);
    let id = only_drawing(&mut chart);
    assert_eq!(anchors(&mut chart, id).len(), 3);
    assert_eq!(chart.drawings().active_tool(), None);
    assert!(events(&mut chart).contains(&ChartEvent::DrawingCreated(id)));

    // Enter: the anchor following the pointer is dropped.
    chart.drawings().start_tool("path").unwrap();
    click(&mut chart, pts[0]);
    mv(&mut chart, pts[1]);
    click(&mut chart, pts[1]);
    mv(&mut chart, pts[2]);
    assert!(key(&mut chart, Key::Enter, NONE));
    assert_eq!(chart.drawings().ids().count(), 2);
    let last = chart.drawings().ids().last().unwrap();
    assert_eq!(anchors(&mut chart, last).len(), 2);

    // Escape with one anchor: dropped; with two: finished.
    chart.drawings().start_tool("path").unwrap();
    click(&mut chart, pts[0]);
    assert!(!key(&mut chart, Key::Enter, NONE), "too few anchors");
    assert!(key(&mut chart, Key::Escape, NONE));
    assert_eq!(chart.drawings().ids().count(), 2);
    chart.drawings().start_tool("path").unwrap();
    click(&mut chart, pts[0]);
    mv(&mut chart, pts[2]);
    click(&mut chart, pts[2]);
    assert!(key(&mut chart, Key::Escape, NONE));
    assert_eq!(chart.drawings().ids().count(), 3);
}

/// A brush is drawn by dragging: an anchor every 3 CSS px, not
/// snapped to bars; a click without moving draws nothing.
#[test]
fn freehand_drawings_follow_the_drag() {
    let mut chart = chart();
    chart.drawings().start_tool("brush").unwrap();
    let a = at(&chart, 0.3, 0.5);
    down(&mut chart, a);
    for k in 1..=40 {
        let p = PointCss {
            x: a.x + k as f32 * 1.5,
            y: a.y + (k as f32 * 0.3).sin() * 20.0,
        };
        mv(&mut chart, p);
    }
    let end = PointCss {
        x: a.x + 60.0,
        y: a.y,
    };
    up(&mut chart, end);
    let id = only_drawing(&mut chart);
    let pts = anchors(&mut chart, id);
    assert!((10..=41).contains(&pts.len()), "{}", pts.len());
    assert!(
        pts.iter().any(|a| (a.time - START) % MIN != 0),
        "not snapped"
    );
    assert_eq!(chart.drawings().active_tool(), None);
    // The brush is selected but shows no handles; it moves as a whole.
    assert_eq!(chart.drawings().selected(), Some(id));

    chart.drawings().start_tool("brush").unwrap();
    let p = at(&chart, 0.7, 0.7);
    click(&mut chart, p);
    assert_eq!(chart.drawings().ids().count(), 1);
}

/// r11: tools with more than 4 handles; each handle moves its own anchor.
#[test]
fn many_handles_move_their_anchors() {
    let mut chart = chart();
    let pane = chart.main_pane();
    let hs: Vec<Anchor> = (0..7)
        .map(|k| Anchor {
            time: START + (200 + 8 * k) * MIN,
            price: if k % 2 == 1 { 106.0 } else { 98.0 } + if k == 3 { 3.0 } else { 0.0 },
        })
        .collect();
    let id = chart
        .drawings()
        .add(pane, &DrawingData::new("head_and_shoulders", hs.clone()))
        .unwrap();
    chart.time_scale().fit_content();
    chart
        .price_scale(pane, ScaleSide::Right)
        .unwrap()
        .set_range(90.0, 115.0)
        .unwrap();
    chart.drawings().select(Some(id)).unwrap();
    events(&mut chart);
    let from = screen(&mut chart, hs[6].time, hs[6].price);
    let to = PointCss {
        x: from.x,
        y: from.y - 30.0,
    };
    down(&mut chart, from);
    mv(&mut chart, to);
    up(&mut chart, to);
    let after = anchors(&mut chart, id);
    assert_eq!(after[..6], hs[..6]);
    assert!(after[6].price > hs[6].price);
}
