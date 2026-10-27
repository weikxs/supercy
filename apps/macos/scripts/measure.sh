#!/usr/bin/env bash
# Measures the built app without touching the user's data: it runs with --isolated (in-memory
# pasteboard, key and data under /tmp/supercy, no global shortcut). A first launch fills 500
# demo items and quits; a relaunch idles 60 s, opens the history, searches, pastes into the
# in-memory pasteboard, closes it, opens and closes the settings (off screen) and idles 10 min.
# Memory (physical footprint) is sampled at each phase; CPU over both idle windows.
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
launch() {
  env HOME="$SANDBOX/home" CFFIXED_USER_HOME="$SANDBOX/home" TMPDIR="$SANDBOX/tmp/" \
    SUPERCY_DATA_DIR="$SANDBOX/data" SUPERCY_MEASURE="$1" SUPERCY_MEASURE_BASELINE="${SUPERCY_MEASURE_BASELINE:-0}" \
    SUPERCY_MEASURE_IDLE="${SUPERCY_MEASURE_IDLE:-600}" \
    SUPERCY_MEASURE_NO_THUMBNAILS="${SUPERCY_MEASURE_NO_THUMBNAILS:-0}" \
    ${MEASURE_MALLOC_LOG:+MallocStackLogging=1} \
    "$app/Contents/MacOS/Supercy" --isolated >"$2" 2>/dev/null &
}

# 1. Fill the history with 500 items and quit.
launch seed "$SANDBOX/seed.log"
wait $!
grep -q '^phase seeded' "$SANDBOX/seed.log" || { echo "measure: seeding failed" >&2; exit 1; }

# 2. Relaunch with those items and follow the scenario.
log="$SANDBOX/measure.log"
launch run "$log"
pid=$!

# CPU time in seconds from ps's [[dd-]hh:]mm:ss.xx.
cpu_seconds() {
  ps -o time= -p "$pid" | awk -F: '{ s = 0; for (i = 1; i <= NF; i++) s = s * 60 + $i; print s }'
}
footprint() {
  vmmap --summary "$pid" 2>/dev/null | awk '/^Physical footprint:/ { print $3; exit }'
}
now() { python3 -c 'import time; print(time.time())'; }
# CPU used between two samples, over the wall time between them.
cpu_window() {
  awk -v a="$2" -v b="$3" -v t0="$4" -v t1="$5" -v n="$1" \
    'BEGIN { printf "%s cpu: %.3f s over %.0f s (%.3f%%)\n", n, b - a, t1 - t0, (b - a) / (t1 - t0) * 100 }'
}

seen=0
printf '%-18s %s\n' phase footprint
while kill -0 "$pid" 2>/dev/null; do
  lines=$(grep -c '^phase ' "$log" || true)
  while [[ "$seen" -lt "$lines" ]]; do
    seen=$((seen + 1))
    name=$(grep '^phase ' "$log" | sed -n "${seen}p" | cut -d' ' -f2)
    case "$name" in
      idle-60s-start) cpu_a=$(cpu_seconds) t_a=$(now) ;;
      idle-60s-end) cpu_b=$(cpu_seconds) t_b=$(now) ;;
      idle-10min-start) cpu_c=$(cpu_seconds) t_c=$(now) ;;
      idle-10min-end) cpu_d=$(cpu_seconds) t_d=$(now) ;;
    esac
    printf '%-18s %s\n' "$name" "$(footprint)"
    if [[ -n "${MEASURE_DETAIL_DIR:-}" ]]; then
      vmmap --summary "$pid" >"$MEASURE_DETAIL_DIR/$name.txt" 2>/dev/null || true
      [[ "$name" == idle-10min-start ]] && sample "$pid" 5 -file "$MEASURE_DETAIL_DIR/idle-sample.txt" >/dev/null 2>&1
      case "$name" in
        idle-60s-end | history-open | searched | history-closed) heap "$pid" >"$MEASURE_DETAIL_DIR/$name.heap" 2>/dev/null || true ;;
      esac
      case "$name" in
        history-open)
          [[ -n "${MEASURE_MALLOC_LOG:-}" ]] && malloc_history "$pid" -allBySize 2>/dev/null | head -c 3000000 \
            >"$MEASURE_DETAIL_DIR/$name.mallocs" || true ;;
      esac
      pgrep -lf 'Supercy.app/Contents/MacOS/Supercy' >"$MEASURE_DETAIL_DIR/$name.processes" || true
    fi
  done
  sleep 0.5
done
[[ -n "${cpu_b:-}" ]] && cpu_window "idle 60 s" "$cpu_a" "$cpu_b" "$t_a" "$t_b"
[[ -n "${cpu_d:-}" ]] && cpu_window "idle 10 min" "$cpu_c" "$cpu_d" "$t_c" "$t_d"
exit 0
