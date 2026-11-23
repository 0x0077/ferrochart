<!-- source: f3b74de278 -->
# Rust API

API 文档由 rustdoc 生成。在本地生成：

```bash
cargo doc --workspace --exclude ferrochart-py --no-deps --open
```

发布在 docs.rs 上：

| Crate | 内容 |
|---|---|
| [`ferrochart-core`](https://docs.rs/ferrochart-core) | `Chart`、数据模型、坐标轴、输入、`Frame`、`Renderer` trait、`transform_helpers` |
| [`ferrochart-render-wgpu`](https://docs.rs/ferrochart-render-wgpu) | `GpuContext`、`WgpuRenderer`（surface 或离屏） |
| [`ferrochart-render-cpu`](https://docs.rs/ferrochart-render-cpu) | `CpuRenderer`（tiny-skia） |
| [`ferrochart-text`](https://docs.rs/ferrochart-text) | `TextEngine`、内置字体 |
| [`ferrochart-winit`](https://docs.rs/ferrochart-winit) | `run`、`ChartApp`、`InputConverter` |
| [`ferrochart-indicators`](https://docs.rs/ferrochart-indicators) | 增量指标 |
| [`ferrochart-orderflow`](https://docs.rs/ferrochart-orderflow) | 足迹图、热力图和 TPO 系列（[订单流](orderflow.md)） |

扩展图表（图层、自定义系列、原生 GPU 系列、变换、画线工具）的方法见[插件](plugins.md)。

## 从哪里开始

- `ferrochart_core::Chart`：用 `ChartOptions` 创建，调用 `resize`，添加系列，
  设置数据，通过 `handle_input` 传入 `InputEvent`，调用 `build_frame`，再用
  `drain_events` 读取事件。
- `ferrochart_core::render::Renderer`：两个渲染器都实现了它；调用方式是
  `render(chart.frame(), &chart.data_view())`。

## API 遵循的规则

- **不会 panic。** 可能失败的方法返回 `Result`；按 id 查找返回 `Result` 或 `Option`。
- **热路径零分配。** 预热之后，平移、缩放、移动十字光标（`build_frame`）以及
  `update_last` 都不会分配内存；测试用计数分配器来断言这一点。
- **带类型的坐标。** `Css` 和 `Px`（以及基于它们构建的点、尺寸和矩形类型）将 CSS
  像素与设备像素区分开；设备像素比传给 `Chart::resize`。
- **核心保持纯粹。** `ferrochart-core` 除标准库外没有其他依赖（唯一允许的是
  `smallvec`）：不依赖 GPU、窗口、字体、时间或异步库，也没有 `unsafe`。
