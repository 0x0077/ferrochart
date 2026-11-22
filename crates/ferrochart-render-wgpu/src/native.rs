//! Native drawing of custom series: a plugin
//! registered with [`crate::WgpuRenderer::register_series`] draws the
//! [`CustomDraw`] commands of its kind with its own pipelines.

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::frame::{CustomDraw, Frame};

/// What a [`GpuSeries`] gets to prepare a draw.
pub struct GpuSeriesCtx<'a> {
    /// The renderer's device.
    pub device: &'a wgpu::Device,
    /// The renderer's queue. Compute work may be submitted here in
    /// [`GpuSeries::prepare`]; it runs before the frame's render passes.
    pub queue: &'a wgpu::Queue,
    /// Format of the color target the draw goes into.
    pub format: wgpu::TextureFormat,
    /// Target size in physical pixels (for pixel-to-clip-space math).
    pub viewport: [f32; 2],
    /// The frame, e.g. for `frame.styles[draw.style]`.
    pub frame: &'a Frame,
}

/// A native implementation of a custom series kind.
///
/// For every [`CustomDraw`] of its kind, the renderer calls
/// [`prepare`](Self::prepare) while recording the frame and
/// [`draw`](Self::draw) when replaying it, in the order of the frame's
/// commands. A series may be drawn once per frame; key per-series state by
/// `draw.series` and upload incrementally with
/// `data.changes_since(draw.series, synced)`. Read the plugin's data with
/// `data.custom(draw.series)` (downcast through `&dyn Any`) and point
/// positions with `data.custom_index`.
///
/// The output must match the plugin's portable path within the golden
/// tolerance; keep CPU helpers and shaders in the same operation
/// order. Implementations must contain no chart logic (colors and
/// thresholds come from the style and `draw.params`).
pub trait GpuSeries {
    /// The custom series kind it draws (`CustomSeries::kind`).
    fn kind(&self) -> &'static str;
    /// Uploads data and runs compute work for `draw`.
    fn prepare(&mut self, ctx: &GpuSeriesCtx<'_>, draw: &CustomDraw, data: &ChartDataView<'_>);
    /// Records the draw into `pass`. The scissor is already set to the clip;
    /// it may set pipelines, bind groups and vertex buffers freely, but must
    /// not change the scissor or viewport.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, draw: &CustomDraw);
}
