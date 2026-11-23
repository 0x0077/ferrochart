# Plugins

FerroChart has five ways to extend a chart. All of them write drawing commands
into the frame, so they work with every renderer (WebGPU, WebGL2, Vulkan/Metal/DX12
and the CPU renderer) and show up in screenshots and golden images like built-in
series.

| Extension | Use it for | Rust | JavaScript |
|---|---|---|---|
| Layer plugin | Extra drawing on a pane or the whole chart: watermarks, trade marks, session shading | `LayerPlugin` | `chart.addLayer` |
| Custom series | A series that owns its data and draws it: profiles, heatmaps, range boxes | `CustomSeries` | — |
| Native GPU series | A GPU implementation of a custom series kind (compute shaders, large data) | `GpuSeries` | — |
| Series transform | A new OHLC-like series computed from bars: Renko-style charts | `SeriesTransform` | — |
| Drawing tool | A user-drawn object with anchors, handles and hit testing | `DrawingTool` | `chart.drawings.register` |

The runnable Rust example is `examples/plugins` (`cargo run -p example-plugins
--release`); it has a watermark, trade marks and a custom series updated by a
simulated feed. The order flow crate (`ferrochart-orderflow`, see
[Order flow](orderflow.md)) is the largest plugin and uses every Rust extension
point.

## Rules every plugin follows

- **Draw every frame without allocating.** `build` is called for every frame,
  including while the user pans and zooms. Write into the `FrameBuilder` you are
  given (its pools keep their capacity) and keep your own scratch buffers in the
  plugin, cleared and reused. Only draw what is visible: you are given the
  visible range.
- **No panics.** A plugin runs inside the chart's frame; return early instead.
- **Coordinates are physical pixels** (`Px`, `PointPx`, `RectPx`) in Rust and CSS
  pixels in JavaScript. Convert sizes from CSS with `ctx.px(Css(…))` in Rust.
- **Colors come from the style or your options.** Use `ctx.style()` (series
  colors resolved against the theme) or colors you take as options; then the
  plugin follows light and dark themes.
- **Tell the chart about changes.** A layer whose content changes calls
  `invalidate_layer`; a custom series changes its data only through
  `update_custom`.

## Layer plugins

A layer draws on one pane (its plot area) or on the whole chart, at one of four
depths:

| `LayerPosition` | Drawn |
|---|---|
| `Background` | above the grid, behind everything else |
| `BelowSeries` | below the series |
| `AboveSeries` | above series and markers, below drawings and labels |
| `Overlay` | on the overlay layer, below the crosshair |

```rust
use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::frame::TextAlign;
use ferrochart_core::geom::{Css, PointPx, Rgba};
use ferrochart_core::plugin::{LayerCtx, LayerPlugin, LayerPosition};

struct Watermark(&'static str);

impl LayerPlugin for Watermark {
    fn position(&self) -> LayerPosition {
        LayerPosition::Background
    }

    fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
        let r = ctx.rect();
        let center = PointPx { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 };
        let label = Label {
            font: ctx.font(Css(48.0)),
            color: Rgba::hex(0x80_8080).with_alpha(0x40),
            align: TextAlign::Center,
            bg: None,
        };
        out.text(center, self.0, label);
    }
}

// None: the whole chart; Some(pane): that pane's plot.
let id = chart.add_layer(None, Box::new(Watermark("BTCUSD")))?;
```

On a pane, `ctx.coords()` gives the pane's time and price conversions (`x_of`,
`y_of`, `time_at`, `price_at`, `px`, `plot`, price formatting). Find the
visible part of your data with `time_at(plot.x)` and `time_at(plot.right())` and
a binary search, as `TradeMarks` in the example does.

When the layer's own data changes, call `chart.invalidate_layer(id)` so the next
frame redraws it. `chart.remove_layer(id)` removes it.

### In JavaScript

```js
const id = chart.addLayer({
  pane: 0,                  // omit for the whole chart
  position: 'aboveSeries',  // 'background' | 'belowSeries' | 'aboveSeries' | 'overlay'
  build(ctx) {
    const x = ctx.timeToX(entryTime);
    const y = ctx.priceToY(entryPrice);
    if (x === null || y === null) return [];
    return [
      { type: 'line', x1: ctx.rect.x, y1: y, x2: x, y2: y, color: '#2962FF', dash: 'dashed' },
      { type: 'text', x: x + 4, y, text: ctx.formatPrice(entryPrice), color: '#2962FF', align: 'left' },
    ];
  },
});
chart.invalidateLayer(id); // after the plugin's own data changed
chart.removeLayer(id);
```

`build(ctx)` returns an array of commands in CSS pixels from the canvas'
top-left corner: `rect`, `line`, `polyline`, `polygon`, `text` and `icon` (the
`PluginCommand` type lists their fields). An `icon` takes the same extras as a
marker: `image` (a name registered with `chart.registerIcon`), `label`,
`border` and `borderWidth`, so a layer can draw avatars anywhere, not only at
bars (see [Markers](markers.md)). The `ctx` functions (`timeToX`,
`priceToY`, `xToTime`, `yToPrice`, `textWidth`, `formatPrice`) work only during
the call. A plugin that throws or returns an invalid command draws nothing for
that frame (the commands before the bad one are kept); its first error is logged
with `console.warn`.

JavaScript layers are called once per frame across the wasm boundary, so keep
the command count to what is visible (hundreds, not hundreds of thousands).
For large data, write a custom series in Rust.

## Custom series

A custom series owns its data. The chart asks it for its times and price
extremes, places it on the time axis, scales the price axis to it and lets it
draw the visible points.

```rust
use ferrochart_core::chart::SeriesOptions;
use ferrochart_core::data::{DataChange, Timestamp};
use ferrochart_core::drawing::FrameBuilder;
use ferrochart_core::geom::RectPx;
use ferrochart_core::plugin::{AxisMode, CustomSeries, MinMaxIndex, SeriesCtx};

/// A vertical bar from `lo` to `hi` per point.
#[derive(Default)]
struct Ranges {
    time: Vec<Timestamp>,
    range: Vec<(f64, f64)>,
    index: MinMaxIndex,
}

impl Ranges {
    fn push(&mut self, t: Timestamp, lo: f64, hi: f64) -> DataChange {
        self.time.push(t);
        self.range.push((lo, hi));
        let _ = self.index.push(lo, hi);
        DataChange::Append { count: 1 }
    }
}

impl CustomSeries for Ranges {
    fn kind(&self) -> &'static str {
        "example.ranges"
    }
    fn axis_mode(&self) -> AxisMode {
        AxisMode::Slots
    }
    fn times(&self) -> &[Timestamp] {
        &self.time
    }
    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        self.index.query(from, to)
    }
    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
        let w = (ctx.bar_spacing().0 * 0.6).max(1.0);
        for i in from..to {
            let (lo, hi) = self.range[i];
            let (Some(x), Some(y0), Some(y1)) =
                (ctx.x_of_point(i), ctx.coords().y_of(hi), ctx.coords().y_of(lo))
            else {
                continue;
            };
            out.rect(RectPx { x: x - w / 2.0, y: y0, w, h: (y1 - y0).max(1.0) }, ctx.style().up);
        }
    }
}

let id = chart.add_custom_series(chart.main_pane(), Box::new(Ranges::default()), SeriesOptions::default())?;
chart.update_custom(id, |r: &mut Ranges| r.push(60_000, 1.0, 2.0))?;
let ranges: Option<&Ranges> = chart.custom_series::<Ranges>(id);
```

### The contract

| Method | Called | Must |
|---|---|---|
| `kind` | when added | return a unique name, e.g. `"myorg.profile"`; native GPU implementations match on it |
| `axis_mode` | when added | return `Slots` or `Overlay` (below) |
| `times` | after each change | return strictly increasing times (UTC ms); point `i` is `times()[i]` |
| `extremes(from, to)` | every frame | return the price range of points `from..to` in O(log n): use `MinMaxIndex` |
| `build(ctx, from, to, out)` | every frame | draw points `from..to` (the visible ones and one more on each side) without allocating |
| `last_value` | every frame | the value of the last-value label (`None`: no label) |
| `crosshair_value(i)` | when the crosshair moves | the value shown for point `i` |
| `native_params` | every frame, native path only | write up to 8 parameters for the GPU implementation |

**Axis modes.** `Slots`: each point is a slot of the time axis, like the bars
of a built-in series (a footprint column per candle). `Overlay`: points are
placed between the existing slots by their times and add no slots (one-second
heatmap columns on a one-minute chart).

**Changing data.** Change the plugin only through
`chart.update_custom(id, |s: &mut MySeries| { …; change })`, returning what
changed:

| `DataChange` | Meaning |
|---|---|
| `UpdateLast` | the last point changed, its time did not (does not allocate: the real-time path) |
| `Append { count }` | `count` points were added at the end (`count: 0`: nothing changed) |
| `Prepend { count }` | `count` points were added at the beginning (history) |
| `Replace` | anything else |

A change that does not match the data (wrong count, the last time moved) is
applied as `Replace`, so a wrong answer costs time but never corrupts the chart.
Times that stop being strictly increasing return `TimeNotIncreasing`, and the
series shows nothing until a valid change.

**Extremes.** `MinMaxIndex` answers range min/max queries in O(log n), and
`push` / `set_last` keep it current. Use one per series and update it in the
same closure that changes the data.

**Drawing.** `ctx.x_of_point(i)` is the center of point `i` (with
`SeriesOptions::shift`). `ctx.coords()` converts prices and times, gives fonts
and formats prices. `ctx.bar_spacing()` is the distance between slots, and
`ctx.style()` holds the series colors. The `FrameBuilder` writes rectangles,
lines, polylines, polygons, text and icons; rectangles snapped to whole pixels
look sharp at any device pixel ratio.

A custom series cannot share a chart with a transform series (both decide how
the time axis is built).

## Native GPU series

The portable `build` path works everywhere. When a series has too much data
for it, e.g. a heatmap with a million cells, a native implementation can draw
it on the GPU instead. The frame then holds one `DrawCmd::Custom` for the
series instead of its primitives.

```rust
use ferrochart_render_wgpu::{GpuSeries, GpuSeriesCtx};
use ferrochart_core::chart::ChartDataView;
use ferrochart_core::frame::CustomDraw;

struct MySeriesGpu { /* pipelines, per-series buffers */ }

impl GpuSeries for MySeriesGpu {
    fn kind(&self) -> &'static str {
        "myorg.profile" // the CustomSeries::kind it draws
    }
    fn prepare(&mut self, ctx: &GpuSeriesCtx<'_>, draw: &CustomDraw, data: &ChartDataView<'_>) {
        // Upload what changed since the last frame:
        //   data.changes_since(draw.series, synced_generation)
        // Read the plugin: data.custom(draw.series), downcast through &dyn Any.
        // Compute work may be submitted to ctx.queue here.
    }
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, draw: &CustomDraw) {
        // Set the pipeline and bind groups and draw; the scissor is already
        // the pane's plot. Do not change the scissor or viewport.
    }
}

renderer.register_series(Box::new(MySeriesGpu { /* … */ }));
// Before each frame (the winit and web hosts do this for you):
chart.set_native_kinds(renderer.native_kinds());
```

With `ferrochart-winit`, register in `ChartApp::on_renderer`, which is called
whenever the GPU renderer is created.

Rules for native implementations:

- **Same output as the portable path.** Compute layout, thresholds and color
  ranges in the plugin (core side) and pass them through `native_params` and
  `data.custom`. The shader only applies them. Test both paths against the
  same golden image.
- **Check the device.** WebGL2 has no compute shaders or storage buffers.
  Register only when the device supports what you need. Unregistered kinds use
  the portable path automatically.
- **Upload incrementally.** `CustomDraw::generation` and
  `data.changes_since` tell you what changed. Real-time updates usually touch
  only the last point.

`ferrochart_orderflow::register_gpu` is a complete implementation: a compute
shader aggregates heatmap cells per pixel and one quad colors them.

## Series transforms

A transform turns the main series' bars into a new OHLC-like series, as the
built-in Renko, Range Bars, Kagi and Point & Figure do. It is called once per
input bar; `peek` shows what the still-forming last bar would produce.

| Method | Must |
|---|---|
| `name` | return a name, e.g. `"renko"` |
| `output_kind` | return `Candlestick`, `OhlcBar`, `Kagi` or `PointFigure` |
| `push(bar, out)` | commit `bar` and write the elements it completes into `out` |
| `peek(bar, out)` | without changing state, write what `bar` would complete plus the elements still forming |
| `reset` | forget all input |
| `box_size` | Point & Figure only: the price height of one box |

```rust
let id = chart.add_transform_series(pane, Box::new(MyTransform::new()), SeriesOptions::default())?;
chart.set_data(id, SeriesData::Ohlc(bars))?;   // input bars
chart.update_last(id, tick)?;                   // live input
```

A transform series' x axis is its own output, not time, so it cannot share a
chart with other time-based series.

## Drawing tools

A drawing tool is an object the user places with the mouse: it has anchors
(time, price), handles for editing, hit testing, a style and a saved form for
undo/redo and state files.

### In Rust

Implement `DrawingTool` (`kind`, `anchors`, `anchors_mut`, `anchor_count`,
`build`, `hit_test`, `handles`, `style`, `save`; optionally `move_handle`) and
register a factory that rebuilds it from its saved form:

```rust
fn my_tool(data: &DrawingData) -> ferrochart_core::Result<Box<dyn DrawingTool>> {
    Ok(Box::new(MyTool::from_data(data)?))
}

chart.drawings().register("myorg.zone", my_tool);
```

`AnchorCount` decides how it is created: a fixed number of clicks, an open
count finished with a double click or Enter, or freehand. Built-in tools are
listed in `ferrochart_core::drawing::BUILTIN_KINDS` and are good references.

### In JavaScript

```js
chart.drawings.register('myorg.zone', {
  anchorCount: 2, // or { open: 3 } or 'freehand'
  build(ctx) {
    const [a, b] = ctx.anchors;
    if (a.x === null || b.x === null) return [];
    return [{
      type: 'rect',
      x: Math.min(a.x, b.x), y: Math.min(a.y, b.y),
      width: Math.abs(b.x - a.x), height: Math.abs(b.y - a.y),
      color: ctx.style.fill ?? 'rgba(41, 98, 255, 0.2)',
    }];
  },
  // Optional; default: hit inside the drawn bounds.
  hitTest(ctx, point) {
    return false; // true: the drawing; a number: that anchor's handle
  },
});
chart.drawings.startTool('myorg.zone');
```

A JavaScript kind is available on the chart whose `drawings.register` was
called (for `add`, `startTool` and `loadState`); register it on each chart
that needs it. Registering a kind again replaces its implementation on every
chart of the page. Built-in kinds cannot be replaced.

## Testing a plugin

- **Golden images.** Render a fixed scene with `ferrochart-render-cpu`
  (deterministic, no GPU) and compare it to a stored PNG. With a native
  implementation, render it with `ferrochart-render-wgpu` too and compare both
  paths to the same image.
- **No allocations.** Warm up a few frames, then count allocations while
  panning, zooming and moving the crosshair (`build_frame`) and while updating
  the last point. The count should be zero. `ferrochart-orderflow/tests/zero_alloc.rs`
  shows the counting allocator.
- **Headless frames.** `ferrochart_core::testing::MonoMeasurer` (feature
  `testing`) lets you call `chart.build_frame` in unit tests without fonts or a
  GPU.
