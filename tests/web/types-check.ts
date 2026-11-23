// Compile-only check of the published TypeScript types (`npx tsc -p .`).
import init, { FerroChart, FerroPlot, Mark, Axis, Rule, Label, randomWalk, type MarkOptions, type MarkData, type MarkKind, type AxisOptions, type PlotHit, type Bar, type OhlcData, type TransformName, type TransformParams, type BandOptions, type Placement, type ClickEvent, type ClickTarget, type DrawingSyncOp } from '../../crates/ferrochart-wasm/pkg/ferrochart_wasm.js';

export async function example(canvas: HTMLCanvasElement): Promise<void> {
  await init();
  const chart = await FerroChart.create(canvas);
  const dark = await FerroChart.create(canvas, { theme: 'dark', renderer: 'webgl2' });
  const backend: 'webgpu' | 'webgl2' = chart.renderer;
  const candles = chart.addCandlestickSeries();
  chart.addVolumeSeries(candles);
  const data: OhlcData = randomWalk(1, 1000, 0, 60_000);
  candles.setData(data);
  candles.setData({ time: [0, 60_000], open: [1, 2], high: [2, 3], low: [0.5, 1], close: [1.5, 2.5] });
  const bar: Bar = { time: 120_000n, open: 2, high: 3, low: 1, close: 2 };
  candles.update(bar);
  candles.append({ time: [180_000], open: [2], high: [3], low: [1], close: [2] });
  candles.prepend([{ time: new Date(-60_000), open: 1, high: 2, low: 0.5, close: 1.5 }]);
  chart.setCrosshairMode('magnet');
  chart.setBarSpacing(8);
  const cb = (e: unknown) => console.log(e, backend);
  chart.on('crosshairMove', cb);
  chart.off('crosshairMove', cb);
  chart.on('click', (e: ClickEvent) => {
    const t: ClickTarget | null = e.target;
    if (t?.type === 'series' || t?.type === 'marker') console.log(t.series.id === candles.id, e.time, e.price);
  });
  chart.fitContent();
  chart.scrollToRealtime();
  const name: TransformName = 'pointFigure';
  const params: TransformParams = { boxSize: 1, reversal: 3 };
  const pf = dark.addTransformSeries(name, params, { color: '#26a69a' });
  const kind: string = pf.kind;
  pf.setData(data);
  dark.addTransformSeries('renko', { atr: 14, kind: 'ohlc' }, undefined, dark.mainPane).update(bar);
  console.log(kind);
  const dots = chart.addSeries('points', { lineWidth: 3 });
  const bandOpts: BandOptions = { fill: '#26a69a33', fillCrossed: null };
  const band: number = chart.addBand(candles, dots, bandOpts);
  chart.setBandOptions(band, { fillCrossed: 'rgba(239, 83, 80, 0.2)' });
  const read: BandOptions | null = chart.bandOptions(band);
  chart.removeBand(band);
  const placement: Placement = 'volume';
  const vma = chart.addIndicator(candles, 'volumeMa', {}, placement);
  const bands: number[] = vma.bands();
  console.log(read, bands);
  const group = new SyncGroup(1);
  chart.joinSync(group, { drawings: true });
  chart.on('drawingSync', () => {
    const ops: DrawingSyncOp[] = chart.takeDrawingSyncOps();
    for (const op of ops) dark.applyDrawingSync(op);
  });
  dark.loadDrawingSyncState(chart.drawingSyncState());
  dark.destroy();
  chart.destroy();
}

// Chart API: series kinds, panes, indicators, drawings, sync, state.
import { SyncGroup, indicatorNames, type ChartState, type DrawingData } from '../../crates/ferrochart-wasm/pkg/ferrochart_wasm.js';

export async function m4(canvas: HTMLCanvasElement, other: HTMLCanvasElement): Promise<void> {
  const chart = await FerroChart.create(canvas);
  const candles = chart.addCandlestickSeries({ color: '#26a69a', downColor: '#ef5350' });
  const line = chart.addSeries('line', { color: 'rgb(0, 0, 255)', lineStyle: 'dashed', lineWidth: 2 });
  line.setData({ time: [0, 60_000], value: [1, NaN] });
  line.update({ time: 120_000, value: 2 });
  const pane: number = chart.addPane(0.3);
  chart.addSeries('histogram', { baseValue: 0 }, pane);
  chart.setPaneStretch(pane, 0.5);
  const panes: number[] = chart.panes();
  chart.applyPriceScaleOptions({ mode: 'logarithmic', margins: { top: 0.1, bottom: 0.1 } }, panes[0]);
  const scale = chart.priceScale();
  const auto: boolean = scale.autoScale;
  const ind = chart.addIndicator(candles, 'macd', { fast: 12, slow: 26, signal: 9 }, 'newPane');
  ind.outputs().forEach((s) => s.applyOptions({ lastValueVisible: auto }));
  const names: string[] = indicatorNames();
  const pl = candles.createPriceLine({ price: 100, title: names[0] });
  pl.applyOptions({ lineStyle: 'dotted' });
  candles.setMarkers([{ time: 0, position: 'belowBar', shape: 'arrowUp', text: 'buy' }]);
  const drawing: DrawingData = { kind: 'trend_line', anchors: [{ time: 0, price: 1 }, { time: 60_000, price: 2 }] };
  const id: number = chart.drawings.add(drawing);
  chart.drawings.setStyle(id, { line: '#ff0' });
  chart.drawings.undo();
  const range = chart.visibleLogicalRange();
  chart.setVisibleLogicalRange(range.from, range.to);
  const group = new SyncGroup();
  const peer = await FerroChart.create(other);
  chart.joinSync(group, { crosshair: true });
  peer.joinSync(group);
  chart.applySync({ type: 'visibleRange', from: 0, to: 60_000 });
  const state: ChartState = chart.saveState();
  peer.loadState(JSON.parse(JSON.stringify(state)));
  chart.on('drawingCreated', (e) => console.log(e));
  // Callbacks are typed by event name; getters return number | null.
  chart.on('crosshairMove', (e) => { const p = e.point; if (p) console.log(p.logicalIndex, p.time, p.bar?.close); });
  chart.on('warning', (w) => {
    if (w.kind === 'timeAxisMerged') console.log(w.series.id, w.count);
    else if (w.kind === 'dataDropped') console.log(w.series.id);
  });
  chart.on('visibleRangeChanged', (e) => console.log(e.from + e.to));
  // r21 image markers.
  void chart.registerIcon('avatar', 'https://example.com/a.png').then(() => chart.unregisterIcon('avatar'));
  candles.setMarkers([{ time: 0, icon: 'avatar', size: 20, border: '#ef5350', borderWidth: 2 }, { time: 0, label: 'DB' }]);
  candles.applyOptions({ markerStackLimit: 5 });
  chart.on('markerHover', (e) => { if (e) console.log(e.series.id, e.index, e.overflow, e.x, e.y, e.size); });
  chart.on('click', (e) => { if (e.target?.type === 'markerOverflow') console.log(e.target.first, e.target.count); });
  // @ts-expect-error: not an event name
  chart.on('nope', () => {});
  const at: number | null = chart.indexOfTime(new Date(0));
  const t0: number | null = chart.timeAtIndex(0);
  const v = candles.dataAt(0);
  if (v && 'close' in v) console.log(v.close, at, t0);
  candles.update({ time: new Date(240_000), open: 2, high: 3, low: 1, close: 2 });
  candles.setMarkers([{ time: new Date(0), position: 'aboveBar', shape: 'circle' }]);
  chart.on('sync', (e) => console.log(e));
  pl.remove();
  ind.remove();
  peer.destroy();
  chart.destroy();
}

// lightweight-charts compatibility layer.
import { createChart, CandlestickSeries, LineSeries, CrosshairMode, createSeriesMarkers, type UTCTimestamp } from '../../crates/ferrochart-wasm/pkg/lightweight-charts.js';

export async function compat(el: HTMLElement): Promise<void> {
  const chart = createChart(el, { layout: { background: { color: '#131722' } }, crosshair: { mode: CrosshairMode.Magnet } });
  const s = chart.addSeries(CandlestickSeries, { upColor: '#26a69a' });
  s.setData([{ time: 1700000000 as UTCTimestamp, open: 1, high: 2, low: 0.5, close: 1.5 }]);
  const l = chart.addLineSeries({ color: '#2962ff' });
  l.setData([{ time: '2024-01-02', value: 1 }, { time: { year: 2024, month: 1, day: 3 }, value: 2 }]);
  chart.addSeries(LineSeries, {}, 1);
  createSeriesMarkers(s, [{ time: 1700000000, position: 'aboveBar', shape: 'circle' }]);
  chart.timeScale().fitContent();
  chart.subscribeCrosshairMove((p) => console.log(p.time, p.seriesData.get(s)));
  await chart.whenReady();
  console.log(chart.ferrochart?.barSpacing);
  chart.remove();
}

// JS plugins.
import type { LayerPluginJs, DrawingToolJs, PluginCommand } from '../../crates/ferrochart-wasm/pkg/ferrochart_wasm.js';

export async function plugins(canvas: HTMLCanvasElement): Promise<void> {
  const chart = await FerroChart.create(canvas);
  const layer: LayerPluginJs = {
    position: 'background',
    build(ctx) {
      const x = ctx.timeToX(0) ?? 0;
      const cmds: PluginCommand[] = [
        { type: 'text', x, y: ctx.rect.height / 2, text: ctx.formatPrice(1), color: '#888', size: 48, align: 'center' },
        { type: 'line', x1: 0, y1: 0, x2: 10, y2: 10, color: 'red', dash: 'dotted' },
      ];
      return cmds;
    },
  };
  const id: number = chart.addLayer(layer);
  chart.invalidateLayer(id);
  chart.removeLayer(id);
  const tool: DrawingToolJs = {
    anchorCount: 2,
    build: (ctx) => ctx.anchors.map((a) => ({ type: 'icon', x: a.x ?? 0, y: a.y ?? 0, color: '#000' })),
    hitTest: (ctx, p) => p.x < ctx.rect.width,
  };
  chart.drawings.register('my_tool', tool);
  chart.drawings.register('my_path', { anchorCount: { open: 2 }, build: () => [] });
  chart.drawings.register('my_brush', { anchorCount: 'freehand', build: () => [] });
  chart.destroy();
}

export async function plotExample(canvas: HTMLCanvasElement, chart: FerroChart): Promise<void> {
  const plot = await FerroPlot.create(canvas, { theme: 'dark', crosshair: true });
  const backend: 'webgpu' | 'webgl2' = plot.renderer;
  const opts: MarkOptions = { color: '#26a69a', stack: 1, colorScale: { center: 0, domain: [-1, 1] }, valueFormat: { type: 'percent', decimals: 1 } };
  const bars: Mark = plot.addMark('bars', opts);
  const data: MarkData = { x: new Float64Array([0, 1]), y: [1, 2], label: ['a', null], step: ['delta', 'total'] };
  bars.setData(data);
  bars.append({ x: [2], y: [3] });
  bars.update({ x: 2, y: 4, color: null });
  const kind: MarkKind = bars.kind;
  const o: Required<MarkOptions> = bars.options();
  bars.applyOptions({ band: null, icon: 'arrowUp' });
  const x: Axis = plot.axis('x');
  const ax: AxisOptions = { type: 'category', range: 'auto', format: 'compact', title: null };
  x.applyOptions(ax);
  x.setCategories(['A', 'B', 'C']);
  const [lo, hi]: [number, number] = x.view();
  x.setView(lo, hi);
  const rule: Rule = plot.addRule({ axis: 'right', value: 1, to: null, label: 'target' });
  rule.applyOptions({ color: 'red' });
  rule.remove();
  const label: Label = plot.addLabel({ x: 1, y: 2, text: 'peak', align: 'center', offset: [0, -8] });
  label.remove();
  plot.on('hover', (e) => { const h: PlotHit | null = e.hit; console.log(h?.mark.kind, h?.position.x, backend, kind, o); });
  plot.on('click', (e) => console.log(e.x, e.y, e.hit?.index));
  plot.on('viewChange', () => {});
  plot.fit();
  chart.attachSidePlot(plot, 0);
  chart.detachSidePlot(plot);
  bars.remove();
  plot.destroy();
}
