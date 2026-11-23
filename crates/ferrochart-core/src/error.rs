//! Error type of the public API. Hand-written in `thiserror` style; the core does not
//! depend on `thiserror`.
//!

use core::fmt;

/// Errors returned by the `ferrochart-core` public API. The public API never panics.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// An argument is outside its valid domain. The payload names the argument and
    /// the violated condition.
    InvalidArgument(&'static str),
    /// A time column is not strictly increasing. `index` is the position in the
    /// input where `time[index] <= time[index - 1]`. When the input itself is sorted
    /// but does not fit the existing data, it is `0` for an append (first element
    /// not after the last existing time) and `len - 1` for a prepend (last element
    /// not before the first existing time).
    TimeNotIncreasing {
        /// Position of the offending element in the input.
        index: usize,
    },
    /// The input columns do not all have the same length. `column` names the
    /// first column whose length differs from `time`.
    LengthMismatch {
        /// Name of the offending column.
        column: &'static str,
    },
    /// Bar `index` of the input violates the OHLC constraints (a non-finite value,
    /// or `low > min(open, close)` / `high < max(open, close)` in `Reject` mode).
    InvalidOhlc {
        /// Position of the offending bar in the input.
        index: usize,
    },
    /// No series, pane or other object with this id exists.
    UnknownId,
    /// The operation is not supported (yet) for this kind of object. The payload
    /// explains what is missing.
    Unsupported(&'static str),
    /// A required allocation could not be satisfied.
    OutOfMemory,
    /// A fixed limit was reached (for example 1024 icons per chart). The
    /// payload names the limit.
    CapacityExceeded(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(what) => write!(f, "invalid argument: {what}"),
            Self::TimeNotIncreasing { index } => {
                write!(f, "time is not strictly increasing at index {index}")
            }
            Self::LengthMismatch { column } => {
                write!(f, "column `{column}` has a different length than `time`")
            }
            Self::InvalidOhlc { index } => write!(f, "invalid OHLC values in bar {index}"),
            Self::UnknownId => f.write_str("unknown id"),
            Self::Unsupported(what) => write!(f, "unsupported: {what}"),
            Self::OutOfMemory => f.write_str("out of memory"),
            Self::CapacityExceeded(what) => write!(f, "capacity exceeded: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// `Result` with [`Error`] as the default error type.
pub type Result<T, E = Error> = core::result::Result<T, E>;
