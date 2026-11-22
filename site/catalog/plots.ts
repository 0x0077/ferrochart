// The FerroPlot examples: charts whose x axis
// is not time. 25 of them, covering every mark (line, area, scatter, bars,
// waterfall, cells, rules, labels) and every axis (numeric, log, category,
// date, two y axes, a price axis shared with candles). Each category file
// adds its own list after its chart examples.

import { bars, create, load, plot } from './build';
import type { Demo } from './types';

const B = { symbol: 'BTCUSD', interval: '1d' };
const D = { symbol: 'ACME', interval: '1d' };

/** Black–Scholes pieces (no rates). */
const BS = `const ncdf = (x) => {
  const t = 1 / (1 + 0.2316419 * Math.abs(x));
  const p = 0.3989423 * Math.exp(-x * x / 2) * t * (0.3193815 + t * (-0.3565638 + t * (1.781478 + t * (-1.821256 + t * 1.330274))));
  return x > 0 ? 1 - p : p;
};
const npdf = (x) => 0.3989423 * Math.exp(-x * x / 2);
const d1 = (S, K, vol, T) => (Math.log(S / K) + vol * vol * T / 2) / (vol * Math.sqrt(T));`;

/** A 30-day smile from ATM vol and the 25-delta risk reversal and butterfly. */
const SMILE = `// Moneyness in 25-delta units (≈ 0.67 σ√T); the smile is ATM + RR·m/2 + BF·m², vol points
const smileAt = (atm, rr, bf, S, T) => (K) => {
  const m = Math.log(K / S) / (0.67 * atm * Math.sqrt(T));
  return atm + (rr * m / 2 + bf * m * m) / 100;
};`;

/** Daily log returns of `name`'s closes. */
const RETURNS = `const returns = (b) => b.close.slice(1).map((c, i) => Math.log(c / b.close[i]));`;

const MONTHS = "['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']";

const options: Demo[] = [
  {
    id: 'smile',
    title: 'Volatility smile',
    group: 'Smile & surface',
    code: `${plot()}
${load('iv', 'series/optAtmIv', { ...B, count: 31 })}
${load('skew', 'series/optSkew', { ...B, count: 31 })}
${SMILE}
const strikes = Array.from({ length: 61 }, (_, i) => 70 + i); // % of spot
const curve = (i) => {
  const vol = smileAt(iv.iv[i], skew.rr[i], skew.bf[i], 100, 30 / 365);
  return { x: strikes, y: strikes.map((k) => vol(k) * 100) };
};
const last = iv.iv.length - 1;
plot.addMark('line', { color: '#5b6275', lineWidth: 1.5, lineStyle: 'dashed' }).setData(curve(0));
plot.addMark('line', { color: '#c084fc', lineWidth: 2.5 }).setData(curve(last));
plot.addMark('scatter', { color: '#f3e8ff', pointSize: 9 }).setData({ x: [100], y: [iv.iv[last] * 100], label: ['ATM'] });
plot.addLabel({ x: 72, y: curve(0).y[2], text: 'a month ago', color: '#8a93a6', align: 'left', offset: [0, -14] });
plot.axis('x').applyOptions({ title: 'Strike, % of spot', format: { type: 'fixed', decimals: 0 } });
plot.axis('right').applyOptions({ title: '30-day implied vol', format: { type: 'percent', decimals: 0 } });`,
    about: 'Implied volatility across strikes for 30-day BTC options, built from at-the-money vol, the 25-delta risk reversal and the butterfly: today in violet, a month ago dashed. A numeric x axis (strike as a percentage of spot) and line marks, with a labelled scatter point at the money.',
    api: ['FerroPlot.create', "addMark('line')", "addMark('scatter')", 'addLabel'],
  },
  {
    id: 'payoff',
    title: 'Option payoff · long straddle',
    group: 'Smile & surface',
    code: `${plot()}
${bars({ ...B, count: 1 })}
${load('iv', 'series/optAtmIv', { ...B, count: 1 })}
${BS}
const spot = bars.close[0], vol = iv.iv[0], K = Math.round(spot / 1000) * 1000, T = 30 / 365;
const call = (S, t) => (t <= 0 ? Math.max(S - K, 0) : S * ncdf(d1(S, K, vol, t)) - K * ncdf(d1(S, K, vol, t) - vol * Math.sqrt(t)));
const put = (S, t) => call(S, t) - S + K;
const cost = call(spot, T) + put(spot, T); // a call and a put at K, 30 days out
const x = Array.from({ length: 241 }, (_, i) => spot * (0.7 + i * 0.0025));
// At expiry: green above zero, red below
plot.addMark('area', { color: '#2ee6b6', downColor: '#ff5d73', baseValue: 0, fillAlpha: 0.22, lineWidth: 2 })
  .setData({ x, y: x.map((S) => call(S, 0) + put(S, 0) - cost) });
// Halfway to expiry
plot.addMark('line', { color: '#ffb547', lineWidth: 2 }).setData({ x, y: x.map((S) => call(S, T / 2) + put(S, T / 2) - cost) });
plot.addRule({ axis: 'x', value: spot, label: 'spot', lineStyle: 'dashed' });
plot.addRule({ value: 0, color: '#8a93a6' });
plot.axis('x').applyOptions({ title: 'BTC price', format: 'compact' });
plot.axis('right').applyOptions({ title: 'P&L per straddle, $', format: 'compact' });`,
    about: 'Profit and loss of a 30-day at-the-money BTC straddle against the price: at expiry as an area that is green above zero and red below (`downColor` under `baseValue`), and halfway to expiry in amber, priced with Black–Scholes. Rules mark the spot price and zero.',
    api: ["addMark('area')", 'downColor', 'addRule'],
  },
  {
    id: 'greeks',
    title: 'Greeks across the price',
    group: 'Smile & surface',
    code: `${plot()}
${bars({ ...B, count: 1 })}
${load('iv', 'series/optAtmIv', { ...B, count: 1 })}
${BS}
const spot = bars.close[0], vol = iv.iv[0], K = Math.round(spot / 1000) * 1000, T = 21 / 365;
const x = Array.from({ length: 201 }, (_, i) => spot * (0.75 + i * 0.0025));
const delta = x.map((S) => ncdf(d1(S, K, vol, T)));
// Gamma in delta per 1% move; theta in $ per day
const gamma = x.map((S) => npdf(d1(S, K, vol, T)) / (S * vol * Math.sqrt(T)) * S * 0.01);
const theta = x.map((S) => -S * npdf(d1(S, K, vol, T)) * vol / (2 * Math.sqrt(T)) / 365);
const thetaScaled = theta.map((t) => t / Math.min(...theta));
plot.axis('left').applyOptions({ visible: true, title: 'Delta', range: [0, 1] });
plot.addMark('line', { yAxis: 'left', color: '#4cc9f0', lineWidth: 2.5 }).setData({ x, y: delta });
plot.addMark('area', { color: '#c084fc', fillAlpha: 0.2 }).setData({ x, y: gamma.map((g) => g * 100) });
plot.axis('right').applyOptions({ title: 'Gamma, delta per 1% move', format: { type: 'percent', decimals: 1 } });
plot.addMark('line', { yAxis: 'left', color: '#ff5d73', lineWidth: 1.5, lineStyle: 'dotted' })
  .setData({ x, y: thetaScaled });
plot.addRule({ axis: 'x', value: K, label: 'strike', lineStyle: 'dashed' });
plot.addLabel({ x: x[180], y: delta[180], yAxis: 'left', text: 'delta', color: '#4cc9f0', offset: [6, 12] });
plot.addLabel({ x: x[40], y: thetaScaled[40], yAxis: 'left', text: 'theta (scaled)', color: '#ff5d73', align: 'right', offset: [-6, -10] });
plot.axis('x').applyOptions({ title: 'BTC price', format: 'compact' });`,
    about: 'Delta, gamma and theta of a three-week BTC call against the underlying price. Delta (blue) and scaled theta (dotted) use the left axis; gamma (violet area) uses the right one: two y axes on one plot.',
    api: ["axis('left')", 'yAxis', "addMark('area')"],
  },
  {
    id: 'volcone',
    title: 'Volatility cone',
    group: 'Smile & surface',
    code: `${plot()}
${bars({ ...B, count: 1500 })}
${load('iv', 'series/optAtmIv', { ...B, count: 1 })}
${RETURNS}
const r = returns(bars);
const horizons = [10, 20, 30, 60, 90, 120, 180, 252];
const quantile = (a, q) => { const s = [...a].sort((u, v) => u - v); return s[Math.floor(q * (s.length - 1))]; };
// Realised vol over each window length, every day: its range, quartiles and today's value
const cone = horizons.map((h) => {
  const vols = [];
  for (let i = h; i <= r.length; i++) {
    let s = 0;
    for (let j = i - h; j < i; j++) s += r[j] * r[j];
    vols.push(Math.sqrt((s / h) * 365) * 100);
  }
  return { lo: quantile(vols, 0.05), q1: quantile(vols, 0.25), mid: quantile(vols, 0.5), q3: quantile(vols, 0.75), hi: quantile(vols, 0.95), now: vols[vols.length - 1] };
});
const col = (k) => cone.map((c) => c[k]);
plot.addMark('area', { color: '#c084fc', fillAlpha: 0.14, lineWidth: 0.5 }).setData({ x: horizons, y: col('hi'), y0: col('lo') });
plot.addMark('area', { color: '#c084fc', fillAlpha: 0.28, lineWidth: 0.5 }).setData({ x: horizons, y: col('q3'), y0: col('q1') });
plot.addMark('line', { color: '#c084fc', lineWidth: 1.5, lineStyle: 'dashed' }).setData({ x: horizons, y: col('mid') });
plot.addMark('line', { color: '#ffb547', lineWidth: 2.5, icon: 'circle', pointSize: 7 }).setData({ x: horizons, y: col('now') });
plot.addRule({ value: iv.iv[0] * 100, label: 'IV', color: '#4cc9f0', lineStyle: 'dashed' });
plot.axis('x').applyOptions({ title: 'Window, days', format: { type: 'fixed', decimals: 0 } });
plot.axis('right').applyOptions({ title: 'Realised vol', format: { type: 'percent', decimals: 0 } });`,
    about: 'How volatile BTC has been over windows of 10 to 252 days: bands from the 5th to 95th and 25th to 75th percentiles of four years of history (areas with a lower edge, `y0`), the median dashed and today in amber. The rule is current 30-day implied vol, to compare with what has been realised.',
    api: ['y0', "addMark('area')", 'addRule'],
  },
  {
    id: 'impliedpdf',
    title: 'Implied price distribution',
    group: 'Smile & surface',
    side: true,
    code: `${create()}
${bars({ ...B, count: 180 })}
${load('iv', 'series/optAtmIv', { ...B, count: 1 })}
${load('skew', 'series/optSkew', { ...B, count: 1 })}
${BS}
${SMILE}
chart.addCandlestickSeries().setData(bars);
chart.setBarSpacing(5);
chart.scrollToRealtime();
// The distribution beside the candles, on the chart's price scale
${plot('side')}
plot.axis('right').applyOptions({ visible: false });
chart.attachSidePlot(plot);
const S = bars.close[bars.close.length - 1], T = 30 / 365;
const vol = smileAt(iv.iv[0], skew.rr[0], skew.bf[0], S, T);
const call = (K) => { const v = vol(K), d = d1(S, K, v, T); return S * ncdf(d) - K * ncdf(d - v * Math.sqrt(T)); };
// Breeden–Litzenberger: the density of the price at expiry is ∂²C/∂K²
const step = S * 0.005, prices = [], prob = [];
for (let K = S * 0.7; K <= S * 1.3; K += step) {
  prices.push(K);
  prob.push(Math.max((call(K + step) - 2 * call(K) + call(K - step)) / step, 0) * 100); // % per bucket
}
plot.addMark('bars', { orientation: 'horizontal', color: '#c084fc', band: 0.85 }).setData({ x: prob, y: prices });
plot.axis('x').applyOptions({ title: 'Probability, 30 days', format: { type: 'percent', decimals: 1 } });`,
    about: 'Where options say BTC will be in 30 days: the risk-neutral density from the smile (Breeden–Litzenberger), as horizontal bars in a FerroPlot beside the candles. `attachSidePlot` keeps its price axis lined up with the chart\'s while you pan and zoom.',
    api: ['attachSidePlot', "orientation: 'horizontal'", 'FerroPlot'],
  },
  {
    id: 'gex',
    title: 'Gamma exposure by strike',
    group: 'Exposure',
    code: `${plot()}
${bars({ ...B, count: 60 })}
${load('oi', 'series/optOiStrikes', { ...B, count: 60 })}
${load('iv', 'series/optAtmIv', { ...B, count: 1 })}
${BS}
const mean = bars.close.reduce((a, b) => a + b) / bars.close.length;
const strikes = Array.from({ length: 9 }, (_, j) => Math.round(mean / 2500) * 2500 + 2500 * (j - 4));
const S = bars.close[bars.close.length - 1], vol = iv.iv[0], T = 7 / 365, last = oi.time.length - 1;
// Dealers long the calls and short the puts: $ gamma per 1% move at each strike
const gex = strikes.map((K, j) => {
  const gamma = npdf(d1(S, K, vol, T)) / (S * vol * Math.sqrt(T));
  return (oi['call' + (j + 1)][last] - oi['put' + (j + 1)][last]) * gamma * S * S * 0.01;
});
plot.axis('x').applyOptions({ type: 'category', title: 'Strike' });
plot.axis('x').setCategories(strikes.map((k) => (k / 1000).toFixed(1) + 'k'));
plot.addMark('bars', { showValues: true, valueFormat: 'compact' }).setData({
  x: strikes.map((_, j) => j),
  y: gex,
  color: gex.map((g) => (g >= 0 ? '#2ee6b6' : '#ff5d73')),
});
plot.addRule({ value: 0, color: '#8a93a6' });
// Strikes are 2500 apart: the spot's position on the category axis (kept on the grid).
const at = Math.min(strikes.length - 1, Math.max(0, (S - strikes[0]) / 2500));
plot.addRule({ axis: 'x', value: at, label: 'spot', lineStyle: 'dashed' });
plot.axis('right').applyOptions({ title: 'Gamma, $ per 1% move', format: 'compact', includeZero: true });`,
    about: 'Dealer gamma exposure at each of nine BTC strikes: positive (green) where dealers are long gamma and damp moves, negative (red) where they chase them. Bars on a category axis with per-bar colours and value labels; a vertical rule marks the spot price between strikes.',
    api: ["type: 'category'", 'setCategories', "addMark('bars')", 'showValues'],
  },
  {
    id: 'oichain',
    title: 'Open interest by strike',
    group: 'Exposure',
    code: `${plot()}
${bars({ ...B, count: 60 })}
${load('oi', 'series/optOiStrikes', { ...B, count: 60 })}
const mean = bars.close.reduce((a, b) => a + b) / bars.close.length;
const strikes = Array.from({ length: 9 }, (_, j) => Math.round(mean / 2500) * 2500 + 2500 * (j - 4));
const last = oi.time.length - 1, x = strikes.map((_, j) => j);
const col = (side) => strikes.map((_, j) => oi[side + (j + 1)][last]);
plot.axis('x').applyOptions({ type: 'category', title: 'Strike' });
plot.axis('x').setCategories(strikes.map((k) => (k / 1000).toFixed(1) + 'k'));
// One group: the two marks share each strike's slot side by side
plot.addMark('bars', { group: 1, color: '#2ee6b6' }).setData({ x, y: col('call') });
plot.addMark('bars', { group: 1, color: '#ff5d73' }).setData({ x, y: col('put') });
// Max pain: the strike where option holders lose the most
const pain = strikes.map((P) => strikes.reduce((s, K, j) => s + col('call')[j] * Math.max(P - K, 0) + col('put')[j] * Math.max(K - P, 0), 0));
const maxPain = pain.indexOf(Math.min(...pain));
plot.addRule({ axis: 'x', value: maxPain, label: 'max pain', color: '#ffb547', lineStyle: 'dashed' });
plot.addLabel({ x: 8, y: col('call')[8], text: 'calls', color: '#2ee6b6', align: 'right', offset: [-8, -10] });
plot.addLabel({ x: 0, y: col('put')[0], text: 'puts', color: '#ff5d73', align: 'left', offset: [8, -10] });
plot.axis('right').applyOptions({ title: 'Contracts', format: 'compact' });`,
    about: 'Call and put open interest at each BTC strike as grouped bars (marks with the same `group` sit side by side), with the max-pain strike, where option buyers would lose the most at expiry, as a rule.',
    api: ['group', "addMark('bars')", 'addRule'],
  },
];

const rates: Demo[] = [
  {
    id: 'yield',
    title: 'Treasury yield curve',
    group: 'Curves',
    code: `${plot()}
${load('f', 'series/ustCurveFactors', { symbol: 'US10Y', interval: '1d', count: 64 })}
// A Nelson–Siegel curve through the 2y, 5y and 10y yields the factors give
const ns = (t) => [1, (1 - Math.exp(-0.6 * t)) / (0.6 * t), (1 - Math.exp(-0.6 * t)) / (0.6 * t) - Math.exp(-0.6 * t)];
const det = (m) => m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
const curve = (i) => {
  const y10 = f.level[i], y2 = y10 - f.slope[i] / 100, y5 = (y2 + y10 + f.curvature[i] / 100) / 2;
  const A = [ns(2), ns(5), ns(10)], b = [y2, y5, y10];
  // Cramer's rule for the three loadings
  const c = [0, 1, 2].map((k) => det(A.map((row, r) => row.map((v, j) => (j === k ? b[r] : v)))) / det(A));
  return (t) => c[0] * ns(t)[0] + c[1] * ns(t)[1] + c[2] * ns(t)[2];
};
const tenors = [0.25, 0.5, 1, 2, 3, 5, 7, 10, 20, 30];
const fine = Array.from({ length: 80 }, (_, i) => 0.25 * Math.pow(120, i / 79));
const now = curve(f.level.length - 1), before = curve(0);
plot.axis('x').applyOptions({ type: 'log', title: 'Maturity, years' });
plot.addMark('line', { color: '#5b6275', lineWidth: 1.5, lineStyle: 'dashed' }).setData({ x: fine, y: fine.map(before) });
plot.addMark('line', { color: '#4cc9f0', lineWidth: 2.5 }).setData({ x: fine, y: fine.map(now) });
plot.addMark('scatter', { color: '#e6f6ff', pointSize: 7 }).setData({
  x: tenors, y: tenors.map(now), label: ['3M', '6M', '1Y', '2Y', '3Y', '5Y', '7Y', '10Y', '20Y', '30Y'],
});
plot.addLabel({ x: 30, y: before(30), text: 'three months ago', color: '#8a93a6', align: 'right', offset: [0, 14] });
plot.axis('right').applyOptions({ title: 'Yield', format: { type: 'percent', decimals: 2 } });`,
    about: 'The Treasury curve today and three months ago, fitted with Nelson–Siegel through the 2-, 5- and 10-year yields. Maturities sit on a log axis, so the short end gets room; the quoted tenors are labelled scatter points.',
    api: ["type: 'log'", "addMark('line')", 'scatter labels'],
  },
  {
    id: 'krd',
    title: 'Key rate durations',
    group: 'Curves',
    code: `${plot()}
// A bond portfolio and its benchmark: (years to maturity, weight)
const portfolio = [[1.5, 0.1], [3, 0.15], [6, 0.25], [9, 0.2], [14, 0.1], [24, 0.2]];
const benchmark = [[1, 0.15], [2.5, 0.2], [5, 0.25], [8, 0.2], [12, 0.1], [28, 0.1]];
const keys = [0.25, 2, 5, 10, 30];
// Each bond's duration (≈ maturity / 1.04) goes to the two key rates around its maturity
const krd = (book) => keys.map((k, i) => book.reduce((s, [m, w]) => {
  const lo = keys[i - 1] ?? -Infinity, hi = keys[i + 1] ?? Infinity;
  const share = m <= k ? (m <= lo ? 0 : i === 0 ? 1 : (m - lo) / (k - lo)) : (m >= hi ? 0 : (hi - m) / (hi - k));
  return s + w * (m / 1.04) * share;
}, 0));
plot.axis('right').applyOptions({ type: 'category', inverted: true });
plot.axis('right').setCategories(['3M', '2Y', '5Y', '10Y', '30Y']);
const y = keys.map((_, i) => i);
plot.addMark('bars', { orientation: 'horizontal', group: 1, color: '#4cc9f0', showValues: true, valueFormat: { type: 'fixed', decimals: 2 } })
  .setData({ x: krd(portfolio), y });
plot.addMark('bars', { orientation: 'horizontal', group: 1, color: '#5b6275' }).setData({ x: krd(benchmark), y });
plot.addLabel({ x: krd(portfolio)[4], y: 4, text: 'portfolio', color: '#4cc9f0', offset: [6, -14] });
plot.addLabel({ x: krd(benchmark)[4], y: 4, text: 'benchmark', color: '#8a93a6', offset: [6, 14] });
plot.axis('x').applyOptions({ title: 'Duration, years', includeZero: true, format: { type: 'fixed', decimals: 1 } });`,
    about: 'How a bond portfolio\'s interest-rate risk is spread along the curve against its benchmark: the duration attributed to each key rate, as horizontal grouped bars on a category axis (`orientation: \'horizontal\'`).',
    api: ["orientation: 'horizontal'", 'group', 'inverted'],
  },
  {
    id: 'dotplot',
    title: 'Policy rate projections (dot plot)',
    group: 'Curves',
    code: `${plot()}
${load('rate', 'bars', { symbol: 'US10Y', interval: '1d', count: 1 })}
// Nineteen projections per year: rates in 0.125-point steps around a path
// that starts near today's 10-year yield and settles at the longer-run rate.
const years = ['2026', '2027', '2028', 'Longer run'];
const now = Math.round(rate.close[0] * 8) / 8;
const path = [now - 0.25, now - 0.75, now - 1.0, now - 1.125];
const x = [], y = [], size = [], label = [], medians = [];
years.forEach((_, i) => {
  const dots = Array.from({ length: 19 }, (_, j) => Math.round((path[i] + Math.sin(j * 2.3 + i) * (0.35 + i * 0.1)) * 8) / 8);
  const counts = {};
  for (const d of dots) counts[d] = (counts[d] ?? 0) + 1;
  for (const [r, n] of Object.entries(counts)) {
    x.push(i); y.push(Number(r)); size.push(6 + 3 * n); label.push(n > 1 ? String(n) : null);
  }
  medians.push([...dots].sort((a, b) => a - b)[9]);
});
plot.axis('x').applyOptions({ type: 'category' });
plot.axis('x').setCategories(years);
plot.addMark('scatter', { color: '#4cc9f0' }).setData({ x, y, size, label });
plot.addMark('line', { color: '#ffb547', lineWidth: 2, lineStyle: 'dashed', icon: 'square', pointSize: 8 })
  .setData({ x: [0, 1, 2, 3], y: medians });
plot.addRule({ value: now, label: 'now', color: '#8a93a6', lineStyle: 'dotted' });
plot.axis('right').applyOptions({ title: 'Policy rate, end of year', format: { type: 'percent', decimals: 2 } });`,
    about: 'A dot plot of nineteen policy makers\' rate projections per year. Each scatter point is one rate, sized and labelled by how many chose it; the amber line is the median path. Scatter on a category x axis with per-point sizes (`size`) and labels.',
    api: ['size', 'label', "type: 'category'"],
  },
];

const commodities: Demo[] = [
  {
    id: 'term',
    title: 'Futures term structure',
    group: 'Curves',
    code: `${plot()}
${bars({ symbol: 'CL', interval: '1d', count: 31 })}
${load('basis', 'series/clBasis', { symbol: 'CL', interval: '1d', count: 31 })}
// Twelve monthly contracts: the front month is spot plus the basis and the
// slope carries on along the curve, flattening further out.
const DAY = 86_400_000;
const curve = (i) => {
  const t0 = bars.time[i], spot = bars.close[i], b = basis.basis[i];
  const x = [], y = [];
  for (let k = 1; k <= 12; k++) {
    const d = new Date(t0);
    x.push(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + k, 20));
    y.push(spot + b * (1 + 1.6 * Math.log(k)));
  }
  return { x, y };
};
const last = bars.time.length - 1, now = curve(last), before = curve(0);
plot.axis('x').applyOptions({ format: 'date', title: 'Contract expiry' });
plot.addMark('line', { color: '#5b6275', lineWidth: 1.5, lineStyle: 'dashed', icon: 'circle', pointSize: 5 })
  .setData({ x: before.x.map((t) => t - 30 * DAY), y: before.y });
plot.addMark('line', { color: '#ffb547', lineWidth: 2.5, icon: 'circle', pointSize: 7 }).setData(now);
plot.addRule({ value: bars.close[last], label: 'spot', color: '#8a93a6', lineStyle: 'dotted' });
plot.addLabel({ x: now.x[11], y: now.y[11], text: basis.basis[last] < 0 ? 'backwardation' : 'contango', color: '#ffb547', align: 'right', offset: [0, -14] });
plot.axis('right').applyOptions({ title: 'USD per barrel', format: { type: 'fixed', decimals: 2 } });`,
    about: 'Crude oil futures by expiry month today (amber) and a month ago (dashed): downward sloping in backwardation, upward in contango. The x axis holds dates (UTC milliseconds with a `date` format) but is not a time series: each point is a different contract.',
    api: ["format: 'date'", "addMark('line')", 'icon'],
  },
];

const quant: Demo[] = [
  {
    id: 'distrib',
    title: 'Return distribution',
    group: 'Risk',
    code: `${plot()}
${bars({ ...B, count: 1500 })}
${RETURNS}
const r = returns(bars).map((v) => v * 100);
const n = r.length, mean = r.reduce((a, b) => a + b) / n;
const sd = Math.sqrt(r.reduce((a, b) => a + (b - mean) ** 2, 0) / (n - 1));
// A histogram of 0.5-point bins, as density
const w = 0.5, lo = Math.floor(Math.min(...r) / w) * w, bins = Math.ceil((Math.max(...r) - lo) / w);
const counts = new Array(bins).fill(0);
for (const v of r) counts[Math.min(Math.floor((v - lo) / w), bins - 1)]++;
plot.addMark('bars', { color: '#4cc9f0', band: 0.92, fillAlpha: 0.55 })
  .setData({ x: counts.map((_, i) => lo + (i + 0.5) * w), y: counts.map((c) => c / (n * w)) });
// Kernel density (Silverman bandwidth) and the normal fit
const h = 1.06 * sd * Math.pow(n, -0.2);
const grid = Array.from({ length: 200 }, (_, i) => lo + (i / 199) * bins * w);
const kde = grid.map((x) => r.reduce((s, v) => s + Math.exp(-0.5 * ((x - v) / h) ** 2), 0) / (n * h * Math.sqrt(2 * Math.PI)));
plot.addMark('line', { color: '#f3e8ff', lineWidth: 2 }).setData({ x: grid, y: kde });
plot.addMark('line', { color: '#ffb547', lineWidth: 1.5, lineStyle: 'dashed' })
  .setData({ x: grid, y: grid.map((x) => Math.exp(-0.5 * ((x - mean) / sd) ** 2) / (sd * Math.sqrt(2 * Math.PI))) });
const sorted = [...r].sort((a, b) => a - b), var95 = sorted[Math.floor(0.05 * n)];
plot.addRule({ axis: 'x', value: var95, label: 'VaR 95%', color: '#ff5d73', lineStyle: 'dashed' });
plot.addLabel({ x: mean + 1.3 * sd, y: Math.exp(-0.5 * 1.3 ** 2) / (sd * Math.sqrt(2 * Math.PI)), text: 'normal', color: '#ffb547', align: 'left', offset: [10, -6] });
plot.axis('x').applyOptions({ title: 'Daily return', format: { type: 'percent', decimals: 0 } });
plot.axis('right').applyOptions({ title: 'Density', format: { type: 'fixed', decimals: 2 } });`,
    about: 'Four years of BTC daily returns: a histogram, a kernel density estimate (white) and the normal distribution with the same mean and deviation (dashed). The fat tails and the taller peak are where the two curves part; the rule is the one-day 95% value at risk.',
    api: ["addMark('bars')", 'band', 'addRule'],
  },
  {
    id: 'riskret',
    title: 'Risk vs return',
    group: 'Portfolio',
    code: `${plot()}
const symbols = ['BTCUSD', 'ETHUSD', 'SOLUSD', 'ES', 'NQ', 'ACME', 'NOVA', 'XAUUSD', 'EURUSD', 'CL', 'CN300'];
const all = await Promise.all(symbols.map((s) => fetch('/api/bars?symbol=' + s + '&interval=1d&count=366').then((r) => r.json())));
${RETURNS}
// Annualised volatility and return, and the ratio of the two
const stats = all.map((b) => {
  const r = returns(b), m = r.reduce((a, c) => a + c) / r.length;
  const vol = Math.sqrt(r.reduce((a, c) => a + (c - m) ** 2, 0) / (r.length - 1) * 252) * 100;
  return { vol, ret: (b.close[b.close.length - 1] / b.close[0] - 1) * 100 };
});
plot.addMark('scatter', {
  pointSize: 14,
  colorScale: { stops: [[0, '#ff5d73'], [0.5, '#5b6275'], [1, '#2ee6b6']], center: 0, domain: [-1.5, 1.5] },
}).setData({
  x: stats.map((s) => s.vol),
  y: stats.map((s) => s.ret),
  value: stats.map((s) => s.ret / s.vol),
  label: symbols,
});
plot.addRule({ value: 0, color: '#8a93a6' });
plot.axis('x').applyOptions({ title: 'Volatility, annualised', format: { type: 'percent', decimals: 0 }, includeZero: true });
plot.axis('right').applyOptions({ title: 'Return, one year', format: { type: 'percent', decimals: 0 } });`,
    about: 'Eleven markets by one-year return against annualised volatility, coloured by return per unit of risk on a diverging colour scale (red below zero, green above) and labelled with their symbols.',
    api: ['colorScale', 'value', 'scatter labels'],
  },
  {
    id: 'frontier',
    title: 'Efficient frontier',
    group: 'Portfolio',
    code: `${plot()}
const symbols = ['ES', 'XAUUSD', 'BTCUSD', 'EURUSD'];
const all = await Promise.all(symbols.map((s) => fetch('/api/bars?symbol=' + s + '&interval=1d&count=757').then((r) => r.json())));
${RETURNS}
// Simple returns: the mean of log returns understates the expected return by about σ²/2
const r = all.map((b) => returns(b).map(Math.expm1)), n = Math.min(...r.map((x) => x.length));
const mu = r.map((x) => x.slice(-n).reduce((a, b) => a + b) / n * 252);
const cov = r.map((a) => r.map((b) => {
  const ma = a.slice(-n).reduce((s, v) => s + v) / n, mb = b.slice(-n).reduce((s, v) => s + v) / n;
  return a.slice(-n).reduce((s, v, i) => s + (v - ma) * (b.slice(-n)[i] - mb), 0) / (n - 1) * 252;
}));
// 4 000 random long-only portfolios (seeded, so the picture is stable)
let seed = 7;
const rand = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
const xs = [], ys = [], sharpe = [];
for (let k = 0; k < 4000; k++) {
  const w = symbols.map(() => -Math.log(rand()));
  const s = w.reduce((a, b) => a + b);
  const wt = w.map((v) => v / s);
  const ret = wt.reduce((a, v, i) => a + v * mu[i], 0);
  const vol = Math.sqrt(wt.reduce((a, v, i) => a + wt.reduce((b, u, j) => b + v * u * cov[i][j], 0), 0));
  xs.push(vol * 100); ys.push(ret * 100); sharpe.push(ret / vol);
}
plot.addMark('scatter', { pointSize: 3, colorScale: 'sequential', hoverable: false }).setData({ x: xs, y: ys, value: sharpe });
// The frontier: the best return in each band of volatility
const width = 0.5, best = new Map();
xs.forEach((v, i) => { const b = Math.floor(v / width); if (!best.has(b) || ys[best.get(b)] < ys[i]) best.set(b, i); });
const edge = [...best.entries()].sort((a, b) => a[0] - b[0]).map(([, i]) => i);
const fx = [], fy = [];
for (const i of edge) if (!fy.length || ys[i] > fy[fy.length - 1]) { fx.push(xs[i]); fy.push(ys[i]); }
plot.addMark('line', { color: '#ffb547', lineWidth: 2.5 }).setData({ x: fx, y: fy });
plot.addMark('scatter', { color: '#e6f6ff', pointSize: 9, icon: 'square' }).setData({
  x: symbols.map((_, i) => Math.sqrt(cov[i][i]) * 100), y: mu.map((m) => m * 100), label: symbols,
});
const top = sharpe.indexOf(Math.max(...sharpe));
plot.addLabel({ x: xs[top], y: ys[top], text: 'max Sharpe', icon: 'circle', color: '#2ee6b6', align: 'right', offset: [-8, -12] });
plot.axis('x').applyOptions({ title: 'Volatility', format: { type: 'percent', decimals: 0 }, includeZero: true });
plot.axis('right').applyOptions({ title: 'Expected return', format: { type: 'percent', decimals: 0 } });`,
    about: 'Random long-only mixes of stocks, gold, bitcoin and the euro, from three years of daily returns, coloured by Sharpe ratio, with the efficient frontier (the best return for each level of risk) in amber and the single assets as squares.',
    api: ['colorScale', "addMark('scatter')", 'addLabel'],
  },
  {
    id: 'correlation',
    title: 'Correlation matrix',
    group: 'Risk',
    code: `${plot()}
const symbols = ['BTCUSD', 'ETHUSD', 'SOLUSD', 'ES', 'NQ', 'ACME', 'XAUUSD', 'EURUSD', 'CL', 'US10Y'];
const all = await Promise.all(symbols.map((s) => fetch('/api/bars?symbol=' + s + '&interval=1d&count=181').then((r) => r.json())));
${RETURNS}
const r = all.map(returns), n = Math.min(...r.map((x) => x.length));
const corr = (a, b) => {
  a = a.slice(-n); b = b.slice(-n);
  const ma = a.reduce((s, v) => s + v) / n, mb = b.reduce((s, v) => s + v) / n;
  let ab = 0, aa = 0, bb = 0;
  for (let i = 0; i < n; i++) { ab += (a[i] - ma) * (b[i] - mb); aa += (a[i] - ma) ** 2; bb += (b[i] - mb) ** 2; }
  return ab / Math.sqrt(aa * bb);
};
const x = [], y = [], value = [];
symbols.forEach((_, i) => symbols.forEach((_, j) => { x.push(j); y.push(i); value.push(corr(r[i], r[j])); }));
for (const id of ['x', 'right']) {
  plot.axis(id).applyOptions({ type: 'category' });
  plot.axis(id).setCategories(symbols);
}
plot.axis('right').applyOptions({ inverted: true });
plot.addMark('cells', {
  colorScale: { stops: [[0, '#ff5d73'], [0.5, '#151922'], [1, '#4cc9f0']], center: 0, domain: [-1, 1] },
  showValues: true,
  valueFormat: { type: 'fixed', decimals: 2 },
}).setData({ x, y, value });`,
    about: 'Correlations of six months of daily returns between ten markets as a heat map: cells on two category axes with a diverging colour scale centred on zero (blue together, red opposite) and the coefficient written in each cell.',
    api: ["addMark('cells')", 'diverging colorScale', 'showValues'],
  },
  {
    id: 'factors',
    title: 'Factor exposures by month',
    group: 'Factor research',
    code: `${plot()}
${load('f', 'series/quantFactorScores', { symbol: 'ACME', interval: '1mo', count: 12 })}
const factors = [['momentum', '#4cc9f0'], ['value', '#ffb547'], ['quality', '#2ee6b6'], ['lowVol', '#c084fc'], ['size', '#ff5d73'], ['carry', '#8a93a6']];
const months = f.time.map((t) => ${MONTHS}[new Date(t).getUTCMonth()] + ' ' + String(new Date(t).getUTCFullYear()).slice(2));
plot.axis('x').applyOptions({ type: 'category' });
plot.axis('x').setCategories(months);
const x = months.map((_, i) => i);
// One stack: positive scores pile up above zero, negative ones below
for (const [name, color] of factors) plot.addMark('bars', { stack: 1, color }).setData({ x, y: f[name] });
const net = x.map((i) => factors.reduce((s, [name]) => s + f[name][i], 0));
plot.addMark('line', { color: '#f3e8ff', lineWidth: 2, icon: 'circle', pointSize: 6 }).setData({ x, y: net });
// A legend along the top: the labels sit above the highest stack
const top = Math.max(...x.map((i) => factors.reduce((s, [name]) => s + Math.max(f[name][i], 0), 0)));
plot.axis('right').applyOptions({ range: [Math.min(0, ...x.map((i) => factors.reduce((s, [name]) => s + Math.min(f[name][i], 0), 0))) - 0.2, top + 0.6] });
factors.forEach(([name, color], k) => plot.addLabel({ x: 0, y: top + 0.6, text: name, color, align: 'left', offset: [-30 + 84 * k, 10] }));
plot.addRule({ value: 0, color: '#5b6275' });
plot.axis('right').applyOptions({ title: 'Factor score (z)', format: { type: 'fixed', decimals: 1 } });`,
    about: 'Six equity factor scores of a portfolio month by month as stacked bars (marks with the same `stack` add up, positives above zero and negatives below), with the net exposure as a line.',
    api: ['stack', "addMark('bars')", "type: 'category'"],
  },
];

const equities: Demo[] = [
  {
    id: 'incomewf',
    title: 'Income statement waterfall',
    group: 'Earnings',
    code: `${plot()}
${load('fin', 'series/eqFinancials', { symbol: 'ACME', interval: '1q', count: 1 })}
const revenue = fin.revenue[0], net = fin.netIncome[0];
// The steps from revenue to net income; costs are shares of revenue, tax
// is what is left between operating income and net income.
const cogs = -0.42 * revenue, opex = -0.17 * revenue, rnd = -0.12 * revenue;
const operating = revenue + cogs + opex + rnd;
const items = [
  ['Revenue', revenue, 'delta'], ['Cost of sales', cogs, 'delta'], ['Gross profit', 0, 'subtotal'],
  ['SG&A', opex, 'delta'], ['R&D', rnd, 'delta'], ['Operating income', 0, 'subtotal'],
  ['Interest & other', -0.01 * revenue, 'delta'], ['Tax', net - operating + 0.01 * revenue, 'delta'], ['Net income', 0, 'total'],
];
plot.axis('x').applyOptions({ type: 'category' });
plot.axis('x').setCategories(items.map((i) => i[0]));
plot.addMark('waterfall', { color: '#2ee6b6', downColor: '#ff5d73', totalColor: '#4cc9f0', showValues: true, valueFormat: 'compact' }).setData({
  x: items.map((_, i) => i),
  y: items.map((i) => i[1]),
  step: items.map((i) => i[2]),
});
plot.axis('right').applyOptions({ format: 'compact', includeZero: true, title: 'USD, last quarter' });`,
    about: 'One quarter of a company\'s income statement from revenue down to net income as a waterfall: costs step down in red, subtotals and the total are full bars computed by the mark (`step`: delta, subtotal, total).',
    api: ["addMark('waterfall')", 'step', 'totalColor'],
  },
  {
    id: 'seasonality',
    title: 'Seasonality by month',
    group: 'Seasonality',
    code: `${plot()}
${bars({ ...D, count: 1600 })}
// Each year's path from the first close of January, month by month
const years = new Map();
bars.time.forEach((t, i) => {
  const d = new Date(t), y = d.getUTCFullYear(), m = d.getUTCMonth();
  if (!years.has(y)) years.set(y, { start: bars.close[i], end: new Array(12).fill(NaN) });
  years.get(y).end[m] = (bars.close[i] / years.get(y).start - 1) * 100;
});
const list = [...years.entries()];
plot.axis('x').applyOptions({ type: 'category' });
plot.axis('x').setCategories(${MONTHS});
const x = Array.from({ length: 12 }, (_, i) => i);
const grey = ['#3a4050', '#4b5263', '#5b6275', '#6b7286'];
list.slice(0, -1).forEach(([year, v], k) => {
  plot.addMark('line', { color: grey[k % grey.length], lineWidth: 1.5 }).setData({ x, y: v.end });
  plot.addLabel({ x: 11, y: v.end[11], text: String(year), color: '#8a93a6', align: 'left', offset: [8, 0] });
});
const avg = x.map((m) => { const v = list.slice(0, -1).map(([, y]) => y.end[m]).filter(Number.isFinite); return v.reduce((a, b) => a + b, 0) / v.length; });
plot.addMark('line', { color: '#ffb547', lineWidth: 2, lineStyle: 'dashed' }).setData({ x, y: avg });
const [thisYear, cur] = list[list.length - 1];
plot.addMark('line', { color: '#4cc9f0', lineWidth: 3, icon: 'circle', pointSize: 7 }).setData({ x, y: cur.end });
plot.addLabel({ x: 0, y: cur.end[0], text: String(thisYear), color: '#4cc9f0', align: 'right', offset: [-10, 0] });
plot.addRule({ value: 0, color: '#5b6275' });
plot.axis('right').applyOptions({ title: 'Return since the start of the year', format: { type: 'percent', decimals: 0 } });`,
    about: 'A stock\'s return since the first trading day of January for each year, month by month: past years in grey, their average dashed and this year in blue. Lines on a category axis; a NaN for months not traded yet leaves a gap.',
    api: ["type: 'category'", "addMark('line')", 'addLabel'],
  },
  {
    id: 'seasonheat',
    title: 'Monthly returns heat map',
    group: 'Seasonality',
    code: `${plot()}
${bars({ ...D, count: 2500 })}
// Month-end closes, then each month's return
const ends = new Map();
bars.time.forEach((t, i) => { const d = new Date(t); ends.set(d.getUTCFullYear() * 12 + d.getUTCMonth(), bars.close[i]); });
const keys = [...ends.keys()].sort((a, b) => a - b);
const first = Math.floor(keys[1] / 12), years = [];
const x = [], y = [], value = [];
for (let k = 1; k < keys.length; k++) {
  const ret = (ends.get(keys[k]) / ends.get(keys[k - 1]) - 1) * 100;
  const year = Math.floor(keys[k] / 12) - first;
  years[year] = String(first + year);
  x.push(keys[k] % 12); y.push(year); value.push(ret);
}
plot.axis('x').applyOptions({ type: 'category' });
plot.axis('x').setCategories(${MONTHS});
plot.axis('right').applyOptions({ type: 'category', inverted: true });
plot.axis('right').setCategories(years);
plot.addMark('cells', {
  colorScale: { stops: [[0, '#ff5d73'], [0.5, '#151922'], [1, '#2ee6b6']], center: 0, domain: [-12, 12] },
  showValues: true,
  valueFormat: { type: 'percent', decimals: 1 },
}).setData({ x, y, value });`,
    about: 'Every month\'s return of a stock over ten years, one row per year: a cells mark on two category axes with a diverging colour scale (green up, red down) and the return in each cell, so strong and weak months show as columns.',
    api: ["addMark('cells')", 'colorScale', 'valueFormat'],
  },
  {
    id: 'calendar',
    title: 'Daily returns calendar',
    group: 'Seasonality',
    code: `${plot()}
${bars({ ...B, count: 365 })}
// One cell per day: column = week, row = weekday
const DAY = 86_400_000, start = bars.time[1] - ((new Date(bars.time[1]).getUTCDay() + 6) % 7) * DAY;
const x = [], y = [], value = [];
for (let i = 1; i < bars.time.length; i++) {
  const days = Math.round((bars.time[i] - start) / DAY);
  x.push(start + (Math.floor(days / 7) * 7 + 3.5) * DAY); y.push(days % 7); value.push((bars.close[i] / bars.close[i - 1] - 1) * 100);
}
plot.axis('right').applyOptions({ type: 'category', inverted: true });
plot.axis('right').setCategories(['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']);
// x is a date: the middle of each week, so a cell spans Monday to Sunday
// (the cell width comes from the gap between columns)
plot.axis('x').applyOptions({ format: 'date', grid: false });
plot.addMark('cells', {
  band: 0.86,
  colorScale: { stops: [[0, '#ff5d73'], [0.5, '#1c212c'], [1, '#2ee6b6']], center: 0, domain: [-5, 5] },
  valueFormat: { type: 'percent', decimals: 2 },
}).setData({ x, y, value });`,
    about: 'A year of BTC daily returns as a calendar: one cell per day, a column per week and a row per weekday, green for up days and red for down days. Hover a cell for its return. Cells on a date x axis (one column per week) against a category y axis.',
    api: ["addMark('cells')", 'band', "format: 'date'"],
  },
  {
    id: 'rrg',
    title: 'Relative rotation graph',
    group: 'Quality & breadth',
    code: `${plot()}
const symbols = [['ACME', '#4cc9f0'], ['NOVA', '#c084fc'], ['NQ', '#2ee6b6'], ['XAUUSD', '#ffb547'], ['CL', '#ff5d73']];
const get = (s) => fetch('/api/bars?symbol=' + s + '&interval=1w&count=80').then((r) => r.json());
const [bench, ...all] = await Promise.all([get('ES'), ...symbols.map(([s]) => get(s))]);
// JdK-style: RS-Ratio is relative strength against the index, normalised
// around 100; RS-Momentum is the rate of change of that ratio, also around 100.
const norm = (a, n) => a.map((v, i) => {
  if (i < n) return NaN;
  const w = a.slice(i - n, i), m = w.reduce((s, x) => s + x) / n;
  const sd = Math.sqrt(w.reduce((s, x) => s + (x - m) ** 2, 0) / n) || 1;
  return 100 + (v - m) / sd;
});
// Both are smoothed (an exponential average) so the tails turn rather than jump.
const ema = (a, n) => { let e = NaN; return a.map((v) => (e = Number.isNaN(e) ? v : e + (2 / (n + 1)) * (v - e))); };
const n = Math.min(bench.close.length, ...all.map((b) => b.close.length));
let reach = 2;
all.forEach((b, k) => {
  const rs = ema(b.close.slice(-n).map((c, i) => (100 * c) / bench.close.slice(-n)[i]), 4);
  const ratio = ema(norm(rs, 26), 3);
  const mom = ema(norm(ratio.map((v, i) => (i ? v - ratio[i - 1] : NaN)), 10), 3);
  const tail = 10, xs = ratio.slice(-tail), ys = mom.slice(-tail);
  for (const v of [...xs, ...ys]) reach = Math.max(reach, Math.abs(v - 100));
  const [name, color] = symbols[k];
  // The tail goes back ten weeks: x is not monotone, the line follows the data order
  plot.addMark('line', { color, lineWidth: 2, fillAlpha: 0.5 }).setData({ x: xs, y: ys });
  plot.addMark('scatter', { color, pointSize: 10 }).setData({ x: [xs[tail - 1]], y: [ys[tail - 1]], label: [name] });
});
// The same range on both axes, centred on 100, wide enough for every tail
const half = Math.ceil(reach + 0.5);
plot.addRule({ axis: 'x', value: 100, color: '#5b6275' });
plot.addRule({ value: 100, color: '#5b6275' });
for (const [text, x, y, align] of [['Leading', 1, 1, 'right'], ['Weakening', 1, -1, 'right'], ['Lagging', -1, -1, 'left'], ['Improving', -1, 1, 'left']]) {
  plot.addLabel({ x: 100 + x * (half - 0.4), y: 100 + y * (half - 0.4), text, color: '#5b6275', align });
}
plot.axis('x').applyOptions({ title: 'RS-Ratio', range: [100 - half, 100 + half] });
plot.axis('right').applyOptions({ title: 'RS-Momentum', range: [100 - half, 100 + half] });`,
    about: 'Five markets against the equity index on a relative rotation graph: relative strength (x) against its momentum (y), each with a ten-week tail. Lines whose x goes back and forth follow the data order; rules split the four quadrants a market rotates through.',
    api: ["addMark('line')", 'non-monotone x', 'addRule'],
  },
];

const perps: Demo[] = [
  {
    id: 'hourheat',
    title: 'Activity by hour and weekday',
    group: 'Market structure',
    code: `${plot()}
${bars({ symbol: 'BTCUSD', interval: '1h', count: 24 * 7 * 12 })}
// Average volume in each hour of each weekday over twelve weeks (UTC)
const sum = new Array(7 * 24).fill(0), cnt = new Array(7 * 24).fill(0);
bars.time.forEach((t, i) => {
  const d = new Date(t), k = ((d.getUTCDay() + 6) % 7) * 24 + d.getUTCHours();
  sum[k] += bars.volume[i]; cnt[k]++;
});
const x = [], y = [], value = [];
for (let k = 0; k < 7 * 24; k++) { x.push(k % 24); y.push(Math.floor(k / 24)); value.push(sum[k] / cnt[k]); }
plot.axis('x').applyOptions({ type: 'category', title: 'Hour, UTC' });
plot.axis('x').setCategories(Array.from({ length: 24 }, (_, h) => String(h).padStart(2, '0')));
plot.axis('right').applyOptions({ type: 'category', inverted: true });
plot.axis('right').setCategories(['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']);
plot.addMark('cells', {
  band: 0.92,
  colorScale: { stops: [[0, '#10131b'], [0.35, '#1f4e79'], [0.7, '#4cc9f0'], [1, '#f4f9a8']], quantile: 0.02 },
  valueFormat: 'compact',
}).setData({ x, y, value });`,
    about: 'When BTC trades: average hourly volume over twelve weeks by hour of day and weekday, as a heat map with a sequential colour scale whose range ignores the top and bottom 2% (`quantile`). The US and European sessions show as bright columns, weekends as dim rows.',
    api: ["addMark('cells')", 'quantile', "type: 'category'"],
  },
];

const orderflow: Demo[] = [
  {
    id: 'depth',
    title: 'Depth chart beside the candles',
    group: 'Order book',
    side: true,
    code: `${create()}
const to = Math.floor(Date.now() / 60_000) * 60_000;
const range = 'symbol=BTCUSD&from=' + (to - 6 * 3_600_000) + '&to=' + to;
const bars = await fetch('/api/bars?interval=1m&' + range).then((r) => r.json());
chart.addCandlestickSeries().setData(bars);
chart.setBarSpacing(6);
chart.scrollToRealtime();
// The book now, 120 levels of $5 a side
const book = await fetch('/api/depth?symbol=BTCUSD&from=' + (to - 1000) + '&to=' + to + '&every=1000&levels=120&step=10').then((r) => r.json());
const prices = book.prices[book.prices.length - 1], sizes = book.sizes[book.sizes.length - 1];
const half = prices.length / 2; // bids first (ascending), then asks
// Cumulative size from the best bid down and from the best ask up
const bid = { x: [], y: [] }, ask = { x: [], y: [] };
for (let i = half - 1, s = 0; i >= 0; i--) { s += sizes[i]; bid.x.push(s); bid.y.push(prices[i]); }
for (let i = half, s = 0; i < prices.length; i++) { s += sizes[i]; ask.x.push(s); ask.y.push(prices[i]); }
${plot('side')}
plot.axis('right').applyOptions({ visible: false });
chart.attachSidePlot(plot);
plot.addMark('bars', { orientation: 'horizontal', color: '#2ee6b6', band: 1, fillAlpha: 0.6 }).setData(bid);
plot.addMark('bars', { orientation: 'horizontal', color: '#ff5d73', band: 1, fillAlpha: 0.6 }).setData(ask);
plot.addRule({ value: (prices[half - 1] + prices[half]) / 2, color: '#e6f6ff', lineStyle: 'dashed' });
plot.axis('x').applyOptions({ title: 'Cumulative BTC', format: 'compact', includeZero: true });`,
    about: 'The order book as a depth chart beside one-minute candles: cumulative bid size (green) below the mid and ask size (red) above, as horizontal bars in a FerroPlot that shares the chart\'s price axis through `attachSidePlot`. Pan or zoom the candles and the depth follows.',
    api: ['attachSidePlot', "orientation: 'horizontal'", '/api/depth'],
  },
];

const defi: Demo[] = [
  {
    id: 'liqdist',
    title: 'Concentrated liquidity by price',
    group: 'Liquidity',
    side: true,
    code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '1h', count: 24 * 14 })}
${load('pool', 'series/defiPoolDepth', { symbol: 'ETHUSD', interval: '1h', count: 1 })}
chart.addCandlestickSeries().setData(bars);
chart.setBarSpacing(4);
chart.scrollToRealtime();
// Liquidity per $10 tick: most LPs in a band around the price, the rest in
// wide ranges, and a few large positions at round numbers.
const S = bars.close[bars.close.length - 1], tick = 10, prices = [], liquidity = [], color = [];
const share = (d, w) => Math.exp(-0.5 * (d / w) ** 2) / w;
for (let k = -80; k <= 80; k++) {
  const p = Math.round(S / tick) * tick + k * tick, d = p / S - 1;
  const round = p % 250 === 0 ? 2.5 : 1;
  prices.push(p);
  liquidity.push(pool.tvl[0] * (tick / S) * (0.65 * share(d, 0.02) + 0.35 * share(d, 0.09)) * round / 2.5);
  color.push(Math.abs(p - S) < tick / 2 ? '#ffb547' : null); // the active tick
}
${plot('side')}
plot.axis('right').applyOptions({ visible: false });
chart.attachSidePlot(plot);
plot.addMark('bars', { orientation: 'horizontal', color: '#7ee787', band: 0.8 }).setData({ x: liquidity, y: prices, color });
plot.axis('x').applyOptions({ title: 'Liquidity, USD', format: 'compact', includeZero: true });`,
    about: 'How an ETH pool\'s liquidity is spread over $10 price ticks, beside two weeks of hourly candles: a FerroPlot of horizontal bars attached with `attachSidePlot`, so ticks line up with prices. The active tick is amber; walls at round numbers stand out.',
    api: ['attachSidePlot', 'per-bar color', "orientation: 'horizontal'"],
  },
];

const funds: Demo[] = [
  {
    id: 'brinson',
    title: 'Brinson attribution',
    group: 'Attribution',
    code: `${plot()}
// One quarter: sector weights and returns of a fund and its benchmark (%)
const sectors = ['Tech', 'Health', 'Financials', 'Energy', 'Consumer', 'Industrials'];
const wp = [34, 12, 14, 4, 20, 16], wb = [28, 14, 16, 8, 18, 16];
const rp = [9.2, 3.1, 4.4, -2.0, 2.6, 3.9], rb = [8.1, 3.6, 3.9, -3.5, 3.0, 3.2];
const Rb = wb.reduce((s, w, i) => s + (w / 100) * rb[i], 0);
// Brinson–Fachler: allocation, selection and interaction per sector
const alloc = sectors.map((_, i) => ((wp[i] - wb[i]) / 100) * (rb[i] - Rb));
const select = sectors.map((_, i) => (wb[i] / 100) * (rp[i] - rb[i]));
const inter = sectors.map((_, i) => ((wp[i] - wb[i]) / 100) * (rp[i] - rb[i]));
const sum = (a) => a.reduce((s, v) => s + v, 0);
const items = [
  ['Benchmark', Rb, 'delta'],
  ...sectors.map((s, i) => [s, alloc[i] + select[i] + inter[i], 'delta']),
  ['Fund', 0, 'total'],
];
plot.axis('x').applyOptions({ type: 'category' });
plot.axis('x').setCategories(items.map((i) => i[0]));
plot.addMark('waterfall', { color: '#2ee6b6', downColor: '#ff5d73', totalColor: '#4cc9f0', showValues: true, valueFormat: { type: 'percent', decimals: 2 } })
  .setData({ x: items.map((_, i) => i), y: items.map((i) => i[1]), step: items.map((i) => i[2]) });
plot.addLabel({ x: 7, y: Rb + sum(alloc) + sum(select) + sum(inter), text: 'allocation ' + sum(alloc).toFixed(2) + ' · selection ' + sum(select).toFixed(2) + ' · interaction ' + sum(inter).toFixed(2), color: '#8a93a6', align: 'right', offset: [-40, -36] });
plot.axis('right').applyOptions({ title: 'Return, quarter', format: { type: 'percent', decimals: 1 }, includeZero: true });`,
    about: 'Why a fund beat its benchmark last quarter: a waterfall from the benchmark return through each sector\'s Brinson–Fachler effect (allocation, selection and interaction combined) to the fund\'s return. The label splits the total into the three effects.',
    api: ["addMark('waterfall')", "step: 'total'", 'addLabel'],
  },
];

/** The FerroPlot examples by category id. */
export const plotDemos: Record<string, Demo[]> = {
  options,
  rates,
  commodities,
  quant,
  equities,
  perps,
  orderflow,
  defi,
  funds,
};
