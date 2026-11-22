//! Saving and loading the state of a chart.
//!
//! [`ChartState`] is plain data: the view (bar spacing, right offset), the
//! crosshair mode, each pane's height and price scale, and the drawings. It
//! does not contain data, series or indicators, which the host creates (a
//! state is loaded into a chart the host has set up the same way). With the
//! `serde` feature every type derives `Serialize`/`Deserialize`; the format
//! is the host's choice (core does not depend on `serde_json`).

use crate::chart::Chart;
use crate::drawing::{DrawingData, DrawingTool};
use crate::error::{Error, Result};
use crate::frame::Invalidation;
use crate::geom::Css;
use crate::interaction::{CrosshairMode, Drag};
use crate::price_scale::PriceScaleMode;

/// Version written by [`Chart::save_state`]; [`Chart::load_state`] refuses
/// newer ones.
pub const STATE_VERSION: u32 = 1;

/// Saved view, layout and drawings of a chart.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::{SeriesData, SeriesOptions};
/// use ferrochart_core::drawing::{Anchor, DrawingData};
/// use ferrochart_core::frame::SeriesKind;
/// use ferrochart_core::testing::random_walk;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// // The host sets up a chart: series and data.
/// let setup = || -> Result<Chart, ferrochart_core::Error> {
///     let mut chart = Chart::new(ChartOptions::default());
///     let id = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
///     chart.set_data(id, SeriesData::Ohlc(random_walk(1, 100, 0, 60_000)?))?;
///     Ok(chart)
/// };
/// let mut chart = setup()?;
/// let pane = chart.main_pane();
/// chart.drawings().add(pane, &DrawingData::new("horizontal_line", vec![Anchor { time: 0, price: 100.0 }]))?;
/// chart.time_scale().set_bar_spacing(12.0);
/// let state = chart.save_state();
///
/// // Later: the same setup, then the saved state.
/// let mut restored = setup()?;
/// restored.load_state(state.clone())?;
/// assert_eq!(restored.save_state(), state);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChartState {
    /// Format version ([`STATE_VERSION`]).
    pub version: u32,
    /// Bar spacing.
    pub bar_spacing: Css,
    /// Empty space right of the last bar, in bars.
    pub right_offset: f64,
    /// Crosshair mode.
    pub crosshair_mode: CrosshairMode,
    /// Panes, top to bottom.
    pub panes: Vec<PaneState>,
    /// Drawings, bottom to top.
    pub drawings: Vec<SavedDrawing>,
}

/// Saved state of a pane.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaneState {
    /// Relative height.
    pub stretch: f32,
    /// The right price scale.
    pub price_scale: PriceScaleState,
}

/// Saved state of a price scale.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PriceScaleState {
    /// Mode.
    pub mode: PriceScaleMode,
    /// Whether higher prices are drawn lower.
    pub inverted: bool,
    /// Top and bottom margins.
    pub margins: (f32, f32),
    /// The range set by the user `(low, high)`; `None` while auto-scaling
    /// (the range then follows the data and is not saved).
    pub range: Option<(f64, f64)>,
}

/// A saved drawing.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SavedDrawing {
    /// Index of its pane (top to bottom).
    pub pane: usize,
    /// Kind, anchors, style and parameters.
    pub data: DrawingData,
    /// Shown.
    pub visible: bool,
    /// Locked against editing.
    pub locked: bool,
}

impl Chart {
    /// Saves the view, layout and drawings.
    pub fn save_state(&self) -> ChartState {
        let panes = self
            .panes
            .iter()
            .map(|p| {
                let s = &p.price_scale;
                PaneState {
                    stretch: p.stretch,
                    price_scale: PriceScaleState {
                        mode: s.mode(),
                        inverted: s.inverted(),
                        margins: s.margins(),
                        range: (!s.auto_scale()).then(|| s.range()),
                    },
                }
            })
            .collect();
        let drawings = self
            .drawings
            .entries
            .iter()
            // The drawing being created is not finished yet.
            .filter(|e| self.drawings.creating.is_none_or(|c| c.id != e.id))
            .filter_map(|e| {
                Some(SavedDrawing {
                    pane: self.panes.iter().position(|p| p.id == e.pane)?,
                    data: e.tool.save(),
                    visible: e.visible,
                    locked: e.locked,
                })
            })
            .collect();
        ChartState {
            version: STATE_VERSION,
            bar_spacing: Css((self.time_scale.bar_spacing() / f64::from(self.dpr)) as f32),
            right_offset: self.time_scale.right_offset(),
            crosshair_mode: self.input.crosshair,
            panes,
            drawings,
        }
    }

    /// Loads a state saved by [`save_state`](Self::save_state) into a chart
    /// with the same number of panes. The drawings replace the current ones
    /// (with `DrawingRemoved`/`DrawingCreated` events and new ids); the undo
    /// history is cleared.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`], with the chart unchanged, for a newer
    /// version, a different number of panes, a non-finite or non-positive
    /// spacing or offset, invalid margins or range, or a drawing that
    /// [`DrawingManager::add`](crate::drawing::DrawingManager::add) would
    /// refuse.
    pub fn load_state(&mut self, s: ChartState) -> Result<()> {
        if s.version > STATE_VERSION {
            return Err(Error::InvalidArgument(
                "state version is newer than supported",
            ));
        }
        if s.panes.len() != self.panes.len() {
            return Err(Error::InvalidArgument(
                "state has a different number of panes",
            ));
        }
        if !(s.bar_spacing.0.is_finite() && s.bar_spacing.0 > 0.0 && s.right_offset.is_finite()) {
            return Err(Error::InvalidArgument(
                "invalid bar spacing or right offset",
            ));
        }
        for p in &s.panes {
            let ps = &p.price_scale;
            let margin_ok = |m: f32| (0.0..1.0).contains(&m);
            let (top, bottom) = ps.margins;
            let range_ok = ps.range.is_none_or(|(lo, hi)| {
                lo.is_finite()
                    && hi.is_finite()
                    && lo < hi
                    && (ps.mode != PriceScaleMode::Logarithmic || lo > 0.0)
            });
            if !(margin_ok(top) && margin_ok(bottom) && top + bottom < 1.0 && range_ok) {
                return Err(Error::InvalidArgument("invalid price scale state"));
            }
        }
        let mut tools: Vec<(_, Box<dyn DrawingTool>, bool, bool)> =
            Vec::with_capacity(s.drawings.len());
        for d in s.drawings {
            let pane = self
                .panes
                .get(d.pane)
                .ok_or(Error::InvalidArgument("drawing on a missing pane"))?
                .id;
            tools.push((
                pane,
                self.drawings.build_saved(&d.data)?,
                d.visible,
                d.locked,
            ));
        }

        // Valid: apply.
        let ts = &mut self.time_scale;
        ts.set_bar_spacing(f64::from(s.bar_spacing.to_px(self.dpr).0));
        ts.set_right_offset(s.right_offset);
        self.input.crosshair = s.crosshair_mode;
        self.input.kinetic = None;
        if matches!(self.input.drag, Drag::EditingDrawing { .. }) {
            self.input.drag = Drag::Idle;
        }
        for (pane, p) in self.panes.iter_mut().zip(&s.panes) {
            pane.stretch = if p.stretch.is_finite() && p.stretch > 0.0 {
                p.stretch
            } else {
                1.0
            };
            let ps = &mut pane.price_scale;
            let st = p.price_scale;
            // Checked above, so these cannot fail.
            let _ = ps.set_mode(st.mode);
            let _ = ps.set_margins(st.margins.0, st.margins.1);
            ps.set_inverted(st.inverted);
            match st.range {
                Some((lo, hi)) => {
                    let _ = ps.set_range(lo, hi);
                }
                None => ps.set_auto_scale(true),
            }
        }
        self.drawings.replace_all(tools);
        self.invalidation = Invalidation::Full;
        Ok(())
    }
}
