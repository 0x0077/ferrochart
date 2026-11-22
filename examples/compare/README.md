# Library comparison

Same-scenario comparison of FerroChart, [lightweight-charts](https://github.com/tradingview/lightweight-charts)
and [Vela](https://github.com/LuxAlgo/Vela) in the browser.

## Method

- **Same data.** Every library gets the same bars from FerroChart's deterministic
  random-walk generator (`randomWalk(2026, n, …)`, one-minute bars with volume).
  Converting them into each library's input format (arrays of objects, seconds
  for lightweight-charts) happens before the clock starts.
- **Same page.** Each run loads one library into the same 1280×760 page in a
  fresh tab, with a candlestick series and volume, dark theme, magnet crosshair.
- **Same input.** Playwright drives Google Chrome with trusted mouse events:
  dragging back and forth for 4 s, and 30 wheel notches out and back in.
  Vela's eased zoom/pan animations are turned off so that every library renders
  each input immediately.
- **Measured:** load → first painted frame, and the distribution of
  `requestAnimationFrame` intervals while the script runs: p95 interval and the
  share of "dropped" frames (intervals longer than 1.5 display refreshes). On a
  120 Hz display a smooth run is ≈ 8.3–9.3 ms with 0 % dropped.
- **Scenarios:** pan with the newest 200 bars visible, pan with 100 000 bars
  visible, wheel zoom; each at 1× CPU and with Chrome's 4× CPU throttling (a
  stand-in for a slower laptop).

Frame intervals only show whether a library keeps up with the display; when all
of them do (most rows at 1×), they say nothing about headroom. FerroChart's own
per-frame cost is measured natively by `cargo bench -p ferrochart-bench --bench frame_time`.

## Results

See [results.md](results.md) (generated) and [results.json](results.json).

## Running it

```bash
(cd crates/ferrochart-wasm && wasm-pack build --release --target web)
cd examples/compare
npm install
node bench.mjs              # QUICK=1 node bench.mjs for a short check
```

Screenshots of every pan scenario (before/after) are written to `out/` to check
that each library really shows the scenario's view. To try the page by hand,
serve the repository root (`python3 -m http.server`) and open
`http://localhost:8000/examples/compare/`.
