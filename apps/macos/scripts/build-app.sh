#!/usr/bin/env bash
# Builds Supercy.app (Release, ad-hoc signed) into /tmp/supercy-build for a manual check.
# Building does not run the app; running it records the real clipboard (see README).
set -euo pipefail
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
. "$(dirname "$0")/common.sh"

readonly OUT=/tmp/supercy-build
require_quiet_host
check_kit_isolation
"$app_dir/scripts/build-core.sh"
mkdir -p "$OUT"
cd "$app_dir"
xcodebuild build -project Supercy.xcodeproj -scheme Supercy -configuration Release \
  -destination 'platform=macOS,arch=arm64' -derivedDataPath "$OUT/DerivedData" -jobs "$BUILD_JOBS" -quiet
echo "built: $OUT/DerivedData/Build/Products/Release/Supercy.app"
