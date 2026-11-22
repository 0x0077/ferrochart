// The live chart in the viewer. Imperative on purpose: GPU devices need
// exact create/destroy timing (engine.js), and the outgoing chart turns to
// dust (dissolve.js) while the next one is drawn by the engine's intro
// animation (`playIntro`): the chart shows its grid
// and axes at once, empty, and draws its data from left to right when
// `reveal` is called. React tells it which example to show; it reports
// back through callbacks.
//
// The canvases live in a `surface` this module owns (React never renders
// into it), one `slot` per chart, so the surface can move to another
// element (the details layer) without restarting the chart.
// Browser only.

import { run, dispose, snapshot } from './engine.js';
import { dissolve } from './dissolve.js';
import { reducedMotion } from './reveal.js';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const frames = (n) => new Promise((r) => {
  const step = () => (n-- > 0 ? requestAnimationFrame(step) : r());
  step();
});

/** Intro of a switch: shorter than the opening one, after the dust. */
const SWITCH_DRAW = 600;
const SWITCH_DRAW_AFTER = 300;
/** An intro delay long enough to hold the empty chart until `reveal`. */
const HOLD = 1e9;

/** The charts and plots of a run handle. */
const viewsOf = (handle) => [handle?.chart, handle?.plot].filter(Boolean);

/**
 * @param {HTMLElement} host positioned element the surface goes into
 * @param {{ onShown?: (demo, renderer) => void, onError?: (demo, message) => void, accent?: () => string }} hooks
 */
export function createStage(host, hooks = {}) {
  const surface = document.createElement('div');
  surface.className = 'stage-surface';
  host.append(surface);
  const state = { demo: null, handle: null, slot: null, busy: false, pending: null, alive: true, want: null };
  const accent = () => hooks.accent?.() ?? '#4cc9f0';

  function newSlot() {
    const slot = document.createElement('div');
    slot.className = 'stage-slot';
    const canvas = document.createElement('canvas');
    canvas.className = 'chart';
    slot.append(canvas);
    surface.append(slot);
    return { slot, canvas };
  }

  /** Frees `handle` and removes its slot once the chart is gone. */
  function drop(handle, slot) {
    const gone = dispose(handle, slot?.querySelector('canvas.chart:not([data-side="plot"])') ?? null);
    gone.then(() => slot?.remove());
    return gone;
  }

  /** Draws the held chart (and its side plot) over `duration` ms. */
  function draw(duration) {
    const slot = state.slot;
    if (slot) slot.dataset.shown = '1';
    const pen = accent();
    return Promise.all(viewsOf(state.handle).map((v) => v.playIntro({ duration, pen }).catch(() => {})));
  }

  /**
   * Switches to `demo`. `mode`:
   * - 'switch': the old chart turns to dust and the new one draws itself;
   * - 'intro': the new chart shows empty (grid and axes) until `reveal()`
   *   (the viewer opening);
   * - 'plain': it fades in (deep links, reduced motion).
   * Resolves once the new chart has started (or failed).
   */
  async function show(demo, mode = 'switch') {
    if (!state.alive) return;
    if (state.busy) { state.pending = { demo, mode }; return; }
    if (state.demo === demo && state.handle) return;
    state.busy = true;
    state.demo = demo;
    if (mode !== 'intro') state.want = null;
    const old = state.handle;
    const oldSlot = state.slot;
    const t0 = performance.now();
    let dust = Promise.resolve();
    if (old && oldSlot) {
      const canvas = oldSlot.querySelector('canvas.chart');
      const r = surface.getBoundingClientRect();
      // Only a chart that was on screen turns to dust.
      const seen = oldSlot.dataset.shown === '1';
      const snap = mode === 'switch' && seen && !reducedMotion()
        ? await snapshot(old.view, canvas, r.width, r.height, Math.min(devicePixelRatio || 1, 2))
        : null;
      if (snap) {
        oldSlot.classList.add('leaving');
        dust = dissolve(snap, surface);
      } else {
        oldSlot.style.opacity = '0';
        await sleep(250);
      }
      oldSlot.style.pointerEvents = 'none';
      drop(old, oldSlot);
      state.handle = null;
    } else if (oldSlot) {
      oldSlot.remove(); // a failed start left an empty slot
    }
    surface.querySelectorAll('.error').forEach((n) => n.remove());
    const { slot, canvas } = newSlot();
    state.slot = slot;
    const hold = mode !== 'plain' && !reducedMotion();
    if (!hold) slot.classList.add('entering');
    let ok = false;
    try {
      const handle = await run(demo, canvas, {
        // Empty from the first frame: the data waits for the intro.
        onCreate: hold ? (v) => { v.playIntro({ delay: HOLD }).catch(() => {}); } : undefined,
      });
      if (!state.alive || state.slot !== slot) {
        drop(handle, slot); // cleared while the chart was starting
      } else {
        state.handle = handle;
        ok = true;
        hooks.onShown?.(demo, handle.view?.renderer === 'webgpu' ? 'WebGPU' : 'WebGL2');
      }
    } catch (e) {
      const message = String(e?.message ?? e);
      console.error(e);
      hooks.onError?.(demo, message);
      const err = document.createElement('p');
      err.className = 'error';
      err.textContent = `This chart could not start: ${message}`;
      slot.append(err);
      slot.dataset.shown = '1';
    }
    if (ok) await frames(2); // the first frame lands
    if (!hold) {
      requestAnimationFrame(() => slot.classList.remove('entering'));
      slot.dataset.shown = '1';
    } else if (ok && mode === 'switch' && state.slot === slot) {
      // The drawing follows the dust's sweep from the left.
      const wait = t0 + SWITCH_DRAW_AFTER - performance.now();
      if (wait > 0) await sleep(wait);
      if (state.slot === slot) await draw(SWITCH_DRAW);
    } else if (ok && mode === 'intro' && state.slot === slot && state.want) {
      // `reveal` was called before the chart was ready: draw now.
      const want = state.want;
      state.want = null;
      want.start(draw(want.duration));
    }
    if (!ok && state.want) {
      state.want.start(Promise.resolve());
      state.want = null;
    }
    await dust;
    state.busy = false;
    const next = state.pending;
    state.pending = null;
    if (next && next.demo !== state.demo) show(next.demo, next.mode);
  }

  /**
   * Draws the chart of an 'intro' show (the engine's intro animation with a
   * pen line in the accent color), now or as soon as the chart is ready.
   * Returns `{ started, done, finish }`: `started` resolves when the drawing
   * begins, `done` when it ends.
   * @param {{ duration?: number }} [o]
   */
  function reveal({ duration = 900 } = {}) {
    // Resolved with `{ drawing }`: a bare promise would be adopted, and
    // `started` would wait for the end.
    let begin;
    const began = new Promise((r) => { begin = r; });
    const start = (drawing) => begin({ drawing });
    const done = began.then((b) => b.drawing);
    const started = began.then(() => {});
    if (state.handle && state.slot) start(draw(duration));
    else if (state.slot && state.demo) state.want = { duration, start };
    else start(Promise.resolve());
    return { started, done, finish: skip };
  }

  /** Jumps to the end of the opening or a switch: the whole chart now. */
  function skip() {
    if (state.want) {
      state.want.start(Promise.resolve());
      state.want = null;
    }
    if (state.slot) state.slot.dataset.shown = '1';
    for (const v of viewsOf(state.handle)) {
      try { v.finishIntro(); } catch { /* gone */ }
    }
  }

  /** Destroys the chart; resolves once its GPU device is gone. */
  function clear() {
    skip();
    const gone = state.handle || state.slot ? drop(state.handle, state.slot) : Promise.resolve();
    Object.assign(state, { demo: null, handle: null, slot: null, pending: null, want: null });
    surface.querySelectorAll('.error, .dust, .intact').forEach((n) => n.remove());
    return gone;
  }

  /** For good: `clear` and ignore later calls. */
  function destroy() {
    const gone = clear();
    state.alive = false;
    surface.remove();
    return gone;
  }

  /** Moves the chart to `el` (positioned), or back to `host` with null. */
  function moveTo(el) {
    (el ?? host).append(surface);
  }

  return { show, reveal, skip, clear, destroy, moveTo };
}
