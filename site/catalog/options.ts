import { bars, create, load } from './build';
import { plotDemos } from './plots';
import type { Category } from './types';

const D = { symbol: 'ACME', interval: '1d' };
const B = { symbol: 'BTCUSD', interval: '1d' };

/** Black–Scholes pieces (no rates) shared by the strategy examples. */
const BS = `const ncdf = (x) => {
  const t = 1 / (1 + 0.2316419 * Math.abs(x));
  const p = 0.3989423 * Math.exp(-x * x / 2) * t * (0.3193815 + t * (-0.3565638 + t * (1.781478 + t * (-1.821256 + t * 1.330274))));
  return x > 0 ? 1 - p : p;
};
const npdf = (x) => 0.3989423 * Math.exp(-x * x / 2);
const d1 = (S, K, vol, T) => (Math.log(S / K) + vol * vol * T / 2) / (vol * Math.sqrt(T));`;

export const options: Category = {
  id: 'options',
  title: 'Options & volatility',
  kicker: '12',
  blurb: 'Implied against realised, skew, expected moves and dealer exposure in time.',
  accent: '#c084fc',
  demos: [
    {
      id: 'ivrv',
      title: 'Implied vs realised vol · IV rank',
      group: 'Volatility',
      code: `${create()}
${bars({ ...D, count: 500 })}
${load('iv', 'series/optAtmIv', { ...D, count: 500 })}
chart.addCandlestickSeries().setData(bars);
// 20-day realised volatility, annualised, in percent
const rv = bars.close.map((_, i) => {
  if (i < 20) return NaN;
  let s = 0;
  for (let j = i - 19; j <= i; j++) s += Math.log(bars.close[j] / bars.close[j - 1]) ** 2;
  return Math.sqrt((s / 20) * 252) * 100;
});
const volPane = chart.addPane(0.8);
const ivLine = chart.addSeries('line', { color: '#c084fc', lineWidth: 2 }, volPane);
ivLine.setData({ time: iv.time, value: iv.iv.map((v) => v * 100) });
const rvLine = chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.5 }, volPane);
rvLine.setData({ time: bars.time, value: rv });
chart.addBand(ivLine, rvLine, { fill: 'rgba(192, 132, 252, 0.18)', fillCrossed: 'rgba(76, 201, 240, 0.25)' });
// IV rank: today's IV within its one-year range, 0–100
const rank = iv.iv.map((v, i) => {
  if (i < 251) return NaN;
  const year = iv.iv.slice(i - 251, i + 1);
  const lo = Math.min(...year), hi = Math.max(...year);
  return ((v - lo) / (hi - lo)) * 100;
});
const rankSeries = chart.addSeries('area', { color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.25)', bottomFill: 'rgba(255, 181, 71, 0)', pricePrecision: 0 }, chart.addPane(0.5));
rankSeries.setData({ time: iv.time, value: rank });
for (const price of [20, 80]) rankSeries.createPriceLine({ price, color: '#8a93a6', lineStyle: 'dashed', title: String(price) });
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'At-the-money implied volatility against 20-day realised volatility, with `addBand` shading the gap: violet while options price more movement than the stock delivers. The bottom pane is IV rank, where today\'s IV sits in its one-year range.',
      api: ['addBand', "addSeries('area')", 'createPriceLine'],
    },
    {
      id: 'expmove',
      title: 'Expected move to expiry',
      group: 'Volatility',
      code: `${create()}
${bars({ ...D, count: 120 })}
${load('iv', 'series/optAtmIv', { ...D, count: 1 })}
${load('days', 'forecast', { ...D, ahead: 21, quantiles: 50 })} // the next 21 trading days
chart.addCandlestickSeries().setData(bars);
const n = bars.close.length, spot = bars.close[n - 1], vol = iv.iv[0];
const time = [bars.time[n - 1], ...days.time.slice(1)];
// One and two standard deviations: spot × IV × √(years to the date)
const band = (k) => time.map((t) => spot * k * vol * Math.sqrt((t - time[0]) / 31_557_600_000));
const line = (k, style, width) => {
  const s = chart.addSeries('line', { color: '#c084fc', lineWidth: width, lineStyle: style, lastValueVisible: false, priceLineVisible: false });
  s.setData({ time, value: band(k).map((m) => spot + m) });
  return s;
};
const up1 = line(1, 'solid', 2), down1 = line(-1, 'solid', 2);
line(2, 'dashed', 1);
line(-2, 'dashed', 1);
chart.addBand(up1, down1, { fill: 'rgba(192, 132, 252, 0.16)' });
const move = band(1)[time.length - 1];
up1.createPriceLine({ price: spot + move, color: '#c084fc', title: \`+\${move.toFixed(2)}\` });
down1.createPriceLine({ price: spot - move, color: '#c084fc', title: \`−\${move.toFixed(2)}\` });
chart.setBarSpacing(7);
chart.setRightOffset(3);
chart.scrollToRealtime();`,
      about: 'The move options price in by an expiry 21 trading days out: the stock price ± spot × IV × √t for one (filled with `addBand`) and two standard deviations (dashed). The lines run into the future; data after the last bar adds slots to the time axis.',
      api: ['addBand', 'future timestamps', 'createPriceLine'],
    },
    {
      id: 'rrfly',
      title: 'Risk reversal · butterfly',
      group: 'Volatility',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '1d', count: 400 })}
${load('skew', 'series/optSkew', { symbol: 'BTCUSD', interval: '1d', count: 400 })}
chart.addCandlestickSeries().setData(bars);
const pane = chart.addPane(0.8);
chart.addSeries('baseline', {
  baseValue: 0,
  color: '#2ee6b6', downColor: '#ff5d73',
  topFill: 'rgba(46, 230, 182, 0.22)', bottomFill: 'rgba(255, 93, 115, 0.22)',
}, pane).setData({ time: skew.time, value: skew.rr });
chart.addSeries('line', { color: '#c084fc', lineWidth: 2 }, pane).setData({ time: skew.time, value: skew.bf });
chart.setBarSpacing(5);
chart.scrollToRealtime();`,
      about: 'The 25-delta risk reversal (call minus put implied volatility) as a baseline series: red while puts trade over calls. The violet line is the butterfly, how much the wings cost over at-the-money. Both in volatility points.',
      api: ["addSeries('baseline')", "addSeries('line')", 'addPane'],
    },
    {
      id: 'pcr',
      title: 'Put/call ratio',
      group: 'Flow & positioning',
      code: `${create()}
${bars({ ...D, count: 300 })}
${load('pcr', 'series/optPutCall', { ...D, count: 300 })}
chart.addCandlestickSeries().setData(bars);
const pane = chart.addPane(0.7);
// Above 1 (more puts than calls traded) in red
chart.addSeries('baseline', {
  baseValue: 1,
  color: '#ff5d73', downColor: '#2ee6b6',
  topFill: 'rgba(255, 93, 115, 0.25)', bottomFill: 'rgba(46, 230, 182, 0.12)',
}, pane).setData({ time: pcr.time, value: pcr.ratio });
const avg = pcr.ratio.map((_, i) => (i < 9 ? NaN : pcr.ratio.slice(i - 9, i + 1).reduce((a, b) => a + b) / 10));
chart.addSeries('line', { color: '#ffb547', lineWidth: 2 }, pane).setData({ time: pcr.time, value: avg });
chart.setBarSpacing(6);
chart.scrollToRealtime();`,
      about: 'Daily put volume over call volume as a baseline series around 1, red when puts dominate, with its 10-day average in amber. Spikes tend to come with sell-offs.',
      api: ["addSeries('baseline')", 'baseValue'],
    },
    {
      id: 'optflow',
      title: 'Large option prints',
      group: 'Flow & positioning',
      code: `${create()}
${bars({ ...D, interval: '1h', count: 280 })}
${load('prints', 'events/optBlocks', { ...D, interval: '1h', count: 280 })}
chart.addCandlestickSeries().setData(bars);
const close = new Map(bars.time.map((t, i) => [t, bars.close[i]]));
const max = Math.max(...prints.value.map(Math.abs));
const bubbles = prints.time.map((time, i) => ({
  time,
  price: close.get(time),
  premium: Math.abs(prints.value[i]),
  call: prints.value[i] >= 0,
}));
const big = new Set([...bubbles].sort((a, b) => b.premium - a.premium).slice(0, 5));
chart.addLayer({
  pane: chart.mainPane, // a pane layer: time and price coordinates work
  build(ctx) {
    const out = [];
    for (const b of bubbles) {
      const x = ctx.timeToX(b.time), y = ctx.priceToY(b.price);
      if (x === null || y === null || x < ctx.rect.x - 30 || x > ctx.rect.x + ctx.rect.width + 30) continue;
      const r = 4 + 20 * Math.sqrt(b.premium / max);
      const ring = Array.from({ length: 24 }, (_, k) => [x + r * Math.cos(k * Math.PI / 12), y + r * Math.sin(k * Math.PI / 12)]);
      out.push({ type: 'polygon', points: ring, color: b.call ? 'rgba(46, 230, 182, 0.28)' : 'rgba(255, 93, 115, 0.28)' });
      out.push({ type: 'polyline', points: [...ring, ring[0]], color: b.call ? '#2ee6b6' : '#ff5d73', width: 1 });
      if (big.has(b)) {
        out.push({ type: 'text', x, y: y - r - 9, text: \`$\${(b.premium / 1e6).toFixed(1)}M \${b.call ? 'calls' : 'puts'}\`, color: '#e6e9ef', size: 11, align: 'center' });
      }
    }
    return out;
  },
});
chart.setBarSpacing(7);
chart.scrollToRealtime();`,
      about: 'Large option trades on one-hour candles: a layer plugin draws a bubble per print at the stock price when it traded, its area proportional to the premium, green for calls and red for puts. The five largest are labelled.',
      api: ['addLayer', 'ctx.timeToX', 'ctx.priceToY'],
    },
    {
      id: 'earnmove',
      title: 'Earnings: implied vs actual move',
      group: 'Flow & positioning',
      code: `${create({ maxBarSpacing: 90 })}
${bars({ ...D, count: 1000 })}
${load('earnings', 'events/optEarnings', { ...D, count: 1000 })}
const index = new Map(bars.time.map((t, i) => [t, i]));
// The reaction: close to close on the earnings day, percent
const actual = earnings.time.map((t) => {
  const i = index.get(t);
  return (bars.close[i] / bars.close[i - 1] - 1) * 100;
});
const implied = earnings.value; // the move options priced in, percent
chart.addSeries('histogram', { pricePrecision: 1, priceLineVisible: false }).setData({
  time: earnings.time,
  value: actual,
  color: actual.map((v, i) => (Math.abs(v) > implied[i] ? '#ffb547' : v >= 0 ? '#2ee6b6' : '#ff5d73')),
});
const markers = { color: '#c084fc', lineWidth: 9, lastValueVisible: false, priceLineVisible: false };
chart.addSeries('points', markers).setData({ time: earnings.time, value: implied });
chart.addSeries('points', markers).setData({ time: earnings.time, value: implied.map((v) => -v) });
chart.fitContent();`,
      about: 'One slot per earnings report: the actual one-day move as a histogram and the move options implied as violet points at ± that size. Amber bars beat the implied move; the time axis holds only the report dates.',
      api: ["addSeries('points')", "addSeries('histogram')", 'per-point color'],
    },
    {
      id: 'oitime',
      title: 'Open interest by strike',
      group: 'Exposure',
      code: `${create()}
${bars({ ...B, count: 60 })}
${load('oi', 'series/optOiStrikes', { ...B, count: 60 })}
// Nine strikes 2 500 apart around the average price
const mean = bars.close.reduce((a, b) => a + b) / bars.close.length;
const strikes = Array.from({ length: 9 }, (_, j) => Math.round(mean / 2500) * 2500 + 2500 * (j - 4));
const heatmap = chart.addHeatmap({
  source: 'depth', tickSize: 2500, columnMs: 86_400_000, scale: 'linear',
  colors: ['#14111f', '#3b2370', '#7b3fbf', '#c084fc', '#f3e8ff'],
});
oi.time.forEach((time, i) => heatmap.appendDepth({
  time,
  prices: strikes,
  sizes: strikes.map((_, j) => oi[\`call\${j + 1}\`][i] + oi[\`put\${j + 1}\`][i]),
}));
chart.addCandlestickSeries().setData(bars);
chart.setBarSpacing(16);
chart.scrollToRealtime();`,
      about: 'Calls plus puts open at each strike, day by day, as an `addHeatmap` fed with one column per day: rows are strikes and brighter cells hold more contracts. Calls build above a rising price and puts below a falling one.',
      api: ['addHeatmap', 'Series.appendDepth'],
    },
    {
      id: 'dex',
      title: 'Dealer gamma exposure',
      group: 'Exposure',
      code: `${create()}
${bars({ ...B, count: 60 })}
${load('oi', 'series/optOiStrikes', { ...B, count: 60 })}
${load('iv', 'series/optAtmIv', { ...B, count: 60 })}
${BS}
const mean = bars.close.reduce((a, b) => a + b) / bars.close.length;
const center = Math.round(mean / 2500) * 2500;
const strikes = Array.from({ length: 9 }, (_, j) => center + 2500 * (j - 4));
const step = 250, prices = Array.from({ length: 121 }, (_, k) => center - 15_000 + k * step);
// Dealers long calls and short puts, weekly expiry: their gamma in $ per 1% move at each price
const gex = (i, S) => strikes.reduce((sum, K, j) => {
  const T = 7 / 365, vol = iv.iv[i];
  const gamma = npdf(d1(S, K, vol, T)) / (S * vol * Math.sqrt(T));
  return sum + (oi[\`call\${j + 1}\`][i] - oi[\`put\${j + 1}\`][i]) * gamma * 100 * S * S * 0.01;
}, 0);
const opts = { source: 'depth', tickSize: step, columnMs: 86_400_000, scale: 'linear', range: { percentile: 1 } };
const long = chart.addHeatmap({ ...opts, colors: ['rgba(46, 230, 182, 0)', 'rgba(46, 230, 182, 0.1)', 'rgba(46, 230, 182, 0.2)', 'rgba(46, 230, 182, 0.32)', 'rgba(46, 230, 182, 0.45)'] });
const short = chart.addHeatmap({ ...opts, colors: ['rgba(255, 93, 115, 0)', 'rgba(255, 93, 115, 0.1)', 'rgba(255, 93, 115, 0.2)', 'rgba(255, 93, 115, 0.32)', 'rgba(255, 93, 115, 0.45)'] });
const flip = [];
oi.time.forEach((time, i) => {
  const g = prices.map((S) => gex(i, S));
  long.appendDepth({ time, prices, sizes: g.map((v) => Math.max(v, 0)) });
  short.appendDepth({ time, prices, sizes: g.map((v) => Math.max(-v, 0)) });
  const k = g.findIndex((v, j) => j > 0 && g[j - 1] < 0 && v >= 0); // where gamma turns positive
  flip.push(k > 0 ? prices[k] : NaN);
});
chart.addCandlestickSeries().setData(bars);
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, lineStyle: 'dashed', step: true }).setData({ time: oi.time, value: flip });
chart.setBarSpacing(16);
chart.scrollToRealtime();`,
      about: 'Dealer gamma at every price, day by day, from open interest by strike and implied volatility: green where dealers are long gamma and damp moves, red where they are short and chase them. Two `addHeatmap` series hold the two signs; the amber line is the gamma flip level.',
      api: ['addHeatmap', 'Series.appendDepth', 'heatmap colors'],
    },
    {
      id: 'pnlheat',
      title: 'Strategy P&L by price and date',
      group: 'Strategies',
      code: `${create()}
${bars({ ...B, count: 60 })}
${load('iv', 'series/optAtmIv', { ...B, count: 1 })}
${load('cone', 'forecast', { ...B, ahead: 30, quantiles: '5,95' })}
${BS}
const n = bars.close.length, spot = bars.close[n - 1], vol = iv.iv[0];
const call = (S, K, T) => (T <= 0 ? Math.max(S - K, 0) : S * ncdf(d1(S, K, vol, T)) - K * ncdf(d1(S, K, vol, T) - vol * Math.sqrt(T)));
const put = (S, K, T) => call(S, K, T) - S + K;
// An iron condor: short the 0.95/1.05 strangle, long the 0.90/1.10 wings
const k = (m) => Math.round((spot * m) / 500) * 500;
const value = (S, T) => call(S, k(1.1), T) - call(S, k(1.05), T) - put(S, k(0.95), T) + put(S, k(0.9), T);
const days = cone.time.slice(1);
const expiry = days[days.length - 1], years = (t) => (expiry - t) / 31_557_600_000;
const entry = value(spot, years(bars.time[n - 1]));
const prices = Array.from({ length: 161 }, (_, i) => spot * (0.76 + i * 0.003));
const opts = { source: 'depth', tickSize: spot * 0.003, columnMs: 86_400_000, scale: 'linear', range: { percentile: 1 } };
const gain = chart.addHeatmap({ ...opts, colors: ['rgba(46, 230, 182, 0)', '#0f3d33', '#17765f', '#22b38e', '#2ee6b6'] });
const loss = chart.addHeatmap({ ...opts, colors: ['rgba(255, 93, 115, 0)', '#4a1820', '#8c2b3a', '#cc4357', '#ff5d73'] });
for (const t of days) {
  const pnl = prices.map((S) => value(S, years(t)) - entry); // per condor of 1 BTC
  gain.appendDepth({ time: t, prices, sizes: pnl.map((v) => Math.max(v, 0)) });
  loss.appendDepth({ time: t, prices, sizes: pnl.map((v) => Math.max(-v, 0)) });
}
chart.addCandlestickSeries().setData(bars);
const edge = { color: '#8a93a6', lineStyle: 'dashed', lineWidth: 1, lastValueVisible: false, priceLineVisible: false };
chart.addSeries('line', edge).setData({ time: [bars.time[n - 1], ...days], value: [spot, ...cone.p5.slice(1)] });
chart.addSeries('line', edge).setData({ time: [bars.time[n - 1], ...days], value: [spot, ...cone.p95.slice(1)] });
chart.setBarSpacing(11);
chart.scrollToRealtime();`,
      about: 'What an iron condor on BTC opened today is worth on each future day at each price, priced with Black–Scholes at today\'s implied volatility: green for profit, red for loss. Two `addHeatmap` series hold the two signs; dashed lines are the 5–95% price range.',
      api: ['addHeatmap', 'future timestamps', '/api/forecast'],
    },
    {
      id: 'hedgereplay',
      title: 'Delta hedge replay',
      group: 'Strategies',
      code: `${create()}
${bars({ ...D, count: 60 })}
${load('iv', 'series/optAtmIv', { ...D, count: 60 })}
${BS}
// Short 10 at-the-money calls (1 000 shares) expiring 90 days after the first bar,
// hedged with stock at every close.
const K = Math.round(bars.close[0]), expiry = bars.time[0] + 90 * 86_400_000;
let held = 0, cash = 0;
const shares = [], pnl = [], markers = [];
bars.close.forEach((S, i) => {
  const T = (expiry - bars.time[i]) / 31_557_600_000;
  const target = Math.round(1000 * ncdf(d1(S, K, iv.iv[i], T)));
  const trade = target - held;
  if (trade !== 0) {
    markers.push(trade > 0
      ? { time: bars.time[i], position: 'belowBar', shape: 'arrowUp', color: '#2ee6b6', text: Math.abs(trade) >= 60 ? \`+\${trade}\` : '' }
      : { time: bars.time[i], position: 'aboveBar', shape: 'arrowDown', color: '#ff5d73', text: Math.abs(trade) >= 60 ? \`\${trade}\` : '' });
  }
  cash -= trade * S;
  held = target;
  shares.push(held);
  pnl.push(cash + held * S); // the hedge's mark-to-market
});
const candles = chart.addCandlestickSeries();
candles.setData(bars);
candles.setMarkers(markers);
candles.createPriceLine({ price: K, color: '#c084fc', lineStyle: 'dashed', title: 'Strike' });
chart.addSeries('line', { color: '#4cc9f0', step: true, lineWidth: 2, pricePrecision: 0 }, chart.addPane(0.5))
  .setData({ time: bars.time, value: shares });
chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', pricePrecision: 0 }, chart.addPane(0.5))
  .setData({ time: bars.time, value: pnl });
chart.setBarSpacing(16);
chart.scrollToRealtime();`,
      about: 'Delta hedging a short call position, replayed: markers are the daily stock trades that keep it delta neutral, the middle pane the shares held (step line) and the bottom pane the hedge\'s running P&L. Deltas come from Black–Scholes in a few lines of JavaScript.',
      api: ['Series.setMarkers', 'step lines', "addSeries('baseline')"],
    },
    {
      id: 'gammapnl',
      title: 'Gamma P&L attribution',
      group: 'Strategies',
      code: `${create()}
${bars({ ...D, count: 70 })}
${load('iv', 'series/optAtmIv', { ...D, count: 70 })}
${BS}
// Long 10 at-the-money calls, delta hedged daily: what each Greek earned per day
const K = Math.round(bars.close[0]), expiry = bars.time[0] + 110 * 86_400_000, dt = 1 / 365;
const parts = { gamma: [], theta: [], vega: [] };
for (let i = 1; i < bars.close.length; i++) {
  const S = bars.close[i - 1], vol = iv.iv[i - 1], T = (expiry - bars.time[i - 1]) / 31_557_600_000;
  const x = d1(S, K, vol, T), dS = bars.close[i] - S;
  const gamma = npdf(x) / (S * vol * Math.sqrt(T)), vega = S * npdf(x) * Math.sqrt(T);
  parts.gamma.push(0.5 * gamma * dS * dS * 1000);
  parts.theta.push((-S * npdf(x) * vol) / (2 * Math.sqrt(T)) * dt * 1000);
  parts.vega.push(vega * (iv.iv[i] - vol) * 1000);
}
const time = bars.time.slice(1);
const bar = (color, pane) => chart.addSeries('histogram', { color, pricePrecision: 0, priceLineVisible: false }, pane);
chart.addCandlestickSeries().setData(bars);
// Long options: gamma always earns, theta always costs, so they share a pane
const decay = chart.addPane(0.7);
bar('#2ee6b6', decay).setData({ time, value: parts.gamma });
bar('#ff5d73', decay).setData({ time, value: parts.theta });
bar('#c084fc', chart.addPane(0.7)).setData({ time, value: parts.vega });
const totals = chart.addPane(0.7);
for (const [name, color] of [['gamma', '#2ee6b6'], ['theta', '#ff5d73'], ['vega', '#c084fc']]) {
  let sum = 0;
  chart.addSeries('line', { color, lineWidth: 1.5, pricePrecision: 0 }, totals).setData({ time, value: parts[name].map((x) => (sum += x)) });
}
chart.setBarSpacing(13);
chart.scrollToRealtime();`,
      about: 'A delta-hedged long call position\'s daily P&L split by Greek: what gamma earns (green) against what theta costs (red) in one pane, vega (violet) in the next, and the running totals at the bottom. The Greeks are a few lines of Black–Scholes.',
      api: ["addSeries('histogram')", 'addPane', "addSeries('line')"],
    },
    ...plotDemos.options,
  ],
};
