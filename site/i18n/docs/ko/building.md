<!-- source: 07c5b8920a -->
# 소스에서 빌드하기

Rust 1.93 이상이 필요합니다. 웹 패키지에는 `wasm32-unknown-unknown` 타깃과
[wasm-pack](https://github.com/wasm-bindgen/wasm-pack)이, 브라우저 테스트와 비교
페이지에는 Node.js 20 이상이 필요합니다.

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

## Python 휠과 C 라이브러리

```bash
# Python: a virtualenv with maturin and twine in .venv, plus wasm-pack (for
# the Jupyter front end); the wheel lands in target/wheels
python3 -m venv .venv && .venv/bin/pip install maturin twine
scripts/build-py-wheel.sh
.venv/bin/pip install target/wheels/ferrochart-*.whl

# C library and header (crates/ferrochart-ffi/include/ferrochart.h)
cargo build -p ferrochart-ffi --release
```

## 골든 이미지

렌더링 변경은 `crates/ferrochart-render-cpu/tests/golden/`에 있는 기준 PNG와
비교해 검사합니다. CPU 렌더러와 GPU 렌더러 모두 이 이미지와 일치해야 합니다.
의도적으로 출력을 바꾸는 변경이라면
`FERROCHART_UPDATE_GOLDEN=1 cargo test golden`으로 이미지를 다시 생성하고, 새
이미지는 이유를 설명하여 별도의 커밋으로 올리세요.

## 프로젝트 규칙

작업 규칙은
[CONTRIBUTING.md](https://github.com/0x0077/ferrochart/blob/main/CONTRIBUTING.md)에 있습니다.
