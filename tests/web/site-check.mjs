// Browser check of the built site (site/out) served by the site's Rust
// server with its data API: checks the viewer's opening, search, code
// drawer, all-charts layer and details layer, opens every category's
// viewer, switches through all its charts with the dust transition, and fails on any page
// error, console error, failed request or chart that did not start.
//
//   (cd site && npm run build)
//   cd tests/web && node site-check.mjs [--renderer=webgl2] [--shots=DIR] [--cat=ID] [--reduced-motion]
//
// --url=http://localhost:3000 checks a running `next dev` instead (with the
// API on :8080: `npm run server:api` in site/).
import { mkdir } from 'node:fs/promises';
import { chromium } from 'playwright';
import { startSiteServer, catalog } from './site-server.mjs';

const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.split('=')[1];
const renderer = arg('renderer');
const shots = arg('shots');
if (shots) await mkdir(shots, { recursive: true });

const url = arg('url');
const server = url ? null : await startSiteServer();
const base = url ?? server.url;
const only = arg('cat');
const categories = (await catalog(base)).filter((c) => !only || c.id === only);
let browser, page;
const problems = [];
let step = 'load';
// A wedged page can block even page.evaluate: give up with the step named.
// It pauses the page through the DevTools protocol first and prints the
// JS/wasm stack: a busy loop shows up there, a blocking browser call does not.
let cdp = null;
const watchdog = setTimeout(async () => {
  let where = 'no stack (the page did not pause: blocked outside JavaScript)';
  if (cdp) {
    const paused = new Promise((resolve) => cdp.once('Debugger.paused', (e) => resolve(e)));
    cdp.send('Debugger.pause').catch(() => {});
    const e = await Promise.race([paused, new Promise((r) => setTimeout(() => r(null), 15_000))]);
    if (e) {
      where = e.callFrames
        .slice(0, 25)
        .map((f) => `  ${f.functionName || '(anonymous)'} ${f.url.split('/').pop()}:${f.location.lineNumber + 1}:${f.location.columnNumber + 1}`)
        .join('\n');
    }
  }
  console.error([...problems, `[${step}] no progress for 4 minutes; main thread:`, where].join('\n'));
  process.exit(1);
}, 240_000);
const progress = () => watchdog.refresh();
/** A new browser (and GPU process) with a page that reports problems. */
async function fresh() {
  if (page) {
    const errors = await page.evaluate(() => window.__site?.errors ?? []).catch(() => []);
    problems.push(...errors.map((e) => `chart error: ${e}`));
    page.removeAllListeners(); // requests aborted by the close are not failures
    await page.close().catch(() => {});
    await browser.close().catch(() => {});
  }
  browser = await chromium.launch({ channel: 'chrome', headless: false });
  page = await browser.newPage({ viewport: { width: 1440, height: 900 }, reducedMotion: process.argv.includes('--reduced-motion') ? 'reduce' : 'no-preference' });
  // Counts dust transitions (a snapshot that worked) for the check below.
  await page.addInitScript(() => {
    window.__dust = 0;
    new MutationObserver((list) => {
      for (const m of list) for (const n of m.addedNodes) if (n.classList?.contains('dust')) window.__dust++;
    }).observe(document, { subtree: true, childList: true });
  });
  cdp = await page.context().newCDPSession(page);
  await cdp.send('Debugger.enable');
  page.on('pageerror', (e) => problems.push(`[${step}] page error: ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error') problems.push(`[${step}] console: ${m.text()}`); });
  page.on('requestfailed', (r) => problems.push(`[${step}] request failed: ${r.url()}`));
  page.on('response', (r) => { if (r.status() >= 400) problems.push(`[${step}] HTTP ${r.status()}: ${r.url()}`); });
}
await fresh();

const shot = (name) => shots && page.screenshot({ path: `${shots}/${name}.png` });
const wait = (fn, arg2) => page.waitForFunction(fn, arg2, { timeout: 20000, polling: 250 }).finally(progress);

try {
  await page.goto(`${base}/${renderer ? `?renderer=${renderer}` : ''}`);
  await wait(() => window.__site?.ready);
  await page.waitForTimeout(6500); // the hero's first chart forms
  await shot('hero');
  await page.locator('#library').scrollIntoViewIfNeeded();
  await page.waitForTimeout(2000);
  await shot('library');

  // The opening: the panel grows out of the card, the pen draws the chart,
  // then the rest of the viewer arrives (data-shell) without any input.
  step = 'opening';
  await page.locator('.stack').first().click();
  await wait(() => window.__site.shown);
  await wait(() => document.querySelector('#viewer .panel')?.dataset.shell === 'on');
  await shot('opened');

  step = 'search';
  await page.fill('input[type=search]', 'funding');
  await page.waitForTimeout(300);
  const hits = await page.locator('#viewer-list button').count();
  if (hits < 1) problems.push('search for "funding" found nothing');
  await page.keyboard.press('Escape'); // clears the search

  // The layers: code drawer (C), all charts (A) and details, which takes
  // the live chart and gives it back.
  step = 'code drawer';
  await page.locator('#viewer-list button').first().focus();
  await page.keyboard.press('c');
  await wait(() => document.querySelector('.code-drawer')?.dataset.open === 'on' && document.getElementById('code').textContent.includes('create('));
  await page.keyboard.press('c');
  step = 'all charts';
  await page.keyboard.press('a');
  await page.locator('#all-charts').waitFor();
  const pick = categories[1].demos[0].id;
  await page.locator(`#all-charts button[data-demo="${pick}"]`).click();
  await wait((id) => window.__site.shown === id && !document.getElementById('all-charts'), pick);
  await page.waitForTimeout(1600);
  step = 'details';
  await page.locator('#details-open').click();
  await wait(() => document.querySelector('#details-chart .stage-surface canvas.chart')?.clientWidth > 0);
  await page.waitForTimeout(800);
  await shot('details');
  await page.keyboard.press('Escape'); // closes the details only
  await wait(() => !document.getElementById('details') && document.querySelector('#canvas-host .stage-surface canvas.chart')?.clientWidth > 0);
  if (await page.evaluate(() => document.getElementById('viewer').hidden)) problems.push('Escape in the details closed the viewer');
  await page.keyboard.press('Escape'); // closes
  await wait(() => document.getElementById('viewer').hidden);

  // "Browse all" opens the viewer on the all-charts layer.
  step = 'browse all';
  await page.locator('#browse-all').click();
  await page.locator('#all-charts').waitFor();
  await page.keyboard.press('Escape'); // closes the layer
  await wait(() => !document.getElementById('all-charts'));
  await page.keyboard.press('Escape'); // closes the viewer
  await wait(() => document.getElementById('viewer').hidden);

  // The featured use cases have a stack; every use case has a pill.
  if ((await page.locator('.stack').count()) >= categories.length) problems.push('every use case has a stack on the homepage');
  for (const cat of categories) {
    step = `open ${cat.id}`;
    await page.locator(`.cat-pill[data-cat="${cat.id}"]`).click();
    await wait((id) => window.__site.shown === id, cat.demos[0].id);
    await page.waitForTimeout(1200);
    await shot(`${cat.id}-${cat.demos[0].id}`);
    for (const d of cat.demos.slice(1)) {
      step = `show ${cat.id}/${d.id}`;
      await page.evaluate((id) => window.__site.show(id), d.id);
      await wait((id) => window.__site.shown === id, d.id);
      await page.waitForTimeout(1600); // the dust settles
      await shot(`${cat.id}-${d.id}`);
    }
    step = `close ${cat.id}`;
    await page.keyboard.press('Escape');
    await wait(() => document.getElementById('viewer').hidden);
  }
  // A phone: the viewer becomes one scrolling column.
  step = 'phone';
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator('.stack').first().scrollIntoViewIfNeeded();
  await page.waitForTimeout(1500);
  await shot('phone-library');
  await page.locator('.stack').first().click();
  await wait((id) => window.__site.shown === id, categories[0].demos[0].id);
  await page.waitForTimeout(1200);
  await shot('phone-viewer');
  await page.keyboard.press('Escape');
  await wait(() => document.getElementById('viewer').hidden);

  // Every switch inside a category animates: most should turn to dust.
  const switches = categories.reduce((n, c) => n + c.demos.length - 1, 0);
  const dust = await page.evaluate(() => window.__dust);
  if (!process.argv.includes('--reduced-motion') && dust < switches * 0.8) problems.push(`dust on only ${dust} of ${switches} switches (snapshots failing?)`);
  console.log(`dust on ${dust} of ${switches} switches`);

  const errors = await page.evaluate(() => window.__site.errors);
  problems.push(...errors.map((e) => `chart error: ${e}`));
} catch (e) {
  problems.push(`[${step}] ${e.message.split('\n')[0]}`);
}

clearTimeout(watchdog);
await browser.close();
server?.close();
if (problems.length) {
  console.error(problems.join('\n'));
  process.exit(1);
}
console.log(`site ok: ${categories.length} categories, ${categories.flatMap((c) => c.demos).length} charts${renderer ? ` (${renderer})` : ''}`);
