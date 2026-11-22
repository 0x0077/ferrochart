//! Line decimation: a sorted line with more
//! points than pixel columns keeps, in every pixel column, the same highest
//! and lowest point as the undecimated line.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use ferrochart_core::frame::{DrawCmd, LayerId};
use ferrochart_core::geom::SizeCss;
use ferrochart_core::testing::MonoMeasurer;
use ferrochart_plot::{AxisId, MarkData, MarkKind, MarkOptions, Plot, PlotOptions};
use proptest::prelude::*;

/// Lowest and highest y (f32 bits for exact comparison) per pixel column
/// inside `x0..x1`.
type Extremes = BTreeMap<i64, (u32, u32)>;

fn add(e: &mut Extremes, x: f32, y: f32, (x0, x1): (f32, f32)) {
    if !(x0..x1).contains(&x) {
        return;
    }
    let c = e
        .entry(x.floor() as i64)
        .or_insert((y.to_bits(), y.to_bits()));
    if y < f32::from_bits(c.0) {
        c.0 = y.to_bits();
    }
    if y > f32::from_bits(c.1) {
        c.1 = y.to_bits();
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn decimation_keeps_column_extremes(
        steps in proptest::collection::vec(0.0_f64..2.0, 3000..8000),
        ys in proptest::collection::vec(prop_oneof![30 => -100.0_f64..100.0, 1 => Just(f64::NAN)], 8000),
        view in (0.0_f64..0.3, 0.7_f64..1.0),
    ) {
        let mut p = Plot::new(PlotOptions::default());
        p.resize(SizeCss { w: 300.0, h: 200.0 }, 1.0);
        let m = p.add_mark(MarkKind::Line, MarkOptions::default()).unwrap();
        let mut acc = 0.0;
        let x: Vec<f64> = steps.iter().map(|s| { acc += s; acc }).collect();
        let y = ys[..x.len()].to_vec();
        p.set_data(m, MarkData::xy(x.clone(), y.clone())).unwrap();
        p.build_frame(0.0, &MonoMeasurer);
        let span = x[x.len() - 1] - x[0];
        p.set_view(AxisId::X, x[0] + span * view.0, x[0] + span * view.1).unwrap();
        p.build_frame(0.0, &MonoMeasurer);
        let r = p.layout().plot;
        // Physical pixels at dpr 1.
        let cols = (r.x, r.x + r.w);

        let mut want = Extremes::new();
        // A point between two gaps draws nothing (a line needs two points).
        let joined = |i: usize| {
            (i > 0 && y[i - 1].is_finite()) || y.get(i + 1).is_some_and(|v| v.is_finite())
        };
        for (_, (&xv, &yv)) in x.iter().zip(&y).enumerate().filter(|e| joined(e.0)) {
            if let (Some(px), Some(py)) = (p.value_to_px(AxisId::X, xv), p.value_to_px(AxisId::Right, yv)) {
                add(&mut want, px.0, py.0, cols);
            }
        }
        let f = p.frame();
        let mut got = Extremes::new();
        let mut drawn = 0;
        for cmd in &f.layers[LayerId::Main as usize].cmds {
            if let DrawCmd::Polyline { points, .. } = cmd {
                for q in &f.pools.points[points.range()] {
                    add(&mut got, q.x, q.y, cols);
                    drawn += 1;
                }
            }
        }
        // Decimated: at most four points per column and line piece (a gap
        // splits a column), plus the neighbors outside the view.
        let gaps = y.iter().filter(|v| v.is_nan()).count();
        prop_assert!(drawn <= 4 * (r.w as usize + 2 + gaps), "{drawn} points for {} columns", r.w);
        prop_assert_eq!(got, want);
    }
}
