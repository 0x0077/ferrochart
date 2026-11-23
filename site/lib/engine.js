// Runs the examples with the real FerroChart engine (WebAssembly; WebGPU
// with WebGL2 fallback) and takes pixel snapshots of a chart for the
// dissolve transition. Card thumbnails are pre-rendered images (thumbs/,
// made by tests/web/site-thumbs.mjs), so the page creates no GPU devices
// beyond the chart being viewed. Browser only: import it from effects.

/** Where scripts/copy-wasm.mjs puts the engine (public/wasm/). */
const ENGINE_URL = '/wasm/ferrochart_wasm.js';

let booted = null;

/**
 * Loads the WebAssembly module once; resolves to its exports. Loaded at run
 * time, not bundled: the engine is a prebuilt file next to the site.
 */
export function boot() {
  booted ??= import(/* webpackIgnore: true */ /* turbopackIgnore: true */ ENGINE_URL).then(async (m) => {
    await m.default();
    return m;
  });
  return booted;
}

const AsyncFunction = (async () => {}).constructor;

/** `?renderer=webgl2` forces a backend (for testing). */
const forcedRenderer = () => new URLSearchParams(location.search).get('renderer');

// Creating or destroying GPU devices back to back, or one while another is
// being created (switching charts quickly), can wedge Chrome's GPU
// process: the page then blocks in the next create. Every create and destroy
// on the page goes through this gate, one at a time and GAP_MS apart.
const GAP_MS = 250;
let gate = Promise.resolve();
let lastGpuOp = 0;
function gpuSlot(fn) {
  const next = gate.then(async () => {
    const wait = lastGpuOp + GAP_MS - performance.now();
    if (wait > 0) await new Promise((r) => setTimeout(r, wait));
    try { return await fn(); } finally { lastGpuOp = performance.now(); }
  });
  gate = next.catch(() => {});
  return next;
}

/**
 * Where `/api/...` goes: the page's own origin in production (the Rust
 * server serves both), another one in development (`NEXT_PUBLIC_API_BASE`,
 * e.g. http://127.0.0.1:8080 with `npm run server:api`).
 */
const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? '';

/**
 * Responses of the running example's requests, started before the engine
 * is ready (`prefetch`); each is used once by `apiFetch`.
 */
const prefetched = new Map();

/**
 * Starts the requests `code` makes with a constant URL
 * (`fetch('/api/...')`), so the data downloads while the engine loads and
 * the GPU starts, not after. Forgets those of the previous example.
 */
function prefetch(code) {
  prefetched.clear();
  for (const m of code.matchAll(/fetch\(\s*(['"`])(\/api\/[^'"`$\\]*)\1\s*\)/g)) {
    const url = m[2];
    if (!prefetched.has(url)) prefetched.set(url, fetch(API_BASE + url).catch(() => null));
  }
}

/** `fetch` for the examples: `/api/...` resolved against the API. */
export function apiFetch(input, init) {
  if (typeof input === 'string' && input.startsWith('/api/')) {
    const early = init ? undefined : prefetched.get(input);
    if (early) {
      prefetched.delete(input);
      // A failed early request is made again.
      return early.then((r) => r ?? fetch(API_BASE + input));
    }
    return fetch(API_BASE + input, init);
  }
  return fetch(input, init);
}

/**
 * Loads and compiles the engine in the background once the page is idle, so
 * opening a chart does not wait for it. Skipped with Save-Data.
 */
export function warm() {
  if (navigator.connection?.saveData) return;
  const go = () => boot().catch(() => { booted = null; });
  const idle = () => (window.requestIdleCallback ? requestIdleCallback(go, { timeout: 5000 }) : setTimeout(go, 2000));
  if (document.readyState === 'complete') idle();
  else addEventListener('load', idle, { once: true });
}

/** `WebSocket` for the examples: relative URLs against the API, ws(s). */
class ApiSocket extends WebSocket {
  constructor(url, protocols) {
    const base = API_BASE || location.origin;
    super(new URL(url, base).href.replace(/^http/, 'ws'), protocols);
  }
}

/** Share of the width the chart keeps when an example has a side plot. */
const MAIN_SHARE = 72;

/**
 * Runs `demo.code` on `canvas`; resolves to `{ chart, plot, view, side, stop }`:
 * the FerroChart and FerroPlot the example made (one may be null); `view`,
 * the one shown (the chart if both); `side`, the second canvas of a
 * `demo.side` example (made here, right of `canvas`); `stop`, which ends
 * live feeds, or null. `onCreate(obj)` is called with each chart or plot as
 * soon as it exists, before the example gives it data.
 */
export async function run(demo, canvas, { onCreate } = {}) {
  prefetch(demo.code);
  const { FerroChart, FerroPlot } = await boot();
  const forced = forcedRenderer();
  const made = [];
  const gated = (Engine) => ({
    create: (c, o) => gpuSlot(async () => {
      const obj = await Engine.create(c, forced ? { ...o, renderer: forced } : o);
      made.push(obj);
      onCreate?.(obj);
      return obj;
    }),
  });
  let side = null;
  if (demo.side) {
    // The side plot's canvas: same class (same positioning), right of the chart.
    side = document.createElement('canvas');
    side.className = canvas.className;
    canvas.dataset.side = 'main';
    side.dataset.side = 'plot';
    canvas.style.setProperty('--main-share', `${MAIN_SHARE}%`);
    side.style.setProperty('--main-share', `${MAIN_SHARE}%`);
    canvas.after(side);
  }
  const fn = new AsyncFunction(
    'FerroChart', 'FerroPlot', 'canvas', 'side', 'fetch', 'WebSocket',
    // `stop` is the example's own (live feeds), not the global window.stop.
    `${demo.code}\nreturn typeof stop === 'function' && stop !== globalThis.stop ? stop : null;`,
  );
  try {
    const stop = await fn(gated(FerroChart), gated(FerroPlot), canvas, side, apiFetch, ApiSocket);
    // What the example made, by class (a variable named `chart` could also
    // be an element with that id).
    const chart = made.find((x) => x instanceof FerroChart) ?? null;
    const plot = made.find((x) => x instanceof FerroPlot) ?? null;
    if (!chart && !plot) throw new Error('the example creates neither a chart nor a plot');
    return { chart, plot, view: chart ?? plot, side, stop };
  } catch (e) {
    // Failed after a chart or plot was made (a request, say): free their GPU devices.
    freeAll(made).then(() => side?.remove());
    throw e;
  }
}

/** Destroys a FerroChart or disposes of a FerroPlot. */
function free(obj) {
  try {
    if (typeof obj.destroy === 'function') obj.destroy();
    else obj.dispose();
  } catch { /* already gone */ }
}

/**
 * Frees `objs` (charts and plots), each GPU device in its own gate slot:
 * two destroyed back to back wedge Chrome's GPU process like two creates.
 * Side plots are detached from their chart first. Resolves when all are gone.
 */
function freeAll(objs) {
  const plots = objs.filter((x) => typeof x.destroy !== 'function');
  for (const chart of objs.filter((x) => typeof x.destroy === 'function')) {
    for (const p of plots) {
      try { chart.detachSidePlot(p); } catch { /* not attached */ }
    }
  }
  // Plots first: a chart frees after the plots it drove.
  return Promise.all([...plots, ...objs.filter((x) => !plots.includes(x))].map((x) => gpuSlot(() => free(x))));
}

/**
 * Disposes what `run` returned. The chart and plot are freed through the
 * gate and `canvas` (and the side canvas) removed after: until then they
 * may still draw into it, so keep it laid out. Resolves once all are gone.
 */
export function dispose(handle, canvas) {
  if (!handle) { canvas?.remove(); return Promise.resolve(); }
  try { handle.stop?.(); } catch { /* ignore */ }
  const { chart, plot, side } = handle;
  handle.chart = handle.plot = handle.view = null;
  return freeAll([chart, plot].filter(Boolean)).then(() => {
    canvas?.remove();
    side?.remove();
  });
}

/** Whether a 2D context holds anything but one flat color. */
function hasContent(g, w, h) {
  const { data } = g.getImageData(0, 0, w, h);
  const step = 4 * 97;
  const r0 = data[0], g0 = data[1], b0 = data[2], a0 = data[3];
  if (a0 === 0 && data.every((v, i) => i % 4 !== 3 || v === 0)) return false;
  for (let i = 0; i < data.length; i += step) {
    if (Math.abs(data[i] - r0) + Math.abs(data[i + 1] - g0) + Math.abs(data[i + 2] - b0) > 24) return true;
  }
  return false;
}

/**
 * Copies the chart's (or plot's) next frame into a new 2D canvas of `w` × `h` CSS px
 * (`scale` device px each); null if the browser does not let us read it.
 *
 * The engine draws only when something changed, and a WebGL canvas can only
 * be read during the frame it was drawn in. So we ask for a redraw (crosshair
 * hidden, visible range nudged) and read the canvas in an animation-frame
 * callback registered after the engine's, i.e. in the same frame.
 */
export function snapshot(view, canvas, w, h, scale = 1) {
  // Only a canvas filling the host turns to dust (not a chart with a side plot).
  if (canvas.dataset.side) return Promise.resolve(null);
  try {
    if (typeof view.setCrosshairMode === 'function') {
      view.setCrosshairMode('hidden');
      // A shift far below a pixel: a real change, so the engine redraws.
      const r = view.visibleLogicalRange();
      view.setVisibleLogicalRange(r.from + 1e-6, r.to + 1e-6);
    } else {
      // A plot: the same for its x axis.
      const x = view.axis('x');
      const [lo, hi] = x.view();
      const d = (hi - lo) * 1e-9;
      x.setView(lo + d, hi + d);
      x.free();
    }
  } catch { /* the chart may already be gone */ }
  const width = Math.round(w * scale);
  const height = Math.round(h * scale);
  // Pixels are read in a worker. Reading a GPU canvas back on the main
  // thread is synchronous, and on WebGPU in Chrome it could freeze the page
  // for good now and then; a stuck worker only costs the effect.
  const pixels = new Promise((resolve) => {
    requestAnimationFrame(() => {
      let bitmap;
      try {
        bitmap = createImageBitmap(canvas, { resizeWidth: width, resizeHeight: height });
      } catch {
        resolve(null);
        return;
      }
      bitmap.then((bmp) => readPixels(bmp, width, height).then(resolve), () => resolve(null));
    });
  });
  const timeout = new Promise((r) => setTimeout(() => r(null), SNAPSHOT_TIMEOUT_MS));
  return Promise.race([pixels, timeout]).then((data) => {
    if (!data) return null;
    const out = document.createElement('canvas');
    out.width = width;
    out.height = height;
    const g = out.getContext('2d', { willReadFrequently: true });
    g.putImageData(new ImageData(data, width, height), 0, 0);
    return hasContent(g, width, height) ? out : null;
  });
}

/** How long a snapshot may take before the switch goes on without one. */
const SNAPSHOT_TIMEOUT_MS = 600;

let worker = null;
let nextJob = 0;
/** RGBA bytes of `bmp` (consumed), read in a worker; null on failure. */
function readPixels(bmp, width, height) {
  try {
    worker ??= new Worker('/snapshot-worker.js');
  } catch {
    bmp.close();
    return Promise.resolve(null);
  }
  const id = ++nextJob;
  return new Promise((resolve) => {
    const onMessage = (e) => {
      if (e.data.id !== id) return;
      worker.removeEventListener('message', onMessage);
      resolve(e.data.pixels ? new Uint8ClampedArray(e.data.pixels) : null);
    };
    worker.addEventListener('message', onMessage);
    worker.postMessage({ id, bitmap: bmp, width, height }, [bmp]);
  });
}
