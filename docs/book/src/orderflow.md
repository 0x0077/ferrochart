# Order flow

`ferrochart-orderflow` adds three series built from trades and order books.
They are [custom series](plugins.md#custom-series), so they share the chart's
time axis, price axis, crosshair and panes with the candles.

| Series | Shows | Input |
|---|---|---|
| **Footprint** | per candle, the volume sold and bought at each price level | trades |
| **Heatmap** | order book depth (or traded volume) per time column and price level | order book snapshots or trades |
| **TPO** (market profile) | per session, which periods traded at each price; point of control, value area, initial balance | candles |

All three work with every renderer. With WebGPU and native GPUs, the heatmap
aggregates its cells in a compute shader. On WebGL2 and the CPU renderer it
uses its portable path, and the output is the same.

## Common rules

- **Price levels.** Prices are rounded to a grid of `step = tickSize × rowSize`:
  level `round(price / step)`, with halves going up. A level covers
  `±step / 2` around its center price.
- **Time columns** start at UTC multiples of their period (the footprint's
  timeframe, the heatmap's column length).
- **Trades** are columns of equal length: time (UTC ms, non-decreasing), price,
  size (≥ 0; zero is ignored) and an optional side. A batch is checked first;
  if it is invalid, the chart does not change. Trades in the last column update
  it, and later trades open new columns. Trades earlier than the last column
  are rejected.
- **Aggressor side.** Buy and sell are used as given. Unknown sides follow the
  tick rule: above the previous trade's price → buy, below → sell, equal →
  the previous trade's side (the first trade counts as a buy).

## Footprint

```js
const chart = await FerroChart.create(canvas, { maxBarSpacing: 200 });
const candles = chart.addCandlestickSeries();
const footprint = chart.addFootprint({
  tickSize: 0.25,
  timeframeMs: 60_000,   // the candles' period
  rowSize: 4,            // ticks per row
  mode: 'bidAsk',        // 'bidAsk' | 'delta' | 'volume'
});
footprint.appendTrades({
  time: new Float64Array([...]),
  price: new Float64Array([...]),
  size: new Float64Array([...]),
  side: new Int8Array([...]), // 1 buy, -1 sell, 0 unknown; optional
});
```

What is drawn depends on the bar spacing:

| Bar spacing (CSS px) | Drawn |
|---|---|
| < 12 | nothing (the candles stay visible) |
| 12 – 60 | shaded cells |
| ≥ 60 | cells and numbers |

The default `maxBarSpacing` is 50, so create the chart with a larger one if
you want to zoom in to the numbers.

- **Modes.** `bidAsk`: sell volume in the left half, buy volume in the right.
  `delta`: buy − sell, colored by sign. `volume`: the total. Shading intensity
  is relative to the largest value in the same column.
- **Imbalances** compare diagonally: buy at a level ≥ `imbalanceRatio` × sell
  one level below (a buy imbalance, bar on the right edge). Sell at a level ≥
  ratio × buy one level above is a sell imbalance (bar on the left edge). The
  default ratio is 3; `null` turns imbalances off.
- **POC.** With `showPoc` (the default), the level with the most volume in each
  column is outlined.
- Colors default to the theme's up/down/line/text colors; override them with
  `colors: { buy, sell, volume, text, poc }`.

## Heatmap

```js
const book = chart.addHeatmap({
  source: 'depth',       // or 'trades' (fed with appendTrades)
  tickSize: 0.25,
  columnMs: 1_000,       // may be finer than the candles
});
book.appendDepth({ time: Date.now(), prices: bookPrices, sizes: bookSizes });
```

- Columns are placed by time between the candle slots, so 1-second columns can
  lie under 1-minute candles. They add no slots.
- **Depth.** Within one snapshot, adjacent prices that fall on the same level
  are added. Within one column, each level keeps its largest size over the
  snapshots.
- **Trades.** Sizes are added per column and level; the side is not used.
- **Colors.** Five color stops from small to large values (`colors`, with
  alpha), on a `log` (default) or `linear` scale. The range is
  `{ percentile: 0.99 }` of the visible cells by default, quantized so colors do
  not flicker while panning, or fixed with `{ lo, hi }`.
- The heatmap does not take part in auto scaling by default, because order
  books span far more than the candles; set `autoScale: true` to include it.
- When more levels are visible than 8 per pixel row, nothing is drawn. Zoom
  in or use a larger `rowSize`.

## TPO

```js
const chart = await FerroChart.create(canvas, { maxBarSpacing: 600 });
const tpo = chart.addTpo({
  tickSize: 0.25,
  rowSize: 4,
  periodMs: 30 * 60_000,          // one letter per period (default 30 min)
  sessionOffsetMs: 14.5 * 3_600_000, // New York cash session: 14:30 UTC…
  sessionMs: 6.5 * 3_600_000,        // …for 6.5 hours
});
tpo.appendBars(oneMinuteBars);     // time, high and low are used
```

- **Sessions** start once per UTC day at `sessionOffsetMs` and last
  `sessionMs` (a period to a day; default: the UTC day). Each session is one
  slot of the time axis. Bars outside every session are skipped.
- **Periods** are lettered A–Z, then a–z. A bar belongs to the period its open
  time falls in. Sending the live bar again widens its period's range.
- **Statistics** per session:
  - **POC**: the level with the most TPOs. Ties go to the level nearest the
    session's middle, then to the lower level.
  - **Value area**: `valueArea` of the TPOs (default 70%), grown from the POC
    two levels at a time toward the side with more TPOs (ties go up).
  - **Initial balance**: the range of the first two periods.
- **Drawing.** Letters when every visible session's widest row fits at a
  readable font size, histograms otherwise; one choice per frame. The initial
  balance is a bar on the left, and rows outside the value area are dimmed.

## Rust

```rust
use ferrochart_core::chart::SeriesOptions;
use ferrochart_orderflow::{Footprint, FootprintOptions, Side, TickColumns, append_trades};

let fp = Footprint::new(FootprintOptions::new(0.25, 60_000, &theme))?;
let id = chart.add_custom_series(chart.main_pane(), Box::new(fp), SeriesOptions::default())?;
append_trades(&mut chart, id, &TickColumns {
    time, price, size,
    side: Vec::new(), // all unknown: tick rule
})?;
```

| Series | Options | Data |
|---|---|---|
| `Footprint` | `FootprintOptions::new(tick_size, timeframe_ms, &theme)` | `append_trades` |
| `Heatmap` | `HeatmapOptions::new(tick_size, column_ms, HeatmapSource::Depth \| Trades)` | `append_depth(&DepthSnapshot)` / `append_trades` |
| `Tpo` | `TpoOptions::new(tick_size, &theme)` | `append_bars(&OhlcColumns)` |

Set `ChartOptions::max_bar_spacing` for footprint numbers and TPO letters.
With `ferrochart-render-wgpu`, call `ferrochart_orderflow::register_gpu(&ctx,
&mut renderer)` once (in `ChartApp::on_renderer` with `ferrochart-winit`). It
returns whether the device supports the native heatmap. The default feature
`gpu` provides it; without the feature, the crate does not depend on wgpu.

Session statistics are available from `Tpo::profile(i)` (POC, value area,
initial balance), and the raw cells from `Footprint::grid()` and
`Heatmap::grid()`.

## Performance

A one-minute footprint plus a one-second trade heatmap over 1,000,000 trades
(Apple M4 Max, 1600×900 CSS px at 2×, native heatmap), per frame:

| Scenario | p50 | p99 |
|---|---|---|
| Pan, footprint numbers | 0.47 ms | 1.6 ms |
| Pan, footprint cells | 0.65 ms | 1.6 ms |
| Zoom, footprint + heatmap | 0.78 ms | 3.3 ms |
| Pan, everything visible | 3.9 ms | 6.8 ms |
| Pan, TPO letters | 0.28 ms | 1.1 ms |

Real-time trades at prices already in the last column, order book snapshots
into the last column and live bars in the last TPO session allocate nothing.
New price levels and new columns allocate, like appending candles.

Reproduce with `cargo bench -p ferrochart-bench --bench orderflow`.
