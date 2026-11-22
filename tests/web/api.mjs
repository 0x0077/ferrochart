// Browser test of the wasm API (FerroChart, FerroPlot) and the lightweight-charts
// compatibility layer: runs tests/web/pages/api.html.
//
//   scripts/build-npm.sh
//   cd tests/web && node api.mjs [chrome|chromium|firefox|webkit ...]
import { chromium, firefox, webkit } from 'playwright';
import { serve } from './serve.mjs';

const BROWSERS = {
  chrome: () => chromium.launch({ channel: 'chrome', headless: false }),
  chromium: () => chromium.launch({ headless: false }),
  firefox: () => firefox.launch({ headless: false }),
  webkit: () => webkit.launch({ headless: false }),
};

async function run(name, renderer, port) {
  const browser = await BROWSERS[name]();
  const page = await browser.newPage({ viewport: { width: 1000, height: 1000 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  const label = `${name}/${renderer}`;
  try {
    await page.goto(`http://127.0.0.1:${port}/tests/web/pages/api.html?renderer=${renderer}`);
    // Serve the page's click requests until it is done.
    for (;;) {
      const h = await page.waitForFunction(() => window.__results ? 'done' : window.__clickReq, null, { timeout: 60_000 });
      const req = await h.jsonValue();
      if (req === 'done') break;
      if (req.type === 'dblclick') {
        await page.mouse.dblclick(req.x, req.y);
      } else if (req.type === 'key') {
        await page.keyboard.press(req.key);
      } else if (req.type === 'move') {
        await page.mouse.move(req.x, req.y);
      } else if (req.type === 'drag') {
        const [first, ...rest] = req.points;
        await page.mouse.move(...first);
        await page.mouse.down();
        for (const p of rest) await page.mouse.move(...p);
        await page.mouse.up();
      } else {
        await page.mouse.click(req.x, req.y);
      }
      await page.evaluate(() => { window.__clickReq = null; window.__clickDone(); });
    }
    // Crosshair through the compat layer: a real mouse move.
    const box = await page.evaluate(() => window.__lwcBox);
    if (box) {
      await page.mouse.move(box.x, box.y);
      await page.mouse.move(box.x + 5, box.y);
      await page.waitForTimeout(300);
    }
    const results = await page.evaluate(() => {
      const r = window.__results;
      const m = window.__lwcMoves ?? [];
      const last = m[m.length - 1];
      const ok = last && typeof last.time === 'number' && last.point && last.seriesData.has(window.__lwcSeries);
      r.push({ name: 'lwc crosshair move', ok: !!ok, detail: last ? JSON.stringify({ time: last.time, logical: last.logical, n: m.length }) : 'no events' });
      window.__lwcChart?.remove();
      return r;
    });
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
const names = process.argv.slice(2).length ? process.argv.slice(2) : ['chrome', 'firefox', 'webkit'];
let ok = true;
for (const name of names) {
  for (const renderer of ['auto', 'webgl2']) {
    ok = (await run(name, renderer, port)) && ok;
  }
}
server.close();
process.exit(ok ? 0 : 1);
