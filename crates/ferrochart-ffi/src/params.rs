//! Option values by name → chart types: CSS
//! colors, line styles and enum names, indicators by name with their
//! parameters (defaults from `docs/book/src/indicators.md`), and series transforms by
//! name (`docs/book/src/series.md`). Names are the JS (`camelCase`) ones; the Python
//! binding and the C ABI convert from `snake_case`.
//!
//! Shared source: the wasm and Python bindings include this file with
//! `#[path]` (it lives here because this crate is published on crates.io).
// Each binding uses a different part.
#![allow(dead_code)]

use std::fmt::Write as _;

use ferrochart_core::chart::{MarkerPosition, MarkerShape};
use ferrochart_core::frame::{LineDash, SeriesKind};
use ferrochart_core::geom::Rgba;
use ferrochart_core::interaction::CrosshairMode;
use ferrochart_core::plugin::SeriesTransform;
use ferrochart_core::price_scale::PriceScaleMode;
use ferrochart_core::transform::{Kagi, PointFigure, RangeBars, Renko, Reversal};
use ferrochart_indicators::{self as ind, Builtin};

/// A parameter value read from a JS object (or a Python dict, or C JSON).
pub(crate) use ferrochart_indicators::named::Value;

/// Parses a CSS color: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
/// `rgb(r, g, b)`, `rgba(r, g, b, a)` (alpha 0–1) or `transparent`.
pub(crate) fn color(s: &str) -> Option<Rgba> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("transparent") {
        return Some(Rgba::default());
    }
    if let Some(hex) = s.strip_prefix('#') {
        let digit = |i: usize| u8::from_str_radix(hex.get(i..=i)?, 16).ok();
        let pair = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        return match hex.len() {
            3 | 4 => Some(Rgba {
                r: digit(0)? * 17,
                g: digit(1)? * 17,
                b: digit(2)? * 17,
                a: if hex.len() == 4 { digit(3)? * 17 } else { 255 },
            }),
            6 | 8 => Some(Rgba {
                r: pair(0)?,
                g: pair(2)?,
                b: pair(4)?,
                a: if hex.len() == 8 { pair(6)? } else { 255 },
            }),
            _ => None,
        };
    }
    let lower = s.to_ascii_lowercase();
    let inner = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<f64> = inner
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let channel = |v: f64| (0.0..=255.0).contains(&v).then(|| v.round() as u8);
    match parts.as_slice() {
        [r, g, b] => Some(Rgba {
            r: channel(*r)?,
            g: channel(*g)?,
            b: channel(*b)?,
            a: 255,
        }),
        [r, g, b, a] if (0.0..=1.0).contains(a) => Some(Rgba {
            r: channel(*r)?,
            g: channel(*g)?,
            b: channel(*b)?,
            a: (a * 255.0).round() as u8,
        }),
        _ => None,
    }
}

/// `#rrggbb`, or `rgba(r, g, b, a)` for translucent colors.
pub(crate) fn color_css(c: Rgba) -> String {
    let mut s = String::with_capacity(9);
    if c.a == 255 {
        let _ = write!(s, "#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    } else {
        let a = (f64::from(c.a) / 255.0 * 1000.0).round() / 1000.0;
        let _ = write!(s, "rgba({}, {}, {}, {a})", c.r, c.g, c.b);
    }
    s
}

pub(crate) fn line_dash(s: &str) -> Option<LineDash> {
    Some(match s {
        "solid" => LineDash::Solid,
        "dashed" => LineDash::Dashed,
        "dotted" => LineDash::Dotted,
        _ => return None,
    })
}

pub(crate) fn line_dash_name(d: LineDash) -> &'static str {
    match d {
        LineDash::Solid => "solid",
        LineDash::Dashed => "dashed",
        LineDash::Dotted => "dotted",
    }
}

/// Kinds a host can add (volume series have their own method).
pub(crate) fn series_kind(s: &str) -> Option<SeriesKind> {
    Some(match s {
        "candlestick" => SeriesKind::Candlestick,
        "ohlc" => SeriesKind::OhlcBar,
        "heikinAshi" => SeriesKind::HeikinAshi,
        "line" => SeriesKind::Line,
        "area" => SeriesKind::Area,
        "baseline" => SeriesKind::Baseline,
        "histogram" => SeriesKind::Histogram,
        "points" => SeriesKind::Points,
        _ => return None,
    })
}

pub(crate) fn series_kind_name(k: SeriesKind) -> &'static str {
    match k {
        SeriesKind::Candlestick => "candlestick",
        SeriesKind::OhlcBar => "ohlc",
        SeriesKind::HeikinAshi => "heikinAshi",
        SeriesKind::Line => "line",
        SeriesKind::Area => "area",
        SeriesKind::Baseline => "baseline",
        SeriesKind::Histogram => "histogram",
        SeriesKind::Points => "points",
        SeriesKind::Band => "band",
        SeriesKind::Volume => "volume",
        SeriesKind::Custom => "custom",
        SeriesKind::Kagi => "kagi",
        SeriesKind::PointFigure => "pointFigure",
    }
}

pub(crate) fn scale_mode(s: &str) -> Option<PriceScaleMode> {
    Some(match s {
        "normal" => PriceScaleMode::Normal,
        "logarithmic" => PriceScaleMode::Logarithmic,
        "percentage" => PriceScaleMode::Percentage,
        "indexedTo100" => PriceScaleMode::IndexedTo100,
        _ => return None,
    })
}

pub(crate) fn scale_mode_name(m: PriceScaleMode) -> &'static str {
    match m {
        PriceScaleMode::Normal => "normal",
        PriceScaleMode::Logarithmic => "logarithmic",
        PriceScaleMode::Percentage => "percentage",
        PriceScaleMode::IndexedTo100 => "indexedTo100",
    }
}

pub(crate) fn crosshair_mode(s: &str) -> Option<CrosshairMode> {
    Some(match s {
        "normal" => CrosshairMode::Normal,
        "magnet" => CrosshairMode::Magnet,
        "hidden" => CrosshairMode::Hidden,
        _ => return None,
    })
}

pub(crate) fn crosshair_mode_name(m: CrosshairMode) -> &'static str {
    match m {
        CrosshairMode::Normal => "normal",
        CrosshairMode::Magnet => "magnet",
        CrosshairMode::Hidden => "hidden",
    }
}

pub(crate) fn marker_position(s: &str) -> Option<MarkerPosition> {
    Some(match s {
        "aboveBar" => MarkerPosition::AboveBar,
        "belowBar" => MarkerPosition::BelowBar,
        _ => return None,
    })
}

pub(crate) fn marker_shape(s: &str) -> Option<MarkerShape> {
    Some(match s {
        "circle" => MarkerShape::Circle,
        "square" => MarkerShape::Square,
        "arrowUp" => MarkerShape::ArrowUp,
        "arrowDown" => MarkerShape::ArrowDown,
        _ => return None,
    })
}

/// Indicator names accepted by `addIndicator` (shared with the Rust API:
/// [`ferrochart_indicators::named`]).
pub(crate) const INDICATORS: [&str; 30] = ind::named::NAMES;

/// Builds indicator `name` (a JS name) from parameters read with `get`
/// (missing ones take the defaults of `docs/book/src/indicators.md`).
pub(crate) fn indicator(name: &str, get: impl Fn(&str) -> Value) -> Result<Builtin, String> {
    ind::named::build(name, get).map_err(|e| e.to_string())
}

/// Transform names accepted by `addTransformSeries`, in the order of
/// `docs/book/src/series.md`.
pub(crate) const TRANSFORMS: [&str; 4] = ["renko", "rangeBars", "kagi", "pointFigure"];

/// Builds transform `name` from parameters read with `get`
/// (`docs/book/src/series.md`):
/// - `renko`: `{ boxSize }` or `{ atr }` (ATR period), `kind?`;
/// - `rangeBars`: `{ range }`, `kind?`;
/// - `kagi`: `{ reversal }` (price) or `{ reversalPercent }`;
/// - `pointFigure`: `{ boxSize, reversal? }` (boxes, default 3).
///
/// `kind` is `'candlestick'` (default) or `'ohlc'`.
pub(crate) fn transform(
    name: &str,
    get: impl Fn(&str) -> Value,
) -> Result<Box<dyn SeriesTransform>, String> {
    let num = |key: &str| -> Result<Option<f64>, String> {
        match get(key) {
            Value::Missing => Ok(None),
            Value::Number(v) if v.is_finite() => Ok(Some(v)),
            _ => Err(format!("'{key}' must be a finite number")),
        }
    };
    let int = |key: &str| -> Result<Option<u32>, String> {
        match num(key)? {
            None => Ok(None),
            Some(v) if v.fract() == 0.0 && (1.0..=1e9).contains(&v) => Ok(Some(v as u32)),
            Some(_) => Err(format!("'{key}' must be a positive integer")),
        }
    };
    let need = |key: &str| num(key)?.ok_or_else(|| format!("'{name}' needs '{key}'"));
    let kind = || -> Result<SeriesKind, String> {
        match get("kind") {
            Value::Missing => Ok(SeriesKind::Candlestick),
            Value::Text(s) if s == "candlestick" => Ok(SeriesKind::Candlestick),
            Value::Text(s) if s == "ohlc" => Ok(SeriesKind::OhlcBar),
            _ => Err("'kind' must be 'candlestick' or 'ohlc'".into()),
        }
    };
    let err = |e: ferrochart_core::Error| e.to_string();
    Ok(match name {
        "renko" => {
            let r = match (num("boxSize")?, int("atr")?) {
                (Some(b), None) => Renko::fixed(b),
                (None, Some(n)) => Renko::atr(n as usize),
                _ => return Err("'renko' needs one of 'boxSize' and 'atr'".into()),
            };
            let kind = kind()?;
            Box::new(r.and_then(|r| r.with_kind(kind)).map_err(err)?)
        }
        "rangeBars" => {
            let kind = kind()?;
            Box::new(
                RangeBars::new(need("range")?)
                    .and_then(|r| r.with_kind(kind))
                    .map_err(err)?,
            )
        }
        "kagi" => {
            let rev = match (num("reversal")?, num("reversalPercent")?) {
                (Some(x), None) => Reversal::Fixed(x),
                (None, Some(p)) => Reversal::Percent(p),
                _ => return Err("'kagi' needs one of 'reversal' and 'reversalPercent'".into()),
            };
            Box::new(Kagi::new(rev).map_err(err)?)
        }
        "pointFigure" => Box::new(
            PointFigure::new(need("boxSize")?, int("reversal")?.unwrap_or(3)).map_err(err)?,
        ),
        _ => {
            return Err(format!(
                "unknown transform '{name}' (expected one of: {})",
                TRANSFORMS.join(", ")
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrochart_core::plugin::Indicator;

    #[test]
    fn css_colors() {
        assert_eq!(color("#26a69a"), Some(Rgba::hex(0x0026_A69A)));
        assert_eq!(color("#F00"), Some(Rgba::hex(0x00FF_0000)));
        assert_eq!(
            color("#ff000080"),
            Some(Rgba::hex(0x00FF_0000).with_alpha(0x80))
        );
        assert_eq!(
            color("#f008"),
            Some(Rgba::hex(0x00FF_0000).with_alpha(0x88))
        );
        assert_eq!(
            color("rgba(38, 166, 154, 0.5)"),
            Some(Rgba::hex(0x0026_A69A).with_alpha(128))
        );
        assert_eq!(color("rgb(1 2 3)"), Some(Rgba::hex(0x0001_0203)));
        assert_eq!(color("transparent").map(|c| c.a), Some(0));
        for bad in [
            "",
            "#12",
            "#gggggg",
            "rgb(1,2)",
            "rgb(300,0,0)",
            "red",
            "rgba(1,2,3,2)",
        ] {
            assert_eq!(color(bad), None, "{bad}");
        }
        for c in [
            Rgba::hex(0x0026_A69A),
            Rgba::hex(0x0012_3456).with_alpha(51),
        ] {
            assert_eq!(color(&color_css(c)), Some(c));
        }
    }

    #[test]
    fn names_round_trip() {
        for k in [
            "candlestick",
            "ohlc",
            "heikinAshi",
            "line",
            "area",
            "baseline",
            "histogram",
        ] {
            assert_eq!(series_kind_name(series_kind(k).unwrap()), k);
        }
        for m in ["normal", "logarithmic", "percentage", "indexedTo100"] {
            assert_eq!(scale_mode_name(scale_mode(m).unwrap()), m);
        }
        for m in ["normal", "magnet", "hidden"] {
            assert_eq!(crosshair_mode_name(crosshair_mode(m).unwrap()), m);
        }
        for d in ["solid", "dashed", "dotted"] {
            assert_eq!(line_dash_name(line_dash(d).unwrap()), d);
        }
        assert_eq!(series_kind("volume"), None);
    }

    #[test]
    fn every_indicator_builds_with_defaults() {
        for name in INDICATORS {
            let ind = indicator(name, |_| Value::Missing).unwrap();
            assert!(!ind.outputs().is_empty(), "{name}");
        }
        assert!(indicator("nope", |_| Value::Missing).is_err());
    }

    #[test]
    fn indicator_parameters() {
        let get = |k: &str| match k {
            "n" => Value::Number(50.0),
            "source" => Value::Text("hl2".into()),
            _ => Value::Missing,
        };
        assert_eq!(indicator("sma", get).unwrap().name(), "SMA 50");
        let bad = |v: Value| move |k: &str| if k == "n" { v.clone() } else { Value::Missing };
        assert!(indicator("sma", bad(Value::Number(2.5))).is_err());
        assert!(indicator("sma", bad(Value::Number(-1.0))).is_err());
        assert!(indicator("sma", bad(Value::Text("20".into()))).is_err());
        assert!(indicator("sma", bad(Value::Number(0.0))).is_err());
        let src = |k: &str| {
            if k == "source" {
                Value::Text("x".into())
            } else {
                Value::Missing
            }
        };
        assert!(indicator("ema", src).is_err());
        let anchor = |k: &str| {
            if k == "anchor" {
                Value::Text("week".into())
            } else {
                Value::Missing
            }
        };
        assert!(indicator("vwap", anchor).is_ok());
    }

    /// Parameters from `(key, value)` pairs; other keys are missing.
    fn params<'a>(pairs: &'a [(&'static str, Value)]) -> impl Fn(&str) -> Value + 'a {
        move |k: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map_or(Value::Missing, |(_, v)| v.clone())
        }
    }

    #[test]
    fn transform_parameters() {
        use Value::{Number as N, Text as T};
        let ok = |name: &str, p: &[(&'static str, Value)]| transform(name, params(p));
        let r = ok("renko", &[("boxSize", N(0.5))]).unwrap();
        assert_eq!(
            (r.name(), r.output_kind()),
            ("renko", SeriesKind::Candlestick)
        );
        let r = ok("renko", &[("atr", N(14.0)), ("kind", T("ohlc".into()))]).unwrap();
        assert_eq!(r.output_kind(), SeriesKind::OhlcBar);
        let r = ok("rangeBars", &[("range", N(1.0))]).unwrap();
        assert_eq!(r.output_kind(), SeriesKind::Candlestick);
        let k = ok("kagi", &[("reversalPercent", N(2.0))]).unwrap();
        assert_eq!(k.output_kind(), SeriesKind::Kagi);
        assert!(ok("kagi", &[("reversal", N(1.0))]).is_ok());
        let p = ok("pointFigure", &[("boxSize", N(1.0))]).unwrap();
        assert_eq!(
            (p.output_kind(), p.box_size()),
            (SeriesKind::PointFigure, Some(1.0))
        );
        for name in TRANSFORMS {
            assert!(ok(name, &[]).is_err(), "{name} without parameters");
        }
        for (name, p) in [
            ("renko", vec![("boxSize", N(1.0)), ("atr", N(14.0))]),
            ("renko", vec![("boxSize", N(0.0))]),
            ("renko", vec![("atr", N(2.5))]),
            (
                "renko",
                vec![("boxSize", N(1.0)), ("kind", T("line".into()))],
            ),
            ("rangeBars", vec![("range", T("1".into()))]),
            (
                "kagi",
                vec![("reversal", N(1.0)), ("reversalPercent", N(1.0))],
            ),
            ("kagi", vec![("reversalPercent", N(f64::INFINITY))]),
            (
                "pointFigure",
                vec![("boxSize", N(1.0)), ("reversal", N(0.0))],
            ),
            ("pointFigure", vec![("boxSize", N(-1.0))]),
            ("heikinAshi", vec![]),
        ] {
            assert!(ok(name, &p).is_err(), "{name} {p:?}");
        }
    }
}
