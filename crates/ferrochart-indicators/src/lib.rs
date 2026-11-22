//! Incremental technical indicators for FerroChart.
//!
//! Every function returns a [`Builtin`] that implements
//! [`ferrochart_core::plugin::Indicator`]; attach it with
//! [`Chart::add_indicator`](ferrochart_core::Chart::add_indicator). The
//! formulas, seeds and warmups are specified in `docs/book/src/indicators.md`; all
//! indicators are tested against a numpy reference implementation, and those
//! TA-Lib has against TA-Lib.
//!
//! ```
//! use ferrochart_core::chart::SeriesOptions;
//! use ferrochart_core::frame::SeriesKind;
//! use ferrochart_core::plugin::Placement;
//! use ferrochart_core::testing::random_walk;
//! use ferrochart_core::chart::SeriesData;
//! use ferrochart_core::{Chart, ChartOptions};
//! use ferrochart_indicators::{Source, macd, sma};
//!
//! let mut chart = Chart::new(ChartOptions::default());
//! let candles = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
//! chart.set_data(candles, SeriesData::Ohlc(random_walk(7, 500, 0, 60_000)?))?;
//! chart.add_indicator(candles, Box::new(sma(20, Source::Close)?), Placement::Overlay)?;
//! chart.add_indicator(candles, Box::new(macd(12, 26, 9, Source::Close)?), Placement::NewPane)?;
//! # Ok::<(), ferrochart_core::Error>(())
//! ```
#![forbid(unsafe_code)]

mod calc;
mod kernel;
pub mod named;
mod runner;

use ferrochart_core::chart::BandOptions;
use ferrochart_core::data::DataChange;
use ferrochart_core::geom::Rgba;
use ferrochart_core::plugin::{
    Indicator, IndicatorInput, IndicatorOutputs, OutputKind, OutputSpec, Placement,
};
use ferrochart_core::{Error, Result};

use calc::{
    AdxCalc, AroonCalc, AtrCalc, BollCalc, Calc, CciCalc, ChangeCalc, CmfCalc, DemaCalc,
    DonchianCalc, EmaCalc, HmaCalc, IchimokuCalc, KeltnerCalc, MacdCalc, MfiCalc, ObvCalc, RsiCalc,
    SarCalc, SmaCalc, StochCalc, StochRsiCalc, SuperTrendCalc, TemaCalc, TrixCalc, VolumeMaCalc,
    VwapCalc, VwmaCalc, WillrCalc, WmaCalc,
};
pub use calc::{Source, VwapAnchor};
use runner::{Factory, Runner};

/// A built-in indicator, ready for
/// [`Chart::add_indicator`](ferrochart_core::Chart::add_indicator).
pub struct Builtin {
    name: String,
    outputs: Vec<OutputSpec>,
    placement: Placement,
    warmup: usize,
    volume: bool,
    runner: Runner,
}

impl std::fmt::Debug for Builtin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Builtin")
            .field("name", &self.name)
            .field("outputs", &self.outputs)
            .finish_non_exhaustive()
    }
}

impl Indicator for Builtin {
    fn name(&self) -> &str {
        &self.name
    }

    fn outputs(&self) -> &[OutputSpec] {
        &self.outputs
    }

    fn default_placement(&self) -> Placement {
        self.placement
    }

    fn warmup(&self) -> usize {
        self.warmup
    }

    fn needs_volume(&self) -> bool {
        self.volume
    }

    fn update(
        &mut self,
        input: &IndicatorInput<'_>,
        change: DataChange,
        out: &mut IndicatorOutputs,
    ) {
        match change {
            DataChange::Append { count } => self.runner.append(input, count, out),
            DataChange::UpdateLast => self.runner.update_last(input, out),
            DataChange::Replace | DataChange::Prepend { .. } => self.runner.full(input, out),
        }
    }
}

/// Default colors.
const BLUE: u32 = 0x0029_62FF;
const ORANGE: u32 = 0x00FF_9800;
const PURPLE: u32 = 0x009C_27B0;
const GREEN: u32 = 0x0026_A69A;
const RED: u32 = 0x00EF_5350;
const GRAY: u32 = 0x0078_7B86;

const fn out(name: &'static str, color: u32) -> OutputSpec {
    OutputSpec {
        name,
        kind: OutputKind::Line,
        color: Some(Rgba::hex(color)),
        shift: 0,
        fill_to: None,
    }
}

/// An upper line filled to the lower one (output 2) with the default color.
fn upper_band() -> OutputSpec {
    OutputSpec {
        fill_to: Some((2, BandOptions::default())),
        ..out("upper", BLUE)
    }
}

struct Def {
    name: String,
    outputs: Vec<OutputSpec>,
    placement: Placement,
    warmup: usize,
    volume: bool,
}

fn build(def: Def, factory: Factory) -> Builtin {
    Builtin {
        name: def.name,
        outputs: def.outputs,
        placement: def.placement,
        warmup: def.warmup,
        volume: def.volume,
        runner: Runner::new(factory),
    }
}

fn factory<C: Calc + 'static>(f: impl Fn() -> C + Send + 'static) -> Factory {
    Box::new(move || Box::new(f()) as Box<dyn Calc>)
}

fn period(n: usize, min: usize) -> Result<usize> {
    if n >= min {
        Ok(n)
    } else if min == 1 {
        Err(Error::InvalidArgument("periods must be at least 1"))
    } else {
        Err(Error::InvalidArgument("the period is too short"))
    }
}

fn factor(x: f64) -> Result<f64> {
    if x.is_finite() && x > 0.0 {
        Ok(x)
    } else {
        Err(Error::InvalidArgument("multipliers must be finite and > 0"))
    }
}

/// Single-output overlay on a price.
fn overlay<C: Calc + 'static>(
    name: String,
    output: &'static str,
    warmup: usize,
    volume: bool,
    f: impl Fn() -> C + Send + 'static,
) -> Builtin {
    build(
        Def {
            name,
            outputs: vec![out(output, BLUE)],
            placement: Placement::Overlay,
            warmup,
            volume,
        },
        factory(f),
    )
}

// ---- 1. moving averages ------------------------------------------------------

/// Simple moving average (`docs/book/src/indicators.md`, SMA).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn sma(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(overlay(
        format!("SMA {n}"),
        "sma",
        n - 1,
        false,
        move || SmaCalc::new(n, source),
    ))
}

/// Exponential moving average with an SMA seed (`docs/book/src/indicators.md`, EMA).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn ema(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(overlay(
        format!("EMA {n}"),
        "ema",
        n - 1,
        false,
        move || EmaCalc::new(n, source),
    ))
}

/// Linearly weighted moving average (`docs/book/src/indicators.md`, WMA).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn wma(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(overlay(
        format!("WMA {n}"),
        "wma",
        n - 1,
        false,
        move || WmaCalc::new(n, source),
    ))
}

/// Hull moving average (`docs/book/src/indicators.md`, HMA).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is less than 2.
pub fn hma(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 2)?;
    Ok(overlay(
        format!("HMA {n}"),
        "hma",
        n + n.isqrt() - 2,
        false,
        move || HmaCalc::new(n, source),
    ))
}

/// Volume-weighted moving average (`docs/book/src/indicators.md`, VWMA).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn vwma(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(overlay(
        format!("VWMA {n}"),
        "vwma",
        n - 1,
        true,
        move || VwmaCalc::new(n, source),
    ))
}

/// Double exponential moving average (`docs/book/src/indicators.md`, DEMA).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn dema(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(overlay(
        format!("DEMA {n}"),
        "dema",
        2 * (n - 1),
        false,
        move || DemaCalc::new(n, source),
    ))
}

/// Triple exponential moving average (`docs/book/src/indicators.md`, TEMA).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn tema(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(overlay(
        format!("TEMA {n}"),
        "tema",
        3 * (n - 1),
        false,
        move || TemaCalc::new(n, source),
    ))
}

/// Moving average of the volume, on the volume's pane and scale
/// (`docs/book/src/indicators.md`, Volume MA).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn volume_ma(n: usize) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(build(
        Def {
            name: format!("Volume MA {n}"),
            outputs: vec![out("vma", ORANGE)],
            placement: Placement::Volume,
            warmup: n - 1,
            volume: true,
        },
        factory(move || VolumeMaCalc::new(n)),
    ))
}

// ---- 2. channels -------------------------------------------------------------

/// Bollinger bands: upper, mid, lower (`docs/book/src/indicators.md`, BOLL).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0 or `k` is not finite and positive.
pub fn boll(n: usize, k: f64, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    let k = factor(k)?;
    Ok(build(
        Def {
            name: format!("BOLL {n} {k}"),
            outputs: vec![upper_band(), out("mid", ORANGE), out("lower", BLUE)],
            placement: Placement::Overlay,
            warmup: n - 1,
            volume: false,
        },
        factory(move || BollCalc::new(n, k, source)),
    ))
}

/// Keltner channel: upper, mid, lower (`docs/book/src/indicators.md`, Keltner).
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a zero period or a multiplier that is not
/// finite and positive.
pub fn keltner(n: usize, m: f64, atr_n: usize) -> Result<Builtin> {
    let (n, atr_n, m) = (period(n, 1)?, period(atr_n, 1)?, factor(m)?);
    Ok(build(
        Def {
            name: format!("Keltner {n} {m} {atr_n}"),
            outputs: vec![upper_band(), out("mid", ORANGE), out("lower", BLUE)],
            placement: Placement::Overlay,
            warmup: (n - 1).max(atr_n),
            volume: false,
        },
        factory(move || KeltnerCalc::new(n, m, atr_n)),
    ))
}

/// Donchian channel: upper, mid, lower (`docs/book/src/indicators.md`, Donchian).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn donchian(n: usize) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(build(
        Def {
            name: format!("Donchian {n}"),
            outputs: vec![out("upper", BLUE), out("mid", ORANGE), out("lower", BLUE)],
            placement: Placement::Overlay,
            warmup: n - 1,
            volume: false,
        },
        factory(move || DonchianCalc::new(n)),
    ))
}

/// Ichimoku cloud: tenkan, kijun, `span_a`, `span_b` (drawn `shift` bars
/// later), chikou (drawn `shift` bars earlier) (`docs/book/src/indicators.md`,
/// Ichimoku).
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a zero period or shift.
pub fn ichimoku(tenkan: usize, kijun: usize, senkou: usize, shift: usize) -> Result<Builtin> {
    let (t, k, s) = (period(tenkan, 1)?, period(kijun, 1)?, period(senkou, 1)?);
    let shift = period(shift, 1)?;
    let d = i32::try_from(shift).map_err(|_| Error::InvalidArgument("the shift is too large"))?;
    let shifted = |name, color, by| OutputSpec {
        shift: by,
        ..out(name, color)
    };
    Ok(build(
        Def {
            name: format!("Ichimoku {t} {k} {s} {shift}"),
            outputs: vec![
                out("tenkan", BLUE),
                out("kijun", RED),
                OutputSpec {
                    // The cloud: green where span A is above span B, red below.
                    fill_to: Some((
                        3,
                        BandOptions {
                            fill: Some(Rgba::hex(GREEN).with_alpha(0x33)),
                            fill_crossed: Some(Rgba::hex(RED).with_alpha(0x33)),
                        },
                    )),
                    ..shifted("span_a", GREEN, d)
                },
                shifted("span_b", RED, d),
                shifted("chikou", PURPLE, -d),
            ],
            placement: Placement::Overlay,
            warmup: (s - 1).max(k - 1).max(t - 1),
            volume: false,
        },
        factory(move || IchimokuCalc::new(t, k, s)),
    ))
}

/// Parabolic SAR, as TA-Lib computes it (`docs/book/src/indicators.md`, SAR).
///
/// # Errors
///
/// [`Error::InvalidArgument`] unless `0 < af_start ≤ af_max` and
/// `af_step > 0`, all finite.
pub fn sar(af_start: f64, af_step: f64, af_max: f64) -> Result<Builtin> {
    let (a, s, m) = (factor(af_start)?, factor(af_step)?, factor(af_max)?);
    if a > m {
        return Err(Error::InvalidArgument("af_start must not exceed af_max"));
    }
    Ok(build(
        Def {
            name: format!("SAR {a} {s} {m}"),
            outputs: vec![OutputSpec {
                kind: OutputKind::Points,
                ..out("sar", BLUE)
            }],
            placement: Placement::Overlay,
            warmup: 1,
            volume: false,
        },
        factory(move || SarCalc::new(a, s, m)),
    ))
}

/// `SuperTrend`: the lower band in up trends (`long`), the upper band in down
/// trends (`short`) (`docs/book/src/indicators.md`, `SuperTrend`).
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a zero period or a multiplier that is not
/// finite and positive.
pub fn supertrend(atr_n: usize, m: f64) -> Result<Builtin> {
    let (n, m) = (period(atr_n, 1)?, factor(m)?);
    Ok(build(
        Def {
            name: format!("SuperTrend {n} {m}"),
            outputs: vec![out("long", GREEN), out("short", RED)],
            placement: Placement::Overlay,
            warmup: n,
            volume: false,
        },
        factory(move || SuperTrendCalc::new(n, m)),
    ))
}

/// Volume-weighted average price from `anchor` on (`docs/book/src/indicators.md`, VWAP).
/// `utc_offset_ms` moves the session boundaries.
///
/// # Errors
///
/// None at the moment; `Result` for consistency with the other indicators.
pub fn vwap(anchor: VwapAnchor, utc_offset_ms: i64) -> Result<Builtin> {
    Ok(overlay("VWAP".into(), "vwap", 0, true, move || {
        VwapCalc::new(anchor, utc_offset_ms)
    }))
}

// ---- 3. oscillators ----------------------------------------------------------

/// Single-output oscillator on a new pane.
fn oscillator<C: Calc + 'static>(
    name: String,
    outputs: Vec<OutputSpec>,
    warmup: usize,
    volume: bool,
    f: impl Fn() -> C + Send + 'static,
) -> Builtin {
    build(
        Def {
            name,
            outputs,
            placement: Placement::NewPane,
            warmup,
            volume,
        },
        factory(f),
    )
}

/// MACD: macd, signal, hist (`docs/book/src/indicators.md`, MACD).
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a zero period.
pub fn macd(fast: usize, slow: usize, signal: usize, source: Source) -> Result<Builtin> {
    let (f, s, g) = (period(fast, 1)?, period(slow, 1)?, period(signal, 1)?);
    let hist = OutputSpec {
        color: Some(Rgba::hex(GRAY)),
        ..OutputSpec::histogram("hist")
    };
    Ok(oscillator(
        format!("MACD {f} {s} {g}"),
        vec![out("macd", BLUE), out("signal", ORANGE), hist],
        f.max(s) + g - 2,
        false,
        move || MacdCalc::new(f, s, g, source),
    ))
}

/// Relative strength index (`docs/book/src/indicators.md`, RSI).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn rsi(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("RSI {n}"),
        vec![out("rsi", PURPLE)],
        n,
        false,
        move || RsiCalc::new(n, source),
    ))
}

/// Stochastic oscillator: %K, %D (`docs/book/src/indicators.md`, Stoch).
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a zero period.
pub fn stoch(k: usize, smooth_k: usize, d: usize) -> Result<Builtin> {
    let (k, sk, d) = (period(k, 1)?, period(smooth_k, 1)?, period(d, 1)?);
    Ok(oscillator(
        format!("Stoch {k} {sk} {d}"),
        vec![out("k", BLUE), out("d", ORANGE)],
        k + sk + d - 3,
        false,
        move || StochCalc::new(k, sk, d),
    ))
}

/// Stochastic RSI: %K, %D (`docs/book/src/indicators.md`, `StochRSI`).
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a zero period.
pub fn stochrsi(
    rsi_n: usize,
    stoch_n: usize,
    smooth_k: usize,
    d: usize,
    source: Source,
) -> Result<Builtin> {
    let (r, s) = (period(rsi_n, 1)?, period(stoch_n, 1)?);
    let (sk, d) = (period(smooth_k, 1)?, period(d, 1)?);
    Ok(oscillator(
        format!("StochRSI {r} {s} {sk} {d}"),
        vec![out("k", BLUE), out("d", ORANGE)],
        r + s + sk + d - 3,
        false,
        move || StochRsiCalc::new(r, s, sk, d, source),
    ))
}

/// Commodity channel index (`docs/book/src/indicators.md`, CCI).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn cci(n: usize) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("CCI {n}"),
        vec![out("cci", BLUE)],
        n - 1,
        false,
        move || CciCalc::new(n),
    ))
}

/// Williams %R (`docs/book/src/indicators.md`, Williams %R).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn willr(n: usize) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("%R {n}"),
        vec![out("wr", PURPLE)],
        n - 1,
        false,
        move || WillrCalc::new(n),
    ))
}

/// Money flow index (`docs/book/src/indicators.md`, MFI).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn mfi(n: usize) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("MFI {n}"),
        vec![out("mfi", PURPLE)],
        n,
        true,
        move || MfiCalc::new(n),
    ))
}

/// ADX with the directional indicators: adx, `plus_di`, `minus_di`
/// (`docs/book/src/indicators.md`, ADX / DMI).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn adx(n: usize) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("ADX {n}"),
        vec![
            out("adx", ORANGE),
            out("plus_di", GREEN),
            out("minus_di", RED),
        ],
        2 * n - 1,
        false,
        move || AdxCalc::new(n),
    ))
}

/// Average true range (`docs/book/src/indicators.md`, ATR).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn atr(n: usize) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("ATR {n}"),
        vec![out("atr", RED)],
        n,
        false,
        move || AtrCalc::new(n),
    ))
}

/// On-balance volume (`docs/book/src/indicators.md`, OBV).
///
/// # Errors
///
/// None at the moment; `Result` for consistency with the other indicators.
pub fn obv() -> Result<Builtin> {
    Ok(oscillator(
        "OBV".into(),
        vec![out("obv", BLUE)],
        0,
        true,
        ObvCalc::default,
    ))
}

/// Chaikin money flow (`docs/book/src/indicators.md`, CMF).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn cmf(n: usize) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("CMF {n}"),
        vec![out("cmf", GREEN)],
        n - 1,
        true,
        move || CmfCalc::new(n),
    ))
}

/// Rate of change in percent (`docs/book/src/indicators.md`, ROC).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn roc(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("ROC {n}"),
        vec![out("roc", BLUE)],
        n,
        false,
        move || ChangeCalc::roc(n, source),
    ))
}

/// Momentum (`docs/book/src/indicators.md`, Momentum).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn momentum(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("MOM {n}"),
        vec![out("mom", BLUE)],
        n,
        false,
        move || ChangeCalc::momentum(n, source),
    ))
}

/// TRIX (`docs/book/src/indicators.md`, TRIX).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn trix(n: usize, source: Source) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("TRIX {n}"),
        vec![out("trix", RED)],
        3 * (n - 1) + 1,
        false,
        move || TrixCalc::new(n, source),
    ))
}

/// Aroon: up, down (`docs/book/src/indicators.md`, Aroon).
///
/// # Errors
///
/// [`Error::InvalidArgument`] if `n` is 0.
pub fn aroon(n: usize) -> Result<Builtin> {
    let n = period(n, 1)?;
    Ok(oscillator(
        format!("Aroon {n}"),
        vec![out("up", GREEN), out("down", RED)],
        n,
        false,
        move || AroonCalc::new(n),
    ))
}
