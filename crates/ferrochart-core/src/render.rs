//! The interface every rendering backend implements.
//!
//! Call order (the `&mut` borrow of `build_frame` ends before the two `&self`
//! borrows of `render`):
//!
//! ```ignore
//! if chart.needs_redraw() != Invalidation::None {
//!     chart.build_frame(now_ms, &text);
//!     renderer.render(chart.frame(), &chart.data_view())?;
//! }
//! ```
//!

use core::fmt;

use crate::chart::ChartDataView;
use crate::frame::Frame;
use crate::geom::SizePx;

/// A rendering backend.
pub trait Renderer {
    /// Uploads whatever series data changed since this renderer last synced (it
    /// keeps its own synced generation per series and asks
    /// `data.changes_since`), then draws `frame`. On error the synced generations
    /// must not advance, so the next call catches up.
    ///
    /// # Errors
    ///
    /// A [`RenderError`] if the backend cannot draw (lost surface, out of memory).
    fn render(&mut self, frame: &Frame, data: &ChartDataView<'_>) -> Result<(), RenderError>;

    /// Resizes the render target.
    fn resize(&mut self, size: SizePx);

    /// Custom series kinds this renderer draws natively; pass them to
    /// [`crate::Chart::set_native_kinds`]. None by default.
    fn native_kinds(&self) -> &[&'static str] {
        &[]
    }
}

/// Error from a [`Renderer`].
///
/// # Examples
///
/// ```
/// use ferrochart_core::render::RenderError;
///
/// assert_eq!(RenderError::Backend("surface lost").to_string(), "render backend error: surface lost");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderError {
    /// The target size is zero or too large for the backend.
    InvalidSize,
    /// Backend-specific failure.
    Backend(&'static str),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize => f.write_str("invalid render target size"),
            Self::Backend(what) => write!(f, "render backend error: {what}"),
        }
    }
}

impl std::error::Error for RenderError {}
