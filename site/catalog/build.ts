// Code templates for the examples. Examples are data (catalog/*.ts); the
// code they show and run is assembled here, so hundreds of them stay
// consistent and a change of style is made once.

/** The import line of every example. */
export const IMPORT = "import { FerroChart } from '@ferrochart/core';";

const DATA_NOTE = '// Market data: the synthetic FerroChart demo API. Use your own feed.';
const LIVE_NOTE = '// Market data: OKX spot (BTC, ETH, SOL against USDT) through the FerroChart demo API.';

/** What the integration panel shows before a chart example's code. */
export const PRELUDE = `${IMPORT}

const canvas = document.querySelector('canvas');
${DATA_NOTE}`;

/** What the integration panel shows before `d.code`: the classes it uses,
 * and its canvases. */
export function preludeOf(d: { code: string; side?: boolean; live?: boolean }): string {
  const used = ['FerroChart', 'FerroPlot'].filter((n) => new RegExp(`\\b${n}\\b`).test(d.code));
  if (used.length === 1 && used[0] === 'FerroChart' && !d.side) return d.live ? PRELUDE.replace(DATA_NOTE, LIVE_NOTE) : PRELUDE;
  const canvases = d.side
    ? "const canvas = document.querySelector('#chart');\nconst side = document.querySelector('#side'); // a canvas right of the chart"
    : "const canvas = document.querySelector('canvas');";
  return `import { ${used.join(', ')} } from '@ferrochart/core';\n\n${canvases}\n${d.live ? LIVE_NOTE : DATA_NOTE}`;
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
  /** Real bars (OKX, BTCUSD/ETHUSD/SOLUSD, at most 3000) instead of simulated ones. */
  live?: boolean;
}

export function bars(spec: BarsSpec = {}, name = 'bars'): string {
  return load(name, spec.live ? 'live/bars' : 'bars', { symbol: spec.symbol ?? 'BTCUSD', interval: spec.interval ?? '1m', count: spec.count ?? 3000 });
}

/**
 * Keeps `series` current from the live stream of `spec` (real data): the
 * forming bar every second. `value`: a single-value series (the close).
 * Defines `stop`, which the site calls when the example closes.
 */
export function stream(series: string, spec: BarsSpec, o: { from?: string; value?: boolean } = {}): string {
  const update = o.value ? `${series}.update({ time: msg.time, value: msg.close })` : `${series}.update(msg)`;
  return `// Real time: the forming bar every second; \`since\` completes bars that
// closed after the history was loaded.
const ws = new WebSocket(\`/api/live/stream?symbol=${spec.symbol ?? 'BTCUSD'}&interval=${spec.interval ?? '1m'}&trades=false&since=\${${o.from ?? 'bars'}.time.at(-1)}\`);
ws.onmessage = (e) => {
  const msg = JSON.parse(e.data);
  if (msg.type === 'bar') ${update};
};
const stop = () => ws.close(); // call before chart.destroy()`;
}

/** Ends the code with the view: bar spacing and scrolled to the last bar. */
export function view(spacing: number): string {
  return `chart.setBarSpacing(${spacing});\nchart.scrollToRealtime();`;
}

/** A candlestick chart with `extra` code after the data. */
export function candles(spec: BarsSpec & { spacing?: number; extra?: string; volume?: boolean; stream?: boolean }): string {
  return [
    create(),
    bars(spec),
    'const candles = chart.addCandlestickSeries();',
    'candles.setData(bars);',
    spec.volume ? 'chart.addVolumeSeries(candles);' : '',
    spec.extra ?? '',
    view(spec.spacing ?? 8),
    spec.stream ? `\n${stream('candles', spec)}` : '',
  ]
    .filter(Boolean)
    .join('\n');
}

/** Candles with indicators: `[name, params?, placement?]`. */
export function withIndicators(
  spec: BarsSpec & { spacing?: number; stream?: boolean },
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
