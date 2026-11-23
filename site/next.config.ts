import type { NextConfig } from 'next';

// A static site: `next build` writes plain files to out/, served from any
// static host at https://ferrochart.com. Charts run in the browser (the
// engine is WebAssembly, copied to public/wasm/ by scripts/copy-wasm.mjs).
const config: NextConfig = {
  output: 'export',
  images: { unoptimized: true },
  reactStrictMode: true,
  poweredByHeader: false,
  // app/global-not-found.tsx: one 404 page for the per-language root layouts.
  experimental: { globalNotFound: true },
};

export default config;
