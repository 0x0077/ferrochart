import { bars, create, load } from './build';
import { plotDemos } from './plots';
import type { Category } from './types';

const UP = '#2ee6b6';
const DOWN = '#ff5d73';
const ACCENT = '#5aa9ff';

/** Quarterly fundamentals of the illustrative large-cap stock. */
const Q = { symbol: 'ACME', interval: '1q', count: 24 };
const D = { symbol: 'ACME', interval: '1d' };

/** A small layer plugin that writes a color-keyed legend in a pane's top-left corner. */
function legend(items: [string, string][], pane = 0): string {
  const list = items.map(([t, c]) => `['${t}', '${c}']`).join(', ');
  return `chart.addLayer({ pane: ${pane}, build: (ctx) => [${list}]
  .map(([text, color], i) => ({ type: 'text', x: ctx.rect.x + 10, y: ctx.rect.y + 14 + i * 17, text, color, size: 12 })) });`;
}

/** Quarterly charts show every quarter. */
const allQuarters = (name: string) => `chart.setVisibleLogicalRange(-0.7, ${name}.time.length - 0.3);`;

export const equities: Category = {
  id: 'equities',
  title: 'Equities & fundamentals',
  kicker: '10',
  blurb: 'Earnings, estimates, valuation bands and corporate actions on the price chart.',
  accent: ACCENT,
  demos: [
    // ---------------------------------------------------------------- Earnings
    {
      id: 'financials',
      title: 'Revenue · profit · free cash flow',
      group: 'Earnings',
      code: `${create()}
${load('fin', 'series/eqFinancials', Q)}
const bn = (v) => v / 1e9; // USD billions
// Net income bars drawn over the revenue bars, free cash flow as a line
chart.addSeries('histogram', { color: 'rgba(90, 169, 255, 0.3)', pricePrecision: 1 }).setData({ time: fin.time, value: fin.revenue.map(bn) });
chart.addSeries('histogram', { color: '${ACCENT}', pricePrecision: 1 }).setData({ time: fin.time, value: fin.netIncome.map(bn) });
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 1 }).setData({ time: fin.time, value: fin.fcf.map(bn) });
${legend([['Revenue, $bn', 'rgba(140, 195, 255, 0.85)'], ['Net income', ACCENT], ['Free cash flow', '#ffb547']])}

// Margins: percent of revenue
const margin = (col) => col.map((v, i) => (v / fin.revenue[i]) * 100);
const pane = chart.addPane(0.6);
chart.addSeries('line', { color: '${ACCENT}', lineWidth: 2, pricePrecision: 1 }, pane).setData({ time: fin.time, value: margin(fin.netIncome) });
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 1 }, pane).setData({ time: fin.time, value: margin(fin.fcf) });
${legend([['Net margin %', ACCENT], ['FCF margin %', '#ffb547']], 1)}
${allQuarters('fin')}`,
      about: 'Six years of quarterly results on quarterly timestamps: revenue with net income drawn over it as two `histogram` series, free cash flow as a line, and both margins in a pane below. A small layer plugin writes the legends.',
      api: ["addSeries('histogram')", 'addPane', 'addLayer'],
    },
    {
      id: 'surprise',
      title: 'EPS: actual vs estimate',
      group: 'Earnings',
      code: `${create()}
${load('eps', 'series/eqEps', Q)}
const actual = eps.estimate.map((e, i) => e * (1 + eps.surprise[i]));
const beat = eps.surprise.map((s) => s >= 0);
chart.addSeries('line', { color: '#8b93a7', lineStyle: 'dashed', lineWidth: 1 }).setData({ time: eps.time, value: eps.estimate });
chart.addSeries('points', { color: '#8b93a7', lineWidth: 6 }).setData({ time: eps.time, value: eps.estimate });
// Beats and misses as two point series (NaN leaves a point out)
chart.addSeries('points', { color: '${UP}', lineWidth: 10 }).setData({ time: eps.time, value: actual.map((v, i) => (beat[i] ? v : NaN)) });
chart.addSeries('points', { color: '${DOWN}', lineWidth: 10 }).setData({ time: eps.time, value: actual.map((v, i) => (beat[i] ? NaN : v)) });
${legend([['Consensus estimate', '#8b93a7'], ['Actual: beat', UP], ['Actual: miss', DOWN]])}
chart.applyPriceScaleOptions({ margins: { top: 0.2, bottom: 0.08 } });

// Surprise in percent of the estimate
chart.addSeries('histogram', { pricePrecision: 1 }, chart.addPane(0.6)).setData({
  time: eps.time, value: eps.surprise.map((s) => s * 100), color: beat.map((b) => (b ? '${UP}' : '${DOWN}')),
});
${allQuarters('eps')}`,
      about: 'Reported EPS as two `points` series (sized with `lineWidth`), beats and misses, against the consensus estimate (dashed), with the surprise in percent as a `histogram` below.',
      api: ["addSeries('points')", "addSeries('histogram')", 'per-point color'],
    },
    {
      id: 'margins',
      title: 'Margin trends',
      group: 'Earnings',
      code: `${create()}
${load('m', 'series/eqMargins', Q)}
const pct = (col) => col.map((v) => v * 100);
const lines = [['gross', '${ACCENT}'], ['operating', '#ffb547'], ['net', '${UP}']];
for (const [key, color] of lines) {
  chart.addSeries('line', { color, lineWidth: 2, pricePrecision: 1 }).setData({ time: m.time, value: pct(m[key]) });
  chart.addSeries('points', { color, lineWidth: 7, lastValueVisible: false, priceLineVisible: false }).setData({ time: m.time, value: pct(m[key]) });
}
${legend([['Gross margin %', ACCENT], ['Operating margin %', '#ffb547'], ['Net margin %', UP]])}
chart.applyPriceScaleOptions({ margins: { top: 0.12, bottom: 0.08 } });
${allQuarters('m')}`,
      about: 'Gross, operating and net margin per quarter, each a `line` with `points` on the reported quarters, on one percent scale.',
      api: ["addSeries('line')", "addSeries('points')", 'applyPriceScaleOptions'],
    },
    {
      id: 'guidance',
      title: 'Guidance vs actual',
      group: 'Earnings',
      code: `${create()}
${load('g', 'series/eqGuidance', { ...Q, count: 20 })}
const bn = (v) => v / 1e9;
const low = g.mid.map((v, i) => bn(v * (1 - g.width[i])));
const high = g.mid.map((v, i) => bn(v * (1 + g.width[i])));
const actual = g.actual.map(bn);
actual[actual.length - 1] = NaN; // the current quarter is guided, not yet reported

const style = { color: 'rgba(90, 169, 255, 0.6)', lineWidth: 1, lastValueVisible: false, priceLineVisible: false, pricePrecision: 1 };
const top = chart.addSeries('line', style);
const bottom = chart.addSeries('line', style);
top.setData({ time: g.time, value: high });
bottom.setData({ time: g.time, value: low });
chart.addBand(top, bottom, { fill: 'rgba(90, 169, 255, 0.18)' });

const reported = chart.addSeries('line', { color: '#e6e9ef', lineWidth: 2, pricePrecision: 1 });
reported.setData({ time: g.time, value: actual });
reported.setMarkers(g.time.flatMap((time, i) => {
  const vs = actual[i] > high[i] ? actual[i] / high[i] - 1 : actual[i] < low[i] ? actual[i] / low[i] - 1 : 0;
  if (!vs) return [];
  return [{ time, position: vs > 0 ? 'aboveBar' : 'belowBar', shape: vs > 0 ? 'arrowUp' : 'arrowDown',
    color: vs > 0 ? '${UP}' : '${DOWN}', text: (vs > 0 ? '+' : '') + (vs * 100).toFixed(1) + '%' }];
}));
${legend([['Revenue, $bn', '#e6e9ef'], ['Guided range', ACCENT]])}
${allQuarters('g')}`,
      about: 'Quarterly revenue against the range guided a quarter earlier, filled with `addBand`. Markers give the miss or beat versus the edge of the range; the current quarter shows the guidance only.',
      api: ['addBand', 'setMarkers', "addSeries('line')"],
    },
    {
      id: 'earnreplay',
      title: 'Earnings reaction replay',
      group: 'Earnings',
      code: `${create()}
${bars({ symbol: 'NOVA', interval: '1d', count: 500 })}
${load('reports', 'events/eqEarnings', { symbol: 'NOVA', interval: '1d', count: 500 })}
const markers = reports.time.map((time, i) => ({
  time, position: reports.value[i] >= 0 ? 'belowBar' : 'aboveBar', shape: 'square',
  color: reports.value[i] >= 0 ? '${UP}' : '${DOWN}',
  text: reports.label[i] + ' ' + (reports.value[i] * 100).toFixed(0) + '%',
}));

// Start 120 sessions back and play the rest, one day every 150 ms.
const head = (n) => Object.fromEntries(Object.entries(bars).map(([k, v]) => [k, v.slice(0, n)]));
const candles = chart.addCandlestickSeries();
let next = bars.time.length - 120;
candles.setData(head(next));
candles.setMarkers(markers.filter((m) => m.time < bars.time[next]));
chart.addVolumeSeries(candles);
chart.setBarSpacing(7);
chart.scrollToRealtime();

const timer = setInterval(() => {
  if (next >= bars.time.length) return clearInterval(timer);
  const i = next++;
  candles.update({ time: bars.time[i], open: bars.open[i], high: bars.high[i], low: bars.low[i], close: bars.close[i], volume: bars.volume[i] });
  if (markers.some((m) => m.time === bars.time[i])) candles.setMarkers(markers.filter((m) => m.time <= bars.time[i]));
}, 150);
const stop = () => clearInterval(timer); // pause; call before chart.destroy()`,
      about: 'Daily candles played back with `update`, the way a host app drives a replay; each report marker (beat or miss, with the EPS surprise) appears when the replay reaches its day, so the reaction unfolds bar by bar.',
      api: ['Series.update', 'setMarkers', 'addVolumeSeries'],
    },

    // ---------------------------------------------------------------- Estimates & valuation
    {
      id: 'revisions',
      title: 'Estimate revisions',
      group: 'Estimates & valuation',
      code: `${create()}
${bars({ symbol: 'ACME', interval: '1w', count: 160 })}
${load('rev', 'series/eqRevisions', { symbol: 'ACME', interval: '1w', count: 160 })}
chart.addCandlestickSeries().setData(bars);
chart.addSeries('line', { color: '${ACCENT}', lineWidth: 2, step: true }, chart.addPane(0.6))
  .setData({ time: rev.time, value: rev.consensus });
${legend([['Next-year EPS consensus, $', ACCENT]], 1)}
// Raises up, cuts down: analysts per week
const pane = chart.addPane(0.6);
chart.addSeries('histogram', { color: '${UP}', pricePrecision: 0 }, pane).setData({ time: rev.time, value: rev.raises.map(Math.round) });
chart.addSeries('histogram', { color: '${DOWN}', pricePrecision: 0 }, pane).setData({ time: rev.time, value: rev.cuts.map((v) => -Math.round(v)) });
${legend([['Estimate raises', UP], ['Estimate cuts', DOWN]], 2)}
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0.05 } }, 2);
chart.setBarSpacing(7);
chart.scrollToRealtime();`,
      about: 'Weekly candles over the next-year EPS consensus as a step line, and the number of analysts raising (up) and cutting (down) their estimate each week.',
      api: ["addSeries('line')", 'step lines', "addSeries('histogram')"],
    },
    {
      id: 'valband',
      title: 'Valuation percentile bands',
      group: 'Estimates & valuation',
      code: `${create()}
${bars({ symbol: 'ACME', interval: '1w', count: 260 })}
${load('eps', 'series/eqValuation', { symbol: 'ACME', interval: '1w', count: 260 })}
chart.addCandlestickSeries().setData(bars);

// Trailing P/E, and its percentiles over the five years
const pe = bars.close.map((c, i) => c / eps.epsTtm[i]);
const sorted = [...pe].sort((a, b) => a - b);
const pctl = (p) => sorted[Math.round(p * (sorted.length - 1))];
const pane = chart.addPane(0.9);
const flat = (v, color, lineStyle = 'solid') => {
  const s = chart.addSeries('line', { color, lineStyle, lineWidth: 1, lastValueVisible: false, priceLineVisible: false }, pane);
  s.setData({ time: bars.time, value: bars.time.map(() => v) });
  return s;
};
chart.addBand(flat(pctl(1), 'transparent'), flat(pctl(0.8), 'rgba(255, 93, 115, 0.5)'), { fill: 'rgba(255, 93, 115, 0.12)' });
chart.addBand(flat(pctl(0.2), 'rgba(46, 230, 182, 0.5)'), flat(pctl(0), 'transparent'), { fill: 'rgba(46, 230, 182, 0.12)' });
flat(pctl(0.5), '#8b93a7', 'dashed');
chart.addSeries('line', { color: '#e6e9ef', lineWidth: 2, pricePrecision: 1 }, pane).setData({ time: bars.time, value: pe });
${legend([['P/E (trailing)', '#e6e9ef'], ['Top 20% of history', DOWN], ['Median', '#8b93a7'], ['Bottom 20%', UP]], 1)}
chart.applyPriceScaleOptions({ margins: { top: 0.32, bottom: 0.05 } }, pane);
chart.setVisibleLogicalRange(-1, bars.time.length + 1);`,
      about: 'Five years of weekly trailing P/E (price over trailing EPS) against its own history: `addBand` shades the most expensive and cheapest 20% of readings, with the median dashed. The P/E and its percentiles are computed in a few lines.',
      api: ['addBand', 'addPane', "addSeries('line')"],
    },
    {
      id: 'targets',
      title: 'Analyst price targets',
      group: 'Estimates & valuation',
      code: `${create()}
${bars({ ...D, count: 320 })}
${load('t', 'series/eqTargets', { ...D, count: 320 })}
const opts = (color, lineStyle) => ({ color, lineStyle, lineWidth: 1, lastValueVisible: false, priceLineVisible: false });
const high = chart.addSeries('line', opts('rgba(46, 230, 182, 0.7)', 'solid'));
const low = chart.addSeries('line', opts('rgba(255, 93, 115, 0.7)', 'solid'));
high.setData({ time: t.time, value: t.high });
low.setData({ time: t.time, value: t.low });
chart.addBand(high, low, { fill: 'rgba(90, 169, 255, 0.08)' });
chart.addSeries('line', opts('${ACCENT}', 'dotted')).setData({ time: t.time, value: t.mean });

const candles = chart.addCandlestickSeries();
candles.setData(bars);
// Today's targets on the price axis
const last = t.time.length - 1;
candles.createPriceLine({ price: t.high[last], color: '${UP}', lineStyle: 'dashed', title: 'High target' });
candles.createPriceLine({ price: t.mean[last], color: '${ACCENT}', lineStyle: 'dashed', title: 'Mean target' });
candles.createPriceLine({ price: t.low[last], color: '${DOWN}', lineStyle: 'dashed', title: 'Low target' });
chart.setBarSpacing(5);
chart.scrollToRealtime();`,
      about: 'The history of the high, mean and low 12-month targets around daily candles, with the high–low range filled by `addBand`, and today\'s three targets as labelled price lines.',
      api: ['createPriceLine', 'addBand', "addSeries('line')"],
    },
    {
      id: 'range52',
      title: '52-week range position',
      group: 'Estimates & valuation',
      code: `${create()}
${bars({ ...D, count: 760 })}
// Rolling 52-week (252 sessions) high, low and where the close sits between them
const N = 252, hi = [], lo = [], pos = [];
bars.time.forEach((_, i) => {
  const h = i < N - 1 ? NaN : Math.max(...bars.high.slice(i - N + 1, i + 1));
  const l = i < N - 1 ? NaN : Math.min(...bars.low.slice(i - N + 1, i + 1));
  hi.push(h); lo.push(l); pos.push(((bars.close[i] - l) / (h - l)) * 100);
});
const opts = (color) => ({ color, lineStyle: 'dotted', lineWidth: 1, step: true, lastValueVisible: false, priceLineVisible: false });
chart.addSeries('line', opts('${UP}')).setData({ time: bars.time, value: hi });
chart.addSeries('line', opts('${DOWN}')).setData({ time: bars.time, value: lo });
const candles = chart.addCandlestickSeries();
candles.setData(bars);
candles.createPriceLine({ price: hi.at(-1), color: '${UP}', lineStyle: 'dashed', title: '52w high' });
candles.createPriceLine({ price: lo.at(-1), color: '${DOWN}', lineStyle: 'dashed', title: '52w low' });

chart.addSeries('baseline', { baseValue: 50, color: '${UP}', downColor: '${DOWN}', pricePrecision: 0 }, chart.addPane(0.5))
  .setData({ time: bars.time, value: pos });
chart.applyPriceScaleOptions({ range: { low: 0, high: 100 } }, 1);
chart.setVisibleLogicalRange(bars.time.length - 500, bars.time.length + 2);`,
      about: 'Rolling 52-week high and low as dotted step lines on daily candles, today\'s levels as price lines, and the close\'s position in the range (0–100%) as a `baseline` series around the midpoint.',
      api: ['createPriceLine', "addSeries('baseline')", 'applyPriceScaleOptions'],
    },
    {
      id: 'shortint',
      title: 'Short interest · borrow fee',
      group: 'Estimates & valuation',
      code: `${create()}
${bars({ symbol: 'NOVA', interval: '1d', count: 500 })}
${load('si', 'series/eqShortInterest', { symbol: 'NOVA', interval: '1d', count: 500 })}
chart.addCandlestickSeries().setData(bars);
chart.addSeries('area', { color: '#b388ff', topFill: 'rgba(179, 136, 255, 0.3)', bottomFill: 'rgba(179, 136, 255, 0)', pricePrecision: 1 }, chart.addPane(0.6))
  .setData({ time: si.time, value: si.shortFloat.map((v) => v * 100) });
${legend([['Short interest, % of float', '#b388ff']], 1)}
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 2 }, chart.addPane(0.6))
  .setData({ time: si.time, value: si.borrowFee.map((v) => v * 100) });
${legend([['Borrow fee, % a year', '#ffb547']], 2)}
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'A growth stock with short interest as a percent of free float and the annualised borrow fee in their own panes: both climb into sell-offs and ease in rallies.',
      api: ["addSeries('area')", 'addPane'],
    },

    // ---------------------------------------------------------------- Capital returns
    {
      id: 'dividendsbuyback',
      title: 'Dividends, buybacks & splits',
      group: 'Capital returns',
      code: `${create()}
${bars({ ...D, count: 700 })}
${load('div', 'events/eqDividends', { ...D, count: 700 })}
${load('buyback', 'events/eqBuybackAnnouncements', { ...D, count: 700 })}
${load('split', 'events/eqSplits', { ...D, count: 700 })}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
candles.setMarkers([
  ...div.time.map((time, i) => ({ time, position: 'belowBar', shape: 'circle', color: '${UP}', text: '$' + div.value[i].toFixed(2) })),
  ...buyback.time.map((time, i) => ({ time, position: 'aboveBar', shape: 'arrowDown', color: '${ACCENT}', text: '$' + (buyback.value[i] / 1e9).toFixed(0) + 'B buyback' })),
  ...split.time.map((time, i) => ({ time, position: 'aboveBar', shape: 'square', color: '#ffb547', text: split.value[i] + ':1 split' })),
].sort((a, b) => a.time - b.time));
chart.setBarSpacing(3.5);
chart.scrollToRealtime();
chart.setRightOffset(14);`,
      about: 'Corporate actions from three event feeds as markers on split-adjusted daily candles: quarterly dividends with the amount per share, buyback authorisations and a stock split.',
      api: ['setMarkers', 'addCandlestickSeries'],
    },
    {
      id: 'dividend',
      title: 'Dividend history & coverage',
      group: 'Capital returns',
      code: `${create()}
${load('dv', 'series/eqDividend', Q)}
chart.addSeries('histogram', { color: '${UP}', pricePrecision: 2 }).setData({ time: dv.time, value: dv.dps });
${legend([['Dividend per share, $', UP]])}
// Coverage: earnings per share / dividend per share
const cover = chart.addSeries('baseline', { baseValue: 2, color: '${ACCENT}', downColor: '#ffb547', pricePrecision: 2 }, chart.addPane(0.7));
cover.setData({ time: dv.time, value: dv.coverage });
cover.createPriceLine({ price: 2, color: '#8b93a7', lineStyle: 'dotted', title: '2x covered' });
${legend([['Earnings coverage, x', ACCENT]], 1)}
chart.applyPriceScaleOptions({ margins: { top: 0.15, bottom: 0 } });
${allQuarters('dv')}`,
      about: 'Quarterly dividend per share as a `histogram`, and its earnings coverage below as a `baseline` series around 2x with a price line at the threshold.',
      api: ["addSeries('histogram')", "addSeries('baseline')", 'createPriceLine'],
    },
    {
      id: 'buyback',
      title: 'Buybacks & share count',
      group: 'Capital returns',
      code: `${create()}
${load('bb', 'series/eqBuyback', Q)}
chart.addSeries('histogram', { color: '${ACCENT}', pricePrecision: 1 }).setData({ time: bb.time, value: bb.repurchased.map((v) => v / 1e9) });
${legend([['Shares repurchased, $bn', ACCENT]])}
chart.applyPriceScaleOptions({ margins: { top: 0.15, bottom: 0 } });
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, step: true, pricePrecision: 3 }, chart.addPane(0.8))
  .setData({ time: bb.time, value: bb.shares.map((v) => v / 1e9) });
${legend([['Shares outstanding, bn', '#ffb547']], 1)}
${allQuarters('bb')}`,
      about: 'Money spent on repurchases each quarter as a `histogram` and the shares outstanding as a step line: the share count shrinks as the buybacks retire stock.',
      api: ["addSeries('histogram')", 'step lines', 'addPane'],
    },
    {
      id: 'insider',
      title: 'Insider trades',
      group: 'Capital returns',
      code: `${create()}
${bars({ ...D, count: 260 })}
${load('ins', 'events/eqInsider', { ...D, count: 260 })}
const usdM = ins.value.map((v) => v / 1e6);
const candles = chart.addCandlestickSeries();
candles.setData(bars);
candles.setMarkers(ins.time.map((time, i) => usdM[i] >= 0
  ? { time, position: 'belowBar', shape: 'arrowUp', color: '${UP}', text: 'B ' + usdM[i].toFixed(1) }
  : { time, position: 'aboveBar', shape: 'arrowDown', color: '${DOWN}', text: 'S ' + (-usdM[i]).toFixed(1) }));
chart.addSeries('histogram', { pricePrecision: 1 }, chart.addPane(0.5)).setData({
  time: ins.time, value: usdM, color: usdM.map((v) => (v >= 0 ? '${UP}' : '${DOWN}')),
});
${legend([['Insider net trades, $M', '#8b93a7']], 1)}
chart.setBarSpacing(6);
chart.scrollToRealtime();`,
      about: 'Insider open-market buys (B) and sells (S) as markers on daily candles, labelled in USD millions, with the signed value of each trade as a `histogram` on the same time axis.',
      api: ['setMarkers', "addSeries('histogram')", 'per-point color'],
    },

    // ---------------------------------------------------------------- Quality & breadth
    {
      id: 'segments',
      title: 'Revenue by segment',
      group: 'Quality & breadth',
      code: `${create()}
${load('seg', 'series/eqSegments', Q)}
const keys = ['cloud', 'software', 'devices', 'services'];
const colors = ['${ACCENT}', '#4cc9f0', '#b388ff', '#ffb547'];
// Cumulative sums, USD billions: the top of each segment's stack
let top = seg.time.map(() => 0);
const tops = keys.map((k) => (top = top.map((v, i) => v + seg[k][i] / 1e9)));
// Tallest first, so each segment covers the part of the stack below it
for (let k = keys.length - 1; k >= 0; k--) {
  chart.addSeries('histogram', { color: colors[k], pricePrecision: 1 }).setData({ time: seg.time, value: tops[k] });
}
${legend([['Services', '#ffb547'], ['Devices', '#b388ff'], ['Software', '#4cc9f0'], ['Cloud', ACCENT]])}
chart.applyPriceScaleOptions({ margins: { top: 0.22, bottom: 0 } });
${allQuarters('seg')}`,
      about: 'Quarterly revenue of four business lines as a stacked bar chart: the example sums the segments and draws each running total as a `histogram`, tallest first.',
      api: ["addSeries('histogram')", 'applyPriceScaleOptions'],
    },
    {
      id: 'accrual',
      title: 'Earnings quality',
      group: 'Quality & breadth',
      code: `${create()}
${load('q', 'series/eqQuality', Q)}
const bn = (v) => v / 1e9;
const cfo = chart.addSeries('line', { color: '${UP}', lineWidth: 2, pricePrecision: 2 });
const ni = chart.addSeries('line', { color: '#e6e9ef', lineWidth: 2, pricePrecision: 2 });
cfo.setData({ time: q.time, value: q.cfo.map(bn) });
ni.setData({ time: q.time, value: q.netIncome.map(bn) });
// Green where cash flow covers earnings, red where earnings run ahead of cash
chart.addBand(cfo, ni, { fill: 'rgba(46, 230, 182, 0.18)', fillCrossed: 'rgba(255, 93, 115, 0.3)' });
${legend([['Operating cash flow, $bn', UP], ['Net income, $bn', '#e6e9ef']])}

// Accrual ratio: (net income - operating cash flow) / total assets
const accrual = q.netIncome.map((v, i) => ((v - q.cfo[i]) / q.assets[i]) * 100);
chart.addSeries('histogram', { pricePrecision: 2 }, chart.addPane(0.6)).setData({
  time: q.time, value: accrual, color: accrual.map((v) => (v > 0 ? '${DOWN}' : '${UP}')),
});
${legend([['Accrual ratio, % of assets', '#8b93a7']], 1)}
${allQuarters('q')}`,
      about: 'Net income against operating cash flow with `addBand` filling the gap, green where cash covers earnings and red (`fillCrossed`) where it does not, and the accrual ratio as a `histogram`.',
      api: ['addBand', 'fillCrossed', "addSeries('histogram')"],
    },
    {
      id: 'breadth',
      title: 'Market breadth',
      group: 'Quality & breadth',
      code: `${create()}
${bars({ symbol: 'ES', interval: '1d', count: 500 })}
${load('br', 'series/eqBreadth', { symbol: 'ES', interval: '1d', count: 500 })}
chart.addCandlestickSeries().setData(bars);
chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.5)).setData({
  time: br.time, value: br.net, color: br.net.map((v) => (v >= 0 ? '${UP}' : '${DOWN}')),
});
${legend([['Advancers - decliners', '#8b93a7']], 1)}
// Advance-decline line: the running sum of net advances
let sum = 0;
chart.addSeries('line', { color: '${ACCENT}', lineWidth: 2, pricePrecision: 0 }, chart.addPane(0.6))
  .setData({ time: br.time, value: br.net.map((v) => (sum += v)) });
${legend([['Advance-decline line', ACCENT]], 2)}
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Index futures over daily net advancing issues as a `histogram` and the advance–decline line, their running sum. Divergences between the A/D line and the index show a narrowing rally.',
      api: ["addSeries('histogram')", "addSeries('line')", 'addPane'],
    },
    ...plotDemos.equities,
  ],
};
