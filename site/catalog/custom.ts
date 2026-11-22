import { bars, create, load, view } from './build';
import type { Category } from './types';

const VENUES = `const venues = { exA: 'Exchange A', exB: 'Exchange B', exC: 'Exchange C', exD: 'Exchange D' };
const colors = { exA: '#22d3ee', exB: '#b388ff', exC: '#ffb547', exD: '#ff5d73' };`;

/** A legend drawn by a small layer in the top-left corner of the main pane. */
const LEGEND = (entries: string) => `// A legend in the top-left corner.
chart.addLayer({
  pane: chart.mainPane,
  position: 'overlay',
  build(ctx) {
    const items = ${entries};
    const width = items.reduce((w, [label]) => w + 30 + ctx.textWidth(label, 11), 0);
    let x = ctx.rect.x + 12;
    return [{ type: 'rect', x: x - 6, y: ctx.rect.y + 6, width: width - 8, height: 22, color: '#0b0d12d0' }, ...items.flatMap(([label, color]) => {
      const cmds = [
        { type: 'rect', x, y: ctx.rect.y + 12, width: 10, height: 10, color },
        { type: 'text', x: x + 15, y: ctx.rect.y + 17, text: label, color: '#c9cfdb', size: 11, align: 'left' },
      ];
      x += 30 + ctx.textWidth(label, 11);
      return cmds;
    })];
  },
});`;

export const custom: Category = {
  id: 'custom',
  title: 'Custom series & plugins',
  kicker: '19',
  blurb: 'Stacked, grouped, box and brushed series: layer plugins on the same GPU frame.',
  accent: '#22d3ee',
  demos: [
    {
      id: 'stackedarea',
      title: '100% stacked area',
      group: 'Stacked & grouped',
      code: `${create()}
${load('mix', 'series/assetMix', { symbol: 'BTCUSD', interval: '1d', count: 365 })}
const keys = ['btc', 'eth', 'sol', 'stables', 'other'];
const names = { btc: 'BTC', eth: 'ETH', sol: 'SOL', stables: 'Stablecoins', other: 'Other' };
const colors = { btc: '#22d3ee', eth: '#b388ff', sol: '#2ee6b6', stables: '#ffb547', other: '#8b93a7' };
// Running totals in percent; the tallest goes first so the others paint over it.
let sum = mix.time.map(() => 0);
const tops = keys.map((k) => (sum = sum.map((s, i) => s + mix[k][i] * 100)));
keys.map((k, j) => [k, tops[j]]).reverse().forEach(([k, value]) => {
  chart.addSeries('area', { color: colors[k], topFill: colors[k], bottomFill: colors[k], lineWidth: 1, lastValueVisible: false, priceLineVisible: false })
    .setData({ time: mix.time, value });
});
chart.applyPriceScaleOptions({ range: { low: 0, high: 100 }, margins: { top: 0.02, bottom: 0 } });
${LEGEND('keys.map((k) => [names[k], colors[k]])')}
chart.fitContent();`,
      about: 'Portfolio weights stacked to 100% over a year. The cumulative sums are computed in JS and drawn as opaque area series, tallest first, so each band shows one asset\'s share.',
      api: ["addSeries('area')", 'applyPriceScaleOptions({ range })'],
    },
    {
      id: 'stackedbars',
      title: 'Stacked columns',
      group: 'Stacked & grouped',
      code: `${create()}
${load('vol', 'series/venueVolume', { symbol: 'BTCUSD', interval: '1d', count: 120 })}
${VENUES}
const keys = Object.keys(venues);
// Running totals in billions of USD; the tallest goes first so the others paint over it.
let sum = vol.time.map(() => 0);
const tops = keys.map((k) => (sum = sum.map((s, i) => s + vol[k][i] / 1e9)));
keys.map((k, j) => [k, tops[j]]).reverse().forEach(([k, value]) => {
  chart.addSeries('histogram', { color: colors[k], pricePrecision: 1, lastValueVisible: false, priceLineVisible: false })
    .setData({ time: vol.time, value });
});
${LEGEND('keys.map((k) => [venues[k], colors[k]])')}
${view(9)}`,
      about: 'Daily spot volume by venue (USD billions) stacked into one column per day. Running totals are drawn as histogram series from the tallest down, each painting over the one before.',
      api: ["addSeries('histogram')", 'addLayer'],
    },
    {
      id: 'groupedbars',
      title: 'Grouped columns',
      group: 'Stacked & grouped',
      code: `${create()}
${load('vol', 'series/venueVolume', { symbol: 'BTCUSD', interval: '1w', count: 30 })}
${VENUES}
const keys = Object.keys(venues);
const bn = (k) => vol[k].map((v) => v / 1e9); // USD billions
// An invisible histogram of each week's largest value scales the axis and feeds the crosshair.
chart.addSeries('histogram', { color: 'transparent', pricePrecision: 0, lastValueVisible: false, priceLineVisible: false })
  .setData({ time: vol.time, value: vol.time.map((_, i) => Math.max(...keys.map((k) => bn(k)[i]))) });
const values = Object.fromEntries(keys.map((k) => [k, bn(k)]));
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const slot = ctx.timeToX(vol.time[1]) - ctx.timeToX(vol.time[0]);
    const w = (slot * 0.8) / keys.length;
    const y0 = ctx.priceToY(0);
    const out = [];
    vol.time.forEach((t, i) => {
      const x = ctx.timeToX(t);
      if (x === null || x < ctx.rect.x - slot || x > ctx.rect.x + ctx.rect.width + slot) return;
      keys.forEach((k, j) => {
        const y = ctx.priceToY(values[k][i]);
        out.push({ type: 'rect', x: x - slot * 0.4 + j * w + 1, y, width: w - 2, height: y0 - y, color: colors[k] });
      });
    });
    return out;
  },
});
${LEGEND('keys.map((k) => [venues[k], colors[k]])')}
chart.applyPriceScaleOptions({ margins: { top: 0.12, bottom: 0 } });
${view(34)}`,
      about: 'Weekly volume per venue as side-by-side columns in each week\'s slot. A layer plugin offsets the four columns inside the slot; an invisible histogram keeps the price scale and crosshair.',
      api: ['addLayer', "color: 'transparent'"],
    },
    {
      id: 'dualhist',
      title: 'Buy vs sell volume',
      group: 'Histograms',
      code: `${create()}
const to = Math.floor(Date.now() / 60_000) * 60_000; // the last whole minute
const from = to - 120 * 60_000;
const range = \`symbol=BTCUSD&from=\${from}&to=\${to}\`;
const bars = await fetch(\`/api/bars?interval=1m&\${range}\`).then((r) => r.json());
const trades = await fetch(\`/api/trades?\${range}\`).then((r) => r.json());
// Aggressive buy and sell volume per minute.
const buy = bars.time.map(() => 0), sell = bars.time.map(() => 0);
trades.time.forEach((t, i) => {
  const m = Math.floor((t - from) / 60_000);
  if (m < 0 || m >= bars.time.length) return;
  if (trades.side[i] > 0) buy[m] += trades.size[i];
  else sell[m] += trades.size[i];
});
chart.addCandlestickSeries().setData(bars);
const pane = chart.addPane(0.8);
const opts = { pricePrecision: 1, lastValueVisible: false, priceLineVisible: false };
chart.addSeries('histogram', { ...opts, color: '#2ee6b6aa' }, pane).setData({ time: bars.time, value: buy });
chart.addSeries('histogram', { ...opts, color: '#ff5d73aa' }, pane).setData({ time: bars.time, value: sell.map((v) => -v) });
chart.addSeries('line', { ...opts, color: '#ffb547', lineWidth: 2 }, pane).setData({ time: bars.time, value: buy.map((b, i) => b - sell[i]) });
${view(9)}`,
      about: 'Under one-minute candles: aggressive buy volume up, sell volume down (BTC) and their difference as an amber line, aggregated from the trade feed in a few lines of JS. Two histogram series share a pane around zero.',
      api: ["addSeries('histogram')", 'addPane', '/api/trades'],
    },
    {
      id: 'prettyhist',
      title: 'Rounded gradient columns',
      group: 'Histograms',
      code: `${create()}
${load('vol', 'series/venueVolume', { symbol: 'BTCUSD', interval: '1d', count: 120 })}
const value = vol.exA.map((v) => v / 1e9); // Exchange A, USD billions
// An invisible histogram scales the axis and feeds the crosshair; the layer draws the columns.
chart.addSeries('histogram', { color: 'transparent', pricePrecision: 1, lastValueVisible: false, priceLineVisible: false })
  .setData({ time: vol.time, value });
// Colors from the foot (sky) to the top (violet) of a column.
const mix = (f) => '#' + [[0x4c, 0xb3], [0xc9, 0x88], [0xf0, 0xff]].map(([a, b]) => Math.round(a + (b - a) * f).toString(16).padStart(2, '0')).join('');
const slices = 12;
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const slot = ctx.timeToX(vol.time[1]) - ctx.timeToX(vol.time[0]);
    const w = Math.max(2, slot * 0.66), r = w / 2;
    const y0 = ctx.priceToY(0);
    const out = [];
    vol.time.forEach((t, i) => {
      const x = ctx.timeToX(t);
      if (x === null || x < ctx.rect.x - slot || x > ctx.rect.x + ctx.rect.width + slot) return;
      const y = ctx.priceToY(value[i]);
      // Gradient: horizontal slices from foot to top.
      const h = (y0 - y - r) / slices;
      for (let s = 0; s < slices; s++) {
        const ys = y0 - (s + 1) * h;
        out.push({ type: 'rect', x: x - w / 2, y: ys, width: w, height: h + 0.5, color: mix((s + 1) / slices) });
      }
      // Rounded cap.
      const cap = [];
      for (let a = 0; a <= 12; a++) cap.push([x - r * Math.cos((a * Math.PI) / 12), y + r - r * Math.sin((a * Math.PI) / 12)]);
      out.push({ type: 'polygon', points: cap, color: mix(1) });
    });
    return out;
  },
});
chart.applyPriceScaleOptions({ margins: { top: 0.08, bottom: 0 } });
${view(12)}`,
      about: 'One venue\'s daily volume (USD billions) as rounded columns, each shaded from sky at the foot to violet at the top. A layer plugin draws each column as gradient slices with a polygon cap over an invisible histogram.',
      api: ['addLayer', "PluginCommand 'polygon'"],
    },
    {
      id: 'lollipop',
      title: 'Lollipop',
      group: 'Histograms',
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '1d', count: 150 })}
// Daily returns in percent: a stick per day and a dot on its tip.
const value = bars.close.map((c, i) => (i === 0 ? 0 : (c / bars.close[i - 1] - 1) * 100));
const opts = { pricePrecision: 2, lastValueVisible: false, priceLineVisible: false };
chart.addSeries('histogram', opts).setData({ time: bars.time, value, color: value.map((v) => (v >= 0 ? '#2ee6b670' : '#ff5d7370')) });
// One points series per color (NaN leaves a gap).
chart.addSeries('points', { ...opts, color: '#2ee6b6', lineWidth: 8 }).setData({ time: bars.time, value: value.map((v) => (v >= 0 ? v : NaN)) });
chart.addSeries('points', { ...opts, color: '#ff5d73', lineWidth: 8 }).setData({ time: bars.time, value: value.map((v) => (v < 0 ? v : NaN)) });
${view(6)}`,
      about: 'Daily returns as lollipops: a histogram stick from zero with a dot on its tip, colored by sign. The dots are two points series, one per color, with `NaN` gaps.',
      api: ["addSeries('histogram')", "addSeries('points')"],
    },
    {
      id: 'scatter',
      title: 'Trade scatter',
      group: 'Points & distributions',
      code: `${create()}
${load('trades', 'trades', { symbol: 'BTCUSD', span: 240_000 })}
// A points series per size tier and side; each needs strictly increasing times.
const tiers = [{ min: 0, size: 3, alpha: '80' }, { min: 0.1, size: 6, alpha: 'b0' }, { min: 0.5, size: 11, alpha: 'e0' }];
for (const [side, color] of [[1, '#2ee6b6'], [-1, '#ff5d73']]) {
  tiers.forEach((tier, k) => {
    const max = tiers[k + 1]?.min ?? Infinity;
    const time = [], value = [];
    trades.time.forEach((t, i) => {
      const s = trades.size[i];
      if (trades.side[i] !== side || s < tier.min || s >= max || t <= (time[time.length - 1] ?? -1)) return;
      time.push(t);
      value.push(trades.price[i]);
    });
    chart.addSeries('points', { color: color + tier.alpha, lineWidth: tier.size, lastValueVisible: false, priceLineVisible: false }).setData({ time, value });
  });
}
chart.fitContent();`,
      about: 'Four minutes of trades as points at their price, green for aggressive buys and red for sells, with bigger dots for bigger trades. Six points series, one per side and size tier, on a shared trade-by-trade axis.',
      api: ["addSeries('points')", 'lineWidth', '/api/trades'],
    },
    {
      id: 'boxwhisker',
      title: 'Box & whisker',
      group: 'Points & distributions',
      code: `${create()}
${bars({ symbol: 'SOLUSD', interval: '1h', count: 24 * 60 })}
// Quartiles of each day's hourly closes.
const DAY = 86_400_000, days = new Map();
bars.time.forEach((t, i) => {
  const d = Math.floor(t / DAY) * DAY;
  if (!days.has(d)) days.set(d, []);
  days.get(d).push(bars.close[i]);
});
const q = (s, f) => s[Math.round((s.length - 1) * f)];
const stats = [...days].map(([time, v]) => {
  const s = v.sort((a, b) => a - b);
  return { time, min: s[0], q1: q(s, 0.25), med: q(s, 0.5), q3: q(s, 0.75), max: s[s.length - 1] };
});
const col = (k) => stats.map((s) => s[k]);
// Invisible candles span each day's range for the scale and crosshair.
chart.addCandlestickSeries({ color: 'transparent', downColor: 'transparent', lastValueVisible: false, priceLineVisible: false })
  .setData({ time: col('time'), open: col('q1'), high: col('max'), low: col('min'), close: col('q3') });
chart.addLayer({
  pane: chart.mainPane,
  build(ctx) {
    const slot = ctx.timeToX(stats[1].time) - ctx.timeToX(stats[0].time);
    const w = slot * 0.6;
    const out = [];
    for (const s of stats) {
      const x = ctx.timeToX(s.time);
      if (x === null || x < ctx.rect.x - slot || x > ctx.rect.x + ctx.rect.width + slot) continue;
      const [ymin, y1, ym, y3, ymax] = [s.min, s.q1, s.med, s.q3, s.max].map((p) => ctx.priceToY(p));
      out.push(
        { type: 'line', x1: x, y1: ymax, x2: x, y2: y3, color: '#8b93a7' },
        { type: 'line', x1: x, y1: y1, x2: x, y2: ymin, color: '#8b93a7' },
        { type: 'line', x1: x - w / 4, y1: ymax, x2: x + w / 4, y2: ymax, color: '#8b93a7' },
        { type: 'line', x1: x - w / 4, y1: ymin, x2: x + w / 4, y2: ymin, color: '#8b93a7' },
        { type: 'rect', x: x - w / 2, y: y3, width: w, height: Math.max(1, y1 - y3), color: '#22d3ee40' },
        { type: 'polyline', points: [[x - w / 2, y3], [x + w / 2, y3], [x + w / 2, y1], [x - w / 2, y1], [x - w / 2, y3]], color: '#22d3ee' },
        { type: 'line', x1: x - w / 2, y1: ym, x2: x + w / 2, y2: ym, color: '#ffb547', width: 2 },
      );
    }
    return out;
  },
});
${view(18)}`,
      about: 'The spread of each day\'s hourly closes: box from the first to the third quartile, amber median, whiskers to the extremes. Quartiles are computed in JS; a layer plugin draws the boxes over invisible candles that span each day.',
      api: ['addLayer', "color: 'transparent'"],
    },
    {
      id: 'brushable',
      title: 'Brushed range',
      group: 'Interactive',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '1h', count: 720 })}
chart.addSeries('area', { color: '#22d3ee', topFill: 'rgba(34, 211, 238, 0.25)', bottomFill: 'rgba(34, 211, 238, 0)', lineWidth: 2 })
  .setData({ time: bars.time, value: bars.close });
// The brush runs from the bar under the pointer to the latest one.
const n = bars.time.length;
let from = n - 120;
const layer = chart.addLayer({
  pane: chart.mainPane,
  position: 'belowSeries',
  build(ctx) {
    const x1 = ctx.timeToX(bars.time[from]), x2 = ctx.timeToX(bars.time[n - 1]);
    const y1 = ctx.priceToY(bars.close[from]), y2 = ctx.priceToY(bars.close[n - 1]);
    if (x1 === null || x2 === null || y1 === null || y2 === null) return [];
    const change = (bars.close[n - 1] / bars.close[from] - 1) * 100;
    const color = change >= 0 ? '#2ee6b6' : '#ff5d73';
    const text = \`\${change >= 0 ? '+' : ''}\${change.toFixed(2)}% · \${n - 1 - from} bars\`;
    return [
      { type: 'rect', x: x1, y: ctx.rect.y, width: x2 - x1, height: ctx.rect.height, color: color + '1a' },
      { type: 'line', x1, y1: ctx.rect.y, x2: x1, y2: ctx.rect.y + ctx.rect.height, color, dash: 'dashed' },
      { type: 'line', x1, y1, x2, y2: y1, color: color + '90', dash: 'dotted' },
      { type: 'line', x1: x2, y1, x2, y2, color, width: 2 },
      { type: 'text', x: (x1 + x2) / 2, y: ctx.rect.y + 18, text, color, size: 13, align: 'center', background: '#0b0d12d0', padding: 6 },
    ];
  },
});
chart.on('crosshairMove', ({ point }) => {
  if (point?.time == null) return;
  const i = bars.time.findIndex((t) => t >= point.time);
  if (i >= 0 && i < n - 1 && i !== from) { from = i; chart.invalidateLayer(layer); }
});
${view(4)}`,
      about: 'Move the pointer to brush a range from there to the latest bar: the shaded region, the rise or fall and the percent change follow it. A layer plugin draws the brush; `crosshairMove` updates it and `invalidateLayer` redraws.',
      api: ['addLayer', "on('crosshairMove')", 'invalidateLayer'],
    },
    {
      id: 'streamgraph',
      title: 'Streamgraph',
      group: 'Stacked & grouped',
      code: `${create()}
${load('posts', 'series/streamTopics', { symbol: 'BTCUSD', interval: '1d', count: 240 })}
const keys = ['layer1', 'defi', 'ai', 'gaming', 'memes'];
const names = { layer1: 'Layer 1', defi: 'DeFi', ai: 'AI', gaming: 'Gaming', memes: 'Memes' };
const colors = { layer1: '#22d3ee', defi: '#b388ff', ai: '#2ee6b6', gaming: '#ffb547', memes: '#ff5d73' };
// Stack the topics (thousands of posts) around zero: the first edge sits at minus half the total.
const k = (v) => v / 1e3;
const total = posts.time.map((_, i) => keys.reduce((s, key) => s + k(posts[key][i]), 0));
let edge = total.map((t) => -t / 2);
const edges = [edge, ...keys.map((key) => (edge = edge.map((e, i) => e + k(posts[key][i]))))];
const lines = edges.map((value) => {
  const s = chart.addSeries('line', { color: 'transparent', lastValueVisible: false, priceLineVisible: false, pricePrecision: 0 });
  s.setData({ time: posts.time, value });
  return s;
});
keys.forEach((key, j) => chart.addBand(lines[j + 1], lines[j], { fill: colors[key] + 'd0' }));
${LEGEND('keys.map((k) => [names[k], colors[k]])')}
chart.fitContent();`,
      about: 'Social mentions by topic stacked symmetrically around zero, so the stream swells and thins with the total. The edges are cumulative sums from minus half the total, drawn as invisible lines with an `addBand` fill between each pair.',
      api: ['addBand', "addSeries('line')"],
    },
    {
      id: 'gantt',
      title: 'Session timeline',
      group: 'Interactive',
      code: `${create()}
${bars({ symbol: 'BTCUSD', interval: '30m', count: 240 })}
chart.addSeries('line', { color: '#22d3ee', lineWidth: 2 }).setData({ time: bars.time, value: bars.close });
// Exchange sessions in UTC hours (start, end), one row each.
const rows = [
  ['Sydney', 23, 5], ['Tokyo', 0, 6], ['Hong Kong', 1.5, 8], ['Mumbai', 3.75, 10],
  ['Frankfurt', 7, 15.5], ['London', 8, 16.5], ['New York', 14.5, 21],
];
// A lower pane with a fixed 0..rows scale: a transparent series gives it the time axis.
const pane = chart.addPane(0.9);
chart.addSeries('line', { color: 'transparent', pricePrecision: 0, lastValueVisible: false, priceLineVisible: false }, pane)
  .setData({ time: bars.time, value: bars.time.map(() => 0) });
chart.applyPriceScaleOptions({ range: { low: -0.6, high: rows.length - 0.4 } }, pane);
const H = 3_600_000, DAY = 24 * H;
const colors = ['#4cc9f0', '#4cc9f0', '#4cc9f0', '#4cc9f0', '#b388ff', '#b388ff', '#ffb547'];
chart.addLayer({
  pane,
  build(ctx) {
    const left = ctx.xToTime(ctx.rect.x), right = ctx.xToTime(ctx.rect.x + ctx.rect.width);
    if (left === null || right === null) return [];
    const out = [];
    rows.forEach(([name, from, to], r) => {
      const y = ctx.priceToY(rows.length - 1 - r), h = 14;
      for (let day = Math.floor(left / DAY) * DAY - DAY; day < right; day += DAY) {
        const t1 = day + from * H, t2 = day + (to > from ? to : to + 24) * H;
        const x1 = ctx.timeToX(t1), x2 = ctx.timeToX(t2);
        if (x1 === null || x2 === null) continue;
        out.push({ type: 'rect', x: x1, y: y - h / 2, width: x2 - x1, height: h, color: colors[r] + 'b0' });
      }
      out.push({ type: 'text', x: ctx.rect.x + 8, y, text: name, color: '#e6e9ef', size: 11, align: 'left', background: '#0b0d12c0', padding: 3 });
    });
    return out;
  },
});
${view(6)}`,
      about: 'Thirty-minute BTC prices above a timeline of exchange sessions, one row per venue in UTC: Asia-Pacific in sky, Europe in violet, the US in amber. A layer plugin draws the bars in a pane whose scale is fixed to the row numbers.',
      api: ['addLayer', 'addPane', 'applyPriceScaleOptions({ range })'],
    },
  ],
};
