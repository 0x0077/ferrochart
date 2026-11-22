// Hero artwork: particles drift on a flow field, then gather from left to
// right into a chart, hold, and scatter again. Every cycle draws a new market
// as a different kind of chart (KINDS), named above it. The pointer pushes
// particles away. Canvas 2D on the light page: trails fade into the page
// color (normal blending; additive light would vanish on white).

/** The page background (globals.css --color-bg). */
const BG = [248, 247, 244];
const UP = [8, 153, 129];
const DOWN = [235, 57, 80];
const LINE = [240, 120, 0];
const BLUE = [41, 98, 255];
const MUTED = [138, 144, 156];
/** Grid lines behind the forming chart (globals.css --color-ink). */
const GRID = [13, 15, 20];
const AMBIENT = [[29, 143, 216], [122, 77, 255], [10, 165, 127]];
const BG_CSS = `rgb(${BG[0]}, ${BG[1]}, ${BG[2]})`;

const FLOW_MS = 4200;
const GATHER_MS = 2600;
const HOLD_MS = 3600;

/** One kind per cycle, in this order; all are series the engine draws. */
const KINDS = ['Candlestick', 'OHLC bars', 'Area', 'Baseline', 'Renko', 'Line and volume'];
const LABEL_FONT = '500 12px ui-monospace, SFMono-Regular, Menlo, monospace';

export function startHero(canvas) {
  const g = canvas.getContext('2d');
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
  let w = 0, h = 0, dpr = 1;
  let ps = [];
  let targets = [];
  let frame = null;
  let cycle = 0;
  let phase = 'flow';
  let phaseAt = 0;
  let running = true;
  let visible = true;
  const mouse = { x: -1e4, y: -1e4 };

  function resize() {
    const r = canvas.getBoundingClientRect();
    dpr = Math.min(devicePixelRatio || 1, 2);
    w = r.width;
    h = r.height;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    const n = Math.round(Math.min(6500, Math.max(1800, (w * h) / 200)));
    ps = Array.from({ length: n }, (_, i) => ps[i] ?? particle());
    g.fillStyle = BG_CSS;
    g.fillRect(0, 0, w, h);
  }

  function particle() {
    return {
      x: Math.random() * w, y: Math.random() * h, vx: 0, vy: 0,
      c: AMBIENT[(Math.random() * AMBIENT.length) | 0], t: -1, delay: 0,
    };
  }

  /**
   * Target points of a fresh chart, a different kind each cycle. A point is
   * [x, y, color, alpha?]; fills use a lower alpha and a sparser grid.
   */
  function chart() {
    const kind = KINDS[cycle++ % KINDS.length];
    const wide = w > 900;
    const x0 = wide ? w * 0.53 : w * 0.06;
    const x1 = w * 0.96;
    // Narrow screens: below the headline and buttons, clear of the text.
    const y0 = h * (wide ? 0.2 : 0.76);
    const y1 = h * (wide ? 0.8 : 0.93);
    const n = Math.max(18, Math.round((x1 - x0) / 26));
    const slot = (x1 - x0) / n;
    const pts = [];
    const rect = (l, t, r, b, col, step = 3, a = 1) => {
      for (let y = t; y <= b; y += step) for (let x = l; x <= r; x += step) pts.push([x, y, col, a]);
    };
    const vline = (x, t, b, col, step = 2) => {
      for (let y = t; y <= b; y += step) pts.push([x, y, col]);
    };
    const seg = (ax, ay, bx, by, col, step = 1.8) => {
      const d = Math.hypot(bx - ax, by - ay);
      for (let s = 0; s < d; s += step) pts.push([ax + ((bx - ax) * s) / d, ay + ((by - ay) * s) / d, col]);
    };
    const walk = (m, vol) => {
      const out = [];
      let p = 100;
      for (let i = 0; i < m; i++) {
        out.push(p);
        p *= 1 + (Math.random() - 0.48) * vol;
      }
      return out;
    };
    const scale = (lo, hi, top, bot) => (v) => bot - ((v - lo) / (hi - lo || 1)) * (bot - top);

    if (kind === 'Candlestick' || kind === 'OHLC bars') {
      const c = walk(n + 1, 0.05);
      const bars = c.slice(1).map((p, i) => {
        const o = c[i];
        return { o, c: p, hi: Math.max(o, p) * (1 + Math.random() * 0.018), lo: Math.min(o, p) * (1 - Math.random() * 0.018) };
      });
      const Y = scale(Math.min(...bars.map((b) => b.lo)), Math.max(...bars.map((b) => b.hi)), y0, y1);
      let ema = bars[0].c;
      let prev = null;
      bars.forEach((b, i) => {
        const cx = x0 + slot * (i + 0.5);
        const col = b.c >= b.o ? UP : DOWN;
        if (kind === 'Candlestick') {
          const bw = slot * 0.62;
          const top = Y(Math.max(b.o, b.c));
          const bot = Math.max(Y(Math.min(b.o, b.c)), top + 2);
          rect(cx - bw / 2, top, cx + bw / 2, bot, col);
          vline(cx - 0.5, Y(b.hi), top, col);
          vline(cx - 0.5, bot, Y(b.lo), col);
          // A moving average over the candles.
          ema += (b.c - ema) * 0.3;
          const cur = [cx, Y(ema) - 14];
          if (prev) seg(prev[0], prev[1], cur[0], cur[1], LINE);
          prev = cur;
        } else {
          // Open tick on the left, close tick on the right, 3 px thick.
          const tick = slot * 0.34;
          vline(cx, Y(b.hi), Y(b.lo), col, 1.8);
          for (let x = cx - tick; x < cx; x += 2) for (const dy of [-1, 0.5, 2]) pts.push([x, Y(b.o) + dy, col]);
          for (let x = cx; x <= cx + tick; x += 2) for (const dy of [-1, 0.5, 2]) pts.push([x, Y(b.c) + dy, col]);
        }
      });
    } else if (kind === 'Renko') {
      // Bricks of one box size, cut by price instead of time.
      const c = walk(n * 8, 0.02);
      const box = (Math.max(...c) - Math.min(...c)) / 12;
      // Classic rules: a new brick starts at the top or bottom of the last
      // one, so a reversal needs a move of two boxes.
      const bricks = [];
      let top = c[0], bottom = c[0];
      for (const p of c) {
        while (p >= top + box) {
          bricks.push([top, top + box, UP]);
          bottom = top;
          top += box;
        }
        while (p <= bottom - box) {
          bricks.push([bottom, bottom - box, DOWN]);
          top = bottom;
          bottom -= box;
        }
      }
      const shown = bricks.slice(-n);
      const Y = scale(Math.min(...shown.map((b) => Math.min(b[0], b[1]))), Math.max(...shown.map((b) => Math.max(b[0], b[1]))), y0, y1);
      const bw = slot * 0.82;
      shown.forEach(([a, b, col], i) => {
        const cx = x0 + slot * (i + 0.5);
        rect(cx - bw / 2, Y(Math.max(a, b)) + 1.5, cx + bw / 2, Y(Math.min(a, b)) - 1.5, col);
      });
    } else {
      // Area, Baseline, Line and volume: one smoother line of closes.
      const m = n * 3;
      const c = walk(m, 0.022);
      const bot = kind === 'Line and volume' ? y0 + (y1 - y0) * 0.7 : y1;
      const lo = Math.min(...c), hi = Math.max(...c);
      const Y = scale(lo, hi, y0, bot);
      const dx = (x1 - x0) / (m - 1);
      const ys = c.map(Y);
      const at = (x) => {
        const f = (x - x0) / dx;
        const i = Math.min(m - 2, Math.floor(f));
        return ys[i] + (ys[i + 1] - ys[i]) * (f - i);
      };
      const base = Y((lo + hi) / 2);
      for (let i = 1; i < m; i++) {
        const ax = x0 + dx * (i - 1), bx = x0 + dx * i;
        if (kind !== 'Baseline') seg(ax, ys[i - 1], bx, ys[i], BLUE);
        else {
          // Split the segment where it crosses the baseline.
          const d = Math.hypot(bx - ax, ys[i] - ys[i - 1]);
          for (let s = 0; s < d; s += 1.8) {
            const y = ys[i - 1] + ((ys[i] - ys[i - 1]) * s) / d;
            pts.push([ax + ((bx - ax) * s) / d, y, y <= base ? UP : DOWN]);
          }
        }
      }
      if (kind === 'Area') {
        // Fill to the bottom, fading downwards.
        for (let x = x0; x <= x1; x += 7) {
          const ly = at(x);
          for (let y = ly + 7; y <= y1; y += 7) pts.push([x, y, BLUE, 0.5 * (1 - (y - ly) / (y1 - ly)) + 0.08]);
        }
      } else if (kind === 'Baseline') {
        // Fill between the line and the baseline, green above, red below; dashed baseline.
        for (let x = x0; x <= x1; x += 6) {
          const ly = at(x);
          const col = ly <= base ? UP : DOWN;
          for (let y = Math.min(ly, base) + 5; y < Math.max(ly, base) - 2; y += 6) pts.push([x, y, col, 0.35]);
        }
        for (let x = x0; x <= x1; x += 10) for (let k = 0; k < 5; k += 1.6) pts.push([x + k, base, MUTED]);
      } else {
        // Volume histogram under the line.
        const vb = slot * 0.6;
        for (let i = 0; i < n; i++) {
          const cx = x0 + slot * (i + 0.5);
          const up = at(cx + slot / 2 - 1) <= at(cx - slot / 2 + 1);
          const vh = (y1 - bot - 14) * (0.25 + Math.random() * 0.75);
          rect(cx - vb / 2, y1 - vh, cx + vb / 2, y1, up ? UP : DOWN, 3, 0.55);
        }
      }
    }
    // Keep about 85% of the particles for the chart; the rest drift. If
    // there are more points than that, fills (alpha < 1) go first, so lines
    // and bodies keep no gaps.
    const keep = Math.min(pts.length, Math.round(ps.length * 0.85));
    for (let i = pts.length - 1; i > 0; i--) {
      const j = (Math.random() * (i + 1)) | 0;
      [pts[i], pts[j]] = [pts[j], pts[i]];
    }
    if (pts.length > keep) pts.sort((a, b) => (b[3] ?? 1) - (a[3] ?? 1));
    // Five grid lines over the price range, like the chart's own grid.
    const grid = Array.from({ length: 5 }, (_, i) => y0 + ((y1 - y0) * (i + 0.5)) / 5);
    return { pts: pts.slice(0, keep), x0, x1, y0, grid, kind, total: pts.length };
  }

  function assign() {
    const c = chart();
    frame = c;
    targets = c.pts;
    const order = ps.map((_, i) => i).sort(() => Math.random() - 0.5);
    for (const p of ps) p.t = -1;
    targets.forEach((tp, k) => {
      const p = ps[order[k]];
      p.t = k;
      // Drawn from left to right, like a pen.
      p.delay = ((tp[0] - c.x0) / (c.x1 - c.x0)) * GATHER_MS * 0.7 + Math.random() * 180;
    });
  }

  function step(now) {
    if (!running) return;
    requestAnimationFrame(step);
    // Off screen, tab hidden, or covered by the chart viewer.
    if (!visible || document.hidden || canvas.dataset.paused) return;
    const since = now - phaseAt;
    if (phase === 'flow' && since > FLOW_MS) { assign(); phase = 'gather'; phaseAt = now; }
    else if (phase === 'gather' && since > GATHER_MS) { phase = 'hold'; phaseAt = now; }
    else if (phase === 'hold' && since > HOLD_MS) {
      phase = 'flow'; phaseAt = now;
      for (const p of ps) {
        if (p.t >= 0) { const a = Math.random() * 6.283; p.vx += Math.cos(a) * 2.5; p.vy += Math.sin(a) * 2.5 - 0.6; }
        p.t = -1;
      }
    }
    const t = now;
    const trail = phase === 'flow' ? 0.14 : 0.22;
    g.fillStyle = `rgba(${BG[0]}, ${BG[1]}, ${BG[2]}, ${trail})`;
    g.fillRect(0, 0, w, h);
    const forming = phase !== 'flow';
    const sinceGather = phase === 'gather' ? since : GATHER_MS + since;
    // The grid builds up while the chart forms (each frame adds a little
    // against the trail fade) and fades with the trails when it scatters.
    if (forming && frame) {
      g.fillStyle = `rgba(${GRID[0]}, ${GRID[1]}, ${GRID[2]}, ${0.025 * Math.min(1, sinceGather / 800)})`;
      for (const y of frame.grid) g.fillRect(frame.x0 - 12, Math.round(y), frame.x1 - frame.x0 + 24, 1);
      // The kind's name above the chart, built up the same way.
      g.font = LABEL_FONT;
      g.fillStyle = `rgba(${MUTED[0]}, ${MUTED[1]}, ${MUTED[2]}, ${0.35 * Math.min(1, sinceGather / 800)})`;
      g.fillText(frame.kind.toUpperCase(), frame.x0 - 12, frame.y0 - 22);
    }
    for (const p of ps) {
      const target = forming && p.t >= 0 && sinceGather > p.delay ? targets[p.t] : null;
      if (target) {
        const k = Math.min(1, (sinceGather - p.delay) / 900);
        p.vx = p.vx * 0.82 + (target[0] - p.x) * 0.02 * k;
        p.vy = p.vy * 0.82 + (target[1] - p.y) * 0.02 * k;
      } else {
        const a = (Math.sin(p.x * 0.0021 + t * 0.00017) + Math.sin(p.y * 0.0027 - t * 0.00013) + Math.sin((p.x + p.y) * 0.0013 + t * 0.0001)) * 1.7;
        p.vx = p.vx * 0.965 + Math.cos(a) * 0.07;
        p.vy = p.vy * 0.965 + Math.sin(a) * 0.07;
      }
      const dx = p.x - mouse.x, dy = p.y - mouse.y;
      const d2 = dx * dx + dy * dy;
      if (d2 < 140 * 140 && d2 > 1) {
        const f = (1 - Math.sqrt(d2) / 140) * 1.6;
        const d = Math.sqrt(d2);
        p.vx += (dx / d) * f;
        p.vy += (dy / d) * f;
      }
      p.x += p.vx;
      p.y += p.vy;
      if (!target) {
        if (p.x < -10) p.x += w + 20; else if (p.x > w + 10) p.x -= w + 20;
        if (p.y < -10) p.y += h + 20; else if (p.y > h + 10) p.y -= h + 20;
      }
      const c = target ? target[2] : p.c;
      const settled = target ? Math.min(1, (sinceGather - p.delay) / 1200) : 0;
      const alpha = target ? (0.35 + settled * 0.65) * (target[3] ?? 1) : 0.2;
      g.fillStyle = `rgba(${c[0]}, ${c[1]}, ${c[2]}, ${alpha})`;
      const s = target ? 1.8 + settled * 1.4 : 1.4;
      g.fillRect(p.x, p.y, s, s);
    }
  }

  function staticFrame() {
    g.fillStyle = BG_CSS;
    g.fillRect(0, 0, w, h);
    const c = chart();
    g.fillStyle = `rgba(${GRID[0]}, ${GRID[1]}, ${GRID[2]}, 0.08)`;
    for (const y of c.grid) g.fillRect(c.x0 - 12, Math.round(y), c.x1 - c.x0 + 24, 1);
    g.font = LABEL_FONT;
    g.fillStyle = `rgb(${MUTED[0]}, ${MUTED[1]}, ${MUTED[2]})`;
    g.fillText(c.kind.toUpperCase(), c.x0 - 12, c.y0 - 22);
    for (const [x, y, col, a = 1] of c.pts) {
      g.fillStyle = `rgba(${col[0]}, ${col[1]}, ${col[2]}, ${a})`;
      g.fillRect(x, y, 3.2, 3.2);
    }
  }

  const onResize = () => { resize(); if (reduce) staticFrame(); };
  const onMove = (e) => {
    const r = canvas.getBoundingClientRect();
    mouse.x = e.clientX - r.left;
    mouse.y = e.clientY - r.top;
  };
  const onLeave = () => { mouse.x = mouse.y = -1e4; };
  const io = new IntersectionObserver(([e]) => { visible = e.isIntersecting; });

  resize();
  addEventListener('resize', onResize);
  addEventListener('pointermove', onMove, { passive: true });
  document.documentElement.addEventListener('pointerleave', onLeave);
  io.observe(canvas);

  if (reduce) staticFrame();
  else {
    phaseAt = performance.now() - FLOW_MS + 900; // the first chart forms soon
    requestAnimationFrame(step);
  }
  /** Stops the animation and removes every listener. */
  return () => {
    running = false;
    removeEventListener('resize', onResize);
    removeEventListener('pointermove', onMove);
    document.documentElement.removeEventListener('pointerleave', onLeave);
    io.disconnect();
  };
}
