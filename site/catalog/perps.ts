import { bars, create, load, view } from './build';
import type { Query } from './build';
import { plotDemos } from './plots';
import type { Category } from './types';

const H = { symbol: 'BTCUSD', interval: '1h', count: 720 };

/** BTCUSD at `interval`, `count` bars. */
const btc = (interval: string, count: number): Query => ({ symbol: 'BTCUSD', interval, count });

/** A pane title, drawn by a one-line layer plugin. */
const TITLE = `const title = (pane, text) => chart.addLayer({ pane, build: (ctx) => [
  { type: 'text', x: ctx.rect.x + 8, y: ctx.rect.y + 6, text, color: '#8a93a6', size: 11 },
] });`;

/** A legend in the top-left corner of \`pane\`, drawn by a layer plugin. */
const LEGEND = `const legend = (pane, items) => chart.addLayer({ pane, position: 'overlay', build: (ctx) => items.flatMap(([text, color], i) => [
  { type: 'rect', x: ctx.rect.x + 10, y: ctx.rect.y + 13 + i * 16, width: 12, height: 3, color },
  { type: 'text', x: ctx.rect.x + 28, y: ctx.rect.y + 8 + i * 16, text, color: '#c9d1e0', size: 11 },
]) });`;

export const perps: Category = {
  id: 'perps',
  title: 'Crypto derivatives',
  kicker: '06',
  blurb: 'Funding, open interest, liquidations and basis in panes under price.',
  accent: '#4cc9f0',
  demos: [
    {
      id: 'derivs',
      title: 'Funding · open interest · liquidations',
      group: 'Derivatives',
      code: `${create()}
${bars(H)}
${load('funding', 'metrics/funding', H)}
${load('oi', 'metrics/openInterest', H)}
${load('liq', 'metrics/liquidations', H)}
chart.addCandlestickSeries().setData(bars);
const up = '#2ee6b6', down = '#ff5d73';
chart.addSeries('histogram', { pricePrecision: 3 }, chart.addPane(0.6)).setData({
  time: funding.time, value: funding.value.map((v) => v * 100), // percent
  color: funding.value.map((v) => (v >= 0 ? up : down)),
});
chart.addSeries('area', { color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.25)', bottomFill: 'rgba(76, 201, 240, 0)' }, chart.addPane(0.6))
  .setData({ time: oi.time, value: oi.value });
const liqPane = chart.addPane(0.6);
chart.addSeries('histogram', { color: down }, liqPane).setData({ time: liq.time, value: liq.long.map((v) => -v) });
chart.addSeries('histogram', { color: up }, liqPane).setData({ time: liq.time, value: liq.short });
chart.setBarSpacing(6);
chart.scrollToRealtime();`,
      about: 'Three derived series under one-hour candles: funding per 8 h in percent, colored by sign, open interest, and liquidated longs (down) and shorts (up). All share the time axis and crosshair.',
      api: ["addSeries('histogram')", 'addPane', 'per-point color'],
    },
    {
      id: 'positioning',
      title: 'Long/short ratio · CVD',
      group: 'Derivatives',
      code: `${create()}
${bars({ ...H, interval: '15m', count: 1500 })}
${load('ratio', 'metrics/longShortRatio', { ...H, interval: '15m', count: 1500 })}
${load('cvd', 'metrics/cvd', { ...H, interval: '15m', count: 1500 })}
chart.addCandlestickSeries().setData(bars);
chart.addSeries('baseline', { baseValue: 1, color: '#2ee6b6', downColor: '#ff5d73' }, chart.addPane(0.7))
  .setData({ time: ratio.time, value: ratio.value });
chart.addSeries('line', { color: '#ffb547', lineWidth: 2 }, chart.addPane(0.7))
  .setData({ time: cvd.time, value: cvd.value });
chart.setBarSpacing(6);
chart.scrollToRealtime();`,
      about: 'Account positioning against the move: the long/short ratio around 1 as a baseline series, and cumulative volume delta.',
      api: ["addSeries('baseline')", "addSeries('line')"],
    },
    {
      id: 'volatility',
      title: 'Implied volatility · basis',
      group: 'Derivatives',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '4h', count: 900 })}
${load('iv', 'metrics/impliedVol', { symbol: 'ETHUSD', interval: '4h', count: 900 })}
${load('basis', 'metrics/basis', { symbol: 'ETHUSD', interval: '4h', count: 900 })}
chart.addCandlestickSeries().setData(bars);
chart.addSeries('area', { color: '#b388ff', topFill: 'rgba(179, 136, 255, 0.3)', bottomFill: 'rgba(179, 136, 255, 0)' }, chart.addPane(0.7))
  .setData({ time: iv.time, value: iv.value.map((v) => v * 100) });
chart.addSeries('line', { color: '#4cc9f0', step: true }, chart.addPane(0.7))
  .setData({ time: basis.time, value: basis.value.map((v) => v * 100) });
chart.setBarSpacing(5);
chart.scrollToRealtime();`,
      about: 'At-the-money implied volatility and the annualised futures basis, both in percent, under four-hour candles.',
      api: ["addSeries('area')", 'step lines'],
    },
    {
      id: 'lsr',
      title: 'Long/short ratio · liquidations',
      group: 'Positioning',
      code: `${create()}
${bars(btc('15m', 1000))}
${load('ratio', 'metrics/longShortRatio', btc('15m', 1000))}
${load('liq', 'metrics/liquidations', btc('15m', 1000))}
${TITLE}
chart.addCandlestickSeries().setData(bars);
const ratioPane = chart.addPane(0.55);
chart.addSeries('baseline', { baseValue: 1, color: '#2ee6b6', downColor: '#ff5d73', pricePrecision: 2 }, ratioPane)
  .setData({ time: ratio.time, value: ratio.value });
title(ratioPane, 'Long/short accounts');
const liqPane = chart.addPane(0.55);
const millions = (v) => v / 1e6;
chart.addSeries('histogram', { color: '#ff5d73', pricePrecision: 1 }, liqPane)
  .setData({ time: liq.time, value: liq.long.map((v) => -millions(v)) });
chart.addSeries('histogram', { color: '#2ee6b6', pricePrecision: 1 }, liqPane)
  .setData({ time: liq.time, value: liq.short.map(millions) });
title(liqPane, 'Liquidations, $M: longs down, shorts up');
${view(6)}`,
      about: 'The ratio of long to short accounts as a `baseline` series around 1, and liquidated longs (down) and shorts (up) in $M as two histograms on one pane. Pane titles come from a one-line layer plugin.',
      api: ["addSeries('baseline')", "addSeries('histogram')", 'addPane'],
    },
    {
      id: 'oiquad',
      title: 'Open interest × price quadrants',
      group: 'Positioning',
      code: `${create()}
${bars(H)}
${load('oi', 'metrics/openInterest', H)}
${LEGEND}
chart.addCandlestickSeries().setData(bars);
// Who is acting: the sign of the price move and of the change in open interest.
const kinds = {
  longsOpen: ['Longs opening', '#2ee6b6'], // price up, OI up
  shortsOpen: ['Shorts opening', '#ff5d73'], // price down, OI up
  shortsClose: ['Shorts covering', '#4cc9f0'], // price up, OI down
  longsClose: ['Longs closing', '#ffb547'], // price down, OI down
};
const change = oi.value.map((v, i) => (i ? v - oi.value[i - 1] : 0));
const color = change.map((d, i) => {
  const up = bars.close[i] >= bars.open[i];
  return kinds[d >= 0 ? (up ? 'longsOpen' : 'shortsOpen') : up ? 'shortsClose' : 'longsClose'][1];
});
const pane = chart.addPane(0.8);
chart.addSeries('histogram', { pricePrecision: 0 }, pane).setData({ time: oi.time, value: change, color });
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0.05 } }, pane); // room for the legend
legend(pane, Object.values(kinds));
const oiPane = chart.addPane(0.45);
chart.addSeries('line', { color: '#8a93a6', lineWidth: 1.5, pricePrecision: 0 }, oiPane).setData({ time: oi.time, value: oi.value });
legend(oiPane, [['Open interest, contracts', '#8a93a6']]);
${view(6)}`,
      about: 'The hourly change in open interest, colored by which of the four price/OI quadrants the bar falls in, with open interest itself below. The legend is drawn by a small layer plugin.',
      api: ["addSeries('histogram')", 'per-point color', 'addLayer'],
    },
    {
      id: 'liqtimeline',
      title: 'Liquidation bubbles',
      group: 'Liquidations',
      code: `${create()}
${bars(H)}
${load('liq', 'metrics/liquidations', H)}
chart.addCandlestickSeries().setData(bars);
// One bubble per hour with more than $10M liquidated: longs under the low, shorts over the high.
const bubbles = [];
liq.time.forEach((time, i) => {
  if (liq.long[i] > 1e7) bubbles.push({ time, price: bars.low[i], usd: liq.long[i], color: 'rgba(255, 93, 115, 0.55)' });
  if (liq.short[i] > 1e7) bubbles.push({ time, price: bars.high[i], usd: liq.short[i], color: 'rgba(46, 230, 182, 0.55)' });
});
chart.addLayer({
  pane: 0,
  build(ctx) {
    const out = [];
    for (const b of bubbles) {
      const x = ctx.timeToX(b.time);
      if (x === null || x < ctx.rect.x - 40 || x > ctx.rect.x + ctx.rect.width + 40) continue;
      const size = Math.min(56, 6 + 3.2 * Math.sqrt(b.usd / 1e6));
      const y = ctx.priceToY(b.price);
      if (y === null) continue;
      out.push({ type: 'icon', x, y, size, color: b.color });
      if (b.usd > 5e7) out.push({ type: 'text', x, y: y - 6, text: \`$\${Math.round(b.usd / 1e6)}M\`, color: '#e6ebf5', size: 10, align: 'center' });
    }
    return out;
  },
});
${view(7)}`,
      about: 'A layer plugin draws a bubble for every hour with over $10M liquidated, sized by the amount: longs under the candle (red) and shorts above it (green). The largest are labeled.',
      api: ['addLayer', "{ type: 'icon' }", 'ctx.timeToX'],
    },
    {
      id: 'liqheat',
      title: 'Liquidation heatmap',
      group: 'Liquidations',
      code: `${create()}
${bars(H)}
${load('oi', 'metrics/openInterest', H)}
// A simple model: new open interest each hour is split over leverages, half long and
// half short; a level is cleared once the price trades through it.
const leverage = [[10, 0.3], [25, 0.35], [50, 0.25], [100, 0.1]];
const step = 150; // $150 rows
const heatmap = chart.addHeatmap({
  source: 'depth', tickSize: step, columnMs: 3_600_000, scale: 'linear', range: { percentile: 0.98 },
  colors: ['rgba(11, 13, 18, 0)', '#1b2f52', '#2b6f9e', '#e9b949', '#fff4c2'],
});
let levels = [];
for (let i = 1; i < bars.time.length; i++) {
  const added = Math.max(0, oi.value[i] - oi.value[i - 1]);
  for (const [lev, share] of leverage) {
    levels.push({ price: bars.close[i] * (1 - 1 / lev), size: (added * share) / 2, long: true });
    levels.push({ price: bars.close[i] * (1 + 1 / lev), size: (added * share) / 2, long: false });
  }
  levels = levels.filter((l) => (l.long ? bars.low[i] > l.price : bars.high[i] < l.price));
  levels.forEach((l) => (l.size *= 0.96)); // positions closed by hand
  const rows = new Map();
  for (const l of levels) {
    const row = Math.round(l.price / step) * step;
    rows.set(row, (rows.get(row) ?? 0) + l.size);
  }
  heatmap.appendDepth({ time: bars.time[i], prices: [...rows.keys()], sizes: [...rows.values()] });
}
chart.addCandlestickSeries().setData(bars);
${view(7)}`,
      about: 'Estimated liquidation levels per hour as an `addHeatmap` fed with `appendDepth`: each hour\'s new open interest is spread over 10–100× leverage, and a level disappears once price trades through it. Bright bands are clusters that price is drawn to.',
      api: ['addHeatmap', 'Series.appendDepth'],
    },
    {
      id: 'premium',
      title: 'Premium index',
      group: 'Funding & basis',
      code: `${create()}
${bars(btc('5m', 864))}
${load('premium', 'series/perpPremium', btc('5m', 864))}
chart.addCandlestickSeries().setData(bars);
chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', pricePrecision: 3 }, chart.addPane(0.6))
  .setData({ time: premium.time, value: premium.value.map((v) => v * 100) }); // percent
${view(4)}`,
      about: 'The perpetual\'s premium over the index price in percent, as a `baseline` series: green when the perp trades rich (longs pay), red when it trades at a discount.',
      api: ["addSeries('baseline')", 'baseValue'],
    },
    {
      id: 'fundcum',
      title: 'Cumulative funding by venue',
      group: 'Funding & basis',
      code: `${create()}
${bars(btc('1d', 365))}
${load('funding', 'series/venueFunding', btc('1d', 365))}
${LEGEND}
chart.addCandlestickSeries().setData(bars);
const pane = chart.addPane(0.8);
const venues = [['venueA', 'Venue A', '#4cc9f0'], ['venueB', 'Venue B', '#b388ff'], ['venueC', 'Venue C', '#ffb547']];
for (const [col, , color] of venues) {
  let total = 0; // three 8-hour payments a day, in percent
  const value = funding[col].map((r) => (total += 3 * r * 100));
  chart.addSeries('line', { color, lineWidth: 2, pricePrecision: 2 }, pane).setData({ time: funding.time, value });
}
legend(pane, venues.map(([, name, color]) => [name, color]));
chart.fitContent();`,
      about: 'What holding a long perpetual for a year cost on three venues: the per-8-hour funding rate summed into a running total in percent. The legend is drawn by a layer plugin.',
      api: ["addSeries('line')", 'addPane', 'addLayer'],
    },
    {
      id: 'toptrader',
      title: 'Top traders vs all accounts',
      group: 'Positioning',
      code: `${create()}
${bars(H)}
${load('lsr', 'series/traderLsr', H)}
${LEGEND}
chart.addCandlestickSeries().setData(bars);
const pane = chart.addPane(0.8);
const top = chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 2 }, pane);
top.setData({ time: lsr.time, value: lsr.top });
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.5, pricePrecision: 2 }, pane).setData({ time: lsr.time, value: lsr.retail });
top.createPriceLine({ price: 1, color: '#5c6577', lineStyle: 'dashed', axisLabelVisible: false });
legend(pane, [['Top traders, by position', '#ffb547'], ['All accounts', '#4cc9f0']]);
${view(6)}`,
      about: 'The long/short ratio of the largest traders against that of all accounts. The crowd tends to chase moves while top traders lean against them. The legend is drawn by a layer plugin.',
      api: ["addSeries('line')", 'createPriceLine', 'addLayer'],
    },
    {
      id: 'cmegap',
      title: 'Futures weekend gaps',
      group: 'Funding & basis',
      code: `${create()}
${bars(btc('2h', 900))}
// Dated futures close on Friday at 22:00 UTC and reopen on Sunday at 22:00 UTC, while spot
// trades through the weekend: the reopen often gaps away from Friday's close.
const H2 = 2 * 3_600_000; // bar length
const gaps = [];
bars.time.forEach((t, i) => {
  const d = new Date(t);
  if (d.getUTCDay() !== 0 || d.getUTCHours() !== 22) return;
  const fri = bars.time.findIndex((u) => u === t - 2 * 86_400_000 - H2);
  if (fri < 0) return;
  const close = bars.close[fri], open = bars.open[i];
  if (Math.abs(open / close - 1) < 0.004) return;
  let filled = null;
  for (let j = i; j < bars.time.length && filled === null; j++) {
    if (bars.low[j] <= close && bars.high[j] >= close) filled = bars.time[j];
  }
  gaps.push({ from: bars.time[fri] + H2, reopen: t, filled, lo: Math.min(close, open), hi: Math.max(close, open) });
});
chart.addCandlestickSeries().setData(bars);
chart.addLayer({
  pane: 0,
  build(ctx) {
    const out = [];
    const right = ctx.rect.x + ctx.rect.width;
    for (const g of gaps) {
      const x0 = ctx.timeToX(g.from), x1 = ctx.timeToX(g.reopen);
      if (x0 === null || x1 === null || x0 > right) continue;
      const x2 = g.filled === null ? right : ctx.timeToX(g.filled);
      if (x2 === null || x2 < ctx.rect.x) continue;
      const y0 = ctx.priceToY(g.hi), y1 = ctx.priceToY(g.lo);
      out.push({ type: 'rect', x: x0, y: ctx.rect.y, width: x1 - x0, height: ctx.rect.height, color: 'rgba(138, 147, 166, 0.07)' });
      const open = g.filled === null;
      const edge = open ? '#ffb547' : '#5c6577';
      out.push({ type: 'rect', x: x0, y: y0, width: x2 - x0, height: Math.max(2, y1 - y0), color: open ? 'rgba(255, 181, 71, 0.16)' : 'rgba(138, 147, 166, 0.16)' });
      out.push({ type: 'line', x1: x0, y1: y0, x2, y2: y0, color: edge, width: 1 });
      out.push({ type: 'line', x1: x0, y1: y1, x2, y2: y1, color: edge, width: 1 });
      const pct = (g.hi / g.lo - 1) * 100;
      out.push({ type: 'text', x: x0 + 4, y: y0 - 18, text: \`gap \${pct.toFixed(1)}%\${open ? ', open' : ', filled'}\`, color: edge, size: 10, background: '#0b0d12', padding: 2 });
    }
    return out;
  },
});
${view(4.5)}`,
      about: 'A layer plugin shades each weekend and draws the gap between a futures market\'s Friday close and its Sunday reopen, both taken from round-the-clock spot candles here. A gap is grey once price trades back to the Friday close and amber while it is still open.',
      api: ['addLayer', "{ type: 'rect' }", 'ctx.priceToY'],
    },
    {
      id: 'exshare',
      title: 'Volume share by venue',
      group: 'Venues',
      code: `${create()}
${load('share', 'series/venueShare', btc('1d', 365))}
const venues = [['venueA', 'Venue A', '#4cc9f0'], ['venueB', 'Venue B', '#b388ff'], ['venueC', 'Venue C', '#2ee6b6'], ['venueD', 'Venue D', '#ffb547'], ['venueE', 'Venue E', '#ff5d73']];
// Stack the shares: each area is drawn from its running total down, the largest total first.
const totals = [];
let sum = share.time.map(() => 0);
for (const [col] of venues) {
  sum = sum.map((s, i) => s + share[col][i] * 100);
  totals.push(sum);
}
const opts = { lineWidth: 1, lastValueVisible: false, priceLineVisible: false, pricePrecision: 1 };
for (let k = venues.length - 1; k >= 0; k--) {
  const color = venues[k][2];
  chart.addSeries('area', { ...opts, color: '#0b0d12', topFill: color, bottomFill: color }).setData({ time: share.time, value: totals[k] });
}
chart.applyPriceScaleOptions({ range: { low: 0, high: 100 }, margins: { top: 0, bottom: 0 } });
// Label each band at its latest midpoint.
chart.addLayer({
  pane: 0,
  position: 'overlay',
  build(ctx) {
    const last = share.time.length - 1;
    return venues.map(([col, name], k) => {
      const mid = totals[k][last] - (share[col][last] * 100) / 2;
      const y = ctx.priceToY(mid);
      return { type: 'text', x: ctx.rect.x + ctx.rect.width - 8, y: y - 7, text: \`\${name}  \${(share[col][last] * 100).toFixed(0)}%\`, color: '#0b0d12', size: 11, align: 'right' };
    });
  },
});
chart.fitContent();`,
      about: 'Each venue\'s share of perpetual volume, stacked to 100% with `area` series drawn from the top total down on a fixed 0–100 scale. A layer plugin labels the bands with their latest share.',
      api: ["addSeries('area')", "applyPriceScaleOptions({ range })", 'addLayer'],
    },
    {
      id: 'insfund',
      title: 'Insurance fund · auto-deleveraging',
      group: 'Liquidations',
      code: `${create()}
${bars(btc('4h', 1080))}
${load('fund', 'series/insuranceFund', btc('4h', 1080))}
${load('adl', 'events/adlEvents', btc('4h', 1080))}
chart.addCandlestickSeries().setData(bars);
const balance = chart.addSeries('area', {
  color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.3)', bottomFill: 'rgba(76, 201, 240, 0.02)', pricePrecision: 2,
}, chart.addPane(0.7));
balance.setData({ time: fund.time, value: fund.value.map((v) => v / 1e9) }); // $B
balance.setMarkers(adl.time.map((time, i) => ({
  time, position: 'aboveBar', shape: 'arrowDown', color: '#ffb547', text: \`ADL $\${Math.round(adl.value[i] / 1e6)}M\`,
})));
${view(3)}`,
      about: 'The insurance fund balance in $B under four-hour candles. Markers flag auto-deleveraging rounds after violent moves, with the notional closed.',
      api: ['Series.setMarkers', "addSeries('area')"],
    },
    {
      id: 'oiweighted',
      title: 'OI-weighted funding',
      group: 'Funding & basis',
      code: `${create()}
${bars(H)}
${load('funding', 'series/venueFunding', H)}
${load('oi', 'series/venueOi', H)}
// Average the venues' rates, weighting each by its open interest.
const venues = ['venueA', 'venueB', 'venueC'];
const value = funding.time.map((_, i) => {
  let paid = 0, total = 0;
  for (const v of venues) {
    paid += funding[v][i] * oi[v][i];
    total += oi[v][i];
  }
  return (paid / total) * 100; // percent per 8 h
});
chart.addCandlestickSeries().setData(bars);
chart.addSeries('baseline', {
  baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', pricePrecision: 3,
  topFill: 'rgba(46, 230, 182, 0.28)', bottomFill: 'rgba(255, 93, 115, 0.28)',
}, chart.addPane(0.6)).setData({ time: funding.time, value });
${view(6)}`,
      about: 'One funding rate for the market: the venues\' per-8-hour rates averaged with their open interest as weights, as a `baseline` series that turns red when shorts pay.',
      api: ["addSeries('baseline')", 'baseValue'],
    },
    ...plotDemos.perps,
  ],
};
