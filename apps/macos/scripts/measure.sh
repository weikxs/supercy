#!/usr/bin/env bash
# Measures the built app without touching the user's data: it runs with --isolated (in-memory
# pasteboard and keystore, no global shortcut, data under /tmp/supercy) and SUPERCY_MEASURE=1,
# which fills 500 demo items, opens and closes the history and the settings off screen, then
# idles 60 s. Memory (physical footprint) is sampled at each phase; CPU over the idle minute.
set -euo pipefail
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
. "$(dirname "$0")/common.sh"

require_quiet_host
require_no_trial
check_kit_isolation
"$app_dir/scripts/build-core.sh" >/dev/null

rm -rf "$SANDBOX"
mkdir -p "$SANDBOX/home" "$SANDBOX/tmp" "$SANDBOX/DerivedData" "$SANDBOX/data"
trap 'rm -rf "$SANDBOX"' EXIT

cd "$app_dir"
env HOME="$SANDBOX/home" CFFIXED_USER_HOME="$SANDBOX/home" TMPDIR="$SANDBOX/tmp/" \
  xcodebuild build -project Supercy.xcodeproj -scheme Supercy -configuration Release \
    -destination 'platform=macOS,arch=arm64' -derivedDataPath "$SANDBOX/DerivedData" -jobs "$BUILD_JOBS" -quiet

app="$SANDBOX/DerivedData/Build/Products/Release/Supercy.app"
du -sh "$app" | awk '{print "app size: " $1}'
log="$SANDBOX/measure.log"
env HOME="$SANDBOX/home" CFFIXED_USER_HOME="$SANDBOX/home" TMPDIR="$SANDBOX/tmp/" \
  SUPERCY_DATA_DIR="$SANDBOX/data" SUPERCY_MEASURE=1 SUPERCY_MEASURE_BASELINE="${SUPERCY_MEASURE_BASELINE:-0}" \
  "$app/Contents/MacOS/Supercy" --isolated >"$log" 2>/dev/null &
pid=$!

# CPU time in seconds from ps's [[dd-]hh:]mm:ss.xx.
cpu_seconds() {
  ps -o time= -p "$pid" | awk -F: '{ s = 0; for (i = 1; i <= NF; i++) s = s * 60 + $i; print s }'
}
footprint() {
  vmmap --summary "$pid" 2>/dev/null | awk '/^Physical footprint:/ { print $3; exit }'
}

seen=0
printf '%-18s %s\n' phase footprint
while kill -0 "$pid" 2>/dev/null; do
  lines=$(grep -c '^phase ' "$log" || true)
  while [[ "$seen" -lt "$lines" ]]; do
    seen=$((seen + 1))
    name=$(grep '^phase ' "$log" | sed -n "${seen}p" | cut -d' ' -f2)
    case "$name" in
      idle-60s-start) cpu_start=$(cpu_seconds) ;;
      idle-60s-end) cpu_end=$(cpu_seconds) ;;
    esac
    printf '%-18s %s\n' "$name" "$(footprint)"
    if [[ -n "${MEASURE_DETAIL_DIR:-}" ]]; then
      vmmap --summary "$pid" >"$MEASURE_DETAIL_DIR/$name.txt" 2>/dev/null || true
    fi
  done
  sleep 0.5
done
if [[ -n "${cpu_start:-}" && -n "${cpu_end:-}" ]]; then
  awk -v a="$cpu_start" -v b="$cpu_end" 'BEGIN { printf "idle cpu over 60 s: %.3f s (%.3f%%)\n", b - a, (b - a) / 60 * 100 }'
fi
