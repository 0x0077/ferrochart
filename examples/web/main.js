// FerroChart web demo.
//
// Build the package first, then serve the repository root:
//   (cd crates/ferrochart-wasm && wasm-pack build --release --target web)
//   python3 -m http.server 8000      # open http://localhost:8000/examples/web/
//
// URL parameters: ?renderer=auto|webgpu|webgl2  &bars=<n>  &theme=dark|light  &live=0
//
// Also shows the JS plugin API: a watermark layer (`addLayer`) and a
// "price range" drawing tool (`drawings.register`, armed by the Range button).
import init, { FerroChart, randomWalk } from '../../crates/ferrochart-wasm/pkg/ferrochart_wasm.js';

const MINUTE = 60_000;
const START = 1_104_537_600_000; // 2005-01-01T00:00:00Z
const params = new URLSearchParams(location.search);
const $ = (id) => document.getElementById(id);
const canvas = $('chart');

// Exposed for the automated browser test (tests/web/smoke.mjs).
window.__ferro = { ready: false, error: null, renderer: null };

let chart = null;
let candles = null;
let data = null;
let last = null;
let feed = null;

function fmt(v) {
  return v == null ? '—' : v.toFixed(2);
}

function showError(e) {
  const box = $('error');
  box.textContent = `Cannot create the chart: ${e?.message ?? e}`;
  box.style.display = 'grid';
  window.__ferro.error = String(e?.message ?? e);
}

function generate(n) {
  const t0 = performance.now();
  data = randomWalk(2026, n, START, MINUTE);
  const i = data.close.length - 1;
  last = {
    time: Number(data.time[i]),
    open: data.open[i], high: data.high[i], low: data.low[i], close: data.close[i],
    volume: data.volume[i],
  };
  return performance.now() - t0;
}

async function build() {
  feed && clearInterval(feed);
  chart?.destroy();
  chart = null;
  const theme = $('theme').value;
  document.documentElement.style.colorScheme = theme;
  document.body.style.background = theme === 'dark' ? '#131722' : '#ffffff';
  document.body.style.color = theme === 'dark' ? '#d1d4dc' : '#131722';
  chart = await FerroChart.create(canvas, {
    theme,
    renderer: params.get('renderer') ?? 'auto',
  });
  candles = chart.addCandlestickSeries();
  chart.addVolumeSeries(candles);
  const t0 = performance.now();
  candles.setData(data);
  const setDataMs = performance.now() - t0;
  chart.setCrosshairMode($('crosshair').value);
  chart.addLayer(watermark);
  chart.drawings.register('demo_price_range', priceRange);
  chart.on('crosshairMove', ({ point }) => {
    const b = point?.bar;
    $('legend').textContent = b
      ? `O ${fmt(b.open)}  H ${fmt(b.high)}  L ${fmt(b.low)}  C ${fmt(b.close)}  V ${b.volume?.toFixed(0) ?? '—'}`
      : '';
  });
  $('stats').textContent =
    `${chart.renderer} · ${data.close.length.toLocaleString()} bars · setData ${setDataMs.toFixed(0)} ms`;
  startFeed();
  Object.assign(window.__ferro, { ready: true, renderer: chart.renderer, setDataMs });
}

// A chart-wide layer behind the series. `build` runs every frame and returns
// drawing commands in CSS px.
const watermark = {
  position: 'background',
  build(ctx) {
    const r = ctx.rect;
    return [{
      type: 'text', x: r.x + r.width / 2, y: r.y + r.height / 2,
      text: 'FERROCHART', size: 64, align: 'center', color: '#80808030',
    }];
  },
};

// A two-click drawing tool: a box between the anchors labelled with the price
// change and the number of bars. Anchors carry time/price and their x/y.
const priceRange = {
  anchorCount: 2,
  build(ctx) {
    const [p, q] = ctx.anchors;
    if (!p || !q || p.x === null || q.x === null || p.y === null || q.y === null) return [];
    const up = q.price >= p.price;
    const color = up ? '#26a69a' : '#ef5350';
    const x = Math.min(p.x, q.x), y = Math.min(p.y, q.y);
    const w = Math.abs(q.x - p.x), h = Math.abs(q.y - p.y);
    const change = q.price - p.price;
    const pct = (100 * change) / p.price;
    const bars = Math.round(Math.abs(q.time - p.time) / MINUTE);
    const sign = up ? '+' : '';
    return [
      { type: 'rect', x, y, width: w, height: h, color: `${color}30` },
      { type: 'line', x1: x + w / 2, y1: p.y, x2: x + w / 2, y2: q.y, color, width: 1 },
      {
        type: 'text', x: x + w / 2, y: up ? y - 14 : y + h + 14, align: 'center', color: '#ffffff',
        text: `${sign}${ctx.formatPrice(change)} (${sign}${pct.toFixed(2)}%) · ${bars} bars`,
        background: color, padding: 4,
      },
    ];
  },
};

function startFeed() {
  feed && clearInterval(feed);
  feed = null;
  if (!$('live').checked) return;
  let ticks = 0;
  feed = setInterval(() => {
    if (++ticks >= 100) { // a new one-minute bar every second
      ticks = 0;
      const c = last.close;
      last = { time: last.time + MINUTE, open: c, high: c, low: c, close: c, volume: 0 };
    }
    const close = Math.max(0.01, last.close * (1 + 0.0004 * (Math.random() * 2 - 1)));
    last.close = close;
    last.high = Math.max(last.high, close);
    last.low = Math.min(last.low, close);
    last.volume += 1 + Math.random();
    candles.update(last);
  }, 10);
}

async function main() {
  await init();
  const bars = Number(params.get('bars') ?? $('bars').value);
  $('bars').value = String(bars);
  $('theme').value = params.get('theme') ?? 'dark';
  $('live').checked = params.get('live') !== '0';
  generate(bars);
  await build();
  $('bars').onchange = async () => { generate(Number($('bars').value)); await build(); };
  $('theme').onchange = build;
  $('crosshair').onchange = () => chart.setCrosshairMode($('crosshair').value);
  $('live').onchange = startFeed;
  $('fit').onclick = () => chart.fitContent();
  $('range').onclick = () => { chart.drawings.startTool('demo_price_range'); canvas.focus(); };
  canvas.focus();
}

main().catch(showError);
