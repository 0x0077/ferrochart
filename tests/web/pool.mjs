// Browser test of the chart pools and snapshots (r25): runs
// tests/web/pages/pool.html.
//
//   scripts/build-npm.sh
//   cd tests/web && node pool.mjs [chromium|firefox|webkit ...]
import { chromium, firefox, webkit } from 'playwright';
import { serve } from './serve.mjs';

const BROWSERS = {
  chromium: () => chromium.launch({ headless: false }),
  firefox: () => firefox.launch({ headless: false }),
  webkit: () => webkit.launch({ headless: false }),
};

async function run(name, renderer, port) {
  const browser = await BROWSERS[name]();
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  const label = `${name}/${renderer}`;
  try {
    await page.goto(`http://127.0.0.1:${port}/tests/web/pages/pool.html?renderer=${renderer}`);
    await page.waitForFunction(() => window.__results, null, { timeout: 60_000 });
    const results = await page.evaluate(() => window.__results);
    let ok = errors.length === 0;
    for (const r of results) {
      ok &&= r.ok;
      if (!r.ok) console.log(`  FAIL ${r.name}: ${r.detail}`);
    }
    console.log(`${ok ? 'PASS' : 'FAIL'} ${label}: ${results.filter((r) => r.ok).length}/${results.length} checks`
      + (errors.length ? ` errors=${JSON.stringify(errors)}` : ''));
    return ok;
  } catch (e) {
    console.log(`FAIL ${label}: ${e.message}${errors.length ? ` errors=${JSON.stringify(errors)}` : ''}`);
    return false;
  } finally {
    await browser.close();
  }
}

const server = await serve();
const { port } = server.address();
const names = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(BROWSERS);
let all = true;
for (const name of names) {
  for (const renderer of ['auto', 'webgl2']) {
    all = (await run(name, renderer, port)) && all;
  }
}
server.close();
process.exit(all ? 0 : 1);
