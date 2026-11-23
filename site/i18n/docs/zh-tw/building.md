<!-- source: 07c5b8920a -->
# 從原始碼建置

需要 Rust 1.93 或更新版本。建置 Web 套件需要 `wasm32-unknown-unknown` 目標與 [wasm-pack](https://github.com/wasm-bindgen/wasm-pack)。執行瀏覽器測試與比較頁面需要 Node.js 20+。

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

## Python wheel 與 C 函式庫

```bash
# Python: a virtualenv with maturin and twine in .venv, plus wasm-pack (for
# the Jupyter front end); the wheel lands in target/wheels
python3 -m venv .venv && .venv/bin/pip install maturin twine
scripts/build-py-wheel.sh
.venv/bin/pip install target/wheels/ferrochart-*.whl

# C library and header (crates/ferrochart-ffi/include/ferrochart.h)
cargo build -p ferrochart-ffi --release
```

## 黃金圖像

渲染相關的變更會與 `crates/ferrochart-render-cpu/tests/golden/` 中的參考 PNG 比對；CPU 與 GPU 渲染器都必須與之相符。若某項變更本來就會改變輸出，請以 `FERROCHART_UPDATE_GOLDEN=1 cargo test golden` 重新產生它們，並將新影像單獨提交，同時說明原因。

## 專案規則

工作規則請參見 [CONTRIBUTING.md](https://github.com/0x0077/ferrochart/blob/main/CONTRIBUTING.md)。
