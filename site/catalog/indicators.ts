import { bars, candles, create, load, view, withIndicators } from './build';
import type { Category } from './types';

/** Forecast quantile paths joined to the last bar: the path starts at the last close. */
const FORECAST = (quantiles: string) => `${bars({ symbol: 'BTCUSD', interval: '1h', count: 400 })}
${load('fc', 'forecast', { symbol: 'BTCUSD', interval: '1h', ahead: 72, quantiles })}
// The paths start at the last price: join them to the last bar.
const time = [bars.time[bars.time.length - 1], ...fc.time.slice(1)];
const path = (q, color, o = {}) => {
  const s = chart.addSeries('line', { color, lineWidth: 1, lastValueVisible: false, priceLineVisible: false, ...o });
  s.setData({ time, value: fc[q] });
  return s;
};`;

export const indicators: Category = {
  id: 'indicators',
  title: 'Technical analysis',
  kicker: '03',
  blurb: '30 incremental indicators in panes or on price, checked against reference data, plus patterns, events and projections.',
  accent: '#ffb547',
  demos: [
    {
      id: 'macd-rsi',
      title: 'Bollinger · MACD · RSI',
      group: 'Panes & oscillators',
      code: withIndicators({ symbol: 'BTCUSD', interval: '5m', count: 3000, spacing: 7 }, [['boll'], ['macd', {}, 'newPane'], ['rsi', {}, 'newPane']]),
      about: 'Each indicator keeps its own state and updates with the last bar in O(1). Panes resize by dragging their separators.',
      api: ['addIndicator', "placement 'newPane'"],
    },
    {
      id: 'oscillators',
      title: 'Oscillators',
      group: 'Panes & oscillators',
      code: withIndicators({ symbol: 'NQ', interval: '5m', count: 2500, spacing: 7 }, [['stoch', {}, 'newPane'], ['adx', {}, 'newPane'], ['cci', {}, 'newPane']]),
      about: 'Stochastic, ADX and CCI in their own panes, sharing the time axis and the crosshair.',
      api: ['addIndicator', 'indicatorNames()'],
    },
    {
      id: 'ichimoku',
      title: 'Ichimoku cloud',
      group: 'Overlays',
      code: withIndicators({ symbol: 'ETHUSD', interval: '1h', count: 2000, spacing: 7 }, [['ichimoku']]),
      about: 'Five lines and the cloud between them, displaced forward in time. The cloud is a band fill drawn by the core, identical on every backend.',
      api: ["addIndicator('ichimoku')"],
    },
    {
      id: 'trend',
      title: 'Supertrend · SAR',
      group: 'Overlays',
      code: withIndicators({ symbol: 'SOLUSD', interval: '15m', count: 2000, spacing: 8 }, [['supertrend'], ['sar']]),
      about: 'Trend-following stops on the price pane. Parabolic SAR is drawn as dots with the points series.',
      api: ["addIndicator('supertrend')", "addIndicator('sar')"],
    },
    {
      id: 'supertrend',
      title: 'Supertrend signals',
      group: 'Overlays',
      code: candles({
        symbol: 'ETHUSD',
        interval: '1h',
        count: 1500,
        spacing: 8,
        extra: `const st = chart.addIndicator(candles, 'supertrend', { atrN: 10, m: 3 });
// Outputs: the long stop (NaN while short) and the short stop. A flip starts or ends the long line.
const [long] = st.outputs();
const markers = [];
for (let i = 12; i < bars.time.length; i++) {
  const isLong = long.dataAt(i) !== null, wasLong = long.dataAt(i - 1) !== null;
  if (isLong === wasLong) continue;
  markers.push(isLong
    ? { time: bars.time[i], position: 'belowBar', shape: 'arrowUp', color: '#2ee6b6', text: 'Buy' }
    : { time: bars.time[i], position: 'aboveBar', shape: 'arrowDown', color: '#ff5d73', text: 'Sell' });
}
candles.setMarkers(markers);`,
      }),
      about: 'The SuperTrend stop in green under longs and red over shorts, with a marker wherever it flips. The flips are read back from the indicator\'s output series with `dataAt`.',
      api: ["addIndicator('supertrend')", 'Indicator.outputs', 'setMarkers'],
    },
    {
      id: 'psar',
      title: 'Parabolic SAR + Keltner',
      group: 'Overlays',
      code: withIndicators({ symbol: 'XAUUSD', interval: '1h', count: 1500, spacing: 9 }, [['keltner', { n: 20, m: 2, atrN: 10 }], ['sar']]),
      about: 'A Keltner channel (EMA 20 ± 2 ATR) as the volatility envelope and Parabolic SAR dots as the trailing stop. Both are built-in indicators on the price pane.',
      api: ["addIndicator('keltner')", "addIndicator('sar')"],
    },
    {
      id: 'donchian',
      title: 'Donchian breakouts',
      group: 'Overlays',
      code: candles({
        symbol: 'BTCUSD',
        interval: '4h',
        count: 1200,
        spacing: 7,
        extra: `chart.addIndicator(candles, 'donchian', { n: 20 });
// A breakout: the close leaves the channel of the previous 20 bars.
const n = 20, markers = [];
for (let i = n; i < bars.time.length; i++) {
  const hi = Math.max(...bars.high.slice(i - n, i)), lo = Math.min(...bars.low.slice(i - n, i));
  const prev = bars.close[i - 1];
  if (bars.close[i] > hi && prev <= hi) markers.push({ time: bars.time[i], position: 'belowBar', shape: 'arrowUp', color: '#2ee6b6' });
  if (bars.close[i] < lo && prev >= lo) markers.push({ time: bars.time[i], position: 'aboveBar', shape: 'arrowDown', color: '#ff5d73' });
}
candles.setMarkers(markers);`,
      }),
      about: 'The 20-bar Donchian channel (highest high, lowest low and their midpoint), with arrows where the close first breaks out of the previous channel: the classic turtle entry.',
      api: ["addIndicator('donchian')", 'setMarkers'],
    },
    {
      id: 'patterns',
      title: 'Zigzag & swing labels',
      group: 'Price structure',
      code: candles({
        symbol: 'ETHUSD',
        interval: '1h',
        count: 1200,
        spacing: 7,
        extra: `// Zigzag: a new pivot after a 2.5% reversal from the running extreme.
const pct = 0.025, pivots = [];
let up = true, ext = 0; // the extreme of the current leg
for (let i = 1; i < bars.time.length; i++) {
  if (up) {
    if (bars.high[i] >= bars.high[ext]) ext = i;
    else if (bars.low[i] < bars.high[ext] * (1 - pct)) { pivots.push({ i: ext, high: true }); up = false; ext = i; }
  } else if (bars.low[i] <= bars.low[ext]) ext = i;
  else if (bars.high[i] > bars.low[ext] * (1 + pct)) { pivots.push({ i: ext, high: false }); up = true; ext = i; }
}
const price = (p) => (p.high ? bars.high[p.i] : bars.low[p.i]);
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, lastValueVisible: false, priceLineVisible: false })
  .setData({ time: pivots.map((p) => bars.time[p.i]), value: pivots.map(price) });
// Higher high, lower high, higher low, lower low against the previous pivot of the same kind.
candles.setMarkers(pivots.slice(2).map((p, k) => {
  const higher = price(p) > price(pivots[k]);
  const text = (higher ? 'H' : 'L') + (p.high ? 'H' : 'L');
  return { time: bars.time[p.i], position: p.high ? 'aboveBar' : 'belowBar', shape: 'circle', color: higher ? '#2ee6b6' : '#ff5d73', text };
}));`,
      }),
      about: 'A 2.5% zigzag drawn as a line series through the swing pivots, each pivot labelled HH, LH, HL or LL against the previous one of its kind: the market structure at a glance. Computed in a few lines of JS.',
      api: ["addSeries('line')", 'setMarkers'],
    },
    {
      id: 'pivots',
      title: 'Daily pivot points',
      group: 'Price structure',
      code: `${create()}
${bars({ symbol: 'ES', interval: '5m', count: 780 })} // ten sessions
chart.addCandlestickSeries().setData(bars);
// Floor pivots from the previous session's high, low and close.
const DAY = 86_400_000;
const days = [];
bars.time.forEach((t, i) => {
  const d = Math.floor(t / DAY);
  if (days.at(-1)?.d !== d) days.push({ d, first: i, high: -Infinity, low: Infinity });
  const day = days.at(-1);
  Object.assign(day, { last: i, high: Math.max(day.high, bars.high[i]), low: Math.min(day.low, bars.low[i]), close: bars.close[i] });
});
const levels = { R2: '#ff5d73', R1: '#ff5d73', P: '#ffb547', S1: '#2ee6b6', S2: '#2ee6b6' };
const value = Object.fromEntries(Object.keys(levels).map((k) => [k, bars.time.map(() => NaN)]));
days.slice(1).forEach((day, k) => {
  const { high: h, low: l, close: c } = days[k];
  const p = (h + l + c) / 3;
  const at = { P: p, R1: 2 * p - l, S1: 2 * p - h, R2: p + h - l, S2: p - h + l };
  day.levels = at;
  for (let i = day.first + 1; i <= day.last; i++) for (const k of Object.keys(at)) value[k][i] = at[k];
});
for (const [k, color] of Object.entries(levels)) {
  chart.addSeries('line', { color, lineWidth: k === 'P' ? 2 : 1, lineStyle: k === 'P' ? 'solid' : 'dashed', lastValueVisible: false, priceLineVisible: false })
    .setData({ time: bars.time, value: value[k] });
}
// Session separators and level names.
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const out = [];
    for (const day of days) {
      const x = ctx.timeToX(bars.time[day.first]);
      if (x === null) continue;
      out.push({ type: 'line', x1: x, y1: ctx.rect.y, x2: x, y2: ctx.rect.y + ctx.rect.height, color: '#8b93a740', dash: 'dotted' });
      for (const [k, p] of Object.entries(day.levels ?? {})) {
        const y = ctx.priceToY(p);
        if (y !== null) out.push({ type: 'text', x: x + 6, y: y - 7, text: k, color: levels[k], size: 10, align: 'left' });
      }
    }
    return out;
  },
});
${view(4)}`,
      about: 'Classic floor pivots (P, R1–R2, S1–S2) from each previous session, drawn as line series that start fresh every day. A layer plugin adds the session separators and level names.',
      api: ["addSeries('line')", 'addLayer'],
    },
    {
      id: 'gaps',
      title: 'Gaps and gap fills',
      group: 'Price structure',
      code: `${create()}
${bars({ symbol: 'ACME', interval: '1d', count: 500 })}
chart.addCandlestickSeries().setData(bars);
// Opening gaps of 4% or more, and the bar that closed each one.
const gaps = [];
for (let i = 1; i < bars.time.length; i++) {
  const from = bars.close[i - 1], to = bars.open[i];
  if (Math.abs(to / from - 1) < 0.04) continue;
  let fill = i;
  while (fill < bars.time.length && (to > from ? bars.low[fill] > from : bars.high[fill] < from)) fill++;
  gaps.push({ i, from, to, fill: fill < bars.time.length ? fill : null });
}
const filled = gaps.filter((g) => g.fill !== null);
const days = filled.map((g) => g.fill - g.i).sort((a, b) => a - b);
const summary = \`\${gaps.length} gaps of 4%+ · \${Math.round((100 * filled.length) / gaps.length)}% filled · median \${days[days.length >> 1]} bars to fill\`;
chart.addLayer({
  pane: chart.mainPane,
  position: 'belowSeries',
  build(ctx) {
    const out = [];
    const right = ctx.rect.x + ctx.rect.width;
    for (const g of gaps) {
      const x1 = ctx.timeToX(bars.time[g.i - 1]);
      const x2 = g.fill === null ? right : ctx.timeToX(bars.time[g.fill]);
      if (x1 === null || x2 === null || x2 < ctx.rect.x || x1 > right) continue;
      const y1 = ctx.priceToY(g.from), y2 = ctx.priceToY(g.to);
      const color = g.to > g.from ? '#2ee6b6' : '#ff5d73';
      out.push({ type: 'rect', x: x1, y: Math.min(y1, y2), width: x2 - x1, height: Math.abs(y2 - y1), color: color + (g.fill === null ? '55' : '30') });
    }
    out.push({ type: 'text', x: ctx.rect.x + 12, y: ctx.rect.y + 16, text: summary, color: '#e6e9ef', size: 12, align: 'left', background: '#161a22e0', padding: 6 });
    return out;
  },
});
${view(9)}`,
      about: 'Every opening gap of 4% or more as a box from the previous close to the open, running until a later bar trades back through it; open gaps run to the edge. A layer plugin draws the boxes and the summary statistics.',
      api: ['addLayer', "position: 'belowSeries'"],
    },
    {
      id: 'shaded',
      title: 'Regime shading',
      group: 'Price structure',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '4h', count: 1200 })}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
chart.addIndicator(candles, 'sma', { n: 50 });
// Runs of bars closing above or below their 50-bar average.
const n = 50, runs = [];
let sum = 0;
bars.close.forEach((c, i) => {
  sum += c - (i >= n ? bars.close[i - n] : 0);
  if (i < n - 1) return;
  const above = c > sum / n;
  if (runs.at(-1)?.above === above) runs.at(-1).last = i;
  else runs.push({ above, first: i, last: i });
});
chart.addLayer({
  pane: chart.mainPane,
  position: 'background',
  build(ctx) {
    const half = (ctx.timeToX(bars.time[1]) - ctx.timeToX(bars.time[0])) / 2;
    const out = [];
    for (const r of runs) {
      const x1 = ctx.timeToX(bars.time[r.first]), x2 = ctx.timeToX(bars.time[r.last]);
      if (x1 === null || x2 === null) continue;
      out.push({ type: 'rect', x: x1 - half, y: ctx.rect.y, width: x2 - x1 + 2 * half, height: ctx.rect.height, color: r.above ? '#2ee6b61a' : '#ff5d731a' });
    }
    return out;
  },
});
${view(5)}`,
      about: 'The background turns green while price closes above its 50-bar average and red while below, making trend regimes visible at a glance. The SMA is a built-in indicator; a background layer plugin shades the runs.',
      api: ['addLayer', "position: 'background'", "addIndicator('sma')"],
    },
    {
      id: 'events',
      title: 'Corporate events',
      group: 'Events & annotation',
      code: `${create()}
${bars({ symbol: 'ACME', interval: '1d', count: 500 })}
${load('earnings', 'events/corpEarnings', { symbol: 'ACME', interval: '1d', count: 500 })}
${load('dividends', 'events/corpDividends', { symbol: 'ACME', interval: '1d', count: 500 })}
${load('splits', 'events/corpSplits', { symbol: 'ACME', interval: '1d', count: 500 })}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
const markers = [
  ...earnings.time.map((time, i) => ({
    time, position: 'aboveBar', shape: 'circle', color: earnings.value[i] >= 0 ? '#2ee6b6' : '#ff5d73',
    text: \`E \${earnings.value[i] >= 0 ? '+' : ''}\${earnings.value[i].toFixed(1)}%\`,
  })),
  ...dividends.time.map((time, i) => ({ time, position: 'belowBar', shape: 'square', color: '#4cc9f0', text: \`$\${dividends.value[i].toFixed(2)}\` })),
  ...splits.time.map((time, i) => ({ time, position: 'belowBar', shape: 'arrowUp', color: '#ffb547', text: \`\${splits.value[i]}:1 split\` })),
];
candles.setMarkers(markers.sort((a, b) => a.time - b.time));
${view(5)}`,
      about: 'Earnings (with the EPS surprise, green for a beat), dividends and a stock split as markers on daily candles. Each event type is its own feed; `setMarkers` takes them merged and sorted by time.',
      api: ['setMarkers'],
    },
    {
      id: 'eventline',
      title: 'Event timeline',
      group: 'Events & annotation',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '1d', count: 365 })}
${load('events', 'events/macroEvents', { symbol: 'BTCUSD', interval: '1d', count: 365 })}
const line = chart.addSeries('area', { color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.22)', bottomFill: 'rgba(76, 201, 240, 0)', lineWidth: 2 });
line.setData({ time: bars.time, value: bars.close });
// Reactions above the line when the price rose, below when it fell.
line.setMarkers(events.time.map((time, i) => {
  const up = events.value[i] >= 0;
  return { time, position: up ? 'aboveBar' : 'belowBar', shape: up ? 'arrowUp' : 'arrowDown', color: up ? '#2ee6b6' : '#ff5d73', text: events.label[i] };
}));
chart.setVisibleLogicalRange(-6, bars.time.length + 14);`,
      about: 'A year of daily closes with the events that moved them: rate decisions, inflation and jobs data, upgrades, outages and regulation. Markers sit above the line for rallies and below for sell-offs.',
      api: ["addSeries('area')", 'setMarkers'],
    },
    {
      id: 'teaching',
      title: 'Annotated lesson',
      group: 'Events & annotation',
      code: candles({
        symbol: 'SOLUSD',
        interval: '1h',
        count: 600,
        spacing: 7,
        extra: `chart.addIndicator(candles, 'ema', { n: 50 });
// The last big swing: its low and high over the last 220 bars.
const n = bars.time.length;
let lo = n - 220, hi = n - 220;
for (let i = n - 220; i < n; i++) {
  if (bars.low[i] < bars.low[lo]) lo = i;
  if (bars.high[i] > bars.high[hi]) hi = i;
}
const [a, b] = lo < hi ? [lo, hi] : [hi, lo];
const at = (i) => ({ time: bars.time[i], price: i === lo ? bars.low[i] : bars.high[i] });
chart.drawings.add({ kind: 'trend_line', anchors: [at(a), at(b)], style: { line: '#ffb547', lineWidth: 2 } });
chart.drawings.add({ kind: 'fib_retracement', anchors: [at(a), at(b)] });
// 3 · The pullback zone, between the 50% and 61.8% levels.
const level = (f) => bars.low[lo] + (bars.high[hi] - bars.low[lo]) * f;
const mid = Math.round(a + (b - a) * 0.55);
chart.drawings.add({
  kind: 'callout',
  anchors: [{ time: bars.time[mid], price: level(0.56) }, { time: bars.time[Math.round(a + (b - a) * 0.15)], price: level(lo < hi ? 0.9 : 0.25) }],
  params: { text: '3 · Pullback zone: 50-61.8%' },
});
// Numbered steps on the bars.
candles.setMarkers([
  { time: bars.time[a], position: a === lo ? 'belowBar' : 'aboveBar', shape: 'circle', color: '#ffb547', text: '1 · Swing start' },
  { time: bars.time[b], position: b === hi ? 'aboveBar' : 'belowBar', shape: 'circle', color: '#ffb547', text: '2 · Swing end' },
]);`,
      }).replace('chart.scrollToRealtime();', 'chart.setVisibleLogicalRange(n - 235, n + 25);'),
      about: 'A teaching chart: the last big swing traced with a trend line, Fibonacci levels on it, numbered markers and a callout, over candles and an EMA. Pages step through it by showing drawings one at a time with `drawings.setVisible`.',
      api: ['drawings.add', 'setMarkers', 'drawings.setVisible'],
    },
    {
      id: 'analog',
      title: 'Historical analog',
      group: 'Projections',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '4h', count: 2000 })}
chart.addCandlestickSeries().setData(bars);
// Find the past window whose shape best matches the last 60 bars (correlation of log prices).
const m = 60, ahead = 40, n = bars.time.length;
const logs = bars.close.map(Math.log);
const corr = (a, b) => {
  const ma = a.reduce((s, v) => s + v) / a.length, mb = b.reduce((s, v) => s + v) / b.length;
  let ab = 0, aa = 0, bb = 0;
  a.forEach((v, i) => { ab += (v - ma) * (b[i] - mb); aa += (v - ma) ** 2; bb += (b[i] - mb) ** 2; });
  return ab / Math.sqrt(aa * bb);
};
const now = logs.slice(n - m);
let best = { r: -1, end: 0 };
for (let end = m; end < n - m - ahead; end++) {
  const r = corr(logs.slice(end - m, end), now);
  if (r > best.r) best = { r, end };
}
// The match and what followed it, scaled to today's price: over the last 60 bars, then past them.
const step = bars.time[n - 1] - bars.time[n - 2];
const scale = bars.close[n - 1] / bars.close[best.end - 1];
const k = Array.from({ length: m + ahead }, (_, j) => j - m + 1);
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, lastValueVisible: false, priceLineVisible: false }).setData({
  time: k.map((j) => (j <= 0 ? bars.time[n - 1 + j] : bars.time[n - 1] + j * step)),
  value: k.map((j) => bars.close[best.end - 1 + j] * scale),
});
const day = (i) => new Date(bars.time[i]).toISOString().slice(0, 10);
const label = \`Best match: \${day(best.end - m)} to \${day(best.end - 1)} · r = \${best.r.toFixed(2)}\`;
// Shade the window that was matched, with the result.
chart.addLayer({
  pane: chart.mainPane,
  position: 'background',
  build(ctx) {
    const x1 = ctx.timeToX(bars.time[n - m]), x2 = ctx.timeToX(bars.time[n - 1]);
    if (x1 === null || x2 === null) return [];
    return [
      { type: 'rect', x: x1, y: ctx.rect.y, width: x2 - x1, height: ctx.rect.height, color: '#4cc9f014' },
      { type: 'text', x: x1 + 6, y: ctx.rect.y + ctx.rect.height - 12, text: 'Last 60 bars', color: '#4cc9f0', size: 11, align: 'left' },
      { type: 'text', x: ctx.rect.x + 10, y: ctx.rect.y + 16, text: label, color: '#ffb547', size: 12, align: 'left', background: '#161a22e0', padding: 6 },
    ];
  },
});
chart.setBarSpacing(6);
chart.setVisibleLogicalRange(n - 150, n + ahead + 4);`,
      about: 'Searches the history for the 60-bar window most correlated with the latest 60 and lays it, scaled, over the last bars as an amber line that runs on into the future with what followed. A layer plugin shades the matched window and names the match. An illustration of analog search, not a prediction.',
      api: ['addLayer', "addSeries('line')", 'future times'],
    },
    {
      id: 'forecast',
      title: 'Forecast band',
      group: 'Projections',
      code: `${create()}
${FORECAST('5,25,50,75,95')}
chart.addCandlestickSeries().setData(bars);
const p75 = path('p75', '#b388ff80'), p25 = path('p25', '#b388ff80');
chart.addBand(p75, p25, { fill: 'rgba(179, 136, 255, 0.22)' });
path('p95', '#b388ff50', { lineStyle: 'dotted' });
path('p5', '#b388ff50', { lineStyle: 'dotted' });
path('p50', '#b388ff', { lineWidth: 2, lineStyle: 'dashed', lastValueVisible: true });
chart.setBarSpacing(6);
chart.scrollToRealtime();`,
      about: 'A median path extended 72 hours past the last candle, with the 25–75% range as a filled band and the 5% and 95% paths dotted. Future timestamps simply add slots to the time axis.',
      api: ['addBand', "addSeries('line')", 'future times'],
    },
    {
      id: 'fan',
      title: 'Fan chart',
      group: 'Projections',
      code: `${create()}
${FORECAST('5,15,25,35,50,65,75,85,95')}
chart.addCandlestickSeries().setData(bars);
// Nested bands, darker toward the median.
const pairs = [['p95', 'p5', 0.12], ['p85', 'p15', 0.16], ['p75', 'p25', 0.2], ['p65', 'p35', 0.26]];
for (const [hi, lo, alpha] of pairs) {
  chart.addBand(path(hi, 'transparent'), path(lo, 'transparent'), { fill: \`rgba(255, 181, 71, \${alpha})\` });
}
path('p50', '#ffb547', { lineWidth: 2 });
chart.setBarSpacing(6);
chart.scrollToRealtime();`,
      about: 'A fan of nested quantile bands, 5–95% outermost and 35–65% innermost, widening with the horizon. Four `addBand` fills between invisible quantile lines, darker toward the median.',
      api: ['addBand', 'future times'],
    },
  ],
};
