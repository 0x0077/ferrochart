// Browser test of the Python package's Jupyter front end:
// builds the fixture with the Python package, then runs
// tests/web/pages/jupyter.html (a chart) and jupyter-plot.html (a plot).
//
//   scripts/build-py-static.sh && (cd crates/ferrochart-py && maturin develop --release)
//   cd tests/web && node jupyter.mjs [chrome|chromium|firefox|webkit ...]
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { PNG } from 'pngjs';
import { fileURLToPath } from 'node:url';
import { chromium, firefox, webkit } from 'playwright';
import { serve } from './serve.mjs';

const BROWSERS = {
  chrome: () => chromium.launch({ channel: 'chrome', headless: false }),
  chromium: () => chromium.launch({ headless: false }),
  firefox: () => firefox.launch({ headless: false }),
  webkit: () => webkit.launch({ headless: false }),
};

const root = fileURLToPath(new URL('../..', import.meta.url));
const python = process.env.PYTHON ?? `${root}.venv/bin/python`;
const fixture = spawnSync(python, ['tests/web/jupyter_fixture.py', 'tests/web/out/jupyter'], { cwd: root, encoding: 'utf8' });
if (fixture.status !== 0) {
  console.log(`FAIL fixture: ${fixture.stderr}`);
  process.exit(1);
}

/** Share of pixels differing by more than `tol` in some channel. */
function differing(a, b, tol = 48) {
  if (a.width !== b.width || a.height !== b.height) return 1;
  let n = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    if ([0, 1, 2].some((c) => Math.abs(a.data[i + c] - b.data[i + c]) > tol)) n++;
  }
  return n / (a.width * a.height);
}

async function run(name, port, pageName) {
  const browser = await BROWSERS[name]();
  const page = await browser.newPage({ viewport: { width: 1000, height: 700 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  try {
    await page.goto(`http://127.0.0.1:${port}/tests/web/pages/${pageName}.html`);
    const refs = pageName === 'jupyter' ? 'jupyter' : 'jupyter/plot';
    // Serve the page's screenshot comparisons until it is done.
    for (;;) {
      const h = await page.waitForFunction(() => (window.__results ? 'done' : window.__compareReq), null, { timeout: 60_000 });
      const req = await h.jsonValue();
      if (req === 'done') break;
      if (typeof req === 'object' && req.move) {
        const b = await page.locator('canvas').boundingBox();
        await page.mouse.move(b.x + req.move[0], b.y + req.move[1]);
        await page.evaluate(() => { window.__compareReq = null; window.__compareDone(0); });
        continue;
      }
      await page.waitForTimeout(200);
      const bytes = await page.locator('canvas').screenshot();
      // Kept next to the reference for inspection.
      writeFileSync(`${root}tests/web/out/${refs}/${name}-${req}`, bytes);
      const shot = PNG.sync.read(bytes);
      const ref = PNG.sync.read(readFileSync(`${root}tests/web/out/${refs}/${req}`));
      const d = differing(shot, ref);
      await page.evaluate((v) => { window.__compareReq = null; window.__compareDone(v); }, d);
    }
    const results = await page.evaluate(() => window.__results);
    let ok = errors.length === 0;
    for (const r of results) {
      ok &&= r.ok;
      if (!r.ok) console.log(`  FAIL ${r.name}: ${r.detail}`);
    }
    const detail = results.filter((r) => r.name.includes('draws')).map((r) => r.detail).join(', ');
    console.log(`${ok ? 'PASS' : 'FAIL'} ${name} ${pageName}: ${results.filter((r) => r.ok).length}/${results.length} checks (${detail})`
      + (errors.length ? ` errors=${JSON.stringify(errors)}` : ''));
    return ok;
  } catch (e) {
    console.log(`FAIL ${name}: ${e.message}`);
    return false;
  } finally {
    await browser.close();
  }
}

const server = await serve();
const { port } = server.address();
let ok = true;
for (const name of process.argv.slice(2).length ? process.argv.slice(2) : ['chrome', 'firefox', 'webkit']) {
  for (const pageName of ['jupyter', 'jupyter-plot']) ok = (await run(name, port, pageName)) && ok;
}
server.close();
process.exit(ok ? 0 : 1);
