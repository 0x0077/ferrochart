// Smoke test in the real Safari through safaridriver (WebDriver), since
// Playwright's WebKit is not Safari.
//
// One-time setup: Safari → Settings → Advanced → "Show features for web
// developers", then Develop → "Allow Remote Automation".
//
//   (cd crates/ferrochart-wasm && wasm-pack build --release --target web)
//   cd tests/web && npm install && node safari.mjs
//
// For each renderer: load examples/web with 1M bars, wait for the chart,
// check that candles are drawn, drag-pan with trusted WebDriver input and
// check that the frame changed.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';

const ROOT = fileURLToPath(new URL('../..', import.meta.url));
const OUT = fileURLToPath(new URL('./out/', import.meta.url));
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };
const DRIVER_PORT = 4445;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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

async function wd(method, path, body) {
  const res = await fetch(`http://127.0.0.1:${DRIVER_PORT}${path}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await res.json();
  if (json.value?.error) throw new Error(`${json.value.error}: ${json.value.message}`);
  return json.value;
}

function candlePixels(png) {
  const { data } = PNG.sync.read(png);
  let colored = 0;
  for (let i = 0; i < data.length; i += 4) {
    const [r, g, b] = [data[i], data[i + 1], data[i + 2]];
    if ((g > r + 40 && g > b && g > 100) || (r > g + 60 && r > 150)) colored++;
  }
  return colored / (data.length / 4);
}

async function run(sid, port, renderer) {
  const s = `/session/${sid}`;
  await wd('POST', `${s}/url`, { url: `http://127.0.0.1:${port}/examples/web/?bars=1000000&renderer=${renderer}&live=1` });
  let state = null;
  for (let i = 0; i < 120; i++) {
    state = await wd('POST', `${s}/execute/sync`, {
      script: 'return window.__ferro ? { ready: window.__ferro.ready, error: window.__ferro.error, renderer: window.__ferro.renderer, setDataMs: window.__ferro.setDataMs } : null;',
      args: [],
    });
    if (state?.ready || state?.error) break;
    await sleep(500);
  }
  if (!state?.ready) throw new Error(state?.error ?? 'chart not ready after 60 s');
  const ua = await wd('POST', `${s}/execute/sync`, { script: 'return navigator.userAgent;', args: [] });
  await sleep(500);
  const canvas = await wd('POST', `${s}/element`, { using: 'css selector', value: '#chart' });
  const el = Object.values(canvas)[0];
  const before = Buffer.from(await wd('GET', `${s}/element/${el}/screenshot`), 'base64');
  const pointer = [{ type: 'pointerMove', duration: 0, origin: { 'element-6066-11e4-a52e-4f735466cecf': el }, x: 0, y: 0 },
    { type: 'pointerDown', button: 0 }];
  for (let i = 1; i <= 10; i++) pointer.push({ type: 'pointerMove', duration: 30, origin: 'pointer', x: 20, y: 0 });
  pointer.push({ type: 'pointerUp', button: 0 });
  await wd('POST', `${s}/actions`, { actions: [{ type: 'pointer', id: 'mouse', parameters: { pointerType: 'mouse' }, actions: pointer }] });
  await wd('DELETE', `${s}/actions`);
  await sleep(800);
  const after = Buffer.from(await wd('GET', `${s}/element/${el}/screenshot`), 'base64');
  await mkdir(OUT, { recursive: true });
  await writeFile(join(OUT, `safari-${renderer}.png`), after);
  const colored = candlePixels(after);
  const changed = !before.equals(after);
  const ok = state.renderer && changed && colored > 0.002;
  console.log(`${ok ? 'PASS' : 'FAIL'} safari/${renderer}: backend=${state.renderer} setData=${state.setDataMs.toFixed(0)}ms`
    + ` candlePixels=${(colored * 100).toFixed(2)}% interactionChangedFrame=${changed}`);
  return { ok, ua };
}

const server = await serve();
const { port } = server.address();
const driver = spawn('safaridriver', ['-p', String(DRIVER_PORT)], { stdio: 'ignore' });
await sleep(1500);
let ok = true;
let sid = null;
try {
  const session = await wd('POST', '/session', { capabilities: { alwaysMatch: { browserName: 'safari' } } });
  sid = session.sessionId;
  await wd('POST', `/session/${sid}/window/rect`, { width: 1200, height: 800 });
  for (const renderer of ['auto', 'webgl2']) {
    try {
      const r = await run(sid, port, renderer);
      ok = r.ok && ok;
      if (renderer === 'auto') console.log(`  ${r.ua}`);
    } catch (e) {
      console.log(`FAIL safari/${renderer}: ${e.message}`);
      ok = false;
    }
  }
} finally {
  if (sid) await wd('DELETE', `/session/${sid}`).catch(() => {});
  driver.kill();
  server.close();
}
process.exit(ok ? 0 : 1);
