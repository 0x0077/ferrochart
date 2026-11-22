// "Turn to dust": a snapshot of the outgoing chart breaks into particles that
// the wind carries off, sweeping from left to right. Ahead of the sweep the
// snapshot itself stays on screen at full resolution; particles take over
// where it has passed. Particles are drawn into one ImageData per frame, so
// tens of thousands stay cheap.

const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

/** Share of the duration a particle may start late: the ragged edge. */
const RAGGED = 0.14;
/** Share of the duration the sweep takes to cross the chart. */
const SWEEP = 0.5;

/**
 * Plays the effect for `snap` (a canvas of the chart, any resolution) over
 * `host` (positioned, the chart's size). Resolves when it is over.
 */
export function dissolve(snap, host, { duration = 1250 } = {}) {
  if (reducedMotion() || !snap) return Promise.resolve();
  const w = Math.round(host.clientWidth);
  const h = Math.round(host.clientHeight);
  const scale = snap.width / w;
  const src = snap.getContext('2d', { willReadFrequently: true }).getImageData(0, 0, snap.width, snap.height).data;
  const sw = snap.width;
  // The chart background (top-left pixel): skipped, it matches the panel.
  const br = src[0], bg = src[1], bb = src[2];

  const STEP = 2;
  const ps = [];
  for (let y = 0; y < h; y += STEP) {
    for (let x = 0; x < w; x += STEP) {
      const i = ((Math.round(y * scale) * sw) + Math.round(x * scale)) * 4;
      const r = src[i], g = src[i + 1], b = src[i + 2];
      if (Math.abs(r - br) + Math.abs(g - bg) + Math.abs(b - bb) < 18) continue;
      ps.push({
        x, y, r, g, b,
        delay: (x / w) * duration * SWEEP + Math.random() * duration * RAGGED,
        life: duration * (0.32 + Math.random() * 0.2),
        vx: 0.6 + Math.random() * 1.8,
        vy: -0.4 - Math.random() * 1.4,
        seed: Math.random() * 6.283,
      });
    }
  }

  // The intact part, sharp, clipped away as the sweep advances.
  snap.className = 'intact';
  host.append(snap);
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  canvas.className = 'dust';
  host.append(canvas);
  const g = canvas.getContext('2d');
  const frame = g.createImageData(w, h);
  const px = frame.data;
  const t0 = performance.now();

  return new Promise((resolve) => {
    function tick(now) {
      const t = now - t0;
      // Left of `edge` some particles have started; right of it none has.
      const edge = Math.min(w, (t / (duration * SWEEP)) * w);
      snap.style.clipPath = `inset(0 0 0 ${edge}px)`;
      px.fill(0);
      let alive = 0;
      for (const p of ps) {
        let x = p.x, y = p.y, a = 255;
        const age = t - p.delay;
        if (age > 0) {
          const k = age / p.life;
          if (k >= 1) continue;
          const f = age / 16;
          // Wind to the right, lift, and a curl of turbulence.
          x += p.vx * f * (1 + k * 2.2) + Math.sin(p.seed + f * 0.09) * 6 * k;
          y += p.vy * f * (1 + k) + Math.cos(p.seed + f * 0.07) * 4 * k;
          a = 255 * (1 - k) * (1 - k);
        } else if (p.x >= edge) {
          alive++;
          continue; // still shown by the intact snapshot
        }
        alive++;
        const xi = x | 0, yi = y | 0;
        if (xi < 0 || yi < 0 || xi >= w - 1 || yi >= h - 1) continue;
        // A 2 × 2 grain; embers glow warmer as they go.
        const warm = age > 0 ? Math.min(1, age / p.life) * 0.5 : 0;
        const r = p.r + (255 - p.r) * warm;
        const gg = p.g + (150 - p.g) * warm * 0.6;
        const b = p.b * (1 - warm);
        for (const o of [0, 4, w * 4, w * 4 + 4]) {
          const i = (yi * w + xi) * 4 + o;
          px[i] = r; px[i + 1] = gg; px[i + 2] = b; px[i + 3] = a;
        }
      }
      g.putImageData(frame, 0, 0);
      if (alive > 0 && t < duration * 1.6) requestAnimationFrame(tick);
      else {
        canvas.remove();
        snap.remove();
        resolve();
      }
    }
    requestAnimationFrame(tick);
  });
}
