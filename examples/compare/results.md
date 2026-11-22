Measured 2026-11-20 on Apple M4 Max, 36 GB, darwin 25.3.0, Google Chrome 157.0.8103.51, page 1280×760.

Cells: p95 frame interval (ms) / dropped frames (interval > 1.5 refreshes). Lower is better.

| Library | Bars | CPU | Load → first paint | Pan, 200 visible | Pan, 100k visible | Wheel zoom |
|---|---:|---:|---:|---:|---:|---:|
| ferrochart (webgpu) | 100,000 | 1× | 15 ms | 9.3 / 0% | 9.3 / 0% | 9.2 / 0% |
| ferrochart (webgpu) | 100,000 | 4× | 65 ms | 9.2 / 0% | 9.2 / 0% | 9.2 / 0% |
| lightweight-charts (v5.3.0) | 100,000 | 1× | 156 ms | 9.3 / 0% | 67.5 / 46% | 9.3 / 0% |
| lightweight-charts (v5.3.0) | 100,000 | 4× | 573 ms | 9.2 / 0% | 208.6 / 47% | 9.2 / 0% |
| vela (native renderer) | 100,000 | 1× | 50 ms | 9.3 / 0% | n/a¹ | 9.2 / 0% |
| vela (native renderer) | 100,000 | 4× | 192 ms | 9.2 / 0% | n/a¹ | 9.3 / 0% |
| ferrochart (webgpu) | 1,000,000 | 1× | 43 ms | 9.3 / 0% | 9.3 / 0% | 9.1 / 0% |
| ferrochart (webgpu) | 1,000,000 | 4× | 150 ms | 9.2 / 0% | 9.3 / 0% | 9.2 / 0% |
| lightweight-charts (v5.3.0) | 1,000,000 | 1× | 1603 ms | 9.2 / 0% | 74.9 / 36% | 9.3 / 0% |
| lightweight-charts (v5.3.0) | 1,000,000 | 4× | 6537 ms | 9.3 / 1% | 225.8 / 44% | 25.5 / 52% |
| vela (native renderer) | 1,000,000 | 1× | 77 ms | 9.3 / 0% | n/a¹ | 9.3 / 0% |
| vela (native renderer) | 1,000,000 | 4× | 328 ms | 9.2 / 0% | n/a¹ | 9.3 / 0% |

¹ Vela cannot zoom out past 0.5 px per bar (a constant in its `ViewportState.ts`), about 2.4k bars in this page.
