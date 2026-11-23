// The site in other languages: language roots, translated example pages and
// guide chapters, the language menu, addresses kept by the viewer, anchors
// shared with English, hreflang, 404. In Chrome against a built site:
//   node site-i18n.mjs --url=http://127.0.0.1:8090
import { chromium } from 'playwright';
const base = (process.argv.find((a) => a.startsWith('--url='))?.slice(6) ?? 'http://127.0.0.1:8090').replace(/\/$/, '');
let failed = 0;
const b = await chromium.launch({ channel: 'chrome', headless: false });
const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
const errs = [];
p.on('console', (m) => { if (m.type() === 'error') errs.push(`${p.url()}: ${m.text()}`); });
p.on('pageerror', (e) => errs.push(e.message));
const ok = (c, msg) => { if (!c) failed++; console.log(c ? 'PASS' : 'FAIL', msg); };
const shown = (id) => p.waitForFunction((id) => window.__site?.shown === id, id, { timeout: 25000 }).then(() => true, () => false);
const loc = () => p.evaluate(() => location.pathname + location.hash);

await p.goto(`${base}/zh`);
ok((await p.getAttribute('html', 'lang')) === 'zh-CN', 'zh html lang');
ok((await p.title()).includes('跨平台'), `zh title: ${await p.title()}`);
const pill = await p.locator('.cat-pill').first().innerText();
ok(/[一-鿿]/.test(pill), `zh pill: ${pill}`);
const hreflang = await p.locator('link[rel=alternate][hreflang]').evaluateAll((l) => l.map((x) => `${x.hreflang}=${x.getAttribute('href')}`));
ok(hreflang.length === 6, `hreflang: ${hreflang.join(' ')}`);

await p.goto(`${base}/zh/charts/price/ohlc`);
ok(await shown('ohlc'), 'zh chart page opens');
await p.waitForTimeout(1500);
const title = await p.locator('#stage-title').innerText();
const about = await p.locator('#about').innerText();
ok(/[一-鿿]/.test(title) && /[一-鿿]/.test(about), `zh viewer: ${title} | ${about.slice(0, 40)}`);
ok((await loc()) === '/zh/charts/price/ohlc', `address ${await loc()}`);
await p.evaluate(() => window.__site.show('heikin'));
await p.waitForTimeout(1500);
const about2 = await p.locator('#about').innerText();
ok((await loc()) === '/zh/charts/price/heikin' && /[一-鿿]/.test(about2), `switch: ${await loc()} | ${about2.slice(0, 30)}`);
await p.fill('input[type=search]', '足迹');
await p.waitForTimeout(400);
ok((await p.locator('#viewer-list button').count()) > 0, 'search in Chinese finds charts');
await p.keyboard.press('Escape');
await p.keyboard.press('Escape');
await p.waitForFunction(() => document.getElementById('viewer').hidden, null, { timeout: 10000 });
ok((await loc()) === '/zh', `close → ${await loc()}`);

await p.goto(`${base}/zh/docs/web`);
await p.locator('.lang-menu summary').click();
await p.locator('.lang-menu a[lang=ja]').click();
await p.waitForURL(/\/ja\/docs\/web/);
ok((await p.getAttribute('html', 'lang')) === 'ja', `menu → ${await loc()}`);
const para = await p.locator('article.doc p').first().innerText();
ok(/[\u3040-\u30ff]/.test(para), `ja chapter text: ${para.slice(0, 30)}`);

// Anchors come from the English chapter.
await p.goto(`${base}/docs/data`);
const enIds = await p.locator('article.doc :is(h2,h3)').evaluateAll((l) => l.map((x) => x.id));
await p.goto(`${base}/ko/docs/data`);
const koIds = await p.locator('article.doc :is(h2,h3)').evaluateAll((l) => l.map((x) => x.id));
ok(enIds.join() === koIds.join(), `ko anchors = en (${koIds.length})`);
const apiHref = await p.locator('nav a', { hasText: 'All classes' }).count() + await p.locator('nav a[href="/docs/api"]').count();
ok(apiHref > 0, 'ko nav links the English API reference');
await p.locator('.lang-menu summary').click();
await p.locator('.lang-menu a[lang=en]').click();
await p.waitForURL(/\/docs\/data\/?$/);
ok((await loc()).replace(/\/$/, '') === '/docs/data', `back to English: ${await loc()}`);

await p.goto(`${base}/docs/api`);
await p.locator('.lang-menu summary').click();
await p.locator('.lang-menu a[lang=zh-tw]').click();
await p.waitForURL(/zh-tw/);
ok((await loc()).replace(/\/$/, '') === '/zh-tw/docs', `API page → ${await loc()}`);

const r = await p.goto(`${base}/zh/nope`);
ok(r.status() === 404, `404 (${r.status()})`);
await p.goto(`${base}/ko/pro`);
await p.waitForTimeout(3000);
ok((await p.title()).includes('트레이딩'), `ko pro title: ${await p.title()}`);
const real = errs.filter((e) => !/404|Failed to load resource/.test(e));
console.log('errors:', real.join('\n') || 'none');
await b.close();
process.exit(failed || real.length ? 1 : 0);
