//! Platform-independent core of the FerroChart financial charting engine.
//!
//! The core holds the data model, scales, layout, LOD and interaction state, and
//! produces a backend-agnostic `Frame`. It never depends on GPU, windowing, font,
//! time or async libraries and builds for `wasm32-unknown-unknown`.
//!
#![forbid(unsafe_code)]

pub mod chart;
pub mod data;
pub mod drawing;
pub mod error;
pub mod format;
pub mod frame;
pub mod geom;
pub mod icon;
pub mod interaction;
pub mod pane;
pub mod plugin;
pub mod price_scale;
pub mod render;
mod series;
pub mod state;
pub mod sync;
#[cfg(feature = "testing")]
pub mod testing;
pub mod text;
pub mod theme;
pub mod time_scale;
pub mod transform;
pub mod transform_helpers;

pub use chart::{Chart, ChartOptions};
pub use error::{Error, Result};
