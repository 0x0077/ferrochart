#!/usr/bin/env bash
# Builds the npm package @ferrochart/core into crates/ferrochart-wasm/pkg
# and checks its contents with `npm pack --dry-run`.
# Publishing is a separate, manual step: `cd crates/ferrochart-wasm/pkg && npm publish --access public`.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)

. scripts/release-env.sh
(cd crates/ferrochart-wasm && wasm-pack build --release --target web)

pkg=crates/ferrochart-wasm/pkg
cp LICENSE-APACHE "$pkg/"
# lightweight-charts compatibility layer: '@ferrochart/core/lightweight-charts'.
# The entry point (index.js: init on first create) is hand-written too.
for f in index.js index.d.ts lightweight-charts.js lightweight-charts.d.ts; do
    cp "crates/ferrochart-wasm/js/$f" "$pkg/$f"
done
rm -f "$pkg/.gitignore"

node - "$pkg/package.json" "$version" <<'EOF'
const fs = require('node:fs');
const [file, version] = process.argv.slice(2);
const p = JSON.parse(fs.readFileSync(file, 'utf8'));
const out = {
  name: '@ferrochart/core',
  version,
  description: 'High-performance financial charts (WebGPU/WebGL2) compiled from Rust',
  license: p.license,
  repository: { type: 'git', url: 'git+https://github.com/0x0077/ferrochart.git' },
  homepage: 'https://ferrochart.com',
  bugs: 'https://github.com/0x0077/ferrochart/issues',
  keywords: ['chart', 'candlestick', 'financial', 'trading', 'webgpu', 'webgl', 'wasm'],
  type: 'module',
  main: 'index.js',
  module: 'index.js',
  types: 'index.d.ts',
  exports: {
    '.': { types: './index.d.ts', default: './index.js' },
    './lightweight-charts': {
      types: './lightweight-charts.d.ts',
      default: './lightweight-charts.js',
    },
  },
  sideEffects: p.sideEffects ?? false,
  files: [
    ...new Set([
      ...p.files,
      'index.js',
      'index.d.ts',
      'lightweight-charts.js',
      'lightweight-charts.d.ts',
      'README.md',
      'LICENSE-APACHE',
    ]),
  ],
};
fs.writeFileSync(file, JSON.stringify(out, null, 2) + '\n');
EOF

# Every shipped file, the .wasm included (shader sources, panic locations).
scripts/check-shipped.sh "$pkg"

wasm="$pkg/ferrochart_wasm_bg.wasm"
printf 'wasm: %s bytes, %s bytes gzip (P7 budget 1572864)\n' \
    "$(wc -c < "$wasm" | tr -d ' ')" "$(gzip -9 -c "$wasm" | wc -c | tr -d ' ')"
(cd "$pkg" && npm pack --dry-run 2>&1 | sed -n '/Tarball Contents/,$p')
