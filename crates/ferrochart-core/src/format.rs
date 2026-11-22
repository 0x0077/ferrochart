//! Number formats of axis and value labels: fixed decimals, automatic
//! precision with subscript zeros, percentages and K/M/B.
//!

use core::fmt::Write;

/// How numbers are written on axes and labels (price axis, `Plot` axes,
/// value labels).
///
/// ```
/// use ferrochart_core::format::NumberFormat;
///
/// let mut s = String::new();
/// NumberFormat::Auto(8).write(&mut s, 0.000_012_34);
/// assert_eq!(s, "0.0₄1234");
/// s.clear();
/// NumberFormat::Compact.write(&mut s, 1_250_000.0);
/// assert_eq!(s, "1.25M");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberFormat {
    /// Fixed decimals (a price series with a set precision).
    Fixed(u8),
    /// Automatic precision with these decimals: subscript zeros below 1.
    Auto(u8),
    /// Fixed decimals with a `%` suffix (percentage scales).
    Percent(u8),
    /// K/M/B suffixes (a volume-only pane).
    Compact,
}

impl NumberFormat {
    /// Writes the label of `v`.
    pub fn write(self, out: &mut impl core::fmt::Write, v: f64) {
        use crate::price_scale::{write_compact, write_price};
        match self {
            Self::Fixed(p) => write_price(out, v, p),
            Self::Auto(p) => write_price_auto(out, v, p),
            Self::Percent(p) => {
                write_price(out, v, p);
                let _ = out.write_char('%');
            }
            Self::Compact => write_compact(out, v),
        }
    }

    /// The format for the axis ticks `ticks`: fixed decimals are widened
    /// (up to 30) until every tick is written exactly, so neighbouring ticks
    /// never read the same when the tick step is finer than the precision.
    #[must_use]
    pub fn for_ticks(self, ticks: &[f64]) -> Self {
        let widen = |p: u8| {
            ticks.iter().fold(p, |d, &t| {
                let mut d = d;
                while d < 30 {
                    let x = t * 10f64.powi(i32::from(d));
                    if (x - x.round()).abs() <= 1e-12 * x.abs().max(1.0) {
                        break;
                    }
                    d += 1;
                }
                d
            })
        };
        match self {
            Self::Fixed(p) => Self::Fixed(widen(p)),
            Self::Auto(p) => Self::Auto(widen(p)),
            Self::Percent(p) => Self::Percent(widen(p)),
            Self::Compact => Self::Compact,
        }
    }
}

/// Decimals of automatic precision for prices of magnitude `magnitude`: at
/// least 2 decimals and 4 significant digits, at most 30; 2 for zero or
/// non-finite magnitudes.
pub fn auto_decimals(magnitude: f64) -> u8 {
    let m = magnitude.abs();
    if !(m.is_finite() && m > 0.0) {
        return 2;
    }
    // floor(log10 m), corrected where log10 is off by an ulp at a power of 10.
    let mut e = m.log10().floor();
    if 10f64.powf(e) > m {
        e -= 1.0;
    } else if 10f64.powf(e + 1.0) <= m {
        e += 1.0;
    }
    (3.0 - e).clamp(2.0, 30.0) as u8
}

/// Writes `price` with `decimals` decimals in the automatic style: below 1,
/// four or more zeros after the point become `0.0` and a subscript count
/// (`0.0₄1234`); with four or more decimals, a price that rounds to zero
/// is `0`.
pub(crate) fn write_price_auto(out: &mut impl Write, price: f64, decimals: u8) {
    if price.is_nan() || price.abs() >= 1.0 {
        crate::price_scale::write_price(out, price, decimals);
        return;
    }
    let mut s = crate::text::StackStr::<64>::new();
    crate::price_scale::write_price(&mut s, price, decimals);
    let text = s.as_str();
    let (sign, unsigned) = text.strip_prefix('-').map_or(("", text), |r| ("-", r));
    let Some(fraction) = unsigned.strip_prefix("0.") else {
        let _ = out.write_str(text); // rounded up to 1
        return;
    };
    let digits = fraction.trim_start_matches('0');
    let zeros = fraction.len() - digits.len();
    if zeros < 4 {
        let _ = out.write_str(text);
    } else if digits.is_empty() {
        let _ = out.write_char('0');
    } else {
        let _ = out.write_str(sign);
        let _ = out.write_str("0.0");
        let (tens, ones) = (zeros / 10, zeros % 10);
        for d in [tens, ones].into_iter().skip(usize::from(tens == 0)) {
            let _ = out.write_char(char::from_u32(0x2080 + d as u32).unwrap_or('0'));
        }
        let _ = out.write_str(digits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_labels_widen_to_the_tick_step() {
        let ticks: Vec<f64> = (0..5).map(|m| f64::from(m) * 0.0025 + 1.0).collect();
        assert_eq!(
            NumberFormat::Fixed(2).for_ticks(&ticks),
            NumberFormat::Fixed(4)
        );
        assert_eq!(
            NumberFormat::Percent(0).for_ticks(&[0.5, 1.0]),
            NumberFormat::Percent(1)
        );
        // Coarser steps keep the precision; it is never narrowed.
        assert_eq!(
            NumberFormat::Fixed(2).for_ticks(&[100.0, 150.0]),
            NumberFormat::Fixed(2)
        );
        assert_eq!(
            NumberFormat::Fixed(0).for_ticks(&[1e6, 2e6]),
            NumberFormat::Fixed(0)
        );
        assert_eq!(
            NumberFormat::Fixed(2).for_ticks(&[1.0 / 3.0]),
            NumberFormat::Fixed(12) // exact within 1e-12
        );
        assert_eq!(
            NumberFormat::Auto(8).for_ticks(&[0.000_012_345, 0.000_012_35]),
            NumberFormat::Auto(9)
        );
        assert_eq!(
            NumberFormat::Compact.for_ticks(&[0.5]),
            NumberFormat::Compact
        );
    }
}
