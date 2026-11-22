import { create, load } from './build';
import type { Category } from './types';

const FOUR_H = { symbol: 'BTCUSD', interval: '4h', count: 360 };

export const prediction: Category = {
  id: 'prediction',
  title: 'Prediction markets',
  kicker: '18',
  blurb: 'Outcome probabilities, their consistency and how they resolved.',
  accent: '#facc15',
  demos: [
    {
      id: 'probline',
      title: 'Probability with news',
      group: 'Probabilities',
      code: `${create()}
${load('market', 'series/predYes', FOUR_H)}
${load('news', 'events/predNews', FOUR_H)}
const yes = chart.addSeries('line', { color: '#facc15', lineWidth: 2, pricePrecision: 0 });
yes.setData({ time: market.time, value: market.yes.map((p) => p * 100) }); // % chance of Yes
yes.createPriceLine({ price: 50, color: '#8b93a7', lineStyle: 'dotted', axisLabelVisible: false });
yes.setMarkers(news.time.map((time, i) => (news.value[i] >= 0
  ? { time, position: 'aboveBar', shape: 'arrowUp', color: '#2ee6b6', text: news.label[i] }
  : { time, position: 'belowBar', shape: 'arrowDown', color: '#ff5d73', text: news.label[i] })));
chart.applyPriceScaleOptions({ range: { low: 0, high: 100 }, margins: { top: 0.02, bottom: 0.02 } });
chart.setBarSpacing(2.8);
chart.scrollToRealtime();`,
      about: 'Two months of a binary market’s Yes price as a percent chance, on a fixed 0–100 scale, with the news that moved it marked by `setMarkers`.',
      api: ['setMarkers', 'applyPriceScaleOptions({ range })'],
    },
    {
      id: 'outcomes',
      title: 'Multi-outcome market',
      group: 'Probabilities',
      code: `${create()}
${load('market', 'series/predOutcomes', { symbol: 'BTCUSD', interval: '4h', count: 540 })}
const outcomes = [
  { key: 'a', name: 'A', color: '#facc15', fill: '#5c4d14' },
  { key: 'b', name: 'B', color: '#4cc9f0', fill: '#1d4b5c' },
  { key: 'c', name: 'C', color: '#b388ff', fill: '#3d3262' },
  { key: 'd', name: 'D', color: '#2ee6b6', fill: '#164f43' },
  { key: 'e', name: 'E', color: '#ff5d73', fill: '#5a2a33' },
];
// Stacked areas: each layer is the running total up to its outcome, in percent. The
// tallest goes first, so the lower layers, with opaque fills, cover it.
const last = market.time.length - 1;
for (let k = outcomes.length - 1; k >= 0; k--) {
  const below = outcomes.slice(0, k + 1);
  const value = market.time.map((t, i) => below.reduce((sum, o) => sum + market[o.key][i], 0) * 100);
  const { key, name, color, fill } = outcomes[k];
  const layer = chart.addSeries('area', { color, topFill: fill, bottomFill: fill, lineWidth: 1, pricePrecision: 0, lastValueVisible: false, priceLineVisible: false });
  layer.setData({ time: market.time, value });
  layer.setMarkers([{ time: market.time[last], position: 'belowBar', shape: 'square', color, text: \`\${name} \${Math.round(market[key][last] * 100)}%\` }]);
}
chart.applyPriceScaleOptions({ range: { low: 0, high: 100 }, margins: { top: 0, bottom: 0 } });
chart.setVisibleLogicalRange(0, last + 20); // room on the right for the labels`,
      about: 'Three months of a five-outcome market. Each band is one outcome’s probability; together they fill 0–100%. The stack is five `area` series of running totals, with the latest odds as markers.',
      api: ["addSeries('area')", 'stacking', 'setMarkers'],
    },
    {
      id: 'marketconstraints',
      title: 'Cross-market consistency',
      group: 'Probabilities',
      code: `${create()}
${load('m', 'series/predLinked', FOUR_H)}
// Three markets on mutually exclusive outcomes: their Yes prices should add up to 100%.
const pct = (xs) => xs.map((p) => p * 100);
chart.addSeries('line', { color: '#facc15', lineWidth: 2 }).setData({ time: m.time, value: pct(m.a) });
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2 }).setData({ time: m.time, value: pct(m.b) });
chart.addSeries('line', { color: '#b388ff', lineWidth: 2 }).setData({ time: m.time, value: pct(m.c) });
const sum = chart.addSeries('baseline', { baseValue: 100, color: '#ff5d73', downColor: '#2ee6b6', lineWidth: 2 }, chart.addPane(0.7));
sum.setData({ time: m.time, value: m.a.map((a, i) => (a + m.b[i] + m.c[i]) * 100) });
sum.createPriceLine({ price: 100, color: '#8b93a7', lineStyle: 'dashed', title: 'Consistent' });
chart.setBarSpacing(2.8);
chart.scrollToRealtime();`,
      about: 'Yes prices of three markets on mutually exclusive outcomes (A amber, B blue, C violet) and, below, their sum against 100%. Above it the set is overpriced (sell all three), below it underpriced (buy all three).',
      api: ["addSeries('baseline')", 'createPriceLine', 'addPane'],
    },
    {
      id: 'resolutionpath',
      title: 'Resolution timeline',
      group: 'Resolution',
      code: `${create()}
${load('market', 'series/predDispute', { symbol: 'BTCUSD', interval: '1h', count: 240 })}
const time = market.time, n = time.length, hour = 3_600_000;
// The market closed 84 hours ago; a proposal was disputed and went to a 48-hour vote.
const phases = [
  { from: 0, to: n - 84, label: 'Trading', fill: null },
  { from: n - 84, to: n - 82, label: 'Proposed: Yes', fill: 'rgba(76, 201, 240, 0.16)' },
  { from: n - 82, to: n - 80, label: 'Disputed', fill: 'rgba(255, 93, 115, 0.18)' },
  { from: n - 80, to: n - 32, label: 'Vote', fill: 'rgba(179, 136, 255, 0.14)' },
  { from: n - 32, to: n, label: 'Resolved: Yes', fill: 'rgba(46, 230, 182, 0.12)' },
].map((p) => ({ ...p, start: time[p.from], end: p.to < n ? time[p.to] : time[n - 1] + hour }));

// Prices until resolution; settled shares no longer trade.
const resolved = phases[4].start;
const yes = chart.addSeries('line', { color: '#facc15', lineWidth: 2, pricePrecision: 0 });
yes.setData({ time, value: market.yes.map((p, i) => (time[i] < resolved ? p * 100 : NaN)) });
chart.applyPriceScaleOptions({ range: { low: 0, high: 100 }, margins: { top: 0.02, bottom: 0.02 } });

chart.addLayer({
  pane: chart.mainPane,
  position: 'background',
  build(ctx) {
    const out = [];
    const top = ctx.rect.y, bottom = ctx.rect.y + ctx.rect.height, right = ctx.rect.x + ctx.rect.width;
    phases.forEach((p, row) => {
      const x1 = ctx.timeToX(p.start), x2 = ctx.timeToX(p.end);
      if (x1 === null || x2 === null || x2 < ctx.rect.x || x1 > right) return;
      if (p.fill) out.push({ type: 'rect', x: x1, y: top, width: x2 - x1, height: bottom - top, color: p.fill });
      out.push({ type: 'line', x1, y1: top, x2: x1, y2: bottom, color: 'rgba(139, 147, 167, 0.5)', width: 1, dash: 'dotted' });
      // One label row per phase, so that the short phases don't overlap.
      const y = top + 8 + (row % 4) * 20;
      out.push({ type: 'text', x: Math.max(x1, ctx.rect.x) + 6, y, text: p.label, color: '#d7dce5', size: 11, background: 'rgba(11, 13, 18, 0.75)', padding: 3 });
    });
    return out;
  },
});
chart.setBarSpacing(4.2);
chart.scrollToRealtime();`,
      about: 'The last ten days of a market: trading, then a proposed outcome, a dispute, a 48-hour vote and the payout. A layer plugin shades each phase and labels it; the Yes price stops at resolution, when Yes shares pay 100.',
      api: ['addLayer', 'ctx.timeToX', 'NaN gaps'],
    },
    {
      id: 'forecastrevision',
      title: 'Forecast revisions',
      group: 'Resolution',
      code: `${create()}
${load('fc', 'series/predForecast', { symbol: 'BTCUSD', interval: '1w', count: 30 })}
const n = fc.time.length;
// Uncertainty narrows as the vote approaches: ±(1 + 0.45 · √weeks left) points.
const width = fc.time.map((t, i) => 1 + 0.45 * Math.sqrt(n - 1 - i));
const style = { color: 'rgba(250, 204, 21, 0.4)', lineWidth: 1, lastValueVisible: false, priceLineVisible: false };
const upper = chart.addSeries('line', style);
const lower = chart.addSeries('line', style);
upper.setData({ time: fc.time, value: fc.forecast.map((v, i) => v + width[i]) });
lower.setData({ time: fc.time, value: fc.forecast.map((v, i) => v - width[i]) });
chart.addBand(upper, lower, { fill: 'rgba(250, 204, 21, 0.1)' });
const forecast = chart.addSeries('line', { color: '#facc15', lineWidth: 2, step: true });
forecast.setData({ time: fc.time, value: fc.forecast }); // vote share, %
forecast.createPriceLine({ price: fc.result[n - 1], color: '#2ee6b6', lineWidth: 2, title: 'Result' });
chart.setBarSpacing(32);
chart.scrollToRealtime();`,
      about: 'Thirty weekly revisions of a published vote-share forecast (step line) with its uncertainty band, narrowing as the vote nears, and the final result as a price line.',
      api: ['step lines', 'addBand', 'createPriceLine'],
    },
  ],
};
