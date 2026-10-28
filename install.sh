#!/bin/sh
# Supercy installer for macOS (Supercy is available for macOS only).
#
#   curl -fsSL https://raw.githubusercontent.com/weikxs/supercy/main/install.sh | sh
#
# It downloads the universal disk image from the GitHub release, checks it against the release's
# checksums.txt and copies Supercy.app into /Applications (or ~/Applications). Nothing else is
# changed. Your clipboard history, its key in the login keychain and your settings are never
# touched: they stay where Supercy keeps them whatever you install, upgrade or remove.
#
# Options (or the environment variables in brackets):
#   --version <tag>     install this release, such as v0.1.0 [SUPERCY_VERSION]; default: the latest
#   --dir <directory>   where Supercy.app goes [SUPERCY_INSTALL_DIR]
#   --help              show this help
#
# Run it again to upgrade.
set -eu

REPO="weikxs/supercy"
BASE_URL="${SUPERCY_BASE_URL:-https://github.com/$REPO/releases}"
# The oldest macOS the app runs on (MACOSX_DEPLOYMENT_TARGET of the Xcode project).
MACOS_MIN_MAJOR=14

say() { printf 'supercy-install: %s\n' "$*"; }
fail() {
	printf 'supercy-install: error: %s\n' "$*" >&2
	exit 1
}

usage() {
	cat <<'EOF'
Supercy installer for macOS.

  curl -fsSL https://raw.githubusercontent.com/weikxs/supercy/main/install.sh | sh
  curl -fsSL https://raw.githubusercontent.com/weikxs/supercy/main/install.sh | sh -s -- [options]

Options (or environment variables):
  --version <tag>     install this release, such as v0.1.0 (SUPERCY_VERSION); default: the latest
  --dir <directory>   where Supercy.app goes (SUPERCY_INSTALL_DIR); default: /Applications,
                      or ~/Applications when /Applications is not writable
  --help              show this help

Run it again to upgrade. Your clipboard history and settings are never touched.
EOF
}

parse_args() {
	version="${SUPERCY_VERSION:-}"
	install_dir="${SUPERCY_INSTALL_DIR:-}"
	while [ $# -gt 0 ]; do
		case "$1" in
		--version) [ $# -ge 2 ] || fail "--version needs a tag, such as v0.1.0"; version="$2"; shift 2 ;;
		--version=*) version="${1#*=}"; shift ;;
		--dir) [ $# -ge 2 ] || fail "--dir needs a directory"; install_dir="$2"; shift 2 ;;
		--dir=*) install_dir="${1#*=}"; shift ;;
		-h | --help) usage; exit 0 ;;
		*) fail "unknown option: $1 (see --help)" ;;
		esac
	done
	case "$version" in
	"" | v[0-9]*.[0-9]*.[0-9]*) ;;
	[0-9]*.[0-9]*.[0-9]*) version="v$version" ;;
	*) fail "not a release tag: $version (expected something like v0.1.0)" ;;
	esac
}

check_system() {
	[ "$(uname -s)" = Darwin ] || fail "Supercy is available for macOS only; $(uname -s) is not supported"
	major=$(sw_vers -productVersion | cut -d. -f1)
	[ "$major" -ge "$MACOS_MIN_MAJOR" ] 2>/dev/null ||
		fail "Supercy needs macOS $MACOS_MIN_MAJOR (Sonoma) or newer; this Mac runs $(sw_vers -productVersion)"
	command -v curl >/dev/null 2>&1 || fail "curl is needed to download Supercy"
	command -v shasum >/dev/null 2>&1 || fail "shasum is needed to check the download"
	command -v hdiutil >/dev/null 2>&1 || fail "hdiutil is needed to open the disk image"
}

# Releases are only fetched over HTTPS; a plain http:// SUPERCY_BASE_URL (a test server) is the only exception.
download() {
	case "$BASE_URL" in
	https://*) curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$2" "$1" ;;
	*) curl -fsSL --retry 3 -o "$2" "$1" ;;
	esac
}

# latest_version prints the tag of the newest release, read from where releases/latest redirects
# (no API call, so no rate limit).
latest_version() {
	url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$BASE_URL/latest") ||
		fail "could not reach $BASE_URL/latest"
	tag="${url##*/}"
	case "$tag" in
	v[0-9]*.[0-9]*.[0-9]*) printf '%s\n' "$tag" ;;
	*) fail "could not find the latest release at $BASE_URL/latest" ;;
	esac
}

# fetch_and_verify downloads the disk image and checksums.txt into $tmp and stops unless the image's
# SHA-256 matches its line in checksums.txt.
fetch_and_verify() {
	file="supercy_${version#v}_darwin_universal.dmg"
	say "downloading $file ($version)"
	download "$BASE_URL/download/$version/$file" "$tmp/$file" ||
		fail "could not download $file; is $version a Supercy release?"
	download "$BASE_URL/download/$version/checksums.txt" "$tmp/checksums.txt" ||
		fail "could not download checksums.txt for $version"
	expected=$(awk -v f="$file" '$2 == f || $2 == "*" f { print $1 }' "$tmp/checksums.txt")
	[ -n "$expected" ] || fail "checksums.txt has no entry for $file"
	actual=$(cd "$tmp" && shasum -a 256 "$file" | awk '{ print $1 }')
	[ "$expected" = "$actual" ] || fail "checksum mismatch for $file (expected $expected, got $actual); nothing was installed"
	say "checksum OK"
}

# expand_dir makes a --dir value absolute.
expand_dir() {
	case "$1" in
	"~"/*) printf '%s\n' "$HOME/${1#"~/"}" ;;
	/*) printf '%s\n' "$1" ;;
	*) printf '%s\n' "$(pwd)/$1" ;;
	esac
}

install_app() {
	if pgrep -x Supercy >/dev/null 2>&1; then
		fail "Supercy is running; quit it (menu bar icon → Quit Supercy), then run the installer again"
	fi
	if [ -z "$install_dir" ]; then
		if [ -w /Applications ]; then install_dir=/Applications; else install_dir="$HOME/Applications"; fi
	fi
	install_dir=$(expand_dir "$install_dir")
	mkdir -p "$install_dir" || fail "cannot create $install_dir"
	[ -w "$install_dir" ] || fail "$install_dir is not writable; choose another with --dir"
	mount="$tmp/mount"
	mkdir -p "$mount"
	hdiutil attach -nobrowse -readonly -quiet -mountpoint "$mount" "$tmp/$file" || fail "could not open $file"
	mounted=1
	[ -d "$mount/Supercy.app" ] || fail "$file has no Supercy.app"
	# Copy next to the target, then swap, so an interrupted install never leaves half an app.
	rm -rf "$install_dir/.Supercy.app.new"
	ditto "$mount/Supercy.app" "$install_dir/.Supercy.app.new" || fail "cannot write to $install_dir"
	hdiutil detach -quiet "$mount" && mounted=0
	# Downloaded by this script, not a browser: no "downloaded from the internet" prompt.
	xattr -dr com.apple.quarantine "$install_dir/.Supercy.app.new" 2>/dev/null || true
	rm -rf "$install_dir/Supercy.app"
	mv "$install_dir/.Supercy.app.new" "$install_dir/Supercy.app"
	app="$install_dir/Supercy.app"
	[ -x "$app/Contents/MacOS/Supercy" ] || fail "Supercy.app was copied but its program is missing"
	say "installed Supercy ${version#v} to $app"
	say "done. Start it from Launchpad or with: open \"$app\""
}

cleanup() {
	if [ "${mounted:-0}" = 1 ]; then hdiutil detach -quiet -force "$mount" 2>/dev/null || true; fi
	rm -rf "$tmp"
}

main() {
	parse_args "$@"
	check_system
	[ -n "$version" ] || version=$(latest_version)
	tmp=$(mktemp -d 2>/dev/null || mktemp -d -t supercy)
	mounted=0
	trap cleanup EXIT INT TERM
	fetch_and_verify
	install_app
}

main "$@"
