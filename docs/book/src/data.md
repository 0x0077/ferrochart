# Data

This chapter covers getting bars and values into a series: the data formats,
the validation rules, real-time updates, loading older history and reading
data back.

## Columns and rows

`setData` replaces all data of a series. It accepts two shapes: an object of
columns (one array per field) or an array of row objects.

### Columns

OHLC series (candlestick, OHLC bars, Heikin-Ashi and transforms) take
`OhlcData`:

| Column | Type | Notes |
|---|---|---|
| `time` | `BigInt64Array`, `Float64Array` or `number[]` | Bar open time, UTC milliseconds, strictly increasing |
| `open`, `high`, `low`, `close` | `Float64Array` or `number[]` | |
| `volume` | `Float64Array` or `number[]` | Optional |

Value series (line, area, baseline, histogram, points) take `ValueData`:

| Column | Type | Notes |
|---|---|---|
| `time` | `BigInt64Array`, `Float64Array` or `number[]` | UTC milliseconds, strictly increasing |
| `value` | `Float64Array` or `number[]` | `NaN` is a gap |
| `color` | `(string \| null)[]` or `Uint32Array` | Optional per-point colors, drawn by histograms only (see [Series types](series.md#histogram)) |

All columns must have the same length.

```ts
candles.setData({
  time: new BigInt64Array([1704067200000n, 1704067260000n, 1704067320000n]),
  open: new Float64Array([100.0, 100.4, 100.1]),
  high: new Float64Array([100.6, 100.5, 100.9]),
  low: new Float64Array([99.8, 99.9, 100.0]),
  close: new Float64Array([100.4, 100.1, 100.8]),
  volume: new Float64Array([1200, 900, 1500]),
});

line.setData({ time: [1704067200000, 1704067260000], value: [100.4, 100.1] });
```

### Rows

An array of objects is also accepted: `OhlcRow` (`{ time, open, high, low,
close, volume? }`) for OHLC series and `ValueRow` (`{ time, value?, color? }`)
for value series. In rows, `time` can also be a `Date`.

```ts
candles.setData([
  { time: new Date('2024-01-01T00:00:00Z'), open: 100.0, high: 100.6, low: 99.8, close: 100.4 },
  { time: new Date('2024-01-01T00:01:00Z'), open: 100.4, high: 100.5, low: 99.9, close: 100.1 },
]);

line.setData([
  { time: 1704067200000, value: 100.4 },
  { time: 1704067260000 },                 // no value: a gap
  { time: 1704067320000, value: 100.8 },
]);
```

A row without `value` (or with `NaN`) is a gap. A missing OHLC field throws an
error that names it (`data[3] has no 'open'`). If some rows have `volume` and
others not, the missing volumes are 0.

### Which to use

Rows are converted to columns before anything else happens, so both give the
same result. Use rows when your data already comes as objects (a JSON API, a
few hundred bars) and the code is simpler that way. Use columns for large data:
typed arrays are copied into the engine with one `memcpy` per column, with no
per-bar object to read, so `setData` with a million bars takes a few tens of
milliseconds. If you receive binary data (for example from a WebSocket or a
file), build typed arrays directly and skip objects altogether.

### Times

Times are UTC milliseconds since the Unix epoch, as whole numbers.

- `BigInt64Array` times are used as they are.
- `Float64Array` and `number[]` times are converted to integers; a value that is
  not finite, has a fractional part, or is beyond ±9·10¹⁵ throws
  (`time[5] is not an integer millisecond timestamp`).
- In rows and in `update`, `time` can be a number, a bigint or a `Date`.

If your source uses seconds, multiply by 1000.

The chart shows times in UTC.

## Validation

`setData` and `update` check the data first. If anything is wrong they throw,
and the series keeps its previous data.

| Rule | Error message |
|---|---|
| Times strictly increasing (no duplicates) | `time is not strictly increasing at index 7` |
| All columns as long as `time` | ``column `open` has a different length than `time` `` |
| Prices and volume finite, except gap bars | `invalid OHLC values in bar 12` |
| `high` ≥ max(`open`, `close`) and `low` ≤ min(`open`, `close`) | `invalid OHLC values in bar 12` |
| Columns are arrays of numbers / typed arrays | `column 'high' must be a Float64Array or an array of numbers` |

The index in the message is the position in the data you passed.

A bar whose four prices are all `NaN` is a gap bar (see [Gaps](#gaps)); it is
valid, and its volume is set to 0.

## Gaps

- In a value series, a `NaN` value (or a row without `value`) is a gap. Lines,
  areas and baselines are interrupted there; histograms and points draw
  nothing for it.
- In an OHLC series, a bar with `NaN` for open, high, low and close is a gap
  and draws nothing.

`dataAt` returns `null` for a gap.

### Several series on one time axis

All series of a chart share one time axis: logical index 42 is the same time
in every series. When series have different times, the axis is the union of
their times. A series has no point at a time only the others have: OHLC
series, histograms and points show nothing there; lines, areas and baselines
connect the neighboring points. If new times fall between existing bars of the
axis, the axis is rebuilt and the chart sends a `warning` event.

## Bar quality flags

Use `setBarFlags` on candlestick, OHLC bar or Heikin-Ashi series to describe
data your application has filled in or clipped. Flags do not change the
prices; pass the prepared OHLC data first:

```ts
const start = Date.UTC(2024, 0, 1);
candles.setData([
  { time: start, open: 100, high: 102, low: 99, close: 101 },
  { time: start + 60_000, open: 101, high: 101, low: 101, close: 101 },
  { time: start + 120_000, open: 101, high: 104, low: 100, close: 103 },
]);
candles.applyOptions({ filledBarColor: '#78909c' });
candles.setBarFlags([
  { time: start + 60_000, filled: true },
  { time: start + 120_000, clipped: true },
]);
// Clear all quality flags when they are no longer needed:
// candles.setBarFlags([]);
```

- `filled` marks a bar your source synthesized without real trades. It is
  drawn hollow in `filledBarColor` (by default the series' main color at
  40% opacity). A truly missing bar can instead be a [gap](#gaps); this flag
  does not fill missing data for you.
- `clipped` marks a bar whose outlier wick you clipped. A tick appears at
  each end of its wick. Keep the raw high/low values in your application,
  keyed by time, and show them in your tooltip; the chart only has the
  clipped values you supplied. Both flags can be set on one bar.
- Each call replaces all flags; `[]` clears them. Times are UTC milliseconds
  (or `Date`/`bigint`, as for data); for duplicate times the last entry wins.
  When zooming out merges bars, the merged bar is filled only if all its
  bars are filled, and clipped if any is clipped.

Python uses `candles.set_bar_flags(times, filled=..., clipped=...)`, with
boolean arrays of the same length as `times`, and
`candles.apply_options(filled_bar_color="#78909c")`. See
[Python](python.md#options-indicators-and-drawings) and
[Series options](series.md#series-options).

## Real-time updates

`update` changes the last bar or adds a new one:

- Same time as the last bar: replaces it (the forming bar).
- Later time: appends a new bar.
- Earlier time: throws.

```ts
// OHLC series
candles.update({ time: 1704067380000, open: 100.8, high: 101.2, low: 100.7, close: 101.0, volume: 300 });
// Value series
line.update({ time: 1704067380000, value: 101.0 });
```

The bar is validated like `setData` data. Replacing the last bar does not
allocate memory, and the chart redraws at most once per animation frame, so
calling `update` on every trade is fine. Indicators on the series are updated
incrementally. A volume series follows its source; it has no `update` of its
own.

For a transform series (Renko, Kagi and so on), each `update` is an input bar
or tick and the drawn elements change accordingly; see
[Bar transforms](series.md#bar-transforms).

### Appending a batch

To add several bars at the end (a batch from your feed after a reconnect,
say), call `append` with columns or rows, in any form `setData` takes. Times
must come after the last bar, and the volume column must be present exactly
when the series has one; otherwise it throws and the data stays as it was.

```ts
candles.append([
  { time: 1704067440000, open: 101.0, high: 101.4, low: 100.9, close: 101.3, volume: 250 },
  { time: 1704067500000, open: 101.3, high: 101.5, low: 101.1, close: 101.2, volume: 180 },
]);
```

Use `update` for the forming bar, `append` for finished bars that arrive
together.

## Loading older history

`prepend` inserts older bars before the first one, in any form `setData`
takes. The bars already there keep their logical indices; the new ones get
negative indices (the bar before index 0 is -1). The view does not move, and
indicators on the series are recomputed. Times must come before the first bar;
the volume rule is the same as for `append`.

The chart tells you when to load: the `needMoreHistory` event is sent when
fewer bars than one screen width are left before the left edge of the view.
`barsBeforeStart` is the number of bars left (negative when empty space before
the data is already visible). It is sent once per first bar: after a
`prepend` the first bar is a new one, so the event comes again when the user
scrolls close to it.

### Worked example

```ts
import { FerroChart, type OhlcRow } from '@ferrochart/core';

// Your backend: up to `count` bars that end before `beforeMs`, oldest first.
declare function fetchBars(symbol: string, beforeMs: number, count: number): Promise<OhlcRow[]>;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const latest = await fetchBars('BTCUSD', Date.now(), 1000);
candles.setData(latest);
let first = Number(latest[0]?.time ?? Date.now());
let loading = false;
let exhausted = false;

chart.on('needMoreHistory', async () => {
  if (loading || exhausted) return;
  loading = true;
  try {
    // Only bars before the first one: an overlapping bar would make prepend throw.
    const older = (await fetchBars('BTCUSD', first, 1000)).filter((b) => Number(b.time) < first);
    if (older.length === 0) {
      exhausted = true; // the backend has nothing older
      return;
    }
    candles.prepend(older);
    first = Number(older[0].time);
  } finally {
    loading = false;
  }
});
```

The `loading` flag guards against overlapping requests from your own code
(the event itself does not repeat for the same first bar). Live bars keep
going through `update` meanwhile: `prepend` and `update` work on opposite
ends.

## Reading data back

| Method | Returns |
|---|---|
| `series.dataAt(index)` | `{ open, high, low, close, volume }` for OHLC series (`volume` is `null` without a volume column), `{ value }` for value series; `null` for a gap or outside the data |
| `chart.timeAtIndex(index)` | Time (UTC ms) of the bar at a logical index, `null` outside the data |
| `chart.indexOfTime(time)` | Logical index of the bar with exactly that time, `null` if there is none |
| `chart.dataRange()` | `{ from, to }`: logical indices of the first and last bar; `null` without data |

Fractional indices are rounded to the nearest bar.

```ts
const range = chart.dataRange();
if (range) {
  const last = candles.dataAt(range.to);              // the forming bar
  const t = chart.timeAtIndex(range.to);              // its time
  console.log(t, last);
}

const i = chart.indexOfTime(Date.UTC(2024, 0, 2, 9, 30));
if (i !== null) console.log(candles.dataAt(i));
```

For a series with a `shift`, `dataAt(index)` returns the point drawn at
`index`.

## Logical indices

Every bar has a logical index. Visible ranges, crosshair positions and clicks
are reported in these indices, and they are what `setVisibleLogicalRange` and
`logicalToCoordinate` take; see [Time scale and viewport](time-scale.md). All
series share the time axis, so index `i` is the same time in every series.

The first bar has index 0 after `setData`. Indices are fractional where
positions between bars matter (the edges of the view, the pointer): bar `i`
covers `i - 0.5` to `i + 0.5`.

## Level of detail

Bars are stored as columns: one array of times, one of opens, and so on. This
is what keeps loading fast and memory small: about 58 bytes per bar including
the time index and the level-of-detail pyramid.

When you zoom out until several bars share a pixel, FerroChart draws aggregated
bars from a precomputed pyramid (each level halves the count, keeping the true
high and low). The number of things drawn per frame is therefore bounded by
the screen width — about four per physical pixel — whatever the data size.

All data changes are incremental internally: the renderer is told which range
changed and uploads only that.

## Volume

A volume series reads the volume column of an OHLC series:
`chart.addVolumeSeries(candles)`. It has no data of its own; pass `volume` to
the candles' `setData` and `update`. See [Series types](series.md#volume).

## Example: a simulated live feed

A chart with 500 bars of history and a new price every 250 ms. A new
one-minute bar starts whenever the clock passes a minute boundary.

```ts
import { FerroChart, randomWalk, type Bar } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const interval = 60_000;
const now = Date.now();
const start = Math.floor(now / interval) * interval - 500 * interval;
const history = randomWalk(42, 500, start, interval); // deterministic demo data
candles.setData(history);

const n = history.close.length;
let bar: Bar = {
  time: Number(history.time[n - 1]),
  open: history.open[n - 1],
  high: history.high[n - 1],
  low: history.low[n - 1],
  close: history.close[n - 1],
  volume: history.volume?.[n - 1] ?? 0,
};

const timer = setInterval(() => {
  const t = Math.floor(Date.now() / interval) * interval;
  const price = bar.close * (1 + (Math.random() - 0.5) * 0.002);
  const size = Math.round(Math.random() * 10);
  if (t > Number(bar.time)) {
    // A new minute: open a new bar at the last close.
    bar = { time: t, open: bar.close, high: Math.max(bar.close, price), low: Math.min(bar.close, price), close: price, volume: size };
  } else {
    bar = { ...bar, high: Math.max(bar.high, price), low: Math.min(bar.low, price), close: price, volume: (bar.volume ?? 0) + size };
  }
  candles.update(bar); // same time: replaces, later time: appends
}, 250);

// When the chart goes away:
// clearInterval(timer); chart.destroy();
```

The view keeps its distance from the latest bar (the right offset), so each
appended bar scrolls the content one bar to the left, whether the user is
looking at the latest data or has scrolled back. `chart.scrollToRealtime()`
brings the latest bar back into view (see
[Time scale and viewport](time-scale.md#right-offset)).
