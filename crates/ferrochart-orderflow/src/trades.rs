//! Trade input: time-and-sales columns and the tick
//! rule for trades whose aggressor is unknown.

use ferrochart_core::data::Timestamp;
use ferrochart_core::error::{Error, Result};

/// The aggressor of a trade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Side {
    /// A buyer lifted the ask.
    Buy,
    /// A seller hit the bid.
    Sell,
    /// Not reported: classified by the tick rule (see [`TickRule`]).
    #[default]
    Unknown,
}

/// Trades as columns of equal length.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickColumns {
    /// Trade times, UTC ms, non-decreasing.
    pub time: Vec<Timestamp>,
    /// Prices.
    pub price: Vec<f64>,
    /// Sizes (≥ 0; 0 is ignored).
    pub size: Vec<f64>,
    /// Aggressors; empty: all [`Side::Unknown`].
    pub side: Vec<Side>,
}

impl TickColumns {
    /// Number of trades.
    pub fn len(&self) -> usize {
        self.time.len()
    }

    /// Whether there are no trades.
    pub fn is_empty(&self) -> bool {
        self.time.is_empty()
    }

    /// Trade `i`.
    pub(crate) fn get(&self, i: usize) -> (Timestamp, f64, f64, Side) {
        (
            self.time[i],
            self.price[i],
            self.size[i],
            self.side.get(i).copied().unwrap_or_default(),
        )
    }

    /// Checks lengths, values and order; `after`: no trade may be earlier.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for columns of different lengths, a
    /// non-finite price or a negative or non-finite size;
    /// [`Error::TimeNotIncreasing`] for a time earlier than the previous
    /// one (or than `after`).
    pub(crate) fn check(&self, after: Option<Timestamp>) -> Result<()> {
        let n = self.time.len();
        if self.price.len() != n
            || self.size.len() != n
            || !(self.side.is_empty() || self.side.len() == n)
        {
            return Err(Error::InvalidArgument(
                "trade columns must have the same length",
            ));
        }
        let mut prev = after.unwrap_or(Timestamp::MIN);
        for (i, &t) in self.time.iter().enumerate() {
            if t < prev {
                return Err(Error::TimeNotIncreasing { index: i });
            }
            prev = t;
        }
        if self.price.iter().any(|p| !p.is_finite()) {
            return Err(Error::InvalidArgument("trade prices must be finite"));
        }
        if self.size.iter().any(|s| !(s.is_finite() && *s >= 0.0)) {
            return Err(Error::InvalidArgument(
                "trade sizes must be finite and non-negative",
            ));
        }
        Ok(())
    }
}

/// Classifies trades of unknown side by the tick rule: a trade above the
/// previous price is a buy, below it a sell, at the same price the same as
/// the previous trade. The first unknown trade with no reference is a buy.
///
/// # Examples
///
/// ```
/// use ferrochart_orderflow::{Side, TickRule};
///
/// let mut rule = TickRule::default();
/// assert_eq!(rule.classify(10.0, Side::Unknown), Side::Buy);
/// assert_eq!(rule.classify(9.5, Side::Unknown), Side::Sell);
/// assert_eq!(rule.classify(9.5, Side::Unknown), Side::Sell);
/// assert_eq!(rule.classify(9.5, Side::Buy), Side::Buy);
/// assert_eq!(rule.classify(9.5, Side::Unknown), Side::Buy);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickRule {
    last: Option<(f64, Side)>,
}

impl TickRule {
    /// The side of a trade at `price` reported as `side` (never `Unknown`).
    pub fn classify(&mut self, price: f64, side: Side) -> Side {
        let side = match (side, self.last) {
            (Side::Unknown, Some((p, s))) => match price.partial_cmp(&p) {
                Some(core::cmp::Ordering::Greater) => Side::Buy,
                Some(core::cmp::Ordering::Less) => Side::Sell,
                _ => s,
            },
            (Side::Unknown, None) => Side::Buy,
            (s, _) => s,
        };
        self.last = Some((price, side));
        side
    }
}
