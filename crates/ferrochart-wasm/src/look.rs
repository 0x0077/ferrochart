//! Chart look from JavaScript (r25): themes given as objects, the axes,
//! grid and padding, the interaction switches, the crosshair style and the
//! `'sparkline'` preset.
//!

use ferrochart_core::appearance::{
    Appearance, CrosshairStyle, DotStyle, GridLines, InteractionOptions, Padding,
};
use ferrochart_core::geom::{Css, Rgba};
use ferrochart_core::theme::Theme;
use js_sys::Object;
use wasm_bindgen::prelude::*;

use crate::host::{js_err, set};
use crate::js::{color_js, get, opt_bool, opt_f64, opt_string, parse_color};

/// The theme keys (camelCase of `Theme`'s fields) besides `base`.
const THEME_COLORS: [&str; 14] = [
    "background",
    "text",
    "grid",
    "axisLine",
    "up",
    "down",
    "upWick",
    "downWick",
    "volumeUp",
    "volumeDown",
    "crosshair",
    "crosshairLabelBg",
    "crosshairLabelText",
    "line",
];

fn color_slot<'a>(t: &'a mut Theme, k: &str) -> Option<&'a mut Rgba> {
    Some(match k {
        "background" => &mut t.background,
        "text" => &mut t.text,
        "grid" => &mut t.grid,
        "axisLine" => &mut t.axis_line,
        "up" => &mut t.up,
        "down" => &mut t.down,
        "upWick" => &mut t.up_wick,
        "downWick" => &mut t.down_wick,
        "volumeUp" => &mut t.volume_up,
        "volumeDown" => &mut t.volume_down,
        "crosshair" => &mut t.crosshair,
        "crosshairLabelBg" => &mut t.crosshair_label_bg,
        "crosshairLabelText" => &mut t.crosshair_label_text,
        "line" => &mut t.line,
        _ => return None,
    })
}

/// A `theme` option: `'light'`, `'dark'`, or `{ base?, <color>…, fontSize?,
/// axisPadding? }` (CSS colors, CSS px; keys not given come from `base`,
/// default `'light'`). Unknown keys are errors (a typo would otherwise be
/// ignored silently).
pub(crate) fn theme(v: &JsValue) -> Result<Theme, JsError> {
    if let Some(s) = v.as_string() {
        return named(&s);
    }
    if !v.is_object() {
        return Err(js_err("'theme' must be 'light', 'dark' or an object"));
    }
    let mut t = match opt_string(v, "base")? {
        None => Theme::light(),
        Some(s) => named(&s)?,
    };
    for key in Object::keys(v.unchecked_ref::<Object>()).iter() {
        let Some(k) = key.as_string() else { continue };
        match k.as_str() {
            "base" => {}
            "fontSize" | "axisPadding" => {
                let x = opt_f64(v, &k)?.unwrap_or(f64::NAN);
                if !(x.is_finite() && (0.0..=100.0).contains(&x)) {
                    return Err(js_err(format!("theme '{k}' must be a number 0..=100")));
                }
                let slot = if k == "fontSize" {
                    &mut t.font_size
                } else {
                    &mut t.axis_padding
                };
                *slot = Css(x as f32);
            }
            other => {
                let Some(slot) = color_slot(&mut t, other) else {
                    return Err(js_err(format!(
                        "unknown theme key '{other}' (expected base, {}, fontSize, axisPadding)",
                        THEME_COLORS.join(", ")
                    )));
                };
                let s = get(v, other)
                    .as_string()
                    .ok_or_else(|| js_err(format!("theme '{other}' must be a CSS color")))?;
                *slot = parse_color(&s, other)?;
            }
        }
    }
    Ok(t)
}

fn named(s: &str) -> Result<Theme, JsError> {
    match s {
        "light" => Ok(Theme::light()),
        "dark" => Ok(Theme::dark()),
        other => Err(js_err(format!(
            "unknown theme '{other}' (expected 'light' or 'dark')"
        ))),
    }
}

/// The theme as the object `theme` accepts (`options().theme`).
pub(crate) fn theme_js(t: &Theme) -> JsValue {
    let o = Object::new();
    let mut t = *t;
    for k in THEME_COLORS {
        if let Some(c) = color_slot(&mut t, k) {
            set(&o, k, &color_js(Some(*c)));
        }
    }
    set(&o, "fontSize", &JsValue::from_f64(f64::from(t.font_size.0)));
    set(
        &o,
        "axisPadding",
        &JsValue::from_f64(f64::from(t.axis_padding.0)),
    );
    o.into()
}

/// Everything about a chart's look besides its theme.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Look {
    pub appearance: Appearance,
    pub interaction: InteractionOptions,
    pub crosshair: CrosshairStyle,
}

impl Look {
    /// What `preset` expands to.
    fn preset(name: &str) -> Result<Self, JsError> {
        match name {
            "sparkline" => Ok(Self {
                appearance: Appearance::sparkline(),
                interaction: InteractionOptions::none(),
                crosshair: CrosshairStyle {
                    horizontal: false,
                    labels: false,
                    dot: Some(DotStyle::default()),
                    ..CrosshairStyle::default()
                },
            }),
            other => Err(js_err(format!(
                "unknown preset '{other}' (expected 'sparkline')"
            ))),
        }
    }

    /// `self` with the keys of `o` applied: first `preset`, then the keys
    /// given; keys not given keep their value. On error `self` is unchanged.
    pub(crate) fn applied(mut self, o: &JsValue) -> Result<Self, JsError> {
        if o.is_undefined() || o.is_null() {
            return Ok(self);
        }
        if let Some(p) = opt_string(o, "preset")? {
            self = Self::preset(&p)?;
        }
        let a = &mut self.appearance;
        if let Some(b) = opt_bool(o, "priceAxis")? {
            a.price_axis = b;
        }
        if let Some(b) = opt_bool(o, "timeAxis")? {
            a.time_axis = b;
        }
        let grid = get(o, "grid");
        if let Some(b) = grid.as_bool() {
            a.grid = if b {
                GridLines::default()
            } else {
                GridLines::NONE
            };
        } else if grid.is_object() {
            if let Some(b) = opt_bool(&grid, "horizontal")? {
                a.grid.horizontal = b;
            }
            if let Some(b) = opt_bool(&grid, "vertical")? {
                a.grid.vertical = b;
            }
        } else if !grid.is_undefined() {
            return Err(js_err(
                "'grid' must be a boolean or { horizontal, vertical }",
            ));
        }
        let pad = get(o, "padding");
        if let Some(x) = pad.as_f64() {
            a.padding = Padding::all(css_len(x, "padding")?);
        } else if pad.is_object() {
            for (k, slot) in [
                ("top", &mut a.padding.top),
                ("right", &mut a.padding.right),
                ("bottom", &mut a.padding.bottom),
                ("left", &mut a.padding.left),
            ] {
                if let Some(x) = opt_f64(&pad, k)? {
                    *slot = css_len(x, k)?;
                }
            }
        } else if !pad.is_undefined() {
            return Err(js_err(
                "'padding' must be a number or { top, right, bottom, left }",
            ));
        }
        let i = get(o, "interaction");
        if let Some(b) = i.as_bool() {
            self.interaction = if b {
                InteractionOptions::default()
            } else {
                InteractionOptions::none()
            };
        } else if i.is_object() {
            let g = &mut self.interaction;
            for (k, slot) in [
                ("pan", &mut g.pan),
                ("zoom", &mut g.zoom),
                ("crosshair", &mut g.crosshair),
            ] {
                if let Some(b) = opt_bool(&i, k)? {
                    *slot = b;
                }
            }
        } else if !i.is_undefined() {
            return Err(js_err(
                "'interaction' must be a boolean or { pan, zoom, crosshair }",
            ));
        }
        let c = get(o, "crosshairStyle");
        if c.is_object() {
            let s = &mut self.crosshair;
            for (k, slot) in [
                ("vertical", &mut s.vertical),
                ("horizontal", &mut s.horizontal),
                ("labels", &mut s.labels),
            ] {
                if let Some(b) = opt_bool(&c, k)? {
                    *slot = b;
                }
            }
            let dot = get(&c, "dot");
            if dot.is_null() {
                s.dot = None;
            } else if dot.is_object() {
                let mut d = s.dot.unwrap_or_default();
                if let Some(r) = opt_f64(&dot, "radius")? {
                    if !(r.is_finite() && r > 0.0 && r <= 64.0) {
                        return Err(js_err("'dot.radius' must be a number in (0, 64]"));
                    }
                    d.radius = Css(r as f32);
                }
                if let Some(color) = crate::js::opt_color(&dot, "color")? {
                    d.color = color;
                }
                s.dot = Some(d);
            } else if !dot.is_undefined() {
                return Err(js_err("'dot' must be { radius, color } or null"));
            }
        } else if !c.is_undefined() {
            return Err(js_err(
                "'crosshairStyle' must be { vertical, horizontal, labels, dot }",
            ));
        }
        Ok(self)
    }

    /// The keys `applied` reads, with their values (`options()`).
    pub(crate) fn write(&self, o: &Object) {
        let a = &self.appearance;
        let b = |v: bool| JsValue::from_bool(v);
        let n = |c: Css| JsValue::from_f64(f64::from(c.0));
        set(o, "priceAxis", &b(a.price_axis));
        set(o, "timeAxis", &b(a.time_axis));
        let grid = Object::new();
        set(&grid, "horizontal", &b(a.grid.horizontal));
        set(&grid, "vertical", &b(a.grid.vertical));
        set(o, "grid", &grid);
        let pad = Object::new();
        set(&pad, "top", &n(a.padding.top));
        set(&pad, "right", &n(a.padding.right));
        set(&pad, "bottom", &n(a.padding.bottom));
        set(&pad, "left", &n(a.padding.left));
        set(o, "padding", &pad);
        let i = Object::new();
        set(&i, "pan", &b(self.interaction.pan));
        set(&i, "zoom", &b(self.interaction.zoom));
        set(&i, "crosshair", &b(self.interaction.crosshair));
        set(o, "interaction", &i);
        let c = Object::new();
        set(&c, "vertical", &b(self.crosshair.vertical));
        set(&c, "horizontal", &b(self.crosshair.horizontal));
        set(&c, "labels", &b(self.crosshair.labels));
        let dot = self.crosshair.dot.map_or(JsValue::NULL, |d| {
            let o = Object::new();
            set(&o, "radius", &n(d.radius));
            set(&o, "color", &color_js(d.color));
            o.into()
        });
        set(&c, "dot", &dot);
        set(o, "crosshairStyle", &c);
    }
}

fn css_len(x: f64, k: &str) -> Result<Css, JsError> {
    if x.is_finite() && (0.0..=1000.0).contains(&x) {
        Ok(Css(x as f32))
    } else {
        Err(js_err(format!("'{k}' must be a number 0..=1000 (CSS px)")))
    }
}
