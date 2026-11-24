//! How a chart looks and which input it takes, beyond the theme colors:
//! axes, grid and padding (r25).
//!

use crate::geom::{Css, Rgba};

/// Which grid lines are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridLines {
    /// Lines at the price ticks.
    pub horizontal: bool,
    /// Lines at the time ticks.
    pub vertical: bool,
}

impl Default for GridLines {
    fn default() -> Self {
        Self {
            horizontal: true,
            vertical: true,
        }
    }
}

impl GridLines {
    /// No grid lines.
    pub const NONE: Self = Self {
        horizontal: false,
        vertical: false,
    };
}

/// Empty space around the plot area, in CSS px. Data and grid stay out of
/// it; the pointer over it is outside the plot (no crosshair).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Padding {
    /// Above the first pane.
    pub top: Css,
    /// Between the plot area and the price axis (or the right edge).
    pub right: Css,
    /// Between the last pane and the time axis (or the bottom edge).
    pub bottom: Css,
    /// Left of the plot area.
    pub left: Css,
}

impl Padding {
    /// The same space on all four sides.
    pub const fn all(v: Css) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
}

/// Axes, grid and padding of a chart.
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::appearance::Appearance;
///
/// // A sparkline: no axes, no grid, 2 CSS px of padding.
/// let chart = Chart::new(ChartOptions { appearance: Appearance::sparkline(), ..ChartOptions::default() });
/// assert!(!chart.appearance().price_axis);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Appearance {
    /// The price axis (all panes). Hidden: the plot takes the full width and
    /// no price labels are drawn (last value, price lines, crosshair).
    pub price_axis: bool,
    /// The time axis. Hidden: the panes take the full height.
    pub time_axis: bool,
    /// Grid lines.
    pub grid: GridLines,
    /// Space around the plot area.
    pub padding: Padding,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            price_axis: true,
            time_axis: true,
            grid: GridLines::default(),
            padding: Padding::default(),
        }
    }
}

impl Appearance {
    /// No axes, no grid, 2 CSS px of padding on every side.
    pub const fn sparkline() -> Self {
        Self {
            price_axis: false,
            time_axis: false,
            grid: GridLines::NONE,
            padding: Padding::all(Css(2.0)),
        }
    }
}

/// Which gestures a chart answers (r25). Input that a switched-off
/// gesture would take is reported as not consumed, so the host can let the
/// page have it; presses still report clicks.
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::appearance::InteractionOptions;
///
/// let chart = Chart::new(ChartOptions { interaction: InteractionOptions::none(), ..ChartOptions::default() });
/// assert!(!chart.interaction().pan);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionOptions {
    /// Dragging the plot or time axis, Shift + wheel, horizontal wheel,
    /// arrow keys left/right; kinetic scrolling.
    pub pan: bool,
    /// Wheel, pinch (host or two fingers), dragging the axes, arrow keys
    /// up/down, double-clicking an axis.
    pub zoom: bool,
    /// The crosshair under the pointer or a long-pressed finger (and from a
    /// synced chart).
    pub crosshair: bool,
}

impl Default for InteractionOptions {
    fn default() -> Self {
        Self {
            pan: true,
            zoom: true,
            crosshair: true,
        }
    }
}

impl InteractionOptions {
    /// Everything off: a chart that only reports clicks.
    pub const fn none() -> Self {
        Self {
            pan: false,
            zoom: false,
            crosshair: false,
        }
    }
}

/// A dot on the crosshair at the first series' value (r25).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DotStyle {
    /// Radius in CSS px (without the 1 px ring in the background color).
    pub radius: Css,
    /// Fill color; `None`: the series' color at that value (a baseline's
    /// color above or below its base, a candle's up or down color).
    pub color: Option<Rgba>,
}

impl Default for DotStyle {
    fn default() -> Self {
        Self {
            radius: Css(4.0),
            color: None,
        }
    }
}

/// What the crosshair draws (r25).
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::appearance::{CrosshairStyle, DotStyle};
///
/// // A quote card: a vertical line and a dot on the line, no labels.
/// let style = CrosshairStyle { horizontal: false, labels: false, dot: Some(DotStyle::default()), ..CrosshairStyle::default() };
/// let chart = Chart::new(ChartOptions { crosshair_style: style, ..ChartOptions::default() });
/// assert_eq!(chart.crosshair_style(), style);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrosshairStyle {
    /// The vertical line.
    pub vertical: bool,
    /// The horizontal line.
    pub horizontal: bool,
    /// The price and time labels on the axes.
    pub labels: bool,
    /// A dot at the first series' value; `None` for none.
    pub dot: Option<DotStyle>,
}

impl Default for CrosshairStyle {
    fn default() -> Self {
        Self {
            vertical: true,
            horizontal: true,
            labels: true,
            dot: None,
        }
    }
}
