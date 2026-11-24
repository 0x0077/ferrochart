//! Gradual display of the data: the data layer is
//! clipped to the revealed part of each pane, grid and axes are whole, price
//! lines, value labels and the crosshair wait for the end, and progress 1
//! draws exactly what no reveal draws.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::chart::{PriceLineOptions, SeriesData, SeriesOptions};
use ferrochart_core::frame::{DrawCmd, Frame, LayerId, LineInst, Reveal, SeriesKind};
use ferrochart_core::geom::{PointCss, RectPx, Rgba, SizeCss};
use ferrochart_core::interaction::{InputEvent, Modifiers};
use ferrochart_core::pane::PaneOptions;
use ferrochart_core::testing::{MonoMeasurer, random_walk};
use ferrochart_core::{Chart, ChartOptions};

const START: i64 = 1_704_067_200_000;
const PEN: Rgba = Rgba::hex(0x004c_c9f0);

/// Candles with a price line in the main pane, volume in a second pane, and
/// the pointer over the main pane (a crosshair).
fn chart() -> Chart {
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
    let main = chart.main_pane();
    let id = chart
        .add_series(main, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    let lower = chart.add_pane(PaneOptions {
        stretch_factor: 0.3,
    });
    chart
        .add_series(lower, SeriesKind::Volume, SeriesOptions::volume_of(id))
        .unwrap();
    let data = random_walk(3, 300, START, 60_000).unwrap();
    let last = data.close[data.close.len() - 1];
    chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
    chart
        .create_price_line(
            id,
            PriceLineOptions {
                price: last,
                ..PriceLineOptions::default()
            },
        )
        .unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    chart.handle_input(InputEvent::PointerMove {
        id: 0,
        kind: ferrochart_core::interaction::PointerKind::Mouse,
        pos: PointCss { x: 300.0, y: 120.0 },
        mods: Modifiers::default(),
        time_ms: 0.0,
    });
    chart.build_frame(0.0, &MonoMeasurer);
    chart
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

/// The number of text runs of all layers.
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

/// The line segments a layer draws, resolved from the pool (pool order
/// depends on what other layers emitted first).
fn lines(frame: &Frame, layer: LayerId) -> Vec<LineInst> {
    frame.layers[layer as usize]
        .cmds
        .iter()
        .flat_map(|c| match c {
            DrawCmd::Lines(s) => {
                frame.pools.lines[s.start as usize..(s.start + s.len) as usize].to_vec()
            }
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn data_is_clipped_to_the_revealed_part_of_each_pane() {
    let mut chart = chart();
    let plots: Vec<RectPx> = clips(chart.frame(), LayerId::Main);
    chart.set_reveal(Some(Reveal {
        progress: 0.4,
        pen: None,
    }));
    chart.build_frame(0.0, &MonoMeasurer);
    let revealed = clips(chart.frame(), LayerId::Main);
    assert_eq!(plots.len(), revealed.len());
    for (full, cut) in plots.iter().zip(&revealed) {
        assert_eq!((cut.x, cut.y, cut.h), (full.x, full.y, full.h));
        assert_eq!(cut.w, (full.w * 0.4).round());
    }
    // Background (grid) is whole: same commands as without the reveal.
    let mut plain = self::chart();
    plain.build_frame(0.0, &MonoMeasurer);
    assert_eq!(
        lines(chart.frame(), LayerId::Background),
        lines(plain.frame(), LayerId::Background)
    );
}

#[test]
fn labels_and_crosshair_wait_for_the_end() {
    let mut chart = chart();
    let with_labels = texts(chart.frame());
    chart.set_reveal(Some(Reveal {
        progress: 0.7,
        pen: None,
    }));
    chart.build_frame(0.0, &MonoMeasurer);
    let during = texts(chart.frame());
    // Value labels, the price line's label and the crosshair labels go;
    // tick labels stay.
    assert!(
        during > 0 && during < with_labels,
        "{during} of {with_labels}"
    );
    chart.set_reveal(Some(Reveal {
        progress: 1.0,
        pen: Some(PEN),
    }));
    chart.build_frame(0.0, &MonoMeasurer);
    assert_eq!(texts(chart.frame()), with_labels);
}

#[test]
fn progress_one_draws_what_no_reveal_draws() {
    let mut a = chart();
    let mut b = chart();
    a.set_reveal(Some(Reveal {
        progress: 1.0,
        pen: Some(PEN),
    }));
    a.build_frame(0.0, &MonoMeasurer);
    b.set_reveal(None);
    b.build_frame(0.0, &MonoMeasurer);
    for l in 0..3 {
        assert_eq!(
            a.frame().layers[l].cmds,
            b.frame().layers[l].cmds,
            "layer {l}"
        );
    }
    assert_eq!(a.frame().pools.lines, b.frame().pools.lines);
}

#[test]
fn the_pen_is_drawn_at_the_cut_off_of_each_pane() {
    let mut chart = chart();
    let before = chart.frame().pools.lines.len();
    chart.set_reveal(Some(Reveal {
        progress: 0.5,
        pen: Some(PEN),
    }));
    chart.build_frame(0.0, &MonoMeasurer);
    let frame = chart.frame();
    let pens: Vec<_> = frame
        .pools
        .lines
        .iter()
        .filter(|l| {
            l.stroke.color.r == PEN.r && l.stroke.color.g == PEN.g && l.stroke.color.b == PEN.b
        })
        .collect();
    // Two panes, three strokes each (glow, glow, line), vertical.
    assert_eq!(pens.len(), 6, "{before} lines before");
    for l in &pens {
        assert_eq!(l.from.x, l.to.x);
    }
    let cut = clips(frame, LayerId::Main)[0];
    assert!(pens.iter().any(|l| l.from.x == cut.x + cut.w));
    // None at 0 or without a pen.
    chart.set_reveal(Some(Reveal {
        progress: 0.0,
        pen: Some(PEN),
    }));
    chart.build_frame(0.0, &MonoMeasurer);
    assert!(
        !chart
            .frame()
            .pools
            .lines
            .iter()
            .any(|l| l.stroke.color.b == PEN.b && l.stroke.color.r == PEN.r)
    );
}

#[test]
fn progress_is_clamped_and_nan_hides_the_data() {
    let plot = RectPx {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 10.0,
    };
    let at = |p: f32| {
        Reveal {
            progress: p,
            pen: None,
        }
        .clip(plot)
        .w
    };
    assert_eq!(at(-1.0), 0.0);
    assert_eq!(at(f32::NAN), 0.0);
    assert_eq!(at(2.0), 100.0);
    assert!(
        Reveal {
            progress: 3.0,
            pen: None
        }
        .is_done()
    );
}
