<!-- source: 118e97bde8 -->
# 訂單流

`ferrochart-orderflow` 新增三種由成交與委託簿建構的系列。它們是[自訂系列](plugins.md#custom-series)，因此與 K 線共用圖表的時間軸、價格軸、十字游標與窗格。

| 系列 | 顯示內容 | 輸入 |
|---|---|---|
| **足跡圖（Footprint）** | 每根 K 線在各價位上的賣出與買進成交量 | 成交 |
| **熱力圖（Heatmap）** | 每個時間欄與價位的委託簿深度（或成交量） | 委託簿快照或成交 |
| **TPO**（市場輪廓） | 每個交易時段中，各價位在哪些時段有成交；控制點、價值區、初始平衡區 | K 線 |

三者皆可搭配所有渲染器使用。使用 WebGPU 與原生 GPU 時，熱力圖會在 compute shader 中彙總其格子。在 WebGL2 與 CPU 渲染器上，它使用可攜式路徑，輸出結果相同。

## 共通規則

- **價位。** 價格會四捨五入到 `step = tickSize × rowSize` 的格點上：價位為 `round(price / step)`，恰為一半時進位。每個價位涵蓋其中心價格 `±step / 2` 的範圍。
- **時間欄**從其週期（足跡圖的時間週期、熱力圖的欄長度）的 UTC 整數倍開始。
- **成交**是等長的欄：時間（UTC ms，非遞減）、價格、數量（≥ 0；0 會被忽略），以及選用的買賣方向。一批資料會先經過檢查；若無效，圖表不會改變。落在最後一欄的成交會更新該欄，較晚的成交則開啟新欄。早於最後一欄的成交會被拒絕。
- **主動方。** 買與賣依給定值使用。方向未知時採用 tick 規則：高於前一筆成交價 → 買，低於 → 賣，相等 → 沿用前一筆成交的方向（第一筆成交視為買）。

## 足跡圖

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

繪製的內容取決於 K 棒間距：

| K 棒間距（CSS px） | 繪製內容 |
|---|---|
| < 12 | 不繪製（K 線仍然可見） |
| 12 – 60 | 著色的格子 |
| ≥ 60 | 格子與數字 |

預設的 `maxBarSpacing` 為 50，因此若想放大到能看見數字，請以更大的值建立圖表。

- **模式。** `bidAsk`：左半部為賣出量，右半部為買進量。`delta`：買 − 賣，依正負著色。`volume`：總量。著色深淺相對於同一欄中的最大值。
- **失衡**以對角方式比較：某價位的買進量 ≥ `imbalanceRatio` × 下一個價位的賣出量，即為買方失衡（右緣顯示一條棒）。某價位的賣出量 ≥ 比率 × 上一個價位的買進量，即為賣方失衡（左緣顯示一條棒）。預設比率為 3；`null` 會關閉失衡顯示。
- **POC。** 啟用 `showPoc`（預設）時，每欄中成交量最大的價位會加上外框。
- 顏色預設使用主題的上漲／下跌／線條／文字顏色；可用 `colors: { buy, sell, volume, text, poc }` 覆寫。

## 熱力圖

```js
const book = chart.addHeatmap({
  source: 'depth',       // or 'trades' (fed with appendTrades)
  tickSize: 0.25,
  columnMs: 1_000,       // may be finer than the candles
});
book.appendDepth({ time: Date.now(), prices: bookPrices, sizes: bookSizes });
```

- 欄依時間放置在 K 線槽位之間，因此 1 秒的欄可以位於 1 分鐘的 K 線下方。它們不會新增槽位。
- **深度。** 在同一份快照中，落在同一價位的相鄰價格會相加。在同一欄中，每個價位保留各快照中的最大數量。
- **成交。** 數量依欄與價位相加；不使用買賣方向。
- **顏色。** 由小到大共五個色階（`colors`，含 alpha），採用 `log`（預設）或 `linear` 刻度。範圍預設為可見格子的 `{ percentile: 0.99 }`，並經過量化，使顏色在平移時不會閃爍；也可以用 `{ lo, hi }` 固定。
- 熱力圖預設不參與自動縮放，因為委託簿的價格跨度遠大於 K 線；設定 `autoScale: true` 即可將它納入。
- 當每個像素列可見的價位超過 8 個時，不會繪製任何東西。請放大或使用更大的 `rowSize`。

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

- **交易時段**每個 UTC 日在 `sessionOffsetMs` 開始一次，持續 `sessionMs`（一個時段長度到一天；預設：UTC 日）。每個交易時段是時間軸上的一個槽位。不屬於任何交易時段的 K 棒會被略過。
- **時段（period）**依序以 A–Z、再以 a–z 字母標示。K 棒歸屬於其開盤時間所在的時段。再次送出即時 K 棒會擴大其時段的範圍。
- 每個交易時段的**統計**：
  - **POC**：TPO 最多的價位。若平手，取最接近交易時段中間的價位，再取較低的價位。
  - **價值區**：占 TPO 的 `valueArea`（預設 70%），從 POC 開始每次擴展兩個價位，朝 TPO 較多的一側擴展（平手時向上）。
  - **初始平衡區**：前兩個時段的範圍。
- **繪製。** 當每個可見交易時段最寬的一列都能以可讀的字級容納時顯示字母，否則顯示直方圖；每影格統一選擇一種。初始平衡區以左側的一條棒表示，價值區以外的列會調暗。

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

| 系列 | 選項 | 資料 |
|---|---|---|
| `Footprint` | `FootprintOptions::new(tick_size, timeframe_ms, &theme)` | `append_trades` |
| `Heatmap` | `HeatmapOptions::new(tick_size, column_ms, HeatmapSource::Depth \| Trades)` | `append_depth(&DepthSnapshot)` / `append_trades` |
| `Tpo` | `TpoOptions::new(tick_size, &theme)` | `append_bars(&OhlcColumns)` |

若要顯示足跡圖數字與 TPO 字母，請設定 `ChartOptions::max_bar_spacing`。使用 `ferrochart-render-wgpu` 時，請呼叫一次 `ferrochart_orderflow::register_gpu(&ctx,
&mut renderer)`（搭配 `ferrochart-winit` 時在 `ChartApp::on_renderer` 中呼叫）。它會傳回裝置是否支援原生熱力圖。此功能由預設 feature `gpu` 提供；不啟用該 feature 時，此 crate 不依賴 wgpu。

交易時段的統計可從 `Tpo::profile(i)` 取得（POC、價值區、初始平衡區），原始格子則可從 `Footprint::grid()` 與 `Heatmap::grid()` 取得。

## 效能

在 1,000,000 筆成交上同時顯示一分鐘足跡圖與一秒成交熱力圖（Apple M4 Max，1600×900 CSS px、2×，原生熱力圖），每影格：

| 情境 | p50 | p99 |
|---|---|---|
| 平移，足跡圖數字 | 0.47 ms | 1.6 ms |
| 平移，足跡圖格子 | 0.65 ms | 1.6 ms |
| 縮放，足跡圖 + 熱力圖 | 0.78 ms | 3.3 ms |
| 平移，全部可見 | 3.9 ms | 6.8 ms |
| 平移，TPO 字母 | 0.28 ms | 1.1 ms |

價格已存在於最後一欄的即時成交、寫入最後一欄的委託簿快照，以及最後一個 TPO 交易時段中的即時 K 棒，都不會配置記憶體。新的價位與新的欄會配置記憶體，與附加 K 線相同。

以 `cargo bench -p ferrochart-bench --bench orderflow` 重現結果。
