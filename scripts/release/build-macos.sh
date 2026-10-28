#!/bin/sh
# Builds the universal (Apple silicon and Intel) Supercy.app in the Release configuration and packages
# it as the release's disk image, <out>/supercy_<version>_darwin_universal.dmg. The Rust core is built
# for both architectures and joined with lipo; the Xcode project is built with both architectures
# and that library. The app keeps the ad-hoc signature of the Xcode project: it is not signed with an
# Apple Developer ID or notarized.
#
# Release and Full workflows only: it adds Rust targets to the toolchain, which a developer's machine
# should not get from a script (docs/development/standards.md, section 9). Developers build the app
# with make macos-app.
#
# usage: scripts/release/build-macos.sh <version> <out>
#   e.g. build-macos.sh 0.1.0 dist
set -eu

version=${1:?usage: build-macos.sh <version> <out>}
out=${2:?usage: build-macos.sh <version> <out>}

say() { echo "build-macos: $*"; }
fail() {
	echo "build-macos: $*" >&2
	exit 1
}

[ "$(uname -s)" = Darwin ] || fail "the macOS app is built on macOS"
[ "${CI:-}" = true ] || fail "runs in CI only (the release and full workflows); use make macos-app on a Mac"

cd "$(dirname "$0")/../.."
root=$(pwd)
readonly ARCHS="arm64 x86_64"
readonly RUST_TARGETS="aarch64-apple-darwin x86_64-apple-darwin"
readonly UNIVERSAL_LIB="$root/target/universal-apple-darwin/release/libcy_ffi.a"

app_version=$(scripts/release/app-version.sh)
[ "$app_version" = "${version%%-*}" ] || fail "the app's version is $app_version, not ${version%%-*}"

mkdir -p "$out"
out=$(cd "$out" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

# 1. The Rust core for both architectures, joined into one static library.
libs=""
for target in $RUST_TARGETS; do
	say "building the Rust core for $target"
	cargo build -p cy-ffi --release --locked --target "$target"
	libs="$libs $root/target/$target/release/libcy_ffi.a"
done
mkdir -p "$(dirname "$UNIVERSAL_LIB")"
# shellcheck disable=SC2086 # one path per target, none with spaces
lipo -create $libs -output "$UNIVERSAL_LIB"
say "core: $(lipo -archs "$UNIVERSAL_LIB")"

# 2. The app, for both architectures, linked against the universal library.
say "building Supercy.app"
(cd apps/macos && xcodebuild build -project Supercy.xcodeproj -scheme Supercy -configuration Release \
	-destination 'generic/platform=macOS' -derivedDataPath "$work/DerivedData" \
	ARCHS="$ARCHS" ONLY_ACTIVE_ARCH=NO OTHER_LDFLAGS="$UNIVERSAL_LIB" -quiet)
app="$work/DerivedData/Build/Products/Release/Supercy.app"
[ -x "$app/Contents/MacOS/Supercy" ] || fail "$app was not built"
archs=$(lipo -archs "$app/Contents/MacOS/Supercy")
for want in $ARCHS; do
	case " $archs " in *" $want "*) ;; *) fail "Supercy.app is not universal: $archs (no $want)" ;; esac
done
built=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$app/Contents/Info.plist")
[ "$built" = "$app_version" ] || fail "Supercy.app says version $built, expected $app_version"
codesign --verify --deep --strict "$app" || fail "the app's ad-hoc signature does not verify"

# 3. The disk image: the app and a link to Applications.
mkdir -p "$work/dmg"
ditto "$app" "$work/dmg/Supercy.app"
ln -s /Applications "$work/dmg/Applications"
name="supercy_${version}_darwin_universal.dmg"
hdiutil create -quiet -volname Supercy -srcfolder "$work/dmg" -ov -format UDZO "$out/$name"
say "$out/$name ($archs, version $built)"
