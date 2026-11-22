import { bars, create, load } from './build';
import type { Category } from './types';

/** A legend in the top-left corner of `pane`, drawn by a layer plugin. */
const LEGEND = `const legend = (pane, items) => chart.addLayer({ pane, position: 'overlay', build: (ctx) => items.flatMap(([text, color], i) => [
  { type: 'rect', x: ctx.rect.x + 10, y: ctx.rect.y + 13 + i * 16, width: 12, height: 3, color },
  { type: 'text', x: ctx.rect.x + 28, y: ctx.rect.y + 8 + i * 16, text, color: '#c9d1e0', size: 11 },
]) });`;

export const execution: Category = {
  id: 'execution',
  title: 'Execution & trade review',
  kicker: '05',
  blurb: 'Fills against plans, slippage, queue position and the trades you made, on the price they met.',
  accent: '#9b8cff',
  demos: [
    {
      id: 'parentprogress',
      title: 'Parent order progress',
      group: 'Order execution',
      code: `${create()}
// A day and three hours of minute bars: the last 3 h are the order so far,
// the same minutes a day earlier give the expected volume curve.
${bars({ interval: '1m', count: 1620 })}
${load('fills', 'series/execFills', { symbol: 'BTCUSD', interval: '1m', count: 180 })}
const TARGET = 750, DONE = 180, LEFT = 120; // BTC; minutes elapsed and to go
const n = bars.time.length;
chart.addCandlestickSeries().setData({
  time: bars.time.slice(-DONE), open: bars.open.slice(-DONE), high: bars.high.slice(-DONE),
  low: bars.low.slice(-DONE), close: bars.close.slice(-DONE),
});

// The schedule over the whole order: past minutes and the ones still to come.
const time = [...bars.time.slice(-DONE), ...Array.from({ length: LEFT }, (_, j) => bars.time[n - 1] + (j + 1) * 60_000)];
const expected = time.map((_, i) => bars.volume[n - DONE - 1440 + i]); // the same minute yesterday
const total = expected.reduce((s, v) => s + v, 0);
let sum = 0;
const vwapPlan = expected.map((v) => (sum += v) / total * TARGET);
const twapPlan = time.map((_, i) => ((i + 1) / time.length) * TARGET);
let filled = 0;
const actual = fills.qty.map((q) => (filled += q));

const pane = chart.addPane(1.2);
chart.addSeries('line', { color: '#8a93a6', lineStyle: 'dashed', lineWidth: 1, pricePrecision: 0, lastValueVisible: false, priceLineVisible: false }, pane)
  .setData({ time, value: twapPlan });
const plan = chart.addSeries('line', { color: '#9b8cff', lineWidth: 1.5, pricePrecision: 0, priceLineVisible: false }, pane);
plan.setData({ time, value: vwapPlan });
const done = chart.addSeries('line', { color: '#e8ecf4', lineWidth: 2.5, pricePrecision: 0, priceLineVisible: false }, pane);
done.setData({ time: fills.time, value: actual });
// Green while ahead of the VWAP schedule, red while behind.
chart.addBand(done, plan, { fill: 'rgba(46, 230, 182, 0.25)', fillCrossed: 'rgba(255, 93, 115, 0.3)' });
done.createPriceLine({ price: TARGET, color: '#ffb547', lineStyle: 'dotted', title: 'target' });
${LEGEND}
legend(pane, [['filled', '#e8ecf4'], ['VWAP schedule', '#9b8cff'], ['TWAP schedule', '#8a93a6']]);
chart.setBarSpacing(3.3);
chart.scrollToRealtime();`,
      about: 'A five-hour buy order of 750 BTC, three hours in: cumulative fills against a VWAP schedule built from yesterday\'s volume curve and a straight TWAP line, both running to the end time. `addBand` shades green while the order is ahead of the VWAP schedule and red while behind.',
      api: ['addBand({ fillCrossed })', 'createPriceLine', 'addPane'],
    },
    {
      id: 'participation',
      title: 'Participation rate',
      group: 'Order execution',
      code: `${create()}
${bars({ interval: '1m', count: 300 })}
${load('fills', 'series/execFills', { symbol: 'BTCUSD', interval: '1m', count: 300 })}
chart.addCandlestickSeries().setData(bars);

// Our share of market volume over a rolling 20 minutes, in percent.
const W = 20, LOW = 7, HIGH = 13;
const rate = bars.time.map((_, i) => {
  if (i < W - 1) return NaN;
  let ours = 0, market = 0;
  for (let k = i - W + 1; k <= i; k++) { ours += fills.qty[k]; market += bars.volume[k]; }
  return (100 * ours) / market;
});
const pane = chart.addPane(0.8);
const guide = { color: '#5d6577', lineStyle: 'dashed', lineWidth: 1, pricePrecision: 1, lastValueVisible: false, priceLineVisible: false };
const high = chart.addSeries('line', guide, pane);
const low = chart.addSeries('line', guide, pane);
high.setData({ time: bars.time, value: bars.time.map(() => HIGH) });
low.setData({ time: bars.time, value: bars.time.map(() => LOW) });
chart.addBand(high, low, { fill: 'rgba(155, 140, 255, 0.12)' });
// The rate, with the stretches outside the 7–13% band around the 10% target in amber.
chart.addSeries('line', { color: '#9b8cff', lineWidth: 2, pricePrecision: 1 }, pane)
  .setData({ time: bars.time, value: rate });
const outside = (i) => rate[i] < LOW || rate[i] > HIGH;
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 1, lastValueVisible: false, priceLineVisible: false }, pane)
  .setData({ time: bars.time, value: rate.map((r, i) => (outside(i) || outside(i - 1) || outside(i + 1) ? r : NaN)) });
chart.setBarSpacing(3.4);
chart.scrollToRealtime();`,
      about: 'A volume-participation algorithm aiming at 10% of the market: its share of volume over a rolling 20 minutes, against a 7 to 13% tolerance band filled with `addBand`. A second line with gaps (`NaN`) traces the stretches outside the band in amber.',
      api: ['addBand', "addSeries('line')", 'NaN gaps'],
    },
    {
      id: 'queuepos',
      title: 'Queue position',
      group: 'Order execution',
      code: `${create()}
const to = Math.floor(Date.now() / 60_000) * 60_000, from = to - 10 * 60_000;
const range = \`symbol=BTCUSD&from=\${from}&to=\${to}\`;
const bars = await fetch(\`/api/bars?interval=1s&\${range}\`).then((r) => r.json());
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
// The book once a second, 60 levels of $0.50 a side.
const book = await fetch(\`/api/depth?\${range}&every=1000&levels=60\`).then((r) => r.json());
const candles = chart.addCandlestickSeries();
candles.setData(bars);

// A buy limit order placed 30 s in, at a whole dollar $15 under the last trade.
const placed = from + 30_000, first = (placed - book.time[0]) / 1000;
let j = trades.time.findIndex((t) => t >= placed);
const PRICE = Math.floor(trades.price[j - 1]) - 15;
const sizeAt = (i) => { const k = book.prices[i].indexOf(PRICE); return k < 0 ? Infinity : book.sizes[i][k]; };
// Ahead of us: the size resting at our price when we joined. Sells at our price
// eat into it; cancellations can only shrink it to what is still displayed.
let ahead = sizeAt(first), filledAt = null;
const queue = { time: [], value: [] };
for (let i = first; i < book.time.length && filledAt === null; i++) {
  for (; j < trades.time.length && trades.time[j] < book.time[i] + 1000; j++) {
    if (trades.side[j] < 0 && trades.price[j] === PRICE) ahead -= trades.size[j];
    if (trades.price[j] < PRICE || ahead <= 0) { ahead = 0; filledAt = trades.time[j]; break; }
  }
  ahead = Math.min(ahead, sizeAt(i));
  queue.time.push(book.time[i]);
  queue.value.push(Number.isFinite(ahead) ? ahead : NaN);
}
chart.addSeries('area', {
  color: '#9b8cff', topFill: 'rgba(155, 140, 255, 0.35)', bottomFill: 'rgba(155, 140, 255, 0.02)', step: true, pricePrecision: 2,
}, chart.addPane(0.5)).setData(queue);

candles.createPriceLine({ price: PRICE, color: '#9b8cff', lineStyle: 'dashed', title: 'our bid' });
const markers = [{ time: placed, position: 'aboveBar', shape: 'arrowDown', color: '#9b8cff', text: 'placed' }];
if (filledAt !== null) markers.push({ time: Math.floor(filledAt / 1000) * 1000, position: 'belowBar', shape: 'circle', color: '#2ee6b6', text: 'filled' });
candles.setMarkers(markers);
chart.setBarSpacing(1.7);
chart.scrollToRealtime();`,
      about: 'Where a resting bid stands in the queue at its price: the BTC ahead of it, worked out from one-second order-book snapshots and the trades at that price, shrinks step by step until the price trades through and the order fills. The limit price is a `createPriceLine` and markers show when it was placed and filled.',
      api: ['createPriceLine', 'Series.setMarkers', "addSeries('area', { step })"],
    },
    {
      id: 'slippageband',
      title: 'Slippage quantile band',
      group: 'Costs & slippage',
      code: `${create()}
// About 700 fills over three days; value: slippage against the arrival mid, bps.
${load('fills', 'events/execSlippage', { symbol: 'BTCUSD', interval: '1m', count: 4000 })}

// Rolling 10th, 50th and 90th percentiles over the last 50 fills.
const W = 50;
const quantile = (q) => fills.value.map((_, i) => {
  if (i < W - 1) return NaN;
  const s = fills.value.slice(i - W + 1, i + 1).sort((a, b) => a - b);
  return s[Math.round(q * (W - 1))];
});
const line = (value, options) => {
  const s = chart.addSeries('line', { lineWidth: 1, pricePrecision: 1, lastValueVisible: false, priceLineVisible: false, ...options });
  s.setData({ time: fills.time, value });
  return s;
};
const p90 = line(quantile(0.9), { color: '#9b8cff' });
const p10 = line(quantile(0.1), { color: '#9b8cff' });
chart.addBand(p90, p10, { fill: 'rgba(155, 140, 255, 0.16)' });
const median = line(quantile(0.5), { color: '#e8ecf4', lineWidth: 2, lastValueVisible: true });
median.createPriceLine({ price: 0, color: '#5d6577', lineStyle: 'dashed', title: 'arrival mid' });

// Each fill as a dot: red when it cost us, green when it improved on the mid.
const dots = (color, keep) => chart.addSeries('points', { color, lineWidth: 5, pricePrecision: 1, lastValueVisible: false, priceLineVisible: false })
  .setData({ time: fills.time, value: fills.value.map((v) => (keep(v) ? v : NaN)) });
dots('#ff5d73', (v) => v > 0);
dots('#2ee6b6', (v) => v <= 0);
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Every fill\'s slippage against the arrival mid in basis points, as red (cost) and green (improvement) points, with the rolling median and a 10th to 90th percentile band over the last 50 fills. The time axis has one slot per fill.',
      api: ["addSeries('points')", 'addBand', 'createPriceLine'],
    },
    {
      id: 'fillvs',
      title: 'Reference, quote & fill',
      group: 'Costs & slippage',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '5s', count: 360 })}
${load('spread', 'series/microSpread', { symbol: 'ETHUSD', interval: '5s', count: 360 })}
// Buy fills; value: fill price against the best ask, bps.
${load('fills', 'events/execQuoteFills', { symbol: 'ETHUSD', interval: '5s', count: 360 })}

const ask = bars.close.map((mid, i) => mid * (1 + spread.spread[i] / 20_000));
chart.addSeries('line', { color: '#8a93a6', lineWidth: 1, pricePrecision: 2, lastValueVisible: false, priceLineVisible: false })
  .setData({ time: bars.time, value: bars.close });
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.5, step: true, pricePrecision: 2, priceLineVisible: false })
  .setData({ time: bars.time, value: ask });

// Fill prices: green at or better than the quoted ask, red when worse.
const at = new Map(bars.time.map((t, i) => [t, i]));
const price = fills.time.map((t, i) => ask[at.get(t)] * (1 + fills.value[i] / 10_000));
const dots = (color, keep) => chart.addSeries('points', { color, lineWidth: 8, pricePrecision: 2, lastValueVisible: false, priceLineVisible: false })
  .setData({ time: fills.time, value: price.map((p, i) => (keep(fills.value[i]) ? p : NaN)) });
dots('#ff5d73', (v) => v > 0);
dots('#2ee6b6', (v) => v <= 0);
chart.addSeries('histogram', { pricePrecision: 2, priceLineVisible: false }, chart.addPane(0.4)).setData({
  time: fills.time, value: fills.value, color: fills.value.map((v) => (v > 0 ? '#ff5d73' : '#2ee6b6')),
});
${LEGEND}
legend(0, [['mid (reference)', '#8a93a6'], ['best ask (quote)', '#4cc9f0']]);
chart.setBarSpacing(7);
chart.scrollToRealtime();`,
      about: 'A taker algorithm\'s buy fills on five-second data: the mid as the reference price, the best ask it was quoted as a step line, and each fill as a point, green when it was at or inside the quote and red when it paid more. The pane shows each fill against the quote in basis points.',
      api: ["addSeries('points')", "addSeries('line', { step })", 'per-point color (histogram)'],
    },
    {
      id: 'sizeband',
      title: 'Price for your size',
      group: 'Costs & slippage',
      code: `${create()}
// An order-book snapshot every 5 s for 15 minutes: 80 levels of $1 a side, bids then asks.
const to = Math.floor(Date.now() / 60_000) * 60_000;
const book = await fetch(\`/api/depth?symbol=BTCUSD&from=\${to - 900_000}&to=\${to}&every=5000&levels=80&step=2\`).then((r) => r.json());

// Average price to buy or sell \`size\` BTC at once, walking the book from the touch.
function sweep(prices, sizes, size, side) {
  const n = prices.length / 2;
  let left = size, cost = 0;
  for (let k = 0; k < n && left > 0; k++) {
    const i = side > 0 ? n + k : n - 1 - k, take = Math.min(left, sizes[i]);
    cost += take * prices[i];
    left -= take;
  }
  return left > 0 ? NaN : cost / size;
}
const time = book.time;
const at = (size, side) => book.prices.map((p, i) => sweep(p, book.sizes[i], size, side));
const mid = book.prices.map((p) => (p[p.length / 2 - 1] + p[p.length / 2]) / 2);
const line = (value, color, width = 1) => {
  const s = chart.addSeries('line', { color, lineWidth: width, pricePrecision: 1, lastValueVisible: false, priceLineVisible: false });
  s.setData({ time, value });
  return s;
};
const buyBig = line(at(250, 1), '#ff5d73'), sellBig = line(at(250, -1), '#2ee6b6');
const buySmall = line(at(25, 1), '#ff9aa8'), sellSmall = line(at(25, -1), '#8af0d4');
chart.addBand(buyBig, sellBig, { fill: 'rgba(155, 140, 255, 0.12)' });
chart.addBand(buySmall, sellSmall, { fill: 'rgba(155, 140, 255, 0.22)' });
line(mid, '#e8ecf4', 1.5);

// Round-trip cost of 250 BTC, bps.
const buy = at(250, 1), sell = at(250, -1);
chart.addSeries('area', { color: '#9b8cff', topFill: 'rgba(155, 140, 255, 0.3)', bottomFill: 'rgba(155, 140, 255, 0.02)', pricePrecision: 1 }, chart.addPane(0.4))
  .setData({ time, value: mid.map((m, i) => ((buy[i] - sell[i]) / m) * 10_000) });
${LEGEND}
legend(0, [['buy / sell 25 BTC', '#ff9aa8'], ['buy / sell 250 BTC', '#ff5d73'], ['mid', '#e8ecf4']]);
chart.setBarSpacing(5.5);
chart.scrollToRealtime();`,
      about: 'What you would pay or receive for a market order of 25 or 250 BTC, worked out by sweeping each order-book snapshot from the touch: two nested bands around the mid, filled with `addBand`. The pane shows the round-trip cost of 250 BTC in basis points.',
      api: ['addBand', "addSeries('line')", "addSeries('area')"],
    },
    {
      id: 'tradeplot',
      title: 'Trades on the chart',
      group: 'Trade review',
      code: `${create()}
${bars({ interval: '15m', count: 220 })}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
chart.addIndicator(candles, 'sma', { n: 8 }).outputs()[0].applyOptions({ color: '#4cc9f0', lineWidth: 1 });
chart.addIndicator(candles, 'sma', { n: 21 }).outputs()[0].applyOptions({ color: '#ffb547', lineWidth: 1 });

// Long-only trades of an 8/21 moving-average cross, filled at the next bar's open.
const sma = (n) => bars.close.map((_, i) => (i < n - 1 ? NaN : bars.close.slice(i - n + 1, i + 1).reduce((a, b) => a + b) / n));
const fast = sma(8), slow = sma(21), trades = [];
let entry = null;
for (let i = 21; i < bars.time.length - 1; i++) {
  const up = fast[i] > slow[i] && fast[i - 1] <= slow[i - 1], down = fast[i] < slow[i] && fast[i - 1] >= slow[i - 1];
  if (up && !entry) entry = { from: i + 1, open: bars.open[i + 1] };
  else if (down && entry) { trades.push({ ...entry, to: i + 1, close: bars.open[i + 1] }); entry = null; }
}
candles.setMarkers(trades.flatMap((t) => {
  const pnl = (t.close / t.open - 1) * 100;
  return [
    { time: bars.time[t.from], position: 'belowBar', shape: 'arrowUp', color: '#4cc9f0', text: 'Buy' },
    { time: bars.time[t.to], position: 'aboveBar', shape: 'arrowDown', color: pnl >= 0 ? '#2ee6b6' : '#ff5d73', text: \`\${pnl >= 0 ? '+' : ''}\${pnl.toFixed(1)}%\` },
  ];
}));

// Each holding period: a box from entry to exit price and a line joining the fills.
chart.addLayer({
  pane: 0,
  position: 'belowSeries',
  build(ctx) {
    const out = [];
    for (const t of trades) {
      const x0 = ctx.timeToX(bars.time[t.from]), x1 = ctx.timeToX(bars.time[t.to]);
      const y0 = ctx.priceToY(t.open), y1 = ctx.priceToY(t.close);
      if (x0 === null || x1 === null || y0 === null || y1 === null) continue;
      const win = t.close >= t.open;
      out.push({ type: 'rect', x: x0, y: Math.min(y0, y1), width: x1 - x0, height: Math.max(1, Math.abs(y1 - y0)), color: win ? 'rgba(46, 230, 182, 0.1)' : 'rgba(255, 93, 115, 0.1)' });
      out.push({ type: 'line', x1: x0, y1: y0, x2: x1, y2: y1, color: win ? '#2ee6b6' : '#ff5d73', width: 1, dash: 'dashed' });
    }
    return out;
  },
});
chart.setBarSpacing(4.6);
chart.scrollToRealtime();`,
      about: 'The trades of a simple moving-average cross on 15-minute candles: buy and sell markers with each trade\'s result, and a layer plugin that shades every holding period from entry to exit price, green for winners and red for losers.',
      api: ['Series.setMarkers', 'addLayer', 'addIndicator'],
    },
    {
      id: 'signallatency',
      title: 'Signal-to-fill latency',
      group: 'Trade review',
      code: `${create()}
${bars({ interval: '1s', count: 600 })}
// Buy signals; value: seconds from the signal to the fill.
${load('signals', 'events/execBuySignals', { symbol: 'BTCUSD', interval: '1s', count: 600 })}
const line = chart.addSeries('line', { color: '#c9d1e0', lineWidth: 1.5 });
line.setData({ time: bars.time, value: bars.close });

// Each fill lands \`latency\` seconds after its signal; slippage is the move in between.
// Signals whose fill is still inside the data, one fill per second at most.
const at = new Map(bars.time.map((t, i) => [t, i]));
const fills = [];
signals.time.forEach((t, k) => {
  const i = at.get(t), j = i + Math.round(signals.value[k]);
  if (i === undefined || j >= bars.time.length || bars.time[j] <= (fills.at(-1)?.time ?? -Infinity)) return;
  fills.push({ signal: t, time: bars.time[j], latency: signals.value[k], bps: (bars.close[j] / bars.close[i] - 1) * 10_000 });
});
line.setMarkers(fills.flatMap((f) => [
  { time: f.signal, position: 'belowBar', shape: 'arrowUp', color: '#9b8cff' },
  { time: f.time, position: 'aboveBar', shape: 'circle', color: f.bps > 0 ? '#ff5d73' : '#2ee6b6', text: \`\${f.bps > 0 ? '+' : ''}\${f.bps.toFixed(1)}\` },
]));

// Panes: latency in seconds and slippage in bps, one dot per fill.
const dots = (pane, color, value) => chart.addSeries('points', { color, lineWidth: 7, pricePrecision: 1, priceLineVisible: false, lastValueVisible: false }, pane)
  .setData({ time: fills.map((f) => f.time), value });
dots(chart.addPane(0.4), '#9b8cff', fills.map((f) => f.latency));
const slipPane = chart.addPane(0.4);
dots(slipPane, '#ff5d73', fills.map((f) => (f.bps > 0 ? f.bps : NaN)));
dots(slipPane, '#2ee6b6', fills.map((f) => (f.bps <= 0 ? f.bps : NaN)));
chart.setBarSpacing(1.7);
chart.scrollToRealtime();`,
      about: 'Buy signals (violet arrows) and their fills (circles labelled with the slippage in bps) on one-second prices. The panes show each fill\'s delay in seconds and the price move it cost; slower fills tend to pay more when the market runs.',
      api: ['Series.setMarkers', "addSeries('points')", 'addPane'],
    },
    {
      id: 'feeerosion',
      title: 'Fee erosion',
      group: 'Trade review',
      code: `${create()}
// A year of daily strategy results: gross equity and turnover (x equity).
${load('daily', 'series/execStrategy', { symbol: 'BTCUSD', interval: '1d', count: 365 })}
const FEE = 0.0006; // 6 bps per unit of turnover

const gross = daily.grossEquity.map((v) => (v / daily.grossEquity[0]) * 100);
const net = [100], fees = [0];
for (let i = 1; i < gross.length; i++) {
  const paid = net[i - 1] * daily.turnover[i] * FEE;
  net.push(net[i - 1] * (gross[i] / gross[i - 1]) - paid);
  fees.push(fees[i - 1] + paid);
}
const grossLine = chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2, pricePrecision: 1 });
const netLine = chart.addSeries('line', { color: '#9b8cff', lineWidth: 2, pricePrecision: 1 });
grossLine.setData({ time: daily.time, value: gross });
netLine.setData({ time: daily.time, value: net });
chart.addBand(grossLine, netLine, { fill: 'rgba(255, 181, 71, 0.22)' });
const feePane = chart.addPane(0.35);
chart.addSeries('area', { color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.35)', bottomFill: 'rgba(255, 181, 71, 0.02)', pricePrecision: 1 }, feePane)
  .setData({ time: daily.time, value: fees });
${LEGEND}
legend(0, [['gross equity', '#4cc9f0'], ['net of fees', '#9b8cff'], ['fees paid', '#ffb547']]);
chart.setBarSpacing(2.7);
chart.scrollToRealtime();`,
      about: 'One year of a strategy indexed to 100, before and after trading fees of 6 bps per unit of turnover: `addBand` fills the widening gap in amber, and the pane shows the cumulative fees in percent of starting capital.',
      api: ['addBand', "addSeries('line')", "addSeries('area')"],
    },
    {
      id: 'xexspread',
      title: 'Cross-exchange spread',
      group: 'Costs & slippage',
      code: `${create()}
${bars({ interval: '1m', count: 240 })}
${load('venues', 'series/xexVenues', { symbol: 'BTCUSD', interval: '1m', count: 240 })}
const colors = { a: '#4cc9f0', b: '#ffb547', c: '#b388ff' };
const line = (value, color, pane) => chart.addSeries('line', { color, lineWidth: 1.5, pricePrecision: 1, priceLineVisible: false }, pane)
  .setData({ time: bars.time, value });
line(bars.close, colors.a);
line(venues.venueB, colors.b);
line(venues.venueC, colors.c);

// Each venue against Exchange A, bps.
const pane = chart.addPane(0.6);
const bps = (v) => v.map((p, i) => (p / bars.close[i] - 1) * 10_000);
line(bps(venues.venueB), colors.b, pane);
line(bps(venues.venueC), colors.c, pane);
chart.addSeries('line', { color: '#5d6577', lineWidth: 1, lineStyle: 'dashed', lastValueVisible: false, priceLineVisible: false }, pane)
  .setData({ time: bars.time, value: bars.time.map(() => 0) });
${LEGEND}
legend(0, [['Exchange A', colors.a], ['Exchange B', colors.b], ['Exchange C', colors.c]]);
chart.setBarSpacing(7);
chart.scrollToRealtime();`,
      about: 'One market quoted on three venues: the prices overlaid, and below them Exchange B and C against Exchange A in basis points, where a steady premium and a lagging venue stand out. Each venue is a `line` series on the shared time axis.',
      api: ["addSeries('line')", 'addPane'],
    },
  ],
};
