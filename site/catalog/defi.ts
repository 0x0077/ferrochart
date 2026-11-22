import { bars, create, load } from './build';
import { plotDemos } from './plots';
import type { Category } from './types';

const DAY = { symbol: 'ETHUSD', interval: '1d', count: 365 };
const HOUR = { symbol: 'ETHUSD', interval: '1h', count: 720 };

export const defi: Category = {
  id: 'defi',
  title: 'DeFi & AMMs',
  kicker: '08',
  blurb: 'Pools, liquidity ranges, lending, stablecoins and staking over time.',
  accent: '#7ee787',
  demos: [
    // ----------------------------------------------------------- Liquidity
    {
      id: 'lprange',
      title: 'LP range on pool candles',
      group: 'Liquidity',
      code: `${create()}
${bars(HOUR)}
const candles = chart.addCandlestickSeries();
candles.setData(bars);

// A concentrated-liquidity position opened 12 days ago, ±7% around the entry price.
const entry = bars.time.length - 288;
const lower = bars.close[entry] * 0.93, upper = bars.close[entry] * 1.07;
const edge = (price) => bars.time.map((t, i) => (i < entry ? NaN : price));
const style = { color: '#7ee787', lineWidth: 1, lineStyle: 'dashed', priceLineVisible: false };
const hi = chart.addSeries('line', style);
const lo = chart.addSeries('line', style);
hi.setData({ time: bars.time, value: edge(upper) });
lo.setData({ time: bars.time, value: edge(lower) });
chart.addBand(hi, lo, { fill: 'rgba(126, 231, 135, 0.08)' });

// Markers where the price leaves or re-enters the range (and stays for 6 hours).
const markers = [{ time: bars.time[entry], position: 'belowBar', shape: 'arrowUp', color: '#7ee787', text: 'Add liquidity' }];
let inRange = true, streak = 0;
for (let i = entry + 1; i < bars.time.length; i++) {
  const inside = bars.close[i] >= lower && bars.close[i] <= upper;
  streak = inside === inRange ? 0 : streak + 1;
  if (streak < 6) continue;
  inRange = inside;
  streak = 0;
  const t = bars.time[i - 5];
  markers.push(inside
    ? { time: t, position: 'belowBar', shape: 'circle', color: '#2ee6b6', text: 'In range' }
    : { time: t, position: 'aboveBar', shape: 'circle', color: '#ffb547', text: 'Out of range' });
}
candles.setMarkers(markers);
chart.setVisibleLogicalRange(entry - 40, bars.time.length + 4);`,
      about: 'One-hour pool candles with a liquidity position: two dashed lines from the entry bar with `addBand` between them mark the range, and markers show where the price left it and came back.',
      api: ['addBand', 'setMarkers', 'NaN gaps'],
    },
    {
      id: 'lanes',
      title: 'LP positions',
      group: 'Liquidity',
      code: `${create()}
${bars(HOUR)}
chart.addCandlestickSeries().setData(bars);

// Positions: opened at bar \`from\`, closed at \`to\` (null: still open), range around the entry price.
const n = bars.time.length;
const colors = ['#4cc9f0', '#b388ff', '#ffb547', '#7ee787', '#ff5d73', '#4cc9f0'];
const positions = [
  { from: 30, to: 260, lo: 0.9, hi: 1.08 },
  { from: 120, to: 330, lo: 0.97, hi: 1.03 },
  { from: 280, to: 520, lo: 0.92, hi: 1.05 },
  { from: 360, to: 470, lo: 0.98, hi: 1.02 },
  { from: 500, to: null, lo: 0.94, hi: 1.06 },
  { from: 610, to: null, lo: 0.985, hi: 1.015 },
].map((p, k) => ({
  start: bars.time[p.from],
  end: bars.time[p.to ?? n - 1],
  lo: bars.close[p.from] * p.lo,
  hi: bars.close[p.from] * p.hi,
  color: colors[k],
  label: \`LP \${k + 1} · \${Math.round((p.hi - p.lo) * 100)}% wide\`,
}));

chart.addLayer({
  pane: chart.mainPane,
  position: 'belowSeries',
  build(ctx) {
    const out = [];
    const right = ctx.rect.x + ctx.rect.width;
    for (const p of positions) {
      const x1 = ctx.timeToX(p.start), x2 = ctx.timeToX(p.end);
      const y1 = ctx.priceToY(p.hi), y2 = ctx.priceToY(p.lo);
      if (x1 === null || x2 === null || y1 === null || y2 === null) continue;
      if (x2 < ctx.rect.x || x1 > right) continue;
      out.push({ type: 'rect', x: x1, y: y1, width: x2 - x1, height: y2 - y1, color: p.color + '22' });
      out.push({ type: 'polyline', points: [[x1, y1], [x2, y1], [x2, y2], [x1, y2], [x1, y1]], color: p.color + 'aa', width: 1 });
      out.push({ type: 'text', x: Math.max(x1, ctx.rect.x) + 4, y: y1 + 4, text: p.label, color: p.color, size: 11 });
    }
    return out;
  },
});
chart.setBarSpacing(1.4);
chart.scrollToRealtime();`,
      about: 'Six liquidity positions of one pool over a month of one-hour candles. A layer plugin draws each one as a rectangle from its open to its close time, between its lower and upper price.',
      api: ['addLayer', 'ctx.timeToX', 'ctx.priceToY'],
    },
    {
      id: 'ammheat',
      title: 'AMM liquidity history',
      group: 'Liquidity',
      code: `${create()}
${bars(HOUR)}
${load('pool', 'series/defiPoolDepth', HOUR)}
// Liquidity per $10 price level, each hour: most LPs sit in a narrow band around a
// slowly re-centred price, the rest in wide ranges.
const tick = 10;
const heatmap = chart.addHeatmap({
  source: 'depth', tickSize: tick, columnMs: 3_600_000, scale: 'linear',
  colors: ['rgba(18, 57, 90, 0)', '#12395a', '#1f7a8c', '#7ee787', '#f4f9a8'],
});
const share = (d, width) => Math.exp(-0.5 * (d / width) ** 2) / width;
let center = bars.close[0];
bars.time.forEach((time, i) => {
  center += (bars.close[i] - center) / 36;
  const prices = [], sizes = [];
  for (let k = -90; k <= 90; k++) {
    const price = Math.round(center / tick) * tick + k * tick;
    const d = price / center - 1;
    prices.push(price);
    sizes.push(pool.tvl[i] * (tick / center) * (0.65 * share(d, 0.025) + 0.35 * share(d, 0.1)) / 2.5);
  }
  heatmap.appendDepth({ time, prices, sizes });
});
chart.addCandlestickSeries().setData(bars);
chart.setBarSpacing(2.5);
chart.scrollToRealtime();`,
      about: 'Where the liquidity of a pool sat over a month: a depth heatmap with one column per hour and one row per $10 level, under the candles. The shape is modelled in the example from the pool’s value locked: a tight band that follows the price slowly plus wide ranges.',
      api: ['addHeatmap', 'Series.appendDepth'],
    },
    {
      id: 'lpreplay',
      title: 'LP position replay',
      group: 'Liquidity',
      code: `${create()}
${bars(HOUR)}
const candles = chart.addCandlestickSeries();
candles.setData(bars);

// Replay a strategy: a ±5% range, moved to the price after 8 hours out of range.
const width = 0.05, patience = 8;
const ranges = [];
let cur = { from: 0, lo: bars.close[0] * (1 - width), hi: bars.close[0] * (1 + width) };
let out = 0;
for (let i = 1; i < bars.time.length; i++) {
  const c = bars.close[i];
  out = c < cur.lo || c > cur.hi ? out + 1 : 0;
  if (out < patience) continue;
  ranges.push({ ...cur, to: i });
  cur = { from: i, lo: c * (1 - width), hi: c * (1 + width) };
  out = 0;
}
ranges.push({ ...cur, to: bars.time.length - 1 });
candles.setMarkers(ranges.slice(1).map((r) => ({
  time: bars.time[r.from], position: 'belowBar', shape: 'arrowUp', color: '#7ee787',
})));

chart.addLayer({
  pane: chart.mainPane,
  position: 'belowSeries',
  build(ctx) {
    const cmds = [];
    for (const r of ranges) {
      const x1 = ctx.timeToX(bars.time[r.from]), x2 = ctx.timeToX(bars.time[r.to]);
      const y1 = ctx.priceToY(r.hi), y2 = ctx.priceToY(r.lo);
      if (x1 === null || x2 === null || y1 === null || y2 === null) continue;
      if (x2 < ctx.rect.x || x1 > ctx.rect.x + ctx.rect.width) continue;
      cmds.push({ type: 'rect', x: x1, y: y1, width: x2 - x1, height: y2 - y1, color: 'rgba(126, 231, 135, 0.13)' });
      cmds.push({ type: 'line', x1, y1, x2, y2: y1, color: '#7ee787', width: 1 });
      cmds.push({ type: 'line', x1, y1: y2, x2, y2, color: '#7ee787', width: 1 });
    }
    return cmds;
  },
});
chart.setBarSpacing(1.4);
chart.scrollToRealtime();`,
      about: 'A month of a rebalancing LP strategy replayed on one-hour candles: the range is ±5% and moves to the price after eight hours outside it. A layer plugin draws each range as a rectangle; arrows mark the rebalances.',
      api: ['addLayer', 'setMarkers'],
    },
    // ----------------------------------------------------------- Pools & yield
    {
      id: 'poolstats',
      title: 'Pool TVL · volume · fees',
      group: 'Pools & yield',
      code: `${create()}
${load('pool', 'series/defiPool', DAY)}
const fee = 0.0005; // 0.05% fee tier
chart.addSeries('area', { color: '#7ee787', topFill: 'rgba(126, 231, 135, 0.3)', bottomFill: 'rgba(126, 231, 135, 0.02)' })
  .setData({ time: pool.time, value: pool.tvl.map((v) => v / 1e6) }); // USD m
chart.addSeries('histogram', { color: '#4cc9f0' }, chart.addPane(0.6))
  .setData({ time: pool.time, value: pool.volume.map((v) => v / 1e6) });
// Fee APR: the last 7 days' fees, annualised, over the value locked.
const apr = pool.volume.map((v, i) => {
  const week = pool.volume.slice(Math.max(0, i - 6), i + 1);
  const daily = week.reduce((a, b) => a + b, 0) / week.length;
  return (daily * fee * 365 / pool.tvl[i]) * 100;
});
chart.addSeries('line', { color: '#ffb547', lineWidth: 2 }, chart.addPane(0.6))
  .setData({ time: pool.time, value: apr });
chart.setBarSpacing(2.8);
chart.scrollToRealtime();`,
      about: 'A year of one pool: value locked in USD millions, daily swap volume, and the fee APR, a day’s fees annualised over the value locked. Three panes share one time axis and crosshair.',
      api: ['addPane', "addSeries('area')", "addSeries('histogram')"],
    },
    {
      id: 'apybreak',
      title: 'Yield by source',
      group: 'Pools & yield',
      code: `${create()}
${load('apy', 'series/defiApy', DAY)}
// Stacked areas: the total (fees + rewards) first, then fees in front of it.
const total = apy.fees.map((v, i) => v + apy.rewards[i]);
chart.addSeries('area', { color: '#b388ff', topFill: '#3d3262', bottomFill: '#3d3262' })
  .setData({ time: apy.time, value: total });
chart.addSeries('area', { color: '#4cc9f0', topFill: '#1d4b5c', bottomFill: '#1d4b5c', lastValueVisible: false })
  .setData({ time: apy.time, value: apy.fees });
chart.applyPriceScaleOptions({ margins: { top: 0.1, bottom: 0 } });
chart.setBarSpacing(2.8);
chart.scrollToRealtime();`,
      about: 'A pool’s yield in percent a year, split by source: trading fees (blue) and token rewards (violet) stacked on top. The rewards taper off over the year. The stack is two `area` series, the total behind the fees.',
      api: ["addSeries('area')", 'stacking'],
    },
    {
      id: 'tvlbreak',
      title: 'TVL change: price vs flows',
      group: 'Pools & yield',
      code: `${create()}
${load('src', 'series/defiTvlSource', { ...DAY, count: 240 })}
const up = '#2ee6b6', down = '#ff5d73';
const time = src.time;
// Deposits in ETH: a starting balance plus net flows. TVL = tokens × price, and each
// day's change splits exactly into a price effect (yesterday's tokens × price change)
// and net flows (today's deposits × today's price).
let held = 2.2e6;
const tokens = src.netFlow.map((q) => (held += q));
const tvl = tokens.map((q, i) => q * src.price[i] / 1e9); // USD bn
const priceEffect = time.map((t, i) => (i ? tokens[i - 1] * (src.price[i] - src.price[i - 1]) / 1e6 : 0)); // USD m
const flows = src.netFlow.map((q, i) => q * src.price[i] / 1e6);
const signed = (v) => (v >= 0 ? up : down);
chart.addSeries('area', { color: '#7ee787', topFill: 'rgba(126, 231, 135, 0.25)', bottomFill: 'rgba(126, 231, 135, 0)' })
  .setData({ time, value: tvl });
chart.addSeries('histogram', { color: '#4cc9f0' }, chart.addPane(0.6))
  .setData({ time, value: priceEffect, color: priceEffect.map(signed) });
chart.addSeries('histogram', { color: '#4cc9f0' }, chart.addPane(0.6))
  .setData({ time, value: flows, color: flows.map(signed) });
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Value locked in USD billions and where each day’s change came from: the price effect on the tokens already deposited (middle) and net deposits or withdrawals at today’s price (bottom), in USD millions. The two add up to the change exactly.',
      api: ["addSeries('histogram')", 'per-point color', 'addPane'],
    },
    {
      id: 'revincentive',
      title: 'Revenue vs incentives',
      group: 'Pools & yield',
      code: `${create()}
${load('rev', 'series/defiRevenue', { symbol: 'ETHUSD', interval: '1w', count: 104 })}
const m = (v) => v / 1e6; // USD m per week
chart.addSeries('histogram', { color: '#2ee6b6' }).setData({ time: rev.time, value: rev.revenue.map(m) });
chart.addSeries('histogram', { color: '#ff5d73' }).setData({ time: rev.time, value: rev.incentives.map((v) => -m(v)) });
// Net: revenue kept after paying incentives.
chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', lineWidth: 2 }, chart.addPane(0.7))
  .setData({ time: rev.time, value: rev.revenue.map((v, i) => m(v - rev.incentives[i])) });
chart.setBarSpacing(9);
chart.scrollToRealtime();`,
      about: 'Two years of a protocol’s weekly revenue (up) against the token incentives it paid out (down), in USD millions, and the net as a `baseline` series around zero: the protocol turns profitable as the incentives taper.',
      api: ["addSeries('histogram')", "addSeries('baseline')"],
    },
    {
      id: 'lvrdecomp',
      title: 'LP P&L: fees vs LVR',
      group: 'Pools & yield',
      code: `${bars({ ...HOUR, count: 1440 })}
${load('lp', 'series/defiLpFees', { ...HOUR, count: 1440 })}
// Loss-versus-rebalancing of a $10M full-range position: value × r² / 8 per bar.
const value = 10e6;
let fees = 0, lvr = 0;
const cumFees = [], cumLvr = [], net = [];
bars.time.forEach((t, i) => {
  const r = i ? Math.log(bars.close[i] / bars.close[i - 1]) : 0;
  fees += lp.fees[i] / 1e3;
  lvr += (value * r * r) / 8 / 1e3;
  cumFees.push(fees);
  cumLvr.push(-lvr);
  net.push(fees - lvr);
});
${create()}
chart.addSeries('line', { color: '#2ee6b6', lineWidth: 2 }).setData({ time: bars.time, value: cumFees });
chart.addSeries('line', { color: '#ff5d73', lineWidth: 2 }).setData({ time: bars.time, value: cumLvr });
chart.addSeries('baseline', { baseValue: 0, color: '#ffb547', downColor: '#ff5d73', lineWidth: 2 })
  .setData({ time: bars.time, value: net });
chart.setVisibleLogicalRange(0, bars.time.length + 8);`,
      about: 'Two months of an LP position’s cumulative P&L in USD thousands: fees earned (green), loss-versus-rebalancing computed from hourly returns (red, negative) and the net as a `baseline` series. Big moves show as steps in the LVR line.',
      api: ["addSeries('baseline')", "addSeries('line')"],
    },
    {
      id: 'rwa',
      title: 'Tokenised treasury fund',
      group: 'Pools & yield',
      code: `${create()}
${load('fund', 'series/defiRwa', DAY)}
chart.addSeries('area', { color: '#7ee787', topFill: 'rgba(126, 231, 135, 0.28)', bottomFill: 'rgba(126, 231, 135, 0.02)' })
  .setData({ time: fund.time, value: fund.aum.map((v) => v / 1e6) }); // USD m
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, step: true }, chart.addPane(0.5))
  .setData({ time: fund.time, value: fund.yield }); // % a year
chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73' }, chart.addPane(0.5))
  .setData({ time: fund.time, value: fund.premium.map((v) => v * 1e4) }); // bp to NAV
chart.setBarSpacing(2.8);
chart.scrollToRealtime();`,
      about: 'A year of a tokenised money-market fund: assets in USD millions, the yield it pays (a step line that follows the policy rate) and the token’s premium or discount to NAV in basis points.',
      api: ['addPane', 'step lines', "addSeries('baseline')"],
    },
    // ----------------------------------------------------------- Lending
    {
      id: 'lendrates',
      title: 'Lending rates',
      group: 'Lending',
      code: `${create()}
${load('market', 'series/defiLending', DAY)}
// Kinked rate model: borrow rate from utilisation; suppliers get it pro rata, less a reserve.
const kink = 0.9, slope1 = 0.06, slope2 = 0.6, reserveFactor = 0.1;
const borrowRate = (u) => (u <= kink ? slope1 * u / kink : slope1 + slope2 * (u - kink) / (1 - kink));
const borrow = market.utilization.map((u) => borrowRate(u) * 100);
const supply = market.utilization.map((u, i) => borrow[i] * u * (1 - reserveFactor));
const b = chart.addSeries('line', { color: '#ffb547', lineWidth: 2 });
const s = chart.addSeries('line', { color: '#2ee6b6', lineWidth: 2 });
b.setData({ time: market.time, value: borrow });
s.setData({ time: market.time, value: supply });
chart.addBand(b, s, { fill: 'rgba(255, 181, 71, 0.12)' });
const pane = chart.addPane(0.6);
const util = chart.addSeries('area', { color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.25)', bottomFill: 'rgba(76, 201, 240, 0)' }, pane);
util.setData({ time: market.time, value: market.utilization.map((u) => u * 100) });
util.createPriceLine({ price: kink * 100, color: '#ff5d73', lineStyle: 'dashed', title: 'Kink' });
chart.setBarSpacing(2.8);
chart.scrollToRealtime();`,
      about: 'Borrow (amber) and supply (green) APR of a stablecoin market in percent, with the spread filled by `addBand`. Both come from utilisation (bottom) through a kinked rate model in the example: rates jump when utilisation passes the 90% kink.',
      api: ['addBand', 'createPriceLine', 'addPane'],
    },
    {
      id: 'baddebt',
      title: 'Bad debt vs reserve',
      group: 'Lending',
      code: `${create()}
${bars({ ...DAY, count: 730 })}
${load('risk', 'series/defiBadDebt', { ...DAY, count: 730 })}
const m = (v) => v / 1e6; // USD m
chart.addSeries('area', { color: '#ff5d73', topFill: 'rgba(255, 93, 115, 0.4)', bottomFill: 'rgba(255, 93, 115, 0.05)' })
  .setData({ time: risk.time, value: risk.badDebt.map(m) });
chart.addSeries('line', { color: '#2ee6b6', lineWidth: 2 })
  .setData({ time: risk.time, value: risk.reserve.map(m) });
chart.addSeries('line', { color: '#4cc9f0' }, chart.addPane(0.45))
  .setData({ time: bars.time, value: bars.close });
chart.setBarSpacing(1.4);
chart.scrollToRealtime();`,
      about: 'Two years of a lending protocol’s bad debt (red area) against the reserve set aside to cover it (green), in USD millions, with the collateral price below: bad debt builds up in sell-offs.',
      api: ["addSeries('area')", 'addPane'],
    },
    {
      id: 'auctionpath',
      title: 'Liquidation auction',
      group: 'Lending',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '15s', count: 200 })}
// A Dutch auction: starts 20% above the market, price cut 1% a minute.
// Keepers buy 40% of the lot at a 3% discount and the rest at 5%.
const kick = 40;
const time = bars.time;
const auction = time.map(() => NaN);
const discount = time.map(() => NaN);
const markers = [{ time: time[kick], position: 'aboveBar', shape: 'arrowDown', color: '#ffb547', text: 'Kick' }];
let left = 1;
for (let i = kick; i < time.length && left > 0; i++) {
  auction[i] = bars.close[kick] * 1.2 * 0.99 ** Math.floor((i - kick) / 4);
  discount[i] = (auction[i] / bars.close[i] - 1) * 100;
  const take = left === 1 && discount[i] <= -3 ? 0.4 : discount[i] <= -5 ? left : 0;
  if (!take) continue;
  left -= take;
  markers.push({ time: time[i], position: 'belowBar', shape: 'arrowUp', color: '#2ee6b6', text: \`Take \${Math.round(take * 100)}%\` });
}
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2 }).setData({ time, value: bars.close });
const lot = chart.addSeries('line', { color: '#ffb547', lineWidth: 2, step: true });
lot.setData({ time, value: auction });
lot.setMarkers(markers);
const pane = chart.addPane(0.6);
chart.addSeries('baseline', { baseValue: 0, color: '#ffb547', downColor: '#2ee6b6' }, pane)
  .setData({ time, value: discount }); // % vs market
chart.setVisibleLogicalRange(kick - 15, kick + 115);`,
      about: 'One collateral auction on 15-second data: the auction price (amber steps) starts above the market (blue) and falls 1% a minute until keepers buy, marked where they take the lot. Below, the auction price against the market in percent.',
      api: ['step lines', 'setMarkers', 'setVisibleLogicalRange'],
    },
    // ----------------------------------------------------------- Stablecoins & staking
    {
      id: 'depeg',
      title: 'Stablecoin depeg',
      group: 'Stablecoins & staking',
      code: `${create()}
${load('coin', 'series/defiStable', { symbol: 'ETHUSD', interval: '4h', count: 720 })}
const peg = chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', lineWidth: 2 });
peg.setData({ time: coin.time, value: coin.price.map((p) => (p - 1) * 1e4) }); // bp from $1
peg.createPriceLine({ price: -50, color: '#ffb547', lineStyle: 'dashed', title: 'Depeg −50 bp' });
chart.addSeries('area', { color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.25)', bottomFill: 'rgba(76, 201, 240, 0)' }, chart.addPane(0.5))
  .setData({ time: coin.time, value: coin.supply.map((v) => v / 1e9) }); // USD bn
chart.setBarSpacing(1.4);
chart.scrollToRealtime();`,
      about: 'Four months of a stablecoin’s price every four hours as basis points from $1, as a `baseline` series around the peg, with a −50 bp alert line. Below, the circulating supply in USD billions: holders redeem during the depegs.',
      api: ["addSeries('baseline')", 'createPriceLine', 'addPane'],
    },
    {
      id: 'mintredeem',
      title: 'Mint & redeem flows',
      group: 'Stablecoins & staking',
      code: `${create()}
${load('flows', 'series/defiMintRedeem', { ...DAY, count: 240 })}
// Supply: the running sum of net issuance from a starting supply.
let supply = 8e9;
const total = flows.net.map((v) => (supply += v) / 1e9);
chart.addSeries('line', { color: '#7ee787', lineWidth: 2 }).setData({ time: flows.time, value: total }); // USD bn
chart.addSeries('histogram', {}, chart.addPane(0.7)).setData({
  time: flows.time,
  value: flows.net.map((v) => v / 1e6), // USD m
  color: flows.net.map((v) => (v >= 0 ? '#2ee6b6' : '#ff5d73')),
});
chart.setBarSpacing(4.2);
chart.scrollToRealtime();`,
      about: 'A stablecoin’s daily net issuance in USD millions, mints up and redemptions down, and the supply it adds up to in USD billions. The supply is a running sum in the example.',
      api: ["addSeries('histogram')", 'per-point color'],
    },
    {
      id: 'stablesupply',
      title: 'Stablecoin supply by chain',
      group: 'Stablecoins & staking',
      code: `${create()}
${load('supply', 'series/defiStableChains', { ...DAY, count: 730 })}
const chains = [
  { key: 'chainA', color: '#4cc9f0', fill: '#1d4b5c' },
  { key: 'chainB', color: '#b388ff', fill: '#3d3262' },
  { key: 'chainC', color: '#7ee787', fill: '#2a5034' },
  { key: 'chainD', color: '#ffb547', fill: '#5a4524' },
  { key: 'chainE', color: '#ff5d73', fill: '#5a2a33' },
];
// Stacked areas: each layer is the running total up to its chain, in USD bn. The
// tallest goes first, so the lower layers, with opaque fills, cover it.
for (let k = chains.length - 1; k >= 0; k--) {
  const below = chains.slice(0, k + 1);
  const value = supply.time.map((t, i) => below.reduce((sum, c) => sum + supply[c.key][i], 0) / 1e9);
  const { color, fill } = chains[k];
  if (k === chains.length - 1) {
    chart.applyPriceScaleOptions({ range: { low: 0, high: Math.max(...value) * 1.08 }, margins: { top: 0, bottom: 0 } });
  }
  chart.addSeries('area', { color, topFill: fill, bottomFill: fill, lineWidth: 1, lastValueVisible: false, priceLineVisible: false })
    .setData({ time: supply.time, value });
}
chart.setBarSpacing(1.4);
chart.scrollToRealtime();`,
      about: 'Two years of one stablecoin’s supply in USD billions, stacked by chain: Chain A at the bottom, Chain E on top. Each layer is an `area` series of a cumulative sum, added from the top down so the lower ones cover the higher.',
      api: ["addSeries('area')", 'stacking'],
    },
    {
      id: 'unstakeq',
      title: 'Staking queues',
      group: 'Stablecoins & staking',
      code: `${create()}
${load('queue', 'series/defiUnstake', DAY)}
const k = (v) => v / 1e3; // thousand ETH
chart.addSeries('area', { color: '#ff5d73', topFill: 'rgba(255, 93, 115, 0.3)', bottomFill: 'rgba(255, 93, 115, 0)' })
  .setData({ time: queue.time, value: queue.exit.map(k) });
chart.addSeries('area', { color: '#2ee6b6', topFill: 'rgba(46, 230, 182, 0.3)', bottomFill: 'rgba(46, 230, 182, 0)' })
  .setData({ time: queue.time, value: queue.entry.map(k) });
// Wait in days: the queue over the churn limit, 57 600 ETH a day.
const pane = chart.addPane(0.6);
const days = (v) => v / 57_600;
chart.addSeries('line', { color: '#ff5d73', lineWidth: 2 }, pane).setData({ time: queue.time, value: queue.exit.map(days) });
chart.addSeries('line', { color: '#2ee6b6', lineWidth: 2 }, pane).setData({ time: queue.time, value: queue.entry.map(days) });
chart.setBarSpacing(2.8);
chart.scrollToRealtime();`,
      about: 'A year of the staking queues in thousand ETH: waiting to exit (red) and to enter (green). Below, the wait in days, the queue over the daily churn limit. Exits pile up when the price falls.',
      api: ["addSeries('area')", 'addPane'],
    },
    {
      id: 'lstpeg',
      title: 'LST · LRT discount',
      group: 'Stablecoins & staking',
      code: `${create()}
${load('peg', 'series/defiLst', DAY)}
const pct = (v) => v * 100; // % to backing
const lst = chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', lineWidth: 2 });
lst.setData({ time: peg.time, value: peg.lst.map(pct) });
lst.createPriceLine({ price: 0, color: '#8b93a7', lineStyle: 'dotted', title: 'Staking token' });
const lrt = chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', lineWidth: 2 }, chart.addPane(1));
lrt.setData({ time: peg.time, value: peg.lrt.map(pct) });
lrt.createPriceLine({ price: 0, color: '#8b93a7', lineStyle: 'dotted', title: 'Restaking token' });
chart.setBarSpacing(2.8);
chart.scrollToRealtime();`,
      about: 'Market price of a liquid staking token (top) and a restaking token (bottom) against the ETH backing them, in percent, as `baseline` series around par. The restaking token trades at wider discounts in sell-offs.',
      api: ["addSeries('baseline')", 'createPriceLine'],
    },
    // ----------------------------------------------------------- Prices & tokens
    {
      id: 'oraclelag',
      title: 'Oracle deviation',
      group: 'Prices & tokens',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '1m', count: 720 })}
// A push oracle: updates when the market moves 0.5% from it, or after an hour.
const threshold = 0.005, heartbeat = 60;
const oracle = [];
let last = bars.close[0], since = 0;
for (const c of bars.close) {
  if (Math.abs(c / last - 1) >= threshold || ++since >= heartbeat) { last = c; since = 0; }
  oracle.push(last);
}
chart.addSeries('line', { color: '#4cc9f0' }).setData({ time: bars.time, value: bars.close });
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, step: true }).setData({ time: bars.time, value: oracle });
const pane = chart.addPane(0.6);
const dev = chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73' }, pane);
dev.setData({ time: bars.time, value: bars.close.map((c, i) => (c / oracle[i] - 1) * 1e4) }); // bp
dev.createPriceLine({ price: 50, color: '#ffb547', lineStyle: 'dashed', title: '+50 bp' });
dev.createPriceLine({ price: -50, color: '#ffb547', lineStyle: 'dashed', title: '−50 bp' });
chart.setBarSpacing(2);
chart.scrollToRealtime();`,
      about: 'Twelve hours of one-minute prices: the market (blue) and an oracle (amber steps) that updates on a 0.5% move or an hourly heartbeat. Below, the market’s deviation from the oracle in basis points, against the ±50 bp trigger.',
      api: ['step lines', "addSeries('baseline')", 'createPriceLine'],
    },
    {
      id: 'airdrop',
      title: 'Airdrop claims & selling',
      group: 'Prices & tokens',
      code: `${create()}
${bars({ symbol: 'MEME', interval: '1d', count: 90 })}
${load('drop', 'series/defiAirdrop', { symbol: 'MEME', interval: '1d', count: 90 })}
// Cumulative tokens claimed and sold since the airdrop (the first bar), in millions.
const running = (xs) => { let s = 0; return xs.map((v) => (s += v / 1e6)); };
const claimed = chart.addSeries('area', { color: '#7ee787', topFill: 'rgba(126, 231, 135, 0.3)', bottomFill: 'rgba(126, 231, 135, 0.03)' });
claimed.setData({ time: drop.time, value: running(drop.claimed) });
const allocation = 200; // million tokens
claimed.createPriceLine({ price: allocation, color: '#8b93a7', lineStyle: 'dashed', title: 'Allocation' });
chart.applyPriceScaleOptions({ range: { low: 0, high: allocation * 1.1 } });
claimed.setMarkers([{ time: drop.time[0], position: 'aboveBar', shape: 'arrowDown', color: '#7ee787', text: 'Airdrop' }]);
chart.addSeries('area', { color: '#ff5d73', topFill: 'rgba(255, 93, 115, 0.35)', bottomFill: 'rgba(255, 93, 115, 0.05)' })
  .setData({ time: drop.time, value: running(drop.sold) });
chart.addCandlestickSeries({}, chart.addPane(0.6)).setData(bars);
chart.setBarSpacing(11);
chart.scrollToRealtime();`,
      about: 'The three months after a token airdrop: tokens claimed (green) and sold (red), cumulative in millions against the 200 M allocation, with the token’s daily candles below. Claims are front-loaded; selling follows them.',
      api: ["addSeries('area')", 'createPriceLine', 'setMarkers'],
    },
    {
      id: 'xpool',
      title: 'Cross-pool spreads',
      group: 'Prices & tokens',
      code: `${create()}
${load('px', 'series/defiPools', { symbol: 'ETHUSD', interval: '1m', count: 360 })}
chart.addSeries('line', { color: '#8b93a7', lineWidth: 2 }).setData({ time: px.time, value: px.ref });
// Each pool's price against the reference venue, in basis points, in its own pane.
const pools = [['pool1', 'Pool 1', '#4cc9f0'], ['pool2', 'Pool 2', '#b388ff'], ['pool3', 'Pool 3', '#ffb547']];
for (const [key, name, color] of pools) {
  const spread = chart.addSeries('baseline', { baseValue: 0, color, downColor: color, lineWidth: 1 }, chart.addPane(0.5));
  spread.setData({ time: px.time, value: px[key].map((p, i) => (p / px.ref[i] - 1) * 1e4) });
  spread.createPriceLine({ price: 0, color: '#8b93a7', lineStyle: 'dotted', title: name });
}
chart.setBarSpacing(3);
chart.scrollToRealtime();`,
      about: 'Six hours of one-minute ETH prices: the reference venue on top and, below, each pool’s deviation from it in basis points. Pools that update more slowly fall further behind in fast moves until arbitrage closes the gap.',
      api: ["addSeries('baseline')", 'addPane', 'createPriceLine'],
    },
    ...plotDemos.defi,
  ],
};
