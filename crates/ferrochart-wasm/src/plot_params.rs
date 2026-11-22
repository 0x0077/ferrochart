//! Names of plot options in the bindings: the
//! strings JS (and Python) use for mark kinds, axes, formats, symbols and
//! waterfall steps, both ways. Browser-independent, so tested natively.
// Used only by the wasm32 binding; compiled everywhere so it is tested natively.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use ferrochart_core::frame::TextAlign;
use ferrochart_core::transform_helpers::{
    ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_CIRCLE, ICON_SQUARE,
};
use ferrochart_plot::{AxisFormat, AxisId, AxisKind, MarkKind, Orientation, WaterfallStep};

/// Mark kinds and their names.
const MARK_KINDS: [(&str, MarkKind); 6] = [
    ("line", MarkKind::Line),
    ("area", MarkKind::Area),
    ("scatter", MarkKind::Scatter),
    ("bars", MarkKind::Bars),
    ("waterfall", MarkKind::Waterfall),
    ("cells", MarkKind::Cells),
];

const AXES: [(&str, AxisId); 3] = [
    ("x", AxisId::X),
    ("left", AxisId::Left),
    ("right", AxisId::Right),
];

const AXIS_KINDS: [(&str, AxisKind); 3] = [
    ("linear", AxisKind::Linear),
    ("log", AxisKind::Log),
    ("category", AxisKind::Category),
];

const ICONS: [(&str, u16); 4] = [
    ("circle", ICON_CIRCLE),
    ("square", ICON_SQUARE),
    ("arrowUp", ICON_ARROW_UP),
    ("arrowDown", ICON_ARROW_DOWN),
];

const STEPS: [(&str, WaterfallStep); 3] = [
    ("delta", WaterfallStep::Delta),
    ("subtotal", WaterfallStep::Subtotal),
    ("total", WaterfallStep::Total),
];

const ALIGNS: [(&str, TextAlign); 3] = [
    ("left", TextAlign::Left),
    ("center", TextAlign::Center),
    ("right", TextAlign::Right),
];

const ORIENTATIONS: [(&str, Orientation); 2] = [
    ("vertical", Orientation::Vertical),
    ("horizontal", Orientation::Horizontal),
];

fn find<T: Copy>(table: &[(&str, T)], s: &str) -> Option<T> {
    table.iter().find(|e| e.0 == s).map(|e| e.1)
}

fn name<T: Copy + PartialEq>(table: &[(&'static str, T)], v: T) -> &'static str {
    table.iter().find(|e| e.1 == v).map_or("", |e| e.0)
}

pub(crate) fn mark_kind(s: &str) -> Option<MarkKind> {
    find(&MARK_KINDS, s)
}

pub(crate) fn mark_kind_name(k: MarkKind) -> &'static str {
    name(&MARK_KINDS, k)
}

pub(crate) fn axis_id(s: &str) -> Option<AxisId> {
    find(&AXES, s)
}

pub(crate) fn axis_id_name(a: AxisId) -> &'static str {
    name(&AXES, a)
}

pub(crate) fn axis_kind(s: &str) -> Option<AxisKind> {
    find(&AXIS_KINDS, s)
}

pub(crate) fn axis_kind_name(k: AxisKind) -> &'static str {
    name(&AXIS_KINDS, k)
}

pub(crate) fn icon(s: &str) -> Option<u16> {
    find(&ICONS, s)
}

/// The name of symbol `i` (`circle` for an unknown number).
pub(crate) fn icon_name(i: u16) -> &'static str {
    match name(&ICONS, i) {
        "" => "circle",
        n => n,
    }
}

pub(crate) fn waterfall_step(s: &str) -> Option<WaterfallStep> {
    find(&STEPS, s)
}

pub(crate) fn text_align(s: &str) -> Option<TextAlign> {
    find(&ALIGNS, s)
}

pub(crate) fn orientation(s: &str) -> Option<Orientation> {
    find(&ORIENTATIONS, s)
}

pub(crate) fn orientation_name(o: Orientation) -> &'static str {
    name(&ORIENTATIONS, o)
}

/// A format without parameters: `auto`, `compact` or `date` (UTC).
pub(crate) fn axis_format(s: &str) -> Option<AxisFormat> {
    Some(match s {
        "auto" => AxisFormat::Auto,
        "compact" => AxisFormat::Compact,
        "date" => AxisFormat::Date { utc_offset_min: 0 },
        _ => return None,
    })
}

/// A format with decimals: `fixed` or `percent` (0 to 12 decimals).
pub(crate) fn axis_format_decimals(kind: &str, decimals: f64) -> Option<AxisFormat> {
    if !(decimals.fract() == 0.0 && (0.0..=12.0).contains(&decimals)) {
        return None;
    }
    let d = decimals as u8;
    Some(match kind {
        "fixed" => AxisFormat::Fixed(d),
        "percent" => AxisFormat::Percent(d),
        _ => return None,
    })
}

/// The parts of a format as the bindings write it: its type, its decimals
/// (`fixed`, `percent`) and its UTC offset in minutes (`date`).
pub(crate) fn axis_format_parts(f: AxisFormat) -> (&'static str, Option<u8>, Option<i32>) {
    match f {
        AxisFormat::Auto => ("auto", None, None),
        AxisFormat::Compact => ("compact", None, None),
        AxisFormat::Fixed(d) => ("fixed", Some(d), None),
        AxisFormat::Percent(d) => ("percent", Some(d), None),
        AxisFormat::Date { utc_offset_min } => ("date", None, Some(utc_offset_min)),
    }
}

/// Price at `t` (0 at the top of a pane, 1 at the bottom, may be outside)
/// for a pane showing `lo..hi`, linearly or on a log scale, possibly
/// inverted. Used to line a side plot up with a chart pane.
pub(crate) fn price_at(t: f64, (lo, hi): (f64, f64), log: bool, inverted: bool) -> f64 {
    let (top, bottom) = if inverted { (lo, hi) } else { (hi, lo) };
    if log {
        if top > 0.0 && bottom > 0.0 {
            (top.ln() + t * (bottom.ln() - top.ln())).exp()
        } else {
            f64::NAN
        }
    } else {
        top + t * (bottom - top)
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for (n, k) in MARK_KINDS {
            assert_eq!(mark_kind(n), Some(k));
            assert_eq!(mark_kind_name(k), n);
        }
        for (n, a) in AXES {
            assert_eq!(axis_id_name(axis_id(n).unwrap()), n);
            assert_eq!(axis_id_name(a), n);
        }
        for (n, k) in AXIS_KINDS {
            assert_eq!(axis_kind_name(axis_kind(n).unwrap()), n);
            assert_eq!(axis_kind_name(k), n);
        }
        for (n, i) in ICONS {
            assert_eq!(icon(n), Some(i));
            assert_eq!(icon_name(i), n);
        }
        assert_eq!(icon_name(999), "circle");
        for (n, a) in ALIGNS {
            assert_eq!(text_align(n), Some(a));
        }
        for (n, o) in ORIENTATIONS {
            assert_eq!(orientation(n), Some(o));
            assert_eq!(orientation_name(o), n);
        }
        assert_eq!(waterfall_step("total"), Some(WaterfallStep::Total));
        assert_eq!(mark_kind("pie"), None);
        assert_eq!(axis_id("y"), None);
    }

    #[test]
    fn formats() {
        assert_eq!(axis_format("compact"), Some(AxisFormat::Compact));
        assert_eq!(axis_format("fixed"), None);
        assert_eq!(
            axis_format_decimals("fixed", 2.0),
            Some(AxisFormat::Fixed(2))
        );
        assert_eq!(
            axis_format_decimals("percent", 0.0),
            Some(AxisFormat::Percent(0))
        );
        assert_eq!(axis_format_decimals("fixed", 1.5), None);
        assert_eq!(axis_format_decimals("fixed", 13.0), None);
        assert_eq!(axis_format_decimals("auto", 2.0), None);
        assert_eq!(
            axis_format_parts(AxisFormat::Date { utc_offset_min: 60 }),
            ("date", None, Some(60))
        );
        assert_eq!(
            axis_format_parts(AxisFormat::Fixed(3)),
            ("fixed", Some(3), None)
        );
    }

    #[test]
    fn prices_across_a_pane() {
        assert_eq!(price_at(0.0, (10.0, 20.0), false, false), 20.0);
        assert_eq!(price_at(1.0, (10.0, 20.0), false, false), 10.0);
        // Beyond the pane: extrapolated.
        assert_eq!(price_at(-0.5, (10.0, 20.0), false, false), 25.0);
        assert_eq!(price_at(0.0, (10.0, 20.0), false, true), 10.0);
        let mid = price_at(0.5, (1.0, 100.0), true, false);
        assert!((mid - 10.0).abs() < 1e-9, "{mid}");
        assert!(price_at(0.5, (-1.0, 100.0), true, false).is_nan());
    }
}
