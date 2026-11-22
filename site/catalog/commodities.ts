import { bars, create, load } from './build';
import { plotDemos } from './plots';
import type { Category } from './types';

const CL = { symbol: 'CL', interval: '1d' };
const CLW = { symbol: 'CL', interval: '1w' };

export const commodities: Category = {
  id: 'commodities',
  title: 'Futures & commodities',
  kicker: '14',
  blurb: 'Spreads, crack and crush margins, positioning, inventories and weather.',
  accent: '#f59e0b',
  demos: [
    {
      id: 'basis',
      title: 'Futures basis',
      group: 'Curves & positioning',
      code: `${create()}
${bars({ ...CL, count: 400 })}
${load('basis', 'series/clBasis', { ...CL, count: 400 })}
chart.addCandlestickSeries().setData(bars);
const pos = { color: '#2ee6b6', downColor: '#ff5d73', topFill: 'rgba(46, 230, 182, 0.2)', bottomFill: 'rgba(255, 93, 115, 0.22)' };
// Futures minus spot, USD per barrel: below zero the curve is in backwardation
chart.addSeries('baseline', { ...pos, baseValue: 0 }, chart.addPane(0.6))
  .setData({ time: basis.time, value: basis.basis });
// Annualised against a front month about 30 days out, percent
chart.addSeries('line', { color: '#f59e0b', lineWidth: 1.5, pricePrecision: 1 }, chart.addPane(0.5))
  .setData({ time: basis.time, value: basis.basis.map((b, i) => (b / bars.close[i]) * (365 / 30) * 100) });
chart.setBarSpacing(5);
chart.scrollToRealtime();`,
      about: 'Daily crude oil candles over the basis, front-month futures minus spot in USD per barrel, as a baseline series: red while the market is in backwardation. The bottom pane annualises it as a percent of the price.',
      api: ["addSeries('baseline')", 'addPane'],
    },
    {
      id: 'cot',
      title: 'Positioning report',
      group: 'Curves & positioning',
      code: `${create()}
${bars({ ...CLW, count: 200 })}
${load('cot', 'series/cotCrude', { ...CLW, count: 200 })}
chart.addCandlestickSeries().setData(bars);
// Net futures positions by trader category, thousands of contracts
const k = (col) => cot[col].map((v) => v / 1000);
const nets = chart.addPane(0.9);
const managed = chart.addSeries('line', { color: '#f59e0b', lineWidth: 2, pricePrecision: 0 }, nets);
managed.setData({ time: cot.time, value: k('managedMoney') });
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 2, pricePrecision: 0 }, nets).setData({ time: cot.time, value: k('swapDealers') });
chart.addSeries('line', { color: '#b388ff', lineWidth: 1.5, pricePrecision: 0 }, nets).setData({ time: cot.time, value: k('otherReportables') });
managed.createPriceLine({ price: 0, color: '#8a93a6', lineStyle: 'dashed', axisLabelVisible: false });
// What managed money bought or sold each week
const change = k('managedMoney').map((v, i, a) => (i ? v - a[i - 1] : NaN));
chart.addSeries('histogram', { pricePrecision: 0, priceLineVisible: false }, chart.addPane(0.5)).setData({
  time: cot.time,
  value: change,
  color: change.map((v) => (v >= 0 ? '#2ee6b6' : '#ff5d73')),
});
chart.setBarSpacing(6);
chart.scrollToRealtime();`,
      about: 'Weekly crude oil candles over the weekly positioning report: net positions of managed money (amber), swap dealers (blue) and other reportables (violet) in thousands of contracts, and managed money\'s weekly change as a signed histogram.',
      api: ["addSeries('line')", "addSeries('histogram')", 'per-point color'],
    },
    {
      id: 'invprice',
      title: 'Inventories vs price',
      group: 'Curves & positioning',
      code: `${create()}
${bars({ ...CLW, count: 160 })}
${load('inv', 'series/clInventory', { ...CLW, count: 160 })}
chart.addCandlestickSeries().setData(bars);
// Commercial stocks, million barrels
chart.addSeries('area', { color: '#f59e0b', topFill: 'rgba(245, 158, 11, 0.25)', bottomFill: 'rgba(245, 158, 11, 0.02)', pricePrecision: 1 }, chart.addPane(0.6))
  .setData({ time: inv.time, value: inv.stocks });
// Weekly build (up) or draw (down)
const change = inv.stocks.map((v, i, a) => (i ? v - a[i - 1] : NaN));
chart.addSeries('histogram', { pricePrecision: 1, priceLineVisible: false }, chart.addPane(0.5)).setData({
  time: inv.time,
  value: change,
  color: change.map((v) => (v >= 0 ? '#ff5d73' : '#2ee6b6')),
});
chart.setBarSpacing(7);
chart.scrollToRealtime();`,
      about: 'Weekly crude oil candles over commercial inventories in million barrels and the weekly change: builds in red, draws in green. Falling stocks tend to come with a rising price.',
      api: ["addSeries('area')", "addSeries('histogram')", 'addPane'],
    },
    {
      id: 'crack',
      title: '3-2-1 crack spread',
      group: 'Spreads & margins',
      code: `${create()}
${bars({ ...CL, count: 500 }, 'crude')}
${bars({ symbol: 'RB', interval: '1d', count: 500 }, 'gasoline')}
${bars({ symbol: 'HO', interval: '1d', count: 500 }, 'heatingOil')}
// Products are quoted per gallon: 42 gallons to the barrel
const perBarrel = (b) => b.close.map((v) => v * 42);
const gas = perBarrel(gasoline), diesel = perBarrel(heatingOil);
// Three barrels of crude make two of gasoline and one of distillate
const crack = crude.close.map((c, i) => (2 * gas[i] + diesel[i] - 3 * c) / 3);
const avg = crack.reduce((a, b) => a + b) / crack.length;
const legs = [[crude.close, '#e6e9ef'], [gas, '#f59e0b'], [diesel, '#4cc9f0']];
for (const [value, color] of legs) chart.addSeries('line', { color, lineWidth: 1.5 }).setData({ time: crude.time, value });
chart.addSeries('baseline', {
  baseValue: avg,
  color: '#2ee6b6', downColor: '#ff5d73',
  topFill: 'rgba(46, 230, 182, 0.22)', bottomFill: 'rgba(255, 93, 115, 0.22)',
}, chart.addPane(0.8)).setData({ time: crude.time, value: crack });
chart.setBarSpacing(2);
chart.scrollToRealtime();`,
      about: 'A refiner\'s margin from three futures: crude (white), gasoline (amber) and heating oil (blue) in USD per barrel above, and the 3-2-1 crack spread below as a baseline series around its average, computed in JavaScript from the closes.',
      api: ["addSeries('baseline')", 'baseValue', "addSeries('line')"],
    },
    {
      id: 'crush',
      title: 'Soybean crush margin',
      group: 'Spreads & margins',
      code: `${create()}
${bars({ symbol: 'ZS', interval: '1d', count: 500 }, 'beans')}
${bars({ symbol: 'ZM', interval: '1d', count: 500 }, 'meal')}
${bars({ symbol: 'ZL', interval: '1d', count: 500 }, 'oil')}
// A bushel (cents) yields 0.022 short tons of meal (USD per ton) and 11 lb of oil (cents per lb)
const crush = beans.close.map((b, i) => 0.022 * meal.close[i] + (11 * oil.close[i]) / 100 - b / 100);
const avg = crush.reduce((a, b) => a + b) / crush.length;
chart.addSeries('baseline', {
  baseValue: avg,
  color: '#2ee6b6', downColor: '#ff5d73',
  topFill: 'rgba(46, 230, 182, 0.22)', bottomFill: 'rgba(255, 93, 115, 0.22)',
}).setData({ time: beans.time, value: crush });
// The three legs, rebased to 100
const legs = chart.addPane(0.8);
const series = [[beans, '#f59e0b'], [meal, '#b388ff'], [oil, '#4cc9f0']];
for (const [b, color] of series) chart.addSeries('line', { color, lineWidth: 1.5 }, legs).setData({ time: b.time, value: b.close });
chart.applyPriceScaleOptions({ mode: 'indexedTo100' }, legs);
chart.setBarSpacing(2);
chart.scrollToRealtime();`,
      about: 'The board crush, what a processor earns turning a bushel of soybeans into meal and oil, in USD per bushel as a baseline series around its average. Below, soybeans (amber), meal (violet) and oil (blue) rebased to 100 with `indexedTo100` on that pane only.',
      api: ["addSeries('baseline')", "applyPriceScaleOptions({ mode: 'indexedTo100' }, pane)"],
    },
    {
      id: 'freight',
      title: 'Freight and origin spread',
      group: 'Spreads & margins',
      code: `${create()}
${bars({ symbol: 'ZS', interval: '1d', count: 500 })}
${load('freight', 'series/freight', { symbol: 'ZS', interval: '1d', count: 500 })}
chart.addSeries('area', { color: '#f59e0b', topFill: 'rgba(245, 158, 11, 0.28)', bottomFill: 'rgba(245, 158, 11, 0)', lineWidth: 2, pricePrecision: 0 })
  .setData({ time: freight.time, value: freight.index });
// Origin B minus origin A, USD per tonne: positive, B is the dearer origin
chart.addSeries('baseline', {
  baseValue: 0, pricePrecision: 1,
  color: '#4cc9f0', downColor: '#b388ff',
  topFill: 'rgba(76, 201, 240, 0.22)', bottomFill: 'rgba(179, 136, 255, 0.22)',
}, chart.addPane(0.7)).setData({ time: freight.time, value: freight.spread });
chart.addSeries('line', { color: '#e6e9ef', lineWidth: 1.5 }, chart.addPane(0.6)).setData({ time: bars.time, value: bars.close });
chart.setBarSpacing(2);
chart.scrollToRealtime();`,
      about: 'A dry bulk freight index (points), the export price spread between two soybean origins in USD per tonne as a baseline series, and soybean futures below, all on one time axis and crosshair.',
      api: ["addSeries('area')", "addSeries('baseline')", 'addPane'],
    },
    {
      id: 'residualload',
      title: 'Residual load',
      group: 'Energy & environment',
      code: `${create()}
${load('grid', 'series/powerLoad', { symbol: 'NG', interval: '1d', count: 365 })}
${bars({ symbol: 'NG', interval: '1d', count: 365 })}
// Daily averages, GW: demand, minus solar, minus wind is what thermal plants must cover
const line = (color, value, width = 1) => {
  const s = chart.addSeries('line', { color, lineWidth: width, pricePrecision: 1, priceLineVisible: false });
  s.setData({ time: grid.time, value });
  return s;
};
const demand = line('#e6e9ef', grid.load, 2);
const afterSolar = line('#f59e0b', grid.load.map((v, i) => v - grid.solar[i]));
const residual = chart.addSeries('area', {
  color: '#ff5d73', topFill: 'rgba(255, 93, 115, 0.3)', bottomFill: 'rgba(255, 93, 115, 0.05)', lineWidth: 2, pricePrecision: 1,
});
residual.setData({ time: grid.time, value: grid.load.map((v, i) => v - grid.solar[i] - grid.wind[i]) });
chart.addBand(demand, afterSolar, { fill: 'rgba(245, 158, 11, 0.35)' });
chart.addBand(afterSolar, residual, { fill: 'rgba(76, 201, 240, 0.3)' });
chart.addSeries('line', { color: '#4cc9f0', lineWidth: 1.5 }, chart.addPane(0.5)).setData({ time: bars.time, value: bars.close });
chart.setBarSpacing(3);
chart.scrollToRealtime();`,
      about: 'A year of grid demand (white) with solar (amber) and wind (blue) taken off it as `addBand` fills, leaving the residual load (red) that gas and coal plants must supply. Natural gas futures below move with it.',
      api: ['addBand', "addSeries('area')", 'addPane'],
    },
    {
      id: 'carbon',
      title: 'Carbon price · allowance balance',
      group: 'Energy & environment',
      code: `${create()}
${bars({ symbol: 'EUA', interval: '1w', count: 156 })}
${load('bal', 'series/euaBalance', { symbol: 'EUA', interval: '1w', count: 156 })}
chart.addCandlestickSeries().setData(bars);
// Allowances issued minus verified emissions, Mt CO2 a week
chart.addSeries('histogram', { pricePrecision: 1, priceLineVisible: false }, chart.addPane(0.6)).setData({
  time: bal.time,
  value: bal.balance,
  color: bal.balance.map((v) => (v >= 0 ? '#2ee6b6' : '#ff5d73')),
});
// The bank: the running surplus since the first week
let bank = 0;
chart.addSeries('baseline', {
  baseValue: 0, pricePrecision: 0,
  color: '#2ee6b6', downColor: '#ff5d73',
  topFill: 'rgba(46, 230, 182, 0.2)', bottomFill: 'rgba(255, 93, 115, 0.22)',
}, chart.addPane(0.6)).setData({ time: bal.time, value: bal.balance.map((v) => (bank += v)) });
chart.setBarSpacing(7);
chart.scrollToRealtime();`,
      about: 'Weekly carbon allowance candles over the market\'s balance: allowances issued minus verified emissions each week (red: a deficit), and the running surplus as a baseline series. Deficits tend to lift the price.',
      api: ["addSeries('histogram')", "addSeries('baseline')", 'per-point color'],
    },
    {
      id: 'weatherdd',
      title: 'Heating and cooling degree days',
      group: 'Energy & environment',
      code: `${create()}
${bars({ symbol: 'NG', interval: '1d', count: 500 })}
${load('temp', 'series/tempAnomaly', { symbol: 'NG', interval: '1d', count: 500 })}
chart.addCandlestickSeries().setData(bars);
// Daily mean temperature, °F: a seasonal normal plus the anomaly
const normal = (t) => 55 - 22 * Math.cos((2 * Math.PI * (t / 86_400_000 - 20)) / 365.25);
const mean = temp.time.map((t, i) => normal(t) + temp.anomaly[i]);
// Degree days against 65 °F: heating below, cooling above
const pane = chart.addPane(0.8);
const bar = { pricePrecision: 0, lastValueVisible: false, priceLineVisible: false };
chart.addSeries('histogram', { ...bar, color: '#4cc9f0' }, pane).setData({ time: temp.time, value: mean.map((m) => Math.max(65 - m, 0)) });
chart.addSeries('histogram', { ...bar, color: '#f59e0b' }, pane).setData({ time: temp.time, value: mean.map((m) => Math.max(m - 65, 0)) });
chart.addSeries('line', { ...bar, color: '#e6e9ef', lineStyle: 'dashed' }, pane)
  .setData({ time: temp.time, value: temp.time.map((t) => Math.abs(65 - normal(t))) });
chart.setBarSpacing(3);
chart.scrollToRealtime();`,
      about: 'Natural gas candles over daily heating (blue) and cooling (amber) degree days, the gap between the mean temperature and 65 °F, with the seasonal normal dashed. Colder-than-normal winters lift heating demand and the price.',
      api: ["addSeries('histogram')", 'addPane', 'lineStyle'],
    },
    ...plotDemos.commodities,
  ],
};
