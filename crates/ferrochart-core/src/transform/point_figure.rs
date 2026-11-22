//! Point & Figure (rules in `docs/book/src/series.md`).

use crate::error::{Error, Result};
use crate::frame::SeriesKind;
use crate::plugin::{InputBar, SeriesTransform, TransformBar, TransformOutput};

use super::{emit, fresh_volume, positive};

/// A column of boxes `lo..=hi`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Column {
    lo: i64,
    hi: i64,
    x: bool,
    volume: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct State {
    /// The box of the first close.
    k0: Option<i64>,
    column: Option<Column>,
    /// Volume before the first column.
    volume: f64,
}

const START: State = State {
    k0: None,
    column: None,
    volume: 0.0,
};

/// Point & Figure columns of X (rising) and O (falling) boxes. The last
/// column is forming.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::SeriesOptions;
/// use ferrochart_core::transform::PointFigure;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.add_transform_series(
///     chart.main_pane(),
///     Box::new(PointFigure::new(1.0, 3)?),
///     SeriesOptions::default(),
/// )?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct PointFigure {
    size: f64,
    reversal: i64,
    state: State,
}

impl PointFigure {
    /// Boxes of `size`; a new column needs `reversal` boxes.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] unless `size` is finite and positive and
    /// `reversal` at least 1.
    pub fn new(size: f64, reversal: u32) -> Result<Self> {
        if reversal == 0 {
            return Err(Error::InvalidArgument(
                "the reversal must be at least 1 box",
            ));
        }
        Ok(Self {
            size: positive(size, "the box size must be positive")?,
            reversal: i64::from(reversal),
            state: START,
        })
    }

    /// Highest box reached going up (its top at or below `c`).
    fn up(&self, c: f64) -> i64 {
        (c / self.size).floor() as i64 - 1
    }

    /// Lowest box reached going down (its bottom at or above `c`).
    fn down(&self, c: f64) -> i64 {
        (c / self.size).ceil() as i64
    }

    fn element(&self, col: &Column) -> TransformBar {
        let (low, high) = (col.lo as f64 * self.size, (col.hi + 1) as f64 * self.size);
        let (open, close) = if col.x { (low, high) } else { (high, low) };
        TransformBar {
            open,
            high,
            low,
            close,
            volume: col.volume,
            flag: u8::from(col.x),
        }
    }

    fn step(&self, s: &mut State, bar: InputBar, out: &mut TransformOutput) {
        let c = bar.close;
        let v = bar.volume;
        let Some(k0) = s.k0 else {
            s.k0 = Some((c / self.size).floor() as i64);
            s.volume += v;
            return;
        };
        let (up, down) = (self.up(c), self.down(c));
        let Some(col) = s.column.as_mut() else {
            s.volume += v;
            let first = if up >= k0 {
                Some((k0, up, true))
            } else if down <= k0 {
                Some((down, k0, false))
            } else {
                None
            };
            if let Some((lo, hi, x)) = first {
                s.column = Some(Column {
                    lo,
                    hi,
                    x,
                    volume: s.volume,
                });
            }
            return;
        };
        let next = if col.x {
            if up > col.hi {
                col.hi = up;
                None
            } else if col.hi - down >= self.reversal {
                Some((down, col.hi - 1, false))
            } else {
                None
            }
        } else if down < col.lo {
            col.lo = down;
            None
        } else if up - col.lo >= self.reversal {
            Some((col.lo + 1, up, true))
        } else {
            None
        };
        if let Some((lo, hi, x)) = next {
            if !emit(out, self.element(col)) {
                return;
            }
            *col = Column {
                lo,
                hi,
                x,
                volume: fresh_volume(v),
            };
        }
        col.volume += v;
    }
}

impl SeriesTransform for PointFigure {
    fn name(&self) -> &'static str {
        "point_figure"
    }

    fn output_kind(&self) -> SeriesKind {
        SeriesKind::PointFigure
    }

    fn push(&mut self, bar: InputBar, out: &mut TransformOutput) {
        let mut s = self.state;
        self.step(&mut s, bar, out);
        self.state = s;
    }

    fn peek(&self, bar: InputBar, out: &mut TransformOutput) {
        let mut s = self.state;
        self.step(&mut s, bar, out);
        if out.check().is_ok()
            && let Some(col) = s.column
        {
            emit(out, self.element(&col));
        }
    }

    fn reset(&mut self) {
        self.state = START;
    }

    fn box_size(&self) -> Option<f64> {
        Some(self.size)
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::transform::tests::{run, tick};

    /// `(low, high, x)` of each column.
    fn cols(bars: &[TransformBar]) -> Vec<(f64, f64, bool)> {
        bars.iter().map(|b| (b.low, b.high, b.flag == 1)).collect()
    }

    #[test]
    fn columns_reverse_after_three_boxes() {
        let mut p = PointFigure::new(1.0, 3).unwrap();
        // Start in box 10. 12.2 reaches box 11 (top 12): X 10..=11.
        // 14.5: X up to box 13. 12.0: 13 - 12 = 1 < 3. 10.9: 13 - 11 = 2.
        // 10.0: 13 - 10 = 3: O boxes 10..=12.
        let prices = [10.5, 12.2, 14.5, 12.0, 10.9, 10.0];
        let bars: Vec<_> = prices
            .iter()
            .enumerate()
            .map(|(i, &x)| tick(i as i64, x))
            .collect();
        let out = run(&mut p, &bars);
        assert_eq!(cols(&out), [(10.0, 14.0, true), (10.0, 13.0, false)]);
        assert_eq!((out[0].open, out[0].close), (10.0, 14.0));
        assert_eq!((out[1].open, out[1].close), (13.0, 10.0));
        assert_eq!(p.box_size(), Some(1.0));
    }

    #[test]
    fn first_column_can_fall() {
        let mut p = PointFigure::new(2.0, 2).unwrap();
        // Box 5 (10..12); 9.5 is below its bottom: O column of box 5.
        let out = run(&mut p, &[tick(0, 11.0), tick(1, 9.5)]);
        assert_eq!(cols(&out), [(10.0, 12.0, false)]);
        // Back up: X needs up(c) - 5 >= 2, i.e. box 7 (top 16).
        let out = run(&mut p, &[tick(0, 11.0), tick(1, 9.5), tick(2, 15.9)]);
        assert_eq!(out.len(), 1);
        let out = run(&mut p, &[tick(0, 11.0), tick(1, 9.5), tick(2, 16.0)]);
        assert_eq!(cols(&out), [(10.0, 12.0, false), (12.0, 16.0, true)]);
    }
}
