//! Plot axes: kinds, ranges, label formats, the value ↔ pixel mapping, ticks
//! and tick labels.
//!

use core::fmt::Write;

use ferrochart_core::format::{NumberFormat, auto_decimals};
use ferrochart_core::frame::FontStyleRef;
use ferrochart_core::price_scale::{linear_ticks, log_ticks};
use ferrochart_core::text::{StackStr, TextMeasurer, text_width};
use ferrochart_core::time_scale::{TickWeight, format_tick, time_ticks};

/// Which axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AxisId {
    /// The horizontal axis.
    X,
    /// The vertical axis on the left.
    Left,
    /// The vertical axis on the right.
    Right,
}

/// How values map to positions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AxisKind {
    /// Linear.
    #[default]
    Linear,
    /// Logarithmic; non-positive values are not drawn.
    Log,
    /// Equal slots: category `k` covers `[k, k + 1)`, its center is `k + 0.5`.
    Category,
}

/// The value range of an axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AxisRange {
    /// From the visible data of the marks bound to the axis (all categories
    /// on a category axis), with `margin` (a fraction of the range) at both
    /// ends; `include_zero` keeps 0 inside.
    Auto {
        /// Keep 0 inside the range.
        include_zero: bool,
        /// Fraction of the range added at both ends.
        margin: f32,
    },
    /// A fixed range.
    Fixed(f64, f64),
}

impl Default for AxisRange {
    fn default() -> Self {
        Self::Auto {
            include_zero: false,
            margin: 0.05,
        }
    }
}

/// How tick labels are written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AxisFormat {
    /// Automatic precision: at least 2 decimals and 4 significant digits,
    /// subscript zeros for small values (like price labels).
    #[default]
    Auto,
    /// Fixed decimals.
    Fixed(u8),
    /// Fixed decimals and a `%` suffix (values are percent).
    Percent(u8),
    /// K/M/B suffixes.
    Compact,
    /// Values are UTC milliseconds, shown as dates and times at this offset.
    Date {
        /// Offset of the shown local time from UTC, in minutes.
        utc_offset_min: i32,
    },
}

/// Options of one axis.
///
/// ```
/// use ferrochart_plot::{AxisKind, AxisOptions};
///
/// let x = AxisOptions::category();
/// assert_eq!(x.kind, AxisKind::Category);
/// assert!(!x.grid);
/// ```
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // independent switches
pub struct AxisOptions {
    /// Linear, logarithmic or category.
    pub kind: AxisKind,
    /// Automatic or fixed range.
    pub range: AxisRange,
    /// Tick label format.
    pub format: AxisFormat,
    /// Values decrease to the right (x) or upwards (y).
    pub inverted: bool,
    /// Ticks, labels and the axis line are drawn.
    pub visible: bool,
    /// Grid lines at the ticks (category axes: at slot boundaries).
    pub grid: bool,
    /// Title drawn beside the axis.
    pub title: Option<String>,
    /// Panning and zooming leave the axis alone.
    pub locked: bool,
}

impl Default for AxisOptions {
    fn default() -> Self {
        Self::linear()
    }
}

impl AxisOptions {
    /// A visible linear axis with an automatic range and grid lines.
    pub fn linear() -> Self {
        Self {
            kind: AxisKind::Linear,
            range: AxisRange::default(),
            format: AxisFormat::Auto,
            inverted: false,
            visible: true,
            grid: true,
            title: None,
            locked: false,
        }
    }

    /// A visible logarithmic axis.
    pub fn log() -> Self {
        Self {
            kind: AxisKind::Log,
            ..Self::linear()
        }
    }

    /// A visible category axis without grid lines.
    pub fn category() -> Self {
        Self {
            kind: AxisKind::Category,
            range: AxisRange::Auto {
                include_zero: false,
                margin: 0.0,
            },
            grid: false,
            ..Self::linear()
        }
    }

    /// A visible linear axis of UTC milliseconds, labelled as dates.
    pub fn date() -> Self {
        Self {
            format: AxisFormat::Date { utc_offset_min: 0 },
            ..Self::linear()
        }
    }

    /// The same axis, not drawn.
    #[must_use]
    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }
}

/// Smallest slot of a category axis, in CSS px: views narrower than this per
/// category are not allowed.
pub(crate) const MIN_SLOT_CSS: f32 = 3.0;

/// One tick label of the last layout.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TickLabel {
    /// Value (category axes: the category number).
    pub value: f64,
    /// Calendar weight for date axes.
    pub weight: TickWeight,
}

/// An axis and its state.
#[derive(Clone, Debug)]
pub(crate) struct Axis {
    pub opts: AxisOptions,
    pub categories: Vec<String>,
    /// Widths of the category labels at `label_font_size` (0: not measured).
    label_widths: Vec<f32>,
    label_font_size: f32,
    /// Current range in data units (category units on a category axis).
    pub view: (f64, f64),
    /// The view follows the data (`AxisRange::Auto` and not moved by the user).
    pub auto: bool,
    /// Pixel position of the start of the axis (left or top edge of the plot).
    pub start: f64,
    /// Length in pixels.
    pub len: f64,
    pub horizontal: bool,
    /// Ticks of the last layout.
    pub ticks: Vec<TickLabel>,
    tick_values: Vec<f64>,
    time_buf: Vec<(i64, TickWeight)>,
    /// Number format of the ticks of the last layout.
    pub number: NumberFormat,
    /// Every n-th category is labelled.
    pub category_step: usize,
}

/// Values to pixels along one axis: `origin + (f(v + shift) − a)·scale`,
/// where `f` is the identity or `ln` (log axes; NaN for `v ≤ 0`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct PxMap {
    log: bool,
    shift: f64,
    a: f64,
    origin: f64,
    scale: f64,
}

impl PxMap {
    /// Whether `v` has a pixel position (finite; positive on a log axis).
    #[inline]
    pub fn has_px(&self, v: f64) -> bool {
        v.is_finite() && (!self.log || v > 0.0)
    }

    /// Pixel position of `v`.
    #[inline]
    pub fn px(&self, v: f64) -> f64 {
        let u = if self.log {
            if v > 0.0 { v.ln() } else { f64::NAN }
        } else {
            v + self.shift
        };
        self.origin + (u - self.a) * self.scale
    }
}

impl Axis {
    pub fn new(opts: AxisOptions, horizontal: bool) -> Self {
        let mut a = Self {
            opts,
            categories: Vec::new(),
            label_widths: Vec::new(),
            label_font_size: 0.0,
            view: (0.0, 1.0),
            auto: true,
            start: 0.0,
            len: 0.0,
            horizontal,
            ticks: Vec::new(),
            tick_values: Vec::new(),
            time_buf: Vec::new(),
            number: NumberFormat::Auto(2),
            category_step: 1,
        };
        let _ = a.ticks.try_reserve(64);
        let _ = a.tick_values.try_reserve(64);
        let _ = a.time_buf.try_reserve(64);
        a.reset_range();
        a
    }

    /// Applies new options; the view follows the new range.
    pub fn set_options(&mut self, opts: AxisOptions) {
        self.opts = opts;
        self.reset_range();
    }

    /// Back to the configured range (automatic or fixed).
    pub fn reset_range(&mut self) {
        match self.opts.range {
            AxisRange::Fixed(a, b) => {
                self.view = (a.min(b), a.max(b));
                self.auto = false;
            }
            AxisRange::Auto { .. } => self.auto = true,
        }
    }

    pub fn set_categories(&mut self, labels: &[&str]) {
        self.categories.clear();
        self.categories
            .extend(labels.iter().map(|s| (*s).to_owned()));
        self.label_widths.clear();
        self.label_font_size = 0.0;
    }

    /// Number of categories (at least `min`, the largest index used by data).
    pub fn category_count(&self, min: usize) -> usize {
        self.categories.len().max(min)
    }

    /// Applies an automatic range from the data extent `ext` (`None`: no
    /// data); category axes span all `n` categories. `anchored`: the axis
    /// carries the values of bars or areas, so an end at zero gets no
    /// margin (zero is not added to the range).
    pub fn apply_auto(&mut self, ext: Option<(f64, f64)>, n_categories: usize, anchored: bool) {
        if !self.auto {
            return;
        }
        let AxisRange::Auto {
            include_zero,
            margin,
        } = self.opts.range
        else {
            return;
        };
        if self.opts.kind == AxisKind::Category {
            self.view = (0.0, n_categories.max(1) as f64);
            return;
        }
        let Some((mut lo, mut hi)) = ext else {
            self.view = if self.opts.kind == AxisKind::Log {
                (1.0, 10.0)
            } else {
                (0.0, 1.0)
            };
            return;
        };
        if include_zero && self.opts.kind != AxisKind::Log {
            lo = lo.min(0.0);
            hi = hi.max(0.0);
        }
        let m = f64::from(margin.max(0.0));
        if self.opts.kind == AxisKind::Log {
            let (a, b) = (
                lo.max(f64::MIN_POSITIVE).ln(),
                hi.max(f64::MIN_POSITIVE).ln(),
            );
            let (a, b) = if b - a < 1e-9 {
                (a - 0.5, b + 0.5)
            } else {
                (a, b)
            };
            let pad = (b - a) * m;
            self.view = ((a - pad).exp(), (b + pad).exp());
        } else {
            if hi - lo <= f64::EPSILON * hi.abs().max(1.0) {
                let d = if hi == 0.0 { 1.0 } else { hi.abs() * 0.05 };
                lo -= d;
                hi += d;
            }
            let pad = (hi - lo) * m;
            // Keep a zero baseline on the axis edge (bars start there).
            let keep = include_zero || anchored;
            let lo2 = if keep && lo == 0.0 { 0.0 } else { lo - pad };
            let hi2 = if keep && hi == 0.0 { 0.0 } else { hi + pad };
            self.view = (lo2, hi2);
        }
    }

    fn f(&self, v: f64) -> f64 {
        if self.opts.kind == AxisKind::Log {
            if v > 0.0 { v.ln() } else { f64::NAN }
        } else {
            v
        }
    }

    fn f_inv(&self, u: f64) -> f64 {
        if self.opts.kind == AxisKind::Log {
            u.exp()
        } else {
            u
        }
    }

    /// The map from values to pixels in the current view and layout, for
    /// converting many values (`to_px`, `point_px` use it too).
    pub fn px_map(&self) -> PxMap {
        let (a, b) = (self.f(self.view.0), self.f(self.view.1));
        let k = self.len / (b - a);
        // Increasing values go right on x and up on y, unless inverted.
        let forward = self.horizontal != self.opts.inverted;
        PxMap {
            log: self.opts.kind == AxisKind::Log,
            shift: 0.0,
            a,
            origin: if forward {
                self.start
            } else {
                self.start + self.len
            },
            scale: if forward { k } else { -k },
        }
    }

    /// As [`px_map`](Self::px_map), placing points at slot centers on a
    /// category axis.
    pub fn point_map(&self) -> PxMap {
        let mut m = self.px_map();
        if self.opts.kind == AxisKind::Category {
            m.shift = 0.5;
        }
        m
    }

    /// Pixel position of value `v` (NaN when it has none: log of ≤ 0).
    pub fn to_px(&self, v: f64) -> f64 {
        self.px_map().px(v)
    }

    /// Pixel position of a point at `v`: on a category axis, the center of
    /// slot `v`.
    pub fn point_px(&self, v: f64) -> f64 {
        self.point_map().px(v)
    }

    /// Value at pixel position `p`.
    pub fn value_at(&self, p: f64) -> f64 {
        let mut t = if self.horizontal {
            (p - self.start) / self.len
        } else {
            (self.start + self.len - p) / self.len
        };
        if self.opts.inverted {
            t = 1.0 - t;
        }
        let (a, b) = (self.f(self.view.0), self.f(self.view.1));
        self.f_inv(a + t * (b - a))
    }

    /// Pixels per category slot (category axes).
    pub fn slot_px(&self) -> f64 {
        let span = self.view.1 - self.view.0;
        if span > 0.0 { self.len / span } else { 0.0 }
    }

    /// Pans by `dp` pixels (in the direction of increasing pixel position);
    /// whether the view moved. Category axes stay within their categories.
    pub fn pan_px(&mut self, dp: f64, n_categories: usize) -> bool {
        if self.opts.locked || self.len <= 0.0 || dp == 0.0 || !dp.is_finite() {
            return false;
        }
        let sign =
            if self.horizontal { 1.0 } else { -1.0 } * if self.opts.inverted { -1.0 } else { 1.0 };
        let (a, b) = (self.f(self.view.0), self.f(self.view.1));
        let du = -sign * dp / self.len * (b - a);
        let (mut na, mut nb) = (a + du, b + du);
        if self.opts.kind == AxisKind::Category {
            let n = n_categories.max(1) as f64;
            let span = nb - na;
            if span >= n {
                return false; // everything is visible: nothing to pan
            }
            if na < 0.0 {
                (na, nb) = (0.0, span);
            } else if nb > n {
                (na, nb) = (n - span, n);
            }
        }
        self.set_view_checked(na, nb)
    }

    /// Zooms by `factor` (> 1 zooms in) around pixel position `anchor`;
    /// whether the view changed. Category axes do not zoom.
    pub fn zoom_px(&mut self, anchor: f64, factor: f64) -> bool {
        if self.opts.locked
            || self.len <= 0.0
            || !(factor.is_finite() && factor > 0.0)
            || self.opts.kind == AxisKind::Category
        {
            return false;
        }
        let (a, b) = (self.f(self.view.0), self.f(self.view.1));
        let c = self.f(self.value_at(anchor));
        if !c.is_finite() {
            return false;
        }
        let (na, nb) = (c + (a - c) / factor, c + (b - c) / factor);
        let span = nb - na;
        // Keep at least a few ulps and at most a sane range.
        if !(span.is_finite() && span > (a.abs().max(b.abs()) * 1e-12).max(1e-300) && span < 1e300)
        {
            return false;
        }
        self.set_view_checked(na, nb)
    }

    /// Sets the view from `na..nb` in mapped units (ln on a log axis) if
    /// both ends are finite and it differs; the axis stops following data.
    fn set_view_checked(&mut self, na: f64, nb: f64) -> bool {
        let view = (self.f_inv(na), self.f_inv(nb));
        let ok = view.0.is_finite() && view.1.is_finite() && view.0 < view.1;
        #[allow(clippy::float_cmp)] // exact: unchanged view
        let moved = ok && view != self.view;
        if moved {
            self.view = view;
            self.auto = false;
        }
        moved
    }

    /// Category axes show at most one category per [`MIN_SLOT_CSS`]: a
    /// wider view keeps its start (0 for an automatic view) and narrows.
    pub fn limit_category_span(&mut self, n_categories: usize, dpr: f32) {
        if self.opts.kind != AxisKind::Category || self.len <= 0.0 {
            return;
        }
        let max_span = (self.len / f64::from(MIN_SLOT_CSS * dpr)).floor().max(1.0);
        let (lo, hi) = self.view;
        if hi - lo <= max_span {
            return;
        }
        let n = n_categories.max(1) as f64;
        let lo = if self.auto {
            0.0
        } else {
            lo.clamp(0.0, (n - max_span).max(0.0))
        };
        self.view = (lo, lo + max_span);
    }

    /// Computes the ticks and the number format for the current view and
    /// pixel span. `min_spacing` is the smallest distance between ticks.
    pub fn layout_ticks(&mut self, min_spacing: f64) {
        self.ticks.clear();
        let (lo, hi) = (self.view.0.min(self.view.1), self.view.0.max(self.view.1));
        match (self.opts.kind, self.opts.format) {
            (AxisKind::Category, _) => {}
            (_, AxisFormat::Date { utc_offset_min }) => {
                let off = i64::from(utc_offset_min) * 60_000;
                if lo.is_finite() && hi.is_finite() && hi - lo < 1e16 {
                    time_ticks(
                        lo as i64 + off,
                        hi as i64 + off,
                        self.len,
                        min_spacing,
                        &mut self.time_buf,
                    );
                    for &(t, weight) in &self.time_buf {
                        self.ticks.push(TickLabel {
                            value: (t - off) as f64,
                            weight,
                        });
                    }
                }
            }
            (kind, format) => {
                if kind == AxisKind::Log {
                    log_ticks(lo, hi, self.len, min_spacing, &mut self.tick_values);
                } else {
                    linear_ticks(lo, hi, self.len, min_spacing, &mut self.tick_values);
                }
                // Ticks show the decimals their values need (subscript
                // zeros for small values); `number_format` gives the
                // precision of single values (rule labels, bar values).
                let base = match format {
                    AxisFormat::Auto => NumberFormat::Auto(0),
                    other => number_format(other, lo, hi),
                };
                self.number = base.for_ticks(&self.tick_values);
                for &value in &self.tick_values {
                    self.ticks.push(TickLabel {
                        value,
                        weight: TickWeight::Second,
                    });
                }
            }
        }
    }

    /// Writes the label of tick `t`.
    pub fn write_tick(&self, t: TickLabel, out: &mut impl Write) {
        match self.opts.format {
            AxisFormat::Date { utc_offset_min } if self.opts.kind != AxisKind::Category => {
                let off = i64::from(utc_offset_min) * 60_000;
                let _ = format_tick(t.value as i64 + off, t.weight, out);
            }
            _ => self.number.write(out, t.value),
        }
    }

    /// Measures the category labels (when the font size changed) and returns
    /// the widest one.
    pub fn category_label_widths(&mut self, text: &dyn TextMeasurer, font: FontStyleRef) -> f32 {
        #[allow(clippy::float_cmp)] // exact: the size the widths were measured at
        if self.label_font_size != font.size || self.label_widths.len() != self.categories.len() {
            self.label_widths.clear();
            for c in &self.categories {
                self.label_widths.push(text_width(text, c, font));
            }
            self.label_font_size = font.size;
        }
        self.label_widths.iter().copied().fold(0.0, f32::max)
    }

    /// Width of the widest tick label of the last layout.
    pub fn widest_tick(&self, text: &dyn TextMeasurer, font: FontStyleRef) -> f32 {
        let mut w: f32 = 0.0;
        for &t in &self.ticks {
            let mut s = StackStr::<64>::new();
            self.write_tick(t, &mut s);
            w = w.max(text_width(text, s.as_str(), font));
        }
        w
    }
}

/// The number format of a single value `v` (bar and cell texts). `Auto`
/// writes the decimals `v` needs, at most those of automatic precision:
/// `4.2`, `0.3333`, `1234.57`.
pub(crate) fn value_format(format: AxisFormat, v: f64) -> NumberFormat {
    match format {
        AxisFormat::Auto | AxisFormat::Date { .. } => {
            let cap = auto_decimals(v);
            match NumberFormat::Auto(0).for_ticks(&[v]) {
                NumberFormat::Auto(d) => NumberFormat::Auto(d.min(cap)),
                other => other,
            }
        }
        other => number_format(other, v, v),
    }
}

/// The number format of values in `lo..=hi` written as `format`.
pub(crate) fn number_format(format: AxisFormat, lo: f64, hi: f64) -> NumberFormat {
    match format {
        AxisFormat::Auto | AxisFormat::Date { .. } => {
            NumberFormat::Auto(auto_decimals(lo.abs().max(hi.abs())))
        }
        AxisFormat::Fixed(p) => NumberFormat::Fixed(p),
        AxisFormat::Percent(p) => NumberFormat::Percent(p),
        AxisFormat::Compact => NumberFormat::Compact,
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn axis(kind: AxisKind, view: (f64, f64), horizontal: bool) -> Axis {
        let mut a = Axis::new(
            AxisOptions {
                kind,
                range: AxisRange::Fixed(view.0, view.1),
                ..AxisOptions::linear()
            },
            horizontal,
        );
        a.start = 10.0;
        a.len = 100.0;
        a
    }

    #[test]
    fn linear_and_log_mapping() {
        let x = axis(AxisKind::Linear, (0.0, 50.0), true);
        assert_eq!(x.to_px(25.0), 60.0);
        assert_eq!(x.value_at(60.0), 25.0);
        let y = axis(AxisKind::Linear, (0.0, 50.0), false);
        assert_eq!(y.to_px(0.0), 110.0);
        assert_eq!(y.to_px(50.0), 10.0);
        let l = axis(AxisKind::Log, (1.0, 100.0), true);
        assert!((l.to_px(10.0) - 60.0).abs() < 1e-9);
        assert!((l.value_at(60.0) - 10.0).abs() < 1e-9);
        assert!(l.to_px(-1.0).is_nan());
    }

    #[test]
    fn inverted_axis_flips() {
        let mut y = axis(AxisKind::Linear, (0.0, 10.0), false);
        y.opts.inverted = true;
        assert_eq!(y.to_px(0.0), 10.0);
        assert_eq!(y.value_at(110.0), 10.0);
    }

    #[test]
    fn auto_range_with_zero_and_margin() {
        let mut a = Axis::new(
            AxisOptions {
                range: AxisRange::Auto {
                    include_zero: true,
                    margin: 0.1,
                },
                ..AxisOptions::linear()
            },
            false,
        );
        a.apply_auto(Some((2.0, 12.0)), 0, false);
        // The margin is an f32: 0.1 is 0.100000001 in f64.
        assert_eq!(a.view.0, 0.0);
        assert!((a.view.1 - 13.2).abs() < 1e-6);
        a.apply_auto(Some((-5.0, 5.0)), 0, false);
        assert!((a.view.0 + 6.0).abs() < 1e-6 && (a.view.1 - 6.0).abs() < 1e-6);
        let mut c = Axis::new(AxisOptions::category(), true);
        c.apply_auto(None, 7, false);
        assert_eq!(c.view, (0.0, 7.0));
    }

    #[test]
    fn category_pan_is_bounded() {
        let mut c = axis(AxisKind::Category, (0.0, 4.0), true);
        assert!(c.pan_px(-50.0, 10)); // drag left: later categories come in
        assert_eq!(c.view, (2.0, 6.0));
        c.pan_px(-1000.0, 10);
        assert_eq!(c.view, (6.0, 10.0));
        c.pan_px(1000.0, 10);
        assert_eq!(c.view, (0.0, 4.0));
        let mut all = axis(AxisKind::Category, (0.0, 10.0), true);
        assert!(!all.pan_px(30.0, 10));
        assert_eq!(all.view, (0.0, 10.0));
    }

    #[test]
    fn zoom_keeps_the_anchor() {
        let mut x = axis(AxisKind::Linear, (0.0, 100.0), true);
        let before = x.value_at(35.0);
        x.zoom_px(35.0, 2.0);
        assert!((x.value_at(35.0) - before).abs() < 1e-9);
        assert!((x.view.1 - x.view.0 - 50.0).abs() < 1e-9);
    }

    #[test]
    fn ticks_and_formats() {
        let mut x = axis(AxisKind::Linear, (0.0, 1.0), true);
        x.len = 500.0;
        x.layout_ticks(90.0);
        let labels: Vec<String> = x
            .ticks
            .iter()
            .map(|&t| {
                let mut s = String::new();
                x.write_tick(t, &mut s);
                s
            })
            .collect();
        // Ticks take the decimals their values need.
        assert_eq!(labels, ["0.0", "0.2", "0.4", "0.6", "0.8", "1.0"]);
        let mut tiny = axis(AxisKind::Linear, (0.000_010, 0.000_014), true);
        tiny.len = 500.0;
        tiny.layout_ticks(90.0);
        let mut s = String::new();
        tiny.write_tick(tiny.ticks[1], &mut s);
        assert_eq!(s, "0.0₄12");
        let mut d = axis(
            AxisKind::Linear,
            (1_704_067_200_000.0, 1_735_689_600_000.0),
            true,
        );
        d.opts.format = AxisFormat::Date { utc_offset_min: 0 };
        d.len = 600.0;
        d.layout_ticks(60.0);
        let mut s = String::new();
        d.write_tick(d.ticks[0], &mut s);
        assert_eq!(s, "2024");
    }
}
