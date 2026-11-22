import { create, load } from './build';
import type { Category } from './types';

const rose = '#fb7185', up = '#2ee6b6', down = '#ff5d73', amber = '#ffb547', sky = '#4cc9f0', violet = '#b388ff', grey = '#8b93a7';

/** Calendar months from now, for the calculators: `month(k)` is UTC ms. */
const MONTHS = `const now = new Date();
const month = (k) => Date.UTC(now.getUTCFullYear(), now.getUTCMonth() + k, 1);`;

export const finance: Category = {
  id: 'finance',
  title: 'Private markets & personal finance',
  kicker: '17',
  blurb: 'J-curves, runways, valuations, mortgages and retirement paths.',
  accent: rose,
  demos: [
    // ----------------------------------------------------------- Private funds
    {
      id: 'jcurve',
      title: 'Private fund J-curve',
      group: 'Private funds',
      code: `${create({ maxBarSpacing: 80 })}
// A $100m fund over its 12-year life, projected with a simple pacing model:
// each year calls a share of the unfunded commitment and distributes a
// growing share of the net asset value.
const commitment = 100, growth = 0.12, life = 12;
const callRate = [0.25, 0.33, 0.45, 0.5, 0.6, 0.7, 1, 1, 1, 1, 1, 1];
const vintage = new Date().getUTCFullYear() - 3;
const time = [], net = [], cumulative = [];
let paid = 0, nav = 0, total = 0;
for (let y = 0; y < life; y++) {
  const call = callRate[y] * (commitment - paid);
  const dist = nav * (1 + growth) * (y / (life - 1)) ** 2.5;
  paid += call;
  nav = nav * (1 + growth) + call - dist;
  total += dist - call;
  time.push(Date.UTC(vintage + y, 0, 1));
  net.push(dist - call);
  cumulative.push(total);
}
const bars = chart.addSeries('histogram', { pricePrecision: 1 });
bars.setData({ time, value: net, color: net.map((v) => (v >= 0 ? 'rgba(46, 230, 182, 0.7)' : 'rgba(255, 93, 115, 0.7)')) });
const line = chart.addSeries('line', { color: '${amber}', lineWidth: 3, pricePrecision: 1 });
line.setData({ time, value: cumulative });
bars.createPriceLine({ price: 0, color: '${grey}', lineWidth: 1, axisLabelVisible: false });
const trough = cumulative.indexOf(Math.min(...cumulative));
const payback = cumulative.findIndex((v) => v >= 0);
line.setMarkers([
  { time: time[trough], position: 'belowBar', shape: 'arrowUp', color: '${down}', text: 'Trough ' + cumulative[trough].toFixed(0) + 'm' },
  { time: time[payback], position: 'aboveBar', shape: 'arrowDown', color: '${up}', text: 'Break-even' },
]);
chart.fitContent();`,
      about: 'Net cash flow to investors per year in millions (calls red, distributions green) and the cumulative net cash flow in amber: the J. The schedule is projected in the example with a simple pacing model, so it runs years into the future; markers show the trough and the break-even year.',
      api: ["addSeries('histogram')", 'createPriceLine', 'setMarkers'],
    },
    {
      id: 'capitalcalls',
      title: 'Capital calls & distributions',
      group: 'Private funds',
      code: `${create()}
${load('f', 'series/finFundCashflows', { interval: '1q', count: 28 })}
const pane = chart.addPane(0.8);
// Calls are cash out of the investor's pocket: negative bars.
chart.addSeries('histogram', { color: '${down}', pricePrecision: 1 }).setData({ time: f.time, value: f.called.map((v) => -v) });
chart.addSeries('histogram', { color: '${up}', pricePrecision: 1 }).setData({ time: f.time, value: f.distributed });
let total = 0;
const net = f.time.map((_, i) => (total += f.distributed[i] - f.called[i]));
const cum = chart.addSeries('baseline', {
  baseValue: 0, color: '${up}', downColor: '${down}',
  topFill: 'rgba(46, 230, 182, 0.3)', bottomFill: 'rgba(255, 93, 115, 0.3)', pricePrecision: 1,
}, pane);
cum.setData({ time: f.time, value: net });
chart.fitContent();`,
      about: 'Seven years of quarterly capital calls (down) and distributions (up) of one fund commitment in millions, and below, the cumulative net cash flow as a `baseline` series: deep in the red while the fund invests, climbing back as it exits.',
      api: ["addSeries('histogram')", "addSeries('baseline')", 'addPane'],
    },
    {
      id: 'tvpi',
      title: 'DPI · TVPI',
      group: 'Private funds',
      code: `${create()}
${load('m', 'series/finFundMultiples', { interval: '1q', count: 32 })}
const tvpi = chart.addSeries('line', { color: '${rose}', lineWidth: 2.5, pricePrecision: 2 });
const dpi = chart.addSeries('line', { color: '${up}', lineWidth: 2.5, pricePrecision: 2 });
tvpi.setData({ time: m.time, value: m.tvpi });
dpi.setData({ time: m.time, value: m.dpi });
// Between them: value still held, not yet paid out (RVPI).
chart.addBand(tvpi, dpi, { fill: 'rgba(251, 113, 133, 0.18)' });
tvpi.createPriceLine({ price: m.tvpi.at(-1), color: '${rose}', lineStyle: 'dotted', axisLabelVisible: false, title: 'TVPI' });
dpi.createPriceLine({ price: m.dpi.at(-1), color: '${up}', lineStyle: 'dotted', axisLabelVisible: false, title: 'DPI' });
dpi.createPriceLine({ price: 1, color: '${grey}', lineStyle: 'dashed', title: '1.0x' });
chart.setBarSpacing(30);
chart.scrollToRealtime();`,
      about: 'A fund\'s quarterly multiples: total value to paid-in (TVPI) and distributed to paid-in (DPI). `addBand` fills the gap, the residual value still held (RVPI), which turns into cash as DPI catches up.',
      api: ['addBand', 'createPriceLine', "addSeries('line')"],
    },
    {
      id: 'aummix',
      title: 'Platform AUM by business',
      group: 'Private funds',
      code: `${create()}
${load('a', 'series/finAum', { interval: '1q', count: 40 })}
const lines = [
  ['privateEquity', 'Private equity', '${rose}'], ['credit', 'Credit', '${sky}'],
  ['realEstate', 'Real estate', '${amber}'], ['infrastructure', 'Infrastructure', '${violet}'],
];
// Stack: each line is the running total; bands fill between neighbours.
let sum = a.time.map(() => 0);
let below = chart.addSeries('line', { color: 'transparent', lastValueVisible: false, priceLineVisible: false });
below.setData({ time: a.time, value: sum });
for (const [key, title, color] of lines) {
  sum = sum.map((s, i) => s + a[key][i]);
  const line = chart.addSeries('line', { color, lineWidth: 1.5, pricePrecision: 0 });
  line.setData({ time: a.time, value: sum });
  line.createPriceLine({ price: sum.at(-1), color, lineStyle: 'dotted', axisLabelVisible: false, title });
  chart.addBand(line, below, { fill: color + '55' });
  below = line;
}
chart.setBarSpacing(24);
chart.scrollToRealtime();`,
      about: 'Ten years of assets under management in billions by business line, stacked: each line is a running total and `addBand` fills between neighbours. Each layer is named at its top edge by a price line with a `title`.',
      api: ['addBand', 'createPriceLine', "addSeries('line')"],
    },

    // ----------------------------------------------------------- Startups & companies
    {
      id: 'cashrunway',
      title: 'Cash runway scenarios',
      group: 'Companies',
      code: `${create()}
${load('r', 'series/finRunway', { interval: '1mo', count: 18, ahead: 30 })}
// History up to this month, then three burn scenarios from the last balance.
const last = r.time.findLastIndex((t) => t <= Date.now());
const actual = r.cash.map((v, i) => (i <= last ? v : NaN));
const history = chart.addSeries('area', { color: '#f0f0f0', topFill: 'rgba(240, 240, 240, 0.15)', bottomFill: 'rgba(240, 240, 240, 0)', pricePrecision: 1 });
history.setData({ time: r.time, value: actual });
history.createPriceLine({ price: 0, color: '${grey}', lineWidth: 1, title: 'out of cash' });
const scenarios = [
  ['Growth hiring', '${down}', 1.06], ['Current plan', '${amber}', 1.02], ['Hiring freeze', '${up}', 0.95],
];
scenarios.forEach(([name, color, monthly], n) => {
  let cash = r.cash[last], burn = r.burn[last], out = null;
  const value = r.time.map((t, i) => {
    if (i < last || out !== null) return NaN;
    if (i > last) { burn *= monthly; cash -= burn; }
    if (cash <= 0) { out = t; return 0; }
    return cash;
  });
  const line = chart.addSeries('line', { color, lineWidth: 2, lineStyle: 'dashed', pricePrecision: 1, lastValueVisible: false, priceLineVisible: false });
  line.setData({ time: r.time, value });
  // Alternate label sides so neighbouring markers do not collide.
  if (out) line.setMarkers([{ time: out, position: n % 2 ? 'belowBar' : 'aboveBar', shape: n % 2 ? 'arrowUp' : 'arrowDown', color, text: name }]);
  else line.createPriceLine({ price: value.findLast((v) => v > 0), color, lineStyle: 'dotted', axisLabelVisible: false, title: name });
});
chart.fitContent();`,
      about: 'A startup\'s cash in millions: 18 months of history, then three burn scenarios projected 30 months ahead from the last balance and burn. The series reaches into the future with `ahead`; markers show the month each scenario runs out of cash.',
      api: ['ahead', "lineStyle: 'dashed'", 'setMarkers', 'createPriceLine'],
    },
    {
      id: 'rounds',
      title: 'Funding rounds & valuation',
      group: 'Companies',
      code: `${create()}
${load('rounds', 'events/finRounds', { interval: '1mo', count: 90 })}
// A monthly axis from the first round to now.
const time = [];
const first = new Date(rounds.time[0]);
for (let k = 0; Date.UTC(first.getUTCFullYear(), first.getUTCMonth() + k, 1) <= Date.now(); k++) {
  time.push(Date.UTC(first.getUTCFullYear(), first.getUTCMonth() + k, 1));
}
// Post-money valuation if each round sells 20% of the company.
const names = ['Seed', 'Series A', 'Series B', 'Series C', 'Series D', 'Series E', 'Series F'];
const post = rounds.value.map((raised) => raised / 0.2);
let k = -1;
const valuation = time.map((t) => {
  while (k + 1 < rounds.time.length && rounds.time[k + 1] <= t) k++;
  return post[k];
});
const step = chart.addSeries('area', {
  color: '${rose}', topFill: 'rgba(251, 113, 133, 0.3)', bottomFill: 'rgba(251, 113, 133, 0.02)', step: true, lineWidth: 2, pricePrecision: 0,
});
step.setData({ time, value: valuation });
step.setMarkers(rounds.time.map((t, i) => ({
  time: t, position: 'aboveBar', shape: 'circle', color: '${rose}', text: names[i] + ' · $' + post[i].toFixed(0) + 'm',
})));
chart.applyPriceScaleOptions({ mode: 'logarithmic', margins: { top: 0.15, bottom: 0.05 } });
chart.addSeries('histogram', { color: '${amber}', pricePrecision: 1 }, chart.addPane(0.4))
  .setData({ time: rounds.time, value: rounds.value });
chart.setVisibleLogicalRange(-8, time.length + 8);`,
      about: 'A startup\'s post-money valuation in millions as a step `area` on a `logarithmic` scale, one step per funding round, with the amount raised per round below. The rounds come from an events endpoint; the valuation assumes each round sells 20% of the company.',
      api: ['step lines', "mode: 'logarithmic'", 'setMarkers'],
    },
    {
      id: 'covenantheadroom',
      title: 'Debt covenant headroom',
      group: 'Companies',
      code: `${create()}
${load('c', 'series/finCovenant', { interval: '1q', count: 28 })}
const limit = 4.0; // maximum net debt / EBITDA
const lev = chart.addSeries('line', { color: '${sky}', lineWidth: 2.5, pricePrecision: 2 });
lev.setData({ time: c.time, value: c.leverage });
lev.createPriceLine({ price: limit, color: '${down}', lineWidth: 2, title: 'covenant ' + limit.toFixed(1) + 'x' });
chart.applyPriceScaleOptions({ range: { low: 2, high: 4.5 } });

// Headroom: how far EBITDA could fall before the covenant is breached.
const headroom = c.leverage.map((v) => (1 - v / limit) * 100);
const h = chart.addSeries('baseline', {
  baseValue: 0, color: '${up}', downColor: '${down}',
  topFill: 'rgba(46, 230, 182, 0.3)', bottomFill: 'rgba(255, 93, 115, 0.4)', pricePrecision: 1,
}, chart.addPane(0.8));
h.setData({ time: c.time, value: headroom });
h.createPriceLine({ price: 10, color: '${amber}', lineStyle: 'dashed', title: 'watch' });
chart.fitContent();`,
      about: 'A borrower\'s quarterly net debt to EBITDA against a 4.0x maintenance covenant (red price line), and below, the headroom in percent: how far EBITDA could fall before a breach. Under 10% puts the loan on the watch list.',
      api: ['createPriceLine', "addSeries('baseline')", 'addPane'],
    },
    {
      id: 'cashcycle',
      title: 'Cash conversion cycle',
      group: 'Companies',
      code: `${create()}
${load('c', 'series/finCashCycle', { interval: '1q', count: 24 })}
const lines = [['dso', 'DSO', '${sky}'], ['dio', 'DIO', '${amber}'], ['dpo', 'DPO', '${violet}']];
for (const [key, title, color] of lines) {
  const line = chart.addSeries('line', { color, lineWidth: 2, pricePrecision: 0 });
  line.setData({ time: c.time, value: c[key] });
  line.createPriceLine({ price: c[key].at(-1), color, lineStyle: 'dotted', axisLabelVisible: false, title });
}
// Days between paying suppliers and collecting from customers.
const ccc = c.time.map((_, i) => c.dso[i] + c.dio[i] - c.dpo[i]);
const avg = ccc.reduce((a, b) => a + b, 0) / ccc.length;
const bars = chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.8));
bars.setData({ time: c.time, value: ccc, color: ccc.map((v) => (v > avg ? '${down}' : '${up}')) });
bars.createPriceLine({ price: avg, color: '#f0f0f0', lineStyle: 'dashed', title: 'average' });
chart.setBarSpacing(36);
chart.scrollToRealtime();`,
      about: 'Days sales outstanding (DSO), days inventory outstanding (DIO) and days payables outstanding (DPO) by quarter, and below, the cash conversion cycle DSO + DIO − DPO. Red quarters tie up cash for longer than average.',
      api: ["addSeries('line')", "addSeries('histogram')", 'per-point color'],
    },

    // ----------------------------------------------------------- Personal finance
    {
      id: 'amortization',
      title: 'Mortgage amortization',
      group: 'Personal finance',
      code: `${create()}
${MONTHS}
// A $400,000 30-year fixed mortgage at 6.5%, totalled per year.
const loan = 400000, rate = 0.065 / 12, years = 30;
const payment = (loan * rate) / (1 - (1 + rate) ** -(years * 12));
const time = [], interest = [], principal = [], balance = [];
let owed = loan;
for (let y = 0; y < years; y++) {
  let i = 0;
  for (let m = 0; m < 12; m++) {
    const part = owed * rate;
    i += part;
    owed -= payment - part;
  }
  time.push(month(12 * y + 12));
  interest.push(i / 1000);
  principal.push((12 * payment - i) / 1000);
  balance.push(Math.max(owed, 0) / 1000);
}
// Stacked: the year's payments in amber, the principal part drawn over it.
chart.addSeries('histogram', { color: '${amber}', pricePrecision: 1 }).setData({ time, value: time.map((_, k) => interest[k] + principal[k]) });
chart.addSeries('histogram', { color: '${sky}', pricePrecision: 1 }).setData({ time, value: principal });
chart.applyPriceScaleOptions({ margins: { top: 0.12, bottom: 0 } });
chart.addSeries('area', {
  color: '${rose}', topFill: 'rgba(251, 113, 133, 0.3)', bottomFill: 'rgba(251, 113, 133, 0.02)', pricePrecision: 0,
}, chart.addPane(0.6)).setData({ time, value: balance });
chart.fitContent();`,
      about: 'Each year\'s payments on a 30-year fixed mortgage, in thousands, split into interest (amber) and principal (blue) by drawing two histograms over each other, with the remaining balance below. Interest dominates for the first half; the schedule is computed in the example and runs into the future.',
      api: ["addSeries('histogram')", "addSeries('area')", 'addPane'],
    },
    {
      id: 'prepay',
      title: 'Prepayment vs standard',
      group: 'Personal finance',
      code: `${create()}
${MONTHS}
// The same $400,000 loan at 6.5%, paid as scheduled or with $500 extra a month.
const loan = 400000, rate = 0.065 / 12, n = 360;
const payment = (loan * rate) / (1 - (1 + rate) ** -n);
const schedule = (extra) => {
  let owed = loan, paid = 0;
  const balance = [], interest = [];
  for (let k = 1; k <= n; k++) {
    const i = owed * rate;
    paid += owed > 0 ? i : 0;
    owed = Math.max(0, owed - (payment + extra - i));
    balance.push(owed / 1000);
    interest.push(paid / 1000);
  }
  return { balance, interest };
};
const base = schedule(0), fast = schedule(500);
const time = base.balance.map((_, k) => month(k + 1));
const standard = chart.addSeries('line', { color: '${grey}', lineWidth: 2, pricePrecision: 0 });
const extra = chart.addSeries('line', { color: '${up}', lineWidth: 2.5, pricePrecision: 0 });
standard.setData({ time, value: base.balance });
extra.setData({ time, value: fast.balance });
chart.addBand(standard, extra, { fill: 'rgba(46, 230, 182, 0.15)' });
const done = fast.balance.findIndex((b) => b === 0);
extra.setMarkers([{ time: time[done], position: 'aboveBar', shape: 'arrowDown', color: '${up}', text: 'Paid off ' + ((n - done) / 12).toFixed(1) + ' years early' }]);

// Interest saved so far, in thousands.
chart.addSeries('area', {
  color: '${amber}', topFill: 'rgba(255, 181, 71, 0.35)', bottomFill: 'rgba(255, 181, 71, 0.02)', pricePrecision: 0,
}, chart.addPane(0.6)).setData({ time, value: time.map((_, k) => base.interest[k] - fast.interest[k]) });
chart.fitContent();`,
      about: 'Remaining balance in thousands of a 30-year mortgage paid as scheduled (grey) or with $500 extra a month (green), with `addBand` between them and the cumulative interest saved below. Both schedules are computed in the example.',
      api: ['addBand', 'setMarkers', "addSeries('area')"],
    },
    {
      id: 'debtplan',
      title: 'Debt payoff: snowball vs avalanche',
      group: 'Personal finance',
      code: `${create()}
${MONTHS}
// Four debts and $1,100 a month to pay them.
const debts = [
  { name: 'Card', balance: 19000, apr: 0.269, min: 380 },
  { name: 'Medical bill', balance: 1500, apr: 0, min: 50 },
  { name: 'Personal loan', balance: 4500, apr: 0.11, min: 150 },
  { name: 'Car loan', balance: 9000, apr: 0.069, min: 260 },
];
const budget = 1100;
// Snowball pays the smallest balance first, avalanche the highest rate.
const simulate = (order) => {
  const d = debts.map((x) => ({ ...x })), left = [];
  while (d.some((x) => x.balance > 0) && left.length < 240) {
    d.forEach((x) => (x.balance *= 1 + x.apr / 12));
    let cash = budget;
    for (const x of d) { const p = Math.min(x.min, x.balance); x.balance -= p; cash -= p; }
    for (const x of [...d].sort(order)) { const p = Math.min(cash, x.balance); x.balance -= p; cash -= p; }
    left.push(d.reduce((s, x) => s + x.balance, 0) / 1000);
  }
  return left;
};
const snowball = simulate((a, b) => (a.balance || 1e12) - (b.balance || 1e12));
const avalanche = simulate((a, b) => b.apr - a.apr);
const time = Array.from({ length: Math.max(snowball.length, avalanche.length) + 1 }, (_, k) => month(k));
const total = debts.reduce((s, x) => s + x.balance, 0) / 1000;
const pad = (v) => [total, ...v, ...Array(time.length - 1 - v.length).fill(NaN)];

const s = chart.addSeries('line', { color: '${amber}', lineWidth: 2.5, pricePrecision: 1 });
const a = chart.addSeries('line', { color: '${sky}', lineWidth: 2.5, pricePrecision: 1 });
s.setData({ time, value: pad(snowball) });
a.setData({ time, value: pad(avalanche) });
chart.addBand(s, a, { fill: 'rgba(76, 201, 240, 0.2)' });
const fmt = (k) => new Date(time[k]).toLocaleDateString('en-US', { month: 'short', year: 'numeric', timeZone: 'UTC' });
s.setMarkers([{ time: time[snowball.length], position: 'aboveBar', shape: 'arrowDown', color: '${amber}', text: 'Snowball · ' + fmt(snowball.length) }]);
a.setMarkers([{ time: time[avalanche.length], position: 'belowBar', shape: 'arrowUp', color: '${sky}', text: 'Avalanche · ' + fmt(avalanche.length) }]);
chart.setVisibleLogicalRange(-1, time.length + 6);`,
      about: 'Total debt left in thousands under two payoff plans with the same monthly budget: snowball (smallest balance first, amber) and avalanche (highest rate first, blue). Both are simulated in the example; the band is the extra debt the snowball carries, and markers show each debt-free month.',
      api: ['addBand', 'setMarkers', "addSeries('line')"],
    },
    {
      id: 'networth',
      title: 'Net worth breakdown',
      group: 'Personal finance',
      code: `${create()}
${load('w', 'series/finNetWorth', { interval: '1mo', count: 120 })}
// Stacked bars: assets up, liabilities down, in thousands. Draw the tallest
// stack first and each shorter one over it.
const stack = (keys) => w.time.map((_, i) => keys.reduce((s, k) => s + w[k][i], 0));
const layers = [
  [['home', 'investments', 'cash'], '${amber}', 1], [['home', 'investments'], '${sky}', 1], [['home'], '#3a4a66', 1],
  [['mortgage', 'otherDebt'], '${down}', -1], [['mortgage'], '#8a3346', -1],
];
for (const [keys, color, sign] of layers) {
  chart.addSeries('histogram', { color, pricePrecision: 0, lastValueVisible: false, priceLineVisible: false })
    .setData({ time: w.time, value: stack(keys).map((v) => sign * v) });
}
const assets = stack(['home', 'investments', 'cash']), debts = stack(['mortgage', 'otherDebt']);
const net = chart.addSeries('line', { color: '#f0f0f0', lineWidth: 2.5, pricePrecision: 0 });
net.setData({ time: w.time, value: assets.map((v, i) => v - debts[i]) });
net.createPriceLine({ price: assets.at(-1) - debts.at(-1), color: '#f0f0f0', lineStyle: 'dotted', axisLabelVisible: false, title: 'net worth' });
chart.setBarSpacing(8);
chart.scrollToRealtime();`,
      about: 'Ten years of a household balance sheet in thousands: home (slate), investments (blue) and cash (amber) stacked up, mortgage (dark red) and other debt (red) stacked down, as overlaid `histogram` series, with net worth as a white line.',
      api: ["addSeries('histogram')", "addSeries('line')", 'createPriceLine'],
    },
    {
      id: 'retirement',
      title: 'Retirement savings fan',
      group: 'Personal finance',
      code: `${create()}
${load('f', 'forecast', { symbol: 'ES', interval: '1w', ahead: 1040, quantiles: '5,25,50,75,95' })}
// $650k invested for 20 years: market quantile paths (relative to today)
// plus an assumed 5% a year average return, in thousands.
const start = 650, drift = 0.05, year = 365.25 * 86400000;
const path = (q) => f[q].map((p, i) => (start * p * Math.exp(drift * (f.time[i] - f.time[0]) / year)) / f[q][0]);
const quiet = { lineWidth: 1, lastValueVisible: false, priceLineVisible: false, pricePrecision: 0 };
const p5 = chart.addSeries('line', { ...quiet, color: 'rgba(251, 113, 133, 0.35)' });
const p95 = chart.addSeries('line', { ...quiet, color: 'rgba(251, 113, 133, 0.35)' });
const p25 = chart.addSeries('line', { ...quiet, color: 'rgba(251, 113, 133, 0.6)' });
const p75 = chart.addSeries('line', { ...quiet, color: 'rgba(251, 113, 133, 0.6)' });
p5.setData({ time: f.time, value: path('p5') });
p95.setData({ time: f.time, value: path('p95') });
p25.setData({ time: f.time, value: path('p25') });
p75.setData({ time: f.time, value: path('p75') });
chart.addBand(p95, p5, { fill: 'rgba(251, 113, 133, 0.12)' });
chart.addBand(p75, p25, { fill: 'rgba(251, 113, 133, 0.22)' });
const median = chart.addSeries('line', { color: '${rose}', lineWidth: 2.5, pricePrecision: 0 });
median.setData({ time: f.time, value: path('p50') });
median.createPriceLine({ price: 2000, color: '${up}', lineStyle: 'dashed', title: 'goal $2m' });
chart.fitContent();`,
      about: 'Twenty years of weekly outcomes for a $650k portfolio: the median path and the 25–75% and 5–95% ranges filled with `addBand`, against a $2m goal. The quantile paths come from the forecast endpoint, scaled with an assumed 5% average yearly return in the example.',
      api: ['addBand', 'createPriceLine', '/api/forecast'],
    },
  ],
};
