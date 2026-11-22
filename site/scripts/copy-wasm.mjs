// Copies the engine's WebAssembly build into public/wasm/, where the site
// loads it from at run time. Build it first:
//
//   (cd ../crates/ferrochart-wasm && wasm-pack build --release --target web)
// In the site's own repository the build is a copy in vendor/wasm.
import { existsSync } from 'node:fs';
import { copyFile, mkdir, access } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const VENDORED = new URL('../vendor/wasm/', import.meta.url);
const PKG = existsSync(VENDORED) ? VENDORED : new URL('../../crates/ferrochart-wasm/pkg/', import.meta.url);
const OUT = new URL('../public/wasm/', import.meta.url);
const FILES = ['ferrochart_wasm.js', 'ferrochart_wasm_bg.wasm'];

try {
  await access(new URL(FILES[1], PKG));
} catch {
  console.error(`No engine build in ${fileURLToPath(PKG)}.\nRun: (cd crates/ferrochart-wasm && wasm-pack build --release --target web)`);
  process.exit(1);
}
await mkdir(OUT, { recursive: true });
for (const f of FILES) await copyFile(new URL(f, PKG), new URL(f, OUT));
console.log(`engine copied to ${fileURLToPath(OUT)}`);
