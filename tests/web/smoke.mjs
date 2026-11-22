// Browser smoke test for the web demo (runs in Chrome, Firefox and Safari/WebKit).
//
// For each browser and renderer: load examples/web, wait for the chart, check
// that it drew candles, pan with the mouse, zoom with the wheel, and fail on
// any page error. Screenshots go to tests/web/out/.
//
//   (cd crates/ferrochart-wasm && wasm-pack build --release --target web)
//   cd tests/web && npm install && npx playwright install firefox webkit
//   node smoke.mjs [chrome|chromium|firefox|webkit ...]
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, firefox, webkit } from 'playwright';
import { PNG } from 'pngjs';
import { serve } from './serve.mjs';

const OUT = fileURLToPath(new URL('./out/', import.meta.url));

const BROWSERS = {
  // Google Chrome as installed on this machine (WebGPU enabled by default).
  chrome: () => chromium.launch({ channel: 'chrome', headless: false }),
  chromium: () => chromium.launch({ headless: false }),
  firefox: () => firefox.launch({ headless: false }),
  webkit: () => webkit.launch({ headless: false }),
};

/** Fraction of screenshot pixels with candle colors (green/red). */
function candlePixels(png) {
  const { data } = PNG.sync.read(png);
  let colored = 0;
  let total = 0;
  for (let i = 0; i < data.length; i += 4) {
    const [r, g, b] = [data[i], data[i + 1], data[i + 2]];
    total++;
    if ((g > r + 40 && g > b && g > 100) || (r > g + 60 && r > 150)) colored++;
  }
  return colored / total;
}

async function run(name, renderer, port) {
  const browser = await BROWSERS[name]();
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  const label = `${name}/${renderer}`;
  try {
    await page.goto(`http://127.0.0.1:${port}/examples/web/?bars=1000000&renderer=${renderer}&live=1`);
    await page.waitForFunction(() => window.__ferro.ready || window.__ferro.error, null, { timeout: 60_000 });
    const state = await page.evaluate(() => ({ ...window.__ferro, chart: undefined }));
    if (state.error) throw new Error(state.error);
    const box = await page.locator('#chart').boundingBox();
    const cx = box.x + box.width / 2;
    const cy = box.y + box.height / 2;
    await page.waitForTimeout(500);
    const before = await page.locator('#chart').screenshot();
    // Pan right then wheel-zoom; the crosshair follows the mouse.
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    for (let i = 1; i <= 10; i++) await page.mouse.move(cx + i * 20, cy);
    await page.mouse.up();
    await page.mouse.wheel(0, 400);
    await page.waitForTimeout(800);
    const after = await page.locator('#chart').screenshot();
    await mkdir(OUT, { recursive: true });
    await page.screenshot({ path: join(OUT, `${name}-${renderer}.png`) });
    const colored = candlePixels(after);
    const changed = !before.equals(after);
    const ok = errors.length === 0 && changed && colored > 0.002;
    console.log(`${ok ? 'PASS' : 'FAIL'} ${label}: backend=${state.renderer} setData=${state.setDataMs.toFixed(0)}ms`
      + ` candlePixels=${(colored * 100).toFixed(2)}% interactionChangedFrame=${changed}`
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
