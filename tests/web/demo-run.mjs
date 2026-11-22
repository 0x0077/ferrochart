// Runs chart-library examples (FerroChart and FerroPlot) straight from site/catalog (no Next build)
// against the site's Rust server, one fresh page each, and reports any page
// error, console error or warning (a failing layer plugin warns), failed
// request or exception. Screenshots go to --out (default out/demos/).
//
//   cd tests/web
//   node --experimental-strip-types --no-warnings demo-run.mjs [--out=DIR] [--cat=ID] [id...]
//
// Without ids or --cat: every example. Needs the engine built:
//   (cd crates/ferrochart-wasm && wasm-pack build --release --target web)
import { copyFile, mkdir, rm, writeFile } from 'node:fs/promises';
import { register } from 'node:module';
import { fileURLToPath } from 'node:url';
import { readFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { PNG } from 'pngjs';
import { startSiteServer } from './site-server.mjs';

register('./ts-resolve.mjs', import.meta.url);
const { categories } = await import('../../site/catalog/index.ts');

const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.split('=')[1];
const out = arg('out') ?? fileURLToPath(new URL('./out/demos/', import.meta.url));
const cat = arg('cat');
const wanted = process.argv.slice(2).filter((a) => !a.startsWith('--'));
const demos = categories
  .filter((c) => !cat || c.id === cat)
  .flatMap((c) => c.demos)
  .filter((d) => !wanted.length || wanted.includes(d.id));
const unknown = wanted.filter((id) => !demos.some((d) => d.id === id));
if (unknown.length) { console.error(`unknown example(s): ${unknown.join(', ')}`); process.exit(2); }

// A throwaway site: the harness page and the engine.
const site = fileURLToPath(new URL(`./out/demo-site-${process.pid}/`, import.meta.url));
const pkg = fileURLToPath(new URL('../../crates/ferrochart-wasm/pkg/', import.meta.url));
await mkdir(site, { recursive: true });
await mkdir(out, { recursive: true });
await writeFile(`${site}index.html`, '<!doctype html><meta charset="utf-8"><link rel="icon" href="data:,"><style>html,body{margin:0;height:100%;background:#0b0d12}canvas{display:block;width:100vw;height:100vh}canvas[data-side]{float:left}canvas[data-side=main]{width:72vw}canvas[data-side=plot]{width:28vw}</style><canvas id="chart"></canvas><script type="module" src="./demo-harness.js"></script>');
await copyFile(fileURLToPath(new URL('./pages/demo-harness.js', import.meta.url)), `${site}demo-harness.js`);
for (const f of ['ferrochart_wasm.js', 'ferrochart_wasm_bg.wasm']) await copyFile(`${pkg}${f}`, `${site}${f}`);

const server = await startSiteServer(site);
const launch = () => chromium.launch({ channel: 'chrome', headless: false });
let browser = await launch();
let failed = 0;

/** Whether a screenshot is (almost) one flat colour: the chart drew nothing. */
function isBlank({ data, width, height }) {
  const [r0, g0, b0] = [data[0], data[1], data[2]];
  let differ = 0;
  const step = 4 * 7;
  for (let i = 0; i < data.length; i += step) {
    if (Math.abs(data[i] - r0) + Math.abs(data[i + 1] - g0) + Math.abs(data[i + 2] - b0) > 24) differ++;
  }
  return differ < (width * height) / 7 * 0.005;
}

async function runOne(d) {
  const page = await browser.newPage({ viewport: { width: 1100, height: 680 } });
  const problems = [];
  page.on('pageerror', (e) => problems.push(`page error: ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error' || m.type() === 'warning') problems.push(`console ${m.type()}: ${m.text()}`); });
  page.on('requestfailed', (r) => problems.push(`request failed: ${r.url()}`));
  page.on('response', (r) => { if (r.status() >= 400) problems.push(`HTTP ${r.status()}: ${r.url()}`); });
  try {
    await page.goto(server.url);
    await page.waitForFunction(() => window.harnessReady, null, { timeout: 15000 });
    await page.evaluate(([code, side]) => window.runDemo(code, side), [d.code, !!d.side]);
    await page.waitForTimeout(900);
    await page.screenshot({ path: `${out}/${d.id}.png` });
    if (isBlank(PNG.sync.read(await readFile(`${out}/${d.id}.png`)))) problems.push('blank chart: nothing drawn');
  } catch (e) {
    problems.push(`exception: ${String(e.message).split('\n')[0]}`);
  } finally {
    await Promise.race([page.close(), new Promise((r) => setTimeout(r, 5000))]);
  }
  return problems;
}

for (const d of demos) {
  let problems = await runOne(d);
  if (problems.some((p) => p.startsWith('exception: page.') || p.includes('Target'))) {
    // A wedged GPU process: new browser, one more try.
    await browser.close().catch(() => {});
    browser = await launch();
    problems = await runOne(d);
  }
  if (problems.length) {
    failed++;
    console.log(`FAIL  ${d.id}\n      ${problems.join('\n      ')}`);
  } else {
    console.log(`ok    ${d.id}`);
  }
}
await browser.close();
server.close();
await rm(site, { recursive: true, force: true });
console.log(`${demos.length - failed}/${demos.length} ok; screenshots in ${out}`);
process.exit(failed ? 1 : 0);
