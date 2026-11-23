<!-- source: 07c5b8920a -->
# ソースからのビルド

Rust 1.93以降が必要です。Webパッケージには`wasm32-unknown-unknown`ターゲットと [wasm-pack](https://github.com/wasm-bindgen/wasm-pack) が、ブラウザーテストと比較ページにはNode.js 20以降が必要です。

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

## PythonのwheelとCライブラリ

```bash
# Python: a virtualenv with maturin and twine in .venv, plus wasm-pack (for
# the Jupyter front end); the wheel lands in target/wheels
python3 -m venv .venv && .venv/bin/pip install maturin twine
scripts/build-py-wheel.sh
.venv/bin/pip install target/wheels/ferrochart-*.whl

# C library and header (crates/ferrochart-ffi/include/ferrochart.h)
cargo build -p ferrochart-ffi --release
```

## 黄金画像

レンダリングの変更は、`crates/ferrochart-render-cpu/tests/golden/`にある参照PNGと照合されます。CPUレンダラーとGPUレンダラーの両方がこれに一致しなければなりません。出力を変えることを意図した変更の場合は、`FERROCHART_UPDATE_GOLDEN=1 cargo test golden`で再生成し、新しい画像を理由を添えて単独でコミットしてください。

## プロジェクトのルール

作業ルールは [CONTRIBUTING.md](https://github.com/0x0077/ferrochart/blob/main/CONTRIBUTING.md) にあります。
