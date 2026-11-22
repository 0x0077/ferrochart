// Page script of demo-run.mjs: runs one example's code on a full-page canvas
// (with a second canvas on the right for examples with a side plot).
import init, { FerroChart, FerroPlot } from './ferrochart_wasm.js';

const AsyncFunction = (async () => {}).constructor;
await init();
window.runDemo = async (code, withSide) => {
  const canvas = document.getElementById('chart');
  let side = null;
  if (withSide) {
    side = document.createElement('canvas');
    canvas.dataset.side = 'main';
    side.dataset.side = 'plot';
    canvas.after(side);
  }
  // What the example creates (not its variables: `chart` may also name the
  // canvas element, whose id is "chart").
  const created = [];
  // No attribution logo: snapshots and card images show the chart alone.
  const recorded = (Engine) => ({ create: async (c, o) => { const x = await Engine.create(c, { ...o, attributionLogo: false }); created.push(x); return x; } });
  const fn = new AsyncFunction('FerroChart', 'FerroPlot', 'canvas', 'side', code);
  await fn(recorded(FerroChart), recorded(FerroPlot), canvas, side);
  const made = {
    chart: created.find((x) => x instanceof FerroChart) ?? null,
    plot: created.find((x) => x instanceof FerroPlot) ?? null,
  };
  if (!made.chart && !made.plot) throw new Error('the example creates neither a chart nor a plot');
  // Keep references: an unreferenced chart is freed by the engine's
  // finalizer and stops drawing.
  window.__made = made;
  made.chart?.setCrosshairMode('hidden');
  return true;
};
window.harnessReady = true;
