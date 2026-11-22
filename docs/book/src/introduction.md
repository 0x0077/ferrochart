# Introduction

FerroChart is an open-source financial charting engine written in Rust. One
core drives every platform: the browser through WebAssembly (WebGPU or
WebGL2), native applications through wgpu (Metal, Vulkan, DirectX 12), Python
and C, and headless rendering on servers.

What it is built for:

- **Large data.** Ten million bars load in tens of milliseconds and pan as smoothly
  as two hundred: the per-frame work depends on the screen, not on the data size.
- **Real-time updates.** Updating the last bar costs well under a microsecond in
  the core and redraws only what changed.
- **One look everywhere.** All backends draw from the same frame description and
  are checked against the same golden images.

## Architecture in one picture

```text
 your app ──► Chart (ferrochart-core) ──► Frame ──► renderer ──► pixels
              data, scales, layout,        plain       wgpu (GPU) / tiny-skia (CPU)
              interaction, LOD             draw list
```

- `ferrochart-core` holds the data and all chart logic: scales, ticks, layout,
  input handling. It has no GPU, window, font or async dependencies and runs
  anywhere, including `wasm32-unknown-unknown`.
- Each frame it produces a `Frame`: a flat list of draw commands. Series data
  is not copied into it; renderers read the columns directly and upload only
  what changed.
- Renderers turn frames into pixels. `ferrochart-render-wgpu` is the primary
  one; `ferrochart-render-cpu` draws the same frames on the CPU.
- Bindings wrap it all for a platform: `ferrochart-wasm` (npm),
  `ferrochart-winit` (desktop), `ferrochart-py` (PyPI) and `ferrochart-ffi`
  (the C library).

## Where to start

| You use | Read |
|---|---|
| JavaScript or TypeScript in a web page | [Web](web.md) |
| React, Vue or another framework | [React and Vue](frameworks.md) |
| Rust on the desktop | [Desktop](desktop.md) |
| Python, Jupyter or pandas | [Python](python.md) |
| C, C++, C# or another native language | [C, C++ and C#](native.md) |
| Images on a server | [Offscreen](offscreen.md) |
