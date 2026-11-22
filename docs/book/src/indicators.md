# Indicators

FerroChart has 30 built-in technical indicators. An indicator reads a source
series, computes one or more output series, and keeps them up to date as the
source changes. The outputs are ordinary series: you style them, read their
values and put markers on them like any other series.

## Adding an indicator

```ts
const sma = chart.addIndicator(candles, 'sma', { n: 50 });
const boll = chart.addIndicator(candles, 'boll', { n: 20, k: 2 });
const macd = chart.addIndicator(candles, 'macd', { fast: 12, slow: 26, signal: 9 });
const rsi = chart.addIndicator(candles, 'rsi');        // all defaults
```

`chart.addIndicator(source, name, params?, placement?)` returns an
`Indicator`.

| Argument | Meaning |
|---|---|
| `source` | The series to compute from. Any series except volume and custom series. |
| `name` | One of the names in the table below, e.g. `'sma'`, `'stochRsi'`. `snake_case` spellings (`'stoch_rsi'`, `'volume_ma'`) are accepted too. |
| `params` | Parameters by name, in camelCase. Missing ones take their defaults. |
| `placement` | Where the outputs go; default `'default'`. |

The indicator is computed over all existing data at once, before
`addIndicator` returns.

### Placement

```ts
type Placement = 'default' | 'overlay' | 'newPane' | 'volume' | number;
```

| Value | Outputs go to |
|---|---|
| `'default'` | The indicator's usual place: `'overlay'` for moving averages and channels, `'newPane'` for oscillators, `'volume'` for Volume MA (see the table) |
| `'overlay'` | The source's pane, on its price scale |
| `'newPane'` | A new pane below the others, created for this indicator |
| `'volume'` | The pane of the source's volume series, on the volume scale. Without a volume series for the source, a new pane |
| a pane id | That pane (from `chart.panes()`, `chart.mainPane`, `chart.addPane()` or `series.pane`) |

```ts
// Two oscillators in one pane.
const rsi = chart.addIndicator(candles, 'rsi');
const pane = rsi.outputs()[0].pane;
chart.addIndicator(candles, 'stochRsi', {}, pane);
```

A pane created by `'newPane'` (or by `'volume'` without a volume series) is
removed with the indicator when nothing else is left on it.

### Source series

An OHLC series (candlestick, OHLC bar, Heikin-Ashi) gives the indicator its
open, high, low, close and volume. A single-value series (line, area,
baseline, histogram, points) gives `open = high = low = close = value` and no
volume. So you can compute an RSI of a line series, but not an OBV.

Indicators that need volume (VWMA, Volume MA, VWAP, MFI, OBV, CMF) throw when
the source is a single-value series, or an OHLC series whose data has no
volume column.

Bars where the source has no value, such as times that only another series
has, are skipped: the outputs are `NaN` there and the bar does not count in
the indicator's windows.

### Indicators of indicators

The source can be an output of another indicator. For example, a 9-bar SMA of
the RSI, in the RSI's pane:

```ts
const rsiLine = chart.addIndicator(candles, 'rsi').outputs()[0];
const rsiAvg = chart.addIndicator(rsiLine, 'sma', { n: 9 }, rsiLine.pane);
```

Removing the RSI removes the SMA too.

## The built-in indicators

`indicatorNames()` returns the 30 names in the order of this table.
Outputs are listed in the order `outputs()` returns them. All outputs are
lines unless marked (histogram) or (points).

| Name | Indicator | Parameters (defaults) | Outputs | Default placement |
|---|---|---|---|---|
| `sma` | Simple moving average | `n` (20), `source` | sma | overlay |
| `ema` | Exponential moving average | `n` (20), `source` | ema | overlay |
| `wma` | Weighted moving average | `n` (20), `source` | wma | overlay |
| `hma` | Hull moving average | `n` (20, at least 2), `source` | hma | overlay |
| `vwma` | Volume-weighted moving average | `n` (20), `source` | vwma | overlay |
| `dema` | Double EMA | `n` (20), `source` | dema | overlay |
| `tema` | Triple EMA | `n` (20), `source` | tema | overlay |
| `volumeMa` | Moving average of volume | `n` (20) | vma | volume |
| `boll` | Bollinger bands | `n` (20), `k` (2), `source` | upper, mid, lower; band upper–lower | overlay |
| `keltner` | Keltner channel | `n` (20), `m` (2), `atrN` (10) | upper, mid, lower; band upper–lower | overlay |
| `donchian` | Donchian channel | `n` (20) | upper, mid, lower | overlay |
| `ichimoku` | Ichimoku cloud | `tenkan` (9), `kijun` (26), `senkou` (52), `shift` (26) | tenkan, kijun, span A, span B, chikou; band span A–span B | overlay |
| `sar` | Parabolic SAR | `afStart` (0.02), `afStep` (0.02), `afMax` (0.2) | sar (points) | overlay |
| `supertrend` | SuperTrend | `atrN` (10), `m` (3) | long, short | overlay |
| `vwap` | Volume-weighted average price | `anchor` (`'day'`), `utcOffsetMs` (0) | vwap | overlay |
| `macd` | MACD | `fast` (12), `slow` (26), `signal` (9), `source` | macd, signal, hist (histogram) | new pane |
| `rsi` | Relative strength index | `n` (14), `source` | rsi | new pane |
| `stoch` | Stochastic oscillator | `k` (14), `smoothK` (3), `d` (3) | %K, %D | new pane |
| `stochRsi` | Stochastic RSI | `rsiN` (14), `stochN` (14), `smoothK` (3), `d` (3), `source` | %K, %D | new pane |
| `cci` | Commodity channel index | `n` (20) | cci | new pane |
| `willr` | Williams %R | `n` (14) | %R | new pane |
| `mfi` | Money flow index | `n` (14) | mfi | new pane |
| `adx` | ADX with directional indicators | `n` (14) | adx, +DI, −DI | new pane |
| `atr` | Average true range | `n` (14) | atr | new pane |
| `obv` | On-balance volume | none | obv | new pane |
| `cmf` | Chaikin money flow | `n` (20) | cmf | new pane |
| `roc` | Rate of change (%) | `n` (10), `source` | roc | new pane |
| `momentum` | Momentum | `n` (10), `source` | momentum | new pane |
| `trix` | TRIX | `n` (15), `source` | trix | new pane |
| `aroon` | Aroon | `n` (14) | up, down | new pane |

### Parameter values

| Parameter | Values |
|---|---|
| Periods (`n`, `fast`, `slow`, `signal`, `k` of `stoch`, `smoothK`, `d`, `rsiN`, `stochN`, `atrN`, `tenkan`, `kijun`, `senkou`, `shift`) | Whole numbers, at least 1 (`hma`: `n` at least 2) |
| Multipliers (`k` of `boll`, `m`) | Finite numbers greater than 0 |
| `afStart`, `afStep`, `afMax` | Finite numbers greater than 0, `afStart` not above `afMax` |
| `source` | `'close'` (default), `'open'`, `'high'`, `'low'`, `'hl2'` (H+L)/2, `'hlc3'` (H+L+C)/3, `'ohlc4'` (O+H+L+C)/4 |
| `anchor` (`vwap`) | `'day'` (default), `'week'` (weeks start on Monday), `'month'`, or a time in UTC ms to accumulate from |
| `utcOffsetMs` (`vwap`) | Whole milliseconds added to UTC before splitting days, weeks and months, e.g. `-5 * 3600_000` for New York standard time |

```ts
chart.addIndicator(candles, 'ema', { n: 21, source: 'hlc3' });
chart.addIndicator(candles, 'vwap', { anchor: 'week' });
chart.addIndicator(candles, 'vwap', { anchor: Date.UTC(2024, 2, 1) }); // anchored VWAP
chart.addIndicator(candles, 'vwap', { utcOffsetMs: 9 * 3600_000 });   // sessions in UTC+9
```

Parameter names must be written exactly as in the table. A key the indicator
does not use, including snake_case spellings such as `atr_n`, throws an error
that lists the accepted keys:

```ts
chart.addIndicator(candles, 'sma', { length: 20 });
// Error: unknown parameter 'length' for indicator 'sma' (expected: n, source)
```

`addTransformSeries` checks its parameters the same way.

### Formulas

In brief; `P` is the chosen `source` price. "Warm-up" is the number of leading
bars whose output is `NaN`.

**Moving averages and channels**

- **SMA**: mean of the last `n` values of `P`. Warm-up `n − 1`.
- **EMA**: `e = e + α(P − e)`, `α = 2/(n + 1)`, seeded with the SMA of the
  first `n` values. Warm-up `n − 1`.
- **WMA**: weights `n, n−1, …, 1` from the newest value. Warm-up `n − 1`.
- **HMA**: `WMA(2·WMA(P, n/2) − WMA(P, n), √n)` (halves and roots rounded
  down). Warm-up `n + ⌊√n⌋ − 2`.
- **VWMA**: `Σ(P·V) / ΣV` over `n` bars (`NaN` when the volume sum is 0).
  Warm-up `n − 1`.
- **DEMA**: `2·e1 − e2`, where `e1 = EMA(P, n)` and `e2 = EMA(e1, n)`. Warm-up
  `2(n − 1)`.
- **TEMA**: `3·e1 − 3·e2 + e3`, `e3 = EMA(e2, n)`. Warm-up `3(n − 1)`.
- **Volume MA**: SMA of the volume. Warm-up `n − 1`.
- **BOLL**: mid = `SMA(P, n)`, upper and lower = mid ± `k` × the population
  standard deviation over `n` bars. Warm-up `n − 1`.
- **Keltner**: mid = `EMA(close, n)`, upper and lower = mid ± `m` ×
  `ATR(atrN)`. Warm-up `max(n − 1, atrN)`.
- **Donchian**: upper = highest high of `n` bars, lower = lowest low, mid = their
  average. Warm-up `n − 1`.
- **Ichimoku**: tenkan and kijun are the midpoints of the high-low range over
  `tenkan` and `kijun` bars. Span A = (tenkan + kijun)/2 and span B = the
  midpoint over `senkou` bars, both drawn `shift` bars ahead (the last `shift`
  values appear right of the last bar). Chikou is the close drawn `shift` bars
  back.
- **SAR**: Parabolic SAR with acceleration from `afStart` in steps of `afStep`
  up to `afMax`, computed as TA-Lib does. Warm-up 1.
- **SuperTrend**: bands `HL2 ± m·ATR(atrN)` that only tighten while the trend
  lasts; the trend flips when the close crosses the active band. `long` (the
  lower band) has values in up trends, `short` (the upper band) in down trends,
  `NaN` otherwise. Warm-up `atrN`.
- **VWAP**: `Σ(HLC3·V) / ΣV` from the anchor on, restarting at each day, week
  or month (in UTC plus `utcOffsetMs`), or from the given time on (`NaN`
  before it). Warm-up 0.

**Oscillators**

- **MACD**: macd = `EMA(P, fast) − EMA(P, slow)`, signal = `EMA(macd, signal)`,
  hist = macd − signal. Warm-up `slow − 1` for macd, `slow + signal − 2` for
  signal and hist.
- **RSI**: Wilder's smoothing (`r = r + (x − r)/n`) of gains and losses;
  `100·gain/(gain + loss)`, 50 when both are 0. Warm-up `n`.
- **Stoch**: raw = `100·(C − lowest low)/(highest high − lowest low)` over `k`
  bars (50 when the range is 0); %K = `SMA(raw, smoothK)`, %D = `SMA(%K, d)`.
  Warm-up `k + smoothK − 2` for %K, `d − 1` more for %D.
- **StochRSI**: the stochastic formula applied to `RSI(P, rsiN)` over `stochN`
  bars, then smoothed like Stoch. Warm-up `rsiN + stochN + smoothK − 2` for
  %K, `d − 1` more for %D.
- **CCI**: `(TP − SMA(TP, n)) / (0.015 · mean deviation)`, `TP` = HLC3; 0 when
  the deviation is 0. Warm-up `n − 1`.
- **Williams %R**: `−100·(highest high − C)/(highest high − lowest low)` over
  `n` bars; −50 when the range is 0. Warm-up `n − 1`.
- **MFI**: `100·positive flow/(positive + negative flow)` over `n` bars, where
  flow = HLC3 × volume and its sign follows the change of HLC3; 50 when both
  are 0. Warm-up `n`.
- **ADX**: +DI and −DI from Wilder-smoothed directional movement and true
  range; adx = Wilder average of `100·|+DI − −DI|/(+DI + −DI)`. Warm-up `n` for
  the DIs, `2n − 1` for adx.
- **ATR**: Wilder average of the true range. Warm-up `n`.
- **OBV**: running total of volume, added on up closes and subtracted on down
  closes. Warm-up 0.
- **CMF**: `Σ(MFM·V)/ΣV` over `n` bars, `MFM = ((C − L) − (H − C))/(H − L)`.
  Warm-up `n − 1`.
- **ROC**: `100·(P / P[n bars ago] − 1)`. Warm-up `n`.
- **Momentum**: `P − P[n bars ago]`. Warm-up `n`.
- **TRIX**: percent change of `EMA(EMA(EMA(P, n), n), n)` from the previous bar.
  Warm-up `3(n − 1) + 1`.
- **Aroon**: `100·(n − bars since the highest high)/n` (up) and the same for the
  lowest low (down), over the last `n + 1` bars. Warm-up `n`.

## Styling outputs

`indicator.outputs()` returns the output series. Style them with
`applyOptions`, as any series (see [Series](series.md)):

```ts
const [macdLine, signalLine, hist] = macd.outputs();
macdLine.applyOptions({ color: '#2962ff', lineWidth: 2 });
signalLine.applyOptions({ color: '#ff9800', lineStyle: 'dashed' });
hist.applyOptions({ color: '#787b86' });

const [upper, mid, lower] = boll.outputs();
mid.applyOptions({ lineStyle: 'dotted' });
```

Outputs are created with a line width of 1 CSS px (points: 3, the dot
diameter), a fixed color per output, `priceLineVisible: false` and the last
value label shown. Each `outputs()` call returns new handles to the same
series; compare them by `id`.

An output cannot be removed on its own: `series.remove()` on it throws.
Remove the indicator instead.

### Bands

BOLL and Keltner fill the area between their upper and lower lines; Ichimoku
fills the cloud between span A and span B (green where A is above B, red
below). `indicator.bands()` returns the ids of these fills; change their
colors with `chart.setBandOptions`:

```ts
const [band] = boll.bands();
chart.setBandOptions(band, { fill: 'rgba(41, 98, 255, 0.08)' });

const ichimoku = chart.addIndicator(candles, 'ichimoku');
const [cloud] = ichimoku.bands();
chart.setBandOptions(cloud, { fill: 'rgba(38, 166, 154, 0.25)', fillCrossed: 'rgba(239, 83, 80, 0.25)' });
console.log(chart.bandOptions(cloud)); // { fill, fillCrossed }
```

| Band option | Meaning |
|---|---|
| `fill` | Where the upper output is at or above the lower one |
| `fillCrossed` | Where it is below |

`null` restores the default: for BOLL and Keltner, the upper line's color at
15% opacity (`fillCrossed`: as `fill`). Pass `'transparent'` to hide a fill.
Other indicators have no bands; `bands()` returns an empty array.

## Removing

```ts
macd.remove();
```

`remove()` takes off the indicator, its output series, their price lines and
markers, its bands, any indicators computed from its outputs, and the pane it
created if that pane is now empty. Afterwards, `outputs()`, `bands()` and
`remove()` on the object throw.

Removing the source series removes its indicators.

## Changing parameters

Parameters cannot be changed in place. Remove the indicator and add it again.
To keep custom styles, read them first:

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

Removing an oscillator also removes the pane it created (when nothing else is
on it), so the replacement gets a new pane at the bottom. To keep an
oscillator in a fixed place, create the pane yourself with `chart.addPane()`
and pass its id as the placement: a pane you created is not removed with the
indicator.

## Errors

`addIndicator` throws an `Error` and changes nothing when:

| Problem | Message (example) |
|---|---|
| Unknown name | `unknown indicator 'smaa'` |
| A period that is not a whole number ≥ 0 | `'n' must be a non-negative integer` |
| A period of 0 | `invalid argument: periods must be at least 1` |
| `hma` with `n` below 2 | `invalid argument: the period is too short` |
| A multiplier ≤ 0 or not finite | `invalid argument: multipliers must be finite and > 0` |
| `afStart` above `afMax` | `invalid argument: af_start must not exceed af_max` |
| A non-number where a number is expected | `'k' must be a finite number` |
| Unknown `source` | `unknown source 'median'` |
| Invalid VWAP `anchor` | `'anchor' must be 'day', 'week', 'month' or a time in ms` |
| `utcOffsetMs` not a whole number | `'utcOffsetMs' must be an integer` |
| Invalid `placement` | `placement must be 'default', 'overlay', 'newPane', 'volume' or a pane id` |
| Unknown pane id | `unknown id` |
| A volume series as source | `invalid argument: volume series have no data of their own; use the source series` |
| A volume indicator on a source without volume | `invalid argument: the indicator needs volume` |

```ts
try {
  chart.addIndicator(candles, 'rsi', { n: 0 });
} catch (e) {
  console.error((e as Error).message); // invalid argument: periods must be at least 1
}
```

## Live updates

Indicators follow their source by themselves:

- `series.update(bar)` with the time of the last bar updates the last value of
  every output; with a later time it appends one. Only the new or changed bar
  is computed, not the whole series.
- `series.setData(...)` recomputes the indicators over the new data.

The results of incremental updates are the same as computing over the whole
series again. The outputs change in the same frame as the source.

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];

feed.onTick((bar) => {
  candles.update(bar);
  const last = chart.dataRange();
  if (last) console.log('RSI', rsi.dataAt(last.to)?.value);
});
```

## Listing the names

```ts
import init, { indicatorNames } from '@ferrochart/core';

await init(); // not needed once a chart has been created
console.log(indicatorNames()); // ['sma', 'ema', ..., 'aroon']
```
