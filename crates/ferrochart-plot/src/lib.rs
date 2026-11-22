//! Charts on numeric, logarithmic, category and date axes for FerroChart:
//! lines, areas, scatter, bars, waterfalls and heatmaps.
//!
//! A [`Plot`] produces the same `Frame` as a `Chart`, using immediate
//! primitives only, so every FerroChart renderer draws it unchanged (pass
//! `ChartDataView::empty()` as the data view).
//!
#![forbid(unsafe_code)]

pub mod compute;

mod axis;
mod color;
mod draw;
mod hit;
mod index;
mod interact;
mod mark;
mod plot;
mod shapes;

pub use axis::{AxisFormat, AxisId, AxisKind, AxisOptions, AxisRange};
pub use color::{ColorScale, ScaleDomain, ScaleKind};
pub use interact::{Hit, PlotEvent};
pub use mark::{
    LabelId, LabelOptions, MarkData, MarkId, MarkKind, MarkOptions, MarkPoint, Orientation, RuleId,
    RuleOptions, WaterfallStep,
};
pub use plot::{Plot, PlotLayout, PlotOptions};
