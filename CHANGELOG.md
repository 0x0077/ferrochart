# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-11-23

First public release.

- Crates: `ferrochart-core`, `ferrochart-text`, `ferrochart-render-cpu`,
  `ferrochart-render-wgpu`, `ferrochart-indicators`, `ferrochart-orderflow`,
  `ferrochart-plot`, `ferrochart-winit`.
- npm: `@ferrochart/core` (WebAssembly, TypeScript types, and the
  `@ferrochart/core/lightweight-charts` compatibility layer).
- PyPI: `ferrochart` (wheels).
- C ABI: `ferrochart-ffi`, built from source, with a C header and a C# example.

### Added

- **Core.** Double-ended column storage with a global time index, OHLCV
  validation (reject or repair), an f32 level-of-detail pyramid, and a
  backend-independent `Frame`. No heap allocations while panning, zooming,
  moving the crosshair or updating the last bar.
- **Rendering.** A wgpu GPU backend (WebGPU, WebGL2, Metal, Vulkan, DirectX 12)
  with chunked series buffers and a layer cache, and a tiny-skia CPU backend.
  Both draw the same golden images.
- **Series.** Candlestick, OHLC, Heikin-Ashi, line, area, baseline, histogram,
  volume and points; Renko, range bars, Kagi and point & figure transforms;
  bands between two series.
- **Layout and scales.** Multiple panes with draggable separators; linear, log,
  percentage and indexed price scales; markers, price lines and
  non-overlapping axis labels; light and dark themes.
- **Interaction.** Pointer, wheel, pinch and keyboard input: panning, kinetic
  scrolling, zooming, axis scaling, crosshair (normal, magnet, hidden), click
  events, and a request for more history at the left edge.
- **Indicators** (`ferrochart-indicators`). 30 indicators, updated
  incrementally on every tick, tested against reference data and for
  incremental == full results; optional parallel recomputation (`rayon`).
- **Drawing tools.** 29 tools with styles, undo/redo and serialization, and
  a plugin interface for custom tools.
- **Order flow** (`ferrochart-orderflow`). Footprint (bid/ask, delta, imbalances,
  POC), order-book and trade heatmaps (with a GPU compute path) and TPO
  profiles (value area, initial balance).
- **Plots** (`ferrochart-plot`, `FerroPlot`). Charts without a time axis:
  scatter, bars, heatmaps, payoff diagrams, volatility smiles.
- **Sync and state.** Linked crosshair, visible range and drawings across
  charts (`SyncGroup`); `save_state` / `load_state` (feature `serde`).
- **Plugins.** Custom series (including native GPU implementations), layers,
  indicators, transforms and drawing tools.
- **Hosts and bindings.** Desktop windows (`ferrochart-winit`), the browser
  (`@ferrochart/core`), Python with Jupyter display and PNG output
  (`ferrochart`), and a C ABI; an egui integration example.
- **Docs and examples.** A user guide, API references, 308 live examples on
  ferrochart.com, a desktop demo with 10M bars and a live feed, and a
  same-scenario browser comparison with lightweight-charts and Vela.
