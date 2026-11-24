import { bars, candles, create, stream, view } from './build';
import type { BarsSpec } from './build';
import type { Category } from './types';

/** Real data (OKX) for the examples that stream. */
const ETH5M: BarsSpec = { symbol: 'ETHUSD', interval: '5m', count: 1440, live: true };
const BTC1H: BarsSpec = { symbol: 'BTCUSD', interval: '1h', count: 1440, live: true };

/** Plugin helper: `inView(ctx)` gives the bars on screen and the bar spacing. */
const IN_VIEW = `// The bars on screen (a binary search on time) and the bar spacing, in CSS px.
const lowerBound = (t) => {
  let lo = 0, hi = bars.time.length;
  while (lo < hi) { const m = (lo + hi) >> 1; if (bars.time[m] < t) lo = m + 1; else hi = m; }
  return lo;
};
const inView = (ctx) => ({
  from: Math.max(0, lowerBound(ctx.xToTime(ctx.rect.x)) - 1),
  to: Math.min(bars.time.length, lowerBound(ctx.xToTime(ctx.rect.x + ctx.rect.width)) + 1),
  spacing: ctx.timeToX(bars.time[1]) - ctx.timeToX(bars.time[0]),
});`;

/** A candlestick series that only scales the axis and feeds the crosshair. */
const HIDDEN = `// Invisible candles keep the price scale and the crosshair; a layer draws the bars.
chart.addCandlestickSeries({ color: 'transparent', downColor: 'transparent' }).setData(bars);`;

export const price: Category = {
  id: 'price',
  title: 'Price action',
  kicker: '01',
  blurb: 'Candles, bars and lines for any market, streaming in real time.',
  accent: '#2ee6b6',
  demos: [
    {
      id: 'candles',
      title: 'Candlesticks + volume',
      group: 'Candles & bars',
      code: candles({ symbol: 'BTCUSD', interval: '1m', count: 1440, volume: true, spacing: 9, live: true, stream: true }),
      about: 'The classic view. A day of real one-minute bars, updating every second, on a GPU-rendered price scale with a volume histogram below. Drag to pan, scroll or pinch to zoom.',
      live: true,
      api: ['FerroChart.create', 'addCandlestickSeries', 'addVolumeSeries'],
    },
    {
      id: 'live',
      title: 'Real-time stream',
      group: 'Candles & bars',
      code: candles({ symbol: 'BTCUSD', interval: '1m', count: 1440, spacing: 10, live: true, stream: true, extra: "chart.addIndicator(candles, 'ema', { n: 21 });" }),
      live: true,
      about: '`update` replaces the last bar or appends a new one. Indicators update incrementally with it, so a tick costs microseconds, not a redraw of history.',
      api: ['Series.update', 'addIndicator'],
    },
    {
      id: 'replay',
      title: 'Bar replay',
      group: 'Candles & bars',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '5m', count: 1500 })}
// Start 300 bars back and play the rest, one bar every 120 ms.
const head = (n) => Object.fromEntries(Object.entries(bars).map(([k, v]) => [k, v.slice(0, n)]));
const candles = chart.addCandlestickSeries();
let next = bars.time.length - 300;
candles.setData(head(next));
chart.addIndicator(candles, 'ema', { n: 20 });
chart.setBarSpacing(9);
chart.scrollToRealtime();

const timer = setInterval(() => {
  if (next >= bars.time.length) return clearInterval(timer);
  const i = next++;
  candles.update({ time: bars.time[i], open: bars.open[i], high: bars.high[i], low: bars.low[i], close: bars.close[i], volume: bars.volume[i] });
}, 120);
const stop = () => clearInterval(timer); // pause; call before chart.destroy()`,
      about: 'History played back bar by bar with `update`, the way a live feed would arrive, with an EMA keeping pace incrementally. Play, pause and speed are a few lines of page code around a timer.',
      api: ['Series.update', 'setData', 'addIndicator'],
    },
    {
      id: 'ohlc',
      title: 'OHLC bars',
      group: 'Candles & bars',
      code: [create(), bars({ symbol: 'XAUUSD', interval: '15m', count: 1500 }), "chart.addSeries('ohlc').setData(bars);", view(8)].join('\n'),
      about: 'Open and close ticks on a high–low bar: the denser alternative to candles, common for futures and FX.',
      api: ["addSeries('ohlc')"],
    },
    {
      id: 'heikin',
      title: 'Heikin-Ashi',
      group: 'Candles & bars',
      code: [create(), bars(ETH5M), "const ha = chart.addSeries('heikinAshi');", 'ha.setData(bars);', view(8), '', stream('ha', ETH5M)].join('\n'),
      live: true,
      about: 'Averaged candles that make trends easier to read. Computed from the same OHLC columns; the crosshair still reports real prices.',
      api: ["addSeries('heikinAshi')"],
    },
    {
      id: 'hollow',
      title: 'Hollow candles',
      group: 'Candle styles',
      code: `${create()}
${bars({ symbol: 'NOVA', interval: '1d', count: 400 })}
${HIDDEN}
${IN_VIEW}
const up = '#2ee6b6', down = '#ff5d73';
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const { from, to, spacing } = inView(ctx);
    const half = Math.max(1, spacing * 0.36);
    const out = [];
    for (let i = Math.max(1, from); i < to; i++) {
      const x = ctx.timeToX(bars.time[i]);
      if (x === null) continue;
      // Color: close against the previous close. Fill: close against the open.
      const color = bars.close[i] >= bars.close[i - 1] ? up : down;
      const yo = ctx.priceToY(bars.open[i]), yc = ctx.priceToY(bars.close[i]);
      const top = Math.min(yo, yc), bottom = Math.max(yo, yc, top + 1);
      out.push({ type: 'line', x1: x, y1: ctx.priceToY(bars.high[i]), x2: x, y2: top, color });
      out.push({ type: 'line', x1: x, y1: bottom, x2: x, y2: ctx.priceToY(bars.low[i]), color });
      if (bars.close[i] > bars.open[i]) {
        out.push({ type: 'polyline', color, width: 1.2, points: [[x - half, top], [x + half, top], [x + half, bottom], [x - half, bottom], [x - half, top]] });
      } else {
        out.push({ type: 'rect', x: x - half, y: top, width: 2 * half, height: bottom - top, color });
      }
    }
    return out;
  },
});
${view(11)}`,
      about: 'Rising bars (close above open) are hollow, falling ones filled; the color compares the close with the previous close. A layer plugin draws the candles over an invisible candlestick series that keeps the scale and crosshair.',
      api: ['addLayer', 'PluginCtx.priceToY', "color: 'transparent'"],
    },
    {
      id: 'volcandles',
      title: 'Volume-width candles',
      group: 'Candle styles',
      live: true,
      code: `${create()}
${bars({ symbol: 'SOLUSD', interval: '15m', count: 1200, live: true })}
${HIDDEN}
${IN_VIEW}
const up = '#2ee6b6', down = '#ff5d73';
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const { from, to, spacing } = inView(ctx);
    // Body width: the bar's share of the largest volume on screen.
    let max = 0;
    for (let i = from; i < to; i++) max = Math.max(max, bars.volume[i]);
    const out = [];
    for (let i = from; i < to; i++) {
      const x = ctx.timeToX(bars.time[i]);
      if (x === null) continue;
      const color = bars.close[i] >= bars.open[i] ? up : down;
      const w = Math.max(1.5, spacing * 0.95 * (bars.volume[i] / max));
      const yo = ctx.priceToY(bars.open[i]), yc = ctx.priceToY(bars.close[i]);
      out.push({ type: 'line', x1: x, y1: ctx.priceToY(bars.high[i]), x2: x, y2: ctx.priceToY(bars.low[i]), color });
      out.push({ type: 'rect', x: x - w / 2, y: Math.min(yo, yc), width: w, height: Math.max(1, Math.abs(yc - yo)), color });
    }
    return out;
  },
});
${view(14)}`,
      about: 'Each body\'s width is its volume against the busiest bar on screen, so heavy bars stand out from thin ones. A layer plugin draws the candles; slots stay evenly spaced on the time axis.',
      api: ['addLayer', 'PluginCtx.timeToX'],
    },
    {
      id: 'hlc',
      title: 'HLC bars',
      group: 'Candle styles',
      code: `${create()}
${bars({ symbol: 'ES', interval: '30m', count: 900 })}
${HIDDEN}
${IN_VIEW}
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const { from, to, spacing } = inView(ctx);
    const tick = Math.max(2, spacing * 0.4);
    const out = [];
    for (let i = Math.max(1, from); i < to; i++) {
      const x = ctx.timeToX(bars.time[i]);
      if (x === null) continue;
      const color = bars.close[i] >= bars.close[i - 1] ? '#2ee6b6' : '#ff5d73';
      const yc = ctx.priceToY(bars.close[i]);
      // High–low bar and a close tick to the right; no open.
      out.push({ type: 'line', x1: x, y1: ctx.priceToY(bars.high[i]), x2: x, y2: ctx.priceToY(bars.low[i]), color, width: 1.5 });
      out.push({ type: 'line', x1: x, y1: yc, x2: x + tick, y2: yc, color, width: 1.5 });
    }
    return out;
  },
});
${view(10)}`,
      about: 'High–low bars with only a close tick, colored by the change from the previous close. The engine\'s OHLC bars always carry the open tick, so a layer plugin draws these.',
      api: ['addLayer', "addSeries('ohlc')"],
    },
    {
      id: 'roundcandles',
      title: 'Rounded candles',
      group: 'Candle styles',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '1h', count: 600 })}
${HIDDEN}
${IN_VIEW}
// A rectangle with rounded corners, as a polygon.
const rounded = (x, y, w, h, r) => {
  r = Math.min(r, w / 2, h / 2);
  const pts = [];
  for (const [cx, cy, a] of [[x + w - r, y + r, -90], [x + w - r, y + h - r, 0], [x + r, y + h - r, 90], [x + r, y + r, 180]]) {
    for (let k = 0; k <= 4; k++) {
      const t = ((a + k * 22.5) * Math.PI) / 180;
      pts.push([cx + r * Math.cos(t), cy + r * Math.sin(t)]);
    }
  }
  return pts;
};
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const { from, to, spacing } = inView(ctx);
    const w = Math.max(2, spacing * 0.7);
    const out = [];
    for (let i = from; i < to; i++) {
      const x = ctx.timeToX(bars.time[i]);
      if (x === null) continue;
      const color = bars.close[i] >= bars.open[i] ? '#2ee6b6' : '#ff5d73';
      const yo = ctx.priceToY(bars.open[i]), yc = ctx.priceToY(bars.close[i]);
      const yh = ctx.priceToY(bars.high[i]), yl = ctx.priceToY(bars.low[i]);
      const wick = Math.max(1.5, w * 0.16);
      out.push({ type: 'polygon', color, points: rounded(x - wick / 2, yh, wick, yl - yh, wick / 2) });
      out.push({ type: 'polygon', color, points: rounded(x - w / 2, Math.min(yo, yc), w, Math.max(2, Math.abs(yc - yo)), w * 0.3) });
    }
    return out;
  },
});
${view(16)}`,
      about: 'Candles with rounded bodies and round-capped wicks, for dashboards and consumer apps. A layer plugin draws each one as a polygon.',
      api: ['addLayer', "PluginCommand 'polygon'"],
    },
    {
      id: 'highlow',
      title: 'High–low range bars',
      group: 'Candle styles',
      code: `${create()}
${bars({ symbol: 'XAUUSD', interval: '1d', count: 120 })}
${HIDDEN}
${IN_VIEW}
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const { from, to, spacing } = inView(ctx);
    const w = Math.max(2, spacing * 0.62);
    const labels = spacing >= 30; // numbers only when they fit
    const out = [];
    for (let i = from; i < to; i++) {
      const x = ctx.timeToX(bars.time[i]);
      if (x === null) continue;
      const yh = ctx.priceToY(bars.high[i]), yl = ctx.priceToY(bars.low[i]);
      const color = bars.close[i] >= bars.open[i] ? '#2ee6b6' : '#ff5d73';
      out.push({ type: 'rect', x: x - w / 2, y: yh, width: w, height: Math.max(1, yl - yh), color: color + '99' });
      out.push({ type: 'line', x1: x - w / 2, y1: yh, x2: x + w / 2, y2: yh, color, width: 2 });
      out.push({ type: 'line', x1: x - w / 2, y1: yl, x2: x + w / 2, y2: yl, color, width: 2 });
      if (labels) {
        out.push({ type: 'text', x, y: yh - 9, text: ctx.formatPrice(bars.high[i]), color: '#e6e9ef', size: 10, align: 'center' });
        out.push({ type: 'text', x, y: yl + 9, text: ctx.formatPrice(bars.low[i]), color: '#8b93a7', size: 10, align: 'center' });
      }
    }
    return out;
  },
});
chart.applyPriceScaleOptions({ margins: { top: 0.08, bottom: 0.08 } });
${view(44)}`,
      about: 'Each day as one bar from its low to its high, with both prices printed when zoomed in. A layer plugin draws the bars and labels with `formatPrice`, so the numbers match the axis.',
      api: ['addLayer', 'PluginCtx.formatPrice', "PluginCommand 'text'"],
    },
    {
      id: 'line',
      title: 'Line',
      group: 'Lines & columns',
      code: `${create()}
${bars(BTC1H)}
const line = chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2 });
line.setData({ time: bars.time, value: bars.close });
${view(4)}

${stream('line', BTC1H, { value: true })}`,
      live: true,
      about: 'Closing prices as a single line: the cleanest view of a trend.',
      api: ["addSeries('line')", 'setData'],
    },
    {
      id: 'linemarkers',
      title: 'Line with points',
      group: 'Lines & columns',
      code: `${create()}
${bars({ symbol: 'ACME', interval: '1d', count: 200 })}
const data = { time: bars.time, value: bars.close };
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2 }).setData(data);
// Points are drawn as dots \`lineWidth\` px across.
chart.addSeries('points', { color: '#4cc9f0', lineWidth: 7, lastValueVisible: false, priceLineVisible: false }).setData(data);
${view(14)}`,
      about: 'A line series with a points series on the same data, so each daily close is a visible dot.',
      api: ["addSeries('line')", "addSeries('points')"],
    },
    {
      id: 'step',
      title: 'Step line',
      group: 'Lines & columns',
      code: `${create()}
${bars({ symbol: 'US10Y', interval: '1d', count: 500 })}
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, step: true, pricePrecision: 3 })
  .setData({ time: bars.time, value: bars.close });
${view(6)}`,
      about: 'A yield (in percent) held flat until the next print and then stepping to it, as rates and fixings are usually drawn. `step: true` on a line series.',
      api: ["addSeries('line', { step: true })"],
    },
    {
      id: 'hlcarea',
      title: 'HLC area',
      group: 'Lines & columns',
      live: true,
      code: `${create()}
${bars({ symbol: 'SOLUSD', interval: '1h', count: 1440, live: true })}
const high = chart.addSeries('line', { color: 'rgba(46, 230, 182, 0.5)', lineWidth: 1, lastValueVisible: false, priceLineVisible: false });
const low = chart.addSeries('line', { color: 'rgba(255, 93, 115, 0.5)', lineWidth: 1, lastValueVisible: false, priceLineVisible: false });
const close = chart.addSeries('line', { color: '#e6e9ef', lineWidth: 2 });
high.setData({ time: bars.time, value: bars.high });
low.setData({ time: bars.time, value: bars.low });
close.setData({ time: bars.time, value: bars.close });
chart.addBand(high, close, { fill: 'rgba(46, 230, 182, 0.22)' });
chart.addBand(close, low, { fill: 'rgba(255, 93, 115, 0.22)' });
${view(10)}`,
      about: 'The close as a line, with the room up to the high filled green and down to the low filled red: the range of every bar without candle bodies. Two `addBand` fills between three line series.',
      api: ['addBand', "addSeries('line')"],
    },
    {
      id: 'columns',
      title: 'Columns',
      group: 'Lines & columns',
      code: `${create()}
${bars({ symbol: 'NOVA', interval: '1d', count: 250 })}
// Price columns from the lowest low, colored by the change from the previous close.
const base = Math.min(...bars.low);
chart.addSeries('histogram', { baseValue: base }).setData({
  time: bars.time,
  value: bars.close,
  color: bars.close.map((c, i) => (i === 0 || c >= bars.close[i - 1] ? '#2ee6b6' : '#ff5d73')),
});
${view(9)}`,
      about: 'Closing prices as columns rising from the lowest low, each colored by whether it closed up or down. A histogram series with `baseValue` and per-point colors.',
      api: ["addSeries('histogram')", 'baseValue', 'per-point color'],
    },
    {
      id: 'histogram',
      title: 'Returns histogram',
      group: 'Lines & columns',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '4h', count: 900 })}
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2 }).setData({ time: bars.time, value: bars.close });
// Return of each bar, in percent, in a pane below.
const ret = bars.close.map((c, i) => (i === 0 ? 0 : (c / bars.close[i - 1] - 1) * 100));
chart.addSeries('histogram', { pricePrecision: 2, lastValueVisible: false, priceLineVisible: false }, chart.addPane(0.7)).setData({
  time: bars.time,
  value: ret,
  color: ret.map((r) => (r >= 0 ? '#2ee6b6' : '#ff5d73')),
});
${view(6)}`,
      about: 'The close above and each bar\'s percent return below as a histogram around zero, colored by sign. Large bars mark the volatile sessions.',
      api: ["addSeries('histogram')", 'addPane', 'per-point color'],
    },
    {
      id: 'area',
      title: 'Area',
      group: 'Lines & columns',
      code: `${create()}
${bars({ symbol: 'SOLUSD', interval: '1m', count: 20000 })}
const area = chart.addSeries('area', {
  color: '#4cc9f0',
  topFill: 'rgba(76, 201, 240, 0.32)',
  bottomFill: 'rgba(76, 201, 240, 0)',
  lineWidth: 2,
});
area.setData({ time: bars.time, value: bars.close });
chart.fitContent();`,
      about: 'Twenty thousand points, all on screen. Level-of-detail keeps dense line and area series smooth and free of aliasing at any zoom.',
      api: ["addSeries('area')", 'fitContent'],
    },
    {
      id: 'baseline',
      title: 'Baseline',
      group: 'Lines & columns',
      code: `${create()}
${bars({ symbol: 'ACME', interval: '5m', count: 400 })}
chart.addSeries('baseline', {
  baseValue: bars.open[0],
  color: '#2ee6b6', downColor: '#ff5d73',
  topFill: 'rgba(46, 230, 182, 0.25)', bottomFill: 'rgba(255, 93, 115, 0.25)',
}).setData({ time: bars.time, value: bars.close });
chart.fitContent();`,
      about: 'Above and below a reference price in two colors: performance since the open, P&L against an entry. Gaps where the market was closed stay out of the time axis.',
      api: ["addSeries('baseline')", 'baseValue'],
    },
    {
      id: 'compare',
      title: 'Compare (% change)',
      group: 'Lines & columns',
      code: `${create()}
const colors = { BTCUSD: '#ff7a3d', ETHUSD: '#4cc9f0', SOLUSD: '#b388ff' };
for (const [symbol, color] of Object.entries(colors)) {
  const bars = await fetch(\`/api/bars?symbol=\${symbol}&interval=1h&count=720\`).then((r) => r.json());
  chart.addSeries('line', { color, lineWidth: 2 }).setData({ time: bars.time, value: bars.close });
}
chart.applyPriceScaleOptions({ mode: 'percentage' });
chart.fitContent();`,
      about: 'Three markets on one percentage scale, rebased to the first visible bar as you pan.',
      api: ["applyPriceScaleOptions({ mode: 'percentage' })"],
    },
    {
      id: 'overlaycandles',
      title: 'Two markets as candles',
      group: 'Overlays & context',
      live: true,
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '1h', count: 600, live: true }, 'btc')}
${bars({ symbol: 'ETHUSD', interval: '1h', count: 600, live: true }, 'eth')}
chart.addCandlestickSeries().setData(btc);
chart.addCandlestickSeries({ color: '#4cc9f0', downColor: '#b388ff' }).setData(eth);
// One scale per pane: compare both in percent from the first visible bar.
chart.applyPriceScaleOptions({ mode: 'percentage' });
chart.setVisibleLogicalRange(500, 605);`,
      about: 'Two markets\' candles on one time axis, in their own colors. A pane has a single price scale, so the percentage mode puts both on a common footing.',
      api: ['addCandlestickSeries', "applyPriceScaleOptions({ mode: 'percentage' })"],
    },
    {
      id: 'htfcandles',
      title: 'Higher-timeframe candles',
      group: 'Overlays & context',
      live: true,
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '15m', count: 1440, live: true })}
chart.addCandlestickSeries().setData(bars);
// Four-hour candles built from the 15-minute bars.
const H4 = 4 * 3_600_000;
const big = [];
bars.time.forEach((t, i) => {
  const start = Math.floor(t / H4) * H4;
  const last = big[big.length - 1];
  if (!last || last.start !== start) {
    big.push({ start, first: t, last: t, open: bars.open[i], high: bars.high[i], low: bars.low[i], close: bars.close[i] });
  } else {
    Object.assign(last, { last: t, high: Math.max(last.high, bars.high[i]), low: Math.min(last.low, bars.low[i]), close: bars.close[i] });
  }
});
chart.addLayer({
  pane: chart.mainPane,
  position: 'belowSeries',
  build(ctx) {
    const out = [];
    const half = (ctx.timeToX(bars.time[1]) - ctx.timeToX(bars.time[0])) / 2;
    for (const c of big) {
      const x1 = ctx.timeToX(c.first), x2 = ctx.timeToX(c.last);
      if (x1 === null || x2 === null || x2 < ctx.rect.x || x1 > ctx.rect.x + ctx.rect.width) continue;
      const color = c.close >= c.open ? '#2ee6b6' : '#ff5d73';
      const left = x1 - half, right = x2 + half, mid = (left + right) / 2;
      const yo = ctx.priceToY(c.open), yc = ctx.priceToY(c.close);
      const top = Math.min(yo, yc), bottom = Math.max(yo, yc);
      out.push({ type: 'rect', x: left, y: top, width: right - left, height: Math.max(1, bottom - top), color: color + '1f' });
      out.push({ type: 'polyline', color: color + 'aa', width: 1, points: [[left, top], [right, top], [right, bottom], [left, bottom], [left, top]] });
      out.push({ type: 'line', x1: mid, y1: ctx.priceToY(c.high), x2: mid, y2: top, color: color + 'aa', dash: 'dotted' });
      out.push({ type: 'line', x1: mid, y1: bottom, x2: mid, y2: ctx.priceToY(c.low), color: color + 'aa', dash: 'dotted' });
    }
    return out;
  },
});
${view(7)}`,
      about: 'Fifteen-minute candles with the four-hour candle each belongs to drawn behind them: body as a tinted box, wicks dotted. The four-hour bars are aggregated in a few lines of JS and a layer plugin draws them.',
      api: ['addLayer', "position: 'belowSeries'"],
    },
    {
      id: 'extsession',
      title: 'Trading sessions',
      group: 'Overlays & context',
      code: `${create()}
${bars({ symbol: 'EURUSD', interval: '15m', count: 700 })}
chart.addCandlestickSeries({ pricePrecision: 5 }).setData(bars);
// Sessions in UTC hours.
const sessions = [
  { name: 'Asia', from: 0, to: 7, color: '#b388ff1c' },
  { name: 'London', from: 7, to: 13, color: '#4cc9f01c' },
  { name: 'New York', from: 13, to: 21, color: '#ffb5471c' },
];
const H = 3_600_000, DAY = 24 * H;
// Each day's sessions as [first bar, last bar] time ranges.
const blocks = [];
for (let day = Math.floor(bars.time[0] / DAY) * DAY; day <= bars.time[bars.time.length - 1]; day += DAY) {
  for (const s of sessions) {
    const inside = bars.time.filter((t) => t >= day + s.from * H && t < day + s.to * H);
    if (inside.length) blocks.push({ ...s, first: inside[0], last: inside[inside.length - 1] });
  }
}
chart.addLayer({
  pane: chart.mainPane,
  position: 'background',
  build(ctx) {
    const half = (ctx.timeToX(bars.time[1]) - ctx.timeToX(bars.time[0])) / 2;
    const { y, height } = ctx.rect;
    const out = [];
    for (const b of blocks) {
      const x1 = ctx.timeToX(b.first), x2 = ctx.timeToX(b.last);
      if (x1 === null || x2 === null) continue;
      out.push({ type: 'rect', x: x1 - half, y, width: x2 - x1 + 2 * half, height, color: b.color });
      out.push({ type: 'text', x: x1 - half + 4, y: y + 10, text: b.name, color: '#8b93a7', size: 10, align: 'left' });
    }
    return out;
  },
});
${view(5)}`,
      about: 'Fifteen-minute bars of a currency pair with the Asia, London and New York sessions shaded and labelled. A background layer plugin draws the bands from session times in UTC.',
      api: ['addLayer', "position: 'background'"],
    },
    {
      id: 'drawings',
      title: 'Drawing tools',
      group: 'Overlays & context',
      code: candles({
        symbol: 'BTCUSD',
        interval: '15m',
        count: 1200,
        spacing: 8,
        extra: `const n = bars.close.length;
let lo = n - 400, hi = n - 400;
for (let i = n - 400; i < n; i++) {
  if (bars.low[i] < bars.low[lo]) lo = i;
  if (bars.high[i] > bars.high[hi]) hi = i;
}
const at = (i) => ({ time: bars.time[i], price: i === lo ? bars.low[i] : bars.high[i] });
const [a, b] = lo < hi ? [lo, hi] : [hi, lo];
chart.drawings.add({ kind: 'fib_retracement', anchors: [at(a), at(b)] });
chart.drawings.add({ kind: 'trend_line', anchors: [at(a), at(b)], style: { line: '#ffb547', lineWidth: 2 } });
// Users draw with chart.drawings.startTool('trend_line') and 28 more tools.`,
      }).replace('chart.scrollToRealtime();', 'chart.setVisibleLogicalRange(n - 430, n + 15);'),
      about: '29 built-in tools: lines, channels, Fibonacci, pitchforks, Gann, patterns and positions. Undo/redo, save and load, sync across charts, and custom tools in JavaScript.',
      api: ['drawings.add', 'drawings.startTool', 'drawings.register'],
    },
    {
      id: 'quote-card',
      title: 'Quote card',
      group: 'App style',
      live: true,
      code: `// A quote card: price and change on top, the range's line against the price
// it started from, and range tabs below. The text and tabs are plain HTML
// over the canvas; the chart has no axes, no grid and no panning or zooming.
const chart = await FerroChart.create(canvas, {
  theme: 'dark',
  priceAxis: false,
  timeAxis: false,
  grid: false,
  padding: { top: 92, right: 20, bottom: 60, left: 20 },
  interaction: { pan: false, zoom: false },
  crosshairStyle: { horizontal: false, labels: false, dot: { radius: 5 } },
});
const line = chart.addSeries('baseline', {
  color: '#2ee6b6', downColor: '#ff5d73', topFill: '#2ee6b633', bottomFill: '#ff5d7333',
  lineWidth: 2, lastValueVisible: false, priceLineVisible: false,
});
// The price the range started from (for 1D: 24 hours ago), dashed.
const ref = line.createPriceLine({ price: 0, color: '#8b93a7', lineStyle: 'dashed', lineWidth: 1, axisLabelVisible: false });

const head = document.createElement('div');
head.style.cssText = 'position:absolute;left:24px;top:16px;font:500 13px system-ui;color:#8b93a7;pointer-events:none';
const tabs = document.createElement('div');
tabs.style.cssText = 'position:absolute;left:50%;transform:translateX(-50%);bottom:16px;display:flex;gap:4px';
canvas.after(head, tabs);

const RANGES = { '1D': ['5m', 288], '1W': ['1h', 168], '1M': ['4h', 180], '1Y': ['1d', 365], All: ['1w', 3000] };
const fmt = (v) => v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
let range = '1D', bars = null, base = 0, last = 0, ws = null;

function show(price, time) {
  const change = price - base;
  const color = change >= 0 ? '#2ee6b6' : '#ff5d73';
  const when = time ? new Date(time).toLocaleString('en-US', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }) : range;
  head.innerHTML = \`ETH / USD
    <div style="font:600 32px system-ui;color:#e6e9ef;margin:2px 0">\${fmt(price)}</div>
    <span style="color:\${color}">\${change >= 0 ? '+' : '−'}\${fmt(Math.abs(change))} (\${(change / base * 100).toFixed(2)}%)</span>&nbsp; \${when}\`;
}

async function load(r) {
  range = r;
  for (const b of tabs.children) b.dataset.on = b.textContent === r ? '1' : '';
  ws?.close();
  const [interval, count] = RANGES[r];
  bars = await fetch(\`/api/live/bars?symbol=ETHUSD&interval=\${interval}&count=\${count}\`).then((res) => res.json());
  if (range !== r) return; // another tab was picked meanwhile
  base = bars.open[0];
  last = bars.close.at(-1);
  line.applyOptions({ baseValue: base });
  line.setData({ time: bars.time, value: bars.close });
  ref.applyOptions({ price: base });
  chart.fitContent();
  show(last);
  // Real time: the forming bar every second.
  ws = new WebSocket(\`/api/stream?symbol=ETHUSD&interval=\${interval}&live=true&trades=false&since=\${bars.time.at(-1)}\`);
  ws.onmessage = (e) => {
    const msg = JSON.parse(e.data);
    if (msg.type !== 'bar') return;
    line.update({ time: msg.time, value: msg.close });
    last = msg.close;
    if (!hovering) show(last);
  };
}

for (const r of Object.keys(RANGES)) {
  const b = document.createElement('button');
  b.textContent = r;
  b.style.cssText = 'font:600 12px system-ui;color:#c9cfdb;background:none;border:0;border-radius:8px;padding:6px 12px;cursor:pointer';
  b.onclick = () => load(r);
  tabs.append(b);
}
const style = document.createElement('style');
style.textContent = 'button[data-on="1"]{background:#ffffff1a!important;color:#fff!important}';
tabs.append(style);

// While the pointer is over the line, the header shows the value under it.
let hovering = false;
chart.on('crosshairMove', ({ point }) => {
  const v = point && line.dataAt(point.logicalIndex)?.value;
  hovering = Number.isFinite(v);
  show(hovering ? v : last, hovering ? point.time : null);
});

await load('1D');
const stop = () => ws?.close(); // call before chart.destroy()`,
      about: 'A phone-style quote: price and change over the chart, the range\'s line coloured above and below the price it started from (dashed), and 1D to All tabs. No axes, no grid, no panning; the crosshair is a vertical line with a dot, and the header follows it.',
      api: ['priceAxis', 'timeAxis', 'grid', 'padding', 'interaction', 'crosshairStyle', "addSeries('baseline')"],
    },
    {
      id: 'watchlist',
      title: 'Watchlist',
      group: 'App style',
      live: true,
      code: `// A watchlist: 30 pairs, each row with a 24-hour sparkline. The sparklines
// come from one pool, so they share one GPU device (a page can hold only a
// few WebGL contexts); prices refresh every 2 seconds. A row opens its pair
// in the chart on the left.
const PAIRS = ['BTC', 'ETH', 'SOL', 'BNB', 'XRP', 'DOGE', 'ADA', 'AVAX', 'LINK', 'LTC', 'DOT', 'TRX', 'SHIB', 'BCH', 'NEAR',
  'UNI', 'APT', 'ATOM', 'ETC', 'FIL', 'ARB', 'OP', 'SUI', 'PEPE', 'ICP', 'XLM', 'HBAR', 'AAVE', 'INJ', 'WLD'];
const UP = '#2ee6b6', DOWN = '#ff5d73';
const digits = (p) => (p >= 100 ? 2 : p >= 1 ? 3 : p >= 0.01 ? 5 : 8);

// The list: right of the chart, or below it on a narrow screen.
const narrow = canvas.parentElement.clientWidth < 640;
const list = document.createElement('div');
list.style.cssText = 'position:absolute;overflow-y:auto;background:#0b0d12;font:500 13px system-ui;color:#e6e9ef;'
  + (narrow ? 'left:0;right:0;bottom:0;height:55%;border-top:1px solid #ffffff14' : 'top:0;right:0;bottom:0;width:min(360px,48%);border-left:1px solid #ffffff14');
if (narrow) canvas.style.height = '45%';
else canvas.style.width = 'calc(100% - min(360px, 48%))';
const title = document.createElement('div');
title.style.cssText = 'position:absolute;left:16px;top:12px;font:600 15px system-ui;color:#e6e9ef;pointer-events:none';
canvas.after(list, title);

const chart = await FerroChart.create(canvas, { theme: 'dark' });
const main = chart.addSeries('area', { color: '#4cc9f0', topFill: '#4cc9f055', bottomFill: '#4cc9f000', lineWidth: 2 });
async function open(sym) {
  const bars = await fetch(\`/api/live/bars?symbol=\${sym}USDT&interval=15m&count=672\`).then((r) => r.json());
  main.applyOptions({ pricePrecision: digits(bars.close.at(-1)) });
  main.setData({ time: bars.time, value: bars.close });
  chart.fitContent();
  title.textContent = \`\${sym} / USDT · 15m\`;
  for (const row of list.children) row.style.boxShadow = row.dataset.sym === sym ? 'inset 3px 0 #4cc9f0' : '';
}

const pool = await FerroChart.createPool();
const rows = new Map();
for (const sym of PAIRS) {
  const row = document.createElement('div');
  row.dataset.sym = sym;
  row.style.cssText = 'display:grid;grid-template-columns:64px 1fr 96px;align-items:center;gap:10px;padding:8px 14px;border-bottom:1px solid #ffffff0a;cursor:pointer';
  row.innerHTML = \`<b>\${sym}</b><canvas style="width:100%;height:36px"></canvas>
    <div style="text-align:right"><div class="p">—</div><div class="c" style="font-size:11px">—</div></div>\`;
  row.onclick = () => open(sym);
  list.append(row);
  const spark = await pool.create(row.querySelector('canvas'), { preset: 'sparkline', theme: { base: 'dark', background: '#0b0d12' } });
  const s = spark.addSeries('baseline', { color: UP, downColor: DOWN, topFill: '#00000000', bottomFill: '#00000000', lineWidth: 1.5, lastValueVisible: false, priceLineVisible: false });
  rows.set(sym, { row, spark, s, last: null });
}

// One day of 5-minute closes per pair, against the price 24 hours ago.
await Promise.all(PAIRS.map(async (sym) => {
  const r = rows.get(sym);
  const bars = await fetch(\`/api/live/bars?symbol=\${sym}USDT&interval=5m&count=288\`).then((res) => (res.ok ? res.json() : null));
  if (!bars) return;
  r.s.applyOptions({ baseValue: bars.open[0] });
  r.s.setData({ time: bars.time, value: bars.close });
  r.spark.fitContent();
  r.last = bars.time.at(-1);
}));

async function tick() {
  const quotes = await fetch(\`/api/live/tickers?symbols=\${PAIRS.map((p) => \`\${p}USDT\`).join(',')}\`).then((r) => r.json()).catch(() => []);
  for (const { symbol, price, ref } of quotes) {
    const r = rows.get(symbol.replace(/USDT$/, ''));
    if (!r) continue;
    const pct = (price / ref - 1) * 100;
    r.row.querySelector('.p').textContent = Number(price).toFixed(digits(price));
    const c = r.row.querySelector('.c');
    c.textContent = \`\${pct >= 0 ? '+' : ''}\${pct.toFixed(2)}%\`;
    c.style.color = pct >= 0 ? UP : DOWN;
    // The sparkline follows the price: the last 5-minute point, or a new one.
    if (r.last === null) continue;
    r.last = Math.max(r.last, Math.floor(Date.now() / 300_000) * 300_000);
    r.s.update({ time: r.last, value: Number(price) });
  }
}
await tick();
const timer = setInterval(tick, 2000);
await open('BTC');
const stop = () => clearInterval(timer); // then pool.destroy() and chart.destroy()`,
      about: 'Thirty pairs with a 24-hour sparkline each and prices that refresh every 2 seconds. The sparklines are charts from one pool sharing a GPU device, so a page of many small charts stays within the browser\'s limit of WebGL contexts.',
      api: ['FerroChart.createPool', "preset: 'sparkline'", 'theme object', "addSeries('baseline')"],
    },
  ],
};
