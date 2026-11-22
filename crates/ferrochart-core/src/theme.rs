//! Themes: plain data (colors, sizes). All colors are resolved by the core before
//! the frame is built; backends never see "up" or "down" semantics beyond the
//! per-draw [`SeriesStyle`](crate::frame::SeriesStyle).
//!

use crate::geom::{Css, Rgba};

/// Colors and sizes of a chart.
///
/// # Examples
///
/// ```
/// use ferrochart_core::theme::Theme;
///
/// let dark = Theme::dark();
/// assert_ne!(dark.background, Theme::light().background);
/// assert_eq!(Theme::default(), Theme::light());
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Chart background.
    pub background: Rgba,
    /// Axis label text.
    pub text: Rgba,
    /// Grid lines.
    pub grid: Rgba,
    /// Axis border lines.
    pub axis_line: Rgba,
    /// Up candle body.
    pub up: Rgba,
    /// Down candle body.
    pub down: Rgba,
    /// Up candle wick.
    pub up_wick: Rgba,
    /// Down candle wick.
    pub down_wick: Rgba,
    /// Volume bar of an up bar.
    pub volume_up: Rgba,
    /// Volume bar of a down bar.
    pub volume_down: Rgba,
    /// Crosshair lines.
    pub crosshair: Rgba,
    /// Background of the crosshair's axis labels.
    pub crosshair_label_bg: Rgba,
    /// Text of the crosshair's axis labels.
    pub crosshair_label_text: Rgba,
    /// Axis label font size.
    pub font_size: Css,
    /// Padding around axis labels.
    pub axis_padding: Css,
    /// Default color of line, area and histogram series.
    pub line: Rgba,
}

const UP: Rgba = Rgba::hex(0x0026_A69A);
const DOWN: Rgba = Rgba::hex(0x00EF_5350);

impl Theme {
    /// The built-in light theme.
    pub const fn light() -> Self {
        Self {
            background: Rgba::hex(0x00FF_FFFF),
            text: Rgba::hex(0x001F_232B),
            grid: Rgba::hex(0x00EE_F0F4),
            axis_line: Rgba::hex(0x00D5_D9E0),
            up: UP,
            down: DOWN,
            up_wick: UP,
            down_wick: DOWN,
            volume_up: UP.with_alpha(110),
            volume_down: DOWN.with_alpha(110),
            crosshair: Rgba::hex(0x0095_98A1),
            crosshair_label_bg: Rgba::hex(0x0013_1722),
            crosshair_label_text: Rgba::hex(0x00FF_FFFF),
            font_size: Css(11.0),
            axis_padding: Css(6.0),
            line: Rgba::hex(0x0029_62FF),
        }
    }

    /// The built-in dark theme.
    pub const fn dark() -> Self {
        Self {
            background: Rgba::hex(0x0013_161C),
            text: Rgba::hex(0x00C9_CDD6),
            grid: Rgba::hex(0x0021_292F),
            axis_line: Rgba::hex(0x003A_404A),
            crosshair: Rgba::hex(0x0075_8696),
            crosshair_label_bg: Rgba::hex(0x0036_3A45),
            ..Self::light()
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}
