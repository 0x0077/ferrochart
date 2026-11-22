# Building from source

Requires Rust 1.93 or newer. For the web package: the `wasm32-unknown-unknown`
target and [wasm-pack](https://github.com/wasm-bindgen/wasm-pack). For the
browser tests and the comparison page: Node.js 20+.

```bash
git clone https://github.com/0x0077/ferrochart.git
cd ferrochart

scripts/check.sh                       # everything below, as run before each commit
cargo test --workspace --exclude ferrochart-py
cargo run -p example-desktop --release

# Web package and demo
rustup target add wasm32-unknown-unknown
(cd crates/ferrochart-wasm && wasm-pack build --release --target web)
python3 -m http.server 8000            # open http://localhost:8000/examples/web/

# Browser smoke test (Chrome, Firefox, WebKit) and TypeScript type check
cd tests/web && npm install && npx playwright install firefox webkit
node smoke.mjs && npx tsc -p .

# This book
mdbook serve docs/book
```

## Python wheel and C library

```bash
# Python: a virtualenv with maturin and twine in .venv, plus wasm-pack (for
# the Jupyter front end); the wheel lands in target/wheels
python3 -m venv .venv && .venv/bin/pip install maturin twine
scripts/build-py-wheel.sh
.venv/bin/pip install target/wheels/ferrochart-*.whl

# C library and header (crates/ferrochart-ffi/include/ferrochart.h)
cargo build -p ferrochart-ffi --release
```

## Golden images

Rendering changes are checked against reference PNGs in
`crates/ferrochart-render-cpu/tests/golden/`; both the CPU and the GPU renderer
must match them. If a change is meant to alter the output, regenerate them with
`FERROCHART_UPDATE_GOLDEN=1 cargo test golden` and commit the new images on
their own, explaining why.

## Project rules

The working rules are in
[CONTRIBUTING.md](https://github.com/0x0077/ferrochart/blob/main/CONTRIBUTING.md).
