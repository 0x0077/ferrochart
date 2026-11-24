//! Axes, grid and padding; replacing the theme (r25).
//!
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::appearance::{Appearance, GridLines, Padding};
use ferrochart_core::frame::{DrawCmd, LayerId, SeriesKind};
use ferrochart_core::geom::{Css, RectPx, SizeCss};
use ferrochart_core::testing::{MonoMeasurer, quote_card_scene};
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};

fn chart(appearance: Appearance) -> Chart {
    let mut chart = Chart::new(ChartOptions {
        appearance,
        ..ChartOptions::default()
    });
    chart.resize(SizeCss { w: 400.0, h: 200.0 }, 2.0);
    quote_card_scene(&mut chart, 0, SeriesKind::Baseline).unwrap();
    chart.build_frame(0.0, &MonoMeasurer);
    chart
}

fn plot(chart: &Chart) -> RectPx {
    chart.pane_layout(chart.main_pane()).unwrap().plot
}

#[test]
fn hidden_axes_give_the_plot_the_canvas_less_the_padding() {
    let c = chart(Appearance::sparkline());
    // 2 CSS px of padding at dpr 2.
    assert_eq!(
        plot(&c),
        RectPx {
            x: 4.0,
            y: 4.0,
            w: 792.0,
            h: 392.0
        }
    );
    let layout = c.pane_layout(c.main_pane()).unwrap();
    assert_eq!(layout.price_axis.w, 0.0);

    let full = chart(Appearance {
        price_axis: false,
        time_axis: false,
        ..Appearance::default()
    });
    assert_eq!(
        plot(&full),
        RectPx {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 400.0
        }
    );
}

#[test]
fn each_axis_hides_on_its_own() {
    let shown = plot(&chart(Appearance::default()));
    let no_price = plot(&chart(Appearance {
        price_axis: false,
        ..Appearance::default()
    }));
    let no_time = plot(&chart(Appearance {
        time_axis: false,
        ..Appearance::default()
    }));
    assert!(shown.w < 800.0 && shown.h < 400.0);
    assert_eq!((no_price.w, no_price.h), (800.0, shown.h));
    assert_eq!((no_time.w, no_time.h), (shown.w, 400.0));
}

/// Text runs of the main layer (axis labels, value labels).
fn texts(chart: &Chart, layer: LayerId) -> usize {
    chart.frame().layers[layer as usize]
        .cmds
        .iter()
        .filter(|c| matches!(c, DrawCmd::Text(_)))
        .count()
}

#[test]
fn hidden_axes_draw_no_labels() {
    let shown = chart(Appearance::default());
    assert!(texts(&shown, LayerId::Main) > 0);
    let hidden = chart(Appearance {
        price_axis: false,
        time_axis: false,
        ..Appearance::default()
    });
    assert_eq!(texts(&hidden, LayerId::Main), 0);
}

#[test]
fn grid_lines_switch_off_by_direction() {
    let lines = |grid: GridLines| {
        let c = chart(Appearance {
            grid,
            ..Appearance::default()
        });
        c.frame().layers[LayerId::Background as usize]
            .cmds
            .iter()
            .map(|cmd| match cmd {
                DrawCmd::Lines(lines) => lines.len as usize,
                _ => 0,
            })
            .sum::<usize>()
    };
    let all = lines(GridLines::default());
    let h = lines(GridLines {
        vertical: false,
        ..GridLines::default()
    });
    let v = lines(GridLines {
        horizontal: false,
        ..GridLines::default()
    });
    assert!(h > 0 && v > 0);
    assert_eq!(h + v, all);
    assert_eq!(lines(GridLines::NONE), 0);
}

#[test]
fn set_appearance_and_set_theme_keep_the_view() {
    let mut c = chart(Appearance::default());
    let before = c.time_scale().visible_range();
    let spacing = c.time_scale().bar_spacing();
    c.set_appearance(Appearance {
        padding: Padding::all(Css(5.0)),
        ..Appearance::sparkline()
    });
    c.set_theme(Theme::dark());
    c.build_frame(1.0, &MonoMeasurer);
    assert_eq!(c.frame().clear_color, Theme::dark().background);
    assert_eq!(plot(&c).x, 10.0);
    // The plot got wider as on a resize: same spacing, same right
    // edge, more bars on the left.
    let after = c.time_scale().visible_range();
    assert!((after.1 - before.1).abs() < 1e-9);
    assert!(after.0 < before.0);
    assert!((c.time_scale().bar_spacing() - spacing).abs() < 1e-9);
}
