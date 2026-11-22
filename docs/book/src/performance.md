# Performance

## Targets

| | Scenario | Target |
|---|---|---|
| P1 | `set_data`, 10 million bars, including the LOD pyramid | < 300 ms |
| P2 | Memory per bar (OHLCV, steady state) | < 64 bytes |
| P3 | Core CPU time of a pan/zoom frame | < 0.5 ms, independent of data size |
| P4 | Pan/zoom frame, end to end | < 8 ms |
| P5 | `update_last` | < 20 µs |
| P6 | 16 charts × 1M bars, linked crosshair | steady 60 fps |
| P7 | WebAssembly package (gzip) | ≤ 1.5 MB |

## Measured

Apple M4 Max, release builds:

| | Measured |
|---|---|
| P1 | 73 ms |
| P2 | 58 bytes |
| P3 | 19 µs (1M and 10M bars alike) |
| P4 | p99 2.2 ms (pan), 2.9 ms (zoom), 3.2 ms (pan with all 10M bars visible) — 3200 × 1800 px, Metal |
| P5 | 0.3 µs (17.1 µs with all 30 indicators) |
| P6 | p50 1.49 ms, p95 2.76 ms per frame |
| P7 | 1.4 MB |

## In the browser

Same data, same page and same mouse input for FerroChart, lightweight-charts
and Vela in Chrome (`examples/compare`). With 1M bars:

| | Load → first paint | Pan with 100k bars visible (p95 frame interval / dropped) |
|---|---:|---:|
| FerroChart | 43 ms | 9.3 ms / 0 % |
| lightweight-charts 5.3.0 | 1603 ms | 74.9 ms / 36 % |
| Vela 0.10.13 | 77 ms | n/a — cannot zoom out that far |

## Reproducing

```bash
cargo bench -p ferrochart-core                       # P1, P3, P5
cargo bench -p ferrochart-bench --bench memory        # P2
cargo bench -p ferrochart-bench --bench frame_time    # P4 (needs a GPU)
cargo bench -p ferrochart-bench --bench sync          # P6 (needs a GPU)
cd examples/compare && npm install && node bench.mjs  # browser comparison
```
