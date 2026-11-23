//! WebAssembly bindings for FerroChart: the npm package `@ferrochart/core`.
//!
//! ```js
//! import init, { FerroChart } from '@ferrochart/core';
//! await init();
//! const chart = await FerroChart.create(canvas, { theme: 'dark' }); // WebGPU, else WebGL2
//! const candles = chart.addCandlestickSeries();
//! chart.addVolumeSeries(candles);
//! candles.setData({ time: BigInt64Array, open: Float64Array, high, low, close, volume });
//! candles.update({ time: Date.now(), open, high, low, close });
//! chart.on('crosshairMove', e => console.log(e.point));
//! ```
//!
//! `FerroPlot` draws non-time charts (numeric, category and date
//! axes) on the same canvas host:
//!
//! ```js
//! const plot = await FerroPlot.create(canvas);
//! plot.addMark('line').setData({ x: strikes, y: ivs });
//! ```
//!
//! - Rendering is driven by `requestAnimationFrame` and happens only when the
//!   chart needs a redraw; a `ResizeObserver` and a
//!   `devicePixelRatio` media query keep the backing store sharp.
//! - Pointer, wheel, keyboard, double-click and trackpad pinch input (Ctrl+wheel
//!   in Chrome/Firefox, gesture events in Safari) are handled on the canvas.
//! - No `Canvas2D` fallback: `create` rejects when neither WebGPU nor WebGL2
//!   is available.
//!
//! The binding itself only compiles for `wasm32`; the browser-independent
//! conversions are tested natively.

#[cfg(target_arch = "wasm32")]
mod api;
#[cfg(all(target_arch = "wasm32", feature = "text-browser"))]
mod browser_text;
mod convert;
#[cfg(target_arch = "wasm32")]
mod host;
#[cfg(target_arch = "wasm32")]
mod icons;
#[cfg(target_arch = "wasm32")]
mod intro;
#[cfg(target_arch = "wasm32")]
mod js;
#[cfg(target_arch = "wasm32")]
mod logo;
#[cfg(target_arch = "wasm32")]
mod of_api;
mod orderflow;
// Shared with the Python binding and the C ABI; lives in ferrochart-ffi, the
// one of the three published on crates.io.
#[path = "../../ferrochart-ffi/src/params.rs"]
mod params;
#[cfg(target_arch = "wasm32")]
mod plot_api;
mod plot_params;
#[cfg(target_arch = "wasm32")]
mod plugins;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use api::{Drawings, Indicator, PriceLine, SyncGroup, indicator_names};
#[cfg(target_arch = "wasm32")]
pub use plot_api::{Axis, FerroPlot, Label, Mark, Rule};
#[cfg(target_arch = "wasm32")]
pub use web::{FerroChart, Series, random_walk};
