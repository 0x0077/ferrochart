#!/usr/bin/env bash
# The C ABI from C: checks that the committed header matches the
# Rust source (needs the cbindgen CLI: `cargo install cbindgen`), then
# compiles tests/c/smoke.c against it as C99 and as C++ (warnings are
# errors) and runs it with the shared library.
set -euo pipefail
cd "$(dirname "$0")/.."

ffi=crates/ferrochart-ffi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

if command -v cbindgen >/dev/null; then
    (cd "$ffi" && cbindgen --quiet --config cbindgen.toml --output "$tmp/ferrochart.h")
    if ! diff -u "$ffi/include/ferrochart.h" "$tmp/ferrochart.h"; then
        echo "error: $ffi/include/ferrochart.h is out of date; run:" >&2
        echo "  (cd $ffi && cbindgen --config cbindgen.toml --output include/ferrochart.h)" >&2
        exit 1
    fi
else
    echo "(cbindgen not installed: header not checked; cargo install cbindgen)"
fi

cargo build -q -p ferrochart-ffi
lib=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')/debug
flags=(-Wall -Wextra -Werror -I"$ffi/include" "$ffi/tests/c/smoke.c" -L"$lib" -lferrochart -Wl,-rpath,"$lib" -lm)
cc -std=c99 -pedantic -o "$tmp/smoke_c" "${flags[@]}"
c++ -x c++ -std=c++17 -o "$tmp/smoke_cpp" "${flags[@]}"
"$tmp/smoke_c"
"$tmp/smoke_cpp" >/dev/null
echo "C ABI: header up to date, C and C++ smoke tests passed"
