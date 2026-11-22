//! Gradual display of a plot: marks, rules
//! and labels are clipped to the revealed part of the plot area, axes are
//! whole, rule labels and the hover wait for the end, and progress 1 draws
//! exactly what no reveal draws.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::frame::{DrawCmd, Frame, LayerId, Reveal};
use ferrochart_core::geom::{PointCss, RectPx, Rgba, SizeCss};
use ferrochart_core::interaction::{InputEvent, Modifiers};
use ferrochart_core::testing::MonoMeasurer;
use ferrochart_plot::{
    AxisId, MarkData, MarkKind, MarkOptions, Plot, PlotEvent, PlotOptions, RuleOptions,
};

const PEN: Rgba = Rgba::hex(0x004c_c9f0);

/// A line with a labeled rule, hovered with the crosshair on.
fn plot() -> Plot {
    let mut p = Plot::new(PlotOptions {
        crosshair: true,
        ..PlotOptions::default()
    });
    p.resize(SizeCss { w: 600.0, h: 400.0 }, 1.0);
    let m = p.add_mark(MarkKind::Line, MarkOptions::default()).unwrap();
    let x: Vec<f64> = (0..50).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| (v * 0.2).sin()).collect();
    p.set_data(m, MarkData::xy(x, y)).unwrap();
    p.add_rule(RuleOptions {
        axis: AxisId::Right,
        value: 0.5,
        label: Some("0.50".into()),
        ..RuleOptions::default()
    })
    .unwrap();
    p.build_frame(0.0, &MonoMeasurer);
    let pos = PointCss {
        x: p.value_to_px(AxisId::X, 10.0).unwrap().0,
        y: p.value_to_px(AxisId::Right, 2.0f64.sin()).unwrap().0,
    };
    p.handle_input(InputEvent::PointerMove {
        id: 0,
        pos,
        mods: Modifiers::default(),
        time_ms: 0.0,
    });
    assert!(
        p.drain_events()
            .any(|e| matches!(e, PlotEvent::Hover(Some(_))))
    );
    p.build_frame(0.0, &MonoMeasurer);
    p
}

fn clips(frame: &Frame, layer: LayerId) -> Vec<RectPx> {
    frame.layers[layer as usize]
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::PushClip(r) => Some(*r),
            _ => None,
        })
        .collect()
}

fn texts(frame: &Frame) -> usize {
    frame
        .layers
        .iter()
        .flat_map(|l| &l.cmds)
        .map(|c| match c {
            DrawCmd::Text(s) => s.len as usize,
            _ => 0,
        })
        .sum()
}

#[test]
fn marks_are_clipped_axes_whole_hover_and_rule_labels_hidden() {
    let mut p = plot();
    let plot_rect = p.layout().plot;
    let full_texts = texts(p.frame());
    assert!(!p.frame().layers[LayerId::Overlay as usize].cmds.is_empty());
    let background = p.frame().layers[LayerId::Background as usize].cmds.clone();
    p.set_reveal(Some(Reveal {
        progress: 0.3,
        pen: None,
    }));
    p.build_frame(0.0, &MonoMeasurer);
    let f = p.frame();
    let cut = clips(f, LayerId::Main);
    assert_eq!(cut.len(), 1);
    assert_eq!(cut[0].w, (plot_rect.w * 0.3).round());
    assert_eq!(
        (cut[0].x, cut[0].y, cut[0].h),
        (plot_rect.x, plot_rect.y, plot_rect.h)
    );
    // No hover highlight or crosshair.
    assert!(f.layers[LayerId::Overlay as usize].cmds.is_empty());
    // Tick labels stay; the rule label goes.
    let during = texts(f);
    assert!(
        during > 0 && during < full_texts,
        "{during} of {full_texts}"
    );
    assert_eq!(f.layers[LayerId::Background as usize].cmds, background);
}

#[test]
fn progress_one_draws_what_no_reveal_draws() {
    let mut a = plot();
    let mut b = plot();
    a.set_reveal(Some(Reveal {
        progress: 1.0,
        pen: Some(PEN),
    }));
    a.build_frame(0.0, &MonoMeasurer);
    b.build_frame(0.0, &MonoMeasurer);
    for l in 0..3 {
        assert_eq!(
            a.frame().layers[l].cmds,
            b.frame().layers[l].cmds,
            "layer {l}"
        );
    }
    assert_eq!(a.frame().pools.lines, b.frame().pools.lines);
    assert_eq!(a.reveal().map(|r| r.progress), Some(1.0));
}

#[test]
fn the_pen_is_drawn_at_the_cut() {
    let mut p = plot();
    p.set_reveal(Some(Reveal {
        progress: 0.5,
        pen: Some(PEN),
    }));
    p.build_frame(0.0, &MonoMeasurer);
    let f = p.frame();
    let cut = clips(f, LayerId::Main)[0];
    let pens: Vec<_> = f
        .pools
        .lines
        .iter()
        .filter(|l| (l.stroke.color.r, l.stroke.color.g, l.stroke.color.b) == (PEN.r, PEN.g, PEN.b))
        .collect();
    assert_eq!(pens.len(), 3);
    assert!(
        pens.iter()
            .all(|l| l.from.x == cut.x + cut.w && l.to.x == l.from.x)
    );
}
