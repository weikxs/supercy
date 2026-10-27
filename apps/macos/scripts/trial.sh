#!/usr/bin/env bash
# Starts Supercy in trial mode for a manual check, or ends it and removes everything.
#
#   apps/macos/scripts/trial.sh          build if needed, then start the trial
#   apps/macos/scripts/trial.sh clean    quit the trial and delete /tmp/supercy entirely
#
# The trial reads the real clipboard, registers the history shortcut and can paste (after you
# allow Accessibility), exactly like the app. Unlike the app, its history, settings, caches,
# preferences and saved state all stay under /tmp/supercy (HOME and CFFIXED_USER_HOME point
# there), the data key is kept in memory only (never in the login keychain), and no login item
# is registered. Each start begins with an empty history.
set -euo pipefail
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
. "$(dirname "$0")/common.sh"

readonly PIDFILE="$SANDBOX/trial.pid"
readonly APP="$SANDBOX/DerivedData/Build/Products/Release/Supercy.app"
readonly QUIT_WAIT_SECONDS=10

trial_pid() {
  [[ -f "$PIDFILE" ]] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null && cat "$PIDFILE"
}

build() {
  require_quiet_host
  check_kit_isolation
  "$app_dir/scripts/build-core.sh" >/dev/null
  (cd "$app_dir" && env HOME="$SANDBOX/home" CFFIXED_USER_HOME="$SANDBOX/home" TMPDIR="$SANDBOX/tmp/" \
    xcodebuild build -project Supercy.xcodeproj -scheme Supercy -configuration Release \
      -destination 'platform=macOS,arch=arm64' -derivedDataPath "$SANDBOX/DerivedData" -jobs "$BUILD_JOBS" -quiet)
}

start() {
  if pid=$(trial_pid); then
    echo "trial: already running (pid $pid); end it with: $0 clean"
    return
  fi
  mkdir -p "$SANDBOX/home" "$SANDBOX/tmp" "$SANDBOX/DerivedData"
  [[ -x "$APP/Contents/MacOS/Supercy" ]] || build
  rm -rf "$SANDBOX/data"
  mkdir -p "$SANDBOX/data"
  env HOME="$SANDBOX/home" CFFIXED_USER_HOME="$SANDBOX/home" TMPDIR="$SANDBOX/tmp/" SUPERCY_DATA_DIR="$SANDBOX/data" \
    nohup "$APP/Contents/MacOS/Supercy" --trial >"$SANDBOX/trial.log" 2>&1 &
  echo $! >"$PIDFILE"
  echo "trial: started (pid $!). Supercy's icon is in the menu bar; ⇧⌘V opens the history."
  echo "trial: it records what you copy until you run: $0 clean"
}

clean() {
  if pid=$(trial_pid); then
    kill "$pid"
    for _ in $(seq 1 $((QUIT_WAIT_SECONDS * 2))); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.5
    done
    kill -0 "$pid" 2>/dev/null && kill -9 "$pid"
  fi
  rm -rf "$SANDBOX"
  echo "trial: ended; /tmp/supercy removed."
  echo "trial: if you allowed Accessibility, you can remove Supercy from System Settings > Privacy & Security > Accessibility."
}

case "${1:-start}" in
  start) start ;;
  clean) clean ;;
  *)
    echo "usage: $0 [start|clean]" >&2
    exit 64
    ;;
esac
