// Writes .br and .gz next to the text and WebAssembly files of out/, which
// the Rust server (server/) sends to browsers that accept them, so nothing
// is compressed per request.
import { readdir, readFile, writeFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { brotliCompressSync, gzipSync, constants } from 'node:zlib';

const OUT = new URL('../out/', import.meta.url).pathname;
const TYPES = /\.(html|js|css|json|txt|svg|xml|wasm|map)$/;

async function* files(dir) {
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* files(p);
    else if (TYPES.test(e.name)) yield p;
  }
}

let n = 0, before = 0, after = 0;
for await (const f of files(OUT)) {
  const data = await readFile(f);
  if (data.length < 1024) continue;
  const br = brotliCompressSync(data, { params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_SIZE_HINT]: data.length } });
  await writeFile(`${f}.br`, br);
  await writeFile(`${f}.gz`, gzipSync(data, { level: 9 }));
  n++; before += data.length; after += br.length;
}
console.log(`precompressed ${n} files: ${(before / 1e6).toFixed(2)} MB → ${(after / 1e6).toFixed(2)} MB brotli`);
