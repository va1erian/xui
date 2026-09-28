#!/usr/bin/env bash
# Renders every example headlessly, in light and dark, into target/snapshots/
# (or the directory given as the first argument). No window opens and no
# sandbox is needed: the examples run on the offscreen software backend.
#
#   scripts/snapshots.sh [out-dir]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
out="${1:-target/snapshots}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
rm -f "$out"/*.png

cargo build -q -p xui --features canvas --examples
bin="${CARGO_TARGET_DIR:-target}/debug/examples"

examples=(widgets listview gridview top_bar layout)
for file in crates/xui/examples/controls/*.rs; do
    name="$(basename "$file" .rs)"
    [ "$name" = support ] || examples+=("control_$name")
done

failed=()
for name in "${examples[@]}"; do
    exe="$bin/$name"
    [ -x "$exe" ] || exe="$exe.exe"
    # A demo that hangs must fail the run, not stall it.
    if XUI_SNAPSHOT="$out" ${TIMEOUT:-timeout 120} "$exe"; then
        echo "ok   $name"
    else
        echo "FAIL $name"
        failed+=("$name")
    fi
done

echo "${#examples[@]} examples, ${#failed[@]} failed; images in $out"
[ "${#failed[@]}" -eq 0 ]
