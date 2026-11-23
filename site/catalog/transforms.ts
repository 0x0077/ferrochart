import { bars, create, load, view } from './build';
import type { Category } from './types';

const transform = (name: string, params: string, spacing: number, comment = '') =>
  [create(), bars({ symbol: 'BTCUSD', interval: '5m', count: 4000 }), `chart.addTransformSeries('${name}', ${params}).setData(bars);${comment}`, view(spacing)].join('\n');

/** Seconds each bar took to form, as a histogram in a pane below. */
const DURATION = `// How long each bar took to form, in seconds.
const seconds = bars.time.map((t, i) => (i === 0 ? NaN : (t - bars.time[i - 1]) / 1000));
chart.addSeries('histogram', { color: '#b388ff', pricePrecision: 0 }, chart.addPane(0.45))
  .setData({ time: bars.time, value: seconds });`;

export const transforms: Category = {
  id: 'transforms',
  title: 'Bar types & transforms',
  kicker: '02',
  blurb: 'Bars cut by price, trades, volume or notional instead of the clock: Renko, Kagi, point & figure, line break, tick, volume and dollar bars, delta candles, spreads and continuous contracts.',
  accent: '#b388ff',
  demos: [
    {
      id: 'renko',
      title: 'Renko',
      group: 'Price-driven',
      code: transform('renko', '{ boxSize: 150 }', 9, ' // bars in, bricks out'),
      about: 'Bricks of a fixed (or ATR-based) size, built from your bars as they arrive. The forming brick updates live with `update`.',
      api: ["addTransformSeries('renko')"],
    },
    {
      id: 'range',
      title: 'Range bars',
      group: 'Price-driven',
      code: transform('rangeBars', '{ range: 200 }', 8),
      about: 'Every bar spans the same price range, so quiet periods shrink and busy ones expand.',
      api: ["addTransformSeries('rangeBars')"],
    },
    {
      id: 'kagi',
      title: 'Kagi',
      group: 'Price-driven',
      code: transform('kagi', '{ reversalPercent: 1 }', 10),
      about: 'A line that changes direction on reversals and thickness when it breaks the previous shoulder or waist.',
      api: ["addTransformSeries('kagi')"],
    },
    {
      id: 'pnf',
      title: 'Point & figure',
      group: 'Price-driven',
      code: transform('pointFigure', '{ boxSize: 150, reversal: 3 }', 12),
      about: 'Columns of X and O boxes with a three-box reversal: supply and demand without the noise of time.',
      api: ["addTransformSeries('pointFigure')"],
    },
    {
      id: 'linebreak',
      title: 'Three-line break',
      group: 'Price-driven',
      code: `${create()}
// One-hour bars turned into line-break blocks on the server.
${load('blocks', 'altbars', { type: 'lineBreak', symbol: 'BTCUSD', interval: '1h', count: 3000, lines: 3 })}
chart.addCandlestickSeries().setData(blocks);
${view(10)}`,
      about: 'A new block only when the close beats the previous block; a reversal has to break the extreme of the last three. The server computes the blocks and a candlestick series shows them in order, one slot each.',
      api: ['addCandlestickSeries', 'setData'],
    },
    {
      id: 'tickbars',
      title: 'Tick bars',
      group: 'Activity-driven',
      code: `${create()}
// A bar every 150 trades over the last two hours.
${load('bars', 'altbars', { type: 'tick', symbol: 'BTCUSD', size: 150, span: 7_200_000 })}
const candles = chart.addCandlestickSeries();
candles.setData(bars);
chart.addVolumeSeries(candles, chart.addPane(0.45));
${view(9)}`,
      about: 'Every bar holds the same number of trades, so the volume pane below shows how much each batch of 150 trades moved: large prints stand out. Times are those of each bar\'s last trade.',
      api: ['addCandlestickSeries', 'addVolumeSeries', 'addPane'],
    },
    {
      id: 'volbars',
      title: 'Volume bars',
      group: 'Activity-driven',
      code: `${create()}
// A bar every 40 BTC traded.
${load('bars', 'altbars', { type: 'volume', symbol: 'BTCUSD', size: 40, span: 7_200_000 })}
chart.addCandlestickSeries().setData(bars);
${DURATION}
${view(9)}`,
      about: 'A bar closes every 40 BTC of volume, so each carries the same weight. The pane below shows the seconds each bar took: short when the market is busy, long when it is quiet.',
      api: ['addCandlestickSeries', "addSeries('histogram')"],
    },
    {
      id: 'dollarbars',
      title: 'Dollar bars',
      group: 'Activity-driven',
      code: `${create()}
// A bar every $2.5 million traded.
${load('bars', 'altbars', { type: 'dollar', symbol: 'BTCUSD', size: 2_500_000, span: 7_200_000 })}
chart.addCandlestickSeries().setData(bars);
${DURATION}
${view(9)}`,
      about: 'Bars of equal notional (price × size), which stay comparable when the price level moves a lot. The pane below shows the seconds each bar took to fill.',
      api: ['addCandlestickSeries', "addSeries('histogram')"],
    },
    {
      id: 'deltacandles',
      title: 'Delta candles',
      group: 'Derived series',
      code: `${create()}
const to = Math.floor(Date.now() / 60_000) * 60_000; // the last whole minute
const from = to - 120 * 60_000;
const bars = await fetch(\`/api/bars?symbol=BTCUSD&interval=1m&from=\${from}&to=\${to}\`).then((r) => r.json());
// Cumulative volume delta (buy minus sell volume), as OHLC per minute.
${load('delta', 'altbars', { type: 'delta', symbol: 'BTCUSD', interval: '1m', span: 7_200_000 })}
chart.addCandlestickSeries().setData(bars);
chart.addCandlestickSeries({ color: '#4cc9f0', downColor: '#ffb547', pricePrecision: 1 }, chart.addPane(0.8)).setData(delta);
${view(9)}`,
      about: 'One-minute candles above; below, cumulative volume delta (buy minus sell volume, in BTC) drawn as a candle per minute: open where the last minute ended, wicks at the extremes the delta reached inside the minute. Two candlestick series in two panes.',
      api: ['addCandlestickSeries', 'addPane'],
    },
    {
      id: 'spread',
      title: 'Ratio with bands',
      group: 'Derived series',
      live: true,
      code: `${create()}
${bars({ symbol: 'ETHUSD', interval: '1h', count: 1440, live: true }, 'eth')}
${bars({ symbol: 'BTCUSD', interval: '1h', count: 1440, live: true }, 'btc')}
const ratio = eth.close.map((c, i) => c / btc.close[i]);
// Rolling 100-bar mean and two standard deviations.
const n = 100, mean = [], upper = [], lower = [];
ratio.forEach((_, i) => {
  if (i < n) return mean.push(NaN), upper.push(NaN), lower.push(NaN);
  const w = ratio.slice(i - n, i);
  const m = w.reduce((a, b) => a + b) / n;
  const sd = Math.sqrt(w.reduce((a, b) => a + (b - m) ** 2, 0) / n);
  mean.push(m), upper.push(m + 2 * sd), lower.push(m - 2 * sd);
});
const line = (value, o) => {
  const s = chart.addSeries('line', { pricePrecision: 5, lastValueVisible: false, priceLineVisible: false, ...o });
  s.setData({ time: eth.time, value });
  return s;
};
const hi = line(upper, { color: '#b388ff80', lineWidth: 1 });
const lo = line(lower, { color: '#b388ff80', lineWidth: 1 });
chart.addBand(hi, lo, { fill: 'rgba(179, 136, 255, 0.10)' });
line(mean, { color: '#8b93a7', lineStyle: 'dashed' });
line(ratio, { color: '#e6e9ef', lineWidth: 2, lastValueVisible: true });
${view(5)}`,
      about: 'ETH priced in BTC: the ratio of two markets\' closes, with its 100-bar mean and a two-standard-deviation band. Stretches outside the band are where pairs traders look. The ratio and bands are a few lines of JS; `addBand` fills between them.',
      api: ["addSeries('line')", 'addBand'],
    },
    {
      id: 'rollover',
      title: 'Continuous futures',
      group: 'Derived series',
      code: `${create()}
${bars({ symbol: 'ES', interval: '1d', count: 750 })}
// Quarterly rolls: next contract minus the expiring one, in points.
${load('rolls', 'events/contractRolls', { symbol: 'ES', interval: '1d', count: 750 })}
// The bars are back-adjusted: each earlier contract traded lower by the gaps still to come.
const adjustment = bars.time.map((t) => rolls.time.reduce((sum, r, k) => (r > t ? sum + rolls.value[k] : sum), 0));
const candles = chart.addCandlestickSeries();
candles.setData(bars);
chart.addSeries('line', { color: '#ffb547', lineWidth: 2, lastValueVisible: false, priceLineVisible: false })
  .setData({ time: bars.time, value: bars.close.map((c, i) => c - adjustment[i]) });
candles.setMarkers(rolls.time.map((time, k) => ({
  time, position: 'aboveBar', shape: 'arrowDown', color: '#ffb547', text: \`Roll +\${rolls.value[k].toFixed(1)}\`,
})));
// The adjustment added to each bar, in points.
chart.addSeries('area', { color: '#ffb547', step: true, topFill: '#ffb54740', bottomFill: '#ffb54708', pricePrecision: 1 }, chart.addPane(0.4))
  .setData({ time: bars.time, value: adjustment });
${view(4)}`,
      about: 'A continuous futures contract as back-adjusted candles, with the unadjusted front-month close in amber. At each quarterly roll (marked) the unadjusted price jumps by the gap between contracts; back-adjusting shifts history by the step series in the lower pane, so returns stay true.',
      api: ['addCandlestickSeries', 'setMarkers', "addSeries('line')"],
    },
  ],
};
