# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2026-11-24

Charts in app style: hidden axes and grid, padding, interaction switches,
a crosshair with a dot, themes as objects that change at run time, pools of
many small charts sharing one GPU device, and image export.

### Added

- **Appearance.** The price axis, the time axis and the grid lines can be
  hidden, and the plot area can have padding: `ChartOptions.appearance`,
  `Chart::set_appearance`, `Appearance::sparkline()` (no axes, no grid,
  2 CSS px of padding).
- **Interaction switches.** `ChartOptions.interaction` /
  `Chart::set_interaction` turn panning, zooming and the crosshair off one
  by one (`InteractionOptions::none()` for all three). Input a switched-off
  gesture would take is reported as not consumed, so the page can scroll;
  clicks are still reported.
- **Crosshair style.** `ChartOptions.crosshair_style` /
  `Chart::set_crosshair_style`: the vertical line, the horizontal line and
  the axis labels each on or off, and an optional dot on the first series'
  value (`DotStyle`, colored like the series at that value).
- **Changing the theme.** `Chart::set_theme` replaces the theme of a chart
  that already exists; `Chart::theme` reads it.
- **Shared GPU resources.** `ferrochart_render_wgpu::SharedResources`:
  renderers on one device share pipelines and glyph atlases
  (`WgpuRenderer::offscreen_shared`, `with_surface_shared`,
  `shared_resources`), so many small charts do not each compile the shaders
  and hold their own 8 MB of glyph textures.
- **JavaScript look options.** `FerroChart.create` and the new
  `chart.applyOptions` take `priceAxis`, `timeAxis`, `grid`, `padding`,
  `interaction`, `crosshairStyle` and `preset: 'sparkline'`; `theme` also
  takes an object of colors on a `base` theme and can change after
  creation. `chart.options()` reads them back.

- **lightweight-charts compatibility: look options.** `layout` colors and
  font size, grid visibility and color, `rightPriceScale.visible`,
  `timeScale.visible`, `handleScroll`, `handleScale` and the crosshair
  lines' `visible`/`labelVisible` now map to FerroChart, in `createChart` and
  in `applyOptions` (`layout` no longer has to be set at creation).

- **Python look options.** `Chart(theme=..., appearance=..., interaction=...,
  crosshair_style=..., preset="sparkline")`, `Chart.apply_options(**options)`
  and `Chart.options()`: the JavaScript look options with snake_case keys;
  `theme` also takes a dict of colors. They draw in `to_png` and windows and
  reach the Jupyter widget.

- **C look structs.** `FerroChartTheme`, `FerroChartAppearance`,
  `FerroChartInteraction` and `FerroChartCrosshairStyle`, each with a setter
  (`ferrochart_set_theme`, …) and a getter (`ferrochart_theme`, …);
  `ferrochart_theme_default(dark, out)` fills a built-in theme to edit.

- **Chart pools.** `FerroChart.createPool({ renderer })` returns a
  `ChartPool` whose `create(canvas, options)` makes charts sharing one GPU
  device: dozens of sparklines on a page no longer run into the browser's
  WebGL context limit. Charts out of the viewport are not drawn.
  Rust: `ferrochart_render_wgpu::SharedSurface` and
  `WgpuRenderer::for_shared_surface`; `GpuContext` is `Clone`.
- **Snapshots.** `chart.toImageBitmap()` and `chart.toBlob(type?, quality?)`
  (PNG, WebP or JPEG), also on `FerroPlot`.

### Changed

- Python: `Chart(attribution_logo=...)` defaults to `None`: the Jupyter
  logo follows the small-canvas rule of the JavaScript binding unless set.
- The attribution logo is no longer shown by default on canvases smaller
  than 320×160 CSS px (it follows the size); `attributionLogo: true` or
  `setAttributionLogo(true)` still shows it anywhere.
- With panning or zooming switched off, the canvas' `touch-action` lets the
  page take those gestures.
- Breaking for Rust struct literals: new fields `appearance`,
  `interaction` and `crosshair_style` in `ChartOptions`.
  `testing::quote_card_scene` is new.

## [0.5.0] - 2026-11-24

Markers that carry your data, clustering across bars, bar quality flags
and touch screens.

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
- **Touch screens.** On phones, tablets and touch laptops: two fingers zoom
  and pan, a long press (500 ms) shows the crosshair, which stays after the
  finger lifts until the next tap, and drawings, handles and markers are
  easier to hit with a finger. A double tap is a double click. Plots zoom
  and pan with two fingers and show the hovered element under the finger.
- **`touchScroll`** creation option in JavaScript (`'chart'`, the default,
  or `'page'`) and `setTouchScroll`: with `'page'` a vertical swipe scrolls
  the page, so charts inside articles no longer trap scrolling.
- Desktop windows (winit, Python) now handle touch input.

### Changed

- Breaking for Rust struct literals and matches (hence 0.5.0): new fields
  in `Marker` (`id`, `price`), `SeriesOptions` (`marker_cluster`,
  `filled_bar_color`), `MarkerHit` (`id`, `bars`), `ClickTarget::Marker`
  (`id`) and `ClickTarget::MarkerOverflow` (`id`, `bars`); a new
  `MarkerPosition::AtPrice`.
- C: `FerroChartMarker`, `FerroChartSeriesOptions` and `FerroChartEvent`
  have new fields, so programs built against 0.4 must be recompiled with
  the new header. The JavaScript and Python APIs only add.
- Breaking for Rust code that builds input events: `PointerDown`,
  `PointerMove` and `PointerUp` have a `kind: PointerKind` field (`Mouse`,
  `Touch`, `Pen`), and there is a new `InputEvent::PointerCancel`. In C,
  `FerroChartInput` gains `pointer_kind` at the end (programs built with the
  shorter struct keep working as mouse input) and
  `FERROCHART_INPUT_POINTER_CANCEL`.
- In the browser a cancelled pointer (the page took the gesture over) no
  longer counts as a click.

### Fixed

- **Real-time updates of sparse line series** ([#1](https://github.com/0x0077/ferrochart/issues/1)).
  On a line, area or baseline series with points on only some bars,
  `update_last` now moves the interpolated points before the last one
  with it, as `set_data` of the same points would. It returns
  `DataChange::Replace` in that case, since more than the last value
  changed. Histograms keep their gaps.
- **Gann fan labels** that would overlap near the origin are not drawn.
- **Plot**: the left and right axes are wide enough for their rule labels.
- **Jupyter**: the operations recorded for a live chart stay bounded, so
  views opened later load quickly.

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
