// Starts the site's Rust server (site/server) on a free port, serving the
// built site (site/out) and the data API. Builds it first if needed.
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const MANIFEST = fileURLToPath(new URL('../../site/server/Cargo.toml', import.meta.url));
const SITE = fileURLToPath(new URL('../../site/out/', import.meta.url));

/** Resolves to `{ url, close() }`; serves `site` (default site/out). */
export async function startSiteServer(site = SITE) {
  const build = spawnSync('cargo', ['build', '--release', '--quiet', '--manifest-path', MANIFEST], { stdio: 'inherit' });
  if (build.status !== 0) throw new Error('cannot build site/server');
  const bin = fileURLToPath(new URL('../../site/server/target/release/ferrochart-site-server', import.meta.url));
  const child = spawn(bin, ['--addr', '127.0.0.1:0', '--site', site], { env: { ...process.env, RUST_LOG: 'warn,ferrochart_site_server=info', NO_COLOR: '1' } });
  const url = await new Promise((resolve, reject) => {
    let log = '';
    const timer = setTimeout(() => reject(new Error(`server did not start:\n${log}`)), 15000);
    const onData = (d) => {
      log += d;
      const m = log.match(/listening on (http:\/\/[\d.:]+)/);
      if (m) { clearTimeout(timer); resolve(m[1]); }
    };
    child.stdout.on('data', onData);
    child.stderr.on('data', onData);
    child.on('exit', (code) => reject(new Error(`server exited (${code}):\n${log}`)));
  });
  return { url, close: () => child.kill() };
}

/** The catalog the site was built with. */
export async function catalog(url) {
  const r = await fetch(`${url}/catalog.json`);
  if (!r.ok) throw new Error(`catalog.json: HTTP ${r.status}`);
  return (await r.json()).categories;
}
