// lightweight-charts compatibility layer for @ferrochart/core: the commonly
// used part of the lightweight-charts v4/v5 API on top of FerroChart, to ease
// migration.
//
//   import { createChart, CandlestickSeries } from '@ferrochart/core/lightweight-charts';
//   const chart = createChart(document.getElementById('chart'), { layout: { background: { color: '#131722' } } });
//   const s = chart.addSeries(CandlestickSeries, { upColor: '#26a69a' });   // or chart.addCandlestickSeries()
//   s.setData([{ time: 1700000000, open: 1, high: 2, low: 0.5, close: 1.5 }]);
//   chart.timeScale().fitContent();
//
// Differences from lightweight-charts are listed in the README of the
// package. In short: `createChart` returns at once but the GPU is set up
// asynchronously (calls are queued until then; `chart.whenReady()` resolves
// when it is done); there are no overlay price scales (a series with
// `priceScaleId` other than 'right' gets a pane of its own); colors other
// than the main ones and options without an equivalent are ignored.

import init, { FerroChart } from './ferrochart_wasm.js';

export const ColorType = Object.freeze({ Solid: 'solid', VerticalGradient: 'gradient' });
export const CrosshairMode = Object.freeze({ Normal: 0, Magnet: 1, Hidden: 2, MagnetOHLC: 3 });
export const LineStyle = Object.freeze({ Solid: 0, Dotted: 1, Dashed: 2, LargeDashed: 3, SparseDotted: 4 });
export const LineType = Object.freeze({ Simple: 0, WithSteps: 1, Curved: 2 });
export const PriceScaleMode = Object.freeze({ Normal: 0, Logarithmic: 1, Percentage: 2, IndexedTo100: 3 });

export const CandlestickSeries = Object.freeze({ type: 'Candlestick' });
export const BarSeries = Object.freeze({ type: 'Bar' });
export const LineSeries = Object.freeze({ type: 'Line' });
export const AreaSeries = Object.freeze({ type: 'Area' });
export const BaselineSeries = Object.freeze({ type: 'Baseline' });
export const HistogramSeries = Object.freeze({ type: 'Histogram' });

const KIND = { Candlestick: 'candlestick', Bar: 'ohlc', Line: 'line', Area: 'area', Baseline: 'baseline', Histogram: 'histogram' };
const OHLC = new Set(['Candlestick', 'Bar']);
const SCALE_MODES = ['normal', 'logarithmic', 'percentage', 'indexedTo100'];
const CROSSHAIR = ['normal', 'magnet', 'hidden', 'magnet'];

let initialized = null;
const warned = new Set();
function warnOnce(what) {
  if (!warned.has(what)) {
    warned.add(what);
    console.warn(`FerroChart (lightweight-charts compatibility): ${what}`);
  }
}

// ---------------------------------------------------------------- time

/** lightweight-charts time (UTCTimestamp seconds, 'YYYY-MM-DD' or BusinessDay) → UTC ms. */
function toMs(t) {
  if (typeof t === 'number') return Math.round(t * 1000);
  if (typeof t === 'string') {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(t);
    if (!m) throw new TypeError(`invalid time '${t}' (expected 'YYYY-MM-DD')`);
    return Date.UTC(+m[1], +m[2] - 1, +m[3]);
  }
  if (t && typeof t === 'object' && 'year' in t) return Date.UTC(t.year, t.month - 1, t.day);
  throw new TypeError(`invalid time ${JSON.stringify(t)}`);
}

function timeKind(t) {
  return typeof t === 'string' ? 'string' : typeof t === 'object' ? 'object' : 'seconds';
}

function fromMs(ms, kind) {
  if (kind === 'seconds') return ms / 1000;
  const d = new Date(ms);
  const day = { year: d.getUTCFullYear(), month: d.getUTCMonth() + 1, day: d.getUTCDate() };
  if (kind === 'object') return day;
  const p = (n, w) => String(n).padStart(w, '0');
  return `${p(day.year, 4)}-${p(day.month, 2)}-${p(day.day, 2)}`;
}

// ---------------------------------------------------------------- options

function lineStyle(s) {
  return s === LineStyle.Dashed || s === LineStyle.LargeDashed ? 'dashed'
    : s === LineStyle.Dotted || s === LineStyle.SparseDotted ? 'dotted' : 'solid';
}

function lineStyleBack(s) {
  return s === 'dashed' ? LineStyle.Dashed : s === 'dotted' ? LineStyle.Dotted : LineStyle.Solid;
}

/** lightweight-charts series options → FerroChart series options. */
function seriesOptions(type, o) {
  const out = {};
  const put = (k, v) => { if (v !== undefined) out[k] = v; };
  switch (type) {
    case 'Candlestick':
    case 'Bar':
      put('color', o.upColor);
      put('downColor', o.downColor);
      break;
    case 'Line':
      put('color', o.color);
      break;
    case 'Area':
      put('color', o.lineColor);
      put('topFill', o.topColor);
      put('bottomFill', o.bottomColor);
      break;
    case 'Baseline':
      put('color', o.topLineColor);
      put('downColor', o.bottomLineColor);
      put('topFill', o.topFillColor1);
      put('bottomFill', o.bottomFillColor1);
      if (o.baseValue && typeof o.baseValue.price === 'number') out.baseValue = o.baseValue.price;
      break;
    case 'Histogram':
      put('color', o.color);
      put('baseValue', o.base);
      break;
    default:
  }
  put('lineWidth', o.lineWidth);
  if (o.lineStyle !== undefined) out.lineStyle = lineStyle(o.lineStyle);
  if (o.lineType !== undefined) out.step = o.lineType === LineType.WithSteps;
  put('lastValueVisible', o.lastValueVisible);
  put('priceLineVisible', o.priceLineVisible);
  if (o.priceFormat && typeof o.priceFormat.precision === 'number') out.pricePrecision = o.priceFormat.precision;
  return out;
}

function priceLineOptions(o) {
  const out = {};
  for (const k of ['price', 'color', 'lineWidth', 'axisLabelVisible', 'title']) {
    if (o[k] !== undefined) out[k] = o[k];
  }
  if (o.lineStyle !== undefined) out.lineStyle = lineStyle(o.lineStyle);
  return out;
}

// v5's 'atPriceTop' / 'atPriceBottom' / 'atPriceMiddle' (with `price`) are
// all drawn centered on the price.
const AT_PRICE = ['atPriceTop', 'atPriceBottom', 'atPriceMiddle'];

function markers(list) {
  return list.map((m) => ({
    time: toMs(m.time),
    ...(AT_PRICE.includes(m.position) && Number.isFinite(m.price)
      ? { position: 'atPrice', price: m.price }
      : { position: m.position === 'belowBar' ? 'belowBar' : 'aboveBar' }),
    // FerroChart ids are integers; lightweight-charts' string ids are kept
    // only in `markers()`.
    ...(Number.isSafeInteger(m.id) && m.id >= 0 ? { id: m.id } : {}),
    shape: ['circle', 'square', 'arrowUp', 'arrowDown'].includes(m.shape) ? m.shape : 'circle',
    color: m.color ?? null,
    text: m.text ?? '',
    // lightweight-charts' size is a multiple of its default; 1 is about
    // 10 CSS px. Clamped to FerroChart's 4–64.
    size: typeof m.size === 'number' && Number.isFinite(m.size)
      ? Math.min(64, Math.max(4, m.size * 10)) : null,
  }));
}

function luminance(color) {
  const m = /^#([0-9a-f]{6})/i.exec(color ?? '');
  if (!m) return 1;
  const n = parseInt(m[1], 16);
  return (0.299 * (n >> 16) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255)) / 255;
}

// ---------------------------------------------------------------- chart

class ChartApi {
  constructor(container, options) {
    this._options = options ?? {};
    this._queue = [];
    this._fc = null;
    this._series = new Set();
    this._timeKind = 'seconds';
    this._crosshair = new Set();
    this._logical = new Set();
    this._timeRange = new Set();
    this._canvas = document.createElement('canvas');
    const o = this._options;
    Object.assign(this._canvas.style, {
      display: 'block',
      width: o.width ? `${o.width}px` : '100%',
      height: o.height ? `${o.height}px` : '100%',
    });
    container.append(this._canvas);
    this._ready = this._start();
  }

  async _start() {
    initialized ??= init();
    await initialized;
    const bg = this._options.layout?.background?.color;
    const theme = luminance(bg) < 0.5 ? 'dark' : 'light';
    const fc = await FerroChart.create(this._canvas, { theme });
    if (this._removed) {
      fc.destroy();
      return;
    }
    this._fc = fc;
    fc.on('crosshairMove', (e) => this._emitCrosshair(e.point));
    fc.on('visibleRangeChanged', (e) => this._emitRange(e));
    this._run((f) => this._apply(f, this._options));
    const queue = this._queue;
    this._queue = [];
    for (const op of queue) this._run(op);
  }

  _apply(fc, o) {
    if (o.crosshair?.mode !== undefined) fc.setCrosshairMode(CROSSHAIR[o.crosshair.mode] ?? 'normal');
    if (o.timeScale) this.timeScale().applyOptions(o.timeScale);
  }

  _run(op) {
    try {
      op(this._fc);
    } catch (e) {
      console.error('FerroChart:', e);
    }
  }

  /** Runs `op` now, or once the chart is ready. */
  _do(op) {
    if (this._fc) op(this._fc);
    else if (!this._removed) this._queue.push(op);
  }

  /** The first time seen decides the time format of events and getters. */
  _noteTime(t) {
    if (!this._timeKindSet) {
      this._timeKindSet = true;
      this._timeKind = timeKind(t);
    }
  }

  _first() {
    return this._fc?.dataRange()?.from ?? 0;
  }

  _emitCrosshair(p) {
    if (this._crosshair.size === 0) return;
    const param = { time: undefined, logical: undefined, point: undefined, seriesData: new Map(), paneIndex: undefined };
    if (p) {
      const first = this._first();
      param.logical = p.logicalIndex - first;
      param.time = p.time === null ? undefined : fromMs(p.time, this._timeKind);
      if (p.x !== undefined) param.point = { x: p.x, y: p.y };
      param.paneIndex = this._fc.panes().indexOf(p.pane);
      for (const s of this._series) {
        const d = s._dataAt(p.logicalIndex, p.time);
        if (d) param.seriesData.set(s, d);
      }
    }
    for (const h of [...this._crosshair]) h(param);
  }

  _emitRange(e) {
    const first = this._first();
    const logical = { from: e.from - first, to: e.to - first };
    for (const h of [...this._logical]) h(logical);
    if (this._timeRange.size) {
      const r = this.timeScale().getVisibleRange();
      for (const h of [...this._timeRange]) h(r);
    }
  }

  /** Resolves once the GPU is set up and queued calls have run (FerroChart extension). */
  whenReady() {
    return this._ready;
  }

  /** The underlying `FerroChart`, `null` until ready (FerroChart extension). */
  get ferrochart() {
    return this._fc;
  }

  addSeries(definition, options = {}, paneIndex = 0) {
    const type = definition?.type;
    if (!KIND[type]) throw new TypeError(`unsupported series type ${JSON.stringify(type)}`);
    const s = new SeriesApi(this, type, options);
    let pane = paneIndex;
    if (options.priceScaleId !== undefined && options.priceScaleId !== 'right') {
      warnOnce('overlay and left price scales are not supported; such a series gets a pane of its own');
      pane = -1;
    }
    this._series.add(s);
    this._do((fc) => {
      let id;
      if (pane < 0) {
        id = fc.addPane(0.3);
      } else {
        while (fc.panes().length <= pane) fc.addPane();
        id = fc.panes()[pane];
      }
      s._s = fc.addSeries(KIND[type], seriesOptions(type, options), id);
    });
    return s;
  }

  addCandlestickSeries(o) { return this.addSeries(CandlestickSeries, o); }
  addBarSeries(o) { return this.addSeries(BarSeries, o); }
  addLineSeries(o) { return this.addSeries(LineSeries, o); }
  addAreaSeries(o) { return this.addSeries(AreaSeries, o); }
  addBaselineSeries(o) { return this.addSeries(BaselineSeries, o); }
  addHistogramSeries(o) { return this.addSeries(HistogramSeries, o); }

  removeSeries(series) {
    this._series.delete(series);
    this._do(() => series._s?.remove());
  }

  timeScale() {
    return (this._timeScale ??= new TimeScaleApi(this));
  }

  priceScale(priceScaleId = 'right', paneIndex = 0) {
    if (priceScaleId !== 'right') warnOnce(`price scale '${priceScaleId}' does not exist; using 'right'`);
    return new PriceScaleApi(this, (fc) => fc.panes()[paneIndex]);
  }

  subscribeCrosshairMove(handler) { this._crosshair.add(handler); }
  unsubscribeCrosshairMove(handler) { this._crosshair.delete(handler); }

  subscribeClick() { warnOnce('subscribeClick is not supported yet'); }
  unsubscribeClick() {}

  applyOptions(options) {
    const o = options ?? {};
    if (o.layout && this._fc) warnOnce('layout options take effect only in createChart');
    this._options = { ...this._options, ...o };
    if (o.width || o.height) this.resize(o.width ?? this._canvas.clientWidth, o.height ?? this._canvas.clientHeight);
    if (this._fc) this._apply(this._fc, o);
  }

  options() {
    return this._options;
  }

  resize(width, height) {
    this._canvas.style.width = `${width}px`;
    this._canvas.style.height = `${height}px`;
  }

  remove() {
    this._removed = true;
    this._queue = [];
    this._fc?.destroy();
    this._fc = null;
    this._canvas.remove();
  }
}

class TimeScaleApi {
  constructor(chart) {
    this._c = chart;
  }

  fitContent() { this._c._do((fc) => fc.fitContent()); }
  scrollToRealTime() { this._c._do((fc) => fc.scrollToRealtime()); }
  scrollToPosition(position) { this._c._do((fc) => fc.setRightOffset(position)); }
  scrollPosition() { return this._c._fc?.rightOffset ?? 0; }

  getVisibleLogicalRange() {
    const fc = this._c._fc;
    if (!fc || !fc.dataRange()) return null;
    const r = fc.visibleLogicalRange();
    const first = this._c._first();
    return { from: r.from - first, to: r.to - first };
  }

  setVisibleLogicalRange(range) {
    this._c._do((fc) => {
      const first = this._c._first();
      fc.setVisibleLogicalRange(range.from + first, range.to + first);
    });
  }

  getVisibleRange() {
    const fc = this._c._fc;
    const r = fc?.dataRange();
    if (!r) return null;
    const v = fc.visibleLogicalRange();
    const clamp = (i) => Math.min(Math.max(Math.round(i), r.from), r.to);
    const kind = this._c._timeKind;
    return { from: fromMs(fc.timeAtIndex(clamp(v.from)), kind), to: fromMs(fc.timeAtIndex(clamp(v.to)), kind) };
  }

  setVisibleRange(range) {
    this._c._do((fc) => fc.applySync({ type: 'visibleRange', from: toMs(range.from), to: toMs(range.to) }));
  }

  logicalToCoordinate(logical) {
    const fc = this._c._fc;
    return fc ? fc.logicalToCoordinate(logical + this._c._first()) : null;
  }

  coordinateToLogical(x) {
    const fc = this._c._fc;
    return fc ? fc.coordinateToLogical(x) - this._c._first() : null;
  }

  timeToCoordinate(time) {
    const fc = this._c._fc;
    const i = fc?.indexOfTime(toMs(time));
    return i === null || i === undefined ? null : fc.logicalToCoordinate(i);
  }

  coordinateToTime(x) {
    const fc = this._c._fc;
    if (!fc) return null;
    const t = fc.timeAtIndex(Math.round(fc.coordinateToLogical(x)));
    return t === null ? null : fromMs(t, this._c._timeKind);
  }

  subscribeVisibleLogicalRangeChange(h) { this._c._logical.add(h); }
  unsubscribeVisibleLogicalRangeChange(h) { this._c._logical.delete(h); }
  subscribeVisibleTimeRangeChange(h) { this._c._timeRange.add(h); }
  unsubscribeVisibleTimeRangeChange(h) { this._c._timeRange.delete(h); }

  applyOptions(o) {
    this._c._do((fc) => {
      if (o.barSpacing !== undefined) fc.setBarSpacing(o.barSpacing);
      if (o.rightOffset !== undefined) fc.setRightOffset(o.rightOffset);
    });
  }

  options() {
    const fc = this._c._fc;
    return { barSpacing: fc?.barSpacing ?? 6, rightOffset: fc?.rightOffset ?? 0 };
  }

  width() {
    return this._c._canvas.clientWidth;
  }
}

class PriceScaleApi {
  constructor(chart, pane) {
    this._c = chart;
    this._pane = pane;
  }

  applyOptions(o) {
    this._c._do((fc) => {
      const opts = {};
      if (o.scaleMargins) opts.margins = { top: o.scaleMargins.top, bottom: o.scaleMargins.bottom };
      if (o.mode !== undefined) opts.mode = SCALE_MODES[o.mode] ?? 'normal';
      if (o.invertScale !== undefined) opts.inverted = o.invertScale;
      if (o.autoScale !== undefined) opts.autoScale = o.autoScale;
      fc.applyPriceScaleOptions(opts, this._pane(fc));
    });
  }

  options() {
    const fc = this._c._fc;
    if (!fc) return {};
    const s = fc.priceScale(this._pane(fc));
    return {
      mode: SCALE_MODES.indexOf(s.mode),
      autoScale: s.autoScale,
      invertScale: s.inverted,
      scaleMargins: { top: s.margins.top, bottom: s.margins.bottom },
    };
  }
}

class PriceLineApi {
  constructor(series, options) {
    this._s = series;
    this._options = { ...options };
    this._line = null;
  }

  applyOptions(o) {
    Object.assign(this._options, o);
    this._s._c._do(() => this._line?.applyOptions(priceLineOptions(o)));
  }

  options() {
    return this._options;
  }
}

class SeriesApi {
  constructor(chart, type, options) {
    this._c = chart;
    this._type = type;
    this._options = { ...options };
    this._s = null;
    this._markers = [];
  }

  seriesType() {
    return this._type;
  }

  _columns(items) {
    if (items.length) this._c._noteTime(items[0].time);
    if (OHLC.has(this._type)) {
      // Whitespace items (time only) have no OHLC equivalent: dropped.
      const bars = items.filter((d) => d.close !== undefined);
      const n = bars.length;
      const cols = { time: new Float64Array(n), open: new Float64Array(n), high: new Float64Array(n), low: new Float64Array(n), close: new Float64Array(n) };
      bars.forEach((d, i) => {
        cols.time[i] = toMs(d.time);
        cols.open[i] = d.open;
        cols.high[i] = d.high;
        cols.low[i] = d.low;
        cols.close[i] = d.close;
      });
      return cols;
    }
    const n = items.length;
    const cols = { time: new Float64Array(n), value: new Float64Array(n) };
    let colors = null;
    items.forEach((d, i) => {
      cols.time[i] = toMs(d.time);
      cols.value[i] = d.value ?? NaN;
      if (d.color !== undefined) (colors ??= new Array(n).fill(null))[i] = d.color;
    });
    if (colors) cols.color = colors;
    return cols;
  }

  setData(items) {
    const cols = this._columns(items);
    this._c._do(() => this._s.setData(cols));
  }

  update(item) {
    this._c._noteTime(item.time);
    const time = toMs(item.time);
    const bar = OHLC.has(this._type)
      ? { time, open: item.open, high: item.high, low: item.low, close: item.close }
      : { time, value: item.value ?? NaN };
    this._c._do(() => this._s.update(bar));
  }

  applyOptions(options) {
    Object.assign(this._options, options);
    const o = seriesOptions(this._type, options);
    this._c._do(() => this._s.applyOptions(o));
  }

  options() {
    return this._options;
  }

  priceScale() {
    return new PriceScaleApi(this._c, () => this._s.pane);
  }

  createPriceLine(options) {
    const line = new PriceLineApi(this, options);
    this._c._do(() => {
      line._line = this._s.createPriceLine(priceLineOptions({ price: 0, ...options }));
    });
    return line;
  }

  removePriceLine(line) {
    this._c._do(() => line._line?.remove());
  }

  /** lightweight-charts v4 markers. */
  setMarkers(list) {
    this._markers = list.slice();
    const m = markers(list);
    this._c._do(() => this._s.setMarkers(m));
  }

  markers() {
    return this._markers;
  }

  _dataAt(index, timeMs) {
    const d = this._s?.dataAt(index);
    if (!d) return null;
    const time = timeMs === null ? undefined : fromMs(timeMs, this._c._timeKind);
    return OHLC.has(this._type)
      ? { time, open: d.open, high: d.high, low: d.low, close: d.close }
      : { time, value: d.value };
  }

  dataByIndex(logical) {
    const fc = this._c._fc;
    if (!fc) return null;
    const i = Math.round(logical) + this._c._first();
    return this._dataAt(i, fc.timeAtIndex(i));
  }

  priceToCoordinate() {
    warnOnce('priceToCoordinate is not supported yet');
    return null;
  }

  coordinateToPrice() {
    warnOnce('coordinateToPrice is not supported yet');
    return null;
  }
}

/** lightweight-charts v5 markers plugin. */
export function createSeriesMarkers(series, list = []) {
  series.setMarkers(list);
  return {
    setMarkers: (l) => series.setMarkers(l),
    markers: () => series.markers(),
    detach: () => series.setMarkers([]),
  };
}

/**
 * Creates a chart in `container` (an element or its id). The canvas fills the
 * container unless `options.width`/`options.height` are given.
 */
export function createChart(container, options) {
  const el = typeof container === 'string' ? document.getElementById(container) : container;
  if (!el) throw new TypeError('createChart: container not found');
  return new ChartApi(el, options);
}

export function isUTCTimestamp(t) {
  return typeof t === 'number';
}

export function isBusinessDay(t) {
  return typeof t === 'object' && t !== null && 'year' in t;
}
