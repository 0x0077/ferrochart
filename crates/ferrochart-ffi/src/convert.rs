//! JSON ↔ chart types for the C ABI.
//!
//! The JSON forms are those of the Python binding's dicts (`snake_case` keys),
//! so a state saved from C loads in Python and the other way round:
//! - drawing: `{"kind", "anchors": [{"time": ms, "price"}], "style"?:
//!   {"line", "line_width", "line_dash", "fill", "text", "font_size"},
//!   "params"?: {key: number | bool | string | [numbers]}}`;
//! - state: `{"version", "bar_spacing", "right_offset", "crosshair_mode",
//!   "panes": [{"stretch", "price_scale": {"mode", "inverted", "margins":
//!   [top, bottom], "range": [low, high] | null}}], "drawings": [{"pane",
//!   "visible", "locked", "data": drawing}]}`;
//! - indicator and transform parameters: a flat object with the `snake_case`
//!   names of `docs/book/src/indicators.md` and `docs/book/src/series.md`.

use std::cell::RefCell;

use ferrochart_core::drawing::{Anchor, DrawingData, DrawingStyle, Param};
use ferrochart_core::geom::{Css, Rgba};
use ferrochart_core::interaction::CrosshairMode;
use ferrochart_core::price_scale::PriceScaleMode;
use ferrochart_core::state::{ChartState, PaneState, PriceScaleState, SavedDrawing};

use crate::json::Value;
use crate::params;

/// `snake_case` → `camelCase` (`stoch_rsi` → `stochRsi`).
pub(crate) fn camel(s: &str) -> String {
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

/// `camelCase` → `snake_case` (`indexedTo100` → `indexed_to_100`).
pub(crate) fn snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_digit = false;
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_digit() && !prev_digit && !out.is_empty() {
            out.push('_');
            out.push(c);
        } else {
            out.push(c);
        }
        prev_digit = c.is_ascii_digit();
    }
    out
}

/// Builder messages name parameters in `camelCase`; the C ABI's are
/// `snake_case`. Words in quotes are converted.
pub(crate) fn snake_message(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    for (i, part) in msg.split('\'').enumerate() {
        if i > 0 {
            out.push('\'');
        }
        if i % 2 == 1 && !part.contains(' ') {
            out.push_str(&snake(part));
        } else {
            out.push_str(part);
        }
    }
    out
}

fn check_keys(v: &Value, known: &[&str], what: &str) -> Result<(), String> {
    let members = v
        .members()
        .ok_or_else(|| format!("{what} must be a JSON object"))?;
    for (k, _) in members {
        if !known.contains(&k.as_str()) {
            return Err(format!(
                "unknown {what} key '{k}' (expected one of: {})",
                known.join(", ")
            ));
        }
    }
    Ok(())
}

fn num(v: &Value, k: &str, what: &str) -> Result<f64, String> {
    v.opt(k)
        .ok_or_else(|| format!("{what} needs '{k}'"))?
        .as_f64()
        .ok_or_else(|| format!("'{k}' must be a number"))
}

fn opt_num(v: &Value, k: &str) -> Result<Option<f64>, String> {
    v.opt(k)
        .map(|x| x.as_f64().ok_or_else(|| format!("'{k}' must be a number")))
        .transpose()
}

fn opt_str<'a>(v: &'a Value, k: &str) -> Result<Option<&'a str>, String> {
    v.opt(k)
        .map(|x| x.as_str().ok_or_else(|| format!("'{k}' must be a string")))
        .transpose()
}

fn opt_bool(v: &Value, k: &str) -> Result<Option<bool>, String> {
    v.opt(k)
        .map(|x| {
            x.as_bool()
                .ok_or_else(|| format!("'{k}' must be true or false"))
        })
        .transpose()
}

fn pair(v: &Value, what: &str) -> Result<(f64, f64), String> {
    match v.as_array() {
        Some([a, b]) => match (a.as_f64(), b.as_f64()) {
            (Some(a), Some(b)) => Ok((a, b)),
            _ => Err(format!("{what} must hold two numbers")),
        },
        _ => Err(format!("{what} must hold two numbers")),
    }
}

/// A time in ms from a JSON number (an integer within ±2^53).
fn time_ms(v: &Value, what: &str) -> Result<i64, String> {
    match v.as_f64() {
        Some(t) if t.fract() == 0.0 && t.abs() <= 9_007_199_254_740_992.0 => Ok(t as i64),
        _ => Err(format!("{what} must be an integer (ms)")),
    }
}

/// An RGBA color `0xRRGGBBAA` (the C ABI's color form).
pub(crate) fn rgba(c: u32) -> Rgba {
    let [r, g, b, a] = c.to_be_bytes();
    Rgba { r, g, b, a }
}

/// The inverse of [`rgba`].
pub(crate) fn rgba_u32(c: Rgba) -> u32 {
    u32::from_be_bytes([c.r, c.g, c.b, c.a])
}

fn css_color(v: &Value, k: &str) -> Result<Option<Rgba>, String> {
    opt_str(v, k)?
        .map(|s| params::color(s).ok_or_else(|| format!("'{k}': '{s}' is not a CSS color")))
        .transpose()
}

fn width(v: &Value, k: &str) -> Result<Option<Css>, String> {
    match opt_num(v, k)? {
        Some(w) if !(w.is_finite() && w > 0.0) => Err(format!("'{k}' must be positive")),
        w => Ok(w.map(|w| Css(w as f32))),
    }
}

fn style(base: DrawingStyle, v: &Value) -> Result<DrawingStyle, String> {
    check_keys(
        v,
        &[
            "line",
            "line_width",
            "line_dash",
            "fill",
            "text",
            "font_size",
        ],
        "drawing style",
    )?;
    let mut s = base;
    if let Some(c) = css_color(v, "line")? {
        s.line = c;
    }
    if let Some(w) = width(v, "line_width")? {
        s.line_width = w;
    }
    if let Some(d) = opt_str(v, "line_dash")? {
        s.line_dash =
            params::line_dash(d).ok_or("'line_dash' must be 'solid', 'dashed' or 'dotted'")?;
    }
    if let Some(c) = css_color(v, "fill")? {
        s.fill = c;
    }
    if let Some(c) = css_color(v, "text")? {
        s.text = c;
    }
    if let Some(w) = width(v, "font_size")? {
        s.font_size = w;
    }
    Ok(s)
}

fn param(v: &Value, key: &str) -> Result<Param, String> {
    Ok(match v {
        Value::Number(x) => Param::Number(*x),
        Value::Bool(b) => Param::Bool(*b),
        Value::String(s) => Param::Text(s.clone()),
        Value::Array(a) => Param::Numbers(
            a.iter()
                .map(|x| {
                    x.as_f64()
                        .ok_or_else(|| format!("'{key}' must contain only numbers"))
                })
                .collect::<Result<_, _>>()?,
        ),
        _ => {
            return Err(format!(
                "parameter '{key}' must be a number, bool, string or array of numbers"
            ));
        }
    })
}

/// A drawing from its JSON form.
pub(crate) fn drawing(v: &Value) -> Result<DrawingData, String> {
    check_keys(v, &["kind", "anchors", "style", "params"], "drawing")?;
    let kind = opt_str(v, "kind")?.ok_or("a drawing needs 'kind'")?;
    let anchors = v
        .opt("anchors")
        .and_then(Value::as_array)
        .ok_or("a drawing needs 'anchors' (an array)")?
        .iter()
        .map(|a| {
            check_keys(a, &["time", "price"], "anchor")?;
            let time = time_ms(
                a.opt("time").ok_or("an anchor needs 'time'")?,
                "anchor 'time'",
            )?;
            let price = num(a, "price", "an anchor")?;
            if !price.is_finite() {
                return Err("anchor 'price' must be finite".to_owned());
            }
            Ok(Anchor { time, price })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut d = DrawingData::new(kind, anchors);
    if let Some(s) = v.opt("style") {
        d.style = style(d.style, s)?;
    }
    if let Some(ps) = v.opt("params") {
        let members = ps.members().ok_or("'params' must be an object")?;
        for (k, x) in members {
            d.set(k, param(x, k)?);
        }
    }
    Ok(d)
}

/// The JSON form of a drawing.
pub(crate) fn drawing_json(d: &DrawingData) -> Value {
    let anchors = d
        .anchors
        .iter()
        .map(|a| Value::object([("time", (a.time as f64).into()), ("price", a.price.into())]))
        .collect();
    let s = &d.style;
    let style = Value::object([
        ("line", params::color_css(s.line).as_str().into()),
        ("line_width", f64::from(s.line_width.0).into()),
        ("line_dash", params::line_dash_name(s.line_dash).into()),
        ("fill", params::color_css(s.fill).as_str().into()),
        ("text", params::color_css(s.text).as_str().into()),
        ("font_size", f64::from(s.font_size.0).into()),
    ]);
    let ps = d
        .params
        .iter()
        .map(|(k, p)| {
            let v = match p {
                Param::Number(x) => Value::Number(*x),
                Param::Bool(b) => Value::Bool(*b),
                Param::Text(t) => Value::String(t.clone()),
                Param::Numbers(xs) => Value::Array(xs.iter().map(|&x| x.into()).collect()),
            };
            (k.clone(), v)
        })
        .collect();
    Value::object([
        ("kind", d.kind.as_str().into()),
        ("anchors", Value::Array(anchors)),
        ("style", style),
        ("params", Value::Object(ps)),
    ])
}

/// Python/C name of a price scale mode.
pub(crate) fn scale_mode_name(m: PriceScaleMode) -> &'static str {
    match m {
        PriceScaleMode::Normal => "normal",
        PriceScaleMode::Logarithmic => "logarithmic",
        PriceScaleMode::Percentage => "percentage",
        PriceScaleMode::IndexedTo100 => "indexed_to_100",
    }
}

/// A price scale mode by its C name.
pub(crate) fn scale_mode(s: &str) -> Option<PriceScaleMode> {
    params::scale_mode(&camel(s))
}

/// The JSON form of a state.
pub(crate) fn state_json(s: &ChartState) -> Value {
    let panes = s
        .panes
        .iter()
        .map(|p| {
            let ps = &p.price_scale;
            let range = ps.range.map_or(Value::Null, |(lo, hi)| {
                Value::Array(vec![lo.into(), hi.into()])
            });
            Value::object([
                ("stretch", f64::from(p.stretch).into()),
                (
                    "price_scale",
                    Value::object([
                        ("mode", scale_mode_name(ps.mode).into()),
                        ("inverted", ps.inverted.into()),
                        (
                            "margins",
                            Value::Array(vec![
                                f64::from(ps.margins.0).into(),
                                f64::from(ps.margins.1).into(),
                            ]),
                        ),
                        ("range", range),
                    ]),
                ),
            ])
        })
        .collect();
    let drawings = s
        .drawings
        .iter()
        .map(|d| {
            Value::object([
                ("pane", (d.pane as f64).into()),
                ("visible", d.visible.into()),
                ("locked", d.locked.into()),
                ("data", drawing_json(&d.data)),
            ])
        })
        .collect();
    Value::object([
        ("version", f64::from(s.version).into()),
        ("bar_spacing", f64::from(s.bar_spacing.0).into()),
        ("right_offset", s.right_offset.into()),
        (
            "crosshair_mode",
            params::crosshair_mode_name(s.crosshair_mode).into(),
        ),
        ("panes", Value::Array(panes)),
        ("drawings", Value::Array(drawings)),
    ])
}

/// A state from its JSON form.
pub(crate) fn state(v: &Value) -> Result<ChartState, String> {
    check_keys(
        v,
        &[
            "version",
            "bar_spacing",
            "right_offset",
            "crosshair_mode",
            "panes",
            "drawings",
        ],
        "state",
    )?;
    let version = num(v, "version", "the state")?;
    if !(0.0..=f64::from(u32::MAX)).contains(&version) || version.fract() != 0.0 {
        return Err("state: invalid 'version'".into());
    }
    let crosshair_mode = match opt_str(v, "crosshair_mode")? {
        Some(m) => params::crosshair_mode(m)
            .ok_or_else(|| format!("state: unknown crosshair mode '{m}'"))?,
        None => CrosshairMode::Normal,
    };
    let panes = v
        .opt("panes")
        .and_then(Value::as_array)
        .ok_or("the state needs 'panes' (an array)")?
        .iter()
        .map(|p| {
            check_keys(p, &["stretch", "price_scale"], "pane state")?;
            let scale = p
                .opt("price_scale")
                .ok_or("a pane state needs 'price_scale'")?;
            check_keys(
                scale,
                &["mode", "inverted", "margins", "range"],
                "price scale state",
            )?;
            let mode = opt_str(scale, "mode")?.unwrap_or("normal");
            let mode = scale_mode(mode)
                .ok_or_else(|| format!("state: unknown price scale mode '{mode}'"))?;
            let margins = pair(
                scale
                    .opt("margins")
                    .ok_or("a price scale state needs 'margins'")?,
                "'margins'",
            )?;
            let range = scale.opt("range").map(|r| pair(r, "'range'")).transpose()?;
            Ok(PaneState {
                stretch: num(p, "stretch", "a pane state")? as f32,
                price_scale: PriceScaleState {
                    mode,
                    inverted: opt_bool(scale, "inverted")?.unwrap_or(false),
                    margins: (margins.0 as f32, margins.1 as f32),
                    range,
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let drawings = v
        .opt("drawings")
        .and_then(Value::as_array)
        .ok_or("the state needs 'drawings' (an array)")?
        .iter()
        .map(|d| {
            check_keys(d, &["pane", "visible", "locked", "data"], "saved drawing")?;
            let pane = num(d, "pane", "a saved drawing")?;
            if !(0.0..1e6).contains(&pane) || pane.fract() != 0.0 {
                return Err("state: invalid drawing pane".to_owned());
            }
            Ok(SavedDrawing {
                pane: pane as usize,
                data: drawing(d.opt("data").ok_or("a saved drawing needs 'data'")?)?,
                visible: opt_bool(d, "visible")?.unwrap_or(true),
                locked: opt_bool(d, "locked")?.unwrap_or(false),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ChartState {
        version: version as u32,
        bar_spacing: Css(num(v, "bar_spacing", "the state")? as f32),
        right_offset: num(v, "right_offset", "the state")?,
        crosshair_mode,
        panes,
        drawings,
    })
}

fn param_value(v: Option<&Value>) -> params::Value {
    match v {
        None | Some(Value::Null) => params::Value::Missing,
        Some(Value::Number(x)) => params::Value::Number(*x),
        Some(Value::String(s)) => params::Value::Text(camel(s)),
        Some(_) => params::Value::Other,
    }
}

/// Builds an indicator or transform with `build` (a builder of
/// `params.rs`, which asks parameters by their `camelCase` names) from the
/// JSON object `json` (`snake_case` keys; `None`: all defaults). Keys the
/// builder never asks for are errors, reported before the builder's own
/// error; the builder first runs with every parameter missing so that names
/// it asks only after a failing parameter are known too.
pub(crate) fn build_params<T>(
    json: Option<&Value>,
    what: &str,
    build: impl Fn(&dyn Fn(&str) -> params::Value) -> Result<T, String>,
) -> Result<T, String> {
    let empty = Value::Object(Vec::new());
    let json = json.unwrap_or(&empty);
    let members = json
        .members()
        .ok_or_else(|| format!("the parameters of {what} must be a JSON object"))?;
    let asked = RefCell::new(Vec::<String>::new());
    let ask = |k: &str| {
        let key = snake(k);
        let mut a = asked.borrow_mut();
        if !a.contains(&key) {
            a.push(key);
        }
    };
    let _ = build(&|k| {
        ask(k);
        params::Value::Missing
    });
    let built = build(&|k| {
        ask(k);
        param_value(json.get(&snake(k)))
    });
    let asked = asked.into_inner();
    for (k, _) in members {
        if !asked.contains(k) {
            return Err(format!(
                "unknown parameter '{k}' for {what} (expected: {})",
                asked.join(", ")
            ));
        }
    }
    built.map_err(|m| snake_message(&m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    #[test]
    fn names_convert_both_ways() {
        for (s, c) in [
            ("stoch_rsi", "stochRsi"),
            ("indexed_to_100", "indexedTo100"),
            ("heikin_ashi", "heikinAshi"),
            ("rsi_n", "rsiN"),
            ("sma", "sma"),
        ] {
            assert_eq!(camel(s), c);
            assert_eq!(snake(c), s);
        }
        assert_eq!(
            snake_message("'rsiN' must be a non-negative integer"),
            "'rsi_n' must be a non-negative integer"
        );
    }

    #[test]
    fn indicator_parameters() {
        let p = parse(r#"{"rsi_n": 10, "source": "close"}"#).unwrap();
        assert!(
            build_params(Some(&p), "stoch_rsi", |get| params::indicator(
                "stochRsi", get
            ))
            .is_ok()
        );
        let bad = parse(r#"{"n": 10, "colour": 1}"#).unwrap();
        let e = build_params(Some(&bad), "indicator 'sma'", |get| {
            params::indicator("sma", get)
        })
        .err()
        .unwrap();
        assert!(e.contains("'colour'") && e.contains("n, source"), "{e}");
        let bad = parse(r#"{"rsi_n": 1.5}"#).unwrap();
        let e = build_params(Some(&bad), "x", |get| params::indicator("stochRsi", get))
            .err()
            .unwrap();
        assert!(e.contains("'rsi_n'"), "{e}");
        assert!(build_params(None, "x", |get| params::indicator("macd", get)).is_ok());
    }

    #[test]
    fn drawings_round_trip() {
        let text = r##"{"kind":"fib_retracement","anchors":[{"time":0,"price":1},{"time":3600000,"price":2}],
            "style":{"line":"#ff0000","line_dash":"dashed"},"params":{"levels":[0,0.5,1],"label":"x","on":true}}"##;
        let d = drawing(&parse(text).unwrap()).unwrap();
        assert_eq!(d.style.line, Rgba::hex(0x00FF_0000));
        assert_eq!(
            drawing(&parse(&drawing_json(&d).to_json()).unwrap()).unwrap(),
            d
        );
        for bad in [
            r#"{"kind":"x","anchors":[],"extra":1}"#,
            r#"{"kind":"x","anchors":[{"time":0.5,"price":1}]}"#,
            r#"{"kind":"x","anchors":[{"time":0}]}"#,
            r#"{"kind":"x","anchors":[],"style":{"line":"nope"}}"#,
            r#"{"kind":"x","anchors":[],"params":{"a":{}}}"#,
        ] {
            assert!(drawing(&parse(bad).unwrap()).is_err(), "{bad}");
        }
    }
}
