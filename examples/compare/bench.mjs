// Runs the comparison in Google Chrome with trusted (Playwright) input and
// writes results.json + results.md next to this file.
//
//   (cd crates/ferrochart-wasm && wasm-pack build --release --target web)
//   cd examples/compare && npm install && node bench.mjs
//
// Scenarios, identical for every library (each loaded fresh in the same
// 1280×720 page):
//   - load: construct the chart with the bars until the first frame is painted
//     (conversion into the library's input format is not timed)
//   - pan-200 / pan-100k: show the newest 200 / 100 000 bars, then drag back
//     and forth for 4 s
//   - zoom: from 200 bars, 30 wheel notches out and 30 back in
// Frame statistics are requestAnimationFrame intervals during the scenario;
// "dropped" counts intervals longer than 1.5 display refreshes. Each scenario
// runs at 1× CPU and with Chrome's 4× CPU throttling (a slower laptop).
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import os from 'node:os';
import { chromium } from 'playwright';

const ROOT = fileURLToPath(new URL('../..', import.meta.url));
const HERE = fileURLToPath(new URL('.', import.meta.url));
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript',
  '.wasm': 'application/wasm', '.css': 'text/css' };
const LIBS = ['ferrochart', 'lightweight-charts', 'vela'];
// Scenarios a library cannot show (measuring it would time a different view).
const UNSUPPORTED = {
  // ViewportState.ts: MIN_BAR_SPACING = 0.5 (not an option) caps the zoom-out
  // at about 2.4k bars in this page.
  vela: 'cannot zoom out past 0.5 px/bar',
};
// QUICK=1: one size, no throttling (to check the setup).
const BARS = process.env.QUICK ? [100_000] : [100_000, 1_000_000];
const THROTTLE = process.env.QUICK ? [1] : [1, 4];

function serve() {
  const server = createServer(async (req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname));
    const file = join(ROOT, path.endsWith('/') ? `${path}index.html` : path);
    if (!file.startsWith(ROOT)) { res.writeHead(403).end(); return; }
    try {
      const body = await readFile(file);
      res.writeHead(200, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' });
      res.end(body);
    } catch {
      res.writeHead(404).end();
    }
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

async function drag(page, box, ms) {
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.move(cx, cy);
  await page.mouse.down();
  const t0 = Date.now();
  let i = 0;
  while (Date.now() - t0 < ms) {
    await page.mouse.move(cx + Math.sin(i++ / 20) * 300, cy);
  }
  await page.mouse.up();
}

async function zoom(page, box) {
  await page.mouse.move(box.x + box.width * 0.7, box.y + box.height / 2);
  for (let i = 0; i < 30; i++) { await page.mouse.wheel(0, 120); await page.waitForTimeout(16); }
  for (let i = 0; i < 30; i++) { await page.mouse.wheel(0, -120); await page.waitForTimeout(16); }
}

await mkdir(join(HERE, 'out'), { recursive: true });
const server = await serve();
const url = `http://127.0.0.1:${server.address().port}/examples/compare/`;
const browser = await chromium.launch({ channel: 'chrome', headless: false });
const version = browser.version();
const results = [];
for (const bars of BARS) {
  for (const lib of LIBS) {
    for (const cpu of THROTTLE) {
      const page = await browser.newPage({ viewport: { width: 1280, height: 760 } });
      const errors = [];
      page.on('pageerror', (e) => errors.push(String(e)));
      const cdp = await page.context().newCDPSession(page);
      await page.goto(url);
      await page.waitForFunction(() => window.compare?.ready, null, { timeout: 60_000 });
      await cdp.send('Emulation.setCPUThrottlingRate', { rate: cpu });
      const row = { lib, bars, cpu };
      try {
        const load = await page.evaluate(([l, n]) => window.compare.load(l, n), [lib, bars]);
        row.firstPaintMs = load.firstPaintMs;
        row.detail = load.detail;
        const box = await page.locator('#stage').boundingBox();
        for (const [name, visible] of [['pan-200', 200], ['pan-100k', 100_000]]) {
          if (name === 'pan-100k' && UNSUPPORTED[lib]) { row[name] = { unsupported: UNSUPPORTED[lib] }; continue; }
          await page.evaluate((k) => window.compare.setVisibleBars(k), visible);
          await page.waitForTimeout(300);
          const shot = (tag) => page.locator('#stage').screenshot({ path: join(HERE, 'out', `${lib}-${bars}-${name}-${tag}.png`) });
          if (cpu === 1) await shot('before');
          await page.evaluate(() => window.compare.startRecording());
          await drag(page, box, 4000);
          row[name] = await page.evaluate(() => window.compare.stopRecording());
          if (cpu === 1) await shot('after');
        }
        await page.evaluate(() => window.compare.setVisibleBars(200));
        await page.waitForTimeout(300);
        await page.evaluate(() => window.compare.startRecording());
        await zoom(page, box);
        row.zoom = await page.evaluate(() => window.compare.stopRecording());
      } catch (e) {
        row.error = e.message.split('\n')[0];
      }
      if (errors.length) row.pageErrors = errors;
      results.push(row);
      const f = (s) => (s?.unsupported ? 'n/a' : s ? `${s.fps.toFixed(0)}fps p95 ${s.p95.toFixed(1)}ms drop ${(s.dropped * 100).toFixed(1)}%` : '—');
      console.log(`${lib.padEnd(18)} ${String(bars).padStart(7)} bars ${cpu}x  load ${row.firstPaintMs?.toFixed(0) ?? '—'} ms | `
        + `pan-200 ${f(row['pan-200'])} | pan-100k ${f(row['pan-100k'])} | zoom ${f(row.zoom)}`
        + (row.error ? ` ERROR ${row.error}` : '') + (errors.length ? ` pageErrors=${errors.length}` : ''));
      await page.close();
    }
  }
}
await browser.close();
server.close();

const env = {
  date: new Date().toISOString().slice(0, 10),
  browser: `Google Chrome ${version}`,
  machine: `${os.cpus()[0]?.model ?? os.arch()}, ${Math.round(os.totalmem() / 2 ** 30)} GB, ${os.platform()} ${os.release()}`,
};
if (process.env.QUICK) process.exit(0);
await writeFile(join(HERE, 'results.json'), JSON.stringify({ env, results }, null, 2) + '\n');

const cell = (s) => (s?.unsupported ? `n/a¹` : s ? `${s.p95.toFixed(1)} / ${(s.dropped * 100).toFixed(0)}%` : '—');
let md = `Measured ${env.date} on ${env.machine}, ${env.browser}, page 1280×760.\n\n`;
md += 'Cells: p95 frame interval (ms) / dropped frames (interval > 1.5 refreshes). Lower is better.\n\n';
md += '| Library | Bars | CPU | Load → first paint | Pan, 200 visible | Pan, 100k visible | Wheel zoom |\n';
md += '|---|---:|---:|---:|---:|---:|---:|\n';
for (const r of results) {
  md += `| ${r.lib}${r.detail ? ` (${r.detail})` : ''} | ${r.bars.toLocaleString('en')} | ${r.cpu}× | `
    + `${r.firstPaintMs != null ? `${r.firstPaintMs.toFixed(0)} ms` : r.error ?? '—'} | `
    + `${cell(r['pan-200'])} | ${cell(r['pan-100k'])} | ${cell(r.zoom)} |\n`;
}
md += '\n¹ Vela cannot zoom out past 0.5 px per bar (a constant in its `ViewportState.ts`), about 2.4k bars in this page.\n';
await writeFile(join(HERE, 'results.md'), md);
console.log(`\nwrote ${join(HERE, 'results.md')}`);
