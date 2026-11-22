import { bars, create, load, view } from './build';
import type { Query } from './build';
import type { Category } from './types';

const tok = (interval: string, count: number, ahead?: number): Query =>
  ahead ? { symbol: 'MEME', interval, count, ahead } : { symbol: 'MEME', interval, count };

/** A pane title, drawn by a one-line layer plugin. */
const TITLE = `const title = (pane, text) => chart.addLayer({ pane, build: (ctx) => [
  { type: 'text', x: ctx.rect.x + 8, y: ctx.rect.y + 6, text, color: '#8a93a6', size: 11 },
] });`;

/** A legend in the top-left corner of `pane`, drawn by a layer plugin. */
const LEGEND = `const legend = (pane, items) => chart.addLayer({ pane, position: 'overlay', build: (ctx) => items.flatMap(([text, color], i) => [
  { type: 'rect', x: ctx.rect.x + 10, y: ctx.rect.y + 13 + i * 16, width: 12, height: 3, color },
  { type: 'text', x: ctx.rect.x + 28, y: ctx.rect.y + 8 + i * 16, text, color: '#c9d1e0', size: 11 },
]) });`;

/** Millions with one decimal: `12.3M`. */
const FMT = "const m = (v) => (Math.abs(v) / 1e6).toFixed(1) + 'M';";

export const meme: Category = {
  id: 'meme',
  title: 'Memecoins & wallets',
  kicker: '09',
  blurb: 'Launches, holders, developer wallets and social heat around a volatile token.',
  accent: '#ff6ec7',
  demos: [
    {
      id: 'meme',
      title: 'Market-cap chart with trade labels',
      group: 'Price',
      code: `${create()}
${bars(tok('15m', 600))}
${load('supply', 'series/memeSupply', tok('15m', 600))}
${load('fills', 'events/memeFills', tok('15m', 600))}
${load('news', 'events/memeNews', tok('15m', 600))}
${FMT}
// Quote the token by market cap: every price times the supply, in $M.
const cap = (col) => bars[col].map((p, i) => (p * supply.value[i]) / 1e6);
const candles = chart.addCandlestickSeries({ pricePrecision: 1 });
candles.setData({ time: bars.time, open: cap('open'), high: cap('high'), low: cap('low'), close: cap('close') });
const capAt = new Map(bars.time.map((t, i) => [t, (bars.close[i] * supply.value[i]) / 1e6]));
const markers = fills.time.map((time, i) => {
  const buy = fills.value[i] > 0;
  return buy
    ? { time, position: 'belowBar', shape: 'arrowUp', color: '#2ee6b6', text: 'Buy ' + m(fills.value[i]) }
    : { time, position: 'aboveBar', shape: 'arrowDown', color: '#ff5d73', text: 'Sell ' + m(fills.value[i]) };
});
news.time.forEach((time, i) => markers.push({ time, position: 'aboveBar', shape: 'circle', color: '#ffb547', text: news.label[i] }));
candles.setMarkers(markers.sort((a, b) => a.time - b.time));
// Average entry of the buys (size-weighted), with a take-profit and a stop.
let tokens = 0, cost = 0;
fills.time.forEach((t, i) => {
  if (fills.value[i] > 0) { tokens += fills.value[i]; cost += fills.value[i] * capAt.get(t); }
});
const entry = cost / tokens;
candles.createPriceLine({ price: entry, color: '#4cc9f0', lineStyle: 'dashed', title: 'avg entry' });
candles.createPriceLine({ price: entry * 1.5, color: '#2ee6b6', lineStyle: 'dotted', title: 'TP' });
candles.createPriceLine({ price: entry * 0.8, color: '#ff5d73', lineStyle: 'dotted', title: 'SL' });
${view(5)}`,
      about: 'Fifteen-minute candles quoted in market cap ($M, price times supply), with a trader\'s buys and sells and the headlines as markers, and price lines for the average entry, take-profit and stop.',
      api: ['Series.setMarkers', 'createPriceLine'],
    },
    {
      id: 'meme-bubbles',
      title: 'Trade bubbles',
      group: 'Price',
      code: `${create()}
const to = Math.floor(Date.now() / 60_000) * 60_000; // the last whole minute
const from = to - 120 * 60_000;
const range = 'symbol=MEME&from=' + from + '&to=' + to;
const bars = await fetch('/api/bars?interval=1m&' + range).then((r) => r.json());
const trades = await fetch('/api/trades?' + range).then((r) => r.json());
chart.addCandlestickSeries().setData(bars);
// The 80 largest trades; a bubble's area grows with the notional.
const big = trades.time
  .map((time, i) => ({ time, price: trades.price[i], usd: trades.price[i] * trades.size[i], buy: trades.side[i] > 0 }))
  .sort((a, b) => b.usd - a.usd)
  .slice(0, 80);
const labelFrom = big[9].usd; // label the top ten
chart.addLayer({
  pane: 0,
  build(ctx) {
    const out = [];
    for (const b of big) {
      const x = ctx.timeToX(b.time), y = ctx.priceToY(b.price);
      if (x === null || y === null || x < ctx.rect.x - 30 || x > ctx.rect.x + ctx.rect.width + 30) continue;
      const size = Math.min(48, 4 + 3 * Math.sqrt(b.usd / 1e6));
      out.push({ type: 'icon', x, y, size, color: b.buy ? 'rgba(46, 230, 182, 0.45)' : 'rgba(255, 93, 115, 0.45)' });
      if (b.usd >= labelFrom) out.push({ type: 'text', x, y: y - 6, text: '$' + (b.usd / 1e6).toFixed(1) + 'M', color: '#e6ebf5', size: 10, align: 'center' });
    }
    return out;
  },
});
${view(8)}`,
      about: 'A layer plugin draws a bubble at the time and price of each of the 80 largest trades of the last two hours, sized by its notional and colored by the aggressor: buys green, sells red. The top ten are labeled.',
      api: ['addLayer', "{ type: 'icon' }", 'ctx.priceToY'],
    },
    {
      id: 'meme-line',
      title: 'Market cap step line',
      group: 'Price',
      code: `${create()}
${bars(tok('1h', 720))}
${load('supply', 'series/memeSupply', tok('1h', 720))}
${TITLE}
const cap = bars.close.map((p, i) => (p * supply.value[i]) / 1e6); // $M
const line = chart.addSeries('line', { color: '#ff6ec7', lineWidth: 2, step: true, pricePrecision: 1 });
line.setData({ time: bars.time, value: cap });
line.createPriceLine({ price: Math.max(...cap), color: '#ffb547', lineStyle: 'dashed', title: 'high' });
title(0, 'Market cap, $M');
const supplyPane = chart.addPane(0.35);
chart.addSeries('line', { color: '#8a93a6', lineWidth: 1.5, step: true, pricePrecision: 1 }, supplyPane)
  .setData({ time: supply.time, value: supply.value.map((v) => v / 1e6) });
title(supplyPane, 'Circulating supply, M tokens');
${view(1.4)}`,
      about: 'Market cap as a step line: the hourly close times the circulating supply, which changes in steps as tokens are burned or minted (second pane). A dashed price line marks the high of the window.',
      api: ["addSeries('line', { step: true })", 'createPriceLine'],
    },
    {
      id: 'topholders',
      title: 'Top-holder concentration',
      group: 'Holders',
      code: `${create()}
${bars(tok('4h', 720))}
${load('holders', 'series/memeHolders', tok('4h', 720))}
${LEGEND}
chart.addCandlestickSeries().setData(bars);
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
const groups = [['top1', 'Largest wallet', '#ff6ec7'], ['top2to5', 'Wallets 2-5', '#b388ff'], ['top6to10', 'Wallets 6-10', '#4cc9f0']];
// Stack the shares in percent; draw the largest running total first.
let sum = holders.time.map(() => 0);
const totals = groups.map(([col]) => (sum = sum.map((s, i) => s + holders[col][i] * 100)));
const pane = chart.addPane(0.9);
for (let k = groups.length - 1; k >= 0; k--) {
  const color = groups[k][2];
  chart.addSeries('area', { color: '#0b0d12', lineWidth: 1, topFill: color, bottomFill: color, pricePrecision: 1, priceLineVisible: false, lastValueVisible: k === groups.length - 1 }, pane)
    .setData({ time: holders.time, value: totals[k] });
}
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0 } }, pane);
legend(pane, [...groups].reverse().map(([, name, color]) => [name + ', % of supply', color]));
chart.fitContent();`,
      about: 'How much of the supply the ten largest wallets hold, stacked in percent under four-hour candles: concentration falls as the token rallies and spreads to new holders. The axis label is the top-10 total.',
      api: ["addSeries('area')", 'addPane', "mode: 'logarithmic'"],
    },
    {
      id: 'devwallet',
      title: 'Developer wallet timeline',
      group: 'Holders',
      code: `${create()}
${bars(tok('1h', 720))}
${load('trades', 'events/devTrades', tok('1h', 720))}
${load('transfers', 'events/devTransfers', tok('1h', 720))}
${TITLE}
${FMT}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
const events = [
  ...trades.time.map((time, i) => ({ time, tokens: trades.value[i], kind: trades.value[i] > 0 ? 'buy' : 'sell' })),
  ...transfers.time.map((time, i) => ({ time, tokens: -transfers.value[i], kind: 'transfer' })),
].sort((a, b) => a.time - b.time);
const style = {
  buy: { position: 'belowBar', shape: 'arrowUp', color: '#2ee6b6', label: 'Dev buy ' },
  sell: { position: 'aboveBar', shape: 'arrowDown', color: '#ff5d73', label: 'Dev sell ' },
  transfer: { position: 'aboveBar', shape: 'square', color: '#ffb547', label: 'Out ' },
};
candles.setMarkers(events.map(({ time, tokens, kind }) => {
  const { label, ...s } = style[kind];
  return { time, ...s, text: label + m(tokens) };
}));
// The wallet's balance: a starting allocation changed by every event.
let balance = 120e6, next = 0;
const held = bars.time.map((t) => {
  while (next < events.length && events[next].time <= t) balance += events[next++].tokens;
  return balance / 1e6;
});
const pane = chart.addPane(0.45);
chart.addSeries('area', { color: '#ffb547', step: true, topFill: 'rgba(255, 181, 71, 0.25)', bottomFill: 'rgba(255, 181, 71, 0.02)', pricePrecision: 1 }, pane)
  .setData({ time: bars.time, value: held });
title(pane, 'Developer wallet, M tokens');
${view(3)}`,
      about: 'Every swap and transfer of the developer wallet as a marker on hourly candles, and the wallet\'s balance replayed from those events as a step area below.',
      api: ['Series.setMarkers', "addSeries('area')"],
    },
    {
      id: 'lplock',
      title: 'LP lock and burn',
      group: 'Liquidity',
      code: `${create()}
${bars(tok('4h', 720))}
${load('changes', 'events/lpChanges', tok('4h', 720))}
${load('burns', 'events/lpBurns', tok('4h', 720))}
${LEGEND}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
const events = [
  ...changes.time.map((time, i) => ({ time, kind: changes.label[i], pct: Math.abs(changes.value[i]) })),
  ...burns.time.map((time, i) => ({ time, kind: 'LP burn', pct: burns.value[i] })),
].sort((a, b) => a.time - b.time);
const style = {
  Lock: { position: 'belowBar', shape: 'square', color: '#4cc9f0' },
  Unlock: { position: 'aboveBar', shape: 'arrowDown', color: '#ffb547' },
  'LP burn': { position: 'belowBar', shape: 'circle', color: '#b388ff' },
};
candles.setMarkers(events.map(({ time, kind, pct }) => ({ time, ...style[kind], text: kind + ' ' + pct.toFixed(1) + '%' })));
// Replay the pool's state: percent of LP tokens locked and burned.
let locked = 55, burned = 0, next = 0;
const lockedPct = [], burnedPct = [];
for (const t of bars.time) {
  for (; next < events.length && events[next].time <= t; next++) {
    const { kind, pct } = events[next];
    if (kind === 'Lock') locked = Math.min(100 - burned, locked + pct);
    if (kind === 'Unlock') locked = Math.max(0, locked - pct);
    if (kind === 'LP burn') { burned += pct; locked = Math.min(locked, 100 - burned); }
  }
  lockedPct.push(locked + burned);
  burnedPct.push(burned);
}
const pane = chart.addPane(0.6);
chart.addSeries('area', { color: '#4cc9f0', step: true, topFill: 'rgba(76, 201, 240, 0.35)', bottomFill: 'rgba(76, 201, 240, 0.35)', pricePrecision: 1 }, pane)
  .setData({ time: bars.time, value: lockedPct });
chart.addSeries('area', { color: '#b388ff', step: true, topFill: '#3b2f5c', bottomFill: '#3b2f5c', pricePrecision: 1 }, pane)
  .setData({ time: bars.time, value: burnedPct });
chart.applyPriceScaleOptions({ range: { low: 0, high: 100 } }, pane);
legend(pane, [['Locked + burned, % of LP', '#4cc9f0'], ['Burned, % of LP', '#b388ff']]);
${view(2)}`,
      about: 'Liquidity-pool locks, unlocks and burns as markers, and the share of LP tokens that cannot be pulled, replayed from those events as stacked step areas on a fixed 0–100% scale.',
      api: ['Series.setMarkers', "addSeries('area', { step: true })"],
    },
    {
      id: 'social',
      title: 'Social heat and price',
      group: 'Activity',
      code: `${create()}
${bars(tok('1h', 720))}
${load('social', 'series/memeSocial', tok('1h', 720))}
${TITLE}
chart.addCandlestickSeries().setData(bars);
const mentionPane = chart.addPane(0.6);
chart.addSeries('histogram', { pricePrecision: 1 }, mentionPane).setData({
  time: social.time,
  value: social.mentions.map((v) => v / 1e3),
  color: social.sentiment.map((s) => (s > 0.25 ? '#2ee6b6' : s < -0.25 ? '#ff5d73' : '#5a6478')),
});
title(mentionPane, 'Mentions per hour, thousands (colored by mood)');
const moodPane = chart.addPane(0.5);
chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', pricePrecision: 2 }, moodPane)
  .setData({ time: social.time, value: social.sentiment });
title(moodPane, 'Sentiment, -1 to 1');
${view(3)}`,
      about: 'Mentions per hour as a histogram colored by the mood of the posts, and average sentiment from −1 to 1 as a `baseline` series: hype peaks with the price.',
      api: ["addSeries('histogram')", 'per-point color', "addSeries('baseline')"],
    },
    {
      id: 'washtrade',
      title: 'Suspected wash trading',
      group: 'Activity',
      code: `${create()}
${bars(tok('15m', 600))}
${load('wash', 'series/memeWash', tok('15m', 600))}
${load('flags', 'events/washFlags', tok('15m', 600))}
${TITLE}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
candles.setMarkers(flags.time.map((time, i) => ({
  time, position: 'aboveBar', shape: 'circle', color: '#ff6ec7',
  text: '$' + (flags.value[i] / 1e3).toFixed(0) + 'k',
})));
const share = wash.value.map((v) => v * 100);
const pane = chart.addPane(0.6);
const bars2 = chart.addSeries('histogram', { pricePrecision: 0 }, pane);
bars2.setData({ time: wash.time, value: share, color: share.map((v) => (v > 30 ? '#ff6ec7' : '#4a5163')) });
bars2.createPriceLine({ price: 30, color: '#ff6ec7', lineStyle: 'dashed', title: 'flag' });
title(pane, 'Volume flagged as wash trading, %');
${view(5)}`,
      about: 'The share of each bar\'s volume flagged as likely wash trading, pink above a 30% threshold, with markers on single flagged trades and their notional.',
      api: ["addSeries('histogram')", 'per-point color', 'Series.setMarkers'],
    },
    {
      id: 'arblifetime',
      title: 'Arbitrage window lifetimes',
      group: 'Liquidity',
      code: `${create()}
${load('spread', 'series/memeVenueSpread', tok('5s', 720))}
${TITLE}
// An opportunity lasts while the gap between the two pools stays beyond 25 bp.
const limit = 25, bar = 5_000;
const windows = [];
spread.value.forEach((v, i) => {
  const open = Math.abs(v) > limit, w = windows[windows.length - 1];
  if (open && w && w.end === i - 1 && Math.sign(v) === w.side) {
    w.end = i;
    w.peak = Math.max(w.peak, Math.abs(v));
  } else if (open) windows.push({ start: i, end: i, side: Math.sign(v), peak: Math.abs(v) });
});
const line = chart.addSeries('line', { color: '#c9d1e0', lineWidth: 1, pricePrecision: 1 });
line.setData({ time: spread.time, value: spread.value });
for (const p of [limit, -limit]) line.createPriceLine({ price: p, color: '#5c6577', lineStyle: 'dashed', axisLabelVisible: false });
title(0, 'Pool B over pool A, bp; boxes: open windows');
chart.addLayer({
  pane: 0,
  position: 'belowSeries',
  build(ctx) {
    const out = [];
    for (const w of windows) {
      const x0 = ctx.timeToX(spread.time[w.start] - bar / 2), x1 = ctx.timeToX(spread.time[w.end] + bar / 2);
      if (x0 === null || x1 === null || x1 < ctx.rect.x || x0 > ctx.rect.x + ctx.rect.width) continue;
      const y0 = ctx.priceToY(w.side * w.peak), y1 = ctx.priceToY(w.side * limit);
      out.push({ type: 'rect', x: x0, y: Math.min(y0, y1), width: Math.max(2, x1 - x0), height: Math.abs(y1 - y0), color: w.side > 0 ? 'rgba(76, 201, 240, 0.35)' : 'rgba(255, 181, 71, 0.35)' });
    }
    return out;
  },
});
const pane = chart.addPane(0.5);
const lifetime = spread.time.map(() => NaN);
windows.forEach((w) => (lifetime[w.start] = ((w.end - w.start + 1) * bar) / 1000));
chart.addSeries('histogram', { color: '#ff6ec7', pricePrecision: 0, lastValueVisible: false, priceLineVisible: false }, pane)
  .setData({ time: spread.time, value: lifetime });
title(pane, 'Window lifetime, seconds');
${view(5)}`,
      about: 'The price gap between two pools every 5 seconds. Runs beyond ±25 bp are arbitrage windows: a layer plugin boxes each one from open to close and up to its peak, and the histogram below shows how many seconds it lived.',
      api: ['addLayer', "{ type: 'rect' }", 'NaN gaps'],
    },
    {
      id: 'lifecycle',
      title: 'Launch, graduation, listing',
      group: 'Liquidity',
      code: `${create()}
${bars(tok('1d', 400))}
${load('supply', 'series/memeSupply', tok('1d', 400))}
const cap = bars.close.map((p, i) => p * supply.value[i]);
// Stages by market cap: the bonding curve graduates at $250k, the pool goes live
// ten days later, and an exchange lists the token at $20M.
const grad = cap.findIndex((c) => c >= 250e3);
const live = Math.min(grad + 10, cap.length - 1);
const listed = cap.findIndex((c, i) => i > live && c >= 20e6);
const end = cap.length;
const stages = [
  [0, grad, 'Bonding curve', 'rgba(179, 136, 255, 0.10)'],
  [grad, live, 'Migration', 'rgba(255, 181, 71, 0.16)'],
  [live, listed, 'DEX pool', 'rgba(76, 201, 240, 0.08)'],
  [listed, end, 'Exchange listed', 'rgba(46, 230, 182, 0.08)'],
];
const candles = chart.addCandlestickSeries({ pricePrecision: 6 });
candles.setData(bars);
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
candles.setMarkers([
  { time: bars.time[grad], position: 'belowBar', shape: 'arrowUp', color: '#ffb547', text: 'Graduated' },
  { time: bars.time[live], position: 'belowBar', shape: 'circle', color: '#4cc9f0', text: 'Pool live' },
  { time: bars.time[listed], position: 'belowBar', shape: 'arrowUp', color: '#2ee6b6', text: 'Listed' },
]);
const day = 86_400_000;
chart.addLayer({
  pane: 0,
  position: 'background',
  build(ctx) {
    const out = [];
    for (const [from, to, name, color] of stages) {
      const x0 = ctx.timeToX(bars.time[from] - day / 2), x1 = ctx.timeToX((bars.time[to] ?? bars.time[end - 1] + day) - day / 2);
      if (x0 === null || x1 === null) continue;
      out.push({ type: 'rect', x: x0, y: ctx.rect.y, width: x1 - x0, height: ctx.rect.height, color });
      if (x1 - x0 > 60) out.push({ type: 'text', x: x0 + 6, y: ctx.rect.y + 8, text: name, color: '#c9d1e0', size: 11 });
    }
    return out;
  },
});
chart.fitContent();`,
      about: 'A token\'s first year on a log scale. The stages are set by market cap in the example: a layer plugin shades the bonding curve, the migration, the DEX pool and the exchange-listed phase, and markers flag the transitions.',
      api: ['addLayer', "position: 'background'", 'Series.setMarkers'],
    },
    {
      id: 'groupflow',
      title: 'Wallet group flows',
      group: 'Holders',
      code: `${create()}
${bars(tok('1h', 720))}
${load('flows', 'series/memeGroups', tok('1h', 720))}
${TITLE}
${LEGEND}
chart.addCandlestickSeries().setData(bars);
const flowPane = chart.addPane(0.5);
const smart = flows.smart.map((v) => v / 1e6);
chart.addSeries('histogram', { pricePrecision: 2 }, flowPane).setData({
  time: flows.time, value: smart, color: smart.map((v) => (v >= 0 ? '#2ee6b6' : '#ff5d73')),
});
title(flowPane, 'Smart money net buys per hour, M tokens');
// Holdings change since the start of the window: running sums of each group's flow.
const groups = [['smart', 'Smart money', '#2ee6b6'], ['whales', 'Whales', '#b388ff'], ['retail', 'Retail', '#ffb547']];
const holdPane = chart.addPane(0.8);
for (const [col, , color] of groups) {
  let total = 0;
  chart.addSeries('line', { color, lineWidth: 2, pricePrecision: 0 }, holdPane)
    .setData({ time: flows.time, value: flows[col].map((v) => (total += v / 1e6)) });
}
legend(holdPane, groups.map(([, name, color]) => [name + ', net M tokens', color]));
${view(3)}`,
      about: 'Net buying by labeled wallet groups: smart money\'s hourly flow as a signed histogram, and every group\'s running total since the start of the window as lines. The legend is drawn by a layer plugin.',
      api: ["addSeries('histogram')", "addSeries('line')", 'addPane'],
    },
    {
      id: 'unlock',
      title: 'Unlock schedule and supply',
      group: 'Holders',
      code: `${create()}
${bars(tok('1d', 180))}
${load('unlocks', 'events/memeUnlocks', tok('1d', 180, 180))}
${LEGEND}
${FMT}
chart.addCandlestickSeries({ pricePrecision: 4 }).setData(bars);
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
// Daily supply for the window and the next 180 days: 1B tokens, of which the circulating
// part grows at each unlock.
const day = 86_400_000, total = 1000;
const time = [...bars.time];
for (let i = 1; i <= 180; i++) time.push(bars.time[bars.time.length - 1] + i * day);
let circulating = 420, next = 0;
const circ = time.map((t) => {
  while (next < unlocks.time.length && unlocks.time[next] <= t) circulating += unlocks.value[next++] / 1e6;
  return circulating;
});
const pane = chart.addPane(1);
const locked = { color: '#3a3f4f', topFill: '#2a2e3b', bottomFill: '#2a2e3b', lineWidth: 1, pricePrecision: 0, lastValueVisible: false, priceLineVisible: false };
chart.addSeries('area', locked, pane).setData({ time, value: time.map(() => total) });
const supply = chart.addSeries('area', { color: '#ff6ec7', step: true, topFill: 'rgba(255, 110, 199, 0.45)', bottomFill: 'rgba(255, 110, 199, 0.2)', pricePrecision: 0 }, pane);
supply.setData({ time, value: circ });
const now = bars.time[bars.time.length - 1];
supply.setMarkers(unlocks.time.map((t, i) => ({
  time: t, position: 'aboveBar', shape: 'arrowUp', color: t > now ? '#ffb547' : '#8a93a6', text: m(unlocks.value[i]),
})));
chart.applyPriceScaleOptions({ range: { low: 0, high: 1100 }, margins: { top: 0.2, bottom: 0 } }, pane);
legend(pane, [['Circulating, M tokens', '#ff6ec7'], ['Still locked (of 1B)', '#3a3f4f'], ['Scheduled unlocks', '#ffb547']]);
chart.fitContent();`,
      about: 'Monthly token unlocks over the last half year and the next one: the circulating supply steps up at each unlock (markers, future ones in amber) inside the fixed 1B total. Future times extend the axis past the last candle.',
      api: ["addSeries('area', { step: true })", 'Series.setMarkers', 'future data'],
    },
  ],
};
