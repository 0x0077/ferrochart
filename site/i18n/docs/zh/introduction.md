<!-- source: f6f2d1d064 -->
# 简介

FerroChart 是一个用 Rust 编写的开源金融图表引擎。同一个核心驱动所有平台：浏览器通过
WebAssembly（WebGPU 或 WebGL2），原生应用通过 wgpu（Metal、Vulkan、DirectX 12），
还有 Python 和 C，以及服务器上的无头渲染。

它的设计目标：

- **大数据量。** 一千万根 K 线只需几十毫秒即可加载，平移起来和两百根一样流畅：每帧的工作量取决于屏幕，而不是数据规模。
- **实时更新。** 在核心中更新最后一根 K 线的开销远低于一微秒，并且只重绘发生变化的部分。
- **处处一致的外观。** 所有后端都基于同一份帧描述绘制，并用同一套黄金图像进行校验。

## 一图看懂架构

```text
 your app ──► Chart (ferrochart-core) ──► Frame ──► renderer ──► pixels
              data, scales, layout,        plain       wgpu (GPU) / tiny-skia (CPU)
              interaction, LOD             draw list
```

- `ferrochart-core` 持有数据和全部图表逻辑：坐标轴、刻度、布局、输入处理。它不依赖
  GPU、窗口、字体或异步库，可以在任何地方运行，包括 `wasm32-unknown-unknown`。
- 每一帧它都会生成一个 `Frame`：一个扁平的绘制命令列表。系列数据不会被复制进去；
  渲染器直接读取数据列，只上传发生变化的部分。
- 渲染器把帧转换成像素。`ferrochart-render-wgpu` 是主渲染器；`ferrochart-render-cpu`
  在 CPU 上绘制同样的帧。
- 绑定层为各个平台封装这一切：`ferrochart-wasm`（npm）、`ferrochart-winit`（桌面）、
  `ferrochart-py`（PyPI）和 `ferrochart-ffi`（C 库）。

## 从哪里开始

| 你使用的是 | 阅读 |
|---|---|
| 网页中的 JavaScript 或 TypeScript | [Web](web.md) |
| React、Vue 或其他框架 | [React 与 Vue](frameworks.md) |
| 桌面上的 Rust | [桌面](desktop.md) |
| Python、Jupyter 或 pandas | [Python](python.md) |
| C、C++、C# 或其他原生语言 | [C、C++ 与 C#](native.md) |
| 在服务器上生成图片 | [离屏渲染](offscreen.md) |
