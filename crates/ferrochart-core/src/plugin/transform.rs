//! Series transforms: input bars in, a new
//! OHLC-like series out (Renko, Range Bars, Kagi, Point & Figure). The
//! built-in transforms are in [`crate::transform`].

use crate::frame::SeriesKind;

use super::InputBar;

/// One output element of a [`SeriesTransform`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformBar {
    /// Open (Kagi / P&F: where the line or column starts).
    pub open: f64,
    /// High.
    pub high: f64,
    /// Low.
    pub low: f64,
    /// Close (Kagi / P&F: where the line or column ends).
    pub close: f64,
    /// Volume; NaN when the input has none.
    pub volume: f64,
    /// Kagi: 1 thick (yang) / 0 thin (yin) at the end of the line; P&F: 1 X
    /// / 0 O; otherwise 0.
    pub flag: u8,
}

/// Elements one `push` or `peek` call may write.
pub const MAX_OUTPUT_PER_CALL: usize = 65_536;

/// Reused output buffer of a [`SeriesTransform`] call; clearing keeps the
/// capacity, so a warmed-up buffer does not allocate.
#[derive(Debug, Default)]
pub struct TransformOutput {
    bars: Vec<TransformBar>,
    /// Why the last call stopped early.
    pub(crate) failed: Option<crate::Error>,
}

impl TransformOutput {
    /// Adds an element. Returns `false` when the call already wrote
    /// [`MAX_OUTPUT_PER_CALL`] elements or memory ran out: the transform
    /// should stop, and core reports the error.
    pub fn push(&mut self, bar: TransformBar) -> bool {
        if self.failed.is_some() {
            return false;
        }
        if self.bars.len() >= MAX_OUTPUT_PER_CALL {
            self.failed = Some(crate::Error::InvalidArgument(
                "a transform input produced too many elements (box size too small)",
            ));
            return false;
        }
        if self.bars.try_reserve(1).is_err() {
            self.failed = Some(crate::Error::OutOfMemory);
            return false;
        }
        self.bars.push(bar);
        true
    }

    /// The elements written since the last clear.
    pub fn as_slice(&self) -> &[TransformBar] {
        &self.bars
    }

    pub(crate) fn clear(&mut self) {
        self.bars.clear();
        self.failed = None;
    }

    /// `Err` if the last call stopped early.
    pub(crate) fn check(&self) -> crate::Result<()> {
        self.failed.clone().map_or(Ok(()), Err)
    }
}

/// Turns input bars into a new series, with the peek/push model of
/// indicators: the output is the elements `push` wrote for every
/// input but the last (finished, never changed again) followed by what
/// `peek` writes for the last input (the elements it would finish and the
/// ones still forming). Gap bars are never passed in.
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::SeriesKind;
/// use ferrochart_core::plugin::{InputBar, SeriesTransform, TransformBar, TransformOutput};
///
/// /// One element per input whose close moved by at least 1 since the last one.
/// #[derive(Default)]
/// struct Steps {
///     last: Option<f64>,
/// }
///
/// impl Steps {
///     fn step(last: Option<f64>, bar: &InputBar) -> Option<TransformBar> {
///         let from = last?;
///         ((bar.close - from).abs() >= 1.0).then(|| TransformBar {
///             open: from,
///             high: from.max(bar.close),
///             low: from.min(bar.close),
///             close: bar.close,
///             volume: f64::NAN,
///             flag: 0,
///         })
///     }
/// }
///
/// impl SeriesTransform for Steps {
///     fn name(&self) -> &str {
///         "steps"
///     }
///     fn output_kind(&self) -> SeriesKind {
///         SeriesKind::Candlestick
///     }
///     fn push(&mut self, bar: InputBar, out: &mut TransformOutput) {
///         match Self::step(self.last, &bar) {
///             Some(b) => {
///                 out.push(b);
///                 self.last = Some(bar.close);
///             }
///             None if self.last.is_none() => self.last = Some(bar.close),
///             None => {}
///         }
///     }
///     fn peek(&self, bar: InputBar, out: &mut TransformOutput) {
///         if let Some(b) = Self::step(self.last, &bar) {
///             out.push(b);
///         }
///     }
///     fn reset(&mut self) {
///         self.last = None;
///     }
/// }
/// ```
pub trait SeriesTransform: Send {
    /// Name, e.g. `"renko"`.
    fn name(&self) -> &str;
    /// How the output is drawn: `Candlestick`, `OhlcBar`, `Kagi` or
    /// `PointFigure`.
    fn output_kind(&self) -> SeriesKind;
    /// Commits `bar` and writes the elements it finishes.
    fn push(&mut self, bar: InputBar, out: &mut TransformOutput);
    /// Writes, without changing the state, the elements `bar` would finish
    /// and the ones still forming if it were the last input.
    fn peek(&self, bar: InputBar, out: &mut TransformOutput);
    /// Forgets all inputs.
    fn reset(&mut self);
    /// Point & Figure: the price height of one box, to stack the X/O marks.
    fn box_size(&self) -> Option<f64> {
        None
    }
}
