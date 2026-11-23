<!-- source: 07c5b8920a -->
# 从源码构建

需要 Rust 1.93 或更高版本。构建 Web 包还需要 `wasm32-unknown-unknown` target 和 [wasm-pack](https://github.com/wasm-bindgen/wasm-pack)。运行浏览器测试和对比页面需要 Node.js 20+。

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

## Python wheel 与 C 库

```bash
# Python: a virtualenv with maturin and twine in .venv, plus wasm-pack (for
# the Jupyter front end); the wheel lands in target/wheels
python3 -m venv .venv && .venv/bin/pip install maturin twine
scripts/build-py-wheel.sh
.venv/bin/pip install target/wheels/ferrochart-*.whl

# C library and header (crates/ferrochart-ffi/include/ferrochart.h)
cargo build -p ferrochart-ffi --release
```

## 黄金图像

渲染相关的修改会与 `crates/ferrochart-render-cpu/tests/golden/` 中的参考 PNG 进行比对；CPU 和 GPU 渲染器都必须与之一致。如果某个修改本意就是要改变输出，请用 `FERROCHART_UPDATE_GOLDEN=1 cargo test golden` 重新生成参考图像，并单独提交新图像，说明原因。

## 项目规则

工作规则见 [CONTRIBUTING.md](https://github.com/0x0077/ferrochart/blob/main/CONTRIBUTING.md)。
