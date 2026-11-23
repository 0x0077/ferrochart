<!-- source: f3b74de278 -->
# Rust API

API 文件由 rustdoc 產生。在本機執行：

```bash
cargo doc --workspace --exclude ferrochart-py --no-deps --open
```

發布於 docs.rs：

| Crate | 內容 |
|---|---|
| [`ferrochart-core`](https://docs.rs/ferrochart-core) | `Chart`、資料模型、座標軸、輸入、`Frame`、`Renderer` trait、`transform_helpers` |
| [`ferrochart-render-wgpu`](https://docs.rs/ferrochart-render-wgpu) | `GpuContext`、`WgpuRenderer`（surface 或離屏） |
| [`ferrochart-render-cpu`](https://docs.rs/ferrochart-render-cpu) | `CpuRenderer`（tiny-skia） |
| [`ferrochart-text`](https://docs.rs/ferrochart-text) | `TextEngine`、內建字型 |
| [`ferrochart-winit`](https://docs.rs/ferrochart-winit) | `run`、`ChartApp`、`InputConverter` |
| [`ferrochart-indicators`](https://docs.rs/ferrochart-indicators) | 增量指標 |
| [`ferrochart-orderflow`](https://docs.rs/ferrochart-orderflow) | 足跡圖、熱力圖與 TPO 系列（[訂單流](orderflow.md)） |

擴充圖表（圖層、自訂系列、原生 GPU 系列、座標變換、繪圖工具）的方法請見[外掛](plugins.md)。

## 從哪裡開始

- `ferrochart_core::Chart`：以 `ChartOptions` 建立，呼叫 `resize`，加入系列，設定資料，用 `handle_input` 送入 `InputEvent`，呼叫 `build_frame`，再用 `drain_events` 讀取事件。
- `ferrochart_core::render::Renderer`：兩種渲染器都實作了它；你只需呼叫 `render(chart.frame(), &chart.data_view())`。

## API 遵循的規則

- **不會 panic。** 可能失敗的方法回傳 `Result`；以 id 查詢的方法回傳 `Result` 或 `Option`。
- **熱路徑上不配置記憶體。** 平移、縮放、移動十字游標（`build_frame`）以及 `update_last` 在預熱之後完全不配置記憶體；測試以計數配置器（counting allocator）驗證這一點。
- **帶型別的座標。** `Css` 與 `Px`（以及以它們為基礎的點、尺寸、矩形型別）將 CSS 像素與裝置像素區分開來；裝置像素比傳給 `Chart::resize`。
- **核心保持純粹。** `ferrochart-core` 除了標準函式庫之外沒有其他相依套件（唯一允許的是 `smallvec`）：不依賴 GPU、視窗、字型、時間或非同步函式庫，也不使用 `unsafe`。
