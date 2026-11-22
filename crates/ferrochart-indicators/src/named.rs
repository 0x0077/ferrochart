//! Indicators by name, with parameters by name (`docs/book/src/indicators.md`): the one
//! mapping the JS, Python and C
//! bindings and the Rust API share, so `"ema"` with `n = 20` means the same
//! everywhere.
//!
//! ```
//! use ferrochart_core::testing::random_walk;
//! use ferrochart_core::{Chart, ChartOptions};
//! use ferrochart_indicators::named::ChartExt;
//!
//! let mut chart = Chart::new(ChartOptions::default());
//! let candles = chart.add_candlestick()?;
//! chart.set_data(candles, random_walk(1, 200, 0, 60_000)?)?;
//! chart.add_indicator_by_name(candles, "ema", &[("n", 20.into())])?;
//! chart.add_indicator_by_name(candles, "macd", &[])?; // defaults, new pane
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fmt;

use ferrochart_core::Chart;
use ferrochart_core::data::{IndicatorId, SeriesId};

use crate::{Builtin, Source, VwapAnchor};

/// An unknown indicator name or an invalid parameter; the message names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamError(pub String);

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParamError {}

impl From<String> for ParamError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ParamError {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Error of [`ChartExt::add_indicator_by_name`].
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Unknown name or invalid parameter.
    Param(ParamError),
    /// The chart refused the indicator (unknown source series, …).
    Chart(ferrochart_core::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Param(e) => e.fmt(f),
            Self::Chart(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<ParamError> for Error {
    fn from(e: ParamError) -> Self {
        Self::Param(e)
    }
}

impl From<ferrochart_core::Error> for Error {
    fn from(e: ferrochart_core::Error) -> Self {
        Self::Chart(e)
    }
}

/// A parameter value, as the bindings read it from a JS object, a Python
/// dict or C JSON. From Rust, use the `From` conversions (`20.into()`,
/// `"hl2".into()`).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Absent (`undefined` or `null`).
    Missing,
    /// A number.
    Number(f64),
    /// A string.
    Text(String),
    /// Anything else.
    Other,
}

/// Indicator names, in the order of `docs/book/src/indicators.md`. These are the JS
/// names (`camelCase`); [`build`] and [`by_name`] also take `snake_case`.
pub const NAMES: [&str; 30] = [
    "sma",
    "ema",
    "wma",
    "hma",
    "vwma",
    "dema",
    "tema",
    "volumeMa",
    "boll",
    "keltner",
    "donchian",
    "ichimoku",
    "sar",
    "supertrend",
    "vwap",
    "macd",
    "rsi",
    "stoch",
    "stochRsi",
    "cci",
    "willr",
    "mfi",
    "adx",
    "atr",
    "obv",
    "cmf",
    "roc",
    "momentum",
    "trix",
    "aroon",
];

/// Builds indicator `name` from parameters read with `get` (missing ones
/// take the defaults of `docs/book/src/indicators.md`). Used by the bindings; from
/// Rust, [`by_name`] is simpler.
///
/// # Errors
///
/// [`ParamError`] for an unknown name or an invalid parameter.
pub fn build(name: &str, get: impl Fn(&str) -> Value) -> Result<Builtin, ParamError> {
    let camel = camel(name);
    let name = camel.as_str();
    let int = |key: &str, default: usize| -> Result<usize, String> {
        match get(key) {
            Value::Missing => Ok(default),
            Value::Number(v) if v.fract() == 0.0 && (0.0..=1e9).contains(&v) => Ok(v as usize),
            _ => Err(format!("'{key}' must be a non-negative integer")),
        }
    };
    let num = |key: &str, default: f64| -> Result<f64, String> {
        match get(key) {
            Value::Missing => Ok(default),
            Value::Number(v) if v.is_finite() => Ok(v),
            _ => Err(format!("'{key}' must be a finite number")),
        }
    };
    let source = || -> Result<Source, String> {
        Ok(match get("source") {
            Value::Missing => Source::Close,
            Value::Text(s) => match s.as_str() {
                "close" => Source::Close,
                "open" => Source::Open,
                "high" => Source::High,
                "low" => Source::Low,
                "hl2" => Source::Hl2,
                "hlc3" => Source::Hlc3,
                "ohlc4" => Source::Ohlc4,
                _ => return Err(format!("unknown source '{s}'")),
            },
            _ => return Err("'source' must be a string".into()),
        })
    };
    let r = match name {
        "sma" => crate::sma(int("n", 20)?, source()?),
        "ema" => crate::ema(int("n", 20)?, source()?),
        "wma" => crate::wma(int("n", 20)?, source()?),
        "hma" => crate::hma(int("n", 20)?, source()?),
        "vwma" => crate::vwma(int("n", 20)?, source()?),
        "dema" => crate::dema(int("n", 20)?, source()?),
        "tema" => crate::tema(int("n", 20)?, source()?),
        "volumeMa" => crate::volume_ma(int("n", 20)?),
        "boll" => crate::boll(int("n", 20)?, num("k", 2.0)?, source()?),
        "keltner" => crate::keltner(int("n", 20)?, num("m", 2.0)?, int("atrN", 10)?),
        "donchian" => crate::donchian(int("n", 20)?),
        "ichimoku" => crate::ichimoku(
            int("tenkan", 9)?,
            int("kijun", 26)?,
            int("senkou", 52)?,
            int("shift", 26)?,
        ),
        "sar" => crate::sar(
            num("afStart", 0.02)?,
            num("afStep", 0.02)?,
            num("afMax", 0.2)?,
        ),
        "supertrend" => crate::supertrend(int("atrN", 10)?, num("m", 3.0)?),
        "vwap" => {
            let anchor = match get("anchor") {
                Value::Missing => VwapAnchor::Day,
                Value::Text(s) if s == "day" => VwapAnchor::Day,
                Value::Text(s) if s == "week" => VwapAnchor::Week,
                Value::Text(s) if s == "month" => VwapAnchor::Month,
                Value::Number(t) if t.fract() == 0.0 && t.abs() < 9.0e15 => {
                    VwapAnchor::Time(t as i64)
                }
                _ => {
                    return Err("'anchor' must be 'day', 'week', 'month' or a time in ms".into());
                }
            };
            let offset = num("utcOffsetMs", 0.0)?;
            if offset.fract() != 0.0 || offset.abs() > 1e9 {
                return Err("'utcOffsetMs' must be an integer".into());
            }
            crate::vwap(anchor, offset as i64)
        }
        "macd" => crate::macd(
            int("fast", 12)?,
            int("slow", 26)?,
            int("signal", 9)?,
            source()?,
        ),
        "rsi" => crate::rsi(int("n", 14)?, source()?),
        "stoch" => crate::stoch(int("k", 14)?, int("smoothK", 3)?, int("d", 3)?),
        "stochRsi" => crate::stochrsi(
            int("rsiN", 14)?,
            int("stochN", 14)?,
            int("smoothK", 3)?,
            int("d", 3)?,
            source()?,
        ),
        "cci" => crate::cci(int("n", 20)?),
        "willr" => crate::willr(int("n", 14)?),
        "mfi" => crate::mfi(int("n", 14)?),
        "adx" => crate::adx(int("n", 14)?),
        "atr" => crate::atr(int("n", 14)?),
        "obv" => crate::obv(),
        "cmf" => crate::cmf(int("n", 20)?),
        "roc" => crate::roc(int("n", 10)?, source()?),
        "momentum" => crate::momentum(int("n", 10)?, source()?),
        "trix" => crate::trix(int("n", 15)?, source()?),
        "aroon" => crate::aroon(int("n", 14)?),
        _ => return Err(ParamError(format!("unknown indicator '{name}'"))),
    };
    r.map_err(|e| ParamError(e.to_string()))
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Number(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Number(f64::from(v))
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Self::Number(f64::from(v))
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Text(v.to_owned())
    }
}

/// `snake_case` → `camelCase` (`stoch_rsi` → `stochRsi`); other names unchanged.
fn camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut up = false;
    for c in s.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Builds indicator `name` with `params` (name, value); missing parameters
/// take the defaults of `docs/book/src/indicators.md`. Names and parameter names may be
/// `camelCase` (as in JS) or `snake_case` (as in Python).
///
/// # Errors
///
/// [`ParamError`] for an unknown name or an invalid parameter.
pub fn by_name(name: &str, params: &[(&str, Value)]) -> Result<Builtin, ParamError> {
    build(name, |key| {
        params
            .iter()
            .find(|(k, _)| *k == key || camel(k) == key)
            .map_or(Value::Missing, |(_, v)| v.clone())
    })
}

/// Indicators by name on [`Chart`]: `use ferrochart_indicators::named::ChartExt`.
pub trait ChartExt {
    /// Attaches indicator `name` (see [`NAMES`]) computed from `source`, at
    /// its default placement (overlay or a new pane), with `params` as in
    /// [`by_name`].
    ///
    /// # Errors
    ///
    /// [`Error::Param`] for an unknown name or invalid parameter,
    /// [`Error::Chart`] when the chart refuses it (unknown `source`, …).
    fn add_indicator_by_name(
        &mut self,
        source: SeriesId,
        name: &str,
        params: &[(&str, Value)],
    ) -> Result<IndicatorId, Error>;
}

impl ChartExt for Chart {
    fn add_indicator_by_name(
        &mut self,
        source: SeriesId,
        name: &str,
        params: &[(&str, Value)],
    ) -> Result<IndicatorId, Error> {
        use ferrochart_core::plugin::Indicator as _;
        let ind = by_name(name, params)?;
        let placement = ind.default_placement();
        Ok(self.add_indicator(source, Box::new(ind), placement)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_build_with_defaults_in_both_spellings() {
        for n in NAMES {
            assert!(build(n, |_| Value::Missing).is_ok(), "{n}");
        }
        assert!(by_name("stoch_rsi", &[("rsi_n", 10.into())]).is_ok());
        assert!(by_name("volume_ma", &[]).is_ok());
    }

    #[test]
    fn bad_input_names_the_problem() {
        assert_eq!(
            by_name("nope", &[]).unwrap_err().0,
            "unknown indicator 'nope'"
        );
        assert!(
            by_name("ema", &[("n", 2.5.into())])
                .unwrap_err()
                .0
                .contains("'n'")
        );
        assert!(by_name("ema", &[("source", "x".into())]).is_err());
    }
}
