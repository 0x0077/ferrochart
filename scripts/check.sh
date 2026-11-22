#!/usr/bin/env bash
# Local checks (no hosted CI).
# Usage: scripts/check.sh
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }

step "fmt"
cargo fmt --all --check

step "clippy (workspace)"
cargo clippy --workspace --exclude ferrochart-py --all-targets -- -D warnings

step "clippy (core, all features)"
cargo clippy -p ferrochart-core --all-features --all-targets -- -D warnings

step "test (workspace)"
cargo test --workspace --exclude ferrochart-py

step "test (core, features testing + serde)"
cargo test -p ferrochart-core --features testing,serde

step "test (core + indicators, feature rayon)"
cargo test -p ferrochart-core --features testing,serde,rayon
cargo test -p ferrochart-indicators --features ferrochart-core/rayon

step "C ABI (header, C and C++ smoke tests)"
scripts/test-ffi-c.sh
if command -v dotnet >/dev/null; then
    step "C ABI from C# (examples/csharp)"
    scripts/test-ffi-csharp.sh
else
    echo "(skipping the C# example: no dotnet)"
fi

step "wasm32 build (core)"
cargo build -p ferrochart-core --target wasm32-unknown-unknown --all-features

step "wasm32 build (render-wgpu, WebGL2)"
cargo build -p ferrochart-render-wgpu --target wasm32-unknown-unknown --features webgl

step "clippy (wasm binding, wasm32)"
cargo clippy -p ferrochart-wasm --target wasm32-unknown-unknown -- -D warnings

# Python binding: with a virtualenv in .venv (or $VIRTUAL_ENV) that
# has maturin and pytest, e.g.
#   uv venv .venv && VIRTUAL_ENV=.venv uv pip install maturin pytest numpy pandas polars
venv=${VIRTUAL_ENV:-$PWD/.venv}
if [ -x "$venv/bin/maturin" ] && [ -x "$venv/bin/python" ]; then
    step "clippy (Python binding)"
    PYO3_PYTHON="$venv/bin/python" cargo clippy -p ferrochart-py --all-targets -- -D warnings
    step "test (Python binding)"
    PYO3_PYTHON="$venv/bin/python" cargo test -p ferrochart-py
    (cd crates/ferrochart-py && VIRTUAL_ENV="$venv" "$venv/bin/maturin" develop --release -q)
    "$venv/bin/python" -m pytest -q crates/ferrochart-py/tests
    # show(block=False) is Linux/Windows code: type-check it where possible.
    for t in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc; do
        if rustup target list --installed | grep -q "^$t$"; then
            step "clippy (Python binding, $t)"
            PYO3_CROSS_PYTHON_VERSION=3.10 PYO3_NO_PYTHON=1 \
                cargo clippy -p ferrochart-py --target "$t" -- -D warnings
        fi
    done
else
    echo "(skipping the Python binding: no maturin in $venv)"
fi

# The website's backend (site/server): its own Cargo project, not in the
# engine workspace.
step "site server (fmt, clippy, test)"
cargo fmt --manifest-path site/server/Cargo.toml --check
cargo clippy --manifest-path site/server/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path site/server/Cargo.toml --locked

if command -v mdbook >/dev/null; then
    step "book"
    mdbook build docs/book >/dev/null
fi

msrv=$(sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml)
step "MSRV $msrv"
if rustup toolchain list | grep -q "^$msrv-"; then
    # example-egui: egui needs a newer Rust (see its Cargo.toml).
    cargo "+$msrv" check --workspace --exclude ferrochart-py --exclude example-egui --all-targets --locked
    cargo "+$msrv" test -p ferrochart-core --features testing --locked
else
    echo "Rust $msrv is not installed. Run: rustup toolchain install $msrv --profile minimal" >&2
    exit 1
fi

printf '\nAll checks passed.\n'
