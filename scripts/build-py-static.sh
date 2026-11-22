#!/usr/bin/env bash
# Builds the Jupyter front end of the Python package: the
# WebAssembly build of ferrochart-wasm, copied into
# crates/ferrochart-py/python/ferrochart/static/ (generated, not committed;
# widget.js there is source). Run before `maturin develop` / `maturin build`
# when the wasm binding changed.
set -euo pipefail
cd "$(dirname "$0")/.."

. scripts/release-env.sh
(cd crates/ferrochart-wasm && wasm-pack build --release --target web)

pkg=crates/ferrochart-wasm/pkg
static=crates/ferrochart-py/python/ferrochart/static
cp "$pkg/ferrochart_wasm.js" "$static/"
cp "$pkg/ferrochart_wasm_bg.wasm" "$static/"
scripts/check-shipped.sh "$static"
ls -l "$static"
