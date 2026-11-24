import type { NextConfig } from 'next';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';

/**
 * Content hash of the Pro demo build (copied to public/pro/ by the
 * prebuild/predev script). Its file name never changes, so the hash goes
 * into the URL (components/ProDemo.tsx): a new build is never served from a
 * browser or CDN cache. Empty without a Pro build.
 */
function proVersion(): string {
  try {
    const file = path.join(process.cwd(), 'public', 'pro', 'ferrochart-pro.js');
    return createHash('sha256').update(readFileSync(file)).digest('hex').slice(0, 10);
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
  env: { NEXT_PUBLIC_PRO_VERSION: proVersion() },
  // app/global-not-found.tsx: one 404 page for the per-language root layouts.
  experimental: { globalNotFound: true },
};

export default config;
