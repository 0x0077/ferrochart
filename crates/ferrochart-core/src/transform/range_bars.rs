//! Range Bars (rules in `docs/book/src/series.md`).

use crate::error::Result;
use crate::frame::SeriesKind;
use crate::plugin::{InputBar, SeriesTransform, TransformBar, TransformOutput};

use super::{bar_kind, emit, fresh_volume, positive};

/// Bars whose high and low are a fixed distance apart. The last bar is
/// forming.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::SeriesOptions;
/// use ferrochart_core::transform::RangeBars;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.add_transform_series(
///     chart.main_pane(),
///     Box::new(RangeBars::new(0.5)?),
///     SeriesOptions::default(),
/// )?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct RangeBars {
    range: f64,
    kind: SeriesKind,
    /// The forming bar.
    bar: Option<TransformBar>,
}

impl RangeBars {
    /// Bars of `range` from high to low.
    ///
    /// # Errors
    ///
    /// [`crate::Error::InvalidArgument`] unless `range` is finite and positive.
    pub fn new(range: f64) -> Result<Self> {
        Ok(Self {
            range: positive(range, "the range must be positive")?,
            kind: SeriesKind::Candlestick,
            bar: None,
        })
    }

    /// Drawn as `kind` (`Candlestick`, the default, or `OhlcBar`).
    ///
    /// # Errors
    ///
    /// [`crate::Error::InvalidArgument`] for other kinds.
    pub fn with_kind(mut self, kind: SeriesKind) -> Result<Self> {
        self.kind = bar_kind(kind)?;
        Ok(self)
    }

    /// Moves the forming bar `b` to price `p`; `false` if the output is full.
    fn walk(
        &self,
        b: &mut Option<TransformBar>,
        p: f64,
        v: f64,
        out: &mut TransformOutput,
    ) -> bool {
        let Some(cur) = b.as_mut() else {
            *b = Some(TransformBar {
                open: p,
                high: p,
                low: p,
                close: p,
                volume: fresh_volume(v),
                flag: 0,
            });
            return true;
        };
        let r = self.range;
        loop {
            let q = if p > cur.low + r {
                let q = cur.low + r;
                cur.high = q;
                q
            } else if p < cur.high - r {
                let q = cur.high - r;
                cur.low = q;
                q
            } else {
                break;
            };
            cur.close = q;
            if !emit(out, *cur) {
                return false;
            }
            *cur = TransformBar {
                open: q,
                high: q,
                low: q,
                close: q,
                volume: fresh_volume(v),
                flag: 0,
            };
        }
        cur.high = cur.high.max(p);
        cur.low = cur.low.min(p);
        cur.close = p;
        true
    }

    fn step(&self, b: &mut Option<TransformBar>, bar: InputBar, out: &mut TransformOutput) {
        let path = if bar.close >= bar.open {
            [bar.open, bar.low, bar.high, bar.close]
        } else {
            [bar.open, bar.high, bar.low, bar.close]
        };
        for p in path {
            if !self.walk(b, p, bar.volume, out) {
                return;
            }
        }
        if let Some(cur) = b.as_mut() {
            cur.volume += bar.volume;
        }
    }
}

impl SeriesTransform for RangeBars {
    fn name(&self) -> &'static str {
        "range_bars"
    }

    fn output_kind(&self) -> SeriesKind {
        self.kind
    }

    fn push(&mut self, bar: InputBar, out: &mut TransformOutput) {
        let mut b = self.bar;
        self.step(&mut b, bar, out);
        self.bar = b;
    }

    fn peek(&self, bar: InputBar, out: &mut TransformOutput) {
        let mut b = self.bar;
        self.step(&mut b, bar, out);
        if let Some(cur) = b {
            emit(out, cur);
        }
    }

    fn reset(&mut self) {
        self.bar = None;
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::transform::tests::{run, tick};

    fn ohlc(b: &TransformBar) -> [f64; 4] {
        [b.open, b.high, b.low, b.close]
    }

    #[test]
    fn bars_close_at_the_range() {
        let mut r = RangeBars::new(1.0).unwrap();
        let bars = [tick(0, 10.0), tick(1, 10.6), tick(2, 12.3), tick(3, 11.8)];
        let out = run(&mut r, &bars);
        let got: Vec<_> = out.iter().map(ohlc).collect();
        assert_eq!(
            got,
            [
                [10.0, 11.0, 10.0, 11.0],
                [11.0, 12.0, 11.0, 12.0],
                // Forming: opened at 12, up to 12.3, back to 11.8.
                [12.0, 12.3, 11.8, 11.8],
            ]
        );
        assert_eq!(out[2].volume, 2.0);
    }

    #[test]
    fn bars_follow_the_intrabar_path() {
        let mut r = RangeBars::new(1.0).unwrap();
        // Up bar: open 10, low 9.5, high 11, close 10.8.
        let bar = InputBar {
            time: 0,
            open: 10.0,
            high: 11.0,
            low: 9.5,
            close: 10.8,
            volume: 4.0,
        };
        let out = run(&mut r, &[bar]);
        let got: Vec<_> = out.iter().map(ohlc).collect();
        assert_eq!(got, [[10.0, 10.5, 9.5, 10.5], [10.5, 11.0, 10.5, 10.8]]);
        assert_eq!(out[0].volume, 0.0);
        assert_eq!(out[1].volume, 4.0);
    }
}
