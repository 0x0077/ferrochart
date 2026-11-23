<!-- source: 32f28d0f34 -->
# 指标

FerroChart 内置 30 个技术指标。指标读取一个源系列，计算出一个或多个输出系列，并在源变化时保持它们同步更新。输出就是普通的系列：你可以像对待其他系列一样设置样式、读取数值、添加标记。

## 添加指标

```ts
const sma = chart.addIndicator(candles, 'sma', { n: 50 });
const boll = chart.addIndicator(candles, 'boll', { n: 20, k: 2 });
const macd = chart.addIndicator(candles, 'macd', { fast: 12, slow: 26, signal: 9 });
const rsi = chart.addIndicator(candles, 'rsi');        // all defaults
```

`chart.addIndicator(source, name, params?, placement?)` 返回一个
`Indicator`。

| 参数 | 含义 |
|---|---|
| `source` | 用于计算的系列。除成交量系列和自定义系列外，任何系列都可以。 |
| `name` | 下表中的名称之一，例如 `'sma'`、`'stochRsi'`。也接受 `snake_case` 写法（`'stoch_rsi'`、`'volume_ma'`）。 |
| `params` | 按名称给出的参数，采用 camelCase。缺省的参数取默认值。 |
| `placement` | 输出放在哪里；默认为 `'default'`。 |

指标会在 `addIndicator` 返回之前，一次性在所有现有数据上完成计算。

### 放置位置

```ts
type Placement = 'default' | 'overlay' | 'newPane' | 'volume' | number;
```

| 值 | 输出放到 |
|---|---|
| `'default'` | 指标的惯常位置：均线和通道为 `'overlay'`，震荡指标为 `'newPane'`，成交量均线为 `'volume'`（见下表） |
| `'overlay'` | 源系列所在窗格，使用其价格轴 |
| `'newPane'` | 为该指标在其他窗格下方新建的窗格 |
| `'volume'` | 源系列的成交量系列所在窗格，使用成交量坐标。源系列没有成交量系列时，新建一个窗格 |
| 窗格 id | 指定的窗格（来自 `chart.panes()`、`chart.mainPane`、`chart.addPane()` 或 `series.pane`） |

```ts
// Two oscillators in one pane.
const rsi = chart.addIndicator(candles, 'rsi');
const pane = rsi.outputs()[0].pane;
chart.addIndicator(candles, 'stochRsi', {}, pane);
```

由 `'newPane'`（或在没有成交量系列时由 `'volume'`）创建的窗格，在其上没有其他内容时会随指标一并移除。

### 源系列

OHLC 系列（K 线、OHLC 柱、Heikin-Ashi）向指标提供开盘价、最高价、最低价、收盘价和成交量。单值系列（折线、面积图、
基线图、柱状图、散点）提供 `open = high = low = close = value`，没有成交量。因此可以对折线系列计算 RSI，但不能计算 OBV。

需要成交量的指标（VWMA、成交量均线、VWAP、MFI、OBV、CMF）在源为单值系列，或源为数据中没有成交量列的 OHLC 系列时，会抛出异常。

源系列没有值的 K 线（例如只有另一个系列才有的时间点）会被跳过：这些位置的输出为 `NaN`，该 K 线也不计入指标的窗口。

### 指标的指标

源可以是另一个指标的输出。例如，在 RSI 窗格中画出 RSI 的 9 周期 SMA：

```ts
const rsiLine = chart.addIndicator(candles, 'rsi').outputs()[0];
const rsiAvg = chart.addIndicator(rsiLine, 'sma', { n: 9 }, rsiLine.pane);
```

移除 RSI 时，SMA 也会被移除。

## 内置指标

`indicatorNames()` 按本表顺序返回这 30 个名称。
输出按 `outputs()` 的返回顺序列出。除标注（histogram，柱状图）或（points，散点）的以外，所有输出都是折线。

| 名称 | 指标 | 参数（默认值） | 输出 | 默认位置 |
|---|---|---|---|---|
| `sma` | 简单移动平均 | `n`（20）、`source` | sma | 叠加 |
| `ema` | 指数移动平均 | `n`（20）、`source` | ema | 叠加 |
| `wma` | 加权移动平均 | `n`（20）、`source` | wma | 叠加 |
| `hma` | 赫尔移动平均 | `n`（20，至少为 2）、`source` | hma | 叠加 |
| `vwma` | 成交量加权移动平均 | `n`（20）、`source` | vwma | 叠加 |
| `dema` | 双重 EMA | `n`（20）、`source` | dema | 叠加 |
| `tema` | 三重 EMA | `n`（20）、`source` | tema | 叠加 |
| `volumeMa` | 成交量移动平均 | `n`（20） | vma | 成交量 |
| `boll` | 布林带 | `n`（20）、`k`（2）、`source` | upper、mid、lower；带状区域 upper–lower | 叠加 |
| `keltner` | 肯特纳通道 | `n`（20）、`m`（2）、`atrN`（10） | upper、mid、lower；带状区域 upper–lower | 叠加 |
| `donchian` | 唐奇安通道 | `n`（20） | upper、mid、lower | 叠加 |
| `ichimoku` | 一目均衡表 | `tenkan`（9）、`kijun`（26）、`senkou`（52）、`shift`（26） | tenkan、kijun、span A、span B、chikou；带状区域 span A–span B | 叠加 |
| `sar` | 抛物线 SAR | `afStart`（0.02）、`afStep`（0.02）、`afMax`（0.2） | sar（points） | 叠加 |
| `supertrend` | SuperTrend | `atrN`（10）、`m`（3） | long、short | 叠加 |
| `vwap` | 成交量加权平均价 | `anchor`（`'day'`）、`utcOffsetMs`（0） | vwap | 叠加 |
| `macd` | MACD | `fast`（12）、`slow`（26）、`signal`（9）、`source` | macd、signal、hist（histogram） | 新窗格 |
| `rsi` | 相对强弱指数 | `n`（14）、`source` | rsi | 新窗格 |
| `stoch` | 随机指标 | `k`（14）、`smoothK`（3）、`d`（3） | %K、%D | 新窗格 |
| `stochRsi` | 随机 RSI | `rsiN`（14）、`stochN`（14）、`smoothK`（3）、`d`（3）、`source` | %K、%D | 新窗格 |
| `cci` | 顺势指标 | `n`（20） | cci | 新窗格 |
| `willr` | 威廉指标 %R | `n`（14） | %R | 新窗格 |
| `mfi` | 资金流量指数 | `n`（14） | mfi | 新窗格 |
| `adx` | ADX 及方向指标 | `n`（14） | adx、+DI、−DI | 新窗格 |
| `atr` | 平均真实波幅 | `n`（14） | atr | 新窗格 |
| `obv` | 能量潮 | 无 | obv | 新窗格 |
| `cmf` | 蔡金资金流 | `n`（20） | cmf | 新窗格 |
| `roc` | 变动率（%） | `n`（10）、`source` | roc | 新窗格 |
| `momentum` | 动量 | `n`（10）、`source` | momentum | 新窗格 |
| `trix` | TRIX | `n`（15）、`source` | trix | 新窗格 |
| `aroon` | 阿隆指标 | `n`（14） | up、down | 新窗格 |

### 参数取值

| 参数 | 取值 |
|---|---|
| 周期（`n`、`fast`、`slow`、`signal`、`stoch` 的 `k`、`smoothK`、`d`、`rsiN`、`stochN`、`atrN`、`tenkan`、`kijun`、`senkou`、`shift`） | 整数，至少为 1（`hma`：`n` 至少为 2） |
| 乘数（`boll` 的 `k`、`m`） | 大于 0 的有限数 |
| `afStart`、`afStep`、`afMax` | 大于 0 的有限数，且 `afStart` 不大于 `afMax` |
| `source` | `'close'`（默认）、`'open'`、`'high'`、`'low'`、`'hl2'` (H+L)/2、`'hlc3'` (H+L+C)/3、`'ohlc4'` (O+H+L+C)/4 |
| `anchor`（`vwap`） | `'day'`（默认）、`'week'`（每周从周一开始）、`'month'`，或作为累计起点的 UTC 毫秒时间 |
| `utcOffsetMs`（`vwap`） | 在划分日、周、月之前加到 UTC 上的整毫秒数，例如纽约标准时间为 `-5 * 3600_000` |

```ts
chart.addIndicator(candles, 'ema', { n: 21, source: 'hlc3' });
chart.addIndicator(candles, 'vwap', { anchor: 'week' });
chart.addIndicator(candles, 'vwap', { anchor: Date.UTC(2024, 2, 1) }); // anchored VWAP
chart.addIndicator(candles, 'vwap', { utcOffsetMs: 9 * 3600_000 });   // sessions in UTC+9
```

参数名必须与表中的写法完全一致。指标不使用的键（包括 `atr_n` 这样的 snake_case 写法）会抛出一个列出可接受键的错误：

```ts
chart.addIndicator(candles, 'sma', { length: 20 });
// Error: unknown parameter 'length' for indicator 'sma' (expected: n, source)
```

`addTransformSeries` 以同样的方式检查参数。

### 公式

简要说明；`P` 为所选的 `source` 价格。"预热期"指输出为 `NaN` 的前导 K 线数量。

**均线与通道**

- **SMA**：`P` 最近 `n` 个值的均值。预热期 `n − 1`。
- **EMA**：`e = e + α(P − e)`，`α = 2/(n + 1)`，以前 `n` 个值的 SMA 作为初值。预热期 `n − 1`。
- **WMA**：从最新值起权重依次为 `n, n−1, …, 1`。预热期 `n − 1`。
- **HMA**：`WMA(2·WMA(P, n/2) − WMA(P, n), √n)`（二分和开方均向下取整）。预热期 `n + ⌊√n⌋ − 2`。
- **VWMA**：`n` 根 K 线上的 `Σ(P·V) / ΣV`（成交量之和为 0 时为 `NaN`）。
  预热期 `n − 1`。
- **DEMA**：`2·e1 − e2`，其中 `e1 = EMA(P, n)`，`e2 = EMA(e1, n)`。预热期
  `2(n − 1)`。
- **TEMA**：`3·e1 − 3·e2 + e3`，`e3 = EMA(e2, n)`。预热期 `3(n − 1)`。
- **成交量均线**：成交量的 SMA。预热期 `n − 1`。
- **BOLL**：中轨 = `SMA(P, n)`，上轨和下轨 = 中轨 ± `k` × `n` 根 K 线的总体标准差。预热期 `n − 1`。
- **Keltner**：中轨 = `EMA(close, n)`，上轨和下轨 = 中轨 ± `m` ×
  `ATR(atrN)`。预热期 `max(n − 1, atrN)`。
- **Donchian**：上轨 = `n` 根 K 线的最高价，下轨 = 最低价，中轨 = 两者的平均值。预热期 `n − 1`。
- **Ichimoku**：转换线（tenkan）和基准线（kijun）分别是 `tenkan` 和 `kijun` 根 K 线的高低区间中点。先行带 A = (tenkan + kijun)/2，先行带 B = `senkou` 根 K 线的区间中点，两者都向前偏移 `shift` 根 K 线绘制（最后 `shift` 个值出现在最后一根 K 线右侧）。迟行线（chikou）是向后偏移 `shift` 根 K 线绘制的收盘价。
- **SAR**：抛物线 SAR，加速因子从 `afStart` 开始，以 `afStep` 为步长，最大到 `afMax`，计算方式与 TA-Lib 相同。预热期 1。
- **SuperTrend**：通道 `HL2 ± m·ATR(atrN)`，在趋势持续期间只收紧不放宽；收盘价穿越当前有效通道时趋势翻转。`long`（下轨）在上升趋势中有值，`short`（上轨）在下降趋势中有值，其余为 `NaN`。预热期 `atrN`。
- **VWAP**：从锚点起计算 `Σ(HLC3·V) / ΣV`，在每个日、周或月（按 UTC 加 `utcOffsetMs`）重新开始，或从给定时间起计算（之前为 `NaN`）。预热期 0。

**震荡指标**

- **MACD**：macd = `EMA(P, fast) − EMA(P, slow)`，signal = `EMA(macd, signal)`，
  hist = macd − signal。macd 预热期 `slow − 1`，signal 和 hist 为 `slow + signal − 2`。
- **RSI**：对涨幅和跌幅做 Wilder 平滑（`r = r + (x − r)/n`）；
  `100·gain/(gain + loss)`，两者都为 0 时取 50。预热期 `n`。
- **Stoch**：原始值 = `k` 根 K 线上的 `100·(C − lowest low)/(highest high − lowest low)`（区间为 0 时取 50）；%K = `SMA(raw, smoothK)`，%D = `SMA(%K, d)`。
  %K 预热期 `k + smoothK − 2`，%D 再多 `d − 1`。
- **StochRSI**：在 `stochN` 根 K 线上对 `RSI(P, rsiN)` 套用随机指标公式，再像 Stoch 一样平滑。%K 预热期 `rsiN + stochN + smoothK − 2`，%D 再多 `d − 1`。
- **CCI**：`(TP − SMA(TP, n)) / (0.015 · mean deviation)`，`TP` = HLC3；平均偏差为 0 时取 0。预热期 `n − 1`。
- **Williams %R**：`n` 根 K 线上的 `−100·(highest high − C)/(highest high − lowest low)`；区间为 0 时取 −50。预热期 `n − 1`。
- **MFI**：`n` 根 K 线上的 `100·positive flow/(positive + negative flow)`，其中资金流 = HLC3 × 成交量，正负号取决于 HLC3 的变化方向；两者都为 0 时取 50。预热期 `n`。
- **ADX**：+DI 和 −DI 由经 Wilder 平滑的方向变动和真实波幅得出；adx = `100·|+DI − −DI|/(+DI + −DI)` 的 Wilder 平均。DI 预热期 `n`，adx 为 `2n − 1`。
- **ATR**：真实波幅的 Wilder 平均。预热期 `n`。
- **OBV**：成交量的累计值，收盘上涨时加上，收盘下跌时减去。预热期 0。
- **CMF**：`n` 根 K 线上的 `Σ(MFM·V)/ΣV`，`MFM = ((C − L) − (H − C))/(H − L)`。
  预热期 `n − 1`。
- **ROC**：`100·(P / P[n bars ago] − 1)`。预热期 `n`。
- **Momentum**：`P − P[n bars ago]`。预热期 `n`。
- **TRIX**：`EMA(EMA(EMA(P, n), n), n)` 相对上一根 K 线的百分比变化。
  预热期 `3(n − 1) + 1`。
- **Aroon**：在最近 `n + 1` 根 K 线上计算 `100·(n − bars since the highest high)/n`（up），最低价同理（down）。预热期 `n`。

## 设置输出样式

`indicator.outputs()` 返回输出系列。像任何系列一样用
`applyOptions` 设置样式（参见[系列](series.md)）：

```ts
const [macdLine, signalLine, hist] = macd.outputs();
macdLine.applyOptions({ color: '#2962ff', lineWidth: 2 });
signalLine.applyOptions({ color: '#ff9800', lineStyle: 'dashed' });
hist.applyOptions({ color: '#787b86' });

const [upper, mid, lower] = boll.outputs();
mid.applyOptions({ lineStyle: 'dotted' });
```

输出创建时线宽为 1 CSS px（散点为 3，即圆点直径），每个输出有固定的颜色，`priceLineVisible: false`，并显示最新值标签。每次调用 `outputs()` 都会返回指向同一系列的新句柄；请用 `id` 比较它们。

输出不能单独移除：对其调用 `series.remove()` 会抛出异常。请改为移除指标。

### 带状区域

BOLL 和 Keltner 会填充上轨与下轨之间的区域；Ichimoku 会填充先行带 A 与先行带 B 之间的云（A 在 B 上方时为绿色，下方时为红色）。`indicator.bands()` 返回这些填充的 id；用 `chart.setBandOptions` 修改它们的颜色：

```ts
const [band] = boll.bands();
chart.setBandOptions(band, { fill: 'rgba(41, 98, 255, 0.08)' });

const ichimoku = chart.addIndicator(candles, 'ichimoku');
const [cloud] = ichimoku.bands();
chart.setBandOptions(cloud, { fill: 'rgba(38, 166, 154, 0.25)', fillCrossed: 'rgba(239, 83, 80, 0.25)' });
console.log(chart.bandOptions(cloud)); // { fill, fillCrossed }
```

| 带状区域选项 | 含义 |
|---|---|
| `fill` | 上方输出位于下方输出之上或与之重合时 |
| `fillCrossed` | 位于其下方时 |

`null` 恢复默认值：对 BOLL 和 Keltner 而言，是上轨颜色加 15% 不透明度（`fillCrossed`：与 `fill` 相同）。传入 `'transparent'` 可隐藏填充。
其他指标没有带状区域；`bands()` 返回空数组。

## 移除

```ts
macd.remove();
```

`remove()` 会移除指标、它的输出系列及其价格线和标记、它的带状区域、基于其输出计算的所有指标，以及它创建的窗格（如果该窗格已经为空）。此后，对该对象调用 `outputs()`、`bands()` 和
`remove()` 都会抛出异常。

移除源系列会同时移除基于它的指标。

## 修改参数

参数不能原地修改。请移除指标后重新添加。要保留自定义样式，先把它们读出来：

```ts
import type { FerroChart, Indicator, Series, Placement } from '@ferrochart/core';

function replaceIndicator(
  chart: FerroChart,
  old: Indicator,
  source: Series,
  name: string,
  params: Record<string, number | string>,
  placement?: Placement,
): Indicator {
  // `shift` is set by the indicator (Ichimoku), so it is not copied.
  const styles = old.outputs().map((s) => {
    const { shift, ...style } = s.options();
    return style;
  });
  old.remove();
  const next = chart.addIndicator(source, name, params, placement);
  next.outputs().forEach((s, i) => {
    if (styles[i]) s.applyOptions(styles[i]);
  });
  return next;
}

let sma = chart.addIndicator(candles, 'sma', { n: 20 });
sma = replaceIndicator(chart, sma, candles, 'sma', { n: 50 });
```

移除震荡指标也会移除它创建的窗格（当窗格上没有其他内容时），因此替换后的指标会在底部得到一个新窗格。要让震荡指标保持在固定位置，请自己用 `chart.addPane()` 创建窗格，并把它的 id 作为放置位置传入：你自己创建的窗格不会随指标移除。

## 错误

在以下情况下，`addIndicator` 会抛出 `Error`，且不做任何改动：

| 问题 | 消息（示例） |
|---|---|
| 未知名称 | `unknown indicator 'smaa'` |
| 周期不是 ≥ 0 的整数 | `'n' must be a non-negative integer` |
| 周期为 0 | `invalid argument: periods must be at least 1` |
| `hma` 的 `n` 小于 2 | `invalid argument: the period is too short` |
| 乘数 ≤ 0 或不是有限数 | `invalid argument: multipliers must be finite and > 0` |
| `afStart` 大于 `afMax` | `invalid argument: af_start must not exceed af_max` |
| 应为数字处传入了非数字 | `'k' must be a finite number` |
| 未知的 `source` | `unknown source 'median'` |
| 无效的 VWAP `anchor` | `'anchor' must be 'day', 'week', 'month' or a time in ms` |
| `utcOffsetMs` 不是整数 | `'utcOffsetMs' must be an integer` |
| 无效的 `placement` | `placement must be 'default', 'overlay', 'newPane', 'volume' or a pane id` |
| 未知的窗格 id | `unknown id` |
| 以成交量系列作为源 | `invalid argument: volume series have no data of their own; use the source series` |
| 在没有成交量的源上使用成交量类指标 | `invalid argument: the indicator needs volume` |

```ts
try {
  chart.addIndicator(candles, 'rsi', { n: 0 });
} catch (e) {
  console.error((e as Error).message); // invalid argument: periods must be at least 1
}
```

## 实时更新

指标会自动跟随其源系列：

- 以最后一根 K 线的时间调用 `series.update(bar)` 会更新每个输出的最后一个值；时间更晚时则追加一个值。只计算新增或变化的那根 K 线，而不是整个系列。
- `series.setData(...)` 会在新数据上重新计算指标。

增量更新的结果与在整个系列上重新计算的结果相同。输出与源系列在同一帧中变化。

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];

feed.onTick((bar) => {
  candles.update(bar);
  const last = chart.dataRange();
  if (last) console.log('RSI', rsi.dataAt(last.to)?.value);
});
```

## 列出名称

```ts
import init, { indicatorNames } from '@ferrochart/core';

await init(); // not needed once a chart has been created
console.log(indicatorNames()); // ['sma', 'ema', ..., 'aroon']
```
