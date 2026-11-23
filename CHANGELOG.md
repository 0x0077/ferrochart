# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-11-23

Markers that carry your data, clustering across bars and bar quality flags.

### Added

- **Marker ids.** Markers take an `id` (`u64`; in JavaScript an integer up
  to 2^53−1) that click and hover events report. In JavaScript the events
  also hand back the very object passed to `setMarkers` as `marker`, with
  any fields of your own.
- **Markers at a price.** A third position, `atPrice` (Rust
  `MarkerPosition::AtPrice`, Python `"at_price"`, C
  `FERROCHART_MARKER_AT_PRICE`), centers a marker on its `price`, for fills
  and other events at an exact price. These markers are not stacked or
  clustered.
- **Clustering across bars.** A per-series `markerCluster` (CSS px; Python
  `marker_cluster`): when bars get closer than that, each side of a bucket
  of bars shows its first marker and a "+k" badge. Buckets are aligned to
  the logical index, so panning does not regroup. Click and hover events
  for a badge report the bars it stands for (`bars: { from, to }`).
- **Bar quality flags.** `setBarFlags([{ time, filled?, clipped? }])` (Rust
  `Chart::set_bar_flags`, Python `set_bar_flags`, C
  `ferrochart_set_bar_flags`). `filled`: a bar the data source filled in
  without trades, drawn hollow in `filledBarColor`. `clipped`: a bar whose
  outlier wick you clipped (keep the raw values for your tooltip); a tick
  marks each end of the wick. The flags survive level-of-detail merging.
- The lightweight-charts compatibility layer maps v5 `atPriceTop`,
  `atPriceMiddle` and `atPriceBottom` to `atPrice` and passes numeric ids.
- Examples: a meme token's reality layer (filled and clipped bars,
  clustered wallet avatars, wallet cards from the clicked marker) and an LP
  risk canvas (range, hedge liquidation, events at their price).

### Changed

- Breaking for Rust struct literals and matches (hence 0.5.0): new fields
  in `Marker` (`id`, `price`), `SeriesOptions` (`marker_cluster`,
  `filled_bar_color`), `MarkerHit` (`id`, `bars`), `ClickTarget::Marker`
  (`id`) and `ClickTarget::MarkerOverflow` (`id`, `bars`); a new
  `MarkerPosition::AtPrice`.
- C: `FerroChartMarker`, `FerroChartSeriesOptions` and `FerroChartEvent`
  have new fields, so programs built against 0.4 must be recompiled with
  the new header. The JavaScript and Python APIs only add.

## [0.4.0] - 2026-11-23

Image markers, badges and text in any script.

### Added

- **Image markers.** Markers take a `size` (4–64 CSS px), an `icon` (an image
  registered with the chart, clipped to a circle or square), a `label` (up to
  8 characters inside the shape, such as `DB`) and a `border` ring.
  `Chart::add_icon`/`remove_icon`; JavaScript `registerIcon(name, url |
  image | bitmap | ImageData | canvas)` and `unregisterIcon`; Python
  `Chart.add_icon(array)`; C `ferrochart_add_icon`/`ferrochart_remove_icon`.
- **Folding.** A per-series `markerStackLimit`: crowded bars show the first
  markers and a "+k" badge for the rest. Clicking the badge reports a
  `markerOverflow` target.
- **Marker hover.** A `markerHover` event (Rust `ChartEvent::MarkerHover`,
  Python `marker_hover`, C `FERROCHART_EVENT_MARKER_HOVER`) with the marker or
  badge under the pointer, its center and size, for tooltips.
- **Text in any script.** Characters the bundled font lacks come from fallback
  fonts (`WindowOptions::fonts` in winit, `Chart(fonts=[...])` in Python,
  `ferrochart_add_font` in C, `TextEngine::add_fallback_font` in Rust), or on
  the web from the browser's own fonts, including color emoji (feature
  `text-browser`, on by default; no fonts are added to the package). A
  character no font has is drawn as `□`.
- **Grapheme clusters.** Text is laid out by grapheme cluster: on the web, an
  emoji with a skin tone, a ZWJ sequence (families, professions), a flag or a
  letter with a combining accent is drawn as one glyph. Rust:
  `TextEngine::glyphs`, and `GlyphSource` is called with a cluster (`&str`).
- **C ABI.** `ferrochart_set_markers` with `FerroChartMarker`;
  `FerroChartSeriesOptions.marker_stack_limit`.
- Layer plugins' `icon` command takes the same `image`, `label`, `border` and
  `borderWidth`. The lightweight-charts compatibility layer honors marker
  `size`.

### Changed

- Breaking for Rust struct literals (hence 0.4.0): new fields in `Marker`,
  `SeriesOptions`, `IconInst`, `GlyphBitmap` (`rgba`, for color glyphs) and
  `WindowOptions` (`fonts`). The JavaScript, Python and C APIs only add.

### Fixed

- A canvas hidden with `display: none` (CSS size 0) no longer logs "invalid
  render target size" on every change; the chart redraws when it is shown.

### Known limitations

- No shaping or right-to-left layout: Arabic and Hebrew do not come out
  right. Natively (without the browser), emoji sequences are drawn character
  by character.

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
