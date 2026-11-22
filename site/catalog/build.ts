// Code templates for the examples. Examples are data (catalog/*.ts); the
// code they show and run is assembled here, so hundreds of them stay
// consistent and a change of style is made once.

/** The import line of every example. */
export const IMPORT = "import { FerroChart } from '@ferrochart/core';";

const DATA_NOTE = '// Market data: the synthetic FerroChart demo API. Use your own feed.';

/** What the integration panel shows before a chart example's code. */
export const PRELUDE = `${IMPORT}

const canvas = document.querySelector('canvas');
${DATA_NOTE}`;

/** What the integration panel shows before `d.code`: the classes it uses,
 * and its canvases. */
export function preludeOf(d: { code: string; side?: boolean }): string {
  const used = ['FerroChart', 'FerroPlot'].filter((n) => new RegExp(`\\b${n}\\b`).test(d.code));
  if (used.length === 1 && used[0] === 'FerroChart' && !d.side) return PRELUDE;
  const canvases = d.side
    ? "const canvas = document.querySelector('#chart');\nconst side = document.querySelector('#side'); // a canvas right of the chart"
    : "const canvas = document.querySelector('canvas');";
  return `import { ${used.join(', ')} } from '@ferrochart/core';\n\n${canvases}\n${DATA_NOTE}`;
}

/** A FerroPlot on `canvas` (or `side`). */
export function plot(target = 'canvas', name = 'plot'): string {
  return `const ${name} = await FerroPlot.create(${target}, { theme: 'dark' });`;
}

export type Query = Record<string, string | number | boolean>;

/** `/api/<path>?<query>` */
export function apiUrl(path: string, q: Query = {}): string {
  const qs = new URLSearchParams(Object.entries(q).map(([k, v]) => [k, String(v)])).toString();
  return `/api/${path}${qs ? `?${qs}` : ''}`;
}

/** `const name = await fetch(url).then((r) => r.json());` */
export function load(name: string, path: string, q: Query = {}): string {
  return `const ${name} = await fetch('${apiUrl(path, q)}').then((r) => r.json());`;
}

export interface ChartOptions {
  maxBarSpacing?: number;
}

export function create(o: ChartOptions = {}): string {
  const extra = o.maxBarSpacing ? `, maxBarSpacing: ${o.maxBarSpacing}` : '';
  return `const chart = await FerroChart.create(canvas, { theme: 'dark'${extra} });`;
}

export interface BarsSpec {
  symbol?: string;
  interval?: string;
  count?: number;
}

export function bars(spec: BarsSpec = {}, name = 'bars'): string {
  return load(name, 'bars', { symbol: spec.symbol ?? 'BTCUSD', interval: spec.interval ?? '1m', count: spec.count ?? 3000 });
}

/** Ends the code with the view: bar spacing and scrolled to the last bar. */
export function view(spacing: number): string {
  return `chart.setBarSpacing(${spacing});\nchart.scrollToRealtime();`;
}

/** A candlestick chart with `extra` code after the data. */
export function candles(spec: BarsSpec & { spacing?: number; extra?: string; volume?: boolean }): string {
  return [
    create(),
    bars(spec),
    'const candles = chart.addCandlestickSeries();',
    'candles.setData(bars);',
    spec.volume ? 'chart.addVolumeSeries(candles);' : '',
    spec.extra ?? '',
    view(spec.spacing ?? 8),
  ]
    .filter(Boolean)
    .join('\n');
}

/** Candles with indicators: `[name, params?, placement?]`. */
export function withIndicators(
  spec: BarsSpec & { spacing?: number },
  indicators: [string, Record<string, number | string>?, string?][],
): string {
  const lines = indicators.map(([name, params, placement]) => {
    const p = params && Object.keys(params).length ? `, ${js(params)}` : placement ? ', {}' : '';
    return `chart.addIndicator(candles, '${name}'${p}${placement ? `, '${placement}'` : ''});`;
  });
  return candles({ ...spec, extra: lines.join('\n') });
}

/** A JavaScript object literal for simple values. */
export function js(o: Record<string, unknown>): string {
  const parts = Object.entries(o).map(([k, v]) => `${/^[A-Za-z_$][\w$]*$/.test(k) ? k : `'${k}'`}: ${typeof v === 'string' ? `'${v}'` : String(v)}`);
  return `{ ${parts.join(', ')} }`;
}
