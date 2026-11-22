import { bars, create, load } from './build';
import { plotDemos } from './plots';
import type { Category } from './types';

const T = { symbol: 'US10Y', interval: '1d' };
const EUR = { symbol: 'EURUSD', interval: '1d' };

export const rates: Category = {
  id: 'rates',
  title: 'Rates, FX & macro',
  kicker: '13',
  blurb: 'Curves, spreads, auctions, central banks, carry and macro surprises.',
  accent: '#38bdf8',
  demos: [
    {
      id: 'curveshape',
      title: 'Curve level · slope · curvature',
      group: 'Curves & spreads',
      code: `${create()}
${load('curve', 'series/ustCurveFactors', { ...T, count: 750 })}
chart.addSeries('area', { color: '#38bdf8', topFill: 'rgba(56, 189, 248, 0.25)', bottomFill: 'rgba(56, 189, 248, 0)', lineWidth: 2 })
  .setData({ time: curve.time, value: curve.level });
// 10y minus 2y in basis points: red while the curve is inverted
chart.addSeries('baseline', {
  baseValue: 0, pricePrecision: 0,
  color: '#2ee6b6', downColor: '#ff5d73',
  topFill: 'rgba(46, 230, 182, 0.2)', bottomFill: 'rgba(255, 93, 115, 0.25)',
}, chart.addPane(0.7)).setData({ time: curve.time, value: curve.slope });
const belly = chart.addSeries('line', { color: '#b388ff', lineWidth: 1.5, pricePrecision: 0 }, chart.addPane(0.6));
belly.setData({ time: curve.time, value: curve.curvature });
belly.createPriceLine({ price: 0, color: '#8a93a6', lineStyle: 'dashed', axisLabelVisible: false });
chart.fitContent();`,
      about: 'A yield curve in three numbers: the level (the 10-year yield, percent), the slope (10s minus 2s, bp) as a baseline series that turns red when the curve inverts, and the curvature of the belly (2 × 5y − 2y − 10y, bp).',
      api: ["addSeries('baseline')", 'addPane', 'createPriceLine'],
    },
    {
      id: 'breakeven',
      title: 'Real yield · breakeven inflation',
      group: 'Curves & spreads',
      code: `${create()}
${bars({ ...T, count: 500 })}
${load('real', 'series/ustReal', { ...T, count: 500 })}
const nominal = chart.addSeries('line', { color: '#38bdf8', lineWidth: 2 });
nominal.setData({ time: bars.time, value: bars.close });
const realYield = chart.addSeries('line', { color: '#b388ff', lineWidth: 2 });
realYield.setData({ time: real.time, value: real.real });
chart.addBand(nominal, realYield, { fill: 'rgba(255, 181, 71, 0.14)' });
// Breakeven inflation: nominal minus real
chart.addSeries('area', { color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.3)', bottomFill: 'rgba(255, 181, 71, 0)' }, chart.addPane(0.6))
  .setData({ time: bars.time, value: bars.close.map((y, i) => y - real.real[i]) });
chart.setBarSpacing(2);
chart.scrollToRealtime();`,
      about: 'The 10-year nominal yield (blue) and the real yield of inflation-linked bonds (violet). The gap between them, shaded with `addBand` and plotted below, is breakeven inflation: what the market expects inflation to average.',
      api: ['addBand', "addSeries('area')"],
    },
    {
      id: 'swapspread',
      title: 'Swap spread',
      group: 'Curves & spreads',
      code: `${create()}
${bars({ ...T, count: 500 })}
${load('swap', 'series/swapSpread', { ...T, count: 500 })}
chart.addSeries('line', { color: '#38bdf8', lineWidth: 2 }).setData({ time: bars.time, value: bars.close });
// Swap rate = treasury yield + spread (bp)
chart.addSeries('line', { color: '#ffb547', lineWidth: 2 })
  .setData({ time: bars.time, value: bars.close.map((y, i) => y + swap.spread[i] / 100) });
chart.addSeries('baseline', {
  baseValue: 0, pricePrecision: 1,
  color: '#2ee6b6', downColor: '#ff5d73',
  topFill: 'rgba(46, 230, 182, 0.2)', bottomFill: 'rgba(255, 93, 115, 0.22)',
}, chart.addPane(0.6)).setData({ time: swap.time, value: swap.spread });
chart.setBarSpacing(2);
chart.scrollToRealtime();`,
      about: 'The 10-year swap rate (amber) against the treasury yield (blue), and their difference in basis points as a baseline series. A negative swap spread, swaps yielding less than treasuries, shows in red.',
      api: ["addSeries('baseline')", "addSeries('line')"],
    },
    {
      id: 'spreadpct',
      title: 'Credit spread percentiles',
      group: 'Credit & issuance',
      code: `${create()}
${load('credit', 'series/creditSpread', { symbol: 'NQ', interval: '1d', count: 1250 })}
const s = credit.spread;
const sorted = [...s].sort((a, b) => a - b);
const pct = (p) => sorted[Math.round((p / 100) * (sorted.length - 1))];
const flat = (v) => ({ time: credit.time, value: credit.time.map(() => v) });
const guide = { color: '#8a93a6', lineWidth: 1, lineStyle: 'dashed', lastValueVisible: false, priceLineVisible: false };
const edge = { ...guide, lineStyle: 'solid', color: 'rgba(138, 147, 166, 0.6)' };
const p90 = chart.addSeries('line', edge);
p90.setData(flat(pct(90)));
const p10 = chart.addSeries('line', edge);
p10.setData(flat(pct(10)));
chart.addSeries('line', { ...guide, lineStyle: 'dotted' }).setData(flat(pct(50)));
const line = chart.addSeries('line', { color: '#38bdf8', lineWidth: 2, pricePrecision: 0, lastValueVisible: false });
line.setData({ time: credit.time, value: s });
// Where today's spread ranks in its five-year history
const rank = Math.round((sorted.filter((v) => v <= s[s.length - 1]).length / sorted.length) * 100);
line.createPriceLine({ price: s[s.length - 1], color: '#ffb547', title: \`\${rank}th pct\` });
chart.addBand(p90, p10, { fill: 'rgba(56, 189, 248, 0.08)' });
for (const p of [10, 50, 90]) p10.createPriceLine({ price: pct(p), color: '#8a93a6', lineStyle: 'dashed', title: \`p\${p}\`, axisLabelVisible: false });
chart.fitContent();`,
      about: 'A high-yield credit spread over five years against its own distribution: the 10th–90th percentile range shaded with `addBand`, the median dotted, and today\'s percentile rank as a price line.',
      api: ['addBand', 'createPriceLine', "addSeries('line')"],
    },
    {
      id: 'maturitywall',
      title: 'Maturity wall',
      group: 'Credit & issuance',
      code: `${create({ maxBarSpacing: 90 })}
${load('wall', 'series/debtMaturities', { symbol: 'ES', interval: '1y', count: 1, ahead: 10 })} // this year and the next ten
// Stacked: the total behind, high yield in front
chart.addSeries('histogram', { color: '#38bdf8', pricePrecision: 0 })
  .setData({ time: wall.time, value: wall.ig.map((v, i) => v + wall.hy[i]) });
chart.addSeries('histogram', { color: '#ffb547', pricePrecision: 0 })
  .setData({ time: wall.time, value: wall.hy });
chart.fitContent();`,
      about: 'Corporate bonds coming due per year, in USD bn: investment grade (blue) stacked on high yield (amber). One bar per calendar year from the `1y` interval with `ahead`, so the time axis runs ten years into the future.',
      api: ["addSeries('histogram')", 'future timestamps'],
    },
    {
      id: 'cashflowladder',
      title: 'Bond cash-flow ladder',
      group: 'Credit & issuance',
      code: `${create({ maxBarSpacing: 90 })}
// A portfolio: annual coupon (percent), coupons left until maturity, face value (USD)
const bonds = [
  { coupon: 4.25, payments: 4, face: 2_000_000, color: '#38bdf8' },
  { coupon: 3.875, payments: 7, face: 1_500_000, color: '#4cc9f0' },
  { coupon: 4.5, payments: 11, face: 2_500_000, color: '#b388ff' },
  { coupon: 4.0, payments: 15, face: 1_000_000, color: '#2ee6b6' },
  { coupon: 4.625, payments: 20, face: 3_000_000, color: '#ffb547' },
];
// Semi-annual payment dates: the 15th of June and December
const now = new Date();
const first = Date.UTC(now.getUTCFullYear(), now.getUTCMonth() < 5 ? 5 : 11, 15);
const time = Array.from({ length: 20 }, (_, k) => {
  const d = new Date(first);
  d.setUTCMonth(d.getUTCMonth() + 6 * k);
  return d.getTime();
});
// Coupons stacked by bond: running sums, drawn last bond first
const sum = time.map(() => 0);
const layers = bonds.map((b) => time.map((_, k) => (sum[k] += k < b.payments ? (b.face * b.coupon) / 200 : 0)));
bonds.map((b, i) => [b.color, layers[i]]).reverse().forEach(([color, value]) => {
  chart.addSeries('histogram', { color, pricePrecision: 0, lastValueVisible: false, priceLineVisible: false }).setData({ time, value });
});
const principal = time.map((_, k) => bonds.reduce((s, b) => s + (b.payments === k + 1 ? b.face : 0), 0));
chart.addSeries('histogram', { color: '#e6e9ef', pricePrecision: 0, lastValueVisible: false, priceLineVisible: false }, chart.addPane(0.6))
  .setData({ time, value: principal.map((v) => v || NaN) });
chart.fitContent();`,
      about: 'Ten years of cash flows from a five-bond portfolio, one slot per semi-annual payment date: coupons stacked by bond above, steps down as each bond matures, and principal repayments below. The schedule is built in a few lines of JavaScript; every timestamp is in the future.',
      api: ["addSeries('histogram')", 'future timestamps', 'addPane'],
    },
    {
      id: 'tsyauction',
      title: 'Treasury auction results',
      group: 'Central banks & auctions',
      code: `${create({ maxBarSpacing: 60 })}
${bars({ ...T, count: 730 })}
${load('auctions', 'events/tsyAuctions', { ...T, count: 730 })}
${load('stats', 'series/tsyAuctionStats', { ...T, count: 730 })}
// One slot per auction: read the daily series on auction days
const day = new Map(bars.time.map((t, i) => [t, i]));
const at = (col) => auctions.time.map((t) => col[day.get(t)]);
const tail = auctions.value; // bp: stop-out minus when-issued yield
const stop = chart.addSeries('line', { color: '#ffb547', lineWidth: 2 });
stop.setData({ time: auctions.time, value: at(bars.close).map((y, i) => y + tail[i] / 100) });
chart.addSeries('points', { color: '#ffb547', lineWidth: 7, lastValueVisible: false, priceLineVisible: false })
  .setData({ time: auctions.time, value: at(bars.close).map((y, i) => y + tail[i] / 100) });
const cover = chart.addSeries('histogram', { color: '#38bdf8', pricePrecision: 2 }, chart.addPane(0.8));
const btc = at(stats.bidToCover);
cover.setData({ time: auctions.time, value: btc });
cover.createPriceLine({ price: btc.reduce((a, b) => a + b) / btc.length, color: '#8a93a6', lineStyle: 'dashed', title: 'avg' });
chart.addSeries('histogram', { pricePrecision: 1 }, chart.addPane(0.8)).setData({
  time: auctions.time,
  value: tail,
  color: tail.map((v) => (v > 0 ? '#ff5d73' : '#2ee6b6')),
});
chart.fitContent();`,
      about: 'Two years of monthly 10-year auctions on an axis of auction dates only: the stop-out yield, the bid-to-cover ratio against its average, and the tail in basis points, red when the auction cleared above the when-issued yield.',
      api: ["addSeries('points')", "addSeries('histogram')", 'per-point color'],
    },
    {
      id: 'cbbs',
      title: 'Central bank balance sheets',
      group: 'Central banks & auctions',
      code: `${create()}
${load('cb', 'series/cbBalanceSheets', { symbol: 'ES', interval: '1mo', count: 120 })}
const banks = [
  ['bankA', '#38bdf8', '#164e68'],
  ['bankB', '#b388ff', '#40306a'],
  ['bankC', '#2ee6b6', '#145846'],
  ['bankD', '#ffb547', '#6a4a1c'],
];
// Stacked areas: cumulative sums, the top layer drawn first
const sum = cb.time.map(() => 0);
const layers = banks.map(([key, line, fill]) => [line, fill, cb[key].map((v, i) => (sum[i] += v))]);
for (const [color, fill, value] of layers.reverse()) {
  chart.addSeries('area', { color, topFill: fill, bottomFill: fill, lineWidth: 1.5, pricePrecision: 1 }).setData({ time: cb.time, value });
}
chart.fitContent();`,
      about: 'Total assets of four central banks in USD tn over ten years of month-end values, stacked: each area is a running sum, and the largest is drawn first so the smaller ones sit in front.',
      api: ["addSeries('area')", "interval '1mo'", 'stacking'],
    },
    {
      id: 'macroev',
      title: 'Macro data surprises',
      group: 'Central banks & auctions',
      code: `${create()}
${bars({ ...EUR, count: 130 })}
${load('releases', 'events/macroReleases', { ...EUR, count: 130 })}
const candles = chart.addCandlestickSeries({ pricePrecision: 4 });
candles.setData(bars);
const z = releases.value; // actual minus consensus, in standard deviations
candles.setMarkers(releases.time.flatMap((time, i) => (Math.abs(z[i]) < 2 ? [] : [{
  time,
  position: z[i] > 0 ? 'belowBar' : 'aboveBar',
  shape: z[i] > 0 ? 'arrowUp' : 'arrowDown',
  color: z[i] > 0 ? '#2ee6b6' : '#ff5d73',
  text: releases.label[i],
}])));
chart.addSeries('histogram', { pricePrecision: 1 }, chart.addPane(0.6)).setData({
  time: releases.time,
  value: z,
  color: z.map((v) => (v >= 0 ? '#2ee6b6' : '#ff5d73')),
});
chart.setBarSpacing(8);
chart.scrollToRealtime();`,
      about: 'Each data release as a bar of its surprise, actual minus consensus in standard deviations, under daily EUR/USD candles. The biggest surprises are also markers on the candles, labelled with the release.',
      api: ['Series.setMarkers', "addSeries('histogram')", 'per-point color'],
    },
    {
      id: 'carry',
      title: 'FX carry · drawdown',
      group: 'FX',
      code: `${create()}
${load('carry', 'series/fxCarry', { symbol: 'USDJPY', interval: '1d', count: 1250 })}
chart.addSeries('area', { color: '#38bdf8', topFill: 'rgba(56, 189, 248, 0.25)', bottomFill: 'rgba(56, 189, 248, 0)', lineWidth: 2 })
  .setData({ time: carry.time, value: carry.index });
// Drawdown: percent below the running peak
let peak = -Infinity;
const drawdown = carry.index.map((v) => ((peak = Math.max(peak, v)), (v / peak - 1) * 100));
chart.addSeries('baseline', { baseValue: 0, color: '#ff5d73', downColor: '#ff5d73', topFill: 'rgba(0, 0, 0, 0)', bottomFill: 'rgba(255, 93, 115, 0.35)', pricePrecision: 1 }, chart.addPane(0.6))
  .setData({ time: carry.time, value: drawdown });
chart.fitContent();`,
      about: 'Total return of an FX carry basket, long high-yielding currencies against low-yielding funders, and its drawdown from the running peak below: carry earns slowly and gives back fast.',
      api: ["addSeries('area')", "addSeries('baseline')", 'addPane'],
    },
    {
      id: 'ratediff',
      title: 'Rate differential vs FX',
      group: 'FX',
      code: `${create()}
${bars({ symbol: 'USDJPY', interval: '1d', count: 500 })}
${load('diff', 'series/usJpRateDiff', { symbol: 'USDJPY', interval: '1d', count: 500 })}
chart.addSeries('line', { color: '#38bdf8', lineWidth: 2 }).setData({ time: bars.time, value: bars.close });
chart.addSeries('line', { color: '#ffb547', lineWidth: 2 }).setData({ time: diff.time, value: diff.diff });
// Two units on one scale: both rebased to 100 at the first visible bar
chart.applyPriceScaleOptions({ mode: 'indexedTo100' });
chart.addSeries('line', { color: '#ffb547', lineWidth: 1.5, pricePrecision: 2 }, chart.addPane(0.45))
  .setData({ time: diff.time, value: diff.diff });
chart.setBarSpacing(2);
chart.scrollToRealtime();`,
      about: 'USD/JPY (blue) against the two-year yield gap between the two countries (amber), both rebased to 100 with `indexedTo100` so a percent and an exchange rate share one scale. The lower pane shows the gap itself, in percent.',
      api: ["applyPriceScaleOptions({ mode: 'indexedTo100' })", 'addPane'],
    },
    {
      id: 'reer',
      title: 'Real effective exchange rate',
      group: 'FX',
      code: `${create()}
${bars({ ...EUR, count: 1000 })}
${load('eff', 'series/eurEffective', { ...EUR, count: 1000 })}
chart.addSeries('line', { color: 'rgba(56, 189, 248, 0.6)', lineWidth: 1 }).setData({ time: bars.time, value: bars.close });
chart.addSeries('line', { color: '#ffb547', lineWidth: 2 }).setData({ time: eff.time, value: eff.neer });
chart.addSeries('line', { color: '#b388ff', lineWidth: 2 }).setData({ time: eff.time, value: eff.reer });
chart.applyPriceScaleOptions({ mode: 'indexedTo100' });
chart.fitContent();`,
      about: 'EUR/USD (blue) against the euro\'s nominal (amber) and inflation-adjusted real (violet) effective exchange rates, trade-weighted against many currencies. `indexedTo100` rebases all three to the first visible bar as you pan.',
      api: ["applyPriceScaleOptions({ mode: 'indexedTo100' })"],
    },
    {
      id: 'ppp',
      title: 'Purchasing power parity gap',
      group: 'FX',
      code: `${create()}
${bars({ ...EUR, count: 1250 })}
${load('ppp', 'series/eurPpp', { ...EUR, count: 1250 })}
const spot = chart.addSeries('line', { color: '#38bdf8', lineWidth: 2, pricePrecision: 4 });
spot.setData({ time: bars.time, value: bars.close });
const fair = chart.addSeries('line', { color: 'rgba(230, 233, 239, 0.8)', lineWidth: 1.5, pricePrecision: 4 });
fair.setData({ time: ppp.time, value: ppp.fair });
// Above fair value: the euro is overvalued
chart.addBand(spot, fair, { fill: 'rgba(255, 93, 115, 0.22)', fillCrossed: 'rgba(46, 230, 182, 0.22)' });
chart.addSeries('baseline', {
  baseValue: 0, pricePrecision: 1,
  color: '#ff5d73', downColor: '#2ee6b6',
  topFill: 'rgba(255, 93, 115, 0.25)', bottomFill: 'rgba(46, 230, 182, 0.25)',
}, chart.addPane(0.5)).setData({ time: bars.time, value: bars.close.map((v, i) => (v / ppp.fair[i] - 1) * 100) });
chart.fitContent();`,
      about: 'EUR/USD against its purchasing-power-parity fair value. `addBand` with `fillCrossed` shades the gap red where the euro is overvalued and green where it is undervalued; the lower pane is the gap in percent.',
      api: ['addBand', 'fillCrossed', "addSeries('baseline')"],
    },
    {
      id: 'fxsessions',
      title: 'FX trading sessions',
      group: 'FX',
      code: `${create()}
${bars({ symbol: 'EURUSD', interval: '15m', count: 480 })}
chart.addCandlestickSeries({ pricePrecision: 4 }).setData(bars);
// Hours in UTC
const sessions = [
  { name: 'Asia', from: 0, to: 9, color: 'rgba(76, 201, 240, 0.08)', text: '#4cc9f0' },
  { name: 'Europe', from: 7, to: 16, color: 'rgba(179, 136, 255, 0.08)', text: '#b388ff' },
  { name: 'US', from: 12, to: 21, color: 'rgba(255, 181, 71, 0.08)', text: '#ffb547' },
];
const DAY = 86_400_000, HOUR = 3_600_000;
chart.addLayer({
  pane: chart.mainPane,
  position: 'background',
  build(ctx) {
    const { x, y, width, height } = ctx.rect;
    const t0 = ctx.xToTime(x), t1 = ctx.xToTime(x + width);
    if (t0 === null || t1 === null) return [];
    const out = [];
    for (let day = Math.floor(t0 / DAY) * DAY; day <= t1; day += DAY) {
      const weekday = new Date(day).getUTCDay();
      if (weekday === 0 || weekday === 6) continue;
      sessions.forEach((s, row) => {
        const x1 = ctx.timeToX(day + s.from * HOUR), x2 = ctx.timeToX(day + s.to * HOUR);
        if (x1 === null || x2 === null) return;
        out.push({ type: 'rect', x: x1, y, width: x2 - x1, height, color: s.color });
        out.push({ type: 'line', x1, y1: y + 8 + row * 16, x2, y2: y + 8 + row * 16, color: s.text, width: 2 });
        if (x2 - x1 > 50) out.push({ type: 'text', x: x1 + 4, y: y + 15 + row * 16, text: s.name, color: s.text, size: 10 });
      });
    }
    return out;
  },
});
chart.applyPriceScaleOptions({ margins: { top: 0.18, bottom: 0.08 } });
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Fifteen-minute EUR/USD candles over the Asian, European and US sessions. A background layer plugin shades each session and draws its span and name along the top; the overlaps, when two centres trade at once, show as mixed bands.',
      api: ['addLayer', 'ctx.xToTime', 'ctx.timeToX'],
    },
    {
      id: 'bop',
      title: 'Balance of payments',
      group: 'FX',
      code: `${create()}
${load('bop', 'series/bopFlows', { symbol: 'ES', interval: '1q', count: 40 })}
// Signed stacking: positives build up from zero, negatives down
const stack = (pane, parts) => {
  const pos = bop.time.map(() => 0), neg = bop.time.map(() => 0);
  const layers = parts.map(([key, color]) => [color, bop[key].map((v, i) => (v >= 0 ? (pos[i] += v) : (neg[i] += v)))]);
  for (const [color, value] of layers.reverse()) {
    chart.addSeries('histogram', { color, pricePrecision: 0, lastValueVisible: false, priceLineVisible: false }, pane).setData({ time: bop.time, value });
  }
  const net = bop.time.map((_, i) => parts.reduce((s, [key]) => s + bop[key][i], 0));
  chart.addSeries('line', { color: '#e6e9ef', lineWidth: 2, pricePrecision: 0 }, pane).setData({ time: bop.time, value: net });
};
stack(chart.mainPane, [['goods', '#ff5d73'], ['services', '#38bdf8'], ['income', '#b388ff']]);
stack(chart.addPane(1), [['direct', '#ffb547'], ['portfolio', '#2ee6b6'], ['other', '#8a93a6']]);
chart.fitContent();`,
      about: 'Ten years of quarterly flows in USD bn: the current account (goods, services, income) above and the financial account (direct, portfolio, other investment) below. The components are stacked by sign in JavaScript; the white lines are the balances.',
      api: ["addSeries('histogram')", "interval '1q'", 'stacking'],
    },
    ...plotDemos.rates,
  ],
};
