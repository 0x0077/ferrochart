import { bars, create, load } from './build';
import type { Category } from './types';

// A-share convention: red is up, green is down.
const RED = '#ff4d4f';
const GREEN = '#2ee6b6';
const CANDLES = `{ color: '${RED}', downColor: '${GREEN}' }`;

const STOCK = { symbol: 'CNA', interval: '1d' };
const INDEX = { symbol: 'CN300', interval: '1d' };

/** A small layer plugin that writes a color-keyed legend in a pane's top-left corner. */
function legend(items: [string, string][], pane = 0): string {
  const list = items.map(([t, c]) => `['${t}', '${c}']`).join(', ');
  return `chart.addLayer({ pane: ${pane}, build: (ctx) => [${list}]
  .map(([text, color], i) => ({ type: 'text', x: ctx.rect.x + 10, y: ctx.rect.y + 14 + i * 17, text, color, size: 12 })) });`;
}

/** `name` colored red for values ≥ 0, green below. */
const redGreen = (name: string) => `${name}.map((v) => (v >= 0 ? '${RED}' : '${GREEN}'))`;

/** Candles of `name` with A-share colors. */
const candles = (name = 'bars') => `chart.addCandlestickSeries(${CANDLES}).setData(${name});`;

export const ashares: Category = {
  id: 'ashares',
  title: 'China A-shares',
  kicker: '11',
  blurb: 'Intraday tick charts, money flow, limit-up statistics and chip distribution, red for up.',
  accent: RED,
  demos: [
    // ---------------------------------------------------------------- Intraday
    {
      id: 'intraday',
      title: 'Intraday tick chart',
      group: 'Intraday',
      code: `${create()}
${bars({ symbol: 'CNA', interval: '1m', count: 480 })}
// The axis labels times as given: shift them to Beijing time (UTC+8)
const bj = bars.time.map((t) => t + 8 * 3600e3);
const day = (t) => Math.floor(t / 864e5);
// The last session and the close before it
const start = bj.findIndex((t) => day(t) === day(bj.at(-1)));
const prevClose = bars.close[start - 1];
const time = bj.slice(start), close = bars.close.slice(start), volume = bars.volume.slice(start);
// Average price: cumulative turnover / cumulative volume
let turnover = 0, shares = 0;
const avg = close.map((c, i) => (turnover += c * volume[i]) / (shares += volume[i]));

const price = chart.addSeries('baseline', {
  baseValue: prevClose, color: '${RED}', downColor: '${GREEN}',
  topFill: 'rgba(255, 77, 79, 0.12)', bottomFill: 'rgba(46, 230, 182, 0.12)', lineWidth: 1.5,
});
price.setData({ time, value: close });
price.createPriceLine({ price: prevClose, color: '#8b93a7', lineStyle: 'dashed', title: 'Prev close' });
chart.addSeries('line', { color: '#ffb547', lineWidth: 1 }).setData({ time, value: avg });
// A scale symmetric around the previous close, like the percent axis of a tick chart
const dev = Math.max(...close.map((c) => Math.abs(c - prevClose))) * 1.15;
chart.applyPriceScaleOptions({ range: { low: prevClose - dev, high: prevClose + dev } });
${legend([['Price vs previous close', '#e6e9ef'], ['Average price', '#ffb547']])}

// Volume (millions), red when the minute closed higher
chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.35)).setData({
  time, value: volume.map((v) => v / 1e6), color: close.map((c, i) => (c >= (i ? close[i - 1] : prevClose) ? '${RED}' : '${GREEN}')),
});
// The axis always spans the 240 minutes of a session
chart.setVisibleLogicalRange(-1, 240);`,
      about: 'One session of one-minute closes as a `baseline` series around the previous close, the running average price and volume. The 11:30–13:00 lunch break leaves no gap because the time axis is index based; times are shifted to Beijing time for the axis.',
      api: ["addSeries('baseline')", 'createPriceLine', 'setVisibleLogicalRange'],
    },
    {
      id: 'intraday5',
      title: 'Five-day tick chart',
      group: 'Intraday',
      code: `${create()}
${bars({ symbol: 'CNA', interval: '1m', count: 1450 })}
// Times shifted to Beijing time (UTC+8) for the axis; the last five sessions
const bj = bars.time.map((t) => t + 8 * 3600e3);
const day = (t) => Math.floor(t / 864e5);
const days = [...new Set(bj.map(day))].slice(-5);
const start = bj.findIndex((t) => day(t) === days[0]);
const prevClose = bars.close[start - 1];
const time = bj.slice(start), close = bars.close.slice(start), volume = bars.volume.slice(start);
// The average price restarts every session
let d = -1, turnover = 0, shares = 0;
const avg = close.map((c, i) => {
  if (day(time[i]) !== d) { d = day(time[i]); turnover = shares = 0; }
  return (turnover += c * volume[i]) / (shares += volume[i]);
});

const price = chart.addSeries('baseline', {
  baseValue: prevClose, color: '${RED}', downColor: '${GREEN}',
  topFill: 'rgba(255, 77, 79, 0.1)', bottomFill: 'rgba(46, 230, 182, 0.1)', lineWidth: 1.5,
});
price.setData({ time, value: close });
price.createPriceLine({ price: prevClose, color: '#8b93a7', lineStyle: 'dashed', title: 'Prev close' });
chart.addSeries('line', { color: '#ffb547', lineWidth: 1 }).setData({ time, value: avg });
chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.35)).setData({
  time, value: volume.map((v) => v / 1e6), color: close.map((c, i) => (c >= (i ? close[i - 1] : prevClose) ? '${RED}' : '${GREEN}')),
});

// Session separators and dates, drawn by a layer plugin
const opens = time.filter((t, i) => i === 0 || day(t) !== day(time[i - 1]));
chart.addLayer({ pane: 0, position: 'background', build: (ctx) => opens.flatMap((t) => {
  const x = ctx.timeToX(t);
  if (x === null) return [];
  const label = new Date(t).toISOString().slice(5, 10);
  return [
    { type: 'line', x1: x, y1: ctx.rect.y, x2: x, y2: ctx.rect.y + ctx.rect.height, color: 'rgba(139, 147, 167, 0.35)', dash: 'dashed' },
    { type: 'text', x: x + 8, y: ctx.rect.y + 12, text: label, color: '#8b93a7', size: 11, align: 'left' },
  ];
}) });
chart.setVisibleLogicalRange(-1, 1200);`,
      about: 'Five sessions of one-minute closes against the close before them, with the average price restarting each day. A layer plugin draws the session separators and dates.',
      api: ["addSeries('baseline')", 'addLayer', 'PluginCtx.timeToX'],
    },
    {
      id: 'auction',
      title: 'Opening call auction',
      group: 'Intraday',
      code: `${create()}
${bars({ ...STOCK, count: 2 }, 'daily')}
${load('feed', 'series/cnAuction', { symbol: 'CNA', interval: '5s', count: 121 })}
// One row per 5 s of the 09:15–09:25 auction. The demo API serves no pre-open
// timestamps, so the rows are put on the auction clock here, in Beijing time.
const prevClose = daily.close[0];
const t0 = daily.time[1] + (9 * 60 + 15) * 60e3;
const time = feed.gap.map((_, i) => t0 + i * 5000);

// Indicative match price
const price = chart.addSeries('line', { color: '#e6e9ef', lineWidth: 2, step: true });
price.setData({ time, value: feed.gap.map((g) => prevClose * (1 + g)) });
price.createPriceLine({ price: prevClose, color: '#8b93a7', lineStyle: 'dashed', title: 'Prev close' });
price.setMarkers([{ time: t0 + 5 * 60e3, position: 'belowBar', shape: 'arrowUp', color: '#ffb547', text: '09:20 no more cancels' }]);
${legend([['Indicative match price', '#e6e9ef']])}

// Matched volume builds up; the unmatched imbalance is buy (red) or sell (green) surplus
let matched = 0;
chart.addSeries('area', { color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.3)', bottomFill: 'rgba(255, 181, 71, 0)', pricePrecision: 0 }, chart.addPane(0.5))
  .setData({ time, value: feed.matched.map((v) => (matched += v)) });
${legend([['Matched volume, shares', '#ffb547']], 1)}
chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.5)).setData({
  time, value: feed.imbalance, color: ${redGreen('feed.imbalance')},
});
${legend([['Unmatched imbalance, shares', '#8b93a7']], 2)}
chart.setVisibleLogicalRange(-1, time.length);`,
      about: 'The ten-minute opening call auction: the indicative match price as a step line, matched volume building up, and the unmatched buy or sell surplus. A marker flags 09:20, after which orders can no longer be cancelled.',
      api: ['step lines', 'setMarkers', "addSeries('histogram')"],
    },
    {
      id: 'chips',
      title: 'Chip distribution (CYQ)',
      group: 'Intraday',
      code: `${create()}
${bars({ ...STOCK, count: 300 })}
const candles = chart.addCandlestickSeries(${CANDLES});
candles.setData(bars);

// Cost distribution: each day a share of the chips (that day's turnover) changes
// hands, spread over the day's range and peaking at the close. Assumes 3% average
// daily turnover.
const bins = 160, low = Math.min(...bars.low), high = Math.max(...bars.high);
const step = (high - low) / bins;
const avgVol = bars.volume.reduce((a, b) => a + b, 0) / bars.volume.length;
const chips = new Float64Array(bins);
bars.time.forEach((_, i) => {
  const turnover = Math.min(0.9, (0.03 * bars.volume[i]) / avgVol);
  const bin = (p) => Math.min(bins - 1, Math.floor((p - low) / step));
  const a = bin(bars.low[i]), b = bin(bars.high[i]), c = bin(bars.close[i]);
  const weight = (k) => 1 - Math.abs(k - c) / (Math.max(c - a, b - c) + 1);
  let sum = 0;
  for (let k = a; k <= b; k++) sum += weight(k);
  for (let k = 0; k < bins; k++) chips[k] *= 1 - turnover;
  for (let k = a; k <= b; k++) chips[k] += (turnover * weight(k)) / sum;
});
const close = bars.close.at(-1);
const total = chips.reduce((a, b) => a + b, 0);
const profit = chips.reduce((s, c, k) => s + (low + (k + 0.5) * step <= close ? c : 0), 0) / total;
const avgCost = chips.reduce((s, c, k) => s + c * (low + (k + 0.5) * step), 0) / total;
candles.createPriceLine({ price: avgCost, color: '#ffb547', lineStyle: 'dashed', title: 'Avg cost' });
const peak = Math.max(...chips);

// Drawn by a layer plugin: horizontal bars on the right, red in profit, blue trapped
chart.addLayer({ pane: 0, position: 'background', build: (ctx) => {
  const right = ctx.rect.x + ctx.rect.width, width = ctx.rect.width * 0.24;
  const out = [];
  chips.forEach((c, k) => {
    const y0 = ctx.priceToY(low + k * step), y1 = ctx.priceToY(low + (k + 1) * step);
    if (y0 === null || y1 === null) return;
    const w = (c / peak) * width;
    const inProfit = low + (k + 0.5) * step <= close;
    out.push({ type: 'rect', x: right - w, y: y1, width: w, height: Math.max(1, y0 - y1 - 0.5), color: inProfit ? 'rgba(255, 77, 79, 0.55)' : 'rgba(76, 201, 240, 0.5)' });
  });
  out.push({ type: 'text', x: right - 8, y: ctx.rect.y + 14, text: 'Profit ' + (profit * 100).toFixed(0) + '%  Avg cost ' + avgCost.toFixed(2), color: '#e6e9ef', size: 12, align: 'right' });
  return out;
} });
chart.setBarSpacing(4);
chart.scrollToRealtime();
chart.setRightOffset(70);`,
      about: 'Where the float last changed hands: a turnover-decay model in a few lines spreads each day\'s turnover over its range. A layer plugin draws the distribution beside the candles, red for chips in profit at the last close and blue for trapped ones, with the average cost as a price line.',
      api: ['addLayer', 'PluginCtx.priceToY', 'createPriceLine'],
    },

    // ---------------------------------------------------------------- Money flow
    {
      id: 'moneyflow',
      title: 'Main-force money flow',
      group: 'Money flow',
      code: `${create()}
${bars({ ...STOCK, count: 250 })}
${load('flow', 'series/cnMoneyFlow', { ...STOCK, count: 250 })}
${candles()}
// Main force: extra-large plus large orders, CNY millions
const main = flow.xl.map((v, i) => (v + flow.large[i]) / 1e6);
chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.5)).setData({ time: flow.time, value: main, color: ${redGreen('main')} });
${legend([['Main-force net inflow, CNY m', '#8b93a7']], 1)}

// Cumulative net inflow of each order size
const tiers = [['xl', '${RED}'], ['large', '#ffb547'], ['medium', '#4cc9f0'], ['small', '${GREEN}']];
const cum = chart.addPane(0.6);
for (const [key, color] of tiers) {
  let sum = 0;
  chart.addSeries('line', { color, lineWidth: 2, pricePrecision: 0 }, cum).setData({ time: flow.time, value: flow[key].map((v) => (sum += v / 1e6)) });
}
${legend([['Extra-large', RED], ['Large', '#ffb547'], ['Medium', '#4cc9f0'], ['Small', GREEN]], 2)}
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0.05 } }, cum);
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Daily net inflow of extra-large and large orders (the "main force") as a red/green `histogram`, and the cumulative net inflow of each order size below: big orders lean with the price, small ones against it.',
      api: ["addSeries('histogram')", 'per-point color', 'addPane'],
    },
    {
      id: 'connect',
      title: 'Cross-border connect flows',
      group: 'Money flow',
      code: `${create()}
${bars({ ...INDEX, count: 250 })}
${load('flow', 'series/cnConnect', { ...INDEX, count: 250 })}
${candles()}
const bn = flow.net.map((v) => v / 1e9); // CNY billions
chart.addSeries('histogram', { pricePrecision: 1 }, chart.addPane(0.5)).setData({ time: flow.time, value: bn, color: ${redGreen('bn')} });
${legend([['Daily net buying, CNY bn', '#8b93a7']], 1)}
let sum = 0;
chart.addSeries('area', { color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.25)', bottomFill: 'rgba(255, 181, 71, 0)', pricePrecision: 0 }, chart.addPane(0.5))
  .setData({ time: flow.time, value: bn.map((v) => (sum += v)) });
${legend([['Cumulative, CNY bn', '#ffb547']], 2)}
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Net buying of A-shares through the cross-border stock connect under the index: daily flows as a `histogram` and their running total as an `area` series.',
      api: ["addSeries('histogram')", "addSeries('area')"],
    },
    {
      id: 'margin',
      title: 'Margin financing balance',
      group: 'Money flow',
      code: `${create()}
${bars({ ...INDEX, count: 500 })}
${load('m', 'series/cnMargin', { ...INDEX, count: 500 })}
${candles()}
chart.addSeries('area', { color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.3)', bottomFill: 'rgba(255, 181, 71, 0.02)', pricePrecision: 3 }, chart.addPane(0.6))
  .setData({ time: m.time, value: m.financing.map((v) => v / 1e12) });
${legend([['Margin financing, CNY tn', '#ffb547']], 1)}
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2, pricePrecision: 1 }, chart.addPane(0.5))
  .setData({ time: m.time, value: m.lending.map((v) => v / 1e9) });
${legend([['Securities lending, CNY bn', '#4cc9f0']], 2)}
chart.setBarSpacing(2.2);
chart.scrollToRealtime();`,
      about: 'Two years of the market-wide margin financing balance (leveraged longs) and securities lending balance (shorts) on the index\'s time axis, each in its own pane and unit.',
      api: ["addSeries('area')", 'addPane'],
    },
    {
      id: 'blocktrade',
      title: 'Block trades',
      group: 'Money flow',
      code: `${create()}
${bars({ ...STOCK, count: 150 })}
${load('blocks', 'events/cnBlockTrades', { ...STOCK, count: 150 })}
const candles = chart.addCandlestickSeries(${CANDLES});
candles.setData(bars);
// The buyer of each block, above the bar
candles.setMarkers(blocks.time.map((time, i) => ({
  time, position: 'aboveBar', shape: 'circle', color: blocks.value[i] >= 0 ? '${RED}' : '#8b93a7', text: blocks.label[i],
})));
// Premium (red) or discount (green) to the close, percent
const pct = blocks.value.map((v) => v * 100);
chart.addSeries('histogram', { pricePrecision: 1 }, chart.addPane(0.45)).setData({ time: blocks.time, value: pct, color: ${redGreen('pct')} });
${legend([['Block price vs close, %', '#8b93a7']], 1)}
chart.setBarSpacing(7);
chart.scrollToRealtime();`,
      about: 'Off-exchange block trades from an event feed: markers name the buying branch, and the pane below gives each block\'s premium or discount to the day\'s close. Deep discounts usually mean a large holder selling.',
      api: ['setMarkers', "addSeries('histogram')"],
    },

    // ---------------------------------------------------------------- Market sentiment
    {
      id: 'limitstats',
      title: 'Limit-up statistics',
      group: 'Market sentiment',
      code: `${create()}
${bars({ ...INDEX, count: 250 })}
${load('lim', 'series/cnLimits', { ...INDEX, count: 250 })}
${candles()}
// Limit-up stocks up, limit-down stocks down
const counts = chart.addPane(0.6);
chart.addSeries('histogram', { color: '${RED}', pricePrecision: 0 }, counts).setData({ time: lim.time, value: lim.limitUp.map(Math.round) });
chart.addSeries('histogram', { color: '${GREEN}', pricePrecision: 0 }, counts).setData({ time: lim.time, value: lim.limitDown.map((v) => -Math.round(v)) });
${legend([['Limit-up stocks', RED], ['Limit-down stocks', GREEN]], 1)}
chart.applyPriceScaleOptions({ margins: { top: 0.25, bottom: 0.05 } }, counts);
// Failed limit-ups: touched the limit but closed below it
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 0 }, chart.addPane(0.5))
  .setData({ time: lim.time, value: lim.failRate.map((v) => v * 100) });
${legend([['Failed limit-up rate, %', '#ffb547']], 2)}
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'Market-wide counts of stocks closing at the daily price limit, up and down, and the share of limit-ups that failed to hold into the close, a gauge of speculative appetite.',
      api: ["addSeries('histogram')", "addSeries('line')", 'addPane'],
    },
    {
      id: 'sentiment',
      title: 'Market sentiment gauge',
      group: 'Market sentiment',
      code: `${create()}
${bars({ ...INDEX, count: 250 })}
${load('s', 'series/cnSentiment', { ...INDEX, count: 250 })}
${candles()}
// Composite 0-100: breadth, limit-ups and the longest limit-up streak, weighted
const clamp = (v) => Math.max(0, Math.min(1, v));
const score = s.advancers.map((a, i) =>
  100 * (0.5 * clamp(a / 5300) + 0.3 * clamp(s.limitUp[i] / 150) + 0.2 * clamp(s.streak[i] / 12)));
chart.addSeries('baseline', { baseValue: 30, color: '${RED}', downColor: '${GREEN}', pricePrecision: 0 }, chart.addPane(0.6))
  .setData({ time: s.time, value: score });
${legend([['Sentiment score', '#e6e9ef']], 1)}
// Advancing stocks around half the market
chart.addSeries('histogram', { baseValue: 2650, pricePrecision: 0 }, chart.addPane(0.5)).setData({
  time: s.time, value: s.advancers.map(Math.round), color: s.advancers.map((v) => (v >= 2650 ? '${RED}' : '${GREEN}')),
});
${legend([['Advancing stocks', '#8b93a7']], 2)}
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, step: true, pricePrecision: 0 }, chart.addPane(0.4))
  .setData({ time: s.time, value: s.streak.map(Math.round) });
${legend([['Highest limit-up streak, days', '#ffb547']], 3)}
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'A composite "money-making effect" score as a `baseline` series, built from advancing stocks (a `histogram` around half the market, using `baseValue`), the limit-up count and the highest consecutive limit-up streak.',
      api: ["addSeries('baseline')", 'baseValue', 'step lines'],
    },
    {
      id: 'ipo',
      title: 'IPO first-day returns',
      group: 'Market sentiment',
      code: `${create()}
${load('ipo', 'events/cnIpos', { ...INDEX, count: 500 })}
const pct = ipo.value.map((v) => v * 100);
// Gains and breaks below the offer price as two point series
chart.addSeries('points', { color: '${RED}', lineWidth: 7, pricePrecision: 0 }).setData({ time: ipo.time, value: pct.map((v) => (v >= 0 ? v : NaN)) });
chart.addSeries('points', { color: '${GREEN}', lineWidth: 7, pricePrecision: 0 }).setData({ time: ipo.time, value: pct.map((v) => (v < 0 ? v : NaN)) });
${legend([['First-day return, %', RED], ['Broke issue', GREEN]])}

// Share of the last 20 listings that closed below the offer price
const rate = pct.map((_, i) => (i < 19 ? NaN : (pct.slice(i - 19, i + 1).filter((v) => v < 0).length / 20) * 100));
chart.addSeries('area', { color: '#ffb547', topFill: 'rgba(255, 181, 71, 0.3)', bottomFill: 'rgba(255, 181, 71, 0)', pricePrecision: 0 }, chart.addPane(0.5))
  .setData({ time: ipo.time, value: rate });
${legend([['Break rate, last 20 IPOs, %', '#ffb547']], 1)}
chart.setBarSpacing(6);
chart.scrollToRealtime();`,
      about: 'Each new listing\'s first-day return as a point on its listing date, red for gains and green for breaks below the offer price, and the rolling break rate of the last 20 listings below.',
      api: ["addSeries('points')", "addSeries('area')"],
    },

    // ---------------------------------------------------------------- Corporate & derivatives
    {
      id: 'alift',
      title: 'Lock-up expiry calendar',
      group: 'Corporate & derivatives',
      code: `${create()}
${bars({ ...STOCK, count: 200 })}
${load('unlocks', 'events/cnUnlocks', { ...STOCK, count: 200, ahead: 120 })}
${candles()}
// Future sessions (weekdays) give the calendar a slot per day ahead
const days = [...bars.time];
for (let t = bars.time.at(-1); days.length < bars.time.length + 120; ) {
  t += 864e5;
  if (new Date(t).getUTCDay() % 6) days.push(t);
}
const value = new Map(unlocks.time.map((t, i) => [t, unlocks.value[i] / 1e6])); // CNY millions
const now = bars.time.at(-1);
const calendar = chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.6));
calendar.setData({
  time: days, value: days.map((t) => value.get(t) ?? NaN),
  color: days.map((t) => (t > now ? '#ffb547' : '#8b93a7')),
});
// Label the scheduled ones
calendar.setMarkers(unlocks.time.filter((t) => t > now).map((time) => ({
  time, position: 'aboveBar', shape: 'arrowDown', color: '#ffb547', text: value.get(time).toFixed(0) + 'm',
})));
${legend([['Unlocked market value, CNY m', '#8b93a7'], ['Scheduled', '#ffb547']], 1)}
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0 } }, 1);
chart.setVisibleLogicalRange(80, days.length + 2);`,
      about: 'Lock-up expiries of restricted shares under the stock: past ones in grey, the schedule for the next six months in amber. Data in the future extends the time axis, so the calendar continues past the last candle.',
      api: ["addSeries('histogram')", 'future data', 'setMarkers'],
    },
    {
      id: 'cbpremium',
      title: 'Convertible bond premiums',
      group: 'Corporate & derivatives',
      code: `${create()}
${bars({ ...STOCK, count: 400 })}
${load('cb', 'series/cnConvertible', { ...STOCK, count: 400 })}
${candles()}
const pane = chart.addPane(0.9);
const conv = chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2, pricePrecision: 1 }, pane);
const floor = chart.addSeries('line', { color: '#ffb547', lineWidth: 2, pricePrecision: 1 }, pane);
conv.setData({ time: cb.time, value: cb.conversion.map((v) => v * 100) });
floor.setData({ time: cb.time, value: cb.bondFloor.map((v) => v * 100) });
// Blue: trades like a bond (rich to conversion value); amber: trades like the stock
chart.addBand(conv, floor, { fill: 'rgba(76, 201, 240, 0.15)', fillCrossed: 'rgba(255, 181, 71, 0.2)' });
${legend([['Conversion premium, %', '#4cc9f0'], ['Premium to bond floor, %', '#ffb547']], 1)}
chart.applyPriceScaleOptions({ margins: { top: 0.2, bottom: 0.05 } }, pane);
chart.setBarSpacing(2.6);
chart.scrollToRealtime();`,
      about: 'A convertible bond\'s premium over its conversion value and over its straight-bond floor, with `addBand` coloring which one dominates: bond-like when the stock is weak, equity-like after rallies.',
      api: ['addBand', 'fillCrossed', 'addPane'],
    },
    {
      id: 'cbcall',
      title: 'Convertible call countdown',
      group: 'Corporate & derivatives',
      code: `${create()}
${bars({ ...STOCK, count: 400 })}
// The bond was issued 400 sessions ago, convertible at the stock price that day.
// Call clause: the stock closes at or above 130% of the conversion price on 15 of 30 days.
const conversion = bars.close[0];
const trigger = conversion * 1.3;
const counts = bars.close.map((_, i) => bars.close.slice(Math.max(0, i - 29), i + 1).filter((c) => c >= trigger).length);

const candles = chart.addCandlestickSeries(${CANDLES});
candles.setData(bars);
candles.createPriceLine({ price: conversion, color: '#8b93a7', lineStyle: 'dotted', title: 'Conversion price' });
candles.createPriceLine({ price: trigger, color: '#ffb547', lineStyle: 'dashed', title: 'Call trigger 130%' });
// The first day the clause is met: the issuer may now redeem the bond
const called = counts.findIndex((c) => c >= 15);
if (called >= 0) candles.setMarkers([{ time: bars.time[called], position: 'aboveBar', shape: 'arrowDown', color: '#ffb547', text: 'Call triggered' }]);

const days = chart.addSeries('histogram', { pricePrecision: 0 }, chart.addPane(0.5));
days.setData({ time: bars.time, value: counts, color: counts.map((c) => (c >= 15 ? '#ffb547' : '#ffb54766')) });
days.createPriceLine({ price: 15, color: '#ffb547', lineStyle: 'dashed', title: '15 of 30' });
${legend([['Days at or above trigger, last 30', '#ffb547']], 1)}
chart.applyPriceScaleOptions({ range: { low: 0, high: 30 } }, 1);
chart.setBarSpacing(2.6);
chart.scrollToRealtime();`,
      about: 'The stock under a convertible bond with its conversion price and 130% call trigger as price lines, the count of qualifying days in the last 30 as a `histogram`, and a marker where the 15-of-30 call clause is met.',
      api: ['createPriceLine', 'setMarkers', "addSeries('histogram')"],
    },
    {
      id: 'ahpremium',
      title: 'A/H share premium',
      group: 'Corporate & derivatives',
      code: `${create()}
${bars({ ...STOCK, count: 500 }, 'a')}
${bars({ symbol: 'CNAH', interval: '1d', count: 500 }, 'h')}
const HKD_CNY = 0.92; // fixed for the example
// Match the two listings by date and convert the H share to CNY
const hClose = new Map(h.time.map((t, i) => [t, h.close[i] * HKD_CNY]));
const time = a.time.filter((t) => hClose.has(t));
const aPrice = time.map((t) => a.close[a.time.indexOf(t)]);
const hPrice = time.map((t) => hClose.get(t));

chart.addSeries('line', { color: '${RED}', lineWidth: 2 }).setData({ time, value: aPrice });
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2 }).setData({ time, value: hPrice });
${legend([['A share, CNY', RED], ['H share in CNY', '#4cc9f0']])}
const premium = aPrice.map((v, i) => (v / hPrice[i] - 1) * 100);
chart.addSeries('baseline', { baseValue: 0, color: '#ffb547', downColor: '#4cc9f0', pricePrecision: 1 }, chart.addPane(0.7))
  .setData({ time, value: premium });
${legend([['A/H premium, %', '#ffb547']], 1)}
chart.setBarSpacing(2.2);
chart.scrollToRealtime();`,
      about: 'The same company listed in two markets: the A share and the H share converted to CNY, and the premium of A over H as a `baseline` series around zero.',
      api: ["addSeries('baseline')", "addSeries('line')"],
    },
    {
      id: 'warrantcn',
      title: 'Warehouse receipts',
      group: 'Corporate & derivatives',
      code: `${create()}
${bars({ symbol: 'ZS', interval: '1d', count: 250 })}
${load('wr', 'series/cnWarrants', { symbol: 'ZS', interval: '1d', count: 250 })}
${candles()}
const kt = wr.receipts.map((v) => v / 1000); // thousand tonnes
chart.addSeries('area', { color: '#4cc9f0', topFill: 'rgba(76, 201, 240, 0.25)', bottomFill: 'rgba(76, 201, 240, 0)', pricePrecision: 1 }, chart.addPane(0.5))
  .setData({ time: wr.time, value: kt });
${legend([['Registered receipts, kt', '#4cc9f0']], 1)}
// Daily change: red for additions, green for cancellations
const change = kt.map((v, i) => (i ? v - kt[i - 1] : NaN));
chart.addSeries('histogram', { pricePrecision: 1 }, chart.addPane(0.5)).setData({ time: wr.time, value: change, color: ${redGreen('change')} });
${legend([['Daily change, kt', '#8b93a7']], 2)}
chart.setBarSpacing(4);
chart.scrollToRealtime();`,
      about: 'A futures contract with its registered warehouse receipts, deliverable stock in exchange warehouses, and their daily change in red and green: receipts pile up as the price falls.',
      api: ["addSeries('area')", "addSeries('histogram')", 'per-point color'],
    },
  ],
};
