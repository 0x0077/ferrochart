//! Panes: plot areas stacked vertically that share the time scale, each with
//! its own price scale.
//!
//! Heights follow the panes' stretch factors; panes are separated by a
//! 1-physical-px separator that can be dragged (hit area ±4 CSS px). The price
//! axis has the same width in every pane (the widest labels win) and the time
//! axis sits below the last pane.
//!

use crate::data::PaneId;
use crate::format::NumberFormat;
use crate::geom::RectPx;
use crate::price_scale::PriceScale;

/// Options of a new pane.
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::pane::PaneOptions;
///
/// let mut chart = Chart::new(ChartOptions::default());
/// // An indicator pane a third as tall as the main pane.
/// let pane = chart.add_pane(PaneOptions { stretch_factor: 1.0 / 3.0 });
/// assert_eq!(chart.panes().count(), 2);
/// assert_eq!(chart.pane_stretch(pane), Some(1.0 / 3.0));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaneOptions {
    /// Share of the height relative to the other panes' factors. Values that
    /// are not finite and positive are replaced by 1.
    pub stretch_factor: f32,
}

impl Default for PaneOptions {
    fn default() -> Self {
        Self {
            stretch_factor: 1.0,
        }
    }
}

/// Rectangles of one pane in the last built layout, in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaneLayout {
    /// The plot area.
    pub plot: RectPx,
    /// The pane's part of the right price axis.
    pub price_axis: RectPx,
}

/// A pane of a chart.
#[derive(Clone, Debug)]
pub(crate) struct Pane {
    pub(crate) id: PaneId,
    pub(crate) stretch: f32,
    /// The right price scale.
    pub(crate) price_scale: PriceScale,
    /// Overlay scale of a volume series sharing the pane with candles.
    pub(crate) volume_scale: PriceScale,
    /// Price ticks of the last frame (capacity reserved, reused).
    pub(crate) ticks: Vec<f64>,
    /// Plot rectangle of the last layout.
    pub(crate) plot: RectPx,
    pub(crate) labels: NumberFormat,
    /// Whether the pane had data to scale in the last frame.
    pub(crate) has_content: bool,
}

/// Volume occupies the bottom quarter of a pane it shares with candles.
pub(crate) const VOLUME_MARGINS: (f32, f32) = (0.75, 0.0);
/// Price series leave room above and below.
pub(crate) const PRICE_MARGINS: (f32, f32) = (0.1, 0.1);
/// Separator thickness in physical px.
pub(crate) const SEPARATOR_PX: f32 = 1.0;
/// Separator hit area on each side, in CSS px.
pub(crate) const SEPARATOR_HIT_CSS: f32 = 4.0;
/// Dragging a separator keeps both panes at least this tall (CSS px).
pub(crate) const MIN_PANE_CSS: f32 = 24.0;

/// Replaces stretch factors that are not finite and positive by 1.
pub(crate) fn sanitize_stretch(s: f32) -> f32 {
    if s.is_finite() && s > 0.0 { s } else { 1.0 }
}

impl Pane {
    pub(crate) fn new(id: PaneId, stretch: f32) -> Self {
        Self {
            id,
            stretch: sanitize_stretch(stretch),
            price_scale: PriceScale::new(PRICE_MARGINS),
            volume_scale: PriceScale::new(VOLUME_MARGINS),
            ticks: Vec::with_capacity(256),
            plot: RectPx::default(),
            labels: NumberFormat::Auto(2),
            has_content: false,
        }
    }
}

/// Stacks `panes` from `top` down over `total` px (separators included) with
/// plot width `width` starting at `left`. Edges are rounded to whole pixels
/// cumulatively, so the panes tile the area exactly.
pub(crate) fn stack(panes: &mut [Pane], left: f32, top: f32, total: f32, width: f32) {
    let n = panes.len();
    if n == 0 {
        return;
    }
    let separators = SEPARATOR_PX * (n - 1) as f32;
    let avail = f64::from((total - separators).max(0.0));
    let sum: f64 = panes.iter().map(|p| f64::from(p.stretch)).sum();
    let mut acc = 0.0;
    for (i, pane) in panes.iter_mut().enumerate() {
        let gap = SEPARATOR_PX * i as f32;
        let y0 = top + (avail * acc / sum).round() as f32 + gap;
        acc += f64::from(pane.stretch);
        let y1 = top + (avail * acc / sum).round() as f32 + gap;
        pane.plot = RectPx {
            x: left,
            y: y0,
            w: width,
            h: (y1 - y0).max(0.0),
        };
    }
}

/// New stretch factors of two adjacent panes after their separator moved so
/// that the upper pane is `upper_h` and the lower pane `lower_h` px tall; the
/// sum of the two factors is kept.
pub(crate) fn split_stretch(sum: f32, upper_h: f32, lower_h: f32) -> Option<(f32, f32)> {
    let total = upper_h + lower_h;
    if !(total > 0.0 && sum.is_finite() && sum > 0.0) {
        return None;
    }
    let upper = (sum * upper_h / total).max(sum * 1e-3);
    let lower = (sum - upper).max(sum * 1e-3);
    Some((upper, lower))
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // pixel edges are whole numbers
mod tests {
    use super::*;

    fn panes(stretches: &[f32]) -> Vec<Pane> {
        stretches
            .iter()
            .enumerate()
            .map(|(i, &s)| Pane::new(PaneId(i as u32), s))
            .collect()
    }

    #[test]
    fn panes_tile_the_area() {
        let mut p = panes(&[3.0, 1.0, 1.0]);
        stack(&mut p, 0.0, 0.0, 502.0, 700.0);
        assert_eq!(p[0].plot.y, 0.0);
        assert_eq!(p[0].plot.h, 300.0);
        assert_eq!(p[1].plot.y, 301.0);
        assert_eq!(p[1].plot.h, 100.0);
        assert_eq!(p[2].plot.y, 402.0);
        assert_eq!(p[2].plot.bottom(), 502.0);
        for w in p.windows(2) {
            assert_eq!(w[0].plot.bottom() + SEPARATOR_PX, w[1].plot.y);
        }
    }

    #[test]
    fn invalid_stretch_is_one() {
        assert_eq!(sanitize_stretch(f32::NAN), 1.0);
        assert_eq!(sanitize_stretch(-2.0), 1.0);
        assert_eq!(sanitize_stretch(0.5), 0.5);
    }

    #[test]
    fn split_keeps_the_sum() {
        let (a, b) = split_stretch(2.0, 150.0, 50.0).unwrap();
        assert!((a - 1.5).abs() < 1e-6 && (b - 0.5).abs() < 1e-6);
        assert!(split_stretch(2.0, 0.0, 0.0).is_none());
    }
}
