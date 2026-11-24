//! A chart's look from C (r25): theme, appearance (axes, grid, padding),
//! interaction switches and crosshair style, each a versioned struct with
//! a setter and a getter. The native widgets of M6.5 use only these and
//! offscreen rendering.
//!

use ferrochart_core::appearance::{
    Appearance, CrosshairStyle, DotStyle, GridLines, InteractionOptions, Padding,
};
use ferrochart_core::geom::Css;
use ferrochart_core::theme::Theme;

use crate::convert::{rgba, rgba_u32};
use crate::{
    Error, FERROCHART_OK, FerroChart, Result, Sized32, read_sized, with_chart, write_sized,
};

/// A theme: every color `0xRRGGBBAA`. Fill with
/// [`ferrochart_theme_default`] or read with [`ferrochart_theme`], change
/// what you need, then [`ferrochart_set_theme`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FerroChartTheme {
    /// `sizeof(FerroChartTheme)`.
    pub size: u32,
    /// Background.
    pub background: u32,
    /// Axis labels and other text.
    pub text: u32,
    /// Grid lines.
    pub grid: u32,
    /// Axis lines.
    pub axis_line: u32,
    /// Rising candles and bars.
    pub up: u32,
    /// Falling candles and bars.
    pub down: u32,
    /// Wicks of rising candles.
    pub up_wick: u32,
    /// Wicks of falling candles.
    pub down_wick: u32,
    /// Volume bars of rising bars.
    pub volume_up: u32,
    /// Volume bars of falling bars.
    pub volume_down: u32,
    /// Crosshair lines.
    pub crosshair: u32,
    /// Background of the crosshair's axis labels.
    pub crosshair_label_bg: u32,
    /// Text of the crosshair's axis labels.
    pub crosshair_label_text: u32,
    /// Default line color of line series.
    pub line: u32,
    /// Font size, CSS px (0–100).
    pub font_size: f32,
    /// Padding of axis labels, CSS px (0–100).
    pub axis_padding: f32,
}

// SAFETY: repr(C), u32 size first, plain numbers.
unsafe impl Sized32 for FerroChartTheme {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

impl FerroChartTheme {
    fn from_core(t: &Theme) -> Self {
        Self {
            size: size_of::<Self>() as u32,
            background: rgba_u32(t.background),
            text: rgba_u32(t.text),
            grid: rgba_u32(t.grid),
            axis_line: rgba_u32(t.axis_line),
            up: rgba_u32(t.up),
            down: rgba_u32(t.down),
            up_wick: rgba_u32(t.up_wick),
            down_wick: rgba_u32(t.down_wick),
            volume_up: rgba_u32(t.volume_up),
            volume_down: rgba_u32(t.volume_down),
            crosshair: rgba_u32(t.crosshair),
            crosshair_label_bg: rgba_u32(t.crosshair_label_bg),
            crosshair_label_text: rgba_u32(t.crosshair_label_text),
            line: rgba_u32(t.line),
            font_size: t.font_size.0,
            axis_padding: t.axis_padding.0,
        }
    }

    fn to_core(self) -> Result<Theme> {
        for (k, v) in [
            ("font_size", self.font_size),
            ("axis_padding", self.axis_padding),
        ] {
            if !(v.is_finite() && (0.0..=100.0).contains(&v)) {
                return Err(Error::invalid(format!("'{k}' must be 0–100")));
            }
        }
        Ok(Theme {
            background: rgba(self.background),
            text: rgba(self.text),
            grid: rgba(self.grid),
            axis_line: rgba(self.axis_line),
            up: rgba(self.up),
            down: rgba(self.down),
            up_wick: rgba(self.up_wick),
            down_wick: rgba(self.down_wick),
            volume_up: rgba(self.volume_up),
            volume_down: rgba(self.volume_down),
            crosshair: rgba(self.crosshair),
            crosshair_label_bg: rgba(self.crosshair_label_bg),
            crosshair_label_text: rgba(self.crosshair_label_text),
            line: rgba(self.line),
            font_size: Css(self.font_size),
            axis_padding: Css(self.axis_padding),
        })
    }
}

/// Axes, grid and padding (r25). Fill with [`ferrochart_appearance`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FerroChartAppearance {
    /// `sizeof(FerroChartAppearance)`.
    pub size: u32,
    /// Price axis shown (0 or 1).
    pub price_axis: u8,
    /// Time axis shown (0 or 1).
    pub time_axis: u8,
    /// Horizontal grid lines (0 or 1).
    pub grid_horizontal: u8,
    /// Vertical grid lines (0 or 1).
    pub grid_vertical: u8,
    /// Space above the plot, CSS px (0–1000).
    pub padding_top: f32,
    /// Space right of the plot (left of the price axis), CSS px.
    pub padding_right: f32,
    /// Space below the plot (above the time axis), CSS px.
    pub padding_bottom: f32,
    /// Space left of the plot, CSS px.
    pub padding_left: f32,
}

// SAFETY: repr(C), u32 size first, plain numbers.
unsafe impl Sized32 for FerroChartAppearance {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

fn flag(v: u8, k: &str) -> Result<bool> {
    match v {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::invalid(format!("'{k}' must be 0 or 1"))),
    }
}

fn len(v: f32, k: &str) -> Result<Css> {
    if v.is_finite() && (0.0..=1000.0).contains(&v) {
        Ok(Css(v))
    } else {
        Err(Error::invalid(format!("'{k}' must be 0–1000 CSS px")))
    }
}

impl FerroChartAppearance {
    fn from_core(a: &Appearance) -> Self {
        Self {
            size: size_of::<Self>() as u32,
            price_axis: u8::from(a.price_axis),
            time_axis: u8::from(a.time_axis),
            grid_horizontal: u8::from(a.grid.horizontal),
            grid_vertical: u8::from(a.grid.vertical),
            padding_top: a.padding.top.0,
            padding_right: a.padding.right.0,
            padding_bottom: a.padding.bottom.0,
            padding_left: a.padding.left.0,
        }
    }

    fn to_core(self) -> Result<Appearance> {
        Ok(Appearance {
            price_axis: flag(self.price_axis, "price_axis")?,
            time_axis: flag(self.time_axis, "time_axis")?,
            grid: GridLines {
                horizontal: flag(self.grid_horizontal, "grid_horizontal")?,
                vertical: flag(self.grid_vertical, "grid_vertical")?,
            },
            padding: Padding {
                top: len(self.padding_top, "padding_top")?,
                right: len(self.padding_right, "padding_right")?,
                bottom: len(self.padding_bottom, "padding_bottom")?,
                left: len(self.padding_left, "padding_left")?,
            },
        })
    }
}

/// Which gestures the chart handles (r25). Fill with
/// [`ferrochart_interaction`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FerroChartInteraction {
    /// `sizeof(FerroChartInteraction)`.
    pub size: u32,
    /// Panning by drag, wheel and keys (0 or 1).
    pub pan: u8,
    /// Zooming by wheel, pinch and axis drags (0 or 1).
    pub zoom: u8,
    /// Crosshair under the pointer (0 or 1).
    pub crosshair: u8,
    /// Reserved, 0.
    pub reserved: u8,
}

// SAFETY: repr(C), u32 size first, plain numbers.
unsafe impl Sized32 for FerroChartInteraction {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

/// How the crosshair is drawn (r25). Fill with
/// [`ferrochart_crosshair_style`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FerroChartCrosshairStyle {
    /// `sizeof(FerroChartCrosshairStyle)`.
    pub size: u32,
    /// Vertical line (0 or 1).
    pub vertical: u8,
    /// Horizontal line (0 or 1).
    pub horizontal: u8,
    /// Axis labels (0 or 1).
    pub labels: u8,
    /// A dot on the first series' value (0 or 1).
    pub dot: u8,
    /// Dot radius, CSS px (0 < r ≤ 64).
    pub dot_radius: f32,
    /// Dot color `0xRRGGBBAA`; 0: the series' color.
    pub dot_color: u32,
}

// SAFETY: repr(C), u32 size first, plain numbers.
unsafe impl Sized32 for FerroChartCrosshairStyle {
    fn set_size(&mut self) {
        self.size = size_of::<Self>() as u32;
    }
}

impl FerroChartCrosshairStyle {
    fn from_core(s: &CrosshairStyle) -> Self {
        let dot = s.dot.unwrap_or_default();
        Self {
            size: size_of::<Self>() as u32,
            vertical: u8::from(s.vertical),
            horizontal: u8::from(s.horizontal),
            labels: u8::from(s.labels),
            dot: u8::from(s.dot.is_some()),
            dot_radius: dot.radius.0,
            dot_color: dot.color.map_or(0, rgba_u32),
        }
    }

    fn to_core(self) -> Result<CrosshairStyle> {
        let dot = if flag(self.dot, "dot")? {
            let r = self.dot_radius;
            if !(r.is_finite() && r > 0.0 && r <= 64.0) {
                return Err(Error::invalid("'dot_radius' must be in (0, 64]"));
            }
            Some(DotStyle {
                radius: Css(r),
                color: (self.dot_color != 0).then(|| rgba(self.dot_color)),
            })
        } else {
            None
        };
        Ok(CrosshairStyle {
            vertical: flag(self.vertical, "vertical")?,
            horizontal: flag(self.horizontal, "horizontal")?,
            labels: flag(self.labels, "labels")?,
            dot,
        })
    }
}

/// Fills `out` with a built-in theme: light (`dark` 0) or dark (1). Set
/// `out->size = sizeof(FerroChartTheme)` first.
///
/// # Safety
///
/// `out` is null or a struct with `size` set, valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_theme_default(dark: u32, out: *mut FerroChartTheme) -> i32 {
    crate::call(|| {
        let t = match dark {
            0 => Theme::light(),
            1 => Theme::dark(),
            _ => return Err(Error::invalid("'dark' must be 0 or 1")),
        };
        // SAFETY: valid per the contract.
        unsafe { write_sized(out, FerroChartTheme::from_core(&t), "out")? };
        Ok(FERROCHART_OK)
    })
}

/// The chart's theme (set `out->size` first).
///
/// # Safety
///
/// `chart` is a valid chart; `out` is a struct with `size` set, valid for
/// writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_theme(
    chart: *mut FerroChart,
    out: *mut FerroChartTheme,
) -> i32 {
    with_chart(chart, |c| {
        let t = FerroChartTheme::from_core(c.chart.theme());
        // SAFETY: valid per the contract.
        unsafe { write_sized(out, t, "out")? };
        Ok(FERROCHART_OK)
    })
}

/// Replaces the chart's theme; the next frame uses it, the view is kept.
/// Fields beyond `theme->size` keep their current value.
///
/// # Safety
///
/// `chart` is a valid chart; `theme` is a valid `FerroChartTheme`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_theme(
    chart: *mut FerroChart,
    theme: *const FerroChartTheme,
) -> i32 {
    with_chart(chart, |c| {
        let base = *c.chart.theme();
        // SAFETY: valid per the contract.
        let t = unsafe { read_sized(theme, FerroChartTheme::from_core(&base), "theme")? };
        c.chart.set_theme(t.to_core()?);
        Ok(FERROCHART_OK)
    })
}

/// The chart's appearance (set `out->size` first).
///
/// # Safety
///
/// `chart` is a valid chart; `out` is a struct with `size` set, valid for
/// writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_appearance(
    chart: *mut FerroChart,
    out: *mut FerroChartAppearance,
) -> i32 {
    with_chart(chart, |c| {
        let a = FerroChartAppearance::from_core(&c.chart.appearance());
        // SAFETY: valid per the contract.
        unsafe { write_sized(out, a, "out")? };
        Ok(FERROCHART_OK)
    })
}

/// Sets the axes, grid and padding; fields beyond `appearance->size` keep
/// their current value.
///
/// # Safety
///
/// `chart` is a valid chart; `appearance` is a valid `FerroChartAppearance`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_appearance(
    chart: *mut FerroChart,
    appearance: *const FerroChartAppearance,
) -> i32 {
    with_chart(chart, |c| {
        let base = FerroChartAppearance::from_core(&c.chart.appearance());
        // SAFETY: valid per the contract.
        let a = unsafe { read_sized(appearance, base, "appearance")? };
        c.chart.set_appearance(a.to_core()?);
        Ok(FERROCHART_OK)
    })
}

/// The chart's interaction switches (set `out->size` first).
///
/// # Safety
///
/// `chart` is a valid chart; `out` is a struct with `size` set, valid for
/// writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_interaction(
    chart: *mut FerroChart,
    out: *mut FerroChartInteraction,
) -> i32 {
    with_chart(chart, |c| {
        let i = c.chart.interaction();
        let v = FerroChartInteraction {
            size: size_of::<FerroChartInteraction>() as u32,
            pan: u8::from(i.pan),
            zoom: u8::from(i.zoom),
            crosshair: u8::from(i.crosshair),
            reserved: 0,
        };
        // SAFETY: valid per the contract.
        unsafe { write_sized(out, v, "out")? };
        Ok(FERROCHART_OK)
    })
}

/// Switches panning, zooming and the crosshair on or off; a gesture in
/// progress that is switched off ends.
///
/// # Safety
///
/// `chart` is a valid chart; `interaction` is a valid
/// `FerroChartInteraction`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_interaction(
    chart: *mut FerroChart,
    interaction: *const FerroChartInteraction,
) -> i32 {
    with_chart(chart, |c| {
        let cur = c.chart.interaction();
        let base = FerroChartInteraction {
            size: size_of::<FerroChartInteraction>() as u32,
            pan: u8::from(cur.pan),
            zoom: u8::from(cur.zoom),
            crosshair: u8::from(cur.crosshair),
            reserved: 0,
        };
        // SAFETY: valid per the contract.
        let i = unsafe { read_sized(interaction, base, "interaction")? };
        c.chart.set_interaction(InteractionOptions {
            pan: flag(i.pan, "pan")?,
            zoom: flag(i.zoom, "zoom")?,
            crosshair: flag(i.crosshair, "crosshair")?,
        });
        Ok(FERROCHART_OK)
    })
}

/// The chart's crosshair style (set `out->size` first).
///
/// # Safety
///
/// `chart` is a valid chart; `out` is a struct with `size` set, valid for
/// writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_crosshair_style(
    chart: *mut FerroChart,
    out: *mut FerroChartCrosshairStyle,
) -> i32 {
    with_chart(chart, |c| {
        let s = FerroChartCrosshairStyle::from_core(&c.chart.crosshair_style());
        // SAFETY: valid per the contract.
        unsafe { write_sized(out, s, "out")? };
        Ok(FERROCHART_OK)
    })
}

/// Sets how the crosshair is drawn: lines, labels and an optional dot.
///
/// # Safety
///
/// `chart` is a valid chart; `style` is a valid `FerroChartCrosshairStyle`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferrochart_set_crosshair_style(
    chart: *mut FerroChart,
    style: *const FerroChartCrosshairStyle,
) -> i32 {
    with_chart(chart, |c| {
        let base = FerroChartCrosshairStyle::from_core(&c.chart.crosshair_style());
        // SAFETY: valid per the contract.
        let s = unsafe { read_sized(style, base, "style")? };
        c.chart.set_crosshair_style(s.to_core()?);
        Ok(FERROCHART_OK)
    })
}
