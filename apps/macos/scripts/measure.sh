#!/usr/bin/env bash
# Measures the built app's idle memory and CPU without touching the user's data: the app runs
# with --isolated (in-memory pasteboard and keystore, no global shortcut, data under
# /tmp/supercy), for MEASURE_SECONDS, then it is quit and everything is removed.
set -euo pipefail
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
. "$(dirname "$0")/common.sh"

readonly MEASURE_SECONDS=${MEASURE_SECONDS:-15}

require_quiet_host
check_kit_isolation
"$app_dir/scripts/build-core.sh"

rm -rf "$SANDBOX"
mkdir -p "$SANDBOX/home" "$SANDBOX/tmp" "$SANDBOX/DerivedData" "$SANDBOX/data"
trap 'rm -rf "$SANDBOX"' EXIT

cd "$app_dir"
env HOME="$SANDBOX/home" CFFIXED_USER_HOME="$SANDBOX/home" TMPDIR="$SANDBOX/tmp/" \
  xcodebuild build -project Supercy.xcodeproj -scheme Supercy -configuration Release \
    -destination 'platform=macOS,arch=arm64' -derivedDataPath "$SANDBOX/DerivedData" -jobs "$BUILD_JOBS" -quiet

app="$SANDBOX/DerivedData/Build/Products/Release/Supercy.app"
du -sh "$app" | awk '{print "app size: " $1}'
env HOME="$SANDBOX/home" CFFIXED_USER_HOME="$SANDBOX/home" TMPDIR="$SANDBOX/tmp/" SUPERCY_DATA_DIR="$SANDBOX/data" \
  "$app/Contents/MacOS/Supercy" --isolated &
pid=$!
sleep "$MEASURE_SECONDS"
vmmap --summary "$pid" 2>/dev/null | grep -E 'Physical footprint' || true
ps -o rss=,%cpu=,time= -p "$pid" | awk '{printf "resident: %.1f MB, cpu now: %s%%, cpu time: %s\n", $1/1024, $2, $3}'
kill "$pid"
wait "$pid" 2>/dev/null || true
