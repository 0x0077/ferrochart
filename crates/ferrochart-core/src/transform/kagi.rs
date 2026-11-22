//! Kagi (rules in `docs/book/src/series.md`).

use crate::error::Result;
use crate::frame::SeriesKind;
use crate::plugin::{InputBar, SeriesTransform, TransformOutput};

use super::{element, emit, fresh_volume, positive};

/// The move that starts a new Kagi line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reversal {
    /// A price distance.
    Fixed(f64),
    /// A percentage of the current line's end price.
    Percent(f64),
}

/// A Kagi line.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Line {
    open: f64,
    end: f64,
    up: bool,
    /// Thickness at its start (1 thick, 0 thin).
    start_flag: u8,
    volume: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct State {
    /// The first close.
    start: Option<f64>,
    line: Option<Line>,
    /// Start of the previous line (the last shoulder or waist).
    prev_open: Option<f64>,
    /// Volume before the first line.
    volume: f64,
}

const START: State = State {
    start: None,
    line: None,
    prev_open: None,
    volume: 0.0,
};

/// Kagi lines: a new line starts when the price reverses by the reversal
/// amount; lines are thick (yang) after rising above the previous shoulder
/// and thin (yin) after falling below the previous waist. The last line is
/// forming.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::SeriesOptions;
/// use ferrochart_core::transform::{Kagi, Reversal};
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.add_transform_series(
///     chart.main_pane(),
///     Box::new(Kagi::new(Reversal::Percent(1.0))?),
///     SeriesOptions::default(),
/// )?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Kagi {
    reversal: Reversal,
    state: State,
}

impl Kagi {
    /// Kagi lines with `reversal`.
    ///
    /// # Errors
    ///
    /// [`crate::Error::InvalidArgument`] unless the amount is finite and
    /// positive.
    pub fn new(reversal: Reversal) -> Result<Self> {
        let msg = "the reversal amount must be positive";
        let reversal = match reversal {
            Reversal::Fixed(x) => Reversal::Fixed(positive(x, msg)?),
            Reversal::Percent(p) => Reversal::Percent(positive(p, msg)?),
        };
        Ok(Self {
            reversal,
            state: START,
        })
    }

    fn rev(&self, at: f64) -> f64 {
        match self.reversal {
            Reversal::Fixed(x) => x,
            Reversal::Percent(p) => at.abs() * p / 100.0,
        }
    }

    /// Thickness at the end of `line`.
    fn flag(line: &Line, prev_open: Option<f64>) -> u8 {
        match prev_open {
            Some(s) if line.up && line.end > s => 1,
            Some(s) if !line.up && line.end < s => 0,
            _ => line.start_flag,
        }
    }

    fn element(line: &Line, prev_open: Option<f64>) -> crate::plugin::TransformBar {
        element(
            line.open,
            line.end,
            line.volume,
            Self::flag(line, prev_open),
        )
    }

    fn step(&self, s: &mut State, bar: InputBar, out: &mut TransformOutput) {
        let c = bar.close;
        let v = bar.volume;
        let Some(start) = s.start else {
            s.start = Some(c);
            s.volume += v;
            return;
        };
        let Some(line) = s.line.as_mut() else {
            s.volume += v;
            if (c - start).abs() >= self.rev(start) && (c - start).abs() > 0.0 {
                let up = c > start;
                s.line = Some(Line {
                    open: start,
                    end: c,
                    up,
                    start_flag: u8::from(up),
                    volume: s.volume,
                });
            }
            return;
        };
        let extends = if line.up { c > line.end } else { c < line.end };
        let reverses = if line.up { c < line.end } else { c > line.end };
        if extends {
            line.end = c;
        } else if reverses && (c - line.end).abs() >= self.rev(line.end) {
            let done = *line;
            let flag = Self::flag(&done, s.prev_open);
            if !emit(out, Self::element(&done, s.prev_open)) {
                return;
            }
            s.prev_open = Some(done.open);
            *line = Line {
                open: done.end,
                end: c,
                up: !done.up,
                start_flag: flag,
                volume: fresh_volume(v),
            };
        }
        line.volume += v;
    }
}

impl SeriesTransform for Kagi {
    fn name(&self) -> &'static str {
        "kagi"
    }

    fn output_kind(&self) -> SeriesKind {
        SeriesKind::Kagi
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
            && let Some(line) = s.line
        {
            emit(out, Self::element(&line, s.prev_open));
        }
    }

    fn reset(&mut self) {
        self.state = START;
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::transform::tests::{oc, run, tick};

    #[test]
    fn lines_reverse_and_change_thickness() {
        let mut k = Kagi::new(Reversal::Fixed(2.0)).unwrap();
        let prices = [10.0, 11.0, 13.0, 12.0, 10.5, 12.0, 14.0, 9.0];
        let bars: Vec<_> = prices
            .iter()
            .enumerate()
            .map(|(i, &p)| tick(i as i64, p))
            .collect();
        let out = run(&mut k, &bars);
        assert_eq!(
            oc(&out),
            [(10.0, 13.0), (13.0, 10.5), (10.5, 14.0), (14.0, 9.0)]
        );
        let flags: Vec<u8> = out.iter().map(|b| b.flag).collect();
        // Up from 10: thick. Down to 10.5 stays above the start 10: thick.
        // Up to 14 above the shoulder 13: thick. Down to 9 below the waist
        // 10.5: thin.
        assert_eq!(flags, [1, 1, 1, 0]);
        // Each input's volume goes to the line forming after it: 10, 11,
        // 13, 12 | 10.5, 12 | 14 | 9.
        let v: Vec<f64> = out.iter().map(|b| b.volume).collect();
        assert_eq!(v, [4.0, 2.0, 1.0, 1.0]);
    }

    #[test]
    fn percent_reversal_uses_the_end_price() {
        let mut k = Kagi::new(Reversal::Percent(10.0)).unwrap();
        let bars = [
            tick(0, 100.0),
            tick(1, 111.0),
            tick(2, 100.5),
            tick(3, 99.0),
        ];
        // 10% of 111 is 11.1: 100.5 does not reverse, 99 does.
        assert_eq!(oc(&run(&mut k, &bars[..3])), [(100.0, 111.0)]);
        assert_eq!(oc(&run(&mut k, &bars)), [(100.0, 111.0), (111.0, 99.0)]);
    }
}
