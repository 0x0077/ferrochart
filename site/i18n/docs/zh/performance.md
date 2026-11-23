<!-- source: 7e182ce114 -->
# 性能

## 目标

| | 场景 | 目标 |
|---|---|---|
| P1 | `set_data`，1000 万根 K 线，包括 LOD 金字塔 | < 300 ms |
| P2 | 每根 K 线的内存（OHLCV，稳定状态） | < 64 字节 |
| P3 | 平移/缩放帧的核心 CPU 耗时 | < 0.5 ms，与数据量无关 |
| P4 | 平移/缩放帧，端到端 | < 8 ms |
| P5 | `update_last` | < 20 µs |
| P6 | 16 个图表 × 100 万根 K 线，联动十字光标 | 稳定 60 fps |
| P7 | WebAssembly 包（gzip） | ≤ 1.5 MB |

## 实测

Apple M4 Max，release 构建：

| | 实测值 |
|---|---|
| P1 | 73 ms |
| P2 | 58 字节 |
| P3 | 19 µs（100 万和 1000 万根 K 线相同） |
| P4 | p99 2.2 ms（平移）、2.9 ms（缩放）、3.2 ms（1000 万根 K 线全部可见时平移）——3200 × 1800 px，Metal |
| P5 | 0.3 µs（启用全部 30 个指标时 17.1 µs） |
| P6 | 每帧 p50 1.49 ms，p95 2.76 ms |
| P7 | 1.4 MB |

## 在浏览器中

在 Chrome 中，对 FerroChart、lightweight-charts 和 Vela 使用相同的数据、相同的页面和相同的鼠标输入（`examples/compare`）。100 万根 K 线时：

| | 加载 → 首次绘制 | 10 万根 K 线可见时平移（p95 帧间隔 / 丢帧率） |
|---|---:|---:|
| FerroChart | 43 ms | 9.3 ms / 0 % |
| lightweight-charts 5.3.0 | 1603 ms | 74.9 ms / 36 % |
| Vela 0.10.13 | 77 ms | 不适用——无法缩小到这个程度 |

## 复现

```bash
cargo bench -p ferrochart-core                       # P1, P3, P5
cargo bench -p ferrochart-bench --bench memory        # P2
cargo bench -p ferrochart-bench --bench frame_time    # P4 (needs a GPU)
cargo bench -p ferrochart-bench --bench sync          # P6 (needs a GPU)
cd examples/compare && npm install && node bench.mjs  # browser comparison
```
