#!/usr/bin/env bash
# Builds the PyPI wheel of this platform into target/wheels and checks
# it: `twine check`, the files it must contain, and no internal documents or
# local paths. Publishing is a separate, manual step. Needs a virtualenv in
# .venv (or $VIRTUAL_ENV) with maturin and twine, and wasm-pack.
set -euo pipefail
cd "$(dirname "$0")/.."

venv=${VIRTUAL_ENV:-$PWD/.venv}
out=$PWD/target/wheels
# The Jupyter front end, built with the same flags.
scripts/build-py-static.sh

. scripts/release-env.sh
rm -rf "$out"
(cd crates/ferrochart-py && VIRTUAL_ENV="$venv" "$venv/bin/maturin" build --release -o "$out")
"$venv/bin/twine" check --strict "$out"/*.whl

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
for w in "$out"/*.whl; do
    dir="$tmp/$(basename "$w")"
    unzip -q "$w" -d "$dir"
    for f in ferrochart/__init__.py ferrochart/static/widget.js ferrochart/static/ferrochart_wasm.js \
        ferrochart/static/ferrochart_wasm_bg.wasm ferrochart/py.typed; do
        [ -f "$dir/$f" ] || { echo "error: $w lacks $f" >&2; exit 1; }
    done
    ls "$dir"/ferrochart/_native.* >/dev/null
    ls "$dir"/*.dist-info/licenses/LICENSE-APACHE >/dev/null
    if find "$dir" -name '__pycache__' -o -name '*.pyc' -o -name 'sboms' | grep -q .; then
        echo "error: $w contains byte code or an SBOM" >&2
        exit 1
    fi
done
scripts/check-shipped.sh "$tmp"
ls -l "$out"
