import type { NextConfig } from 'next';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';

/**
 * Content hash of files under public/ (copied there by the prebuild/predev
 * scripts). Their names never change, so the hash goes into their URLs: a
 * new build is never served from a browser or CDN cache. Empty when a file
 * is missing.
 */
function version(...files: string[]): string {
  try {
    const h = createHash('sha256');
    for (const f of files) h.update(readFileSync(path.join(process.cwd(), 'public', f)));
    return h.digest('hex').slice(0, 10);
  } catch {
    return '';
  }
}

// A static site: `next build` writes plain files to out/, served from any
// static host at https://ferrochart.com. Charts run in the browser (the
// engine is WebAssembly, copied to public/wasm/ by scripts/copy-wasm.mjs).
const config: NextConfig = {
  output: 'export',
  images: { unoptimized: true },
  reactStrictMode: true,
  poweredByHeader: false,
  env: {
    // The engine (lib/engine.js) and the Pro demo (components/ProDemo.tsx).
    NEXT_PUBLIC_WASM_VERSION: version('wasm/ferrochart_wasm.js', 'wasm/ferrochart_wasm_bg.wasm'),
    NEXT_PUBLIC_PRO_VERSION: version('pro/ferrochart-pro.js'),
  },
  // app/global-not-found.tsx: one 404 page for the per-language root layouts.
  experimental: { globalNotFound: true },
};

export default config;
