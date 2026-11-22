# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0]

First release (crates `ferrochart-core`,
`-indicators`, `-text`, `-render-cpu`, `-render-wgpu`, `-winit`,
`-orderflow`; npm `@ferrochart/core`; PyPI `ferrochart`, wheels only).

### Added

- Cross-platform bindings and order flow:
  - Plugins: layer plugins (`LayerPlugin`, JS `addLayer`), custom series that
    own their data (`CustomSeries`, `MinMaxIndex`), native GPU
    implementations of custom series (`GpuSeries`,
    `WgpuRenderer::register_series`) and JS drawing tools
    (`drawings.register`, including open and freehand tools); a Rust plugin
    example and JS plugins in the web demo.
  - Series transforms: Renko, Range Bars, Kagi and Point & Figure
    (`SeriesTransform`, JS `addTransformSeries`).
  - `Points` series (SAR drawn as dots), `Band` fills between two series and
    indicator bands; `Placement::Volume`.
  - Second batch of drawing tools; drawing sync across charts; `Click`
    events; `TimeScale::set_visible_range`.
  - Parallel full indicator recomputation (feature `rayon`).
  - `ferrochart-orderflow`: footprint (bid/ask, delta and volume, diagonal
    imbalances, POC), order book and trade heatmaps (with a compute-shader
    path on capable GPUs) and TPO market profiles (letters or histograms,
    POC, value area, initial balance). In JS: `addFootprint`, `addHeatmap`,
    `addTpo` and the `maxBarSpacing` create option.
  - `ferrochart-py`: a Python binding (`pip install ferrochart`) with windows,
    PNG export and Jupyter display, indicators, drawings and saved state.
  - `ferrochart-ffi`: a C ABI with a header, and a C# example; an egui
    integration example; `WgpuRenderer` exposes its offscreen target texture.
  - `GpuContext::from_adapter` requests compute and storage-buffer limits
    when the adapter supports them.
  - Benchmarks: order flow frame time (1M trades) and multi-chart sync.
  - User guide chapters on plugins and order flow.

- Advanced features:
  - Multiple panes with draggable separators; line, area, baseline, histogram,
    OHLC bar and Heikin-Ashi series.
  - Percentage and indexed-to-100 price scales, price lines, markers and
    non-overlapping axis labels.
  - `ferrochart-indicators`: an incremental indicator framework and 30
    indicators, checked against reference data (`docs/book/src/indicators.md`) and for
    incremental == full computation.
  - Drawing tools: a plugin framework, 16 built-in kinds (15 tools), undo/redo
    and serialization.
  - `SyncGroup`: crosshair and visible-range sync across charts.
  - `Chart::save_state` / `load_state` (optional `serde` feature).
  - wasm/npm: the full API (series kinds, panes, indicators, drawings, sync,
    state) and a lightweight-charts compatibility layer,
    `@ferrochart/core/lightweight-charts`.

- `ferrochart-wasm` / npm `@ferrochart/core`: WebGPU with WebGL2
  fallback, `requestAnimationFrame` rendering on demand, DOM input including
  trackpad pinch, resize and device-pixel-ratio tracking, events, TypeScript
  types, and `randomWalk` test data.
- `examples/web` demo and `examples/compare`, a same-scenario benchmark against
  lightweight-charts and Vela; browser smoke tests in `tests/web`.
- User guide (`docs/book`, mdBook).
- `GpuContext::request` / `from_adapter` (async, for the web) and
  `WgpuRenderer::take_gpu_error`.

- Workspace skeleton with all crates and shared lints.
- `ferrochart-core::testing::random_walk`: a deterministic, platform-independent
  OHLCV test-data generator (feature `testing`).
- Core data model: double-ended column storage, a global time
  index with stable logical indices, change log with generations, OHLCV store
  with validation (reject or repair), an f32 LOD pyramid with directed rounding,
  and O(log n) range-extremes queries.
- Time scale with weight-based calendar ticks and price scale (linear and
  logarithmic) with nice-number ticks.
- `Chart`: single-pane layout, candlestick and volume series, and generation
  of a backend-agnostic `Frame` with zero heap allocations while panning,
  zooming and on real-time updates.
- `ferrochart-text`: ab_glyph text engine with a bundled 9 KB JetBrains Mono
  subset (OFL 1.1).
- `ferrochart-render-cpu`: tiny-skia CPU renderer and six golden images.
- Criterion benchmarks for `set_data`, `build_frame` and `update_last`, and a
  memory benchmark for P2.
- Interaction: pointer, wheel, pinch and keyboard input with
  panning, kinetic scrolling, zooming, axis scaling and a crosshair (normal,
  magnet, hidden); `CrosshairMove`, `VisibleRangeChanged` and
  `NeedMoreHistory` events.
- `ferrochart-render-wgpu`: GPU backend with chunked, rebased series buffers,
  lazy upload, a layer cache for crosshair-only frames and WebGL2-compatible
  pipelines; passes the CPU backend's golden images.
- `ferrochart-winit`: desktop host (window, surface, input conversion,
  on-demand redraw) and `examples/desktop`, a 10M-bar demo with a simulated
  100 ticks/s feed.
- `frame_time` benchmark for P4 (end-to-end frame time).

### Fixed

- `fit_content` and synced visible ranges missed bars when the price axis
  width changed in the same frame.
- The WebGL2 backend rejected the primitives pipeline (uniform struct smaller
  than 16 bytes).

### Changed

- GPU validation errors no longer panic: the next `render` returns an error.
- `GpuContext::headless`, `with_instance` and `WgpuRenderer::read_pixels` are
  native-only.
- Coordinate transforms are evaluated add-first (`(d + shift) * step`,
  `(v_rel + bias) * a`) so that shader compilers cannot fuse them into FMAs;
  `transform_helpers::chunk_delta` is replaced by `y_bias` and `x_shift`.
