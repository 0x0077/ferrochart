//! Python bindings for FerroChart (PyPI package `ferrochart`): charts and
//! plots.
//!
//! This crate is the native module `ferrochart._native`; the Python package
//! (`python/ferrochart`) wraps it: it turns DataFrames and datetimes into
//! contiguous `int64`/`float64` NumPy arrays and passes options as dicts with
//! `snake_case` keys. Here the arrays are copied into the engine with one
//! `memcpy` per column, the options are checked (unknown keys are errors)
//! and turned into core types. Indicator and transform names and parameters
//! are those of the JS binding (`params.rs` of `ferrochart-wasm`, shared
//! source) in `snake_case`; plot option names are those of `plot_params.rs`
//! (also shared with the JS binding).
//!
//! Built by maturin (`pyproject.toml` turns on `pyo3/extension-module`); the
//! workspace's Rust tests exclude this crate and it is tested with pytest
//! (`crates/ferrochart-py/tests`).

mod chart;
mod drawings;
mod events;
mod opts;
// Option names and indicator/transform builders shared with the JS binding
// and the C ABI.
#[path = "../../ferrochart-ffi/src/params.rs"]
mod params;
mod plot;
mod plot_opts;
// Plot option names shared with the JS binding.
#[path = "../../ferrochart-wasm/src/plot_params.rs"]
mod plot_params;
mod png;
mod shared;
mod window;

use numpy::IntoPyArray;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// A `ValueError` with the message of `e`.
pub(crate) fn err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// `snake_case` → `camelCase` (`atr_n` → `atrN`, `indexed_to_100` →
/// `indexedTo100`), the spelling of `params.rs`.
pub(crate) fn camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper = false;
    for c in s.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// `camelCase` → `snake_case` (`atrN` → `atr_n`).
pub(crate) fn snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Message of `params.rs` with its quoted `camelCase` names in `snake_case`.
pub(crate) fn snake_message(msg: &str) -> String {
    msg.split('\'')
        .enumerate()
        .map(|(i, part)| {
            if i % 2 == 1 {
                snake(part)
            } else {
                part.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("'")
}

/// Deterministic random-walk OHLCV bars, bit-identical to
/// `ferrochart_core::testing::random_walk` and the JS `randomWalk`: a dict of
/// NumPy arrays `time` (int64 ms), `open`, `high`, `low`, `close`, `volume`.
#[pyfunction]
#[pyo3(signature = (seed, n, start_ms = 0, interval_ms = 60_000))]
fn random_walk(
    py: Python<'_>,
    seed: u64,
    n: usize,
    start_ms: i64,
    interval_ms: i64,
) -> PyResult<Bound<'_, PyDict>> {
    let bars =
        ferrochart_core::testing::random_walk(seed, n, start_ms, interval_ms).map_err(err)?;
    let d = PyDict::new(py);
    d.set_item("time", bars.time.into_pyarray(py))?;
    d.set_item("open", bars.open.into_pyarray(py))?;
    d.set_item("high", bars.high.into_pyarray(py))?;
    d.set_item("low", bars.low.into_pyarray(py))?;
    d.set_item("close", bars.close.into_pyarray(py))?;
    if let Some(v) = bars.volume {
        d.set_item("volume", v.into_pyarray(py))?;
    }
    Ok(d)
}

/// Names accepted by `add_indicator`, in the order of `docs/book/src/indicators.md`.
#[pyfunction]
fn indicator_names() -> Vec<String> {
    params::INDICATORS.iter().map(|n| snake(n)).collect()
}

/// The native module.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(random_walk, m)?)?;
    m.add_function(wrap_pyfunction!(indicator_names, m)?)?;
    m.add_function(wrap_pyfunction!(events::process_events, m)?)?;
    m.add_class::<chart::Chart>()?;
    m.add_class::<chart::Series>()?;
    m.add_class::<chart::Indicator>()?;
    m.add_class::<chart::PriceLine>()?;
    m.add_class::<drawings::Drawings>()?;
    m.add_class::<plot::Plot>()?;
    m.add_class::<plot::Mark>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_convert_both_ways() {
        assert_eq!(camel("atr_n"), "atrN");
        assert_eq!(camel("indexed_to_100"), "indexedTo100");
        assert_eq!(camel("sma"), "sma");
        assert_eq!(snake("utcOffsetMs"), "utc_offset_ms");
        assert_eq!(snake("stochRsi"), "stoch_rsi");
        assert_eq!(
            snake_message("'atrN' must be a non-negative integer"),
            "'atr_n' must be a non-negative integer"
        );
        for name in params::INDICATORS {
            assert_eq!(camel(&snake(name)), name);
        }
    }
}
