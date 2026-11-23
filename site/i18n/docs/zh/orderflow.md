<!-- source: 118e97bde8 -->
# 订单流

`ferrochart-orderflow` 提供三种基于成交和订单簿构建的系列。它们是[自定义系列](plugins.md#custom-series)，因此与 K 线共用图表的时间轴、价格轴、十字光标和窗格。

| 系列 | 显示内容 | 输入 |
|---|---|---|
| **足迹图** | 每根 K 线在各价位上的卖出量和买入量 | 成交 |
| **热力图** | 每个时间列、每个价位的订单簿深度（或成交量） | 订单簿快照或成交 |
| **TPO**（市场轮廓） | 每个交易时段中各价位在哪些时段有成交；控制点、价值区、初始平衡区 | K 线 |

三者都适用于所有渲染器。在 WebGPU 和原生 GPU 上，热力图在计算着色器中聚合单元格。在 WebGL2 和 CPU 渲染器上，它使用可移植路径，输出结果相同。

## 通用规则

- **价位。** 价格会被取整到 `step = tickSize × rowSize` 的网格上：价位为 `round(price / step)`，0.5 向上取整。每个价位覆盖其中心价格上下 `±step / 2` 的范围。
- **时间列**从其周期（足迹图的时间周期、热力图的列长度）的 UTC 整数倍处开始。
- **成交**是等长的列：时间（UTC ms，非递减）、价格、数量（≥ 0；0 会被忽略）以及可选的方向。每批数据会先被检查；如果无效，图表不会改变。属于最后一列的成交会更新该列，更晚的成交会开启新列。早于最后一列的成交会被拒绝。
- **主动方方向。** 买入和卖出按给定值使用。未知方向遵循 tick 规则：高于上一笔成交价 → 买入，低于 → 卖出，相等 → 与上一笔成交方向相同（第一笔成交视为买入）。

## 足迹图

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

绘制的内容取决于 K 线间距：

| K 线间距（CSS px） | 绘制内容 |
|---|---|
| < 12 | 不绘制（K 线保持可见） |
| 12 – 60 | 着色单元格 |
| ≥ 60 | 单元格和数字 |

默认 `maxBarSpacing` 为 50，因此如果想放大到能看见数字，请在创建图表时设置更大的值。

- **模式。** `bidAsk`：左半边为卖出量，右半边为买入量。`delta`：买入 − 卖出，按正负着色。`volume`：总量。着色深浅相对于同一列中的最大值。
- **失衡**按对角线比较：某价位的买入量 ≥ `imbalanceRatio` × 下方一个价位的卖出量（买方失衡，右边缘显示一条竖条）。某价位的卖出量 ≥ 比率 × 上方一个价位的买入量为卖方失衡（左边缘显示竖条）。默认比率为 3；`null` 关闭失衡显示。
- **POC。** 启用 `showPoc`（默认）时，每列中成交量最大的价位会被描边。
- 颜色默认使用主题的上涨/下跌/线条/文字颜色；可以用 `colors: { buy, sell, volume, text, poc }` 覆盖。

## 热力图

```js
const book = chart.addHeatmap({
  source: 'depth',       // or 'trades' (fed with appendTrades)
  tickSize: 0.25,
  columnMs: 1_000,       // may be finer than the candles
});
book.appendDepth({ time: Date.now(), prices: bookPrices, sizes: bookSizes });
```

- 列按时间放置在 K 线槽位之间，因此 1 秒的列可以位于 1 分钟 K 线下方。它们不会增加槽位。
- **深度。** 在同一个快照中，落在同一价位上的相邻价格会相加。在同一列中，每个价位取所有快照中的最大数量。
- **成交。** 数量按列和价位累加；不使用方向。
- **颜色。** 从小到大的五个色标（`colors`，带 alpha），采用 `log`（默认）或 `linear` 刻度。范围默认为可见单元格的 `{ percentile: 0.99 }`，并经过量化，使平移时颜色不会闪烁；也可以用 `{ lo, hi }` 固定。
- 热力图默认不参与自动缩放，因为订单簿的价格跨度远大于 K 线；设置 `autoScale: true` 可将其纳入。
- 当每个像素行可见的价位超过 8 个时，不绘制任何内容。请放大或使用更大的 `rowSize`。

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

- **交易时段**在每个 UTC 日的 `sessionOffsetMs` 处开始，持续 `sessionMs`（从一个周期到一天；默认：整个 UTC 日）。每个交易时段占时间轴的一个槽位。不在任何交易时段内的 K 线会被跳过。
- **周期**依次以字母 A–Z、然后 a–z 标记。K 线属于其开盘时间所在的周期。再次发送实时 K 线会扩大其所在周期的范围。
- 每个交易时段的**统计量**：
  - **POC**：TPO 数最多的价位。数量相同时取最接近时段中间的价位，再相同则取较低的价位。
  - **价值区**：占 TPO 总数 `valueArea`（默认 70%）的区域，从 POC 开始每次向 TPO 较多的一侧扩展两个价位（相同时向上）。
  - **初始平衡区**：前两个周期的价格范围。
- **绘制。** 当每个可见交易时段最宽的一行都能以可读的字号放下时绘制字母，否则绘制直方图；每帧统一选择一种。初始平衡区显示为左侧的一条竖条，价值区以外的行会变暗。

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

| 系列 | 选项 | 数据 |
|---|---|---|
| `Footprint` | `FootprintOptions::new(tick_size, timeframe_ms, &theme)` | `append_trades` |
| `Heatmap` | `HeatmapOptions::new(tick_size, column_ms, HeatmapSource::Depth \| Trades)` | `append_depth(&DepthSnapshot)` / `append_trades` |
| `Tpo` | `TpoOptions::new(tick_size, &theme)` | `append_bars(&OhlcColumns)` |

要显示足迹图数字和 TPO 字母，请设置 `ChartOptions::max_bar_spacing`。使用 `ferrochart-render-wgpu` 时，调用一次 `ferrochart_orderflow::register_gpu(&ctx,
&mut renderer)`（使用 `ferrochart-winit` 时在 `ChartApp::on_renderer` 中调用）。它返回设备是否支持原生热力图。默认 feature `gpu` 提供这一功能；不启用该 feature 时，该 crate 不依赖 wgpu。

交易时段统计量可通过 `Tpo::profile(i)` 获取（POC、价值区、初始平衡区），原始单元格可通过 `Footprint::grid()` 和 `Heatmap::grid()` 获取。

## 性能

一分钟足迹图加一秒成交热力图，共 1,000,000 笔成交（Apple M4 Max，1600×900 CSS px，2× 缩放，原生热力图），每帧耗时：

| 场景 | p50 | p99 |
|---|---|---|
| 平移，足迹图数字 | 0.47 ms | 1.6 ms |
| 平移，足迹图单元格 | 0.65 ms | 1.6 ms |
| 缩放，足迹图 + 热力图 | 0.78 ms | 3.3 ms |
| 平移，全部可见 | 3.9 ms | 6.8 ms |
| 平移，TPO 字母 | 0.28 ms | 1.1 ms |

价格已在最后一列中的实时成交、写入最后一列的订单簿快照，以及最后一个 TPO 交易时段中的实时 K 线都不分配内存。新的价位和新的列会分配内存，与追加 K 线一样。

用 `cargo bench -p ferrochart-bench --bench orderflow` 复现。
