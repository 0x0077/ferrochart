//! A chart's look from Python (r25): `theme` as a name or a dict, the axes,
//! grid and padding, the interaction switches, the crosshair style and the
//! `'sparkline'` preset. Keys are the `snake_case` of the JavaScript options
//! and mean the same.
//!

use ferrochart_core::Chart as CoreChart;
use ferrochart_core::appearance::{
    Appearance, CrosshairStyle, DotStyle, GridLines, InteractionOptions, Padding,
};
use ferrochart_core::geom::{Css, Rgba};
use ferrochart_core::theme::Theme;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict};

use crate::err;
use crate::opts::{check_keys, color, opt_bool, opt_color, opt_f64, opt_str};

/// The keys of `Chart.apply_options`.
pub(crate) const KEYS: [&str; 8] = [
    "theme",
    "preset",
    "price_axis",
    "time_axis",
    "grid",
    "padding",
    "interaction",
    "crosshair_style",
];

/// The theme's color keys besides `base`.
const THEME_COLORS: [&str; 14] = [
    "background",
    "text",
    "grid",
    "axis_line",
    "up",
    "down",
    "up_wick",
    "down_wick",
    "volume_up",
    "volume_down",
    "crosshair",
    "crosshair_label_bg",
    "crosshair_label_text",
    "line",
];

fn color_slot<'a>(t: &'a mut Theme, k: &str) -> Option<&'a mut Rgba> {
    Some(match k {
        "background" => &mut t.background,
        "text" => &mut t.text,
        "grid" => &mut t.grid,
        "axis_line" => &mut t.axis_line,
        "up" => &mut t.up,
        "down" => &mut t.down,
        "up_wick" => &mut t.up_wick,
        "down_wick" => &mut t.down_wick,
        "volume_up" => &mut t.volume_up,
        "volume_down" => &mut t.volume_down,
        "crosshair" => &mut t.crosshair,
        "crosshair_label_bg" => &mut t.crosshair_label_bg,
        "crosshair_label_text" => &mut t.crosshair_label_text,
        "line" => &mut t.line,
        _ => return None,
    })
}

pub(crate) fn named(s: &str) -> PyResult<Theme> {
    match s {
        "light" => Ok(Theme::light()),
        "dark" => Ok(Theme::dark()),
        other => Err(err(format!(
            "unknown theme '{other}' (expected 'light' or 'dark')"
        ))),
    }
}

/// `'light'`, `'dark'` or `{ base?, <color>…, font_size?, axis_padding? }`.
pub(crate) fn theme(v: &Bound<'_, PyAny>) -> PyResult<Theme> {
    if let Ok(s) = v.extract::<String>() {
        return named(&s);
    }
    let d = v
        .cast::<PyDict>()
        .map_err(|_| err("'theme' must be 'light', 'dark' or a dict"))?;
    let mut t = match opt_str(d, "base")? {
        None => Theme::light(),
        Some(s) => named(&s)?,
    };
    for (k, _) in d.iter() {
        let k: String = k.extract()?;
        match k.as_str() {
            "base" => {}
            "font_size" | "axis_padding" => {
                let x = opt_f64(d, &k)?.unwrap_or(f64::NAN);
                if !(x.is_finite() && (0.0..=100.0).contains(&x)) {
                    return Err(err(format!("theme '{k}' must be a number 0..=100")));
                }
                let slot = if k == "font_size" {
                    &mut t.font_size
                } else {
                    &mut t.axis_padding
                };
                *slot = Css(x as f32);
            }
            other => {
                let Some(slot) = color_slot(&mut t, other) else {
                    return Err(err(format!(
                        "unknown theme key '{other}' (expected base, {}, font_size, axis_padding)",
                        THEME_COLORS.join(", ")
                    )));
                };
                let s: String = d
                    .get_item(other)?
                    .and_then(|v| v.extract().ok())
                    .ok_or_else(|| err(format!("theme '{other}' must be a CSS color")))?;
                *slot = color(&s, other)?;
            }
        }
    }
    Ok(t)
}

/// The theme as the dict `theme` accepts.
pub(crate) fn theme_py<'py>(py: Python<'py>, t: &Theme) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    let mut t = *t;
    for k in THEME_COLORS {
        if let Some(c) = color_slot(&mut t, k) {
            d.set_item(k, crate::params::color_css(*c))?;
        }
    }
    d.set_item("font_size", t.font_size.0)?;
    d.set_item("axis_padding", t.axis_padding.0)?;
    Ok(d)
}

/// Everything about a chart's look besides its theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Look {
    pub appearance: Appearance,
    pub interaction: InteractionOptions,
    pub crosshair: CrosshairStyle,
}

impl Look {
    pub(crate) fn of(c: &CoreChart) -> Self {
        Self {
            appearance: c.appearance(),
            interaction: c.interaction(),
            crosshair: c.crosshair_style(),
        }
    }

    pub(crate) fn set(self, c: &mut CoreChart) {
        c.set_appearance(self.appearance);
        c.set_interaction(self.interaction);
        c.set_crosshair_style(self.crosshair);
    }

    fn preset(name: &str) -> PyResult<Self> {
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
            other => Err(err(format!(
                "unknown preset '{other}' (expected 'sparkline')"
            ))),
        }
    }

    /// `self` with the look keys of `d` applied: first `preset`, then the
    /// keys given. On error nothing is applied.
    pub(crate) fn applied(mut self, d: &Bound<'_, PyDict>) -> PyResult<Self> {
        if let Some(p) = opt_str(d, "preset")? {
            self = Self::preset(&p)?;
        }
        let a = &mut self.appearance;
        if let Some(b) = opt_bool(d, "price_axis")? {
            a.price_axis = b;
        }
        if let Some(b) = opt_bool(d, "time_axis")? {
            a.time_axis = b;
        }
        if let Some(g) = d.get_item("grid")?.filter(|v| !v.is_none()) {
            if let Ok(b) = g.cast::<PyBool>() {
                a.grid = if b.is_true() {
                    GridLines::default()
                } else {
                    GridLines::NONE
                };
            } else if let Ok(g) = g.cast::<PyDict>() {
                check_keys(g, &["horizontal", "vertical"], "grid option")?;
                if let Some(b) = opt_bool(g, "horizontal")? {
                    a.grid.horizontal = b;
                }
                if let Some(b) = opt_bool(g, "vertical")? {
                    a.grid.vertical = b;
                }
            } else {
                return Err(err("'grid' must be a bool or {horizontal, vertical}"));
            }
        }
        if let Some(p) = d.get_item("padding")?.filter(|v| !v.is_none()) {
            if let Ok(p) = p.cast::<PyDict>() {
                check_keys(p, &["top", "right", "bottom", "left"], "padding side")?;
                for (k, slot) in [
                    ("top", &mut a.padding.top),
                    ("right", &mut a.padding.right),
                    ("bottom", &mut a.padding.bottom),
                    ("left", &mut a.padding.left),
                ] {
                    if let Some(x) = opt_f64(p, k)? {
                        *slot = css_len(x, k)?;
                    }
                }
            } else if !p.is_instance_of::<PyBool>()
                && let Ok(x) = p.extract::<f64>()
            {
                a.padding = Padding::all(css_len(x, "padding")?);
            } else {
                return Err(err(
                    "'padding' must be a number or {top, right, bottom, left}",
                ));
            }
        }
        if let Some(i) = d.get_item("interaction")?.filter(|v| !v.is_none()) {
            if let Ok(b) = i.cast::<PyBool>() {
                self.interaction = if b.is_true() {
                    InteractionOptions::default()
                } else {
                    InteractionOptions::none()
                };
            } else if let Ok(i) = i.cast::<PyDict>() {
                check_keys(i, &["pan", "zoom", "crosshair"], "interaction option")?;
                let g = &mut self.interaction;
                for (k, slot) in [
                    ("pan", &mut g.pan),
                    ("zoom", &mut g.zoom),
                    ("crosshair", &mut g.crosshair),
                ] {
                    if let Some(b) = opt_bool(i, k)? {
                        *slot = b;
                    }
                }
            } else {
                return Err(err(
                    "'interaction' must be a bool or {pan, zoom, crosshair}",
                ));
            }
        }
        if let Some(c) = d.get_item("crosshair_style")?.filter(|v| !v.is_none()) {
            let c = c.cast::<PyDict>().map_err(|_| {
                err("'crosshair_style' must be a dict {vertical, horizontal, labels, dot}")
            })?;
            check_keys(
                c,
                &["vertical", "horizontal", "labels", "dot"],
                "crosshair_style option",
            )?;
            let s = &mut self.crosshair;
            for (k, slot) in [
                ("vertical", &mut s.vertical),
                ("horizontal", &mut s.horizontal),
                ("labels", &mut s.labels),
            ] {
                if let Some(b) = opt_bool(c, k)? {
                    *slot = b;
                }
            }
            match c.get_item("dot")? {
                None => {}
                Some(v) if v.is_none() => s.dot = None,
                Some(v) => {
                    let dot = v
                        .cast::<PyDict>()
                        .map_err(|_| err("'dot' must be a dict {radius, color} or None"))?;
                    check_keys(dot, &["radius", "color"], "dot option")?;
                    let mut out = s.dot.unwrap_or_default();
                    if let Some(r) = opt_f64(dot, "radius")? {
                        if !(r.is_finite() && r > 0.0 && r <= 64.0) {
                            return Err(err("'dot' radius must be a number in (0, 64]"));
                        }
                        out.radius = Css(r as f32);
                    }
                    if let Some(col) = opt_color(dot, "color")? {
                        out.color = col;
                    }
                    s.dot = Some(out);
                }
            }
        }
        Ok(self)
    }

    /// The keys `applied` reads, with their values.
    pub(crate) fn write(&self, py: Python<'_>, d: &Bound<'_, PyDict>) -> PyResult<()> {
        let a = &self.appearance;
        d.set_item("price_axis", a.price_axis)?;
        d.set_item("time_axis", a.time_axis)?;
        let g = PyDict::new(py);
        g.set_item("horizontal", a.grid.horizontal)?;
        g.set_item("vertical", a.grid.vertical)?;
        d.set_item("grid", g)?;
        let p = PyDict::new(py);
        p.set_item("top", a.padding.top.0)?;
        p.set_item("right", a.padding.right.0)?;
        p.set_item("bottom", a.padding.bottom.0)?;
        p.set_item("left", a.padding.left.0)?;
        d.set_item("padding", p)?;
        let i = PyDict::new(py);
        i.set_item("pan", self.interaction.pan)?;
        i.set_item("zoom", self.interaction.zoom)?;
        i.set_item("crosshair", self.interaction.crosshair)?;
        d.set_item("interaction", i)?;
        let c = PyDict::new(py);
        c.set_item("vertical", self.crosshair.vertical)?;
        c.set_item("horizontal", self.crosshair.horizontal)?;
        c.set_item("labels", self.crosshair.labels)?;
        match self.crosshair.dot {
            None => c.set_item("dot", py.None())?,
            Some(dot) => {
                let o = PyDict::new(py);
                o.set_item("radius", dot.radius.0)?;
                o.set_item("color", dot.color.map(crate::params::color_css))?;
                c.set_item("dot", o)?;
            }
        }
        d.set_item("crosshair_style", c)
    }
}

fn css_len(x: f64, k: &str) -> PyResult<Css> {
    if x.is_finite() && (0.0..=1000.0).contains(&x) {
        Ok(Css(x as f32))
    } else {
        Err(err(format!("'{k}' must be a number 0..=1000 (CSS px)")))
    }
}
