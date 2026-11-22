import { bars, create, load, view } from './build';
import type { Category } from './types';

/** A pane with a fixed 0..rows scale for a layer that draws lanes; a transparent series gives it the time axis. */
const LANES = (rows: number, time: string, stretch = 0.5) => `const lanes = chart.addPane(${stretch});
chart.addSeries('line', { color: 'transparent', pricePrecision: 0, lastValueVisible: false, priceLineVisible: false }, lanes)
  .setData({ time: ${time}, value: ${time}.map(() => 0) });
chart.applyPriceScaleOptions({ range: { low: -0.6, high: ${rows} - 0.4 } }, lanes);`;

export const labs: Category = {
  id: 'labs',
  title: 'Labs & flagships',
  kicker: '20',
  blurb: 'Experimental examples and stress tests: a decade of bars, dense order books, latency and look-ahead audits.',
  accent: '#e879f9',
  demos: [
    {
      id: 'deepzoom',
      title: 'Ten years, one series',
      group: 'Flagships',
      code: `${create()}
// Ten years of hourly bars: five requests of 20 000, oldest first.
const H = 3_600_000, PAGE = 20_000 * H;
const now = Math.floor(Date.now() / H) * H;
const pages = await Promise.all([4, 3, 2, 1, 0].map((k) =>
  fetch(\`/api/bars?symbol=BTCUSD&interval=1h&from=\${now - (k + 1) * PAGE}&to=\${now - k * PAGE}\`).then((r) => r.json())));
const bars = { time: [], open: [], high: [], low: [], close: [], volume: [] };
for (const page of pages) {
  page.time.forEach((t, i) => {
    if (t <= (bars.time[bars.time.length - 1] ?? -Infinity)) return; // pages share their edges
    for (const k in bars) bars[k].push(page[k][i]);
  });
}
const candles = chart.addCandlestickSeries();
candles.setData(bars); // ~87 600 bars, one call
chart.addVolumeSeries(candles);
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
chart.fitContent(); // zoom in: every hour is still there`,
      about: 'A decade of hourly candles, about 87 600 bars, in one series on a log scale. The engine reduces what is on screen per pixel, so the full view stays sharp and zooming in reaches single hours without reloading anything.',
      api: ['setData', 'fitContent', "mode: 'logarithmic'"],
    },
    {
      id: 'bookmap4k',
      title: 'Dense order book heatmap',
      group: 'Flagships',
      code: `${create({ maxBarSpacing: 120 })}
const to = Math.floor(Date.now() / 1000) * 1000;
const from = to - 10 * 60_000; // ten minutes
const range = \`symbol=BTCUSD&from=\${from}&to=\${to}\`;
// 2 400 snapshots, 250 ms apart, 200 levels of $2 a side: 960 000 cells.
const book = await fetch(\`/api/depth?\${range}&every=250&levels=200&step=4\`).then((r) => r.json());
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
const ticks = await fetch(\`/api/bars?interval=1s&\${range}\`).then((r) => r.json());
const heatmap = chart.addHeatmap({ source: 'depth', tickSize: 2, columnMs: 250 });
book.time.forEach((time, i) => heatmap.appendDepth({ time, prices: book.prices[i], sizes: book.sizes[i] }));
chart.addSeries('line', { color: '#ffffff', lineWidth: 1.5 }).setData({ time: ticks.time, value: ticks.close });
// Larger trades as bubbles: one points series per side and size.
for (const [side, color] of [[1, '#2ee6b6'], [-1, '#ff5d73']]) {
  for (const [min, max, size] of [[0.25, 1, 7], [1, Infinity, 14]]) {
    const time = [], value = [];
    trades.time.forEach((t, i) => {
      if (trades.side[i] !== side || trades.size[i] < min || trades.size[i] >= max || t <= (time[time.length - 1] ?? 0)) return;
      time.push(t);
      value.push(trades.price[i]);
    });
    chart.addSeries('points', { color: color + 'c0', lineWidth: size, lastValueVisible: false, priceLineVisible: false }).setData({ time, value });
  }
}
chart.setBarSpacing(1.6);
chart.scrollToRealtime();`,
      about: 'Ten minutes of the book at 250 ms and $2 resolution, 960 000 cells, aggregated per pixel on the GPU, with the last price and the larger trades as bubbles sized by quantity and colored by aggressor.',
      api: ['addHeatmap', 'appendDepth', "addSeries('points')"],
    },
    {
      id: 'capacityterrain',
      title: 'Capacity terrain',
      group: 'Flagships',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '15m', count: 600 })}
// Estimated cost (bps) of an order of $0.25M to $10M in each 15-minute bar:
// half the spread plus a square-root impact on the bar's range and volume.
const sizes = Array.from({ length: 40 }, (_, k) => 0.25 * (k + 1)); // USD millions
const terrain = chart.addHeatmap({
  source: 'depth', tickSize: 0.25, columnMs: 15 * 60_000, scale: 'linear',
  range: { lo: 0, hi: 40 },
  colors: ['#0b3d3a', '#1f8a70', '#f4d35e', '#f08a24', '#d7263d'],
});
// Range and volume as two-hour (8-bar) averages, so the estimate is not one bar's noise.
const avg = (f, i) => { let s = 0; for (let j = Math.max(0, i - 7); j <= i; j++) s += f(j); return s / (i - Math.max(0, i - 7) + 1); };
bars.time.forEach((time, i) => {
  const rangeBps = avg((j) => ((bars.high[j] - bars.low[j]) / bars.close[j]) * 1e4, i);
  const traded = avg((j) => (bars.volume[j] * bars.close[j]) / 1e6, i); // USD millions
  const cost = sizes.map((s) => 1 + 0.6 * rangeBps * Math.sqrt(s / Math.max(traded, 1)));
  terrain.appendDepth({ time, prices: sizes, sizes: cost });
});
chart.applyPriceScaleOptions({ range: { low: 0, high: 10.25 } });
// Price below, for context.
chart.addCandlestickSeries({}, chart.addPane(0.6)).setData(bars);
${view(6)}`,
      about: 'How much it would cost to trade, by order size (vertical, USD millions) and time: cool where liquidity is deep, hot where an order would move the price. A heatmap series used for an estimate instead of a book, over the price it describes.',
      api: ['addHeatmap', 'appendDepth', 'applyPriceScaleOptions({ range })'],
    },
    {
      id: 'trustrail',
      title: 'Data trust rail',
      group: 'Data quality',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '5m', count: 600 })}
${load('feed', 'series/feedQuality', { symbol: 'BTCUSD', interval: '5m', count: 600 })}
chart.addCandlestickSeries().setData(bars);
// One cell per bar: deviation from the reference venue beats thin trading.
const median = feed.trades.slice().sort((a, b) => a - b)[feed.trades.length >> 1];
const status = feed.time.map((_, i) =>
  Math.abs(feed.deviation[i]) > 8 ? '#ff5d73' // price off the reference by more than 8 bps
  : feed.trades[i] < median * 0.35 ? '#b388ff' // almost no trades: a gap
  : feed.trades[i] < median * 0.6 ? '#ffb547' // sparse
  : '#2ee6b6');
const rail = chart.addPane(0.1);
chart.addSeries('histogram', { lastValueVisible: false, priceLineVisible: false }, rail)
  .setData({ time: feed.time, value: feed.time.map(() => 1), color: status });
chart.applyPriceScaleOptions({ range: { low: 0, high: 1 } }, rail);
// The deviation itself, in basis points.
chart.addSeries('baseline', { baseValue: 0, color: '#ff5d73', downColor: '#ff5d73', topFill: '#ff5d7330', bottomFill: '#ff5d7330', lineWidth: 1, pricePrecision: 1 }, chart.addPane(0.35))
  .setData({ time: feed.time, value: feed.deviation });
${view(8)}`,
      about: 'A rail under the candles rates every bar: green normal, amber sparse, violet nearly empty (against the median trade count), red when the price strays more than 8 bps from a reference venue (the deviation is plotted below). A histogram with per-point colors in a thin pane.',
      api: ["addSeries('histogram')", 'per-point color', 'addPane'],
    },
    {
      id: 'narrative',
      title: 'Narrative timeline',
      group: 'Context',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '4h', count: 720 })}
${load('chain', 'events/onchainEvents', { symbol: 'ETHUSD', interval: '4h', count: 720 })}
${load('macro', 'events/macroEvents', { symbol: 'ETHUSD', interval: '4h', count: 720 })}
${load('social', 'series/socialHeat', { symbol: 'ETHUSD', interval: '4h', count: 720 })}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
// On-chain events above the bars, macro news below.
candles.setMarkers([
  ...chain.time.map((time, i) => ({ time, position: 'aboveBar', shape: 'circle', color: '#e879f9', text: chain.label[i] })),
  ...macro.time.map((time, i) => ({ time, position: 'belowBar', shape: 'square', color: '#4cc9f0', text: macro.label[i] })),
].sort((a, b) => a.time - b.time));
// Social activity, thousands of posts per four hours.
chart.addSeries('histogram', { color: '#e879f9', pricePrecision: 1 }, chart.addPane(0.35))
  .setData({ time: social.time, value: social.posts.map((v) => v / 1e3) });
${view(5)}`,
      about: 'Price with the story around it: on-chain events above the candles, macro news below, and social activity in the pane underneath, so a move can be read against what was happening. Two event feeds merged into `setMarkers`.',
      api: ['setMarkers', "addSeries('histogram')"],
    },
    {
      id: 'mirror',
      title: 'Execution mirror',
      group: 'Context',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '15m', count: 400 })}
${load('mine', 'events/myFills', { symbol: 'BTCUSD', interval: '15m', count: 400 })}
${load('smart', 'events/smartFills', { symbol: 'BTCUSD', interval: '15m', count: 400 })}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
const index = (t) => bars.time.findIndex((x) => x >= t);
// For each of our fills (at the bar's close): the best price within an hour either side.
const fills = mine.time.map((time, k) => {
  const i = index(time), buy = mine.value[k] >= 0;
  let best = i;
  for (let j = Math.max(0, i - 4); j <= Math.min(bars.time.length - 1, i + 4); j++) {
    if (buy ? bars.low[j] < bars.low[best] : bars.high[j] > bars.high[best]) best = j;
  }
  const price = bars.close[i], bestPrice = buy ? bars.low[best] : bars.high[best];
  return { i, best, buy, price, bestPrice, bps: (Math.abs(price - bestPrice) / price) * 1e4 };
});
candles.setMarkers([
  ...fills.map((f) => ({ time: bars.time[f.i], position: f.buy ? 'belowBar' : 'aboveBar', shape: f.buy ? 'arrowUp' : 'arrowDown', color: '#4cc9f0', text: f.buy ? 'B' : 'S' })),
  ...smart.time.map((time, k) => ({ time, position: smart.value[k] >= 0 ? 'belowBar' : 'aboveBar', shape: 'circle', color: '#e879f9' })),
].sort((a, b) => a.time - b.time));
// Lines from each fill to the best price nearby, with the gap in bps.
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const out = [];
    for (const f of fills) {
      const x1 = ctx.timeToX(bars.time[f.i]), x2 = ctx.timeToX(bars.time[f.best]);
      const y1 = ctx.priceToY(f.price), y2 = ctx.priceToY(f.bestPrice);
      if (x1 === null || x2 === null || y1 === null || y2 === null) continue;
      out.push(
        { type: 'line', x1, y1, x2, y2, color: '#ffb547', dash: 'dashed', width: 1.5 },
        { type: 'icon', x: x1, y: y1, shape: 'circle', size: 7, color: '#4cc9f0' },
        { type: 'icon', x: x2, y: y2, shape: 'circle', size: 7, color: '#ffb547' },
        { type: 'text', x: x2 + 6, y: y2, text: \`\${f.bps.toFixed(0)} bps\`, color: '#ffb547', size: 10, align: 'left' },
      );
    }
    return out;
  },
});
${view(13)}`,
      about: 'A replay mirror for a desk: our fills (sky) against the best price available within an hour either side (amber, with the gap in bps) and the fills of a well-informed account (pink dots). A layer plugin draws the comparison lines.',
      api: ['addLayer', 'setMarkers'],
    },
    {
      id: 'asofaudit',
      title: 'Look-ahead audit',
      group: 'Data quality',
      code: `${create()}
${bars({ symbol: 'ACME', interval: '1d', count: 400 })}
${load('fund', 'events/pitFundamentals', { symbol: 'ACME', interval: '1d', count: 400 })}
${load('macro', 'events/pitMacro', { symbol: 'ACME', interval: '1d', count: 400 })}
${load('alt', 'events/pitAltData', { symbol: 'ACME', interval: '1d', count: 400 })}
chart.addSeries('area', { color: '#e879f9', topFill: '#e879f930', bottomFill: '#e879f900', lineWidth: 2 }).setData({ time: bars.time, value: bars.close });
// Each record: the period it describes ends (as-of) and it is published "lag" days later.
const DAY = 86_400_000;
const rows = [['Fundamentals', fund], ['Macro', macro], ['Alt data', alt]];
${LANES(3, 'bars.time', 0.45)}
chart.addLayer({
  pane: lanes,
  build(ctx) {
    const out = [];
    rows.forEach(([name, feed], r) => {
      const y = ctx.priceToY(2 - r);
      feed.time.forEach((asOf, k) => {
        const x1 = ctx.timeToX(asOf), x2 = ctx.timeToX(asOf + feed.value[k] * DAY);
        if (x1 === null || x2 === null || x2 < ctx.rect.x || x1 > ctx.rect.x + ctx.rect.width) return;
        // Red: known as of this date but not yet published. A backtest that reads it here looks ahead.
        out.push(
          { type: 'rect', x: x1, y: y - 6, width: x2 - x1, height: 12, color: '#ff5d7350' },
          { type: 'icon', x: x1, y, shape: 'circle', size: 6, color: '#8b93a7' },
          { type: 'line', x1: x2, y1: y - 8, x2, y2: y + 8, color: '#2ee6b6', width: 2 },
        );
      });
      out.push({ type: 'text', x: ctx.rect.x + 8, y: y - 16, text: name, color: '#e6e9ef', size: 11, align: 'left' });
    });
    out.push({ type: 'text', x: ctx.rect.x + ctx.rect.width - 8, y: ctx.rect.y + 12, text: 'grey: as-of date · red: not yet public · green: usable', color: '#8b93a7', size: 10, align: 'right' });
    return out;
  },
});
${view(5)}`,
      about: 'For three datasets, when each record describes (grey dot) and when it became public (green tick). Data read inside the red span would not have been available yet: that is look-ahead bias in a backtest. A layer plugin draws the lanes under the price.',
      api: ['addLayer', 'addPane'],
    },
    {
      id: 'resilience',
      title: 'Liquidity resilience',
      group: 'Market quality',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '1m', count: 360 })}
${load('liq', 'series/liqResilience', { symbol: 'BTCUSD', interval: '1m', count: 360 })}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
// Shocks: the three largest one-minute moves.
const moves = bars.close.map((c, i) => ({ i, r: i ? c / bars.close[i - 1] - 1 : 0 }));
const shocks = moves.sort((a, b) => Math.abs(b.r) - Math.abs(a.r)).slice(0, 3).sort((a, b) => a.i - b.i);
candles.setMarkers(shocks.map(({ i, r }) => ({
  time: bars.time[i], position: r > 0 ? 'aboveBar' : 'belowBar', shape: 'circle', color: '#ffb547', text: \`Shock \${(r * 100).toFixed(2)}%\`,
})));
chart.addSeries('area', { color: '#ffb547', topFill: '#ffb54740', bottomFill: '#ffb54700', pricePrecision: 1 }, chart.addPane(0.5))
  .setData({ time: liq.time, value: liq.spread });
chart.addSeries('area', { color: '#4cc9f0', topFill: '#4cc9f040', bottomFill: '#4cc9f000', pricePrecision: 1 }, chart.addPane(0.5))
  .setData({ time: liq.time, value: liq.depth });
chart.fitContent();`,
      about: 'How the market absorbs shocks: one-minute candles with the three largest moves marked, the quoted spread in bps (amber) and resting depth within 10 bps in USD millions (sky). Resilient liquidity snaps back fast after each shock.',
      api: ['addPane', "addSeries('area')", 'setMarkers'],
    },
    {
      id: 'dualclock',
      title: 'Two clocks',
      group: 'Market quality',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '1s', count: 300 })}
${load('lat', 'series/feedLatency', { symbol: 'BTCUSD', interval: '1s', count: 300 })}
// When each one-second price reached us (in order, so never before the previous one).
const received = [];
bars.time.forEach((t, i) => received.push(Math.max(t + Math.round(lat.ms[i]), (received[i - 1] ?? 0) + 1)));
// What we believed at each exchange second: the latest price received by then.
let k = -1;
const seen = bars.time.map((t) => {
  while (k + 1 < received.length && received[k + 1] <= t) k++;
  return k >= 0 ? bars.close[k] : NaN;
});
const opts = { lineWidth: 2, lastValueVisible: false, priceLineVisible: false };
chart.addSeries('line', { ...opts, color: '#4cc9f0' }).setData({ time: bars.time, value: bars.close });
chart.addSeries('line', { ...opts, color: '#ffb547', step: true }).setData({ time: bars.time, value: seen });
const color = (ms) => (ms < 600 ? '#2ee6b6' : ms < 1500 ? '#ffb547' : '#ff5d73');
// A link per tick from exchange time to receive time (timeToX interpolates between bars).
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const out = [];
    bars.time.forEach((t, i) => {
      const x1 = ctx.timeToX(t), x2 = ctx.timeToX(received[i]), y = ctx.priceToY(bars.close[i]);
      if (x1 === null || x2 === null || y === null) return;
      out.push({ type: 'line', x1, y1: y, x2, y2: y, color: color(received[i] - t), width: 3 });
    });
    out.push({ type: 'text', x: ctx.rect.x + 10, y: ctx.rect.y + 14, text: 'sky: exchange price · amber: what we had seen · links: delay', color: '#8b93a7', size: 11, align: 'left' });
    return out;
  },
});
chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.4))
  .setData({ time: bars.time, value: lat.ms, color: lat.ms.map(color) });
${view(14)}`,
      about: 'One-second prices on two clocks: as the exchange stamped them (sky) and what we had actually received at that moment (amber steps). A layer plugin draws each tick\'s delay as a link from exchange time to receive time, colored by latency; the pane below shows the delay in ms.',
      api: ['addLayer', "addSeries('line')", 'per-point color'],
    },
    {
      id: 'atomicitybreak',
      title: 'Leg risk',
      group: 'Market quality',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '1s', count: 900 })}
${load('fills', 'events/legFills', { symbol: 'BTCUSD', interval: '1s', count: 900 })}
chart.addSeries('line', { color: '#e6e9ef', lineWidth: 1.5 }).setData({ time: bars.time, value: bars.close });
// Leg A buys spot, leg B sells the perpetual: each fill as a tick in its lane, height by size.
${LANES(2, 'bars.time', 0.35)}
chart.addLayer({
  pane: lanes,
  build(ctx) {
    const out = [];
    fills.time.forEach((t, k) => {
      const x = ctx.timeToX(t), leg = fills.value[k] >= 0 ? 0 : 1;
      if (x === null) return;
      const y = ctx.priceToY(1 - leg), h = 6 + 40 * Math.abs(fills.value[k]);
      out.push({ type: 'rect', x: x - 2, y: y - h / 2, width: 4, height: h, color: leg ? '#ff5d73' : '#2ee6b6' });
    });
    out.push({ type: 'text', x: ctx.rect.x + 8, y: ctx.priceToY(1) - 18, text: 'Leg A · buy spot', color: '#2ee6b6', size: 11, align: 'left' });
    out.push({ type: 'text', x: ctx.rect.x + 8, y: ctx.priceToY(0) - 18, text: 'Leg B · sell perpetual', color: '#ff5d73', size: 11, align: 'left' });
    return out;
  },
});
// Net exposure in BTC: zero only when both legs have filled the same amount.
let net = 0;
const exposure = bars.time.map((t) => {
  fills.time.forEach((ft, k) => { if (ft === t) net += fills.value[k]; });
  return net;
});
chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', topFill: '#2ee6b630', bottomFill: '#ff5d7330', step: true, pricePrecision: 2 }, chart.addPane(0.45))
  .setData({ time: bars.time, value: exposure });
${view(1.2)}`,
      about: 'A two-leg trade that is meant to be flat but fills leg by leg: the lanes show each fill as it happens, and the step baseline below shows the net BTC exposure carried while one leg runs ahead of the other. A layer plugin draws the lanes.',
      api: ['addLayer', "addSeries('baseline')", 'addPane'],
    },
    {
      id: 'missingimpact',
      title: 'Gap impact lens',
      group: 'Data quality',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '5m', count: 400 })}
${load('gaps', 'events/feedGaps', { symbol: 'ETHUSD', interval: '5m', count: 400 })}
// What a feed with outages delivered (NaN = missing), and a linear patch over each hole.
const n = bars.time.length;
const raw = bars.close.slice();
const holes = [];
gaps.time.forEach((t, k) => {
  const from = bars.time.findIndex((x) => x >= t), to = Math.min(n - 2, from + Math.round(gaps.value[k]));
  if (from < 1) return;
  for (let i = from; i <= to; i++) raw[i] = NaN;
  const last = holes[holes.length - 1];
  if (last && from <= last.to + 2) last.to = Math.max(last.to, to); // overlapping outages merge
  else holes.push({ from, to });
});
const patched = bars.close.map(() => NaN);
for (const { from, to } of holes) {
  const a = bars.close[from - 1], b = bars.close[to + 1];
  for (let i = from - 1; i <= to + 1; i++) patched[i] = a + ((b - a) * (i - from + 1)) / (to - from + 2);
}
const opts = { lastValueVisible: false, priceLineVisible: false };
chart.addSeries('line', { ...opts, color: '#8b93a7', lineWidth: 1, lineStyle: 'dotted' }).setData({ time: bars.time, value: bars.close });
chart.addSeries('line', { ...opts, color: '#4cc9f0', lineWidth: 2 }).setData({ time: bars.time, value: raw });
chart.addSeries('line', { ...opts, color: '#ffb547', lineWidth: 2 }).setData({ time: bars.time, value: patched });
// Shade each hole and print the patch's worst error against the true price.
chart.addLayer({
  pane: chart.mainPane,
  position: 'background',
  build(ctx) {
    const out = [];
    holes.forEach((h, k) => {
      const x1 = ctx.timeToX(bars.time[h.from - 1]), x2 = ctx.timeToX(bars.time[h.to + 1]);
      if (x1 === null || x2 === null) return;
      let err = 0;
      for (let i = h.from; i <= h.to; i++) err = Math.max(err, Math.abs(patched[i] / bars.close[i] - 1));
      out.push(
        { type: 'rect', x: x1, y: ctx.rect.y, width: x2 - x1, height: ctx.rect.height, color: '#ff5d731c' },
        { type: 'text', x: (x1 + x2) / 2, y: ctx.rect.y + 14 + (k % 2) * 16, text: \`\${h.to - h.from + 1} bars · max error \${(err * 100).toFixed(2)}%\`, color: '#ff8a9a', size: 10, align: 'center', background: '#0b0d12c0', padding: 3 },
      );
    });
    return out;
  },
});
chart.fitContent();`,
      about: 'Holes in a feed (shaded), the linear patch over each one (amber) against the true price (dotted grey) and what was received (sky), with the worst error of each patch. A background layer plugin shades the gaps.',
      api: ['addLayer', 'NaN gaps', "addSeries('line')"],
    },
    {
      id: 'gravity',
      title: 'Price gravity',
      group: 'Context',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '1h', count: 1000 })}
chart.addCandlestickSeries().setData(bars);
// Gravity per $250 band: volume traded there, recent bars weighing more.
const step = 250, bands = new Map();
bars.time.forEach((_, i) => {
  const lo = Math.floor(bars.low[i] / step), hi = Math.floor(bars.high[i] / step);
  const w = (bars.volume[i] * Math.exp((i - bars.time.length) / 300)) / (hi - lo + 1);
  for (let b = lo; b <= hi; b++) bands.set(b, (bands.get(b) ?? 0) + w);
});
const max = Math.max(...bands.values());
const top = [...bands].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([b]) => b);
chart.addLayer({
  pane: chart.mainPane,
  position: 'belowSeries',
  build(ctx) {
    const right = ctx.rect.x + ctx.rect.width, out = [];
    for (const [b, v] of bands) {
      const y1 = ctx.priceToY((b + 1) * step), y2 = ctx.priceToY(b * step);
      if (y1 === null || y2 === null || y2 < ctx.rect.y || y1 > ctx.rect.y + ctx.rect.height) continue;
      const f = v / max, w = f * ctx.rect.width * 0.3;
      out.push({ type: 'rect', x: right - w, y: y1 + 0.5, width: w, height: Math.max(1, y2 - y1 - 1), color: f > 0.7 ? '#e879f9a0' : f > 0.4 ? '#b388ff70' : '#4cc9f040' });
      if (top.includes(b)) out.push({ type: 'text', x: right - w - 6, y: (y1 + y2) / 2, text: ctx.formatPrice((b + 0.5) * step), color: '#e879f9', size: 11, align: 'right' });
    }
    return out;
  },
});
${view(5)}`,
      about: 'A sideways profile of where price has spent its volume, weighted toward recent bars, drawn against the right edge: long bands pull price, gaps let it travel. The three strongest levels are labelled. A layer plugin draws the profile.',
      api: ['addLayer', 'PluginCtx.formatPrice'],
    },
  ],
};
