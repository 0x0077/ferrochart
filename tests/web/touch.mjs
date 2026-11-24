// Browser test of touch input, touchScroll and PointerCancel: runs
// tests/web/pages/touch.html.
//
//   scripts/build-npm.sh
//   cd tests/web && node touch.mjs [chromium|webkit ...]
//
// chromium: a Pixel 7 with real touch input through CDP Input.dispatchTouchEvent
//   (two-finger pinch, long-press crosshair, touchScroll 'page' vs 'chart',
//   pointercancel when the page takes a swipe over).
// webkit: an iPhone 15 with Playwright's touchscreen (tap, double tap).
//   Playwright cannot hold a finger down in WebKit, so the long press there
//   is dispatched as touch PointerEvents; two fingers are checked on devices.
import { chromium, webkit, devices } from 'playwright';
import { serve } from './serve.mjs';

const results = [];
const check = (label, name, ok, detail = '') => {
  results.push({ label, name, ok: !!ok });
  if (!ok) console.log(`  FAIL ${label}: ${name} ${typeof detail === 'string' ? detail : JSON.stringify(detail)}`);
};

async function open(browser, device, port, touchScroll) {
  const context = await browser.newContext({ ...devices[device] });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  await page.goto(`http://127.0.0.1:${port}/tests/web/pages/touch.html?touchScroll=${touchScroll}`);
  await page.waitForFunction(() => window.__ready, null, { timeout: 30_000 });
  await page.waitForTimeout(200);
  return { context, page, errors, state: () => page.evaluate(() => window.__state()) };
}

/** Chromium: real touches through CDP. */
async function chromiumTests(port) {
  const label = 'chromium/Pixel 7';
  const browser = await chromium.launch({ headless: false });
  try {
    for (const mode of ['chart', 'page']) {
      const { context, page, errors, state } = await open(browser, 'Pixel 7', port, mode);
      const cdp = await context.newCDPSession(page);
      const touch = (type, points) => cdp.send('Input.dispatchTouchEvent', {
        type, touchPoints: points.map(([x, y], id) => ({ x, y, id })),
      });
      const swipe = async (from, to, steps = 10) => {
        await touch('touchStart', [from]);
        for (let i = 1; i <= steps; i++) {
          await touch('touchMove', [[from[0] + (to[0] - from[0]) * i / steps, from[1] + (to[1] - from[1]) * i / steps]]);
          await page.waitForTimeout(16);
        }
        await touch('touchEnd', []);
        await page.waitForTimeout(150);
      };
      const l = `${label}/${mode}`;

      // Vertical swipe: the page scrolls in 'page' mode (the chart gets a
      // pointercancel: no click, no pan); the chart keeps it in 'chart' mode.
      let s0 = await state();
      await swipe([180, 200], [180, 40]);
      let s1 = await state();
      if (mode === 'page') {
        check(l, 'vertical swipe scrolls the page', s1.scrollY > 50, s1);
        check(l, 'scrolled swipe is no click', s1.clicks === s0.clicks, s1);
        check(l, 'scrolled swipe does not pan', s1.from === s0.from, [s0, s1]);
        await page.waitForTimeout(1500); // the fling ends
        await page.evaluate(() => window.scrollTo(0, 0));
        await page.waitForTimeout(100);
      } else {
        check(l, 'vertical swipe does not scroll the page', s1.scrollY === 0, s1);
      }

      // Horizontal swipe pans the chart in both modes, without a crosshair.
      s0 = await state();
      await swipe([250, 150], [100, 150]);
      s1 = await state();
      check(l, 'horizontal swipe pans', s1.from > s0.from + 3, [s0, s1]);
      check(l, 'horizontal swipe keeps the page', s1.scrollY === s0.scrollY, [s0, s1]);
      check(l, 'finger pan shows no crosshair', s1.crosshair !== true, s1);
      await page.waitForTimeout(1500); // kinetic scrolling ends

      // Two fingers: pinch out to twice the distance.
      s0 = await state();
      await touch('touchStart', [[140, 150]]);
      await touch('touchStart', [[140, 150], [220, 150]]);
      for (let i = 1; i <= 10; i++) {
        await touch('touchMove', [[140 - 4 * i, 150], [220 + 4 * i, 150]]);
        await page.waitForTimeout(16);
      }
      await touch('touchEnd', [[100, 150]]);
      await touch('touchEnd', []);
      await page.waitForTimeout(150);
      s1 = await state();
      const ratio = s1.spacing / s0.spacing;
      check(l, 'pinch zooms by the distance ratio', Math.abs(ratio - 2) < 0.05, { ratio, s0, s1 });
      check(l, 'pinch keeps the page', s1.scrollY === s0.scrollY, [s0, s1]);
      check(l, 'pinch is no click', s1.clicks === s0.clicks, s1);

      // Long press: crosshair after 500 ms; it follows the finger (also
      // vertically in 'page' mode), stays after lifting; a tap hides it.
      await touch('touchStart', [[180, 120]]);
      await page.waitForTimeout(250);
      check(l, 'no crosshair before 500 ms', (await state()).crosshair !== true);
      await page.waitForTimeout(450);
      s0 = await state();
      check(l, 'long press shows the crosshair', s0.crosshair === true, s0);
      for (let i = 1; i <= 8; i++) {
        await touch('touchMove', [[180, 120 + 10 * i]]);
        await page.waitForTimeout(16);
      }
      await page.waitForTimeout(100);
      s1 = await state();
      check(l, 'crosshair follows the finger', s1.crosshairs > s0.crosshairs && s1.crosshair === true, [s0, s1]);
      check(l, 'crosshair drag keeps the page and the view', s1.scrollY === s0.scrollY && s1.from === s0.from, [s0, s1]);
      await touch('touchEnd', []);
      await page.waitForTimeout(150);
      const s2 = await state();
      check(l, 'crosshair stays after lifting', s2.crosshair === true && s2.clicks === s1.clicks, s2);
      await touch('touchStart', [[200, 150]]);
      await touch('touchEnd', []);
      await page.waitForTimeout(150);
      const s3 = await state();
      check(l, 'a tap hides the crosshair and clicks', s3.crosshair === false && s3.clicks === s2.clicks + 1, s3);

      check(l, 'no page errors', errors.length === 0, errors);
      await context.close();
    }
  } finally {
    await browser.close();
  }
}

/** WebKit: taps through Playwright's touchscreen. */
async function webkitTests(port) {
  const l = 'webkit/iPhone 15';
  const browser = await webkit.launch({ headless: false });
  try {
    const { context, page, errors, state } = await open(browser, 'iPhone 15', port, 'page');
    // A tap is a click.
    let s0 = await state();
    await page.touchscreen.tap(180, 120);
    await page.waitForTimeout(400);
    let s1 = await state();
    check(l, 'tap clicks', s1.clicks === s0.clicks + 1, s1);
    // A double tap on the time axis resets the bar spacing.
    await page.evaluate(() => window.__chart.setBarSpacing(30));
    s0 = await state();
    await page.touchscreen.tap(180, 292);
    await page.waitForTimeout(60);
    await page.touchscreen.tap(181, 292);
    await page.waitForTimeout(200);
    s1 = await state();
    check(l, 'double tap on the time axis resets the spacing', s0.spacing === 30 && s1.spacing !== 30, [s0, s1]);
    // Long press, as touch PointerEvents (see the top of the file).
    await page.evaluate(() => {
      const c = document.getElementById('c');
      const r = c.getBoundingClientRect();
      window.__pointer = (type, x, y) => c.dispatchEvent(new PointerEvent(type, {
        pointerId: 5, pointerType: 'touch', isPrimary: true, bubbles: true, cancelable: true,
        clientX: r.left + x, clientY: r.top + y, button: type === 'pointermove' ? -1 : 0,
        buttons: type === 'pointerup' ? 0 : 1,
      }));
      window.__pointer('pointerdown', 180, 120);
    });
    await page.waitForTimeout(250);
    check(l, 'no crosshair before 500 ms', (await state()).crosshair !== true);
    await page.waitForTimeout(450);
    s0 = await state();
    check(l, 'long press shows the crosshair', s0.crosshair === true, s0);
    await page.evaluate(() => { window.__pointer('pointermove', 180, 160); window.__pointer('pointerup', 180, 160); });
    await page.waitForTimeout(150);
    s1 = await state();
    check(l, 'crosshair stays after lifting, no click', s1.crosshair === true && s1.clicks === s0.clicks, s1);
    check(l, 'no page errors', errors.length === 0, errors);
    await context.close();
  } finally {
    await browser.close();
  }
}

const server = await serve();
const { port } = server.address();
const names = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'webkit'];
for (const name of names) {
  try {
    await ({ chromium: chromiumTests, webkit: webkitTests })[name](port);
  } catch (e) {
    check(name, 'run', false, e.message);
  }
}
server.close();
const failed = results.filter((r) => !r.ok);
for (const label of [...new Set(results.map((r) => r.label))]) {
  const rs = results.filter((r) => r.label === label);
  console.log(`${rs.every((r) => r.ok) ? 'PASS' : 'FAIL'} ${label}: ${rs.filter((r) => r.ok).length}/${rs.length} checks`);
}
process.exit(failed.length ? 1 : 0);
