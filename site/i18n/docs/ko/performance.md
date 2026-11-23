<!-- source: 7e182ce114 -->
# 성능

## 목표

| | 시나리오 | 목표 |
|---|---|---|
| P1 | `set_data`, 봉 1,000만 개, LOD 피라미드 포함 | < 300 ms |
| P2 | 봉당 메모리(OHLCV, 정상 상태) | < 64바이트 |
| P3 | 이동/확대·축소 프레임의 코어 CPU 시간 | < 0.5 ms, 데이터 크기와 무관 |
| P4 | 이동/확대·축소 프레임, 엔드 투 엔드 | < 8 ms |
| P5 | `update_last` | < 20 µs |
| P6 | 차트 16개 × 봉 100만 개, 연동 크로스헤어 | 안정적인 60 fps |
| P7 | WebAssembly 패키지(gzip) | ≤ 1.5 MB |

## 측정값

Apple M4 Max, 릴리스 빌드:

| | 측정값 |
|---|---|
| P1 | 73 ms |
| P2 | 58바이트 |
| P3 | 19 µs(봉 100만 개와 1,000만 개 모두) |
| P4 | p99 2.2 ms(이동), 2.9 ms(확대·축소), 3.2 ms(봉 1,000만 개 전체가 보이는 상태에서 이동) — 3200 × 1800 px, Metal |
| P5 | 0.3 µs(지표 30개 전체 적용 시 17.1 µs) |
| P6 | 프레임당 p50 1.49 ms, p95 2.76 ms |
| P7 | 1.4 MB |

## 브라우저에서

Chrome에서 FerroChart, lightweight-charts, Vela에 같은 데이터, 같은 페이지, 같은
마우스 입력을 주었습니다(`examples/compare`). 봉 100만 개 기준:

| | 로드 → 첫 페인트 | 봉 10만 개가 보이는 상태에서 이동(p95 프레임 간격 / 드롭) |
|---|---:|---:|
| FerroChart | 43 ms | 9.3 ms / 0 % |
| lightweight-charts 5.3.0 | 1603 ms | 74.9 ms / 36 % |
| Vela 0.10.13 | 77 ms | 해당 없음 — 그만큼 축소할 수 없음 |

## 재현

```bash
cargo bench -p ferrochart-core                       # P1, P3, P5
cargo bench -p ferrochart-bench --bench memory        # P2
cargo bench -p ferrochart-bench --bench frame_time    # P4 (needs a GPU)
cargo bench -p ferrochart-bench --bench sync          # P6 (needs a GPU)
cd examples/compare && npm install && node bench.mjs  # browser comparison
```
