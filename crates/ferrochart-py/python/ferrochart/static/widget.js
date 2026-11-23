// FerroChart Jupyter widget: the anywidget front end.
// Appended to the wasm-bindgen glue (same module: `__wbg_init`,
// `FerroChart`, `FerroPlot` are in scope). The Python chart or plot sends
// the operations that build it (a snapshot when a view is rendered, then
// each new operation); data columns come as binary buffers in the column
// buffer format of ferrochart/_jupyter.py. The model's `kind` says which
// ('chart' or 'plot').

/** wasm-bindgen initialization of this module instance (once). */
let ferrochartReady = null;

const COLUMN_TYPES = { 1: Float64Array, 2: BigInt64Array, 3: Uint32Array };

/** Column buffer → `{ rows, cols: { name: TypedArray } }` (views, no copy
 * when the buffer is 8-byte aligned). */
function decodeColumns(view) {
  let buf = view.buffer;
  let off = view.byteOffset;
  if (off % 8 !== 0) {
    const copy = new Uint8Array(view.byteLength);
    copy.set(new Uint8Array(buf, off, view.byteLength));
    buf = copy.buffer;
    off = 0;
  }
  const head = new DataView(buf, off, view.byteLength);
  const magic = String.fromCharCode(head.getUint8(0), head.getUint8(1), head.getUint8(2), head.getUint8(3));
  if (magic !== 'FCCB') throw new Error('ferrochart: not a column buffer');
  const version = head.getUint16(4, true);
  if (version !== 1) throw new Error(`ferrochart: column buffer version ${version}`);
  const n = head.getUint16(6, true);
  const rows = head.getUint32(8, true);
  const cols = {};
  let pos = off + 16 + 16 * n;
  for (let i = 0; i < n; i++) {
    const d = off + 16 + 16 * i;
    let name = '';
    for (let j = 0; j < 8; j++) {
      const c = head.getUint8(d - off + j);
      if (c) name += String.fromCharCode(c);
    }
    const T = COLUMN_TYPES[head.getUint8(d - off + 8)];
    if (!T) throw new Error(`ferrochart: column '${name}' has an unknown type`);
    cols[name] = new T(buf, pos, rows);
    const bytes = rows * T.BYTES_PER_ELEMENT;
    pos += bytes + ((8 - (bytes % 8)) % 8);
  }
  return { rows, cols };
}

function rgbaCss(u) {
  return u === 0 ? null : `#${(u >>> 0).toString(16).padStart(8, '0')}`;
}

/** Arguments of `Series.setData` from decoded columns. */
function seriesData({ cols }) {
  const out = { ...cols };
  if (cols.color) out.color = Array.from(cols.color, rgbaCss);
  return out;
}

/** The bars or points of decoded columns as `Series.update` arguments. */
function* rows({ rows: n, cols }) {
  for (let i = 0; i < n; i++) {
    const bar = {};
    for (const [k, v] of Object.entries(cols)) {
      bar[k] = k === 'time' ? Number(v[i]) : k === 'color' ? rgbaCss(v[i]) : v[i];
    }
    yield bar;
  }
}

const EVENT_NAMES = {
  click: 'click',
  crosshair_move: 'crosshairMove',
  visible_range_changed: 'visibleRangeChanged',
  need_more_history: 'needMoreHistory',
  drawing_created: 'drawingCreated',
  drawing_modified: 'drawingModified',
  drawing_removed: 'drawingRemoved',
  drawing_selected: 'drawingSelected',
  warning: 'warning',
  marker_hover: 'markerHover',
};

const PLOT_EVENT_NAMES = {
  hover: 'hover',
  click: 'click',
  view_change: 'viewChange',
};

/** Arguments of `Mark.setData` / `append` from decoded columns and the
 * operation's string columns. */
function markData({ cols }, op) {
  const out = { ...cols };
  if (op.step) out.step = op.step;
  if (op.label) out.label = op.label;
  return out;
}

/** An event payload for Python: series objects become their ids. */
function plain(v) {
  if (v === null || typeof v !== 'object') return typeof v === 'bigint' ? Number(v) : v;
  if (typeof v.id === 'number' && typeof v.setData === 'function') return v.id;
  if (Array.isArray(v)) return v.map(plain);
  const out = {};
  for (const [k, x] of Object.entries(v)) out[k] = plain(x);
  return out;
}

class View {
  constructor(model, el) {
    this.model = model;
    this.box = document.createElement('div');
    this.box.style.position = 'relative';
    this.canvas = document.createElement('canvas');
    this.canvas.style.width = '100%';
    this.canvas.style.height = '100%';
    this.canvas.style.display = 'block';
    this.box.append(this.canvas);
    el.append(this.box);
    this.size(model.get('width'), model.get('height'));
    this.id = Math.random().toString(36).slice(2);
    this.kind = model.get('kind') || 'chart';
    /** The FerroChart or FerroPlot. */
    this.chart = null;
    this.marks = new Map(); // Python mark id → Mark
    this.markIds = new Map(); // Mark id here → Python mark id
    this.rules = new Map();
    this.labels = new Map();
    this.series = new Map();
    this.indicators = new Map();
    this.lines = new Map();
    this.drawings = new Map(); // Python drawing id → id here
    this.seq = null; // snapshot not applied yet
    this.listeners = [];
    this.closed = false;
    // Messages are handled one after the other: registerIcon is asynchronous
    // and the operations after it may use the image.
    this.queue = Promise.resolve();
    this.onMsg = (msg, buffers) => {
      this.queue = this.queue.then(() => this.message(msg, buffers)).catch((e) => console.error('ferrochart:', e));
    };
    this.onEvents = () => this.subscribe();
    model.on('msg:custom', this.onMsg);
    model.on('change:events', this.onEvents);
    model.send({ type: 'init', view: this.id, wasm: ferrochartReady === null });
  }

  size(w, h) {
    this.box.style.width = `min(100%, ${w}px)`;
    this.box.style.height = `${h}px`;
  }

  async message(msg, buffers) {
    if (this.closed) return;
    if (msg.type === 'snapshot') {
      if (msg.to !== this.id || this.seq !== null) return;
      if (msg.wasm !== undefined && ferrochartReady === null) {
        ferrochartReady = __wbg_init({ module_or_path: buffers[msg.wasm].buffer.slice(
          buffers[msg.wasm].byteOffset, buffers[msg.wasm].byteOffset + buffers[msg.wasm].byteLength) });
      }
      if (ferrochartReady === null) {
        // Another view of this module instance is loading it; ask again.
        this.model.send({ type: 'init', view: this.id, wasm: true });
        return;
      }
      await ferrochartReady;
      if (this.closed) return;
      this.chart = await this.create();
      if (this.closed) {
        this.dispose();
        return;
      }
      for (const op of msg.ops) await this.apply(op, buffers);
      this.seq = msg.seq;
      this.subscribe();
    } else if (msg.type === 'ops') {
      if (this.seq === null || msg.seq <= this.seq) return;
      for (const op of msg.ops) await this.apply(op, buffers);
      this.seq = msg.seq;
    }
  }

  create() {
    const theme = this.model.get('theme');
    if (this.kind === 'plot') return FerroPlot.create(this.canvas, { ...(this.model.get('options') || {}), theme });
    return FerroChart.create(this.canvas, { ...(this.model.get('options') || {}), theme });
  }

  dispose() {
    if (!this.chart) return;
    if (this.kind === 'plot') this.chart.dispose();
    else this.chart.destroy();
    this.chart = null;
  }

  /** A plot event for Python: the hit's mark becomes its Python id. */
  plotPayload(e) {
    const out = { ...e };
    if (e.hit) {
      const id = e.hit.mark.id;
      e.hit.mark.free();
      out.hit = { ...e.hit, mark: this.markIds.get(id) ?? id };
    }
    return plain(out);
  }

  /** Forwards the events Python listens to. */
  subscribe() {
    if (!this.chart) return;
    for (const [name, cb] of this.listeners) this.chart.off(name, cb);
    this.listeners = [];
    const plot = this.kind === 'plot';
    for (const py of this.model.get('events') || []) {
      const name = (plot ? PLOT_EVENT_NAMES : EVENT_NAMES)[py];
      if (!name) continue;
      const cb = (e) => this.model.send({ type: 'event', name: py, payload: plot ? this.plotPayload(e) : plain(e) });
      this.chart.on(name, cb);
      this.listeners.push([name, cb]);
    }
  }

  s(id) {
    const s = this.series.get(id);
    if (!s) throw new Error(`ferrochart: unknown series ${id}`);
    return s;
  }

  /** The id here of Python drawing `id`. */
  d(id) {
    const d = this.drawings.get(id);
    if (d === undefined) throw new Error(`ferrochart: unknown drawing ${id}`);
    return d;
  }

  /** Python mark `id`. */
  m(id) {
    const m = this.marks.get(id);
    if (!m) throw new Error(`ferrochart: unknown mark ${id}`);
    return m;
  }

  /** Calls `f` with axis `name` of the plot. */
  axis(name, f) {
    const a = this.chart.axis(name);
    try {
      f(a);
    } finally {
      a.free();
    }
  }

  applyPlot(op, buffers) {
    const p = this.chart;
    switch (op.op) {
      case 'addMark': {
        const m = p.addMark(op.kind, op.options);
        this.marks.set(op.id, m);
        this.markIds.set(m.id, op.id);
        break;
      }
      case 'setData':
        this.m(op.mark).setData(markData(decodeColumns(buffers[op.buffer]), op));
        break;
      case 'append':
        this.m(op.mark).append(markData(decodeColumns(buffers[op.buffer]), op));
        break;
      case 'update':
        this.m(op.mark).update(op.point);
        break;
      case 'markOptions':
        this.m(op.mark).applyOptions(op.options);
        break;
      case 'removeMark': {
        const m = this.m(op.mark);
        this.markIds.delete(m.id);
        m.remove();
        this.marks.delete(op.mark);
        break;
      }
      case 'axis':
        this.axis(op.axis, (a) => a.applyOptions(op.options));
        break;
      case 'categories':
        this.axis(op.axis, (a) => a.setCategories(op.labels));
        break;
      case 'setView':
        this.axis(op.axis, (a) => a.setView(op.lo, op.hi));
        break;
      case 'addRule':
        this.rules.set(op.id, p.addRule(op.options));
        break;
      case 'ruleOptions':
        this.rules.get(op.id)?.applyOptions(op.options);
        break;
      case 'removeRule':
        this.rules.get(op.id)?.remove();
        this.rules.delete(op.id);
        break;
      case 'addLabel':
        this.labels.set(op.id, p.addLabel(op.options));
        break;
      case 'removeLabel':
        this.labels.get(op.id)?.remove();
        this.labels.delete(op.id);
        break;
      case 'fit':
        p.fit();
        break;
      case 'resize':
        this.size(op.width, op.height);
        break;
      default:
        console.warn(`ferrochart: unknown operation '${op.op}'`);
    }
  }

  apply(op, buffers) {
    if (this.kind === 'plot') {
      try {
        this.applyPlot(op, buffers);
      } catch (e) {
        // The Python plot accepted it; report and keep going.
        console.error(`ferrochart: ${op.op} failed:`, e);
      }
      return;
    }
    const c = this.chart;
    const pane = op.pane ?? undefined;
    try {
      switch (op.op) {
        case 'registerIcon': {
          const b = buffers[op.buffer];
          const px = new Uint8ClampedArray(b.buffer, b.byteOffset, b.byteLength);
          return c.registerIcon(op.name, new ImageData(px.slice(), op.width, op.height))
            .catch((e) => console.error(`ferrochart: ${op.op} failed:`, e));
        }
        case 'unregisterIcon':
          c.unregisterIcon(op.name);
          break;
        case 'addSeries':
          this.series.set(op.id, c.addSeries(op.kind, op.options, pane));
          break;
        case 'addVolume':
          this.series.set(op.id, c.addVolumeSeries(this.s(op.source), pane));
          break;
        case 'addTransform':
          this.series.set(op.id, c.addTransformSeries(op.name, op.params, op.options, pane));
          break;
        case 'removeSeries':
          this.s(op.series).remove();
          this.series.delete(op.series);
          break;
        case 'addPane':
          c.addPane(op.stretch);
          break;
        case 'removePane':
          c.removePane(op.pane);
          break;
        case 'addIndicator': {
          const ind = c.addIndicator(this.s(op.source), op.name, op.params, op.placement ?? undefined);
          this.indicators.set(op.id, ind);
          ind.outputs().forEach((s, i) => this.series.set(op.outputs[i], s));
          break;
        }
        case 'removeIndicator':
          this.indicators.get(op.id)?.remove();
          this.indicators.delete(op.id);
          break;
        case 'setData':
          this.s(op.series).setData(seriesData(decodeColumns(buffers[op.buffer])));
          break;
        case 'append': {
          const s = this.s(op.series);
          for (const bar of rows(decodeColumns(buffers[op.buffer]))) s.update(bar);
          break;
        }
        case 'update':
          this.s(op.series).update(op.bar);
          break;
        case 'applyOptions':
          this.s(op.series).applyOptions(op.options);
          break;
        case 'createPriceLine':
          this.lines.set(op.id, this.s(op.series).createPriceLine(op.options));
          break;
        case 'applyPriceLine':
          this.lines.get(op.id)?.applyOptions(op.options);
          break;
        case 'removePriceLine':
          this.lines.get(op.id)?.remove();
          this.lines.delete(op.id);
          break;
        case 'setMarkers':
          this.s(op.series).setMarkers(op.markers);
          break;
        case 'priceScale':
          c.applyPriceScaleOptions(op.options, pane);
          break;
        case 'fitContent':
          c.fitContent();
          break;
        case 'scrollToRealtime':
          c.scrollToRealtime();
          break;
        case 'setBarSpacing':
          c.setBarSpacing(op.value);
          break;
        case 'setRightOffset':
          c.setRightOffset(op.value);
          break;
        case 'setCrosshairMode':
          c.setCrosshairMode(op.value);
          break;
        case 'setVisibleRange':
          c.setVisibleLogicalRange(op.from, op.to);
          break;
        case 'addDrawing':
          this.drawings.set(op.id, c.drawings.add(op.data, pane));
          break;
        case 'removeDrawing':
          c.drawings.remove(this.d(op.id));
          this.drawings.delete(op.id);
          break;
        case 'drawingAnchors':
          c.drawings.setAnchors(this.d(op.id), op.anchors);
          break;
        case 'drawingStyle':
          c.drawings.setStyle(this.d(op.id), op.style);
          break;
        case 'drawingParam':
          c.drawings.setParam(this.d(op.id), op.key, op.value);
          break;
        case 'drawingVisible':
          c.drawings.setVisible(this.d(op.id), op.value);
          break;
        case 'drawingLocked':
          c.drawings.setLocked(this.d(op.id), op.value);
          break;
        case 'drawingFront':
          c.drawings.bringToFront(this.d(op.id));
          break;
        case 'drawingBack':
          c.drawings.sendToBack(this.d(op.id));
          break;
        case 'selectDrawing':
          c.drawings.select(op.id === null ? null : this.d(op.id));
          break;
        case 'startTool':
          c.drawings.startTool(op.kind);
          break;
        case 'cancelTool':
          c.drawings.cancelTool();
          break;
        // Python's undo history has the same drawing changes, so the same step is
        // undone (drawings keep their ids).
        case 'undoDrawing':
          c.drawings.undo();
          break;
        case 'redoDrawing':
          c.drawings.redo();
          break;
        case 'loadState': {
          c.loadState(op.state);
          const ids = c.drawings.ids();
          this.drawings = new Map(op.drawingIds.map((py, i) => [py, ids[i]]));
          break;
        }
        case 'resize':
          this.size(op.width, op.height);
          break;
        default:
          console.warn(`ferrochart: unknown operation '${op.op}'`);
      }
    } catch (e) {
      // The Python chart accepted it; report and keep going.
      console.error(`ferrochart: ${op.op} failed:`, e);
    }
  }

  close() {
    this.closed = true;
    this.model.off('msg:custom', this.onMsg);
    this.model.off('change:events', this.onEvents);
    this.dispose();
  }
}

export default {
  render({ model, el }) {
    const view = new View(model, el);
    return () => view.close();
  },
};
