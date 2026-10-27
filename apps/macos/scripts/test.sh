#!/usr/bin/env bash
# Builds the macOS app and runs its unit tests on this Mac (decision D-1), isolated under
# /tmp/supercy: HOME, CFFIXED_USER_HOME, TMPDIR and Xcode's derived data all live there, the
# tests use an in-memory pasteboard and keystore, and everything is removed afterwards.
# Rust core tests run in Docker (scripts/docker-test.sh), not here.
set -euo pipefail
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
. "$(dirname "$0")/common.sh"

require_quiet_host
check_kit_isolation
"$app_dir/scripts/build-core.sh"

rm -rf "$SANDBOX"
mkdir -p "$SANDBOX/home" "$SANDBOX/tmp" "$SANDBOX/DerivedData"
trap 'rm -rf "$SANDBOX"' EXIT

cd "$app_dir"
env HOME="$SANDBOX/home" CFFIXED_USER_HOME="$SANDBOX/home" TMPDIR="$SANDBOX/tmp/" \
  TEST_RUNNER_HOME="$SANDBOX/home" TEST_RUNNER_CFFIXED_USER_HOME="$SANDBOX/home" TEST_RUNNER_TMPDIR="$SANDBOX/tmp/" \
  xcodebuild test \
    -project Supercy.xcodeproj -scheme Supercy \
    -destination 'platform=macOS,arch=arm64' \
    -derivedDataPath "$SANDBOX/DerivedData" \
    -jobs "$BUILD_JOBS" -parallel-testing-enabled NO \
    2>&1 | grep -E '^(Test Suite|Test Case .* (passed|failed)|.*error:|.*warning:|\*\* (TEST|BUILD) )|Executed' || true
test "${PIPESTATUS[0]}" -eq 0
