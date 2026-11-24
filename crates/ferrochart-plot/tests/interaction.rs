//! Plot interaction: hit testing, hover and click events, pan, zoom,
//! kinetic panning, category scrolling.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp
)]

use ferrochart_core::geom::{Css, PointCss, Px, SizeCss};
use ferrochart_core::interaction::{Button, InputEvent, Modifiers, WheelDelta};
use ferrochart_core::testing::MonoMeasurer;
use ferrochart_plot::{
    AxisId, AxisOptions, AxisRange, Hit, MarkData, MarkKind, MarkOptions, MarkPoint, Plot,
    PlotEvent, PlotOptions,
};
use proptest::prelude::*;

const W: f32 = 600.0;
const H: f32 = 400.0;

fn plot() -> Plot {
    let mut p = Plot::new(PlotOptions::default());
    p.resize(SizeCss { w: W, h: H }, 1.0);
    p
}

fn fixed(lo: f64, hi: f64) -> AxisOptions {
    AxisOptions {
        range: AxisRange::Fixed(lo, hi),
        ..AxisOptions::linear()
    }
}

fn frame(p: &mut Plot, now: f64) {
    p.build_frame(now, &MonoMeasurer);
}

/// CSS position of data point (x, y) on the right axis.
fn at(p: &Plot, x: f64, y: f64) -> PointCss {
    PointCss {
        x: p.value_to_px(AxisId::X, x).unwrap().0,
        y: p.value_to_px(AxisId::Right, y).unwrap().0,
    }
}

fn mv(p: &mut Plot, pos: PointCss, t: f64) {
    p.handle_input(InputEvent::PointerMove {
        id: 0,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos,
        mods: Modifiers::default(),
        time_ms: t,
    });
}

fn down(p: &mut Plot, pos: PointCss, t: f64) {
    p.handle_input(InputEvent::PointerDown {
        id: 0,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos,
        button: Button::Left,
        mods: Modifiers::default(),
        time_ms: t,
    });
}

fn up(p: &mut Plot, pos: PointCss, t: f64) {
    p.handle_input(InputEvent::PointerUp {
        id: 0,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos,
        button: Button::Left,
        mods: Modifiers::default(),
        time_ms: t,
    });
}

fn events(p: &mut Plot) -> Vec<PlotEvent> {
    p.drain_events().collect()
}

/// `None`: no Hover event; `Some(h)`: the last one.
#[allow(clippy::option_option)]
fn hovered(p: &mut Plot) -> Option<Option<Hit>> {
    events(p).into_iter().rev().find_map(|e| match e {
        PlotEvent::Hover(h) => Some(h),
        _ => None,
    })
}

#[test]
fn hover_scatter_points_and_leave() {
    let mut p = plot();
    p.set_axis(AxisId::X, fixed(0.0, 10.0)).unwrap();
    p.set_axis(AxisId::Right, fixed(0.0, 10.0)).unwrap();
    let m = p
        .add_mark(
            MarkKind::Scatter,
            MarkOptions {
                point_size: Css(10.0),
                ..MarkOptions::default()
            },
        )
        .unwrap();
    p.set_data(m, MarkData::xy(vec![2.0, 5.0, 8.0], vec![2.0, 5.0, 8.0]))
        .unwrap();
    frame(&mut p, 0.0);
    let pos = at(&p, 5.0, 5.0);
    mv(
        &mut p,
        PointCss {
            x: pos.x + 3.0,
            y: pos.y,
        },
        1.0,
    );
    let h = hovered(&mut p).unwrap().unwrap();
    assert_eq!((h.mark, h.index, h.x, h.y), (m, 1, 5.0, 5.0));
    assert!((h.pos.x - pos.x).abs() < 0.5 && (h.pos.y - pos.y).abs() < 0.5);
    // Same element: no new event.
    mv(
        &mut p,
        PointCss {
            x: pos.x - 2.0,
            y: pos.y,
        },
        2.0,
    );
    assert_eq!(hovered(&mut p), None);
    // Far from every point.
    let far = at(&p, 6.5, 2.0);
    mv(&mut p, far, 3.0);
    assert_eq!(hovered(&mut p), Some(None));
    mv(&mut p, pos, 4.0);
    assert!(hovered(&mut p).unwrap().is_some());
    p.handle_input(InputEvent::PointerLeave);
    assert_eq!(hovered(&mut p), Some(None));
    // The overlay draws the highlight while hovered.
    mv(&mut p, pos, 5.0);
    frame(&mut p, 5.0);
    assert!(!p.frame().layers[2].cmds.is_empty());
}

#[test]
fn hover_line_takes_the_nearest_x() {
    let mut p = plot();
    let m = p.add_mark(MarkKind::Line, MarkOptions::default()).unwrap();
    let x: Vec<f64> = (0..50).map(f64::from).collect();
    let mut y: Vec<f64> = x.iter().map(|v| v.sin()).collect();
    y[21] = f64::NAN;
    p.set_data(m, MarkData::xy(x, y)).unwrap();
    frame(&mut p, 0.0);
    let pos = at(&p, 30.2, 0.0);
    mv(&mut p, PointCss { x: pos.x, y: 20.0 }, 1.0);
    assert_eq!(hovered(&mut p).unwrap().unwrap().index, 30);
    // The gap is skipped.
    let pos = at(&p, 21.1, 0.0);
    mv(&mut p, PointCss { x: pos.x, y: pos.y }, 2.0);
    let i = hovered(&mut p).unwrap().unwrap().index;
    assert!(i == 20 || i == 22, "{i}");
}

#[test]
fn hover_bars_cells_and_waterfall() {
    let mut p = plot();
    p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
    let a = p
        .add_mark(
            MarkKind::Bars,
            MarkOptions {
                group: Some(1),
                ..MarkOptions::default()
            },
        )
        .unwrap();
    let b = p
        .add_mark(
            MarkKind::Bars,
            MarkOptions {
                group: Some(1),
                ..MarkOptions::default()
            },
        )
        .unwrap();
    p.set_data(a, MarkData::xy(vec![0.0, 1.0, 2.0], vec![5.0, 6.0, 7.0]))
        .unwrap();
    p.set_data(b, MarkData::xy(vec![0.0, 1.0, 2.0], vec![3.0, 4.0, 2.0]))
        .unwrap();
    frame(&mut p, 0.0);
    // Category 1 spans x in [1, 2); the left half belongs to `a`.
    let y = p.value_to_px(AxisId::Right, 1.0).unwrap().0;
    let x = p.value_to_px(AxisId::X, 1.3).unwrap().0;
    mv(&mut p, PointCss { x, y }, 1.0);
    let h = hovered(&mut p).unwrap().unwrap();
    assert_eq!((h.mark, h.index, h.y), (a, 1, 6.0));
    let x = p.value_to_px(AxisId::X, 1.7).unwrap().0;
    mv(&mut p, PointCss { x, y }, 2.0);
    assert_eq!(hovered(&mut p).unwrap().unwrap().mark, b);
    // Above bar `b` of category 1 (4) but below the top of the plot.
    let y = p.value_to_px(AxisId::Right, 5.0).unwrap().0;
    mv(&mut p, PointCss { x, y }, 3.0);
    assert_eq!(hovered(&mut p), Some(None));

    let mut p = plot();
    p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
    p.set_axis(AxisId::Right, AxisOptions::category()).unwrap();
    let c = p.add_mark(MarkKind::Cells, MarkOptions::default()).unwrap();
    let (mut xs, mut ys, mut vs) = (vec![], vec![], vec![]);
    for r in 0..20 {
        for k in 0..30 {
            xs.push(f64::from(k));
            ys.push(f64::from(r));
            vs.push(f64::from(k * r));
        }
    }
    vs[7 * 30 + 12] = f64::NAN;
    let mut d = MarkData::xy(xs, ys);
    d.value = Some(vs);
    p.set_data(c, d).unwrap();
    frame(&mut p, 0.0);
    let cell = |p: &Plot, k: f64, r: f64| PointCss {
        x: p.value_to_px(AxisId::X, k + 0.5).unwrap().0,
        y: p.value_to_px(AxisId::Right, r + 0.5).unwrap().0,
    };
    let q = cell(&p, 17.0, 4.0);
    mv(&mut p, q, 1.0);
    let h = hovered(&mut p).unwrap().unwrap();
    assert_eq!((h.index, h.x, h.y), (4 * 30 + 17, 17.0, 4.0));
    // The last cell (outside the index) and a missing cell.
    let q = cell(&p, 29.0, 19.0);
    mv(&mut p, q, 2.0);
    assert_eq!(hovered(&mut p).unwrap().unwrap().index, 599);
    let q = cell(&p, 12.0, 7.0);
    mv(&mut p, q, 3.0);
    assert_eq!(hovered(&mut p), Some(None));
}

#[test]
fn click_reports_values_and_hit() {
    let mut p = plot();
    p.set_axis(AxisId::X, fixed(0.0, 10.0)).unwrap();
    p.set_axis(AxisId::Right, fixed(0.0, 100.0)).unwrap();
    let m = p
        .add_mark(MarkKind::Scatter, MarkOptions::default())
        .unwrap();
    p.set_data(m, MarkData::xy(vec![4.0], vec![40.0])).unwrap();
    frame(&mut p, 0.0);
    let pos = at(&p, 4.0, 40.0);
    down(&mut p, pos, 0.0);
    up(&mut p, pos, 50.0);
    let click = events(&mut p).into_iter().find_map(|e| match e {
        PlotEvent::Click { hit, x, y } => Some((hit, x, y)),
        _ => None,
    });
    let (hit, x, y) = click.unwrap();
    assert_eq!(hit.unwrap().mark, m);
    assert!((x - 4.0).abs() < 0.05 && (y - 40.0).abs() < 0.5);
}

#[test]
fn drag_pans_and_releases_into_kinetic_motion() {
    let mut p = plot();
    p.set_axis(AxisId::X, fixed(0.0, 100.0)).unwrap();
    p.set_axis(AxisId::Right, fixed(0.0, 100.0)).unwrap();
    frame(&mut p, 0.0);
    let w = f64::from(p.layout().plot.w);
    let start = at(&p, 50.0, 50.0);
    down(&mut p, start, 0.0);
    // 10 px per 10 ms to the right.
    for k in 1..=10 {
        mv(
            &mut p,
            PointCss {
                x: start.x + 10.0 * k as f32,
                y: start.y,
            },
            10.0 * f64::from(k),
        );
    }
    let (lo, hi) = p.view(AxisId::X);
    assert!((lo + 100.0 / w * 100.0).abs() < 1e-6, "{lo}");
    assert!((hi - lo - 100.0).abs() < 1e-9);
    assert_eq!(p.view(AxisId::Right), (0.0, 100.0));
    assert!(events(&mut p).contains(&PlotEvent::ViewChanged));
    up(
        &mut p,
        PointCss {
            x: start.x + 100.0,
            y: start.y,
        },
        100.0,
    );
    assert!(p.is_animating());
    frame(&mut p, 116.0);
    let after = p.view(AxisId::X).0;
    assert!(after < lo, "kinetic keeps moving left: {after} vs {lo}");
    let mut t = 116.0;
    while p.is_animating() && t < 10_000.0 {
        t += 16.0;
        frame(&mut p, t);
    }
    assert!(!p.is_animating(), "kinetic motion ends");
    // Travel until the speed falls under 0.05 px/ms: (v − 0.05)·τ with
    // v = 1 px/ms, τ = 325 ms.
    let travel = (lo - p.view(AxisId::X).0) / 100.0 * w;
    assert!((travel - 308.75).abs() < 2.0, "{travel}");
    // Double click fits.
    p.set_axis(AxisId::X, AxisOptions::linear()).unwrap();
    p.set_view(AxisId::X, 3.0, 4.0).unwrap();
    p.handle_input(InputEvent::DoubleClick {
        pos: start,
        button: Button::Left,
    });
    frame(&mut p, t);
    assert_eq!(p.view(AxisId::X), (0.0, 1.0));
}

#[test]
fn locked_axes_stay_and_wheel_zooms_around_the_pointer() {
    let mut p = plot();
    p.set_axis(AxisId::X, fixed(0.0, 100.0)).unwrap();
    p.set_axis(
        AxisId::Right,
        AxisOptions {
            locked: true,
            ..fixed(0.0, 10.0)
        },
    )
    .unwrap();
    frame(&mut p, 0.0);
    let pos = at(&p, 25.0, 5.0);
    p.handle_input(InputEvent::Wheel {
        pos,
        delta: WheelDelta::Pixels {
            x: Css(0.0),
            y: Css(-100.0),
        },
        mods: Modifiers::default(),
    });
    let (lo, hi) = p.view(AxisId::X);
    assert!(hi - lo < 100.0, "zoomed in");
    let v = p.px_to_value(AxisId::X, Px(pos.x)).unwrap();
    assert!((v - 25.0).abs() < 1e-9, "anchor kept: {v}");
    // Alt zooms y, but the y axis is locked.
    p.handle_input(InputEvent::Wheel {
        pos,
        delta: WheelDelta::Lines { x: 0.0, y: 1.0 },
        mods: Modifiers {
            alt: true,
            ..Modifiers::default()
        },
    });
    assert_eq!(p.view(AxisId::Right), (0.0, 10.0));
    // Dragging pans x only.
    down(&mut p, pos, 0.0);
    mv(
        &mut p,
        PointCss {
            x: pos.x,
            y: pos.y + 50.0,
        },
        10.0,
    );
    mv(
        &mut p,
        PointCss {
            x: pos.x + 20.0,
            y: pos.y + 60.0,
        },
        20.0,
    );
    assert_eq!(p.view(AxisId::Right), (0.0, 10.0));
    assert!(p.view(AxisId::X).0 < lo);
}

#[test]
fn axis_drag_zooms_that_axis() {
    let mut p = plot();
    p.set_axis(AxisId::Right, fixed(0.0, 10.0)).unwrap();
    frame(&mut p, 0.0);
    let r = p.layout().right_axis;
    let pos = PointCss {
        x: r.x + r.w / 2.0,
        y: r.y + r.h / 2.0,
    };
    down(&mut p, pos, 0.0);
    mv(
        &mut p,
        PointCss {
            x: pos.x,
            y: pos.y + 40.0,
        },
        10.0,
    );
    up(
        &mut p,
        PointCss {
            x: pos.x,
            y: pos.y + 40.0,
        },
        20.0,
    );
    let (lo, hi) = p.view(AxisId::Right);
    assert!(hi - lo > 10.0, "dragging down zooms out: {lo}..{hi}");
    assert!((lo.midpoint(hi) - 5.0).abs() < 1e-9, "around the center");
}

#[test]
fn many_categories_scroll() {
    let mut p = plot();
    p.set_axis(AxisId::X, AxisOptions::category()).unwrap();
    let m = p.add_mark(MarkKind::Bars, MarkOptions::default()).unwrap();
    let x: Vec<f64> = (0..1000).map(f64::from).collect();
    p.set_data(m, MarkData::xy(x.clone(), x.clone())).unwrap();
    frame(&mut p, 0.0);
    let w = f64::from(p.layout().plot.w);
    let (lo, hi) = p.view(AxisId::X);
    assert_eq!(
        (lo, hi),
        (0.0, (w / 3.0).floor()),
        "slots of at least 3 CSS px"
    );
    let pos = at(&p, 10.0, 10.0);
    p.handle_input(InputEvent::Wheel {
        pos,
        delta: WheelDelta::Pixels {
            x: Css(0.0),
            y: Css(30.0),
        },
        mods: Modifiers::default(),
    });
    let (lo2, hi2) = p.view(AxisId::X);
    assert!(
        lo2 > 0.0 && (hi2 - lo2 - (hi - lo)).abs() < 1e-9,
        "scrolled: {lo2}..{hi2}"
    );
    // Never past the last category.
    for _ in 0..200 {
        p.handle_input(InputEvent::Wheel {
            pos,
            delta: WheelDelta::Lines { x: 0.0, y: 10.0 },
            mods: Modifiers::default(),
        });
    }
    frame(&mut p, 0.0);
    let end = p.view(AxisId::X).1;
    assert!(end <= 1000.0 && end > 1000.0 - 1e-9, "{end}");

    // Few categories: nothing to scroll.
    let mut q = plot();
    q.set_axis(AxisId::X, AxisOptions::category()).unwrap();
    let m = q.add_mark(MarkKind::Bars, MarkOptions::default()).unwrap();
    q.set_data(m, MarkData::xy(vec![0.0, 1.0], vec![1.0, 2.0]))
        .unwrap();
    frame(&mut q, 0.0);
    q.handle_input(InputEvent::Wheel {
        pos: PointCss { x: 100.0, y: 100.0 },
        delta: WheelDelta::Lines { x: 0.0, y: 3.0 },
        mods: Modifiers::default(),
    });
    assert_eq!(q.view(AxisId::X), (0.0, 2.0));
    assert!(events(&mut q).is_empty());
}

#[test]
fn update_last_keeps_the_index_valid() {
    let mut p = plot();
    p.set_axis(AxisId::X, fixed(0.0, 10.0)).unwrap();
    p.set_axis(AxisId::Right, fixed(0.0, 10.0)).unwrap();
    let m = p
        .add_mark(MarkKind::Scatter, MarkOptions::default())
        .unwrap();
    p.set_data(m, MarkData::xy(vec![1.0, 2.0, 3.0], vec![1.0, 2.0, 3.0]))
        .unwrap();
    p.update_last(m, MarkPoint::xy(8.0, 8.0)).unwrap();
    frame(&mut p, 0.0);
    assert_eq!(p.hit_test(at(&p, 8.0, 8.0)).unwrap().index, 2);
    assert!(p.hit_test(at(&p, 3.0, 3.0)).is_none());
}

/// Nearest scatter point within its radius plus the tolerance, the slow way.
fn brute(p: &Plot, xs: &[f64], ys: &[f64], q: PointCss) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for i in 0..xs.len() {
        let c = at(p, xs[i], ys[i]);
        let d = (c.x - q.x).hypot(c.y - q.y);
        // Default diameter 6 CSS px, tolerance 6.
        if d <= 9.0 && best.is_none_or(|b| d < b.0) {
            best = Some((d, i));
        }
    }
    best.map(|b| b.1)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// The grid index finds what a linear scan finds, after panning and
    /// zooming too.
    #[test]
    fn indexed_hits_match_a_linear_scan(
        pts in proptest::collection::vec((0.0_f64..1000.0, -50.0_f64..50.0), 2..400),
        queries in proptest::collection::vec((0.0_f32..1.0, 0.0_f32..1.0), 1..20),
        zoom in 0.5_f64..4.0,
    ) {
        let mut p = plot();
        let m = p.add_mark(MarkKind::Scatter, MarkOptions::default()).unwrap();
        let xs: Vec<f64> = pts.iter().map(|q| q.0).collect();
        let ys: Vec<f64> = pts.iter().map(|q| q.1).collect();
        p.set_data(m, MarkData::xy(xs.clone(), ys.clone())).unwrap();
        frame(&mut p, 0.0);
        p.set_view(AxisId::X, 200.0, 200.0 + 800.0 / zoom).unwrap();
        frame(&mut p, 0.0);
        let r = p.layout().plot;
        for (u, v) in queries {
            let q = PointCss { x: r.x + u * r.w, y: r.y + v * r.h };
            let got = p.hit_test(q).map(|h| h.index);
            let want = brute(&p, &xs, &ys, q);
            // Ties between equally distant points may resolve either way.
            match (got, want) {
                (Some(g), Some(w)) if g != w => {
                    let d = |i: usize| { let c = at(&p, xs[i], ys[i]); (c.x - q.x).hypot(c.y - q.y) };
                    prop_assert!((d(g) - d(w)).abs() < 1e-3, "{g} vs {w}");
                }
                _ => prop_assert_eq!(got.is_some(), want.is_some()),
            }
        }
    }

    /// Values and pixels convert back and forth on linear and log axes.
    #[test]
    fn values_and_pixels_round_trip(lo in -1e6_f64..1e6, span in 1e-3_f64..1e6, t in 0.0_f64..1.0, log in any::<bool>()) {
        let mut p = plot();
        let (lo, hi) = if log { (lo.abs() + 1e-3, lo.abs() + 1e-3 + span) } else { (lo, lo + span) };
        let opts = AxisOptions { range: AxisRange::Fixed(lo, hi), ..if log { AxisOptions::log() } else { AxisOptions::linear() } };
        p.set_axis(AxisId::X, opts).unwrap();
        frame(&mut p, 0.0);
        let v = if log { (lo.ln() + t * (hi.ln() - lo.ln())).exp() } else { lo + t * span };
        let px = p.value_to_px(AxisId::X, v).unwrap();
        let back = p.px_to_value(AxisId::X, px).unwrap();
        // f32 pixels: about 1e-7 of the plot width in value terms.
        let tol = (hi - lo) * 1e-5 + v.abs() * 1e-6;
        prop_assert!((back - v).abs() <= tol, "{v} -> {px:?} -> {back}");
    }
}

fn touch(p: &mut Plot, phase: u8, id: u64, pos: PointCss, t: f64) {
    use ferrochart_core::interaction::PointerKind::Touch;
    let mods = Modifiers::default();
    let (button, kind, time_ms) = (Button::Left, Touch, t);
    p.handle_input(match phase {
        0 => InputEvent::PointerDown {
            id,
            kind,
            pos,
            button,
            mods,
            time_ms,
        },
        1 => InputEvent::PointerMove {
            id,
            kind,
            pos,
            mods,
            time_ms,
        },
        _ => InputEvent::PointerUp {
            id,
            kind,
            pos,
            button,
            mods,
            time_ms,
        },
    });
}

/// A finger: the hover shows at the press and stays after a tap,
/// points are hit from further away, a pan clears the hover.
#[test]
fn touch_hovers_at_the_finger() {
    let mut p = plot();
    p.set_axis(AxisId::X, fixed(0.0, 10.0)).unwrap();
    p.set_axis(AxisId::Right, fixed(0.0, 10.0)).unwrap();
    let m = p
        .add_mark(
            MarkKind::Scatter,
            MarkOptions {
                point_size: Css(10.0),
                ..MarkOptions::default()
            },
        )
        .unwrap();
    p.set_data(m, MarkData::xy(vec![5.0], vec![5.0])).unwrap();
    frame(&mut p, 0.0);
    let pos = at(&p, 5.0, 5.0);
    // 15 CSS px away: out of reach for the mouse (5 + 6), within a finger's.
    let off = PointCss {
        x: pos.x + 15.0,
        y: pos.y,
    };
    mv(&mut p, off, 1.0);
    assert_eq!(hovered(&mut p), None);
    p.handle_input(InputEvent::PointerLeave);
    events(&mut p);
    touch(&mut p, 0, 1, off, 10.0);
    assert_eq!(hovered(&mut p).unwrap().unwrap().index, 0);
    touch(&mut p, 2, 1, off, 20.0);
    let ev = events(&mut p);
    assert!(
        ev.iter()
            .any(|e| matches!(e, PlotEvent::Click { hit: Some(_), .. }))
    );
    assert!(!ev.iter().any(|e| matches!(e, PlotEvent::Hover(None))));
    // A pan clears it and leaves none behind.
    touch(&mut p, 0, 1, off, 100.0);
    touch(
        &mut p,
        1,
        1,
        PointCss {
            x: off.x - 60.0,
            y: off.y,
        },
        110.0,
    );
    touch(
        &mut p,
        2,
        1,
        PointCss {
            x: off.x - 60.0,
            y: off.y,
        },
        400.0,
    );
    assert_eq!(hovered(&mut p), Some(None));
}

/// Two fingers zoom x around their midpoint; the host's pinch of
/// the same gesture is ignored; lifting one finger pans on.
#[test]
fn two_fingers_zoom_and_pan() {
    let mut p = plot();
    p.set_axis(AxisId::X, fixed(0.0, 100.0)).unwrap();
    p.set_axis(AxisId::Right, fixed(0.0, 100.0)).unwrap();
    let m = p.add_mark(MarkKind::Line, MarkOptions::default()).unwrap();
    p.set_data(m, MarkData::xy(vec![0.0, 100.0], vec![0.0, 100.0]))
        .unwrap();
    frame(&mut p, 0.0);
    let a = at(&p, 40.0, 50.0);
    let b = at(&p, 60.0, 50.0);
    let mid = (a.x + b.x) * 0.5;
    let x_at = |p: &Plot, x: f32| p.px_to_value(AxisId::X, Px(x)).unwrap();
    let before = x_at(&p, mid);
    let span = |p: &Plot| x_at(p, b.x) - x_at(p, a.x);
    let s0 = span(&p);
    touch(&mut p, 0, 1, a, 0.0);
    touch(&mut p, 0, 2, b, 10.0);
    let spread = (b.x - a.x) * 0.5;
    touch(
        &mut p,
        1,
        1,
        PointCss {
            x: mid - spread * 2.0,
            y: a.y,
        },
        20.0,
    );
    touch(
        &mut p,
        1,
        2,
        PointCss {
            x: mid + spread * 2.0,
            y: b.y,
        },
        30.0,
    );
    assert!((x_at(&p, mid) - before).abs() < 1e-6);
    assert!((span(&p) / s0 - 0.5).abs() < 1e-6);
    let s1 = span(&p);
    p.handle_input(InputEvent::Pinch {
        phase: ferrochart_core::interaction::GesturePhase::Update,
        center: PointCss { x: mid, y: a.y },
        scale: 2.0,
    });
    assert_eq!(span(&p), s1);
    assert!(p.captures_touch());
    touch(
        &mut p,
        2,
        2,
        PointCss {
            x: mid + spread * 2.0,
            y: b.y,
        },
        40.0,
    );
    let v = x_at(&p, mid);
    touch(
        &mut p,
        1,
        1,
        PointCss {
            x: mid - spread * 2.0 + 30.0,
            y: a.y,
        },
        50.0,
    );
    assert!(x_at(&p, mid) < v);
    // A one-finger pan leaves vertical swipes to the page.
    assert!(!p.captures_touch());
}
