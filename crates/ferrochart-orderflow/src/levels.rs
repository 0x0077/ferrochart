//! Price levels: prices are stored as level numbers
//! `floor(price / step + 0.5)`, where `step = tick_size × row_size`.

use ferrochart_core::error::{Error, Result};

/// The price grid of an order flow series.
///
/// # Examples
///
/// ```
/// use ferrochart_orderflow::PriceLevels;
///
/// // 0.25 ticks, two ticks per row: rows 0.5 apart.
/// let levels = PriceLevels::new(0.25, 2)?;
/// assert_eq!(levels.level(100.2), Some(200));
/// assert_eq!(levels.level(100.26), Some(201));
/// assert_eq!(levels.price(201), 100.5);
/// assert_eq!(levels.edges(201), (100.25, 100.75));
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PriceLevels {
    step: f64,
}

impl PriceLevels {
    /// Rows of `row_size` ticks of `tick_size`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] if `tick_size` is not finite and positive
    /// or `row_size` is 0.
    pub fn new(tick_size: f64, row_size: u32) -> Result<Self> {
        let step = tick_size * f64::from(row_size);
        if !(tick_size.is_finite() && tick_size > 0.0) || row_size == 0 || !step.is_finite() {
            return Err(Error::InvalidArgument(
                "tick_size must be finite and positive, row_size at least 1",
            ));
        }
        Ok(Self { step })
    }

    /// Height of a row in price.
    pub fn step(&self) -> f64 {
        self.step
    }

    /// The level of `price`; `None` if it is not finite or out of range.
    pub fn level(&self, price: f64) -> Option<i32> {
        let l = (price / self.step + 0.5).floor();
        (l.is_finite() && l >= f64::from(i32::MIN) && l <= f64::from(i32::MAX)).then_some(l as i32)
    }

    /// The price at the center of `level`.
    pub fn price(&self, level: i32) -> f64 {
        f64::from(level) * self.step
    }

    /// The lower and upper edge of `level`.
    pub fn edges(&self, level: i32) -> (f64, f64) {
        let p = self.price(level);
        (p - self.step / 2.0, p + self.step / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_grids() {
        for (tick, rows) in [
            (0.0, 1),
            (-1.0, 1),
            (f64::NAN, 1),
            (f64::INFINITY, 1),
            (1.0, 0),
        ] {
            assert!(PriceLevels::new(tick, rows).is_err(), "{tick} {rows}");
        }
        let l = PriceLevels::new(1.0, 1).unwrap();
        assert_eq!(l.level(f64::NAN), None);
        assert_eq!(l.level(1e300), None);
        assert_eq!(l.level(-0.6), Some(-1));
        assert_eq!(l.level(-0.5), Some(0));
    }
}
