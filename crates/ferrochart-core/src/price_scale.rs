//! Price scales: value-to-y mapping (`y = a * f(p) + b`, `f` = identity or `ln`),
//! auto-scaling, "nice number" ticks and price formatting.
//!

use core::fmt::Write;

use crate::error::{Error, Result};
use crate::frame::{PriceMapping, YTransform};

/// Price scale mode.
///
/// In `Percentage` and `IndexedTo100` mode every series of the scale is
/// normalized to its own first visible value (the close of the first visible
/// bar, or the first visible value of a value series), so series with
/// different price levels can be compared. The scale's range, ticks,
/// labels and crosshair price are then in percent / index points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PriceScaleMode {
    /// Linear prices.
    #[default]
    Normal,
    /// Logarithmic prices.
    Logarithmic,
    /// Percent change from the first visible value: `(p - base) / |base| × 100`.
    Percentage,
    /// Indexed to 100 at the first visible value: `p / |base| × 100`.
    IndexedTo100,
}

impl PriceScaleMode {
    /// Whether values are normalized to a per-series base.
    pub fn is_relative(self) -> bool {
        matches!(self, Self::Percentage | Self::IndexedTo100)
    }
}

/// The linear map `d = k·p + c` from a series' prices to the values its scale
/// shows: the identity, or the percent change from /
/// index to 100 at the series' base value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Normalize {
    k: f64,
    c: f64,
}

impl Normalize {
    pub(crate) const IDENTITY: Self = Self { k: 1.0, c: 0.0 };

    /// The map for `mode` with base value `base`; the identity for absolute
    /// modes, `None` if a relative mode has no usable base (zero or not finite).
    pub(crate) fn new(mode: PriceScaleMode, base: Option<f64>) -> Option<Self> {
        if !mode.is_relative() {
            return Some(Self::IDENTITY);
        }
        let base = base.filter(|b| b.is_finite() && *b != 0.0)?;
        let k = 100.0 / base.abs();
        let c = if mode == PriceScaleMode::Percentage {
            -100.0 * base.signum()
        } else {
            0.0
        };
        Some(Self { k, c })
    }

    /// Displayed value of price `p`.
    pub(crate) fn apply(self, p: f64) -> f64 {
        self.k * p + self.c
    }

    /// Price of displayed value `d`.
    pub(crate) fn invert(self, d: f64) -> f64 {
        (d - self.c) / self.k
    }

    /// The transform of a series from the transform of its scale: since
    /// `y = a·(d − d_ref) + b` and `d = k·p + c`, the series uses `a·k` and the
    /// price reference `invert(d_ref)`.
    pub(crate) fn series_transform(self, scale: YTransform, a: f64) -> YTransform {
        if self == Self::IDENTITY {
            return scale;
        }
        YTransform {
            a: (a * self.k) as f32,
            view_ref: self.invert(scale.view_ref),
            ..scale
        }
    }
}

/// Which side of a pane a price scale sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScaleSide {
    /// Left axis.
    Left,
    /// Right axis.
    #[default]
    Right,
}

/// A price scale.
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::price_scale::{PriceScaleMode, ScaleSide};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let pane = chart.main_pane();
/// let scale = chart.price_scale(pane, ScaleSide::Right).unwrap();
/// scale.set_mode(PriceScaleMode::Logarithmic)?;
/// scale.set_range(10.0, 1000.0)?; // also turns auto-scale off
/// assert!(!scale.auto_scale());
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct PriceScale {
    mode: PriceScaleMode,
    auto_scale: bool,
    inverted: bool,
    margins: (f32, f32),
    range: (f64, f64),
    pub(crate) changed: bool,
}

impl PriceScale {
    pub(crate) fn new(margins: (f32, f32)) -> Self {
        Self {
            mode: PriceScaleMode::Normal,
            auto_scale: true,
            inverted: false,
            margins,
            range: (0.0, 1.0),
            changed: true,
        }
    }

    /// Current mode.
    pub fn mode(&self) -> PriceScaleMode {
        self.mode
    }

    /// Sets the mode. Switching between price modes (`Normal`,
    /// `Logarithmic`) and relative modes (`Percentage`, `IndexedTo100`)
    /// turns auto-scaling on, because the old range is in other units.
    ///
    /// # Errors
    ///
    /// None so far; the `Result` leaves room for modes with preconditions.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::{Chart, ChartOptions};
    /// use ferrochart_core::price_scale::{PriceScaleMode, ScaleSide};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let pane = chart.main_pane();
    /// let scale = chart.price_scale(pane, ScaleSide::Right).unwrap();
    /// scale.set_range(90.0, 110.0)?;
    /// scale.set_mode(PriceScaleMode::Percentage)?;
    /// assert!(scale.auto_scale());
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn set_mode(&mut self, mode: PriceScaleMode) -> Result<()> {
        if mode.is_relative() != self.mode.is_relative() {
            self.auto_scale = true;
        }
        self.mode = mode;
        self.changed = true;
        Ok(())
    }

    /// Whether the range follows the visible data.
    pub fn auto_scale(&self) -> bool {
        self.auto_scale
    }

    /// Turns auto-scaling on or off.
    pub fn set_auto_scale(&mut self, on: bool) {
        self.auto_scale = on;
        self.changed = true;
    }

    /// Whether higher prices are drawn lower.
    pub fn inverted(&self) -> bool {
        self.inverted
    }

    /// Inverts the scale.
    pub fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
        self.changed = true;
    }

    /// Top and bottom margins as fractions of the pane height.
    pub fn margins(&self) -> (f32, f32) {
        self.margins
    }

    /// Sets the top and bottom margins.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] unless both are in `[0, 1)` and their sum is `< 1`.
    pub fn set_margins(&mut self, top: f32, bottom: f32) -> Result<()> {
        let ok = |m: f32| (0.0..1.0).contains(&m);
        if !(ok(top) && ok(bottom) && top + bottom < 1.0) {
            return Err(Error::InvalidArgument(
                "margins must be in [0, 1) and sum < 1",
            ));
        }
        self.margins = (top, bottom);
        self.changed = true;
        Ok(())
    }

    /// Current price range `(low, high)` (without margins).
    pub fn range(&self) -> (f64, f64) {
        self.range
    }

    /// Sets the price range and turns auto-scaling off.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] unless `low < high`, both finite, and (in
    /// logarithmic mode) `low > 0`.
    pub fn set_range(&mut self, low: f64, high: f64) -> Result<()> {
        if !(low.is_finite() && high.is_finite() && low < high) {
            return Err(Error::InvalidArgument(
                "price range must be finite with low < high",
            ));
        }
        if self.mode == PriceScaleMode::Logarithmic && low <= 0.0 {
            return Err(Error::InvalidArgument(
                "logarithmic price range must be positive",
            ));
        }
        self.range = (low, high);
        self.auto_scale = false;
        self.changed = true;
        Ok(())
    }

    fn mapping(&self) -> PriceMapping {
        match self.mode {
            PriceScaleMode::Logarithmic => PriceMapping::Log,
            _ => PriceMapping::Linear,
        }
    }

    fn f(&self, p: f64) -> f64 {
        match self.mapping() {
            PriceMapping::Linear => p,
            PriceMapping::Log => p.ln(),
        }
    }

    fn f_inv(&self, v: f64) -> f64 {
        match self.mapping() {
            PriceMapping::Linear => v,
            PriceMapping::Log => v.exp(),
        }
    }

    /// Auto-scale: sets the range to `(low, high)`, widening degenerate ranges.
    pub(crate) fn fit(&mut self, low: f64, high: f64) {
        if !(self.auto_scale && low.is_finite() && high.is_finite()) {
            return;
        }
        let (mut lo, mut hi) = (low.min(high), low.max(high));
        if self.mapping() == PriceMapping::Log {
            if hi <= 0.0 {
                return;
            }
            lo = lo.max(hi * 1e-9);
        }
        if hi - lo <= hi.abs() * 1e-12 {
            let pad = if hi == 0.0 { 1.0 } else { hi.abs() * 0.01 };
            lo -= pad;
            hi += pad;
            if self.mapping() == PriceMapping::Log {
                lo = lo.max(hi * 1e-9);
            }
        }
        self.range = (lo, hi);
    }

    /// Price at the vertical center of the current range.
    pub(crate) fn center_price(&self) -> f64 {
        self.f_inv((self.f(self.range.0) + self.f(self.range.1)) * 0.5)
    }

    fn set_interactive_range(&mut self, flo: f64, fhi: f64) {
        let (lo, hi) = (self.f_inv(flo), self.f_inv(fhi));
        if lo.is_finite() && hi.is_finite() && hi > lo && hi - lo > hi.abs().max(lo.abs()) * 1e-12 {
            self.range = (lo, hi);
            self.auto_scale = false;
            self.changed = true;
        }
    }

    /// Scales the range by `factor` (> 1 shows more) around `center`, turning
    /// auto-scale off (price-axis drag, Alt + wheel).
    pub(crate) fn zoom_around(&mut self, factor: f64, center: f64) {
        let c = self.f(center);
        if !(factor.is_finite() && factor > 0.0 && c.is_finite()) {
            return;
        }
        let (fl, fh) = (self.f(self.range.0), self.f(self.range.1));
        self.set_interactive_range(c - (c - fl) * factor, c + (fh - c) * factor);
    }

    /// Moves the content down by `dy` px in a pane of `height` px, turning
    /// auto-scale off.
    pub(crate) fn pan_px(&mut self, dy: f64, height: f64) {
        let usable = height * (1.0 - f64::from(self.margins.0) - f64::from(self.margins.1));
        if !(usable > 0.0 && dy.is_finite()) || dy == 0.0 {
            return;
        }
        let (fl, fh) = (self.f(self.range.0), self.f(self.range.1));
        let d = (fh - fl) * dy / usable;
        let d = if self.inverted { -d } else { d };
        self.set_interactive_range(fl + d, fh + d);
    }

    /// The f32 transform for a pane spanning `top..top + height` (physical
    /// px), for the values the scale shows; series of relative scales use
    /// [`series_transform`](Self::series_transform).
    pub(crate) fn transform(&self, top: f64, height: f64) -> YTransform {
        self.transform_and_a(top, height).0
    }

    /// The transform of a series normalized by `norm`.
    pub(crate) fn series_transform(&self, top: f64, height: f64, norm: Normalize) -> YTransform {
        let (t, a) = self.transform_and_a(top, height);
        norm.series_transform(t, a)
    }

    fn transform_and_a(&self, top: f64, height: f64) -> (YTransform, f64) {
        let (mt, mb) = (f64::from(self.margins.0), f64::from(self.margins.1));
        let (fl, fh) = (self.f(self.range.0), self.f(self.range.1));
        let usable = height * (1.0 - mt - mb);
        let span = (fh - fl).max(f64::MIN_POSITIVE);
        let a = if self.inverted {
            usable / span
        } else {
            -usable / span
        };
        let view = (fl + fh) * 0.5;
        let b = top + mt * height + usable * 0.5;
        let t = YTransform {
            a: a as f32,
            b: b as f32,
            view_ref: self.f_inv(view),
            mapping: self.mapping(),
        };
        (t, a)
    }

    /// y (f64) of price `p` in a pane spanning `top..top + height`.
    pub(crate) fn price_to_y(&self, p: f64, top: f64, height: f64) -> f64 {
        let (mt, mb) = (f64::from(self.margins.0), f64::from(self.margins.1));
        let (fl, fh) = (self.f(self.range.0), self.f(self.range.1));
        let usable = height * (1.0 - mt - mb);
        let t = (self.f(p) - fl) / (fh - fl).max(f64::MIN_POSITIVE);
        let from_top = mt * height + usable * (1.0 - t);
        if self.inverted {
            top + height - from_top
        } else {
            top + from_top
        }
    }

    /// Price at y in a pane spanning `top..top + height`.
    pub(crate) fn y_to_price(&self, y: f64, top: f64, height: f64) -> f64 {
        let (mt, mb) = (f64::from(self.margins.0), f64::from(self.margins.1));
        let (fl, fh) = (self.f(self.range.0), self.f(self.range.1));
        let usable = (height * (1.0 - mt - mb)).max(f64::MIN_POSITIVE);
        let from_top = if self.inverted {
            top + height - y
        } else {
            y - top
        };
        let t = 1.0 - (from_top - mt * height) / usable;
        self.f_inv(fl + t * (fh - fl))
    }

    /// Tick prices for a pane of `height` px, at least `min_spacing` px apart,
    /// written to `out` (cleared first, sorted ascending). No allocation once
    /// `out` has capacity.
    pub(crate) fn ticks(&self, height: f64, min_spacing: f64, out: &mut Vec<f64>) {
        out.clear();
        if height <= 0.0 || min_spacing <= 0.0 {
            return;
        }
        let p_a = self.y_to_price(0.0, 0.0, height);
        let p_b = self.y_to_price(height, 0.0, height);
        let (lo, hi) = (p_a.min(p_b), p_a.max(p_b));
        if self.mapping() == PriceMapping::Log && lo > 0.0 {
            log_ticks(lo, hi, height, min_spacing, out);
        } else {
            linear_ticks(lo, hi, height, min_spacing, out);
        }
    }
}

/// Ticks for values `lo..=hi` on a linear axis `length` px long, at least
/// `min_spacing` px apart: multiples of a nice step `{1, 2, 2.5, 5} × 10^k`.
/// Written to `out` (cleared first, ascending); at most 1000 ticks;
/// no allocation once `out` has capacity.
///
/// ```
/// use ferrochart_core::price_scale::linear_ticks;
///
/// let mut out = Vec::new();
/// linear_ticks(0.0, 100.0, 500.0, 100.0, &mut out);
/// assert_eq!(out, [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
/// ```
pub fn linear_ticks(lo: f64, hi: f64, length: f64, min_spacing: f64, out: &mut Vec<f64>) {
    let to_px = |p: f64| (p - lo) / (hi - lo) * length;
    nice_ticks(lo, hi, length, min_spacing, to_px, out);
}

/// Ticks for positive values `lo..=hi` on a logarithmic axis `length` px
/// long, at least `min_spacing` px apart: powers of ten, then 2 and 5 of each
/// decade; ranges under one decade use linear nice steps spaced on the log
/// axis. Written to `out` (cleared first, ascending); empty for `lo <= 0`.
///
/// ```
/// use ferrochart_core::price_scale::log_ticks;
///
/// let mut out = Vec::new();
/// log_ticks(1.0, 1000.0, 600.0, 30.0, &mut out);
/// assert!(out.contains(&1.0) && out.contains(&10.0) && out.contains(&1000.0));
/// ```
pub fn log_ticks(lo: f64, hi: f64, length: f64, min_spacing: f64, out: &mut Vec<f64>) {
    out.clear();
    if lo.is_nan() || lo <= 0.0 {
        return;
    }
    let (fl, fh) = (lo.ln(), hi.ln());
    let to_px = |p: f64| (p.ln() - fl) / (fh - fl) * length;
    if (hi / lo).log10() >= 1.0 {
        if !(hi.is_finite() && lo < hi && length > 0.0 && min_spacing > 0.0) {
            return;
        }
        let d0 = lo.log10().floor() as i32;
        let d1 = hi.log10().ceil() as i32;
        // Powers of ten first, then 2 and 5 in each decade, each only if spaced.
        for mults in [&[1.0][..], &[2.0, 5.0][..]] {
            for d in d0..=d1 {
                for &m in mults {
                    let p = m * 10f64.powi(d);
                    if (lo..=hi).contains(&p) && out.len() < 1000 {
                        push_spaced(out, p, to_px, min_spacing);
                    }
                }
            }
        }
        out.sort_unstable_by(f64::total_cmp);
    } else {
        nice_ticks(lo, hi, length, min_spacing, to_px, out);
    }
}

/// Nice-step ticks in `lo..=hi`, spaced by `to_px`.
fn nice_ticks(
    lo: f64,
    hi: f64,
    length: f64,
    min_spacing: f64,
    to_px: impl Fn(f64) -> f64,
    out: &mut Vec<f64>,
) {
    out.clear();
    if length <= 0.0 || min_spacing <= 0.0 || !(lo.is_finite() && hi.is_finite()) || lo >= hi {
        return;
    }
    let max_ticks = (length / min_spacing).floor().max(1.0);
    let mut step = nice_step((hi - lo) / max_ticks);
    // A step close to the whole range may have no multiple inside it
    // (short panes): fall back to smaller nice steps until one fits.
    for _ in 0..8 {
        if (lo / step).ceil() <= (hi / step).floor() {
            break;
        }
        step = nice_step_below(step);
    }
    let first = (lo / step).ceil() as i64;
    let last = (hi / step).floor() as i64;
    for m in first..=last {
        if out.len() >= 1000 {
            break;
        }
        push_spaced(out, m as f64 * step, &to_px, min_spacing);
    }
    out.sort_unstable_by(f64::total_cmp);
}

/// Pushes `p` unless an existing tick is closer than `min_spacing` px.
fn push_spaced(out: &mut Vec<f64>, p: f64, y: impl Fn(f64) -> f64, min_spacing: f64) {
    let yp = y(p);
    if out.iter().all(|&q| (y(q) - yp).abs() >= min_spacing) {
        out.push(p);
    }
}

/// Smallest "nice" step `{1, 2, 2.5, 5} × 10^k` that is `>= raw`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::price_scale::nice_step;
///
/// assert_eq!(nice_step(0.7), 1.0);
/// assert_eq!(nice_step(2.2), 2.5);
/// assert_eq!(nice_step(31.0), 50.0);
/// assert_eq!(nice_step(0.012), 0.02);
/// ```
pub fn nice_step(raw: f64) -> f64 {
    if !(raw.is_finite() && raw > 0.0) {
        return 1.0;
    }
    let exp = raw.log10().floor() as i32;
    let base = 10f64.powi(exp);
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] {
        let step = m * base;
        if step >= raw * (1.0 - 1e-12) {
            return step;
        }
    }
    10.0 * base
}

/// A nice step roughly 2–2.5× smaller than `step`.
fn nice_step_below(step: f64) -> f64 {
    nice_step(step * 0.39)
}

/// Formats prices for axis labels. Writes into a caller-provided buffer, so the
/// hot path does not allocate once the buffer has capacity.
///
/// # Examples
///
/// ```
/// use ferrochart_core::price_scale::{DefaultPriceFormatter, PriceFormatter};
///
/// let mut s = String::new();
/// DefaultPriceFormatter { precision: Some(2) }.format(1234.5, &mut s);
/// assert_eq!(s, "1234.50");
/// ```
pub trait PriceFormatter: Send {
    /// Appends the label of `price` to `out`.
    fn format(&self, price: f64, out: &mut String);
}

/// Fixed or automatic decimals; never prints `-0.00`.
///
/// With `precision: None` each price gets at least 2 decimals and 4
/// significant digits, and prices below 1 with four or more zeros after the
/// point are written with a subscript count (`0.0₄1234` is `0.00001234`).
///
/// ```
/// use ferrochart_core::price_scale::{DefaultPriceFormatter, PriceFormatter};
///
/// let auto = DefaultPriceFormatter { precision: None };
/// for (price, label) in [(63412.5, "63412.50"), (0.0123456, "0.01235"), (0.00001234, "0.0₄1234")] {
///     let mut s = String::new();
///     auto.format(price, &mut s);
///     assert_eq!(s, label);
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct DefaultPriceFormatter {
    /// Decimal places; `None` is automatic.
    pub precision: Option<u8>,
}

/// Writes `price` with `precision` decimals, avoiding negative zero.
pub(crate) fn write_price(out: &mut impl Write, price: f64, precision: u8) {
    let scale = 10f64.powi(i32::from(precision));
    let p = if (price * scale).round() == 0.0 {
        0.0
    } else {
        price
    };
    let _ = write!(out, "{:.*}", usize::from(precision), p);
}

impl PriceFormatter for DefaultPriceFormatter {
    fn format(&self, price: f64, out: &mut String) {
        match self.precision {
            Some(p) => write_price(out, price, p),
            None => {
                crate::format::write_price_auto(out, price, crate::format::auto_decimals(price));
            }
        }
    }
}

/// Volume-style formatter with K/M/B suffixes (`1.25M`).
///
/// # Examples
///
/// ```
/// use ferrochart_core::price_scale::{CompactFormatter, PriceFormatter};
///
/// let mut s = String::new();
/// CompactFormatter.format(1_250_000.0, &mut s);
/// assert_eq!(s, "1.25M");
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct CompactFormatter;

impl PriceFormatter for CompactFormatter {
    fn format(&self, v: f64, out: &mut String) {
        write_compact(out, v);
    }
}

/// Writes `v` with a K/M/B suffix and at most two decimals, trailing zeros of
/// the fraction trimmed ("1.5K", "2"). Does not allocate.
pub(crate) fn write_compact(out: &mut impl Write, v: f64) {
    let a = v.abs();
    let (x, suffix) = if a >= 1e9 {
        (v / 1e9, "B")
    } else if a >= 1e6 {
        (v / 1e6, "M")
    } else if a >= 1e3 {
        (v / 1e3, "K")
    } else {
        (v, "")
    };
    let mut s = crate::text::StackStr::<48>::new();
    write_price(&mut s, x, 2);
    let mut digits = s.as_str();
    // Trim trailing zeros of the fraction ("1.50K" -> "1.5K", "2.00" -> "2").
    if digits.contains('.') {
        digits = digits.trim_end_matches('0').trim_end_matches('.');
    }
    let _ = out.write_str(digits);
    let _ = out.write_str(suffix);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{auto_decimals, write_price_auto};
    use proptest::prelude::*;

    fn auto(price: f64, decimals: u8) -> String {
        let mut s = String::new();
        write_price_auto(&mut s, price, decimals);
        s
    }

    #[test]
    fn auto_decimals_keep_four_significant_digits() {
        assert_eq!(auto_decimals(63_412.5), 2);
        assert_eq!(auto_decimals(1.2345), 3);
        assert_eq!(auto_decimals(0.012_345), 5);
        assert_eq!(auto_decimals(0.000_012_34), 8);
        assert_eq!(auto_decimals(-0.5), 4);
        assert_eq!(auto_decimals(1e-40), 30);
        assert_eq!(auto_decimals(0.0), 2);
        assert_eq!(auto_decimals(f64::NAN), 2);
        for k in -20..10 {
            let p = 10f64.powi(k);
            assert_eq!(auto_decimals(p), (3 - k).clamp(2, 30) as u8, "1e{k}");
        }
    }

    #[test]
    fn auto_writes_subscript_zeros() {
        assert_eq!(auto(63_412.5, 2), "63412.50");
        assert_eq!(auto(1.234_56, 3), "1.235");
        assert_eq!(auto(0.012_345_6, 5), "0.01235");
        assert_eq!(auto(0.000_123_4, 7), "0.0001234");
        assert_eq!(auto(0.000_012_34, 8), "0.0₄1234");
        assert_eq!(auto(-0.000_012_34, 8), "-0.0₄1234");
        assert_eq!(auto(1.234e-15, 18), "0.0₁₄1234");
        assert_eq!(auto(0.0, 8), "0");
        assert_eq!(auto(0.0, 3), "0.000");
        assert_eq!(auto(-1e-12, 8), "0");
        assert_eq!(auto(0.999_99, 2), "1.00");
        assert_eq!(auto(f64::NAN, 2), "NaN");
    }

    #[test]
    fn no_negative_zero() {
        let mut s = String::new();
        write_price(&mut s, -0.001, 2);
        assert_eq!(s, "0.00");
    }

    #[test]
    fn log_ticks_cover_decades() {
        let mut ps = PriceScale::new((0.0, 0.0));
        ps.set_mode(PriceScaleMode::Logarithmic).unwrap();
        ps.set_range(1.0, 10_000.0).unwrap();
        let mut out = Vec::new();
        ps.ticks(800.0, 25.0, &mut out);
        for p in [1.0, 10.0, 100.0, 1000.0, 10_000.0] {
            assert!(out.contains(&p), "{p} in {out:?}");
        }
    }

    proptest! {
        /// Linear ticks are evenly spaced nice numbers, at least `min` px apart,
        /// and there is at least one tick for any sane range.
        #[test]
        fn linear_ticks_are_nice(lo in -1e6f64..1e6, span in 1e-6f64..1e6, h in 50.0f64..2000.0) {
            let mut ps = PriceScale::new((0.1, 0.1));
            ps.fit(lo, lo + span);
            let mut out = Vec::new();
            ps.ticks(h, 30.0, &mut out);
            prop_assert!(!out.is_empty());
            for w in out.windows(2) {
                let dy = (ps.price_to_y(w[0], 0.0, h) - ps.price_to_y(w[1], 0.0, h)).abs();
                prop_assert!(dy >= 30.0 - 1e-6);
            }
        }

        /// The f32 transform agrees with the f64 mapping to well under a pixel.
        #[test]
        fn transform_matches_f64(lo in 1.0f64..1e5, span in 1e-3f64..1e5, t in 0.0f64..1.0, log: bool) {
            let mut ps = PriceScale::new((0.1, 0.05));
            if log { ps.set_mode(PriceScaleMode::Logarithmic).unwrap(); }
            ps.fit(lo, lo + span);
            let p = lo + t * span;
            let yt = ps.transform(10.0, 600.0);
            let y32 = crate::transform_helpers::y_of_price(&yt, p);
            prop_assert!((f64::from(y32) - ps.price_to_y(p, 10.0, 600.0)).abs() < 0.01);
        }
    }
}
