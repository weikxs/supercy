#!/bin/sh
# Prints the version of the macOS app: VERSION in apps/macos/tools/generate_project.py, which the
# generated Xcode project carries as MARKETING_VERSION. Fails when the project file disagrees (it
# was not regenerated after the version changed). The Cargo workspace version is internal (the
# crates are not published) and is not the app's version.
#
# usage: scripts/release/app-version.sh
set -eu

cd "$(dirname "$0")/../.."
readonly GENERATOR=apps/macos/tools/generate_project.py
readonly PROJECT=apps/macos/Supercy.xcodeproj/project.pbxproj

fail() {
	echo "app-version: $*" >&2
	exit 1
}

version=$(sed -n 's/^VERSION = "\([^"]*\)"$/\1/p' "$GENERATOR")
[ -n "$version" ] || fail "no VERSION = \"X.Y.Z\" line in $GENERATOR"
printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || fail "$GENERATOR has version '$version', not X.Y.Z"

found=$(sed -n 's/^[[:space:]]*MARKETING_VERSION = \(.*\);$/\1/p' "$PROJECT" | sort -u)
[ -n "$found" ] || fail "$PROJECT has no MARKETING_VERSION"
[ "$found" = "$version" ] || fail "$PROJECT has MARKETING_VERSION $(printf '%s' "$found" | tr '\n' ' ')but $GENERATOR says $version; run make macos-project"

printf '%s\n' "$version"
