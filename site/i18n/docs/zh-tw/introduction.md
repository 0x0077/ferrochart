<!-- source: f6f2d1d064 -->
# 簡介

FerroChart 是一套以 Rust 撰寫的開源金融圖表引擎。同一個核心驅動所有平台：在瀏覽器中透過 WebAssembly（WebGPU 或 WebGL2），在原生應用程式中透過 wgpu（Metal、Vulkan、DirectX 12），也支援 Python 與 C，以及在伺服器上進行無頭（headless）渲染。

它的設計目標：

- **大量資料。** 一千萬根 K 線只需數十毫秒即可載入，平移起來和兩百根一樣流暢：每一幀的工作量取決於螢幕，而非資料量。
- **即時更新。** 在核心中更新最後一根 K 線的成本遠低於一微秒，而且只重繪有變動的部分。
- **處處一致的外觀。** 所有後端都依據同一份幀描述進行繪製，並以同一組黃金圖像（golden image）驗證。

## 一張圖看懂架構

```text
 your app ──► Chart (ferrochart-core) ──► Frame ──► renderer ──► pixels
              data, scales, layout,        plain       wgpu (GPU) / tiny-skia (CPU)
              interaction, LOD             draw list
```

- `ferrochart-core` 持有資料與所有圖表邏輯：座標軸、刻度、版面配置、輸入處理。它不依賴任何 GPU、視窗、字型或非同步函式庫，可在任何地方執行，包括 `wasm32-unknown-unknown`。
- 每一幀它都會產生一個 `Frame`：一份扁平的繪圖指令清單。系列資料不會被複製進去；渲染器直接讀取欄位資料，並只上傳有變動的部分。
- 渲染器把幀轉換成像素。`ferrochart-render-wgpu` 是主要的渲染器；`ferrochart-render-cpu` 則在 CPU 上繪製相同的幀。
- 繫結（binding）為各平台封裝上述一切：`ferrochart-wasm`（npm）、`ferrochart-winit`（桌面）、`ferrochart-py`（PyPI）以及 `ferrochart-ffi`（C 函式庫）。

## 從哪裡開始

| 你使用的是 | 請閱讀 |
|---|---|
| 網頁中的 JavaScript 或 TypeScript | [Web](web.md) |
| React、Vue 或其他框架 | [React 與 Vue](frameworks.md) |
| 桌面上的 Rust | [桌面](desktop.md) |
| Python、Jupyter 或 pandas | [Python](python.md) |
| C、C++、C# 或其他原生語言 | [C、C++ 與 C#](native.md) |
| 在伺服器上產生圖片 | [離屏渲染](offscreen.md) |
