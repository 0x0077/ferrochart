import { bars, create } from './build';
import { plotDemos } from './plots';
import type { Category } from './types';

/** The last `minutes` whole minutes of BTCUSD: bars at `interval`, and a
 * `range` query for trades and order books over the same time. */
const lastMinutes = (minutes: number, interval = '1m') => `const to = Math.floor(Date.now() / 60_000) * 60_000; // the last whole minute
const from = to - ${minutes} * 60_000;
const range = \`symbol=BTCUSD&from=\${from}&to=\${to}\`;
const bars = await fetch(\`/api/bars?interval=${interval}&\${range}\`).then((r) => r.json());`;

/** A legend in the top-left corner of `pane`, drawn by a layer plugin. */
const LEGEND = `const legend = (pane, items) => chart.addLayer({ pane, position: 'overlay', build: (ctx) => items.flatMap(([text, color], i) => [
  { type: 'rect', x: ctx.rect.x + 10, y: ctx.rect.y + 13 + i * 16, width: 12, height: 3, color },
  { type: 'text', x: ctx.rect.x + 28, y: ctx.rect.y + 8 + i * 16, text, color: '#c9d1e0', size: 11 },
]) });`;

/** A volume profile over a range of `bars`, with point of control and value area. */
const PROFILE = `// Volume at price over bars [from, to): each bar's volume is spread evenly over
// the rows it spans. Row k covers (lo + k) * row to (lo + k + 1) * row.
function profile(from, to, row) {
  const lo = Math.floor(Math.min(...bars.low.slice(from, to)) / row);
  const hi = Math.floor(Math.max(...bars.high.slice(from, to)) / row);
  const vol = new Array(hi - lo + 1).fill(0);
  for (let i = from; i < to; i++) {
    const a = Math.floor(bars.low[i] / row) - lo, b = Math.floor(bars.high[i] / row) - lo;
    for (let k = a; k <= b; k++) vol[k] += bars.volume[i] / (b - a + 1);
  }
  // Point of control, then the value area: add the busier neighbour until 70% of the volume.
  const total = vol.reduce((s, v) => s + v, 0);
  const poc = vol.indexOf(Math.max(...vol));
  let up = poc, dn = poc, sum = vol[poc];
  while (sum < 0.7 * total) {
    const above = vol[up + 1] ?? -1, below = vol[dn - 1] ?? -1;
    sum += above >= below ? vol[++up] : vol[--dn];
  }
  return { lo, row, vol, poc, up, dn };
}`;

export const orderflow: Category = {
  id: 'orderflow',
  title: 'Order flow',
  kicker: '04',
  blurb: 'Footprint, depth heatmap and market profile, on a compute-shader path.',
  accent: '#ff7a3d',
  demos: [
    {
      id: 'footprint',
      title: 'Footprint',
      group: 'Footprint & profile',
      code: `${create({ maxBarSpacing: 200 })}
${lastMinutes(60)}
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
chart.addCandlestickSeries().setData(bars);
chart.addFootprint({ tickSize: 5, timeframeMs: 60_000 }).appendTrades(trades);
chart.setBarSpacing(96);
chart.setRightOffset(0.5); // half a bar of space after the last one`,
      about: 'Sell and buy volume at every price inside each candle (in $5 rows), with diagonal imbalances and the point of control. Zoom out for shaded cells, in for numbers.',
      api: ['addFootprint', 'Series.appendTrades'],
    },
    {
      id: 'delta',
      title: 'Delta footprint',
      group: 'Footprint & profile',
      code: `${create({ maxBarSpacing: 200 })}
${lastMinutes(90)}
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
chart.addCandlestickSeries().setData(bars);
chart.addFootprint({ tickSize: 5, timeframeMs: 60_000, mode: 'delta' }).appendTrades(trades);
chart.setBarSpacing(64);
chart.setRightOffset(0.5);`,
      about: 'Net aggression per price level: buyers minus sellers, colored by sign.',
      api: ["addFootprint({ mode: 'delta' })"],
    },
    {
      id: 'heatmap',
      title: 'Depth heatmap',
      group: 'Order book',
      code: `${create()}
${lastMinutes(40)}
// 1 200 order-book snapshots, 2 s apart, 40 levels of $5 a side.
const book = await fetch(\`/api/depth?\${range}&every=2000&levels=40&step=10\`).then((r) => r.json());
const heatmap = chart.addHeatmap({ source: 'depth', tickSize: 5, columnMs: 2_000 });
book.time.forEach((time, i) => heatmap.appendDepth({ time, prices: book.prices[i], sizes: book.sizes[i] }));
chart.addCandlestickSeries().setData(bars);
chart.setBarSpacing(22);
chart.scrollToRealtime();`,
      about: 'Resting liquidity over time under one-minute candles; walls sit at fixed prices for a while. On WebGPU a compute shader aggregates the cells per pixel.',
      api: ['addHeatmap', 'Series.appendDepth'],
    },
    {
      id: 'tpo',
      title: 'Market profile (TPO)',
      group: 'Footprint & profile',
      code: `${create({ maxBarSpacing: 600 })}
${bars({ symbol: 'ES', interval: '1m', count: 1950 })} // five cash sessions
const tpo = chart.addTpo({
  tickSize: 0.25,
  rowSize: 28, // 7-point rows
  sessionOffsetMs: 13.5 * 3_600_000, // New York cash session, 13:30 UTC
  sessionMs: 6.5 * 3_600_000,
});
tpo.appendBars(bars);
chart.setBarSpacing(230);
chart.setRightOffset(0.5);`,
      about: 'One profile per session: letters per 30-minute period, point of control, value area and initial balance. Histograms when zoomed out.',
      api: ['addTpo', 'Series.appendBars'],
    },
    {
      id: 'volheat',
      title: 'Volume heat candles',
      group: 'Footprint & profile',
      code: `${create()}
${lastMinutes(75)}
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
// Volume per $10 inside each candle; the brightest cell of a candle is its busiest price.
chart.addFootprint({
  tickSize: 0.5,
  rowSize: 20,
  timeframeMs: 60_000,
  mode: 'volume',
  imbalanceRatio: null,
  colors: { volume: '#ff7a3d', poc: '#ffe1c2' },
}).appendTrades(trades);
chart.addSeries('ohlc', { color: '#2ee6b6', downColor: '#ff5d73' }).setData(bars);
chart.setBarSpacing(13);
chart.scrollToRealtime();`,
      about: 'Each one-minute bar is split into $10 cells shaded by the volume traded there, with the busiest cell outlined; thin OHLC bars on top keep the direction. It is the footprint series in `volume` mode at a spacing too narrow for numbers.',
      api: ["addFootprint({ mode: 'volume' })", "addSeries('ohlc')"],
    },
    {
      id: 'svp',
      title: 'Session volume profile',
      group: 'Footprint & profile',
      code: `${create()}
${bars({ symbol: 'ES', interval: '5m', count: 390 })} // five cash sessions
chart.addCandlestickSeries().setData(bars);
${PROFILE}

// One profile per session (each cash session falls on one UTC day), in 2-point rows.
const day = (i) => Math.floor(bars.time[i] / 86_400_000);
const sessions = [];
for (let start = 0, i = 1; i <= bars.time.length; i++) {
  if (i < bars.time.length && day(i) === day(start)) continue;
  sessions.push({ from: bars.time[start], to: bars.time[i - 1], ...profile(start, i, 2) });
  start = i;
}

chart.addLayer({
  pane: 0,
  position: 'belowSeries',
  build(ctx) {
    const out = [];
    for (const s of sessions) {
      const x0 = ctx.timeToX(s.from), x1 = ctx.timeToX(s.to);
      if (x0 === null || x1 === null) continue;
      const max = s.vol[s.poc];
      s.vol.forEach((v, k) => {
        const top = ctx.priceToY((s.lo + k + 1) * s.row), bottom = ctx.priceToY((s.lo + k) * s.row);
        const color = k === s.poc ? '#ffb547' : k >= s.dn && k <= s.up ? 'rgba(255, 122, 61, 0.42)' : 'rgba(255, 122, 61, 0.16)';
        out.push({ type: 'rect', x: x0, y: top, width: (v / max) * (x1 - x0) * 0.85, height: Math.max(1, bottom - top - 1), color });
      });
    }
    return out;
  },
});
chart.setBarSpacing(2.6);
chart.scrollToRealtime();`,
      about: 'A horizontal volume profile behind every session of five-minute futures candles: the point of control in amber, the 70% value area brighter. A layer plugin draws the profiles, computed once from the bar volumes.',
      api: ['addLayer', 'ctx.priceToY', "position: 'belowSeries'"],
    },
    {
      id: 'profile',
      title: 'Visible-range volume profile',
      group: 'Footprint & profile',
      code: `${create()}
${bars({ interval: '5m', count: 2000 })}
chart.addCandlestickSeries().setData(bars);
${PROFILE}

// Recomputed every frame for the bars in view, so it follows panning and zooming.
chart.addLayer({
  pane: 0,
  position: 'belowSeries',
  build(ctx) {
    const { x, width } = ctx.rect, right = x + width;
    let from = -1, to = -1;
    bars.time.forEach((t, i) => {
      const bx = ctx.timeToX(t);
      if (bx !== null && bx >= x && bx <= right) { if (from < 0) from = i; to = i + 1; }
    });
    if (from < 0) return [];
    // About 60 rows over the visible price range, in whole $5.
    const span = Math.max(...bars.high.slice(from, to)) - Math.min(...bars.low.slice(from, to));
    const p = profile(from, to, 5 * Math.max(1, Math.round(span / 300)));
    const out = [], max = p.vol[p.poc];
    p.vol.forEach((v, k) => {
      const top = ctx.priceToY((p.lo + k + 1) * p.row), bottom = ctx.priceToY((p.lo + k) * p.row);
      const w = (v / max) * width * 0.28;
      const color = k === p.poc ? 'rgba(255, 181, 71, 0.85)' : k >= p.dn && k <= p.up ? 'rgba(255, 122, 61, 0.35)' : 'rgba(255, 122, 61, 0.14)';
      out.push({ type: 'rect', x: right - w, y: top, width: w, height: Math.max(1, bottom - top - 1), color });
    });
    const level = (price, label, color) => {
      const y = ctx.priceToY(price);
      out.push({ type: 'line', x1: x, y1: y, x2: right, y2: y, color, width: 1, dash: 'dashed' });
      out.push({ type: 'text', x: x + 8, y: y - 8, text: \`\${label} \${ctx.formatPrice(price)}\`, color, size: 11, background: '#12151c', padding: 3 });
    };
    level((p.lo + p.poc + 0.5) * p.row, 'POC', '#ffb547');
    level((p.lo + p.up + 1) * p.row, 'VAH', '#8a93a6');
    level((p.lo + p.dn) * p.row, 'VAL', '#8a93a6');
    return out;
  },
});
chart.setBarSpacing(6);
chart.scrollToRealtime();`,
      about: 'The volume profile of whatever is on screen, at the right edge, with the point of control and the value area high and low across the chart. A layer plugin finds the visible bars with `timeToX` and rebuilds the profile every frame.',
      api: ['addLayer', 'ctx.timeToX', 'ctx.formatPrice'],
    },
    {
      id: 'anchoredfixedran',
      title: 'Anchored & fixed-range profile',
      group: 'Footprint & profile',
      code: `${create()}
${bars({ interval: '15m', count: 500 })}
chart.addCandlestickSeries().setData(bars);
${PROFILE}

const n = bars.time.length, ROW = 20;
// A fixed range of bars, and a profile anchored at the highest high of the last 300 bars
// that runs to the latest bar. Click a bar to move the anchor.
let anchor = bars.high.indexOf(Math.max(...bars.high.slice(n - 300)), n - 300);
let ranges;
const update = () => {
  ranges = [
    { label: 'Fixed range', from: n - 420, to: n - 300, ...profile(n - 420, n - 300, ROW) },
    { label: 'Anchored', from: anchor, to: n, ...profile(anchor, n, ROW) },
  ];
};
update();

const layer = chart.addLayer({
  pane: 0,
  position: 'belowSeries',
  build(ctx) {
    const out = [];
    for (const r of ranges) {
      const x0 = ctx.timeToX(bars.time[r.from]), x1 = ctx.timeToX(bars.time[r.to - 1]);
      if (x0 === null || x1 === null) continue;
      const top = ctx.priceToY((r.lo + r.vol.length) * ROW), bottom = ctx.priceToY(r.lo * ROW);
      out.push({ type: 'rect', x: x0, y: top, width: x1 - x0, height: bottom - top, color: 'rgba(255, 122, 61, 0.06)' });
      out.push({ type: 'text', x: x0 + 4, y: top - 17, text: r.label, color: '#ff9a66', size: 11 });
      const max = r.vol[r.poc];
      r.vol.forEach((v, k) => {
        const y0 = ctx.priceToY((r.lo + k + 1) * ROW), y1 = ctx.priceToY((r.lo + k) * ROW);
        const color = k === r.poc ? '#ffb547' : k >= r.dn && k <= r.up ? 'rgba(255, 122, 61, 0.45)' : 'rgba(255, 122, 61, 0.18)';
        out.push({ type: 'rect', x: x0, y: y0, width: (v / max) * (x1 - x0) * 0.5, height: Math.max(1, y1 - y0 - 1), color });
      });
      const poc = ctx.priceToY((r.lo + r.poc + 0.5) * ROW);
      out.push({ type: 'line', x1: x0, y1: poc, x2: x1, y2: poc, color: '#ffb547', width: 1, dash: 'dashed' });
    }
    return out;
  },
});
chart.on('click', (e) => {
  const i = bars.time.indexOf(e.time);
  if (i >= 0 && i < n - 10) { anchor = i; update(); chart.invalidateLayer(layer); }
});
chart.setBarSpacing(2);
chart.scrollToRealtime();`,
      about: 'Two volume profiles drawn inside their own time ranges: one over a fixed stretch of bars and one anchored at a swing high and running to the latest bar. A layer plugin draws them; clicking a bar moves the anchor and `invalidateLayer` redraws.',
      api: ['addLayer', "chart.on('click')", 'invalidateLayer'],
    },
    {
      id: 'obi',
      title: 'Order-book imbalance',
      group: 'Order book',
      code: `${create()}
${lastMinutes(20, '5s')}
// One snapshot every 5 s, 30 levels of $4 a side (bids then asks, ascending).
const book = await fetch(\`/api/depth?\${range}&every=5000&levels=30&step=8\`).then((r) => r.json());
const ramp = (rgb) => [0.12, 0.3, 0.5, 0.75, 1].map((a) => \`rgba(\${rgb}, \${a})\`);
const bidHeavy = chart.addHeatmap({ source: 'depth', tickSize: 4, columnMs: 5_000, colors: ramp('46, 230, 182') });
const askHeavy = chart.addHeatmap({ source: 'depth', tickSize: 4, columnMs: 5_000, colors: ramp('255, 93, 115') });
const n = book.prices[0].length / 2;
const imbalance = [];
book.time.forEach((time, i) => {
  const p = book.prices[i], s = book.sizes[i];
  const bid = { time, prices: [], sizes: [] }, ask = { time, prices: [], sizes: [] };
  let bids = 0, asks = 0;
  // Compare the k-th bid with the k-th ask and paint the excess on the heavier side.
  for (let k = 0; k < n; k++) {
    const b = n - 1 - k, a = n + k, excess = s[b] - s[a];
    if (excess > 0) { bid.prices.unshift(p[b]); bid.sizes.unshift(excess); }
    else { ask.prices.push(p[a]); ask.sizes.push(-excess); }
    if (k < 10) { bids += s[b]; asks += s[a]; }
  }
  bidHeavy.appendDepth(bid);
  askHeavy.appendDepth(ask);
  imbalance.push((bids - asks) / (bids + asks)); // top 10 levels, -1 to 1
});
chart.addCandlestickSeries({ color: '#e8ecf4', downColor: '#8a93a6' }).setData(bars);
chart.addSeries('baseline', { baseValue: 0, color: '#2ee6b6', downColor: '#ff5d73', pricePrecision: 2 }, chart.addPane(0.35))
  .setData({ time: book.time, value: imbalance });
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Where the book leans: at each distance from the touch, the excess of bids over asks is painted green below the price and the excess of asks red above it, as two `addHeatmap` series. The pane shows the imbalance of the top 10 levels from -1 to 1.',
      api: ['addHeatmap({ colors })', 'Series.appendDepth', "addSeries('baseline')"],
    },
    {
      id: 'bookmap',
      title: 'Liquidity heatmap with trades',
      group: 'Order book',
      code: `${create()}
${lastMinutes(8, '5s')}
// One snapshot a second, 40 levels of $2 a side, and every trade.
const book = await fetch(\`/api/depth?\${range}&every=1000&levels=40&step=4\`).then((r) => r.json());
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
const heatmap = chart.addHeatmap({ source: 'depth', tickSize: 2, columnMs: 1_000 });
book.time.forEach((time, i) => heatmap.appendDepth({ time, prices: book.prices[i], sizes: book.sizes[i] }));
chart.addSeries('line', { color: '#e8ecf4', lineWidth: 1.5 }).setData({ time: bars.time, value: bars.close });

// Trades per 5 s and side: one bubble at the volume-weighted price.
const buckets = new Map();
trades.time.forEach((t, i) => {
  const time = Math.floor(t / 5_000) * 5_000, buy = trades.side[i] > 0;
  const key = time + (buy ? 1 : 0);
  const b = buckets.get(key) ?? { time, buy, size: 0, value: 0 };
  b.size += trades.size[i];
  b.value += trades.size[i] * trades.price[i];
  buckets.set(key, b);
});
const bubbles = [...buckets.values()];
// Walls: levels of the latest snapshot with over five times the median size.
const sizes = book.sizes.at(-1), median = [...sizes].sort((a, b) => a - b)[sizes.length >> 1];
const walls = book.prices.at(-1).map((price, i) => ({ price, size: sizes[i] })).filter((w) => w.size > 5 * median)
  .sort((a, b) => b.size - a.size);

chart.addLayer({
  pane: 0,
  build(ctx) {
    const out = [];
    for (const b of bubbles) {
      const x = ctx.timeToX(b.time), y = ctx.priceToY(b.value / b.size);
      if (x === null || y === null) continue;
      const color = b.buy ? 'rgba(46, 230, 182, 0.75)' : 'rgba(255, 93, 115, 0.75)';
      out.push({ type: 'icon', shape: 'circle', x, y, size: 4 + 7 * Math.sqrt(b.size), color });
    }
    const right = ctx.rect.x + ctx.rect.width - 6, used = [];
    for (const w of walls) {
      const y = ctx.priceToY(w.price);
      if (y === null || used.some((u) => Math.abs(u - y) < 16)) continue; // biggest first, no overlaps
      used.push(y);
      out.push({ type: 'text', x: right, y: y - 7, text: \`wall \${w.size.toFixed(0)} BTC\`, color: '#0b0d12', size: 10, align: 'right', background: '#ffee58', padding: 2 });
    }
    return out;
  },
});
chart.setBarSpacing(9);
chart.setRightOffset(9);`,
      about: 'Eight minutes of a one-second order-book heatmap with the last price, buy and sell volume as bubbles every five seconds, and the current walls labelled at the right. A layer plugin draws the bubbles and labels.',
      api: ['addHeatmap', 'addLayer', "icon: 'circle'"],
    },
    {
      id: 'tape-bubbles',
      title: 'Block trade bubbles',
      group: 'Trades',
      code: `${create()}
${lastMinutes(60)}
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
chart.addCandlestickSeries().setData(bars);

// Block trades: $100k notional or more.
const blocks = [];
trades.time.forEach((time, i) => {
  const notional = trades.price[i] * trades.size[i];
  if (notional >= 100_000) blocks.push({ time, price: trades.price[i], notional, buy: trades.side[i] > 0 });
});
const largest = [...blocks].sort((a, b) => b.notional - a.notional).slice(0, 5);

chart.addLayer({
  pane: 0,
  build(ctx) {
    const out = [];
    // A candle is centred on its open time; shift trades half a minute so it spans its minute.
    const at = (b) => [ctx.timeToX(b.time - 30_000), ctx.priceToY(b.price)];
    for (const b of blocks) {
      const [x, y] = at(b);
      if (x === null || y === null) continue;
      const color = b.buy ? 'rgba(46, 230, 182, 0.6)' : 'rgba(255, 93, 115, 0.6)';
      out.push({ type: 'icon', shape: 'circle', x, y, size: 5 + Math.sqrt(b.notional / 3_000), color });
    }
    for (const b of largest) {
      const [x, y] = at(b);
      if (x === null || y === null) continue;
      out.push({ type: 'text', x, y: y - 30, text: b.notional >= 1e6 ? \`$\${(b.notional / 1e6).toFixed(2)}M\` : \`$\${(b.notional / 1e3).toFixed(0)}k\`, color: '#e8ecf4', size: 10, align: 'center', background: 'rgba(11, 13, 18, 0.75)', padding: 2 });
    }
    return out;
  },
});
chart.setBarSpacing(16);
chart.scrollToRealtime();`,
      about: 'Every trade of $100k or more as a bubble on one-minute candles, sized by notional and colored by the aggressor side, with the five largest labelled. A layer plugin draws the bubbles from the raw trades.',
      api: ['addLayer', "icon: 'circle'", 'ctx.timeToX'],
    },
    {
      id: 'iceberg',
      title: 'Icebergs & large fills',
      group: 'Trades',
      code: `${create()}
${lastMinutes(20, '5s')}
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
// The order book once a second, 20 levels of $0.50 a side.
const book = await fetch(\`/api/depth?\${range}&every=1000&levels=20\`).then((r) => r.json());
chart.addCandlestickSeries().setData(bars);

// Size displayed at a price, in the snapshot of that second.
const shown = (time, price) => {
  const i = Math.floor((time - book.time[0]) / 1000), k = book.prices[i]?.indexOf(price) ?? -1;
  return k < 0 ? NaN : book.sizes[i][k];
};
// Runs of aggressive trades at one price and side, with gaps under 15 s.
const open = new Map(), runs = [];
trades.time.forEach((time, i) => {
  const price = trades.price[i], key = price * trades.side[i];
  let r = open.get(key);
  if (!r || time - r.end > 15_000) {
    r = { start: time, end: time, price, buy: trades.side[i] > 0, size: 0, shown: shown(time, price) };
    open.set(key, r);
    runs.push(r);
  }
  r.end = time;
  r.size += trades.size[i];
});
// Twice as much traded as was displayed: the order was bigger than it showed.
const icebergs = runs.filter((r) => r.size >= 2 && r.size > 2 * r.shown);

chart.addLayer({
  pane: 0,
  build(ctx) {
    const out = [], labels = [];
    for (const r of icebergs) {
      const x0 = ctx.timeToX(r.start - 2_500), x1 = ctx.timeToX(r.end - 2_500), y = ctx.priceToY(r.price);
      if (x0 === null || x1 === null || y === null) continue;
      const color = r.buy ? '#2ee6b6' : '#ff5d73';
      out.push({ type: 'line', x1: x0 - 14, y1: y, x2: x1 + 14, y2: y, color, width: 2 });
      out.push({ type: 'icon', shape: 'circle', x: x1, y, size: 6 + 2 * Math.sqrt(r.size), color });
      // A label unless it would overlap an earlier one.
      const text = \`\${r.size.toFixed(1)} BTC, \${(r.size / r.shown).toFixed(1)}x shown\`, w = ctx.textWidth(text, 10) + 4;
      const x = x1 + 12, ly = y - 22;
      if (labels.some((l) => x < l.x + l.w + 4 && l.x < x + w + 4 && Math.abs(l.y - ly) < 18)) continue;
      labels.push({ x, y: ly, w });
      out.push({ type: 'text', x, y: ly, text, color, size: 10, background: 'rgba(11, 13, 18, 0.85)', padding: 2 });
    }
    return out;
  },
});
chart.setBarSpacing(4.5);
chart.scrollToRealtime();`,
      about: 'Prices where aggressive trades took more than twice the size the order book displayed there, a sign of a hidden or refilled order. Trades are grouped into runs at one price and side and compared with one-second book snapshots; a layer plugin marks each run with its size.',
      api: ['addLayer', 'ctx.textWidth', 'ctx.priceToY'],
    },
    {
      id: 'spoof',
      title: 'Pulled-order patterns',
      group: 'Liquidity events',
      code: `${create()}
${lastMinutes(15, '5s')}
// One snapshot a second, 40 levels of $2 a side (bids then asks).
const book = await fetch(\`/api/depth?\${range}&every=1000&levels=40&step=4\`).then((r) => r.json());
const heatmap = chart.addHeatmap({ source: 'depth', tickSize: 2, columnMs: 1_000 });
book.time.forEach((time, i) => heatmap.appendDepth({ time, prices: book.prices[i], sizes: book.sizes[i] }));
chart.addSeries('line', { color: '#e8ecf4', lineWidth: 1.5 }).setData({ time: bars.time, value: bars.close });

// Large orders (14 times the snapshot's median) that appeared away from the price,
// stayed 10 to 40 s and were pulled before the price came within $10.
const alive = new Map(), pulled = [];
book.time.forEach((time, i) => {
  const p = book.prices[i], s = book.sizes[i], mid = (p[39] + p[40]) / 2;
  const median = [...s].sort((a, b) => a - b)[s.length >> 1];
  const seen = new Set();
  p.forEach((price, k) => {
    if (s[k] < 14 * median) return;
    seen.add(price);
    const o = alive.get(price) ?? { price, start: time, size: 0, fresh: i > 0 && price > book.prices[i - 1][0] };
    o.end = time;
    o.size = Math.max(o.size, s[k]);
    alive.set(price, o);
  });
  for (const [price, o] of alive) {
    if (seen.has(price)) continue;
    alive.delete(price);
    const life = (o.end - o.start) / 1000 + 1;
    if (o.fresh && price > p[0] && price < p.at(-1) && Math.abs(price - mid) > 10 && life >= 10 && life <= 40) pulled.push({ ...o, life });
  }
});

chart.addLayer({
  pane: 0,
  build(ctx) {
    const out = [], labels = [];
    for (const o of pulled) {
      const x0 = ctx.timeToX(o.start - 2_500), x1 = ctx.timeToX(o.end - 1_500);
      const y0 = ctx.priceToY(o.price + 3), y1 = ctx.priceToY(o.price - 3);
      if (x0 === null || x1 === null || y0 === null || y1 === null) continue;
      const box = [[x0 - 2, y0], [x1 + 2, y0], [x1 + 2, y1], [x0 - 2, y1], [x0 - 2, y0]];
      out.push({ type: 'polyline', points: box, color: '#ffffff', width: 1.5 });
      // A label unless it would overlap an earlier one.
      const text = \`\${o.size.toFixed(0)} BTC, \${o.life} s\`, w = ctx.textWidth(text, 10) + 4, ly = y0 - 18;
      const lx = Math.min(x0, ctx.rect.x + ctx.rect.width - w - 4); // keep it inside the plot
      if (labels.some((l) => lx < l.x + l.w + 4 && l.x < lx + w + 4 && Math.abs(l.y - ly) < 18)) continue;
      labels.push({ x: lx, y: ly, w });
      out.push({ type: 'text', x: lx, y: ly, text, color: '#ffffff', size: 10, background: 'rgba(11, 13, 18, 0.85)', padding: 2 });
    }
    return out;
  },
});
chart.setBarSpacing(5.5);
chart.scrollToRealtime();`,
      about: 'Large resting orders that appeared away from the price, sat for 10 to 40 seconds and vanished before trading, boxed on a one-second depth heatmap under the last price. Such quick placements and cancellations can be spoofing, though this simple screen also catches ordinary cancellations. A layer plugin draws the boxes.',
      api: ['addHeatmap', 'addLayer', "type: 'polyline'"],
    },
    {
      id: 'tradesize',
      title: 'Trade size distribution',
      group: 'Trades',
      code: `${create()}
${lastMinutes(120)}
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
chart.addCandlestickSeries().setData(bars);

// Volume per minute in four notional tiers.
const tiers = [5_000, 25_000, 100_000, Infinity];
const volume = tiers.map(() => new Array(bars.time.length).fill(0));
trades.time.forEach((t, i) => {
  const bar = Math.floor((t - from) / 60_000), notional = trades.price[i] * trades.size[i];
  volume[tiers.findIndex((max) => notional < max)][bar] += trades.size[i];
});
// Stacked: draw the running totals from the tallest down, so each tier sits on the one below.
const colors = ['#5a2a14', '#a3471c', '#ff7a3d', '#ffd0a8'];
const pane = chart.addPane(0.8);
const stack = volume.map((_, k) => bars.time.map((_, i) => volume.slice(0, k + 1).reduce((s, v) => s + v[i], 0)));
for (let k = tiers.length - 1; k >= 0; k--) {
  chart.addSeries('histogram', { color: colors[k], pricePrecision: 1, lastValueVisible: false, priceLineVisible: false }, pane)
    .setData({ time: bars.time, value: stack[k] });
}
${LEGEND}
legend(pane, [['under $5k', colors[0]], ['$5k to 25k', colors[1]], ['$25k to 100k', colors[2]], ['$100k and up', colors[3]]]);
chart.setBarSpacing(8);
chart.scrollToRealtime();`,
      about: 'BTC volume per minute split into four notional tiers, from small orders at the bottom to block trades on top, under one-minute candles. The stack is four `histogram` series of running totals, drawn tallest first.',
      api: ["addSeries('histogram')", 'addPane', 'addLayer'],
    },
    {
      id: 'orderevents',
      title: 'Adds, cancels, fills & OFI',
      group: 'Order book',
      code: `${create()}
${bars({ interval: '1m', count: 240 })}
const events = await fetch('/api/series/microBookEvents?symbol=BTCUSD&interval=1m&count=240').then((r) => r.json());
chart.addCandlestickSeries().setData(bars);

// Size added to the book above zero; cancelled and filled (the bar's volume) below it.
const book = chart.addPane(0.7);
chart.addSeries('histogram', { color: '#4cc9f0', pricePrecision: 0, lastValueVisible: false, priceLineVisible: false }, book)
  .setData({ time: events.time, value: events.added });
chart.addSeries('histogram', { color: '#8a93a6', lastValueVisible: false, priceLineVisible: false }, book)
  .setData({ time: events.time, value: events.cancelled.map((v, i) => -v - bars.volume[i]) });
chart.addSeries('histogram', { color: '#ffb547', lastValueVisible: false, priceLineVisible: false }, book)
  .setData({ time: bars.time, value: bars.volume.map((v) => -v) });
${LEGEND}
legend(book, [['added', '#4cc9f0'], ['cancelled', '#8a93a6'], ['filled', '#ffb547']]);

// Order flow imbalance: net pressure on the touch, colored by sign.
chart.addSeries('histogram', { priceLineVisible: false, pricePrecision: 1 }, chart.addPane(0.5)).setData({
  time: events.time,
  value: events.ofi,
  color: events.ofi.map((v) => (v >= 0 ? '#2ee6b6' : '#ff5d73')),
});
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Book activity per minute in BTC: size added by new orders above zero, cancelled and filled size stacked below it, and order flow imbalance (positive when bids build or asks are taken) in its own pane. Fills are the bar volume; the other columns come from the `microBookEvents` series.',
      api: ["addSeries('histogram')", 'per-point color', 'addPane'],
    },
    {
      id: 'bidask',
      title: 'Bid/ask band & spread',
      group: 'Order book',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '5s', count: 360 })}
const quote = await fetch('/api/series/microSpread?symbol=ETHUSD&interval=5s&count=360').then((r) => r.json());
// Best bid and ask around the mid, from the spread in basis points.
const bid = bars.close.map((mid, i) => mid * (1 - quote.spread[i] / 20_000));
const ask = bars.close.map((mid, i) => mid * (1 + quote.spread[i] / 20_000));
const askLine = chart.addSeries('line', { color: '#ff5d73', lineWidth: 1.5, step: true, pricePrecision: 2 });
const bidLine = chart.addSeries('line', { color: '#2ee6b6', lineWidth: 1.5, step: true, pricePrecision: 2 });
askLine.setData({ time: bars.time, value: ask });
bidLine.setData({ time: bars.time, value: bid });
chart.addBand(askLine, bidLine, { fill: 'rgba(255, 181, 71, 0.22)' });
chart.addSeries('area', {
  color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.3)', bottomFill: 'rgba(255, 181, 71, 0)', step: true, pricePrecision: 2,
}, chart.addPane(0.4)).setData({ time: quote.time, value: quote.spread });
chart.setBarSpacing(8);
chart.scrollToRealtime();`,
      about: 'The best ask (red) and bid (green) as step lines with the spread filled between them by `addBand`, and the spread itself in basis points below. Bid and ask are the mid price plus and minus half the quoted spread.',
      api: ['addBand', "addSeries('line', { step })", "addSeries('area')"],
    },
    {
      id: 'terrain',
      title: 'Liquidity terrain candles',
      group: 'Liquidity events',
      code: `${create()}
${lastMinutes(150)}
// A snapshot every 10 s, 20 levels of $5 a side.
const book = await fetch(\`/api/depth?\${range}&every=10000&levels=20&step=10\`).then((r) => r.json());
chart.addCandlestickSeries().setData(bars);

// Liquidity under each candle: BTC resting within $50 of the price, averaged over its snapshots.
const depth = bars.time.map(() => [0, 0]);
book.time.forEach((t, i) => {
  const bar = depth[Math.floor((t - from) / 60_000)];
  const mid = (book.prices[i][19] + book.prices[i][20]) / 2;
  bar[0] += book.sizes[i].reduce((s, v, k) => s + (Math.abs(book.prices[i][k] - mid) <= 50 ? v : 0), 0);
  bar[1] += 1;
});
const liquidity = depth.map(([sum, n]) => sum / n);
// Rank from 0 (thinnest) to 1 (deepest): the thinnest third amber, the deepest third blue.
const sorted = [...liquidity].sort((a, b) => a - b);
const rank = liquidity.map((v) => sorted.indexOf(v) / (sorted.length - 1));
const tint = (r, alpha) => (r < 1 / 3 ? \`rgba(255, 181, 71, \${alpha * (1 - 3 * r)})\` : r > 2 / 3 ? \`rgba(76, 201, 240, \${alpha * (3 * r - 2)})\` : null);

// The terrain: a column behind each candle, as strong as its liquidity is unusual.
chart.addLayer({
  pane: 0,
  position: 'belowSeries',
  build(ctx) {
    const out = [], { y, height } = ctx.rect;
    bars.time.forEach((t, i) => {
      const x0 = ctx.timeToX(t - 30_000), x1 = ctx.timeToX(t + 30_000), color = tint(rank[i], 0.35);
      if (x0 === null || x1 === null || !color || x1 < ctx.rect.x || x0 > ctx.rect.x + ctx.rect.width) return;
      out.push({ type: 'rect', x: x0, y, width: x1 - x0, height, color });
    });
    return out;
  },
});
chart.addSeries('histogram', { priceLineVisible: false, pricePrecision: 1 }, chart.addPane(0.35)).setData({
  time: bars.time,
  value: liquidity,
  color: rank.map((r) => (r < 1 / 3 ? '#ffb547' : r > 2 / 3 ? '#4cc9f0' : '#4a5160')),
});
chart.setBarSpacing(8);
chart.scrollToRealtime();`,
      about: 'Candles over a terrain of the liquidity that rested near the price while each one formed: amber columns where the book was thinnest, blue where it was deepest. The pane shows the depth within $50 in BTC, colored the same way; a layer plugin draws the columns from order-book snapshots.',
      api: ['addLayer', "position: 'belowSeries'", 'per-point color'],
    },
    {
      id: 'pullshock',
      title: 'Liquidity withdrawal shocks',
      group: 'Liquidity events',
      code: `${create()}
${lastMinutes(30, '5s')}
// One snapshot a second, 3 levels of $1 a side: the liquidity at the touch.
const book = await fetch(\`/api/depth?\${range}&every=1000&levels=3&step=2\`).then((r) => r.json());
chart.addCandlestickSeries().setData(bars);
const near = book.sizes.map((s) => s.reduce((a, b) => a + b, 0));

// Shocks: depth under half its 2-minute median. Spells less than 10 s apart are merged.
const threshold = [], shocks = [];
near.forEach((v, i) => {
  const window = near.slice(Math.max(0, i - 120), i + 1).sort((a, b) => a - b);
  const median = window[window.length >> 1];
  threshold.push(median / 2);
  if (i < 120 || v >= median / 2) return;
  const last = shocks.at(-1), t = book.time[i];
  if (last && t - last.end <= 10_000) { last.end = t; last.low = Math.min(last.low, v / median); }
  else shocks.push({ start: t, end: t, low: v / median });
});

// The depth pane, per 5 s candle: the lowest depth and the threshold.
const per5s = (a) => bars.time.map((_, i) => Math.min(...a.slice(5 * i, 5 * i + 5)));
const depthPane = chart.addPane(0.45);
chart.addSeries('area', { color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.3)', bottomFill: 'rgba(76, 201, 240, 0.02)', pricePrecision: 1, priceLineVisible: false }, depthPane)
  .setData({ time: bars.time, value: per5s(near) });
chart.addSeries('line', { color: '#ff5d73', lineWidth: 1, lineStyle: 'dashed', lastValueVisible: false, priceLineVisible: false }, depthPane)
  .setData({ time: bars.time, value: per5s(threshold) });

// Shade each shock in both panes; label it in the price pane.
const shade = (pane, labelled) => chart.addLayer({
  pane,
  position: 'belowSeries',
  build(ctx) {
    const out = [];
    let lastLabel = -Infinity;
    for (const s of shocks) {
      const x0 = ctx.timeToX(s.start - 2_500), x1 = ctx.timeToX(s.end + 2_500);
      if (x0 === null || x1 === null) continue;
      out.push({ type: 'rect', x: x0, y: ctx.rect.y, width: Math.max(3, x1 - x0), height: ctx.rect.height, color: 'rgba(255, 93, 115, 0.2)' });
      if (labelled && x0 - lastLabel > 60) {
        out.push({ type: 'text', x: x0 + 3, y: ctx.rect.y + 6, text: \`-\${Math.round((1 - s.low) * 100)}%\`, color: '#ff8a9a', size: 11 });
        lastLabel = x0;
      }
    }
    return out;
  },
});
shade(0, true);
shade(depthPane, false);
chart.setBarSpacing(2.8);
chart.scrollToRealtime();`,
      about: 'Moments when liquidity near the touch was pulled. The lower pane shows the depth within three levels of the touch in BTC against half its two-minute median (dashed); spans below it are shaded across both panes and labelled with the deepest drop. A layer plugin per pane draws the spans.',
      api: ['addLayer', "addSeries('area')", 'addPane'],
    },
    {
      id: 'depthrecovery',
      title: 'Depth recovery',
      group: 'Liquidity events',
      code: `${create()}
const to = Math.floor(Date.now() / 60_000) * 60_000;
// One snapshot a second for 30 minutes, 6 levels of $1 a side.
const book = await fetch(\`/api/depth?symbol=BTCUSD&from=\${to - 1_800_000}&to=\${to}&every=1000&levels=6&step=2\`).then((r) => r.json());
// BTC resting within $6 of the touch, averaged over 10 s.
const time = [], depth = [];
for (let i = 0; i + 10 <= book.time.length; i += 10) {
  time.push(book.time[i]);
  depth.push(book.sizes.slice(i, i + 10).flat().reduce((a, b) => a + b, 0) / 10);
}
const typical = [...depth].sort((a, b) => a - b)[depth.length >> 1];

const line = chart.addSeries('baseline', {
  baseValue: typical, color: '#4cc9f0', downColor: '#ff5d73', lineWidth: 1.5,
  topFill: 'rgba(76, 201, 240, 0.15)', bottomFill: 'rgba(255, 93, 115, 0.35)', pricePrecision: 1,
});
line.setData({ time, value: depth });
line.createPriceLine({ price: typical, color: '#8a93a6', lineStyle: 'dashed', title: 'median' });

// Shocks: depth under 70% of the median. Each is labelled with its drop and the time
// until depth was back at the median.
const markers = [];
for (let i = 1; i < depth.length; i++) {
  if (depth[i] >= 0.7 * typical || depth[i - 1] < 0.7 * typical) continue;
  let j = i;
  while (j < depth.length && depth[j] < typical) j++;
  const drop = Math.round((1 - Math.min(...depth.slice(i, j)) / typical) * 100);
  const back = j < depth.length ? \`back in \${(j - i) * 10} s\` : 'recovering';
  markers.push({ time: time[i], position: 'belowBar', shape: 'arrowUp', color: '#ff5d73', text: \`-\${drop}%, \${back}\` });
}
line.setMarkers(markers);
chart.setBarSpacing(5.5);
chart.scrollToRealtime();`,
      about: 'Resting size near the touch in ten-second averages, against its median. Each shock where depth falls under 70% of the median gets a marker with the drop and the seconds until it was back; the baseline series shades the shortfall in red.',
      api: ["addSeries('baseline')", 'Series.setMarkers', 'createPriceLine'],
    },
    ...plotDemos.orderflow,
  ],
};
