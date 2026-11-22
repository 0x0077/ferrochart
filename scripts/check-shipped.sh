#!/usr/bin/env bash
# Fails if shipped files contain the build machine's paths. Usage: scripts/check-shipped.sh FILE_OR_DIR...
#
# Every file is read through `strings`: grep alone may skip matches inside
# binaries (.wasm, .so), which embed shader sources and panic locations.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$(pwd -P)
cd - >/dev/null

pattern="$HOME/|$root"
found=0
while IFS= read -r -d '' f; do
    # All matches (stopping early would end `strings` with SIGPIPE, which
    # pipefail reports as a failure); grep's status 1 means none.
    hits=$(LC_ALL=C strings -a "$f" | LC_ALL=C grep -E -- "$pattern" || true)
    if [ -n "$hits" ]; then
        printf 'error: %s (%s matches), e.g.:\n%s\n' "$f" "$(wc -l <<<"$hits" | tr -d ' ')" \
            "$(sed -n 1,3p <<<"$hits")" >&2
        found=1
    fi
done < <(find "$@" -type f -print0)
if [ "$found" -ne 0 ]; then
    echo 'error: shipped files contain local paths' >&2
    exit 1
fi
echo "checked: no local paths in $*"
