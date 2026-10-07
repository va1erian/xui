#!/usr/bin/env bash
# Renders every fixture with every engine probe and measures the probes'
# sizes. Linux only (the NetSurf probe's C builds only there; on Windows run it
# in WSL). Writes PNGs, timings and sizes under $OUT (default
# target/engine-compare), for report.py.
#
#   scripts/engine-compare/run.sh            # release and small profiles
#   SKIP_BUILD=1 scripts/engine-compare/run.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="${OUT:-$root/target/engine-compare}"
# Building from a Windows checkout under /mnt is slow; keep the target native.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/engine-compare-target}"
engines=(litehtml netsurf blitz)
width="${WIDTH:-1000}"
height="${HEIGHT:-1400}"

mkdir -p "$out/shots"
if [ -z "${SKIP_BUILD:-}" ]; then
    (cd "$here/probes" && cargo build --release && cargo build --profile small)
fi

# Sizes: each probe, and its difference from the engine-less baseline.
{
    echo "profile,engine,bytes"
    for profile in release small; do
        for e in baseline "${engines[@]}"; do
            f="$CARGO_TARGET_DIR/$profile/probe-$e"
            echo "$profile,$e,$(stat -c %s "$f")"
        done
    done
} > "$out/sizes.csv"

# Renders: the committed fixtures, and the live pages if fetch_live.py saved them.
pages=()
for f in "$here"/fixtures/*.html "$here"/fixtures/theoldnet/index.html "$out"/live/*.html; do
    [ -f "$f" ] && pages+=("$f")
done
echo "fixture,engine,ready,ms" > "$out/timings.csv"
for page in "${pages[@]}"; do
    name="$(basename "$page" .html)"
    [ "$name" = index ] && name="$(basename "$(dirname "$page")")"
    for e in "${engines[@]}"; do
        line="$(timeout 90 "$CARGO_TARGET_DIR/release/probe-$e" "$page" "$out/shots/$name.$e.png" "$width" "$height" 2>/dev/null | grep '^PROBE:' || echo "PROBE:$e:ready=crashed:ms=0")"
        IFS=: read -r _ engine ready ms <<< "$line"
        echo "$name,$engine,${ready#ready=},${ms#ms=}" >> "$out/timings.csv"
        echo "$name $line"
    done
done
cat "$out/sizes.csv"
