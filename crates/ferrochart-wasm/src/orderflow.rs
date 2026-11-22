//! Order flow options by name (rules in `docs/book/src/orderflow.md`): the JS objects of
//! `addFootprint`, `addHeatmap` and
//! `addTpo`, and trade sides, read through a getter of dotted keys
//! (`'colors.buy'`, `'colors.0'`). Browser-independent, tested natively.
// Some helpers are only used by the wasm32 binding.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use ferrochart_core::geom::Rgba;
use ferrochart_core::theme::Theme;
use ferrochart_orderflow::{
    ColorScale, FootprintMode, FootprintOptions, HeatmapColors, HeatmapOptions, HeatmapRange,
    HeatmapSource, Side, TpoOptions,
};

use crate::params;

/// A value read from a JS object.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum V {
    /// `undefined`.
    Missing,
    /// `null`.
    Null,
    Number(f64),
    Text(String),
    Bool(bool),
    /// Anything else.
    Other,
}

type Get<'a> = &'a dyn Fn(&str) -> V;

fn number(get: Get<'_>, key: &str) -> Result<Option<f64>, String> {
    match get(key) {
        V::Missing | V::Null => Ok(None),
        V::Number(n) if n.is_finite() => Ok(Some(n)),
        _ => Err(format!("'{key}' must be a finite number")),
    }
}

fn required(get: Get<'_>, key: &str) -> Result<f64, String> {
    number(get, key)?.ok_or_else(|| format!("missing '{key}'"))
}

/// A whole number of milliseconds (or rows).
fn integer(get: Get<'_>, key: &str) -> Result<Option<i64>, String> {
    match number(get, key)? {
        Some(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Ok(Some(n as i64)),
        Some(_) => Err(format!("'{key}' must be a whole number")),
        None => Ok(None),
    }
}

fn row_size(get: Get<'_>) -> Result<Option<u32>, String> {
    integer(get, "rowSize")?
        .map(|n| {
            u32::try_from(n).map_err(|_| "'rowSize' must be a positive whole number".to_owned())
        })
        .transpose()
}

fn text(get: Get<'_>, key: &str) -> Result<Option<String>, String> {
    match get(key) {
        V::Missing | V::Null => Ok(None),
        V::Text(s) => Ok(Some(s)),
        _ => Err(format!("'{key}' must be a string")),
    }
}

fn boolean(get: Get<'_>, key: &str) -> Result<Option<bool>, String> {
    match get(key) {
        V::Missing | V::Null => Ok(None),
        V::Bool(b) => Ok(Some(b)),
        _ => Err(format!("'{key}' must be a boolean")),
    }
}

fn color(get: Get<'_>, key: &str) -> Result<Option<Rgba>, String> {
    text(get, key)?
        .map(|s| params::color(&s).ok_or_else(|| format!("'{key}': invalid color '{s}'")))
        .transpose()
}

/// `addFootprint` options: `{ tickSize, timeframeMs, rowSize?, mode?:
/// 'bidAsk' | 'delta' | 'volume', imbalanceRatio?: number | null (null:
/// off), showPoc?, colors?: { buy, sell, volume, text, poc } }`.
pub(crate) fn footprint(get: Get<'_>, theme: &Theme) -> Result<FootprintOptions, String> {
    let tf = integer(get, "timeframeMs")?.ok_or("missing 'timeframeMs'")?;
    let mut o = FootprintOptions::new(required(get, "tickSize")?, tf, theme);
    if let Some(r) = row_size(get)? {
        o.row_size = r;
    }
    if let Some(m) = text(get, "mode")? {
        o.mode = match m.as_str() {
            "bidAsk" => FootprintMode::BidAsk,
            "delta" => FootprintMode::Delta,
            "volume" => FootprintMode::Volume,
            _ => return Err(format!("unknown footprint mode '{m}'")),
        };
    }
    // Absent: the default (3); null: no imbalances.
    if get("imbalanceRatio") == V::Null {
        o.imbalance_ratio = None;
    } else if let Some(r) = number(get, "imbalanceRatio")? {
        o.imbalance_ratio = Some(r);
    }
    if let Some(b) = boolean(get, "showPoc")? {
        o.show_poc = b;
    }
    let c = &mut o.colors;
    for (key, slot) in [
        ("colors.buy", &mut c.buy),
        ("colors.sell", &mut c.sell),
        ("colors.volume", &mut c.volume),
        ("colors.text", &mut c.text),
        ("colors.poc", &mut c.poc),
    ] {
        if let Some(v) = color(get, key)? {
            *slot = v;
        }
    }
    Ok(o)
}

/// `addHeatmap` options: `{ tickSize, columnMs, source: 'depth' | 'trades',
/// rowSize?, scale?: 'log' | 'linear', range?: { percentile } | { lo, hi },
/// autoScale?, colors?: [5 CSS colors] }`.
pub(crate) fn heatmap(get: Get<'_>) -> Result<HeatmapOptions, String> {
    let source = match text(get, "source")?.as_deref() {
        Some("depth") => HeatmapSource::Depth,
        Some("trades") => HeatmapSource::Trades,
        Some(s) => return Err(format!("unknown heatmap source '{s}'")),
        None => return Err("missing 'source' ('depth' or 'trades')".to_owned()),
    };
    let col = integer(get, "columnMs")?.ok_or("missing 'columnMs'")?;
    let mut o = HeatmapOptions::new(required(get, "tickSize")?, col, source);
    if let Some(r) = row_size(get)? {
        o.row_size = r;
    }
    if let Some(s) = text(get, "scale")? {
        o.scale = match s.as_str() {
            "log" => ColorScale::Log,
            "linear" => ColorScale::Linear,
            _ => return Err(format!("unknown color scale '{s}'")),
        };
    }
    match (
        number(get, "range.percentile")?,
        number(get, "range.lo")?,
        number(get, "range.hi")?,
    ) {
        (None, None, None) => {}
        (Some(p), None, None) => {
            o.range = HeatmapRange::Auto {
                percentile: p as f32,
            };
        }
        (None, Some(lo), Some(hi)) => {
            o.range = HeatmapRange::Fixed {
                lo: lo as f32,
                hi: hi as f32,
            };
        }
        _ => return Err("'range' is { percentile } or { lo, hi }".to_owned()),
    }
    if let Some(b) = boolean(get, "autoScale")? {
        o.auto_scale = b;
    }
    if !matches!(get("colors"), V::Missing | V::Null) {
        let mut stops = [Rgba::default(); 5];
        for (k, s) in stops.iter_mut().enumerate() {
            *s = color(get, &format!("colors.{k}"))?.ok_or("'colors' must be 5 CSS colors")?;
        }
        if !matches!(get("colors.5"), V::Missing | V::Null) {
            return Err("'colors' must be 5 CSS colors".to_owned());
        }
        o.colors = HeatmapColors { stops };
    }
    Ok(o)
}

/// `addTpo` options: `{ tickSize, rowSize?, periodMs?, sessionMs?,
/// sessionOffsetMs?, valueArea?, colors?: { letters, profile, poc,
/// initialBalance } }`.
pub(crate) fn tpo(get: Get<'_>, theme: &Theme) -> Result<TpoOptions, String> {
    let mut o = TpoOptions::new(required(get, "tickSize")?, theme);
    if let Some(r) = row_size(get)? {
        o.row_size = r;
    }
    if let Some(v) = integer(get, "periodMs")? {
        o.period_ms = v;
    }
    if let Some(v) = integer(get, "sessionMs")? {
        o.session_ms = v;
    }
    if let Some(v) = integer(get, "sessionOffsetMs")? {
        o.session_offset_ms = v;
    }
    if let Some(v) = number(get, "valueArea")? {
        o.value_area = v;
    }
    let c = &mut o.colors;
    for (key, slot) in [
        ("colors.letters", &mut c.letters),
        ("colors.profile", &mut c.profile),
        ("colors.poc", &mut c.poc),
        ("colors.initialBalance", &mut c.initial_balance),
    ] {
        if let Some(v) = color(get, key)? {
            *slot = v;
        }
    }
    Ok(o)
}

/// Trade sides: 1 buy, -1 sell, 0 unknown.
pub(crate) fn sides(values: &[f64]) -> Result<Vec<Side>, usize> {
    values
        .iter()
        .enumerate()
        .map(|(i, &v)| match v {
            1.0 => Ok(Side::Buy),
            -1.0 => Ok(Side::Sell),
            0.0 => Ok(Side::Unknown),
            _ => Err(i),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::collections::HashMap;

    use super::*;

    fn reader(pairs: &[(&str, V)]) -> impl Fn(&str) -> V + use<> {
        let map: HashMap<String, V> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        move |k| map.get(k).cloned().unwrap_or(V::Missing)
    }

    fn t(s: &str) -> V {
        V::Text(s.to_owned())
    }

    #[test]
    fn footprint_options() {
        let theme = Theme::dark();
        let get = reader(&[
            ("tickSize", V::Number(0.25)),
            ("timeframeMs", V::Number(60_000.0)),
            ("rowSize", V::Number(4.0)),
            ("mode", t("delta")),
            ("showPoc", V::Bool(false)),
            ("colors", V::Other),
            ("colors.buy", t("#00ff00")),
        ]);
        let o = footprint(&get, &theme).unwrap();
        assert_eq!((o.tick_size, o.timeframe_ms, o.row_size), (0.25, 60_000, 4));
        assert_eq!(
            (o.mode, o.show_poc, o.imbalance_ratio),
            (FootprintMode::Delta, false, Some(3.0))
        );
        assert_eq!(o.colors.buy, Rgba::hex(0x00_FF_00));
        let null = reader(&[
            ("tickSize", V::Number(1.0)),
            ("timeframeMs", V::Number(1.0)),
            ("imbalanceRatio", V::Null),
        ]);
        assert_eq!(footprint(&null, &theme).unwrap().imbalance_ratio, None);
        for bad in [
            reader(&[("tickSize", V::Number(1.0))]),
            reader(&[
                ("tickSize", V::Number(1.0)),
                ("timeframeMs", V::Number(1.5)),
            ]),
            reader(&[
                ("tickSize", V::Number(1.0)),
                ("timeframeMs", V::Number(1.0)),
                ("mode", t("x")),
            ]),
            reader(&[
                ("tickSize", V::Number(1.0)),
                ("timeframeMs", V::Number(1.0)),
                ("rowSize", V::Number(-1.0)),
            ]),
            reader(&[
                ("tickSize", V::Number(1.0)),
                ("timeframeMs", V::Number(1.0)),
                ("showPoc", V::Number(1.0)),
            ]),
            reader(&[
                ("tickSize", V::Number(1.0)),
                ("timeframeMs", V::Number(1.0)),
                ("colors.poc", t("nope")),
            ]),
        ] {
            assert!(footprint(&bad, &theme).is_err());
        }
    }

    #[test]
    fn heatmap_options() {
        let get = reader(&[
            ("tickSize", V::Number(0.5)),
            ("columnMs", V::Number(1_000.0)),
            ("source", t("trades")),
            ("scale", t("linear")),
            ("range", V::Other),
            ("range.lo", V::Number(1.0)),
            ("range.hi", V::Number(9.0)),
            ("autoScale", V::Bool(true)),
            ("colors", V::Other),
            ("colors.0", t("#000")),
            ("colors.1", t("#111")),
            ("colors.2", t("#222")),
            ("colors.3", t("#333")),
            ("colors.4", t("#fff")),
        ]);
        let o = heatmap(&get).unwrap();
        assert_eq!(
            (o.source, o.scale, o.auto_scale),
            (HeatmapSource::Trades, ColorScale::Linear, true)
        );
        assert_eq!(o.range, HeatmapRange::Fixed { lo: 1.0, hi: 9.0 });
        assert_eq!(o.colors.stops[4], Rgba::hex(0xFF_FF_FF));
        let p = reader(&[
            ("tickSize", V::Number(0.5)),
            ("columnMs", V::Number(1_000.0)),
            ("source", t("depth")),
            ("range.percentile", V::Number(0.95)),
        ]);
        assert_eq!(
            heatmap(&p).unwrap().range,
            HeatmapRange::Auto { percentile: 0.95 }
        );
        for bad in [
            reader(&[("tickSize", V::Number(0.5)), ("columnMs", V::Number(1.0))]),
            reader(&[
                ("tickSize", V::Number(0.5)),
                ("columnMs", V::Number(1.0)),
                ("source", t("x")),
            ]),
            reader(&[
                ("tickSize", V::Number(0.5)),
                ("columnMs", V::Number(1.0)),
                ("source", t("depth")),
                ("range.lo", V::Number(1.0)),
            ]),
            reader(&[
                ("tickSize", V::Number(0.5)),
                ("columnMs", V::Number(1.0)),
                ("source", t("depth")),
                ("colors", V::Other),
                ("colors.0", t("#000")),
            ]),
        ] {
            assert!(heatmap(&bad).is_err());
        }
    }

    #[test]
    fn tpo_options_and_sides() {
        let get = reader(&[
            ("tickSize", V::Number(0.25)),
            ("rowSize", V::Number(4.0)),
            ("periodMs", V::Number(900_000.0)),
            ("sessionMs", V::Number(23_400_000.0)),
            ("sessionOffsetMs", V::Number(52_200_000.0)),
            ("valueArea", V::Number(0.68)),
            ("colors.initialBalance", t("rgb(1, 2, 3)")),
        ]);
        let o = tpo(&get, &Theme::light()).unwrap();
        assert_eq!(
            (
                o.row_size,
                o.period_ms,
                o.session_ms,
                o.session_offset_ms,
                o.value_area
            ),
            (4, 900_000, 23_400_000, 52_200_000, 0.68)
        );
        assert_eq!(
            o.colors.initial_balance,
            Rgba {
                r: 1,
                g: 2,
                b: 3,
                a: 255
            }
        );
        assert!(tpo(&reader(&[]), &Theme::light()).is_err());
        assert_eq!(
            sides(&[1.0, -1.0, 0.0]).unwrap(),
            vec![Side::Buy, Side::Sell, Side::Unknown]
        );
        assert_eq!(sides(&[1.0, 2.0]), Err(1));
    }
}
