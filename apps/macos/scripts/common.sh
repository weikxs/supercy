#!/usr/bin/env bash
# Shared checks for the macOS scripts: one heavy job at a time, and only on a quiet machine.
set -euo pipefail

# shellcheck disable=SC2034 # used by the scripts that source this file
readonly SANDBOX=/tmp/supercy
# Do not start while the one-minute load is at or above this.
readonly MAX_LOAD=6
# shellcheck disable=SC2034 # used by the scripts that source this file
readonly BUILD_JOBS=4

repo_root=$(git rev-parse --show-toplevel)
app_dir="$repo_root/apps/macos"

require_quiet_host() {
  if command -v docker >/dev/null 2>&1 && [[ -n "$(docker ps -q 2>/dev/null)" ]]; then
    echo "macos: a Docker container is running; wait for it to finish" >&2
    exit 1
  fi
  local load
  load=$(sysctl -n vm.loadavg | awk '{print $2}')
  if awk -v l="$load" -v m="$MAX_LOAD" 'BEGIN { exit !(l >= m) }'; then
    echo "macos: host load $load is at or above $MAX_LOAD; try again later" >&2
    exit 1
  fi
}

# The Swift code shared with the tests must never reach the user's clipboard, keychain,
# login items or keyboard; only the app target may.
check_kit_isolation() {
  local forbidden='NSPasteboard\.general|SecItem|SMAppService|CGEvent|RegisterEventHotKey|AXIsProcessTrusted'
  if grep -rnE "$forbidden" "$app_dir/Sources/SupercyKit" "$app_dir/Tests"; then
    echo "macos: system APIs found in shared or test code (see above); keep them in Sources/SupercyApp" >&2
    exit 1
  fi
}
