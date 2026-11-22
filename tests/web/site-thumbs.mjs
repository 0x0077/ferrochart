// Renders the card images of the site (site/public/thumbs/<id>.jpg) with the
// real engine in Chrome: one page per example, so each has a fresh GPU device.
// Run after changing an example or the engine's look:
//
//   (cd crates/ferrochart-wasm && wasm-pack build --release --target web)
//   (cd site && npm run build)
//   cd tests/web && node site-thumbs.mjs [--missing] [id...]
//
// The examples load data from the site's Rust server (started here). With
// --missing, only examples without an image. Then rebuild the site so out/
// has the new images.
import { access, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { startSiteServer, catalog } from './site-server.mjs';

const OUT = fileURLToPath(new URL('../../site/public/thumbs/', import.meta.url));
const SIZE = { width: 640, height: 400 }; // CSS px; rendered at 2x
const missing = process.argv.includes('--missing');
const wanted = process.argv.slice(2).filter((a) => !a.startsWith('--'));

await mkdir(OUT, { recursive: true });
const server = await startSiteServer();
const exists = (id) => access(`${OUT}${id}.jpg`).then(() => true, () => false);
const demos = [];
for (const d of (await catalog(server.url)).flatMap((c) => c.demos)) {
  if (wanted.length && !wanted.includes(d.id)) continue;
  if (missing && (await exists(d.id))) continue;
  demos.push(d);
}
console.log(`${demos.length} image(s) to render`);
const launch = () => chromium.launch({ channel: 'chrome', headless: false });
let browser = await launch();
let failed = 0;

async function shoot(d) {
  const page = await browser.newPage({ viewport: SIZE, deviceScaleFactor: 2 });
  try {
    await page.goto(`${server.url}/thumb.html?demo=${d.id}`);
    await page.waitForFunction(() => window.__thumb?.ready || window.__thumb?.error, null, { timeout: 20000, polling: 100 });
    const error = await page.evaluate(() => window.__thumb.error);
    if (error) throw new Error(error);
    const shot = { path: `${OUT}${d.id}.jpg`, type: 'jpeg', quality: 82 };
    // A chart with a side plot has two canvases filling the page: take the page.
    const canvases = page.locator('canvas');
    if ((await canvases.count()) > 1) await page.screenshot(shot);
    else await canvases.screenshot(shot);
  } finally {
    // A wedged page may never close; the browser is replaced then anyway.
    await Promise.race([page.close(), new Promise((r) => setTimeout(r, 5000))]);
  }
}

for (const d of demos) {
  let ok = false;
  for (let attempt = 1; attempt <= 3 && !ok; attempt++) {
    try {
      await shoot(d);
      ok = true;
      console.log(`ok    ${d.id}`);
    } catch (e) {
      console.log(`retry ${d.id} (${attempt}): ${e.message.split('\n')[0]}`);
      // A wedged GPU process takes the browser with it: start a new one.
      await browser.close().catch(() => {});
      browser = await launch();
    }
  }
  if (!ok) failed++;
}
await browser.close();
server.close();
if (failed) {
  console.error(`${failed} image(s) failed`);
  process.exit(1);
}
