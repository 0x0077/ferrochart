//! Renko (rules in `docs/book/src/series.md`).

use crate::error::{Error, Result};
use crate::frame::SeriesKind;
use crate::plugin::{InputBar, SeriesTransform, TransformOutput};

use super::{bar_kind, element, emit, fresh_volume, positive};

/// How the box size is chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BoxSize {
    Fixed(f64),
    /// The mean true range of the first `n` inputs (more while it is 0).
    Atr(usize),
}

/// The grid once the box size is known: bricks are `anchor + k·size`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Grid {
    anchor: f64,
    size: f64,
    top: i64,
    bot: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct State {
    grid: Option<Grid>,
    /// ATR: inputs seen, sum of their true ranges and the last close.
    count: usize,
    tr_sum: f64,
    prev_close: Option<f64>,
    /// Volume not yet in a brick.
    volume: f64,
}

const START: State = State {
    grid: None,
    count: 0,
    tr_sum: 0.0,
    prev_close: None,
    volume: 0.0,
};

/// Renko bricks of a fixed size, or sized by the ATR of the first inputs.
/// Bricks are finished elements only (none is forming).
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::SeriesData;
/// use ferrochart_core::transform::Renko;
/// use ferrochart_core::data::ValueColumns;
/// use ferrochart_core::chart::SeriesOptions;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let renko = chart.add_transform_series(
///     chart.main_pane(),
///     Box::new(Renko::fixed(1.0)?),
///     SeriesOptions::default(),
/// )?;
/// chart.set_data(renko, SeriesData::Value(ValueColumns {
///     time: vec![0, 60_000, 120_000],
///     value: vec![10.0, 12.5, 13.2],
///     color: None,
/// }))?;
/// // 10 → 11 → 12 → 13: three bricks on the time axis.
/// assert_eq!(chart.data_view().time_index().len(), 3);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Renko {
    size: BoxSize,
    kind: SeriesKind,
    state: State,
}

impl Renko {
    /// Bricks of `size`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] unless `size` is finite and positive.
    pub fn fixed(size: f64) -> Result<Self> {
        Ok(Self {
            size: BoxSize::Fixed(positive(size, "the brick size must be positive")?),
            kind: SeriesKind::Candlestick,
            state: START,
        })
    }

    /// Bricks the size of the mean true range of the first `n` inputs.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] if `n` is 0.
    pub fn atr(n: usize) -> Result<Self> {
        if n == 0 {
            return Err(Error::InvalidArgument("the ATR period must be at least 1"));
        }
        Ok(Self {
            size: BoxSize::Atr(n),
            kind: SeriesKind::Candlestick,
            state: START,
        })
    }

    /// Drawn as `kind` (`Candlestick`, the default, or `OhlcBar`).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for other kinds.
    pub fn with_kind(mut self, kind: SeriesKind) -> Result<Self> {
        self.kind = bar_kind(kind)?;
        Ok(self)
    }

    /// The brick size, once known.
    pub fn brick_size(&self) -> Option<f64> {
        self.state.grid.map(|g| g.size)
    }

    fn step(&self, s: &mut State, bar: InputBar, out: &mut TransformOutput) {
        let c = bar.close;
        s.volume += bar.volume;
        let Some(g) = s.grid.as_mut() else {
            match self.size {
                BoxSize::Fixed(size) => {
                    s.grid = Some(Grid {
                        anchor: c,
                        size,
                        top: 0,
                        bot: 0,
                    });
                }
                BoxSize::Atr(n) => {
                    let tr = match s.prev_close {
                        None => bar.high - bar.low,
                        Some(p) => (bar.high - bar.low)
                            .max((bar.high - p).abs())
                            .max((bar.low - p).abs()),
                    };
                    s.count += 1;
                    s.tr_sum += tr;
                    s.prev_close = Some(c);
                    if s.count >= n && s.tr_sum > 0.0 {
                        s.grid = Some(Grid {
                            anchor: c,
                            size: s.tr_sum / s.count as f64,
                            top: 0,
                            bot: 0,
                        });
                    }
                }
            }
            return;
        };
        let level = |k: i64| g.anchor + k as f64 * g.size;
        loop {
            let (open, close) = if c >= level(g.top + 1) {
                let x = (level(g.top), level(g.top + 1));
                g.bot = g.top;
                g.top += 1;
                x
            } else if c <= level(g.bot - 1) {
                let x = (level(g.bot), level(g.bot - 1));
                g.top = g.bot;
                g.bot -= 1;
                x
            } else {
                break;
            };
            let v = core::mem::replace(&mut s.volume, fresh_volume(bar.volume));
            if !emit(out, element(open, close, v, 0)) {
                break;
            }
        }
    }
}

impl SeriesTransform for Renko {
    fn name(&self) -> &'static str {
        "renko"
    }

    fn output_kind(&self) -> SeriesKind {
        self.kind
    }

    fn push(&mut self, bar: InputBar, out: &mut TransformOutput) {
        let mut s = self.state;
        self.step(&mut s, bar, out);
        self.state = s;
    }

    fn peek(&self, bar: InputBar, out: &mut TransformOutput) {
        let mut s = self.state;
        self.step(&mut s, bar, out);
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
    fn bricks_continue_with_one_box_and_reverse_with_two() {
        let mut r = Renko::fixed(1.0).unwrap();
        let bars: Vec<_> = [10.0, 11.5, 12.0, 10.9, 10.0, 9.0, 6.4]
            .iter()
            .enumerate()
            .map(|(i, &p)| tick(i as i64, p))
            .collect();
        let out = run(&mut r, &bars);
        assert_eq!(
            oc(&out),
            [
                (10.0, 11.0),
                (11.0, 12.0),
                // 10.9 is not two boxes down; 10.0 is.
                (11.0, 10.0),
                (10.0, 9.0),
                (9.0, 8.0),
                (8.0, 7.0),
            ]
        );
        // Volume: the inputs before a brick go into it, the rest get 0.
        let v: Vec<f64> = out.iter().map(|b| b.volume).collect();
        assert_eq!(v, [2.0, 1.0, 2.0, 1.0, 1.0, 0.0]);
    }

    #[test]
    fn atr_size_is_fixed_after_n_inputs() {
        let mut r = Renko::atr(2).unwrap();
        let bars = [
            InputBar {
                time: 0,
                open: 10.0,
                high: 11.0,
                low: 10.0,
                close: 10.5,
                volume: f64::NAN,
            },
            InputBar {
                time: 1,
                open: 10.5,
                high: 13.5,
                low: 10.5,
                close: 13.0,
                volume: f64::NAN,
            },
            tick(2, 15.0),
            tick(3, 100.0),
        ];
        // TR 1 and 3: size 2, anchor 13.
        let out = run(&mut r, &bars[..3]);
        assert_eq!(oc(&out), [(13.0, 15.0)]);
        assert!(out[0].volume.is_nan());
        let out = run(&mut r, &bars);
        assert_eq!(out.len(), 1 + 42);
        assert_eq!(r.brick_size(), Some(2.0));
    }
}
