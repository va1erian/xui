#!/usr/bin/env bash
# Renders every example headlessly, in light and dark, into target/snapshots/
# (or the directory given as the first argument). No window opens and no
# sandbox is needed: the examples run on the offscreen software backend.
#
# Each example also runs with XUI_LAYOUT_REPORT, and its layout report must
# have no warning: an example whose layout clips, overlaps or truncates fails
# the run.
#
#   scripts/snapshots.sh [out-dir]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
out="${1:-target/snapshots}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
rm -f "$out"/*.png "$out"/*.log

cargo build -q -p xui --features canvas,rhai --examples
bin="${CARGO_TARGET_DIR:-target}/debug/examples"

examples=(widgets listview gridview top_bar layout containers absolute form script cookbook menu_bar_flicker)
for file in crates/xui/examples/controls/*.rs; do
    name="$(basename "$file" .rs)"
    [ "$name" = support ] || examples+=("control_$name")
done

failed=()
for name in "${examples[@]}"; do
    exe="$bin/$name"
    [ -x "$exe" ] || exe="$exe.exe"
    log="$out/$name.log"
    # A demo that hangs must fail the run, not stall it.
    if XUI_SNAPSHOT="$out" XUI_LAYOUT_REPORT=1 ${TIMEOUT:-timeout 120} "$exe" >"$log" 2>&1; then
        warnings="$(sed -n 's/^XUI_LAYOUT_WARNINGS://p' "$log" | tail -n 1)"
        if [ "$warnings" = "0" ]; then
            rm -f "$log"
            echo "ok   $name"
        else
            echo "FAIL $name (layout warnings: ${warnings:-none reported})"
            grep '^ *! ' "$log" || true
            failed+=("$name")
        fi
    else
        echo "FAIL $name"
        tail -n 20 "$log" 2>/dev/null || true
        failed+=("$name")
    fi
done

echo "${#examples[@]} examples, ${#failed[@]} failed; images in $out"
[ "${#failed[@]}" -eq 0 ]
