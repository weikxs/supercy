#!/bin/sh
# Installs a freshly built disk image with the real install.sh, from a local stand-in for the GitHub
# release, then starts the installed app and checks that it keeps running. Used by the release
# workflow on fresh macOS machines (Apple silicon and Intel) before anything is published, and by the
# full workflow.
#
# The app is started in its isolated mode (--isolated): an in-memory clipboard, its data and key in a
# folder under /tmp, no global shortcut and no login item, so the check never reads a clipboard or
# writes to a keychain. It still opens the encrypted history through the Rust core, which writes its
# files to that folder; the check fails when the folder stays empty.
#
# usage: scripts/release/verify-install.sh <tag> <folder with the release files>
set -eu

tag=${1:?usage: verify-install.sh <tag> <dist>}
dist=${2:?usage: verify-install.sh <tag> <dist>}
readonly RUN_SECONDS=15
readonly PORT=8765
# The oldest macOS the app supports (MACOSX_DEPLOYMENT_TARGET of the Xcode project).
readonly MACOS_MIN=14.0

[ "$(uname -s)" = Darwin ] || {
	echo "verify-install: the app is verified on macOS" >&2
	exit 1
}
cd "$(dirname "$0")/../.."

# The data folder must be under /tmp (LaunchOptions); mktemp's default folder is not.
work=$(mktemp -d /tmp/supercy-verify.XXXXXX)
server=""
app_pid=""
cleanup() {
	[ -z "$app_pid" ] || kill "$app_pid" 2>/dev/null || true
	[ -z "$server" ] || kill "$server" 2>/dev/null || true
	rm -rf "$work"
}
trap cleanup EXIT INT TERM
say() { echo "verify-install: $*"; }
fail() {
	echo "verify-install: FAILED: $*" >&2
	exit 1
}

# The release as install.sh sees it: <base>/download/<tag>/<file> and checksums.txt.
mkdir -p "$work/release/download/$tag"
cp "$dist"/supercy_*_darwin_universal.dmg "$work/release/download/$tag/"
(cd "$work/release/download/$tag" && shasum -a 256 supercy_* >checksums.txt)
python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$work/release" >/dev/null 2>&1 &
server=$!
for _ in 1 2 3 4 5 6 7 8 9 10; do
	curl -fs -o /dev/null "http://127.0.0.1:$PORT/download/$tag/checksums.txt" && break
	sleep 1
done
export SUPERCY_BASE_URL="http://127.0.0.1:$PORT"

apps="$work/Applications"
sh ./install.sh --version "$tag" --dir "$apps"
app="$apps/Supercy.app"
archs=$(lipo -archs "$app/Contents/MacOS/Supercy")
for want in x86_64 arm64; do
	case " $archs " in *" $want "*) ;; *) fail "not universal: $archs (no $want)" ;; esac
done
minos=$(/usr/libexec/PlistBuddy -c "Print :LSMinimumSystemVersion" "$app/Contents/Info.plist")
[ "$minos" = "$MACOS_MIN" ] || fail "LSMinimumSystemVersion is $minos, expected $MACOS_MIN"
[ -z "$(xattr -p com.apple.quarantine "$app" 2>/dev/null || true)" ] || fail "the quarantine flag is still set"
codesign --verify --deep --strict "$app" || fail "the app's signature does not verify"
say "Supercy.app is universal ($archs), needs macOS $minos, no quarantine flag (this machine: $(uname -m))"

# Start it, isolated, with every folder it might write to under $work.
mkdir -p "$work/home" "$work/tmp" "$work/data"
env HOME="$work/home" CFFIXED_USER_HOME="$work/home" TMPDIR="$work/tmp/" SUPERCY_DATA_DIR="$work/data" \
	"$app/Contents/MacOS/Supercy" --isolated >"$work/app.log" 2>&1 &
app_pid=$!
sleep "$RUN_SECONDS"
if ! kill -0 "$app_pid" 2>/dev/null; then
	cat "$work/app.log" >&2
	fail "the app exited within ${RUN_SECONDS}s"
fi
kill "$app_pid" 2>/dev/null || true
wait "$app_pid" 2>/dev/null || true
app_pid=""
[ -n "$(find "$work/data" -mindepth 1 -print -quit)" ] || fail "the app kept running but wrote no history to its data folder"
say "the app started, opened its history in its data folder and kept running for ${RUN_SECONDS}s"
say "OK for $tag"
