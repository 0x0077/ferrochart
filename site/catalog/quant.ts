import { create, load } from './build';
import { plotDemos } from './plots';
import type { Category } from './types';

// Daily backtests on a session market; weekly and monthly research series.
const D = { symbol: 'ES', interval: '1d', count: 750 };
const W = { symbol: 'ES', interval: '1w', count: 260 };
const M = { symbol: 'ES', interval: '1mo', count: 36 };

const up = '#2ee6b6', down = '#ff5d73', lime = '#a3e635', grey = '#8b93a7';

export const quant: Category = {
  id: 'quant',
  title: 'Portfolio, risk & quant',
  kicker: '16',
  blurb: 'Equity curves, drawdowns, rolling statistics, factor research and risk checks.',
  accent: lime,
  demos: [
    // ----------------------------------------------------------- Backtests
    {
      id: 'equity',
      title: 'Equity curve · drawdown',
      group: 'Backtests',
      code: `${create()}
${load('eq', 'series/equityCurve', D)}
const strategy = chart.addSeries('line', { color: '${lime}', lineWidth: 2 });
strategy.setData({ time: eq.time, value: eq.strategy });
const benchmark = chart.addSeries('line', { color: '${grey}', lineWidth: 1.5 });
benchmark.setData({ time: eq.time, value: eq.benchmark });
chart.applyPriceScaleOptions({ mode: 'percentage' });
// Name each line on its last-value line.
strategy.createPriceLine({ price: eq.strategy.at(-1), color: '${lime}', lineStyle: 'dotted', axisLabelVisible: false, title: 'Strategy' });
benchmark.createPriceLine({ price: eq.benchmark.at(-1), color: '${grey}', lineStyle: 'dotted', axisLabelVisible: false, title: 'Benchmark' });

// Drawdown: how far the equity is below its running peak, in percent.
let peak = 0;
const drawdown = eq.strategy.map((v) => {
  peak = Math.max(peak, v);
  return (v / peak - 1) * 100;
});
chart.addSeries('baseline', {
  baseValue: 0, color: '${down}', downColor: '${down}', bottomFill: 'rgba(255, 93, 115, 0.35)', pricePrecision: 1,
}, chart.addPane(0.5)).setData({ time: eq.time, value: drawdown });
chart.fitContent();`,
      about: 'A strategy against its benchmark on a `percentage` price scale, so both read as return since the first visible day, with the drawdown from the running peak in a pane below. The drawdown is computed in a few lines from the equity series.',
      api: ["addSeries('line')", "addSeries('baseline')", 'applyPriceScaleOptions', 'addPane'],
    },
    {
      id: 'ddepth',
      title: 'Underwater curve · worst drawdown',
      group: 'Backtests',
      code: `${create()}
${load('eq', 'series/equityCurve', { ...D, symbol: 'XAUUSD' })}
let peak = 0, peakAt = 0, worst = { depth: 0 };
const under = eq.strategy.map((v, i) => {
  if (v >= peak) { peak = v; peakAt = i; }
  const depth = (v / peak - 1) * 100;
  if (depth < worst.depth) worst = { depth, from: peakAt, trough: i };
  return depth;
});
// The worst drawdown ends when the equity regains its peak.
const end = under.findIndex((d, i) => i > worst.trough && d === 0);

chart.addSeries('line', { color: '${lime}', lineWidth: 2 }).setData({ time: eq.time, value: eq.strategy });
const water = chart.addSeries('baseline', {
  baseValue: 0, color: '#4cc9f0', downColor: '#4cc9f0', bottomFill: 'rgba(76, 201, 240, 0.35)', pricePrecision: 1,
}, chart.addPane(1));
water.setData({ time: eq.time, value: under });
const days = (end > 0 ? end : under.length - 1) - worst.from;
water.setMarkers([
  { time: eq.time[worst.from], position: 'aboveBar', shape: 'arrowDown', color: '#ffb547', text: 'Peak' },
  { time: eq.time[worst.trough], position: 'belowBar', shape: 'arrowUp', color: '${down}', text: worst.depth.toFixed(1) + '%' },
  end > 0
    ? { time: eq.time[end], position: 'aboveBar', shape: 'arrowDown', color: '${up}', text: 'Recovered · ' + days + ' days' }
    : { time: eq.time.at(-1), position: 'aboveBar', shape: 'circle', color: '#ffb547', text: days + ' days, not recovered' },
]);
chart.fitContent();`,
      about: 'The underwater curve, the percentage below the running peak, as a `baseline` series under the equity. Markers show where the worst drawdown starts, how deep it gets and how many trading days it takes to recover.',
      api: ["addSeries('baseline')", 'setMarkers', 'addPane'],
    },
    {
      id: 'jointdd',
      title: 'Joint drawdowns',
      group: 'Backtests',
      code: `${create()}
${load('s', 'series/quantStrats', D)}
const names = ['trend', 'carry', 'value'];
const colors = ['${lime}', '#4cc9f0', '#b388ff'];
const drawdown = (eq) => {
  let peak = 0;
  return eq.map((v) => ((peak = Math.max(peak, v)), (v / peak - 1) * 100));
};
const dds = names.map((n) => drawdown(s[n]));
dds.forEach((dd, k) => {
  const fill = colors[k] + '55';
  chart.addSeries('baseline', { baseValue: 0, color: colors[k], downColor: colors[k], bottomFill: fill, pricePrecision: 1 }, k ? chart.addPane(1) : 0)
    .setData({ time: s.time, value: dd });
});
// How many strategies are more than 10% under water on the same day.
const together = s.time.map((_, i) => dds.filter((dd) => dd[i] < -10).length);
chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.7)).setData({
  time: s.time, value: together,
  color: together.map((n) => ['transparent', '#3a4256', '#ffb547', '${down}'][n]),
});
chart.fitContent();`,
      about: 'Drawdowns of three strategies (trend, carry, value), one `baseline` pane each, and a histogram counting how many are more than 10% under water on the same day. Amber (two) and red (all three) bars show when diversification failed.',
      api: ["addSeries('baseline')", "addSeries('histogram')", 'per-point color'],
    },
    {
      id: 'walkfwd',
      title: 'Walk-forward windows',
      group: 'Backtests',
      code: `${create()}
${load('eq', 'series/equityCurve', D)}
const line = chart.addSeries('line', { color: '${lime}', lineWidth: 2 });
line.setData({ time: eq.time, value: eq.strategy });
chart.applyPriceScaleOptions({ margins: { top: 0.08, bottom: 0.34 } });

// Fit on 250 days, trade the next 60, roll forward by 60.
const train = 250, test = 60;
const folds = [];
for (let s = 0; s + train + test <= eq.time.length; s += test) folds.push(s);

const t = (i) => eq.time[Math.min(i, eq.time.length - 1)];
chart.addLayer({
  pane: 0,
  position: 'belowSeries',
  build(ctx) {
    const out = [], { y, height } = ctx.rect;
    const row = 6, top = y + height - folds.length * (row + 2) - 6;
    folds.forEach((s, k) => {
      const [a, b, c] = [ctx.timeToX(t(s)), ctx.timeToX(t(s + train)), ctx.timeToX(t(s + train + test))];
      if (a === null || b === null || c === null) return;
      const ry = top + k * (row + 2);
      out.push({ type: 'rect', x: a, y: ry, width: b - a, height: row, color: 'rgba(139, 147, 167, 0.35)' });
      out.push({ type: 'rect', x: b, y: ry, width: c - b, height: row, color: '${lime}' });
      // The out-of-sample stretch, shaded behind the equity.
      out.push({ type: 'rect', x: b, y, width: c - b, height: top - y - 8, color: k % 2 ? 'rgba(163, 230, 53, 0.07)' : 'rgba(163, 230, 53, 0.13)' });
    });
    out.push({ type: 'text', x: ctx.rect.x + 8, y: top - 16, text: 'In-sample', color: '${grey}', size: 11 });
    out.push({ type: 'text', x: ctx.rect.x + 76, y: top - 16, text: 'Out-of-sample', color: '${lime}', size: 11 });
    return out;
  },
});
chart.fitContent();`,
      about: 'A walk-forward test: the model is fitted on 250 days and traded on the next 60, then the window rolls on. A layer plugin draws one row per fold under the equity (grey in-sample, green out-of-sample) and shades the out-of-sample stretches.',
      api: ['addLayer', 'ctx.timeToX', 'applyPriceScaleOptions'],
    },
    {
      id: 'isos',
      title: 'In-sample vs out-of-sample',
      group: 'Backtests',
      code: `${create()}
${load('eq', 'series/equityCurve', D)}
chart.addSeries('line', { color: '${lime}', lineWidth: 2 }).setData({ time: eq.time, value: eq.strategy });

// The last 30% of the history was held out while the strategy was designed.
const split = Math.floor(eq.time.length * 0.7);
const sharpe = (v) => {
  const r = v.slice(1).map((x, i) => Math.log(x / v[i]));
  const mean = r.reduce((a, b) => a + b, 0) / r.length;
  const sd = Math.sqrt(r.reduce((a, b) => a + (b - mean) ** 2, 0) / r.length);
  return (mean / sd) * Math.sqrt(252);
};
const inSharpe = sharpe(eq.strategy.slice(0, split + 1));
const outSharpe = sharpe(eq.strategy.slice(split));

chart.addLayer({
  pane: 0,
  position: 'background',
  build(ctx) {
    const x = ctx.timeToX(eq.time[split]);
    if (x === null) return [];
    const { x: left, y, width, height } = ctx.rect;
    const right = left + width;
    return [
      { type: 'rect', x: left, y, width: Math.max(0, x - left), height, color: 'rgba(139, 147, 167, 0.08)' },
      { type: 'rect', x, y, width: Math.max(0, right - x), height, color: 'rgba(163, 230, 53, 0.08)' },
      { type: 'line', x1: x, y1: y, x2: x, y2: y + height, color: '${lime}', width: 1, dash: 'dashed' },
      { type: 'text', x: x - 10, y: y + 18, align: 'right', text: 'In-sample · Sharpe ' + inSharpe.toFixed(2), color: '#c4cad6', size: 12 },
      { type: 'text', x: x + 10, y: y + 18, text: 'Out-of-sample · Sharpe ' + outSharpe.toFixed(2), color: '${lime}', size: 12 },
    ];
  },
});
chart.fitContent();`,
      about: 'One equity curve split into the period the strategy was designed on and the held-out period. A layer plugin shades both, draws the split line and labels each side with its annualised Sharpe ratio, computed in the example.',
      api: ['addLayer', 'ctx.timeToX', 'ctx.rect'],
    },
    {
      id: 'livevsbt',
      title: 'Live vs backtest',
      group: 'Backtests',
      code: `${create()}
${load('s', 'series/quantLive', { ...D, count: 500 })}
// Both rebased to 1 at launch.
const rebase = (v) => v.map((x) => x / v[0]);
const backtest = rebase(s.backtest), live = rebase(s.live);
chart.addSeries('line', { color: '${grey}', lineWidth: 1.5 }).setData({ time: s.time, value: backtest });
chart.addSeries('line', { color: '${lime}', lineWidth: 2 }).setData({ time: s.time, value: live });

// Tracking gap in percent: live below backtest is slippage, fees, delays.
const gap = live.map((v, i) => (v / backtest[i] - 1) * 100);
const pane = chart.addPane(0.6);
const g = chart.addSeries('baseline', {
  baseValue: 0, color: '${up}', downColor: '${down}',
  topFill: 'rgba(46, 230, 182, 0.3)', bottomFill: 'rgba(255, 93, 115, 0.3)', pricePrecision: 2,
}, pane);
g.setData({ time: s.time, value: gap });
g.createPriceLine({ price: -2, color: '#ffb547', lineStyle: 'dashed', title: 'review' });
chart.fitContent();`,
      about: 'The live equity of a strategy against its backtest (grey) since launch, both rebased to 1, with the tracking gap in percent as a `baseline` series below and a review threshold at −2%.',
      api: ["addSeries('baseline')", 'createPriceLine', 'addPane'],
    },
    {
      id: 'rebalcf',
      title: 'Rebalancing counterfactuals',
      group: 'Backtests',
      code: `${create()}
${load('s', 'series/quantRebal', D)}
const rebase = (v) => v.map((x) => x / v[0]);
const monthly = rebase(s.monthly), quarterly = rebase(s.quarterly), hold = rebase(s.buyHold);
const lines = [[monthly, '${lime}', 2], [quarterly, '#4cc9f0', 1.5], [hold, '${grey}', 1.5]].map(([value, color, lineWidth]) => {
  const line = chart.addSeries('line', { color, lineWidth });
  line.setData({ time: s.time, value });
  return line;
});
// Green where monthly rebalancing is ahead of buy-and-hold, red where it lags.
chart.addBand(lines[0], lines[2], { fill: 'rgba(163, 230, 53, 0.18)', fillCrossed: 'rgba(255, 93, 115, 0.22)' });

// Below: each rebalanced version against buy-and-hold, in percent.
const pane = chart.addPane(0.6);
const vsHold = (v) => v.map((x, i) => (x / hold[i] - 1) * 100);
chart.addSeries('baseline', {
  baseValue: 0, color: '${lime}', downColor: '${down}',
  topFill: 'rgba(163, 230, 53, 0.25)', bottomFill: 'rgba(255, 93, 115, 0.25)', pricePrecision: 1,
}, pane).setData({ time: s.time, value: vsHold(monthly) });
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.25, pricePrecision: 1 }, pane).setData({ time: s.time, value: vsHold(quarterly) });
chart.fitContent();`,
      about: 'One portfolio rebalanced monthly (green), quarterly (blue) or never (grey), rebased to 1. `addBand` fills the gap between monthly rebalancing and buy-and-hold, and the pane below shows both rebalanced versions relative to buy-and-hold in percent: drift wins in rallies and loses in sell-offs.',
      api: ['addBand', 'fillCrossed', "addSeries('baseline')"],
    },

    // ----------------------------------------------------------- Risk
    {
      id: 'rolling',
      title: 'Rolling return · volatility · Sharpe · beta',
      group: 'Risk',
      code: `${create()}
${load('eq', 'series/equityCurve', D)}
// Daily log returns, then rolling statistics over 126 days (half a year).
const ret = (v) => v.map((x, i) => (i ? Math.log(x / v[i - 1]) : NaN));
const rs = ret(eq.strategy), rb = ret(eq.benchmark);
const n = 126, ann = 252;
const mean = (a) => a.reduce((x, y) => x + y, 0) / a.length;
const cov = (a, b) => { const ma = mean(a), mb = mean(b); return mean(a.map((x, i) => (x - ma) * (b[i] - mb))); };
const roll = (f) => rs.map((_, i) => (i < n ? NaN : f(rs.slice(i - n + 1, i + 1), rb.slice(i - n + 1, i + 1))));
const time = eq.time;
const signed = {
  baseValue: 0, color: '${up}', downColor: '${down}',
  topFill: 'rgba(46, 230, 182, 0.25)', bottomFill: 'rgba(255, 93, 115, 0.25)', pricePrecision: 1,
};
// Annualised return and volatility in percent, Sharpe, beta to the benchmark.
const panes = [
  ['Return %', 'baseline', signed, (a) => mean(a) * ann * 100],
  ['Volatility %', 'area', { color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.3)', bottomFill: 'rgba(255, 181, 71, 0.02)', pricePrecision: 1 }, (a) => Math.sqrt(cov(a, a) * ann) * 100],
  ['Sharpe', 'baseline', { ...signed, pricePrecision: 2 }, (a) => (mean(a) / Math.sqrt(cov(a, a))) * Math.sqrt(ann)],
  ['Beta', 'line', { color: '#b388ff', lineWidth: 1.5, pricePrecision: 2 }, (a, b) => cov(a, b) / cov(b, b)],
];
panes.forEach(([title, kind, options, f], k) => {
  const value = roll(f);
  const series = chart.addSeries(kind, options, k ? chart.addPane(0.7) : 0);
  series.setData({ time, value });
  // The pane's name, on the last-value line.
  series.createPriceLine({ price: value.at(-1), color: '#c4cad6', lineStyle: 'dotted', axisLabelVisible: false, title });
});
chart.setVisibleLogicalRange(n, time.length + 3);`,
      about: 'Half-year (126-day) rolling statistics of a strategy, computed in the example from its daily returns: annualised return and volatility in percent, the Sharpe ratio, and beta to the benchmark, one pane each on the same time axis, each named on its last-value line.',
      api: ["addSeries('baseline')", "addSeries('area')", 'addPane', 'setVisibleLogicalRange'],
    },
    {
      id: 'corrtime',
      title: 'Rolling correlation',
      group: 'Risk',
      code: `${create()}
${load('a', 'bars', { symbol: 'ZS', interval: '1d', count: 800 })}
${load('b', 'bars', { symbol: 'ZM', interval: '1d', count: 800 })}
// Daily log returns of soybeans and soybean meal.
const r = (c) => c.map((x, i) => (i ? Math.log(x / c[i - 1]) : 0));
const ra = r(a.close), rb = r(b.close), time = a.time;
const corr = (n) => time.map((_, i) => {
  if (i < n) return NaN;
  const x = ra.slice(i - n + 1, i + 1), y = rb.slice(i - n + 1, i + 1);
  const mx = x.reduce((s, v) => s + v, 0) / n, my = y.reduce((s, v) => s + v, 0) / n;
  let sxy = 0, sxx = 0, syy = 0;
  x.forEach((v, k) => { sxy += (v - mx) * (y[k] - my); sxx += (v - mx) ** 2; syy += (y[k] - my) ** 2; });
  return sxy / Math.sqrt(sxx * syy);
});

chart.addSeries('line', { color: '#ffb547', lineWidth: 1.5 }).setData({ time, value: a.close });
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.5 }).setData({ time: b.time, value: b.close });
chart.applyPriceScaleOptions({ mode: 'percentage' });
const pane = chart.addPane(0.9);
const long = corr(250);
chart.addSeries('line', { color: '${grey}', lineWidth: 1.5, lineStyle: 'dashed', pricePrecision: 2 }, pane).setData({ time, value: long });
const short = chart.addSeries('baseline', {
  baseValue: long.at(-1), color: '${lime}', downColor: '${down}',
  topFill: 'rgba(163, 230, 53, 0.25)', bottomFill: 'rgba(255, 93, 115, 0.3)', pricePrecision: 2,
}, pane);
short.setData({ time, value: corr(40) });
chart.setVisibleLogicalRange(250, time.length + 3);`,
      about: 'Soybeans (amber) and soybean meal (blue) on a `percentage` scale, and below, the 40-day correlation of their daily returns as a `baseline` around the 250-day level (dashed). Red stretches are when the usual link weakens.',
      api: ["addSeries('baseline')", 'baseValue', 'applyPriceScaleOptions'],
    },
    {
      id: 'varbt',
      title: 'VaR backtest exceptions',
      group: 'Risk',
      code: `${create()}
${load('p', 'series/quantPnl', { ...D, count: 750 })}
// One-day 99% historical VaR: the 1st percentile of the previous 250 days.
const window = 250;
const vaR = p.pnl.map((_, i) => {
  if (i < window) return NaN;
  const past = p.pnl.slice(i - window, i).sort((a, b) => a - b);
  return past[Math.floor(window * 0.01)];
});
const time = p.time.slice(window), pnl = p.pnl.slice(window), limit = vaR.slice(window);
const breach = pnl.map((v, i) => v < limit[i]);

const bars = chart.addSeries('histogram', { pricePrecision: 0 });
bars.setData({
  time, value: pnl,
  color: pnl.map((v, i) => (breach[i] ? '${down}' : v >= 0 ? 'rgba(46, 230, 182, 0.5)' : 'rgba(255, 93, 115, 0.4)')),
});
bars.setMarkers(time.filter((_, i) => breach[i]).map((t) => ({ time: t, position: 'belowBar', shape: 'arrowUp', color: '${down}' })));
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, step: true, pricePrecision: 0 }).setData({ time, value: limit });
chart.applyPriceScaleOptions({ margins: { top: 0.05, bottom: 0.12 } });
chart.fitContent();`,
      about: 'Daily P&L in thousands as a histogram against a rolling 99% historical VaR (amber step line). Days that lose more than the VaR are exceptions, colored and marked with `setMarkers`; at 99% about five a year are expected.',
      api: ["addSeries('histogram')", 'setMarkers', 'step lines'],
    },
    {
      id: 'lvar',
      title: 'Liquidity-adjusted VaR',
      group: 'Risk',
      code: `${create()}
${load('r', 'series/quantLiqRisk', D)}
// LVaR adds the cost of exiting the book at the current bid-ask spread.
const lvar = r.var.map((v, i) => v + r.liqCost[i]);
const varLine = chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.5 });
const lvarLine = chart.addSeries('line', { color: '#ffb547', lineWidth: 2 });
varLine.setData({ time: r.time, value: r.var });
lvarLine.setData({ time: r.time, value: lvar });
chart.addBand(lvarLine, varLine, { fill: 'rgba(255, 181, 71, 0.25)' });
lvarLine.createPriceLine({ price: lvar.at(-1), color: '#ffb547', lineStyle: 'dotted', axisLabelVisible: false, title: 'LVaR' });
varLine.createPriceLine({ price: r.var.at(-1), color: '#4cc9f0', lineStyle: 'dotted', axisLabelVisible: false, title: 'VaR' });

// The liquidity add-on as a share of VaR.
chart.addSeries('histogram', { color: 'rgba(255, 181, 71, 0.6)', pricePrecision: 0 }, chart.addPane(0.45))
  .setData({ time: r.time, value: r.liqCost.map((c, i) => (c / r.var[i]) * 100) });
chart.setBarSpacing(2);
chart.setRightOffset(6);
chart.scrollToRealtime();`,
      about: 'One-day 99% VaR and liquidity-adjusted VaR in millions, with `addBand` filling the liquidity add-on between them. The pane below shows the add-on as a percentage of VaR: it widens when markets fall and spreads blow out.',
      api: ['addBand', "addSeries('line')", "addSeries('histogram')"],
    },
    {
      id: 'cashstress',
      title: 'Cash stress test',
      group: 'Risk',
      code: `${create()}
${load('c', 'series/quantCash', W)}
const lines = {};
for (const [key, title, color, lineWidth] of [
  ['base', 'Base case', '${grey}', 1.5], ['stress', 'Stress', '#4cc9f0', 2], ['demand', 'Cash needed', '${down}', 1.5],
]) {
  lines[key] = chart.addSeries('line', { color, lineWidth });
  lines[key].setData({ time: c.time, value: c[key] });
  lines[key].createPriceLine({ price: c[key].at(-1), color, lineStyle: 'dotted', axisLabelVisible: false, title });
}
// Red where the cash needed exceeds the stressed balance: a shortfall.
chart.addBand(lines.demand, lines.stress, { fill: 'rgba(255, 93, 115, 0.45)', fillCrossed: 'transparent' });
chart.fitContent();`,
      about: 'A fund\'s weekly cash in millions under a base and a stress scenario, against the cash it could be asked for in margin calls and redemptions (red). `addBand` with a transparent `fillCrossed` fills only the shortfalls, where the need exceeds the stressed balance.',
      api: ['addBand', 'fillCrossed', 'createPriceLine'],
    },
    {
      id: 'fxhedge',
      title: 'Currency hedge effect',
      group: 'Risk',
      code: `${create()}
${load('spx', 'bars', { symbol: 'ES', interval: '1d', count: 500 })}
${load('fx', 'bars', { symbol: 'EURUSD', interval: '1d', count: 600 })}
// A euro investor holding US stocks. Unhedged returns include the dollar's
// move against the euro; the hedged ones pay a carry cost instead.
const eurusd = new Map(fx.time.map((t, i) => [t, fx.close[i]]));
const time = spx.time.filter((t) => eurusd.has(t));
const px = time.map((t) => spx.close[spx.time.indexOf(t)]);
const rate = time.map((t) => eurusd.get(t));
const cost = 0.02 / 252; // 2% a year
const hedgedV = px.map((x, i) => ((x / px[0]) * Math.exp(-cost * i) - 1) * 100);
const unhedgedV = px.map((x, i) => (((x / rate[i]) / (px[0] / rate[0])) - 1) * 100);

const hedged = chart.addSeries('line', { color: '${lime}', lineWidth: 2, pricePrecision: 1 });
const unhedged = chart.addSeries('line', { color: '${grey}', lineWidth: 1.5, pricePrecision: 1 });
hedged.setData({ time, value: hedgedV });
unhedged.setData({ time, value: unhedgedV });
// Green: the hedge added return; red: it cost return.
chart.addBand(hedged, unhedged, { fill: 'rgba(46, 230, 182, 0.22)', fillCrossed: 'rgba(255, 93, 115, 0.22)' });
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.5, pricePrecision: 4 }, chart.addPane(0.45)).setData({ time, value: rate });
chart.fitContent();`,
      about: 'Cumulative return in euros of US stocks, hedged (green) and unhedged (grey), aligned by date in the example. `addBand` shades the hedge\'s profit (green) or loss (red), and EUR/USD sits in the pane below: when the euro rises, the hedge pays.',
      api: ['addBand', 'fillCrossed', 'addPane'],
    },

    // ----------------------------------------------------------- Portfolio
    {
      id: 'alloc',
      title: 'Allocation & drift',
      group: 'Portfolio',
      code: `${create()}
${load('w', 'series/quantAlloc', W)}
// Stack the weights: each line is the running sum, in percent.
const assets = [
  ['equities', '${lime}', 60], ['bonds', '#4cc9f0', 25], ['gold', '#ffb547', 10], ['cash', '#b388ff', 5],
];
let sum = w.time.map(() => 0);
const zero = chart.addSeries('line', { color: 'transparent', lastValueVisible: false, priceLineVisible: false });
zero.setData({ time: w.time, value: sum });
let below = zero, target = 0;
for (const [name, color, goal] of assets) {
  sum = sum.map((s, i) => s + w[name][i] * 100);
  const line = chart.addSeries('line', { color, lineWidth: 1, pricePrecision: 1, lastValueVisible: false, priceLineVisible: false });
  line.setData({ time: w.time, value: sum });
  chart.addBand(line, below, { fill: color + '66' });
  target += goal;
  if (target < 100) line.createPriceLine({ price: target, color, lineStyle: 'dashed', title: name + ' target' });
  below = line;
}
chart.applyPriceScaleOptions({ range: { low: 0, high: 100 }, margins: { top: 0.03, bottom: 0.03 } });
chart.fitContent();`,
      about: 'Weekly weights of four asset classes stacked to 100%: each boundary is a running sum, and `addBand` fills between neighbouring lines. Dashed price lines mark the target boundaries (60/25/10/5), so the distance to them is the drift to rebalance away.',
      api: ['addBand', 'createPriceLine', 'applyPriceScaleOptions'],
    },
    {
      id: 'bump',
      title: 'Factor ranking (bump chart)',
      group: 'Portfolio',
      code: `${create()}
${load('s', 'series/quantFactorScores', M)}
const factors = [
  ['momentum', 'Momentum', '${lime}'], ['value', 'Value', '#4cc9f0'], ['quality', 'Quality', '#ffb547'],
  ['lowVol', 'Low vol', '#b388ff'], ['size', 'Size', '${down}'], ['carry', 'Carry', '#f0f0f0'],
];
// Rank 1 is the best score each month.
const rank = (k, i) => 1 + factors.filter(([other]) => s[other][i] > s[k][i]).length;
const last = s.time.at(-1);
for (const [key, label, color] of factors) {
  const value = s.time.map((_, i) => rank(key, i));
  const line = chart.addSeries('line', { color, lineWidth: 2.5, pricePrecision: 1, lastValueVisible: false, priceLineVisible: false });
  line.setData({ time: s.time, value });
  chart.addSeries('points', { color, lastValueVisible: false, priceLineVisible: false }).setData({ time: s.time, value });
  line.setMarkers([{ time: last, position: 'aboveBar', shape: 'circle', color, text: label }]);
}
chart.applyPriceScaleOptions({ inverted: true, range: { low: 0.5, high: 6.5 }, margins: { top: 0.04, bottom: 0.04 } });
chart.setBarSpacing(26);
chart.setRightOffset(3);
chart.scrollToRealtime();`,
      about: 'The monthly rank of six equity factors by trailing score: one `line` and `points` series per factor, with the price scale `inverted` so rank 1 is on top. A marker at the last month names each line.',
      api: ['applyPriceScaleOptions', 'inverted', "addSeries('points')", 'setMarkers'],
    },

    // ----------------------------------------------------------- Factor research
    {
      id: 'quantile',
      title: 'Quintile backtest',
      group: 'Factor research',
      code: `${create()}
${load('q', 'series/quantQuintiles', D)}
const colors = ['${down}', '#ff9a76', '${grey}', '#7fe0c4', '${up}'];
const growth = (v) => v.map((x) => (x / v[0] - 1) * 100);
['q1', 'q2', 'q3', 'q4', 'q5'].forEach((k, n) => {
  const value = growth(q[k]);
  const line = chart.addSeries('line', { color: colors[n], lineWidth: n === 0 || n === 4 ? 2 : 1.5, pricePrecision: 1 });
  line.setData({ time: q.time, value });
  line.createPriceLine({ price: value.at(-1), color: colors[n], lineStyle: 'dotted', axisLabelVisible: false, title: 'Q' + (n + 1) });
});
// Long the top quintile, short the bottom one.
const ls = q.q5.map((v, i) => ((v / q.q5[0]) / (q.q1[i] / q.q1[0]) - 1) * 100);
chart.addSeries('area', {
  color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.3)', bottomFill: 'rgba(255, 181, 71, 0.02)', pricePrecision: 1,
}, chart.addPane(0.6)).setData({ time: q.time, value: ls });
chart.fitContent();`,
      about: 'Cumulative return in percent of five portfolios sorted on a factor, from the bottom quintile (red) to the top (green). A clean fan with Q5 on top means the factor ranks returns; the pane below is the long Q5, short Q1 spread.',
      api: ["addSeries('line')", "addSeries('area')", 'createPriceLine'],
    },
    {
      id: 'ic',
      title: 'IC · Rank IC',
      group: 'Factor research',
      code: `${create()}
${load('s', 'series/quantIc', W)}
const ic = chart.addSeries('histogram', { pricePrecision: 3 });
ic.setData({
  time: s.time, value: s.ic,
  color: s.ic.map((v) => (v >= 0 ? 'rgba(46, 230, 182, 0.6)' : 'rgba(255, 93, 115, 0.6)')),
});
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.25, pricePrecision: 3 }).setData({ time: s.time, value: s.rankIc });
// 26-week rolling mean of the rank IC.
const n = 26;
const mean = s.rankIc.map((_, i) => (i < n - 1 ? NaN : s.rankIc.slice(i - n + 1, i + 1).reduce((a, b) => a + b, 0) / n));
chart.addSeries('line', { color: '#ffb547', lineWidth: 2.5, pricePrecision: 3 }).setData({ time: s.time, value: mean });
ic.createPriceLine({ price: 0, color: '${grey}', lineWidth: 1, axisLabelVisible: false });
chart.setBarSpacing(4);
chart.setRightOffset(4);
chart.scrollToRealtime();`,
      about: 'A factor\'s weekly information coefficient as a histogram colored by sign, the rank IC (blue) and its 26-week rolling mean (amber), with a zero price line. A mean that stays above zero is what a usable factor looks like.',
      api: ["addSeries('histogram')", 'per-point color', 'createPriceLine'],
    },
    {
      id: 'factorturn',
      title: 'Factor turnover',
      group: 'Factor research',
      code: `${create()}
${load('s', 'series/quantTurnover', { ...M, count: 60 })}
const pct = s.turnover.map((v) => v * 100);
const avg = pct.reduce((a, b) => a + b, 0) / pct.length;
const bars = chart.addSeries('histogram', { pricePrecision: 1 });
// Amber: rebalances that traded half again more than usual.
bars.setData({ time: s.time, value: pct, color: pct.map((v) => (v > avg * 1.5 ? '#ffb547' : 'rgba(163, 230, 53, 0.6)')) });
bars.createPriceLine({ price: avg, color: '#f0f0f0', lineStyle: 'dashed', title: 'average ' + avg.toFixed(0) + '%' });
chart.applyPriceScaleOptions({ margins: { top: 0.1, bottom: 0 } });
chart.setBarSpacing(16);
chart.setRightOffset(2);
chart.scrollToRealtime();`,
      about: 'The share of a monthly-rebalanced factor portfolio traded at each rebalance, in percent, with the average as a dashed price line. Months well above it are highlighted: turnover is what trading costs scale with.',
      api: ["addSeries('histogram')", 'createPriceLine', 'per-point color'],
    },
    {
      id: 'regime',
      title: 'Market regimes',
      group: 'Factor research',
      code: `${create()}
${load('bars', 'bars', { symbol: 'BTCUSD', interval: '1d', count: 520 })}
chart.addCandlestickSeries().setData(bars);

// A simple regime model: trend from a 50-day average, stress from
// 20-day volatility above its one-year median.
const c = bars.close, n = c.length;
const sma = c.map((_, i) => (i < 49 ? NaN : c.slice(i - 49, i + 1).reduce((a, b) => a + b, 0) / 50));
const r = c.map((x, i) => (i ? Math.log(x / c[i - 1]) : 0));
const vol = r.map((_, i) => (i < 20 ? NaN : Math.sqrt(r.slice(i - 19, i + 1).reduce((a, b) => a + b * b, 0) / 20)));
const median = [...vol.filter((v) => v > 0)].sort((a, b) => a - b)[Math.floor(n / 2)];
const raw = c.map((x, i) => (i < 50 ? null : vol[i] > median * 1.25 ? 'stress' : x > sma[i] ? 'bull' : 'bear'));
// Merge into runs, ignoring flips shorter than five days.
const runs = [];
raw.forEach((s, i) => {
  if (!s) return;
  const cur = runs.at(-1);
  if (cur && cur.state === s) cur.to = i;
  else if (cur && i - cur.from < 5) { cur.state = s; cur.to = i; }
  else runs.push({ state: s, from: i, to: i });
});
const shade = { bull: 'rgba(46, 230, 182, 0.12)', bear: 'rgba(255, 93, 115, 0.12)', stress: 'rgba(255, 181, 71, 0.18)' };
const legend = [['Bull', '${up}'], ['Bear', '${down}'], ['Stress', '#ffb547']];
const day = 86400000;

chart.addLayer({
  pane: 0,
  position: 'background',
  build(ctx) {
    const { y, height } = ctx.rect, out = [];
    for (const run of runs) {
      const a = ctx.timeToX(bars.time[run.from] - day / 2), b = ctx.timeToX(bars.time[run.to] + day / 2);
      if (a === null || b === null) continue;
      out.push({ type: 'rect', x: a, y, width: b - a, height, color: shade[run.state] });
    }
    let x = ctx.rect.x + 10;
    for (const [label, color] of legend) {
      out.push({ type: 'rect', x, y: y + 9, width: 10, height: 10, color });
      out.push({ type: 'text', x: x + 15, y: y + 14, text: label, color: '#c4cad6', size: 12 });
      x += 24 + ctx.textWidth(label, 12);
    }
    return out;
  },
});
chart.setBarSpacing(3);
chart.setRightOffset(4);
chart.scrollToRealtime();`,
      about: 'Daily candles over shaded market regimes: bull above a 50-day average, bear below it, and stress when 20-day volatility runs well above its median. The regimes are classified in the example and a layer plugin draws them as background bands with a legend.',
      api: ['addLayer', 'ctx.timeToX', "position: 'background'"],
    },
    ...plotDemos.quant,
  ],
};
