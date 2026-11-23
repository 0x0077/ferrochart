//! Plot layout: the y axes fit their rule labels.
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::geom::SizeCss;
use ferrochart_core::testing::MonoMeasurer;
use ferrochart_plot::{
    AxisId, AxisOptions, AxisRange, MarkData, MarkKind, MarkOptions, Plot, PlotOptions, RuleOptions,
};

fn plot() -> Plot {
    let mut p = Plot::new(PlotOptions::default());
    p.resize(SizeCss { w: 600.0, h: 400.0 }, 1.0);
    p.set_axis(
        AxisId::Right,
        AxisOptions {
            range: AxisRange::Fixed(0.0, 9.0),
            ..AxisOptions::linear()
        },
    )
    .unwrap();
    let m = p.add_mark(MarkKind::Line, MarkOptions::default()).unwrap();
    p.set_data(m, MarkData::xy(vec![0.0, 1.0], vec![1.0, 2.0]))
        .unwrap();
    p
}

fn right_width(p: &mut Plot) -> f32 {
    p.build_frame(0.0, &MonoMeasurer);
    p.layout().right_axis.w
}

#[test]
fn long_rule_labels_widen_the_axis() {
    let mut p = plot();
    let bare = right_width(&mut p);
    p.add_rule(RuleOptions {
        axis: AxisId::Right,
        value: 5.0,
        label: Some("a much longer target".into()),
        ..RuleOptions::default()
    })
    .unwrap();
    let wide = right_width(&mut p);
    assert!(wide > bare, "{wide} > {bare}");

    // A short label fits in the tick labels' width and changes nothing.
    let mut q = plot();
    q.add_rule(RuleOptions {
        axis: AxisId::Right,
        value: 5.0,
        label: Some("5".into()),
        ..RuleOptions::default()
    })
    .unwrap();
    assert_eq!(right_width(&mut q), bare);
}
