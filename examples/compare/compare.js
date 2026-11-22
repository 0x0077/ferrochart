// Same-scenario comparison of FerroChart, lightweight-charts and Vela.
// See README.md in this directory.
//
// The page exposes `window.compare` for the automated runner (bench.mjs):
//   await compare.load(lib, bars)   -> { firstPaintMs }
//   compare.setVisibleBars(k)       -> show the newest k bars
//   compare.startRecording() / compare.stopRecording() -> frame statistics
// Input is driven from outside (Playwright's trusted mouse events), so every
// library sees exactly the same events.
import init, { FerroChart, randomWalk } from '../../crates/ferrochart-wasm/pkg/ferrochart_wasm.js';

const MINUTE = 60_000;
const START = 1_104_537_600_000;
const stage = document.getElementById('stage');
const $ = (id) => document.getElementById(id);

let data = null;
let current = null; // { lib, setVisibleBars, destroy }

/** Same bars for everyone: FerroChart's deterministic generator. */
function bars(n) {
  if (data?.n !== n) {
    const d = randomWalk(2026, n, START, MINUTE);
    data = { n, raw: d, timesMs: Array.from(d.time, Number) };
  }
  return data;
}

/** Resolves after the next frame has been presented (two rAFs). */
const painted = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));

const LIBS = {
  async ferrochart(d) {
    const canvas = document.createElement('canvas');
    stage.append(canvas);
    const t0 = performance.now();
    const chart = await FerroChart.create(canvas, { theme: 'dark' });
    const s = chart.addCandlestickSeries();
    chart.addVolumeSeries(s);
    s.setData(d.raw);
    await painted();
    const firstPaintMs = performance.now() - t0;
    return {
      firstPaintMs,
      detail: chart.renderer,
      setVisibleBars(k) {
        // Plot width ≈ canvas width minus the price axis.
        chart.setBarSpacing(Math.max(1e-3, (canvas.clientWidth - 60) / k));
        chart.scrollToRealtime();
      },
      destroy() { chart.destroy(); canvas.remove(); },
    };
  },

  async 'lightweight-charts'(d) {
    // Input format conversion (objects, seconds) is not timed.
    const n = d.n;
    const candles = new Array(n);
    const volumes = new Array(n);
    for (let i = 0; i < n; i++) {
      const time = d.timesMs[i] / 1000;
      const o = d.raw.open[i];
      const c = d.raw.close[i];
      candles[i] = { time, open: o, high: d.raw.high[i], low: d.raw.low[i], close: c };
      volumes[i] = { time, value: d.raw.volume[i], color: c >= o ? '#26a69a80' : '#ef535080' };
    }
    const el = document.createElement('div');
    stage.append(el);
    const LW = window.LightweightCharts;
    const t0 = performance.now();
    const chart = LW.createChart(el, {
      autoSize: true,
      layout: { background: { color: '#131722' }, textColor: '#d1d4dc' },
      grid: { vertLines: { color: '#2a2e39' }, horzLines: { color: '#2a2e39' } },
      crosshair: { mode: LW.CrosshairMode.Magnet },
      // Allow zooming out as far as the others (default minimum is 0.5 px).
      timeScale: { minBarSpacing: 0.001 },
    });
    const series = chart.addSeries(LW.CandlestickSeries, {});
    const vol = chart.addSeries(LW.HistogramSeries, { priceScaleId: 'vol', priceFormat: { type: 'volume' } });
    chart.priceScale('vol').applyOptions({ scaleMargins: { top: 0.75, bottom: 0 } });
    series.setData(candles);
    vol.setData(volumes);
    await painted();
    const firstPaintMs = performance.now() - t0;
    return {
      firstPaintMs,
      detail: `v${LW.version()}`,
      setVisibleBars(k) {
        chart.timeScale().setVisibleLogicalRange({ from: n - k, to: n - 1 });
      },
      destroy() { chart.remove(); el.remove(); },
    };
  },

  async vela(d) {
    const n = d.n;
    const ohlcv = new Array(n);
    for (let i = 0; i < n; i++) {
      ohlcv[i] = {
        time: d.timesMs[i], open: d.raw.open[i], high: d.raw.high[i],
        low: d.raw.low[i], close: d.raw.close[i], volume: d.raw.volume[i],
      };
    }
    const el = document.createElement('div');
    stage.append(el);
    const t0 = performance.now();
    const chart = new window.Vela.Vela(el, {
      data: ohlcv,
      timeframe: '1',
      theme: 'dark',
      drawings: false,
      // Eased motions would spread one input over many frames; turn them off
      // so every library renders each input immediately.
      animations: { zoom: false, pan: false, autoscale: false, intro: false },
    });
    await chart.ready();
    await painted();
    const firstPaintMs = performance.now() - t0;
    return {
      firstPaintMs,
      detail: 'native renderer',
      setVisibleBars(k) {
        chart.setVisibleRange({ from: d.timesMs[n - k], to: d.timesMs[n - 1] });
      },
      destroy() { chart.destroy(); el.remove(); },
    };
  },
};

let frames = null;
function tick(t) {
  if (!frames) return;
  frames.push(t);
  requestAnimationFrame(tick);
}

function stats(times) {
  const iv = [];
  for (let i = 1; i < times.length; i++) iv.push(times[i] - times[i - 1]);
  iv.sort((a, b) => a - b);
  const q = (p) => iv[Math.min(iv.length - 1, Math.round((iv.length - 1) * p))] ?? NaN;
  const duration = times.at(-1) - times[0];
  // A frame "drops" when its interval exceeds 1.5 display refreshes.
  const vsync = q(0.1);
  return {
    frames: iv.length,
    fps: (iv.length / duration) * 1000,
    p50: q(0.5), p95: q(0.95), p99: q(0.99), max: iv.at(-1),
    dropped: iv.filter((v) => v > vsync * 1.5).length / iv.length,
  };
}

window.compare = {
  async load(lib, n) {
    current?.destroy();
    current = null;
    const d = bars(n);
    current = await LIBS[lib](d);
    current.lib = lib;
    return { firstPaintMs: current.firstPaintMs, detail: current.detail };
  },
  setVisibleBars(k) { current.setVisibleBars(k); },
  startRecording() { frames = []; requestAnimationFrame(tick); },
  stopRecording() { const t = frames; frames = null; return stats(t); },
};

// Manual use: Load, then drag/zoom yourself or press "Run" for a synthetic script.
async function manualLoad() {
  $('status').textContent = 'loading…';
  const r = await window.compare.load($('lib').value, Number($('bars').value));
  $('status').textContent = `${$('lib').value} (${r.detail}): first paint ${r.firstPaintMs.toFixed(0)} ms`;
}

$('load').onclick = manualLoad;
$('run').onclick = async () => {
  // Synthetic (untrusted) events; the published numbers use bench.mjs instead.
  const target = stage.firstElementChild;
  const r = target.getBoundingClientRect();
  const at = (x) => ({ clientX: r.left + x, clientY: r.top + r.height / 2, bubbles: true, pointerId: 1, buttons: 1 });
  window.compare.startRecording();
  target.dispatchEvent(new PointerEvent('pointerdown', at(r.width / 2)));
  target.dispatchEvent(new MouseEvent('mousedown', at(r.width / 2)));
  for (let i = 0; i < 180; i++) {
    const x = r.width / 2 + Math.sin(i / 15) * 200;
    target.dispatchEvent(new PointerEvent('pointermove', at(x)));
    target.dispatchEvent(new MouseEvent('mousemove', at(x)));
    await new Promise(requestAnimationFrame);
  }
  target.dispatchEvent(new PointerEvent('pointerup', at(r.width / 2)));
  target.dispatchEvent(new MouseEvent('mouseup', at(r.width / 2)));
  const s = window.compare.stopRecording();
  $('status').textContent += ` · ${s.fps.toFixed(0)} fps, p95 ${s.p95.toFixed(1)} ms`;
};

await init();
window.compare.ready = true;
