import { bars, create, load } from './build';
import { plotDemos } from './plots';
import type { Category } from './types';

const UP = '#2ee6b6';
const DOWN = '#ff5d73';
const ACCENT = '#34d399';

/** An index fund on the illustrative stock-index futures, daily. */
const D = { symbol: 'ES', interval: '1d' };

/** A small layer plugin that writes a color-keyed legend in a pane's top-left corner. */
function legend(items: [string, string][], pane = 0): string {
  const list = items.map(([t, c]) => `['${t}', '${c}']`).join(', ');
  return `chart.addLayer({ pane: ${pane}, build: (ctx) => [${list}]
  .map(([text, color], i) => ({ type: 'text', x: ctx.rect.x + 10, y: ctx.rect.y + 14 + i * 17, text, color, size: 12 })) });`;
}

export const funds: Category = {
  id: 'funds',
  title: 'Funds & ETFs',
  kicker: '15',
  blurb: 'Premium to NAV, tracking, flows, fees and the managers behind the returns.',
  accent: ACCENT,
  demos: [
    // ---------------------------------------------------------------- Pricing & tracking
    {
      id: 'premdisc',
      title: 'Premium / discount to NAV',
      group: 'Pricing & tracking',
      code: `${create()}
${load('f', 'series/fundCefPrice', { ...D, count: 250 })}
const price = f.nav.map((v, i) => v * (1 + f.premium[i])); // market price
chart.addSeries('line', { color: '#8b93a7', lineWidth: 2 }).setData({ time: f.time, value: f.nav });
chart.addSeries('line', { color: '${ACCENT}', lineWidth: 2 }).setData({ time: f.time, value: price });
${legend([['Net asset value', '#8b93a7'], ['Market price', ACCENT]])}
// Premium (+) or discount (-) to NAV, percent
chart.addSeries('baseline', { baseValue: 0, color: '${UP}', downColor: '${DOWN}', pricePrecision: 2 }, chart.addPane(0.6))
  .setData({ time: f.time, value: f.premium.map((v) => v * 100) });
${legend([['Premium / discount, %', '#e6e9ef']], 1)}
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'A listed fund\'s market price against its net asset value, and the premium or discount of the price to NAV as a `baseline` series around zero: positive in green, negative in red.',
      api: ["addSeries('baseline')", "addSeries('line')", 'addPane'],
    },
    {
      id: 'trackerr',
      title: 'Tracking difference & error',
      group: 'Pricing & tracking',
      code: `${create()}
${load('fund', 'series/fundTracker', { ...D, count: 380 })}
chart.addSeries('line', { color: '#8b93a7', lineWidth: 2 }).setData({ time: fund.time, value: fund.index });
chart.addSeries('line', { color: '${ACCENT}', lineWidth: 1 }).setData({ time: fund.time, value: fund.nav });
chart.applyPriceScaleOptions({ mode: 'indexedTo100' });
${legend([['Index', '#8b93a7'], ['Fund NAV', ACCENT]])}

// Daily tracking difference: fund return minus index return, basis points
const ret = (s) => s.map((v, i) => (i ? v / s[i - 1] - 1 : NaN));
const fr = ret(fund.nav), ir = ret(fund.index);
const td = fr.map((r, i) => (r - ir[i]) * 1e4);
chart.addSeries('histogram', { pricePrecision: 1 }, chart.addPane(0.5)).setData({
  time: fund.time, value: td, color: td.map((v) => (v >= 0 ? '${UP}' : '${DOWN}')),
});
${legend([['Tracking difference, bp', '#8b93a7']], 1)}

// Tracking error: annualised standard deviation of the difference over 60 days, percent
const te = td.map((_, i) => {
  if (i < 60) return NaN;
  const w = td.slice(i - 59, i + 1), m = w.reduce((a, b) => a + b, 0) / 60;
  return (Math.sqrt(w.reduce((a, b) => a + (b - m) ** 2, 0) / 59) * Math.sqrt(252)) / 100;
});
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 2 }, chart.addPane(0.5)).setData({ time: fund.time, value: te });
${legend([['Tracking error, 60d, % a year', '#ffb547']], 2)}
chart.setVisibleLogicalRange(60, fund.time.length + 1);`,
      about: 'An index fund against its index rebased to 100 (`indexedTo100`), the daily tracking difference in basis points and the rolling 60-day tracking error, computed in a few lines.',
      api: ['applyPriceScaleOptions', "addSeries('histogram')", 'addPane'],
    },
    {
      id: 'feedrag',
      title: 'Fee drag',
      group: 'Pricing & tracking',
      code: `${create()}
${load('tr', 'series/fundTotalReturn', { ...D, count: 2520 })}
// Growth of 10,000 in the total-return index, before fees and after annual fees taken daily
const years = (i) => (tr.time[i] - tr.time[0]) / (365.25 * 864e5);
const grow = (fee) => tr.value.map((v, i) => ((10000 * v) / tr.value[0]) * (1 - fee) ** years(i));
const gross = chart.addSeries('line', { color: '#e6e9ef', lineWidth: 2, pricePrecision: 0 });
const cheap = chart.addSeries('line', { color: '${ACCENT}', lineWidth: 1, pricePrecision: 0 });
const dear = chart.addSeries('line', { color: '${DOWN}', lineWidth: 2, pricePrecision: 0 });
gross.setData({ time: tr.time, value: grow(0) });
cheap.setData({ time: tr.time, value: grow(0.001) });
dear.setData({ time: tr.time, value: grow(0.015) });
// The red band is what a 1.5% fee costs over the years
chart.addBand(gross, dear, { fill: 'rgba(255, 93, 115, 0.18)' });
${legend([['Before fees', '#e6e9ef'], ['0.10% a year', ACCENT], ['1.50% a year', DOWN]])}
chart.setVisibleLogicalRange(-2, tr.time.length + 2);`,
      about: 'Ten years of growth of 10,000 in a total-return index before fees, and after a 0.10% and a 1.50% annual fee. `addBand` fills what the higher fee takes, which compounds into a wide gap.',
      api: ['addBand', "addSeries('line')"],
    },

    // ---------------------------------------------------------------- Investor returns
    {
      id: 'dca',
      title: 'Dollar-cost averaging',
      group: 'Investor returns',
      code: `${create()}
${bars({ ...D, count: 1260 })}
// 500 invested on the first session of every month, or the same total up front
const month = (i) => new Date(bars.time[i]).getUTCMonth();
const isNewMonth = bars.time.map((_, i) => i === 0 || month(i) !== month(i - 1));
const total = isNewMonth.filter(Boolean).length * 500;
let units = 0, paid = 0;
const dca = [], invested = [];
bars.close.forEach((c, i) => {
  if (isNewMonth[i]) { units += 500 / c; paid += 500; }
  dca.push(units * c);
  invested.push(paid);
});
const lump = bars.close.map((c) => (total * c) / bars.close[0]);

chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2, pricePrecision: 0 }).setData({ time: bars.time, value: lump });
chart.addSeries('area', { color: '${ACCENT}', topFill: 'rgba(52, 211, 153, 0.25)', bottomFill: 'rgba(52, 211, 153, 0.02)', lineWidth: 2, pricePrecision: 0 })
  .setData({ time: bars.time, value: dca });
chart.addSeries('line', { color: '#8b93a7', lineWidth: 1, step: true, pricePrecision: 0 }).setData({ time: bars.time, value: invested });
${legend([['Lump sum, all invested on day one', '#4cc9f0'], ['Monthly plan, value', ACCENT], ['Monthly plan, paid in', '#8b93a7']])}
chart.setVisibleLogicalRange(-2, bars.time.length + 2);`,
      about: 'Five years of a monthly investment plan against investing the same total on day one: the plan\'s value as an `area`, the money paid in as a step line, and the lump sum as a line.',
      api: ["addSeries('area')", 'step lines'],
    },
    {
      id: 'managertenure',
      title: 'Returns by manager tenure',
      group: 'Investor returns',
      code: `${create()}
${load('nav', 'series/fundGrowthNav', { symbol: 'NOVA', interval: '1d', count: 2000 })}
${load('changes', 'events/fundManagerChanges', { symbol: 'NOVA', interval: '1d', count: 2000 })}
chart.addSeries('line', { color: '#e6e9ef', lineWidth: 2, pricePrecision: 2 }).setData({ time: nav.time, value: nav.nav });
chart.applyPriceScaleOptions({ mode: 'logarithmic', margins: { top: 0.15, bottom: 0.05 } });

// Tenures: from each manager change to the next, with the return over it
const starts = [0, ...changes.time.map((t) => nav.time.indexOf(t)).filter((i) => i > 60)];
const tenures = starts.map((s, k) => {
  const e = k + 1 < starts.length ? starts[k + 1] : nav.time.length - 1;
  return { from: nav.time[s], to: nav.time[e], name: 'Manager ' + 'ABCDEFGH'[k], ret: nav.nav[e] / nav.nav[s] - 1 };
});

// Drawn by a layer plugin: a shaded band and a label per tenure
chart.addLayer({ pane: 0, position: 'background', build: (ctx) => tenures.flatMap((t, k) => {
  const x0 = ctx.timeToX(t.from), x1 = ctx.timeToX(t.to);
  if (x0 === null || x1 === null) return [];
  const label = t.name + '  ' + (t.ret >= 0 ? '+' : '') + (t.ret * 100).toFixed(0) + '%';
  return [
    { type: 'rect', x: x0, y: ctx.rect.y, width: x1 - x0, height: ctx.rect.height, color: k % 2 ? 'rgba(52, 211, 153, 0.07)' : 'rgba(76, 201, 240, 0.04)' },
    { type: 'text', x: x0 + 8, y: ctx.rect.y + 16, text: label, color: t.ret >= 0 ? '${UP}' : '${DOWN}', size: 12, align: 'left' },
  ];
}) });
chart.setVisibleLogicalRange(-2, nav.time.length + 2);`,
      about: 'Eight years of an active fund\'s NAV on a log scale. A layer plugin shades each portfolio manager\'s tenure, taken from an event feed of manager changes, and labels it with the return over it.',
      api: ['addLayer', 'PluginCtx.timeToX', 'applyPriceScaleOptions'],
    },

    // ---------------------------------------------------------------- Holdings & flows
    {
      id: 'topholdings',
      title: 'Top holdings over time',
      group: 'Holdings & flows',
      code: `${create()}
${load('h', 'series/fundHoldings', { symbol: 'ES', interval: '1q', count: 20 })}
const keys = ['stockA', 'stockB', 'stockC', 'stockD', 'stockE'];
const colors = ['${ACCENT}', '#4cc9f0', '#b388ff', '#ffb547', '${DOWN}'];
// Stack the weights (percent of net assets); the top is the top-5 concentration
let top = h.time.map(() => 0);
const tops = keys.map((k) => (top = top.map((v, i) => v + h[k][i] * 100)));
for (let k = keys.length - 1; k >= 0; k--) {
  chart.addSeries('histogram', { color: colors[k], pricePrecision: 1 }).setData({ time: h.time, value: tops[k] });
}
${legend([['Stock E', DOWN], ['Stock D', '#ffb547'], ['Stock C', '#b388ff'], ['Stock B', '#4cc9f0'], ['Stock A, % of NAV', ACCENT]])}
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0 } });
chart.setVisibleLogicalRange(-0.7, h.time.length - 0.3);`,
      about: 'Portfolio weights of a fund\'s five largest holdings from quarterly disclosures, stacked: each segment is one holding and the height of the bar is the top-5 concentration.',
      api: ["addSeries('histogram')", 'applyPriceScaleOptions'],
    },
    {
      id: 'fundshares',
      title: 'Fund shares & net creations',
      group: 'Holdings & flows',
      code: `${create()}
${bars({ ...D, count: 250 })}
${load('u', 'series/fundUnits', { ...D, count: 250 })}
chart.addCandlestickSeries().setData(bars);
chart.addSeries('area', { color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.25)', bottomFill: 'rgba(76, 201, 240, 0.02)', pricePrecision: 3 }, chart.addPane(0.6))
  .setData({ time: u.time, value: u.shares.map((v) => v / 1e9) });
${legend([['Shares outstanding, bn', '#4cc9f0']], 1)}
// Net creations (+) and redemptions (-): the daily change in shares, millions
const net = u.shares.map((v, i) => (i ? (v - u.shares[i - 1]) / 1e6 : NaN));
chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.5)).setData({
  time: u.time, value: net, color: net.map((v) => (v >= 0 ? '${UP}' : '${DOWN}')),
});
${legend([['Net creations, m shares', '#8b93a7']], 2)}
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'An index fund\'s shares outstanding and the daily net creations and redemptions under its index: investors redeem into rallies and buy the dips.',
      api: ["addSeries('area')", "addSeries('histogram')", 'per-point color'],
    },
    ...plotDemos.funds,
  ],
};
