import { bars, create, load } from './build';
import type { Query } from './build';
import type { Category } from './types';

const btc = (interval: string, count: number): Query => ({ symbol: 'BTCUSD', interval, count });
const eth = (interval: string, count: number): Query => ({ symbol: 'ETHUSD', interval, count });
const D = btc('1d', 1500);

/** Price as a thin line (daily cycles are too dense for candles). */
const PRICE = `const price = chart.addSeries('line', { color: '#d9dee8', lineWidth: 1.5 });
price.setData({ time: bars.time, value: bars.close });`;

/** A pane title, drawn by a one-line layer plugin. */
const TITLE = `const title = (pane, text) => chart.addLayer({ pane, build: (ctx) => [
  { type: 'text', x: ctx.rect.x + 8, y: ctx.rect.y + 6, text, color: '#8a93a6', size: 11 },
] });`;

/** A legend in the top-left corner of `pane`, drawn by a layer plugin. */
const LEGEND = `const legend = (pane, items) => chart.addLayer({ pane, position: 'overlay', build: (ctx) => items.flatMap(([text, color], i) => [
  { type: 'rect', x: ctx.rect.x + 10, y: ctx.rect.y + 13 + i * 16, width: 12, height: 3, color },
  { type: 'text', x: ctx.rect.x + 28, y: ctx.rect.y + 8 + i * 16, text, color: '#c9d1e0', size: 11 },
]) });`;

export const onchain: Category = {
  id: 'onchain',
  title: 'On-chain',
  kicker: '07',
  blurb: 'Valuation, supply, holder cohorts and network activity under price.',
  accent: '#f7931a',
  demos: [
    {
      id: 'mvrv',
      title: 'MVRV · realised price',
      group: 'Valuation',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '1d', count: 1500 })}
${load('realized', 'metrics/realizedPrice', { symbol: 'BTCUSD', interval: '1d', count: 1500 })}
${load('mvrv', 'metrics/mvrv', { symbol: 'BTCUSD', interval: '1d', count: 1500 })}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
chart.addSeries('line', { color: '#ffb547', lineWidth: 2 }).setData({ time: realized.time, value: realized.value });
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
const pane = chart.addPane(0.7);
chart.addSeries('baseline', { baseValue: 1, color: '#ff5d73', downColor: '#2ee6b6' }, pane)
  .setData({ time: mvrv.time, value: mvrv.value });
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Price against the average cost of all holders on a log scale; MVRV above 1 means holders are in profit on average.',
      api: ["mode: 'logarithmic'", "addSeries('baseline')"],
    },
    {
      id: 'mvrvband',
      title: 'Realised price bands',
      group: 'Valuation',
      code: `${create()}
${bars(D)}
${load('realized', 'metrics/realizedPrice', D)}
${load('mvrv', 'metrics/mvrv', D)}
// Multiples of the realised price (the holders' average cost), filled between.
const multiples = [0.75, 1, 1.25, 1.5];
const fills = ['rgba(46, 230, 182, 0.22)', 'rgba(76, 201, 240, 0.14)', 'rgba(255, 181, 71, 0.2)'];
const lines = multiples.map((m) => {
  const line = chart.addSeries('line', { color: 'rgba(255, 181, 71, 0.6)', lineWidth: m === 1 ? 2 : 1, lastValueVisible: false, priceLineVisible: false });
  line.setData({ time: realized.time, value: realized.value.map((v) => v * m) });
  return line;
});
fills.forEach((fill, i) => chart.addBand(lines[i + 1], lines[i], { fill }));
${PRICE}
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
const pane = chart.addPane(0.5);
const ratio = chart.addSeries('line', { color: '#b388ff', lineWidth: 1.5, pricePrecision: 2 }, pane);
ratio.setData({ time: mvrv.time, value: mvrv.value });
ratio.createPriceLine({ price: 1, color: '#5c6577', lineStyle: 'dashed' });
chart.fitContent();`,
      about: 'Price on a log scale over bands at 0.75×, 1×, 1.25× and 1.5× the realised price, filled with `addBand`; below, MVRV (price over realised price). Green is below the holders\' cost.',
      api: ['addBand', "mode: 'logarithmic'", 'createPriceLine'],
    },
    {
      id: 'hodlwaves',
      title: 'HODL waves',
      group: 'Holders',
      code: `${create()}
${bars(btc('1w', 400))}
${load('age', 'series/holderAge', btc('1w', 400))}
${PRICE}
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
const cohorts = [
  ['lt1m', 'under 1m', '#ff5d73'], ['m1to3', '1-3m', '#ff8a5c'], ['m3to6', '3-6m', '#ffb547'], ['m6to12', '6-12m', '#ffe08a'],
  ['y1to2', '1-2y', '#9be7c4'], ['y2to3', '2-3y', '#4cc9f0'], ['y3to5', '3-5y', '#5a8dee'], ['y5plus', '5y+', '#7b61ff'],
];
// Stack the shares in percent; draw each band from its running total down, the largest first.
const pane = chart.addPane(1.6);
let sum = age.time.map(() => 0);
const totals = cohorts.map(([col]) => (sum = sum.map((s, i) => s + age[col][i] * 100)));
for (let k = cohorts.length - 1; k >= 0; k--) {
  const color = cohorts[k][2];
  chart.addSeries('area', { color: '#0b0d12', lineWidth: 1, topFill: color, bottomFill: color, lastValueVisible: false, priceLineVisible: false }, pane)
    .setData({ time: age.time, value: totals[k] });
}
chart.applyPriceScaleOptions({ range: { low: 0, high: 100 }, margins: { top: 0, bottom: 0 } }, pane);
const last = age.time.length - 1;
chart.addLayer({
  pane,
  position: 'overlay',
  build: (ctx) => cohorts.map(([col, label], k) => ({
    type: 'text', x: ctx.rect.x + ctx.rect.width - 8, y: ctx.priceToY(totals[k][last] - age[col][last] * 50) - 7,
    text: label, color: '#0b0d12', size: 11, align: 'right',
  })),
});
chart.fitContent();`,
      about: 'Weekly supply by time since the coins last moved, stacked to 100% under the price: young coins (warm) swell in rallies as old coins are spent. The stack is built from cumulative sums of the shares; a layer plugin labels the bands.',
      api: ["addSeries('area')", "applyPriceScaleOptions({ range })", 'addLayer'],
    },
    {
      id: 'sopr',
      title: 'SOPR-colored price',
      group: 'Valuation',
      code: `${create()}
${bars(btc('1d', 500))}
${load('sopr', 'series/sopr', btc('1d', 500))}
// A 7-day average of the spent output profit ratio: above 1, coins move at a profit.
const avg = sopr.value.map((_, i, v) => {
  const w = v.slice(Math.max(0, i - 6), i + 1);
  return w.reduce((a, b) => a + b, 0) / w.length;
});
// Two line series, each with gaps (NaN) outside its regime; a segment runs one point on so they join.
const profit = avg.map((r) => r >= 1);
const regime = (want) => bars.close.map((c, i) => (profit[i] === want || profit[i - 1] === want ? c : NaN));
const opts = { lineWidth: 2, lastValueVisible: false, priceLineVisible: false };
chart.addSeries('line', { ...opts, color: '#2ee6b6' }).setData({ time: bars.time, value: regime(true) });
chart.addSeries('line', { ...opts, color: '#ff5d73' }).setData({ time: bars.time, value: regime(false) });
chart.addSeries('baseline', { baseValue: 1, color: '#2ee6b6', downColor: '#ff5d73', pricePrecision: 3 }, chart.addPane(0.6))
  .setData({ time: sopr.time, value: avg });
chart.fitContent();`,
      about: 'The price is drawn green while the 7-day SOPR is above 1 (coins being spent are sold at a profit) and red below, as two line series with `NaN` gaps. The ratio itself is a `baseline` series around 1.',
      api: ["addSeries('line')", 'NaN gaps', "addSeries('baseline')"],
    },
    {
      id: 'nupl',
      title: 'NUPL cycle zones',
      group: 'Valuation',
      code: `${create()}
${bars(D)}
${load('mvrv', 'metrics/mvrv', D)}
${LEGEND}
${PRICE}
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
// Net unrealised profit/loss as a share of market cap: 1 - realised / market value.
const nupl = mvrv.value.map((m) => 1 - 1 / m);
const zones = [[0.5, 'Belief', '#2ee6b6'], [0.25, 'Optimism', '#4cc9f0'], [0, 'Hope', '#ffb547'], [-Infinity, 'Capitulation', '#ff5d73']];
const zoneColor = (v) => zones.find(([lo]) => v >= lo)[2];
const pane = chart.addPane(0.8);
const series = chart.addSeries('histogram', { pricePrecision: 2, lastValueVisible: false, priceLineVisible: false }, pane);
series.setData({ time: mvrv.time, value: nupl, color: nupl.map(zoneColor) });
for (const [lo, , color] of zones.slice(0, 3)) {
  series.createPriceLine({ price: lo, color, lineStyle: 'dotted', axisLabelVisible: false });
}
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0.05 } }, pane);
legend(pane, zones.map(([lo, name, color]) => [lo === -Infinity ? name + ', below 0' : name + ', from ' + lo, color]));
chart.fitContent();`,
      about: 'NUPL, computed from MVRV as 1 − 1/MVRV, as a histogram colored by cycle zone, with dotted price lines at the zone edges and a legend drawn by a layer plugin. Price sits above on a log scale.',
      api: ["addSeries('histogram')", 'per-point color', 'createPriceLine'],
    },
    {
      id: 'realizedcap',
      title: 'Market cap vs realised cap',
      group: 'Valuation',
      code: `${create()}
${bars(D)}
${load('realized', 'metrics/realizedPrice', D)}
${load('supply', 'series/coinSupply', D)}
${LEGEND}
// Both caps in $B: price and realised price times the circulating supply.
const cap = bars.close.map((p, i) => (p * supply.value[i]) / 1e9);
const realizedCap = realized.value.map((p, i) => (p * supply.value[i]) / 1e9);
const market = chart.addSeries('line', { color: '#d9dee8', lineWidth: 1.5, pricePrecision: 0 });
market.setData({ time: bars.time, value: cap });
const cost = chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 0 });
cost.setData({ time: realized.time, value: realizedCap });
chart.addBand(market, cost, { fill: 'rgba(46, 230, 182, 0.15)', fillCrossed: 'rgba(255, 93, 115, 0.25)' });
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
legend(0, [['Market cap, $B', '#d9dee8'], ['Realised cap, $B', '#ffb547']]);
chart.fitContent();`,
      about: 'Market cap against realised cap (every coin valued at the price it last moved) on a log scale. `addBand` fills green while the market is above cost and red when it falls below.',
      api: ['addBand', 'fillCrossed', "mode: 'logarithmic'"],
    },
    {
      id: 'lthsupply',
      title: 'Long- and short-term holders',
      group: 'Holders',
      code: `${create()}
${bars(D)}
${load('holders', 'series/holderSupply', D)}
${TITLE}
${PRICE}
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
const millions = (v) => v / 1e6;
const longPane = chart.addPane(0.6);
chart.addSeries('area', { color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.3)', bottomFill: 'rgba(76, 201, 240, 0.02)', pricePrecision: 2 }, longPane)
  .setData({ time: holders.time, value: holders.longTerm.map(millions) });
title(longPane, 'Long-term holders, M coins');
const shortPane = chart.addPane(0.6);
chart.addSeries('area', { color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.3)', bottomFill: 'rgba(255, 181, 71, 0.02)', pricePrecision: 2 }, shortPane)
  .setData({ time: holders.time, value: holders.shortTerm.map(millions) });
title(shortPane, 'Short-term holders, M coins');
chart.fitContent();`,
      about: 'Supply held by long-term holders (coins unmoved for 155+ days) and by short-term holders, each in its own pane: long-term holders sell into rallies, which hands coins to the short-term cohort.',
      api: ["addSeries('area')", 'addPane'],
    },
    {
      id: 'cdd',
      title: 'Coin-days destroyed · dormant supply',
      group: 'Holders',
      code: `${create()}
${bars(btc('1d', 730))}
${load('days', 'series/coinDays', btc('1d', 730))}
${TITLE}
${PRICE}
const cddPane = chart.addPane(0.6);
const destroyed = days.destroyed.map((v) => v / 1e6);
const spike = [...destroyed].sort((a, b) => a - b)[Math.floor(destroyed.length * 0.95)];
chart.addSeries('histogram', { color: '#5a6478', pricePrecision: 1 }, cddPane).setData({
  time: days.time, value: destroyed, color: destroyed.map((v) => (v > spike ? '#ffb547' : null)),
});
title(cddPane, 'Coin-days destroyed, M (top 5% in amber)');
const dormantPane = chart.addPane(0.5);
chart.addSeries('line', { color: '#b388ff', lineWidth: 2, pricePrecision: 2 }, dormantPane)
  .setData({ time: days.time, value: days.dormant.map((v) => v / 1e6) });
title(dormantPane, 'Supply dormant 1y+, M coins');
chart.fitContent();`,
      about: 'Coin-days destroyed per day, with the top 5% of days highlighted by per-point color: old coins waking up. Below, the supply that has not moved for a year.',
      api: ["addSeries('histogram')", 'per-point color', 'addPane'],
    },
    {
      id: 'balbands',
      title: 'Supply by balance tier',
      group: 'Holders',
      code: `${create()}
${load('tiers', 'series/balanceTiers', btc('1w', 260))}
${LEGEND}
const groups = [
  ['c1kplus', '1k+ coins', '#7b61ff'], ['c100to1k', '100-1k', '#4cc9f0'], ['c10to100', '10-100', '#2ee6b6'],
  ['c1to10', '1-10', '#ffb547'], ['lt1', 'under 1 coin', '#ff5d73'],
];
for (const [col, , color] of groups) {
  chart.addSeries('line', { color, lineWidth: 2, pricePrecision: 1 }).setData({ time: tiers.time, value: tiers[col].map((v) => v * 100) });
}
chart.applyPriceScaleOptions({ margins: { top: 0.28, bottom: 0.05 } });
legend(0, groups.map(([, name, color]) => [name + ' (% of supply)', color]));
chart.fitContent();`,
      about: 'Five years of weekly data: the share of supply held by addresses in each balance tier, in percent. Large wallets slowly distribute to small ones. The legend is drawn by a layer plugin.',
      api: ["addSeries('line')", 'addLayer'],
    },
    {
      id: 'burnnet',
      title: 'Issuance minus burn',
      group: 'Miners & issuance',
      code: `${create()}
${bars(eth('1d', 365))}
${load('supply', 'series/ethIssuance', eth('1d', 365))}
${TITLE}
chart.addCandlestickSeries().setData(bars);
const net = supply.issued.map((v, i) => v - supply.burned[i]);
const netPane = chart.addPane(0.6);
chart.addSeries('histogram', { pricePrecision: 0 }, netPane).setData({
  time: supply.time, value: net, color: net.map((v) => (v >= 0 ? '#ffb547' : '#b388ff')),
});
title(netPane, 'Net new coins per day: inflation amber, deflation violet');
let total = 0;
const totalPane = chart.addPane(0.6);
chart.addSeries('area', { color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.25)', bottomFill: 'rgba(76, 201, 240, 0.25)', baseValue: 0, pricePrecision: 0 }, totalPane)
  .setData({ time: supply.time, value: net.map((v) => (total += v)) });
title(totalPane, 'Supply change since the start, coins');
chart.fitContent();`,
      about: 'New coins issued minus fees burned, per day, colored by sign; when activity spikes, the burn outruns issuance. The running total below is the net change in supply.',
      api: ["addSeries('histogram')", 'per-point color', "addSeries('area')"],
    },
    {
      id: 'chainfees',
      title: 'Fee revenue by chain',
      group: 'Network & flows',
      code: `${create()}
${load('fees', 'series/chainFees', btc('1w', 104))}
${LEGEND}
const chains = [['chainA', 'Chain A', '#4cc9f0'], ['chainB', 'Chain B', '#b388ff'], ['chainC', 'Chain C', '#2ee6b6'], ['chainD', 'Chain D', '#ffb547'], ['chainE', 'Chain E', '#ff5d73']];
for (const [col, , color] of chains) {
  chart.addSeries('line', { color, lineWidth: 2, pricePrecision: 2 }).setData({ time: fees.time, value: fees[col].map((v) => v / 1e6) });
}
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
legend(0, chains.map(([, name, color]) => [name + ', $M per week', color]));
chart.fitContent();`,
      about: 'Two years of weekly fee revenue on five chains, in $M on a log scale so the small chains stay readable next to the large ones. The legend is drawn by a layer plugin.',
      api: ["addSeries('line')", "mode: 'logarithmic'"],
    },
    {
      id: 'supplyprofit',
      title: 'Supply in profit',
      group: 'Valuation',
      code: `${create()}
${bars(btc('1d', 2200))}
// A simple cost-basis model: each day's volume lands at that day's price and older coins
// decay a little as they move again. Supply in profit is the part below today's price.
const step = 500;
const dist = new Map();
const profit = bars.close.map((close, i) => {
  for (const [row, v] of dist) dist.set(row, v * 0.997);
  const row = Math.round(close / step) * step;
  dist.set(row, (dist.get(row) ?? 0) + bars.volume[i]);
  let below = 0, total = 0;
  for (const [p, v] of dist) {
    total += v;
    if (p <= close) below += v;
  }
  return (below / total) * 100;
});
${PRICE}
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
const pane = chart.addPane(0.8);
const share = chart.addSeries('area', {
  color: '#2ee6b6', topFill: 'rgba(46, 230, 182, 0.35)', bottomFill: 'rgba(46, 230, 182, 0.03)', pricePrecision: 0,
}, pane);
share.setData({ time: bars.time, value: profit });
chart.applyPriceScaleOptions({ range: { low: 0, high: 104 } }, pane);
share.createPriceLine({ price: 95, color: '#ff5d73', lineStyle: 'dashed' });
share.createPriceLine({ price: 50, color: '#ffb547', lineStyle: 'dashed' });
chart.setVisibleLogicalRange(700, 2200); // after a warm-up`,
      about: 'The share of supply that last moved below today\'s price, from a small cost-basis model built from daily volume in the example, on a fixed percent scale (`range` turns auto-scaling off) with price lines at 50% and 95%.',
      api: ["applyPriceScaleOptions({ range })", 'createPriceLine', "addSeries('area')"],
    },
    {
      id: 'minerflow',
      title: 'Miner balance · outflows',
      group: 'Miners & issuance',
      code: `${create()}
${bars(btc('1d', 730))}
${load('miners', 'series/minerFlows', btc('1d', 730))}
${TITLE}
${PRICE}
const balancePane = chart.addPane(0.55);
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 3 }, balancePane)
  .setData({ time: miners.time, value: miners.balance.map((v) => v / 1e6) });
title(balancePane, 'Miner balance, M coins');
const outPane = chart.addPane(0.55);
chart.addSeries('histogram', { color: '#ff5d73', pricePrecision: 0 }, outPane).setData({ time: miners.time, value: miners.outflow });
title(outPane, 'Sent out by miners, coins per day');
chart.fitContent();`,
      about: 'Coins held by miner addresses and what they send out each day: outflows jump on big moves as miners sell into strength or cover costs in drawdowns.',
      api: ["addSeries('line')", "addSeries('histogram')", 'addPane'],
    },
    {
      id: 'hashribbon',
      title: 'Hash ribbons',
      group: 'Miners & issuance',
      code: `${create()}
${bars(btc('1d', 730))}
${load('hash', 'series/hashRate', btc('1d', 730))}
const sma = (v, n) => v.map((_, i) => (i < n - 1 ? NaN : v.slice(i - n + 1, i + 1).reduce((a, b) => a + b, 0) / n));
const fast = sma(hash.value, 30), slow = sma(hash.value, 60);
// Recovery: the 30-day average crosses back above the 60-day one.
const signals = [];
for (let i = 61; i < fast.length; i++) {
  if (fast[i] > slow[i] && fast[i - 1] <= slow[i - 1]) signals.push(hash.time[i]);
}
const price = chart.addCandlestickSeries();
price.setData(bars);
price.setMarkers(signals.map((time) => ({ time, position: 'belowBar', shape: 'arrowUp', color: '#2ee6b6', text: 'Buy' })));
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
const pane = chart.addPane(0.7);
chart.addSeries('line', { color: 'rgba(138, 147, 166, 0.45)', lineWidth: 1, lastValueVisible: false, priceLineVisible: false }, pane)
  .setData({ time: hash.time, value: hash.value });
const f = chart.addSeries('line', { color: '#2ee6b6', lineWidth: 1.5, pricePrecision: 0 }, pane);
f.setData({ time: hash.time, value: fast });
const s = chart.addSeries('line', { color: '#ff5d73', lineWidth: 1.5, pricePrecision: 0 }, pane);
s.setData({ time: hash.time, value: slow });
chart.addBand(f, s, { fill: 'rgba(46, 230, 182, 0.2)', fillCrossed: 'rgba(255, 93, 115, 0.3)' });
chart.setBarSpacing(1.4);
chart.scrollToRealtime();`,
      about: 'Hash rate (EH/s) with its 30- and 60-day averages; `addBand` fills red while the fast one is below (miner capitulation) and markers flag the recoveries on the price.',
      api: ['addBand', 'fillCrossed', 'Series.setMarkers'],
    },
    {
      id: 'puell',
      title: 'Puell multiple',
      group: 'Miners & issuance',
      code: `${create()}
${bars(btc('1d', 1865))}
${load('revenue', 'series/minerRevenue', btc('1d', 1865))}
// Daily miner revenue over its 365-day average; the first year only warms it up.
const puell = revenue.value.map((v, i, all) => {
  if (i < 365) return NaN;
  return v / (all.slice(i - 365, i).reduce((a, b) => a + b, 0) / 365);
});
${PRICE}
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
const pane = chart.addPane(0.7);
const multiple = chart.addSeries('line', { color: '#ffb547', lineWidth: 1.5, pricePrecision: 2 }, pane);
multiple.setData({ time: revenue.time, value: puell });
multiple.createPriceLine({ price: 0.6, color: '#2ee6b6', lineStyle: 'dashed', title: 'cheap' });
multiple.createPriceLine({ price: 1.6, color: '#ff5d73', lineStyle: 'dashed', title: 'hot' });
chart.setVisibleLogicalRange(365, 1865);`,
      about: 'Miner revenue against its own one-year average: low readings mark miner stress, high ones overheated markets. The rolling mean is computed in a few lines; the view starts after the warm-up year.',
      api: ['createPriceLine', 'setVisibleLogicalRange'],
    },
    {
      id: 'nvt',
      title: 'NVT signal',
      group: 'Network & flows',
      code: `${create()}
${bars(D)}
${load('supply', 'series/coinSupply', D)}
${load('volume', 'series/onchainVolume', D)}
// Market cap over the 90-day average of value moved on chain.
const nvt = bars.close.map((p, i) => {
  if (i < 89) return NaN;
  const avg = volume.value.slice(i - 89, i + 1).reduce((a, b) => a + b, 0) / 90;
  return (p * supply.value[i]) / avg;
});
${PRICE}
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
chart.addSeries('baseline', {
  baseValue: 70, color: '#ff5d73', downColor: '#2ee6b6', pricePrecision: 0,
  topFill: 'rgba(255, 93, 115, 0.3)', bottomFill: 'rgba(46, 230, 182, 0.06)',
}, chart.addPane(0.7)).setData({ time: bars.time, value: nvt });
chart.setVisibleLogicalRange(90, 1500);`,
      about: 'The network\'s price-to-earnings: market cap over the 90-day average of value moved on chain, as a `baseline` around 70. High readings mean the price runs ahead of usage.',
      api: ["addSeries('baseline')", 'baseValue'],
    },
    {
      id: 'stakeyield',
      title: 'Staking ratio · yield',
      group: 'Network & flows',
      code: `${create()}
${bars(eth('1d', 1000))}
${load('staking', 'series/ethStaking', eth('1d', 1000))}
${TITLE}
${PRICE}
const ratioPane = chart.addPane(0.6);
chart.addSeries('area', { color: '#b388ff', topFill: 'rgba(179, 136, 255, 0.35)', bottomFill: 'rgba(179, 136, 255, 0.03)', pricePrecision: 1 }, ratioPane)
  .setData({ time: staking.time, value: staking.ratio.map((v) => v * 100) });
title(ratioPane, 'Supply staked, %');
const yieldPane = chart.addPane(0.5);
chart.addSeries('line', { color: '#2ee6b6', lineWidth: 1.5, pricePrecision: 2 }, yieldPane)
  .setData({ time: staking.time, value: staking.yield.map((v) => v * 100) });
title(yieldPane, 'Staking yield, % a year');
chart.fitContent();`,
      about: 'The share of supply staked keeps climbing while the annual staking yield drifts: two panes in percent under the price.',
      api: ["addSeries('area')", 'addPane'],
    },
    {
      id: 'rainbow',
      title: 'Log regression rainbow',
      group: 'Valuation',
      code: `${create()}
${bars(btc('1d', 3000))}
// Fit log(price) = a + b·log(days since the first block) and draw bands around the fit,
// a year past the data.
const genesis = Date.UTC(2009, 0, 3), day = 86_400_000;
const x = bars.time.map((t) => Math.log10((t - genesis) / day));
const y = bars.close.map((p) => Math.log10(p));
const n = x.length, mx = x.reduce((a, b) => a + b) / n, my = y.reduce((a, b) => a + b) / n;
const b = x.reduce((s, xi, i) => s + (xi - mx) * (y[i] - my), 0) / x.reduce((s, xi) => s + (xi - mx) ** 2, 0);
const a = my - b * mx;
const sd = Math.sqrt(y.reduce((s, yi, i) => s + (yi - a - b * x[i]) ** 2, 0) / n);
const time = [...bars.time];
for (let i = 1; i <= 365; i++) time.push(bars.time[n - 1] + i * day);
const colors = ['#7b61ff', '#4c7cf0', '#4cc9f0', '#2ee6b6', '#c4e86b', '#ffe08a', '#ffb547', '#ff8a5c', '#ff5d73'];
const lines = [];
for (let k = 0; k <= colors.length; k++) {
  const offset = (k / colors.length - 0.5) * 4.4 * sd;
  const line = chart.addSeries('line', { color: 'rgba(11, 13, 18, 0.5)', lineWidth: 1, lastValueVisible: false, priceLineVisible: false });
  line.setData({ time, value: time.map((t) => 10 ** (a + offset + b * Math.log10((t - genesis) / day))) });
  lines.push(line);
}
colors.forEach((c, k) => chart.addBand(lines[k + 1], lines[k], { fill: c + '55' }));
chart.addSeries('line', { color: '#ffffff', lineWidth: 1.5 }).setData({ time: bars.time, value: bars.close });
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
chart.fitContent();`,
      about: 'A power-law fit of price against time since the first block, with nine bands of ±2.2 standard deviations filled by `addBand` on a log scale. The bands run a year past the last bar: future times add slots to the axis.',
      api: ['addBand', "mode: 'logarithmic'", 'future data'],
    },
    {
      id: 'exflow',
      title: 'Exchange and bridge net flows',
      group: 'Network & flows',
      code: `${create()}
${bars(btc('1d', 365))}
${load('exchange', 'metrics/exchangeNetflow', btc('1d', 365))}
${load('bridges', 'series/bridgeFlows', btc('1d', 365))}
${TITLE}
chart.addCandlestickSeries().setData(bars);
const signColor = (up, down) => (v) => (v >= 0 ? up : down);
const exPane = chart.addPane(0.6);
chart.addSeries('histogram', { pricePrecision: 0 }, exPane).setData({
  time: exchange.time, value: exchange.value, color: exchange.value.map(signColor('#ff5d73', '#2ee6b6')),
});
title(exPane, 'Exchange net flow, coins: in red, out green');
const bridgePane = chart.addPane(0.6);
const usd = bridges.chainA.map((v) => v / 1e6);
chart.addSeries('histogram', { pricePrecision: 1 }, bridgePane).setData({
  time: bridges.time, value: usd, color: usd.map(signColor('#4cc9f0', '#ffb547')),
});
title(bridgePane, 'Bridged into Chain A, $M: in blue, out amber');
chart.fitContent();`,
      about: 'Coins moved onto exchanges (red, potential selling) or off them (green), and the net value bridged into a chain, each a histogram with per-point colors in its own pane.',
      api: ["addSeries('histogram')", 'per-point color', 'addPane'],
    },
    {
      id: 'activity',
      title: 'Active addresses · transfer sizes',
      group: 'Network & flows',
      code: `${create()}
${bars(btc('1w', 156))}
${load('active', 'metrics/activeAddresses', btc('1w', 156))}
${load('sizes', 'series/transferSizes', btc('1w', 156))}
${TITLE}
${LEGEND}
chart.addCandlestickSeries().setData(bars);
const activePane = chart.addPane(0.55);
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.5, pricePrecision: 1 }, activePane)
  .setData({ time: active.time, value: active.value.map((v) => v / 1e6) });
title(activePane, 'Active addresses per week, millions');
// Stack the buckets: draw the total first, then the lower layers over it.
const tiers = [['small', 'under $1k', '#3f5f8a'], ['medium', '$1k-100k', '#b388ff'], ['large', 'over $100k', '#ffb547']];
const sizePane = chart.addPane(0.8);
for (let k = tiers.length - 1; k >= 0; k--) {
  const value = sizes.time.map((_, i) => tiers.slice(0, k + 1).reduce((s, [col]) => s + sizes[col][i], 0) / 1e6);
  chart.addSeries('histogram', { color: tiers[k][2], pricePrecision: 1, lastValueVisible: false, priceLineVisible: false }, sizePane)
    .setData({ time: sizes.time, value });
}
chart.applyPriceScaleOptions({ margins: { top: 0.35, bottom: 0 } }, sizePane);
legend(sizePane, tiers.map(([, name, color]) => [name + ', million transfers', color]).reverse());
chart.fitContent();`,
      about: 'Three years of weekly data: active addresses, and the number of transfers stacked by size: three histograms drawn tallest first so each layer shows its own bucket.',
      api: ["addSeries('histogram')", "addSeries('line')", 'addPane'],
    },
    {
      id: 'costbasis',
      title: 'Cost basis distribution',
      group: 'Holders',
      code: `${create()}
${bars(btc('1d', 1500))}
// Where the coins last moved: each day's volume lands at that day's price, and older
// coins decay a little every day as they move again.
const step = 1000;
const dist = new Map();
bars.time.forEach((_, i) => {
  for (const [row, v] of dist) dist.set(row, v * 0.997);
  const row = Math.round(bars.close[i] / step) * step;
  dist.set(row, (dist.get(row) ?? 0) + bars.volume[i]);
});
const rows = [...dist].filter(([, v]) => v > 0);
const peak = Math.max(...rows.map(([, v]) => v));
const last = bars.close[bars.close.length - 1];
chart.addCandlestickSeries().setData(bars);
chart.addLayer({
  pane: 0,
  position: 'belowSeries',
  build(ctx) {
    const right = ctx.rect.x + ctx.rect.width, width = ctx.rect.width * 0.24;
    const out = [];
    for (const [row, v] of rows) {
      const y0 = ctx.priceToY(row + step / 2), y1 = ctx.priceToY(row - step / 2);
      if (y0 === null || y1 === null) continue;
      const w = (v / peak) * width;
      out.push({ type: 'rect', x: right - w, y: y0 + 0.5, width: w, height: Math.max(1, y1 - y0 - 1), color: row < last ? 'rgba(46, 230, 182, 0.45)' : 'rgba(255, 93, 115, 0.45)' });
    }
    out.push({ type: 'text', x: right - 8, y: ctx.rect.y + 8, text: 'Supply by cost: in profit / at a loss', color: '#8a93a6', size: 11, align: 'right' });
    return out;
  },
});
chart.setBarSpacing(1.6);
chart.scrollToRealtime();
chart.setRightOffset(150); // room for the profile`,
      about: 'A layer plugin draws, to the right of the candles, how much supply last moved at each price ($1,000 rows), from a simple model built from daily volume: green below the last price (in profit), red above.',
      api: ['addLayer', "{ type: 'rect' }", 'setRightOffset'],
    },
  ],
};
