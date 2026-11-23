<!-- source: 7e182ce114 -->
# 效能

## 目標

| | 情境 | 目標 |
|---|---|---|
| P1 | `set_data`，一千萬根 K 棒，包含 LOD 金字塔 | < 300 ms |
| P2 | 每根 K 棒的記憶體（OHLCV，穩定狀態） | < 64 bytes |
| P3 | 平移／縮放影格的核心 CPU 時間 | < 0.5 ms，與資料量無關 |
| P4 | 平移／縮放影格，端對端 | < 8 ms |
| P5 | `update_last` | < 20 µs |
| P6 | 16 個圖表 × 1M 根 K 棒，連動十字游標 | 穩定 60 fps |
| P7 | WebAssembly 套件（gzip） | ≤ 1.5 MB |

## 實測

Apple M4 Max，release 建置：

| | 實測值 |
|---|---|
| P1 | 73 ms |
| P2 | 58 bytes |
| P3 | 19 µs（1M 與 10M 根 K 棒相同） |
| P4 | p99 2.2 ms（平移）、2.9 ms（縮放）、3.2 ms（一千萬根 K 棒全部可見時平移）——3200 × 1800 px，Metal |
| P5 | 0.3 µs（啟用全部 30 個指標時為 17.1 µs） |
| P6 | 每影格 p50 1.49 ms、p95 2.76 ms |
| P7 | 1.4 MB |

## 在瀏覽器中

在 Chrome 中，以相同的資料、相同的頁面與相同的滑鼠輸入測試 FerroChart、lightweight-charts 與 Vela（`examples/compare`）。1M 根 K 棒時：

| | 載入 → 首次繪製 | 10 萬根 K 棒可見時平移（p95 影格間隔／掉格率） |
|---|---:|---:|
| FerroChart | 43 ms | 9.3 ms / 0 % |
| lightweight-charts 5.3.0 | 1603 ms | 74.9 ms / 36 % |
| Vela 0.10.13 | 77 ms | 不適用——無法縮小到那麼遠 |

## 重現

```bash
cargo bench -p ferrochart-core                       # P1, P3, P5
cargo bench -p ferrochart-bench --bench memory        # P2
cargo bench -p ferrochart-bench --bench frame_time    # P4 (needs a GPU)
cargo bench -p ferrochart-bench --bench sync          # P6 (needs a GPU)
cd examples/compare && npm install && node bench.mjs  # browser comparison
```
