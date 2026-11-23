<!-- source: 8785086f6e -->
# 插件

FerroChart 提供五种扩展图表的方式。它们都向帧中写入绘制命令，因此适用于所有渲染器（WebGPU、WebGL2、Vulkan/Metal/DX12
以及 CPU 渲染器），并且会像内置系列一样出现在截图和黄金图像中。

| 扩展方式 | 用途 | Rust | JavaScript |
|---|---|---|---|
| 图层插件 | 在窗格或整个图表上额外绘制：水印、交易标记、交易时段底色 | `LayerPlugin` | `chart.addLayer` |
| 自定义系列 | 自己持有数据并负责绘制的系列：分布图、热力图、区间框 | `CustomSeries` | — |
| 原生 GPU 系列 | 某种自定义系列的 GPU 实现（计算着色器、大数据量） | `GpuSeries` | — |
| 系列变换 | 由 K 线计算出的新的类 OHLC 系列：Renko 一类的图表 | `SeriesTransform` | — |
| 画线工具 | 用户绘制的对象，带有锚点、控制柄和命中测试 | `DrawingTool` | `chart.drawings.register` |

可运行的 Rust 示例是 `examples/plugins`（`cargo run -p example-plugins
--release`）；其中包含水印、交易标记，以及一个由模拟行情更新的自定义系列。订单流 crate（`ferrochart-orderflow`，参见
[订单流](orderflow.md)）是最大的插件，用到了所有 Rust 扩展点。

## 所有插件都要遵守的规则

- **每帧绘制且不分配内存。** `build` 在每一帧都会被调用，包括用户平移和缩放的过程中。写入传给你的 `FrameBuilder`（它的内存池会保留容量），并把自己的临时缓冲区放在插件内部，清空后复用。只绘制可见部分：可见范围会传给你。
- **不要 panic。** 插件运行在图表的帧构建过程中；遇到问题应提前返回。
- **坐标**在 Rust 中是物理像素（`Px`、`PointPx`、`RectPx`），在 JavaScript 中是 CSS
  像素。在 Rust 中用 `ctx.px(Css(…))` 把 CSS 尺寸转换过来。
- **颜色来自样式或你的选项。** 使用 `ctx.style()`（已根据主题解析的系列颜色）或作为选项接收的颜色；这样插件就能跟随浅色和深色主题。
- **把变化告诉图表。** 内容发生变化的图层要调用
  `invalidate_layer`；自定义系列只能通过 `update_custom` 修改数据。

## 图层插件

图层绘制在一个窗格（其绘图区）或整个图表上，可位于四个深度之一：

| `LayerPosition` | 绘制位置 |
|---|---|
| `Background` | 网格之上，其他一切之下 |
| `BelowSeries` | 系列之下 |
| `AboveSeries` | 系列和标记之上，画线和标签之下 |
| `Overlay` | 覆盖层上，十字光标之下 |

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

在窗格上，`ctx.coords()` 提供该窗格的时间和价格换算（`x_of`、
`y_of`、`time_at`、`price_at`、`px`、`plot`、价格格式化）。像示例中的 `TradeMarks` 那样，用 `time_at(plot.x)` 和 `time_at(plot.right())` 加二分查找找出数据中的可见部分。

图层自身的数据变化时，调用 `chart.invalidate_layer(id)`，让下一帧重绘它。`chart.remove_layer(id)` 用于移除图层。

### 在 JavaScript 中

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

`build(ctx)` 返回一个命令数组，坐标为相对于 canvas 左上角的 CSS 像素：`rect`、`line`、`polyline`、`polygon`、`text` 和 `icon`（`PluginCommand` 类型列出了它们的字段）。`icon` 接受与标记相同的附加字段：`image`（用 `chart.registerIcon` 注册的名称）、`label`、`border` 和 `borderWidth`，因此图层可以在任意位置绘制头像，而不仅限于 K 线处（见[标记与价格线](markers.md)）。`ctx` 中的函数（`timeToX`、
`priceToY`、`xToTime`、`yToPrice`、`textWidth`、`formatPrice`）只在调用期间有效。插件抛出异常或返回无效命令时，该帧什么也不画（无效命令之前的命令会保留）；第一个错误会用 `console.warn` 输出。

JavaScript 图层每帧跨越 wasm 边界调用一次，所以命令数量应限于可见内容（几百个，而不是几十万个）。
对于大数据量，请用 Rust 编写自定义系列。

## 自定义系列

自定义系列持有自己的数据。图表向它询问时间和价格极值，把它放到时间轴上，据此缩放价格轴，并让它绘制可见的点。

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

### 约定

| 方法 | 调用时机 | 必须 |
|---|---|---|
| `kind` | 添加时 | 返回唯一的名称，例如 `"myorg.profile"`；原生 GPU 实现据此匹配 |
| `axis_mode` | 添加时 | 返回 `Slots` 或 `Overlay`（见下文） |
| `times` | 每次变化后 | 返回严格递增的时间（UTC 毫秒）；点 `i` 对应 `times()[i]` |
| `extremes(from, to)` | 每帧 | 以 O(log n) 返回点 `from..to` 的价格范围：使用 `MinMaxIndex` |
| `build(ctx, from, to, out)` | 每帧 | 在不分配内存的前提下绘制点 `from..to`（可见点，两侧各多一个） |
| `last_value` | 每帧 | 最新值标签的值（`None`：不显示标签） |
| `crosshair_value(i)` | 十字光标移动时 | 为点 `i` 显示的值 |
| `native_params` | 每帧，仅原生路径 | 为 GPU 实现写入最多 8 个参数 |

**坐标轴模式。** `Slots`：每个点占据时间轴的一个位置，就像内置系列的 K 线一样（每根 K 线一列足迹图）。`Overlay`：点按时间放在已有位置之间，不增加新位置（一分钟图上的一秒热力图列）。

**修改数据。** 只能通过
`chart.update_custom(id, |s: &mut MySeries| { …; change })` 修改插件，并返回发生了什么变化：

| `DataChange` | 含义 |
|---|---|
| `UpdateLast` | 最后一个点变了，但时间没变（不分配内存：实时路径） |
| `Append { count }` | 在末尾添加了 `count` 个点（`count: 0`：没有变化） |
| `Prepend { count }` | 在开头添加了 `count` 个点（历史数据） |
| `Replace` | 其他任何情况 |

与数据不符的变化（数量不对、最后的时间变了）会按 `Replace` 处理，所以错误的返回值只会浪费时间，永远不会破坏图表。
时间不再严格递增时会返回 `TimeNotIncreasing`，在出现有效变化之前，该系列不显示任何内容。

**极值。** `MinMaxIndex` 以 O(log n) 回答区间最小/最大值查询，
`push` / `set_last` 让它保持最新。每个系列用一个，并在修改数据的同一个闭包里更新它。

**绘制。** `ctx.x_of_point(i)` 是点 `i` 的中心（已考虑
`SeriesOptions::shift`）。`ctx.coords()` 负责换算价格和时间、提供字体并格式化价格。`ctx.bar_spacing()` 是相邻位置之间的距离，
`ctx.style()` 包含系列颜色。`FrameBuilder` 可以写入矩形、线段、折线、多边形、文本和图标；对齐到整像素的矩形在任何设备像素比下都很清晰。

自定义系列不能与变换系列共用一个图表（两者都要决定时间轴如何构建）。

## 原生 GPU 系列

可移植的 `build` 路径在任何地方都能工作。当一个系列的数据量对它来说太大时，例如有一百万个单元格的热力图，可以改由原生实现在 GPU 上绘制。此时帧中为该系列保存的是一条 `DrawCmd::Custom`，而不是它的图元。

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

使用 `ferrochart-winit` 时，在 `ChartApp::on_renderer` 中注册，每次创建 GPU 渲染器时都会调用它。

原生实现的规则：

- **输出与可移植路径相同。** 在插件中（核心一侧）计算布局、阈值和颜色范围，并通过 `native_params` 和
  `data.custom` 传递。着色器只负责应用它们。用同一张黄金图像测试两条路径。
- **检查设备。** WebGL2 没有计算着色器和存储缓冲区。只在设备支持所需功能时才注册。未注册的类型会自动使用可移植路径。
- **增量上传。** `CustomDraw::generation` 和
  `data.changes_since` 会告诉你哪些发生了变化。实时更新通常只涉及最后一个点。

`ferrochart_orderflow::register_gpu` 是一个完整的实现：一个计算着色器按像素聚合热力图单元格，再由一个四边形为它们着色。

## 系列变换

变换把主系列的 K 线转换成新的类 OHLC 系列，就像内置的 Renko、等幅 K 线、Kagi 和点数图一样。每根输入 K 线调用它一次；`peek` 显示仍在形成中的最后一根 K 线会产生什么。

| 方法 | 必须 |
|---|---|
| `name` | 返回一个名称，例如 `"renko"` |
| `output_kind` | 返回 `Candlestick`、`OhlcBar`、`Kagi` 或 `PointFigure` |
| `push(bar, out)` | 提交 `bar`，并把它完成的元素写入 `out` |
| `peek(bar, out)` | 在不改变状态的前提下，写入 `bar` 会完成的元素以及仍在形成中的元素 |
| `reset` | 丢弃所有输入 |
| `box_size` | 仅点数图：一格对应的价格高度 |

```rust
let id = chart.add_transform_series(pane, Box::new(MyTransform::new()), SeriesOptions::default())?;
chart.set_data(id, SeriesData::Ohlc(bars))?;   // input bars
chart.update_last(id, tick)?;                   // live input
```

变换系列的 x 轴是它自己的输出，而不是时间，因此不能与其他基于时间的系列共用一个图表。

## 画线工具

画线工具是用户用鼠标放置的对象：它有锚点（时间、价格）、用于编辑的控制柄、命中测试、样式，以及用于撤销/重做和状态文件的保存形式。

### 在 Rust 中

实现 `DrawingTool`（`kind`、`anchors`、`anchors_mut`、`anchor_count`、
`build`、`hit_test`、`handles`、`style`、`save`；可选 `move_handle`），并注册一个能从保存形式重建它的工厂函数：

```rust
fn my_tool(data: &DrawingData) -> ferrochart_core::Result<Box<dyn DrawingTool>> {
    Ok(Box::new(MyTool::from_data(data)?))
}

chart.drawings().register("myorg.zone", my_tool);
```

`AnchorCount` 决定它的创建方式：固定次数的点击、以双击或 Enter 结束的不定数量，或者自由绘制。内置工具列在 `ferrochart_core::drawing::BUILTIN_KINDS` 中，是很好的参考。

### 在 JavaScript 中

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

JavaScript 类型只在调用了其 `drawings.register` 的那个图表上可用（用于 `add`、`startTool` 和 `loadState`）；请在每个需要它的图表上分别注册。再次注册同一类型会在页面的所有图表上替换其实现。内置类型不能被替换。

## 测试插件

- **黄金图像。** 用 `ferrochart-render-cpu`（结果确定，不需要 GPU）渲染一个固定场景，并与保存的 PNG 比较。如果有原生实现，也用 `ferrochart-render-wgpu` 渲染，并让两条路径与同一张图像比较。
- **零分配。** 先预热几帧，然后在平移、缩放和移动十字光标（`build_frame`）以及更新最后一个点时统计内存分配次数。次数应为零。`ferrochart-orderflow/tests/zero_alloc.rs`
  展示了计数分配器的用法。
- **无头帧构建。** `ferrochart_core::testing::MonoMeasurer`（feature
  `testing`）让你可以在单元测试中调用 `chart.build_frame`，而不需要字体或 GPU。
