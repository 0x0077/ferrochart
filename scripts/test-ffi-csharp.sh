#!/usr/bin/env bash
# The C ABI from C#: builds the shared library and runs the
# P/Invoke example in examples/csharp, which exits non-zero if any of its
# checks fail. Needs the .NET SDK (10 or later). Usage:
#   scripts/test-ffi-csharp.sh [output directory]   (default: a temporary one)
set -euo pipefail
cd "$(dirname "$0")/.."

out=${1:-}
if [ -z "$out" ]; then
    out=$(mktemp -d)
    trap 'rm -rf "$out"' EXIT
fi

# Release: the example renders 300 frames on the CPU (about 20× slower in debug).
cargo build -q -p ferrochart-ffi --release
target=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
case "$(uname -s)" in
    Darwin) lib=libferrochart.dylib ;;
    MINGW* | MSYS* | CYGWIN*) lib=ferrochart.dll ;;
    *) lib=libferrochart.so ;;
esac
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
FERROCHART_LIB="$target/release/$lib" dotnet run --project examples/csharp -- "$out"
test -s "$out/ferrochart-csharp.png"
echo "C# example passed"
