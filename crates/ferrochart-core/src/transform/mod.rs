//! Built-in series transforms: Renko, Range
//! Bars, Kagi and Point & Figure. Their exact rules are in
//! `docs/book/src/series.md`. Add one to a chart with
//! [`Chart::add_transform_series`](crate::Chart::add_transform_series).

use crate::error::{Error, Result};
use crate::frame::SeriesKind;
use crate::plugin::{TransformBar, TransformOutput};

mod kagi;
mod point_figure;
mod range_bars;
mod renko;
pub(crate) mod store;

pub use kagi::{Kagi, Reversal};
pub use point_figure::PointFigure;
pub use range_bars::RangeBars;
pub use renko::Renko;

/// `Err(InvalidArgument)` unless `x` is finite and positive.
fn positive(x: f64, what: &'static str) -> Result<f64> {
    if x.is_finite() && x > 0.0 {
        Ok(x)
    } else {
        Err(Error::InvalidArgument(what))
    }
}

/// `Ok` for the kinds an OHLC-producing transform can be drawn as.
fn bar_kind(kind: SeriesKind) -> Result<SeriesKind> {
    match kind {
        SeriesKind::Candlestick | SeriesKind::OhlcBar => Ok(kind),
        _ => Err(Error::InvalidArgument(
            "this transform is drawn as candlesticks or OHLC bars",
        )),
    }
}

/// An element from `open` to `close`.
fn element(open: f64, close: f64, volume: f64, flag: u8) -> TransformBar {
    TransformBar {
        open,
        high: open.max(close),
        low: open.min(close),
        close,
        volume,
        flag,
    }
}

/// Writes `bar`; `false` if the transform must stop.
fn emit(out: &mut TransformOutput, bar: TransformBar) -> bool {
    out.push(bar)
}

/// Volume of an element opened by an input with volume `v` (0, or NaN when
/// the input has none).
fn fresh_volume(v: f64) -> f64 {
    if v.is_nan() { f64::NAN } else { 0.0 }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::plugin::{InputBar, SeriesTransform};

    /// A tick: all four prices `p`.
    pub(crate) fn tick(time: i64, p: f64) -> InputBar {
        InputBar {
            time,
            open: p,
            high: p,
            low: p,
            close: p,
            volume: 1.0,
        }
    }

    /// The output of `t` over `bars`: push all but the last, peek the last.
    pub(crate) fn run(t: &mut dyn SeriesTransform, bars: &[InputBar]) -> Vec<TransformBar> {
        t.reset();
        let mut out = TransformOutput::default();
        let mut all = Vec::new();
        if let Some((last, rest)) = bars.split_last() {
            for b in rest {
                out.clear();
                t.push(*b, &mut out);
                all.extend_from_slice(out.as_slice());
            }
            out.clear();
            t.peek(*last, &mut out);
            all.extend_from_slice(out.as_slice());
        }
        all
    }

    /// `(open, close)` of each element.
    pub(crate) fn oc(bars: &[TransformBar]) -> Vec<(f64, f64)> {
        bars.iter().map(|b| (b.open, b.close)).collect()
    }

    #[test]
    fn parameters_are_checked() {
        assert!(Renko::fixed(0.0).is_err());
        assert!(Renko::fixed(f64::NAN).is_err());
        assert!(Renko::atr(0).is_err());
        assert!(RangeBars::new(-1.0).is_err());
        assert!(Kagi::new(Reversal::Percent(0.0)).is_err());
        assert!(PointFigure::new(1.0, 0).is_err());
        assert!(
            Renko::fixed(1.0)
                .and_then(|r| r.with_kind(SeriesKind::Kagi))
                .is_err()
        );
    }

    #[test]
    fn output_stops_at_the_limit() {
        let mut r = Renko::fixed(1e-9).unwrap();
        let mut out = TransformOutput::default();
        r.push(tick(0, 0.0), &mut out);
        r.peek(tick(1, 1.0), &mut out);
        assert_eq!(out.as_slice().len(), crate::plugin::MAX_OUTPUT_PER_CALL);
        assert!(out.check().is_err());
    }
}
