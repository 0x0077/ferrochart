//! Layer plugins: extra drawing on a pane or on the
//! whole chart, e.g. a watermark or trade marks.

use crate::drawing::{DrawingCtx, FrameBuilder};
use crate::frame::{FontId, FontStyleRef};
use crate::geom::{Css, Px, RectPx};
use crate::text::{TextMeasurer, text_width};

/// Where a layer plugin draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerPosition {
    /// Background layer, above the grid.
    Background,
    /// Main layer, below the series.
    BelowSeries,
    /// Main layer, above the series and markers, below drawings and labels.
    AboveSeries,
    /// Overlay layer, below the crosshair.
    Overlay,
}

/// Extra drawing on a pane or on the whole chart.
///
/// `build` runs in every frame and must not allocate once warmed up (the
/// frame's pools keep their capacity). When the plugin's own content
/// changes, call [`crate::Chart::invalidate_layer`].
///
/// # Examples
///
/// ```
/// use ferrochart_core::drawing::{FrameBuilder, Label};
/// use ferrochart_core::frame::TextAlign;
/// use ferrochart_core::geom::{Css, PointPx, Rgba};
/// use ferrochart_core::plugin::{LayerCtx, LayerPlugin, LayerPosition};
/// use ferrochart_core::testing::MonoMeasurer;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// struct Watermark;
///
/// impl LayerPlugin for Watermark {
///     fn position(&self) -> LayerPosition {
///         LayerPosition::Background
///     }
///     fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
///         let r = ctx.rect();
///         let center = PointPx { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 };
///         let label = Label {
///             font: ctx.font(Css(48.0)),
///             color: Rgba::hex(0x80_8080).with_alpha(0x40),
///             align: TextAlign::Center,
///             bg: None,
///         };
///         out.text(center, "BTCUSD", label);
///     }
/// }
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let id = chart.add_layer(None, Box::new(Watermark))?;
/// chart.build_frame(0.0, &MonoMeasurer);
/// chart.remove_layer(id)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub trait LayerPlugin: Send {
    /// Where it draws.
    fn position(&self) -> LayerPosition;
    /// Writes the primitives (clipped to [`LayerCtx::rect`]).
    fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>);
}

/// What a [`LayerPlugin`] knows about the chart.
pub struct LayerCtx<'a> {
    pub(crate) rect: RectPx,
    pub(crate) dpr: f32,
    pub(crate) text: &'a dyn TextMeasurer,
    pub(crate) coords: Option<DrawingCtx<'a>>,
}

impl<'a> LayerCtx<'a> {
    /// The area it draws in: the pane's plot, or for a chart-wide plugin the
    /// plots of all panes (the axes excluded).
    pub fn rect(&self) -> RectPx {
        self.rect
    }

    /// Device pixel ratio.
    pub fn dpr(&self) -> f32 {
        self.dpr
    }

    /// `v` in physical pixels.
    pub fn px(&self, v: Css) -> Px {
        v.to_px(self.dpr)
    }

    /// Font of `size`.
    pub fn font(&self, size: Css) -> FontStyleRef {
        FontStyleRef {
            font: FontId(0),
            size: size.to_px(self.dpr).0,
        }
    }

    /// Width of `text`.
    pub fn text_width(&self, text: &str, font: FontStyleRef) -> f32 {
        text_width(self.text, text, font)
    }

    /// Line height of `font`.
    pub fn line_height(&self, font: FontStyleRef) -> f32 {
        self.text.line_height(font)
    }

    /// Time/price conversions of the pane; `None` for chart-wide plugins and
    /// for panes without data.
    pub fn coords(&self) -> Option<&DrawingCtx<'a>> {
        self.coords.as_ref()
    }
}
