#!/bin/sh
# Checks a release tag before anything is built: the tag's version matches the macOS app's version
# (scripts/release/app-version.sh: apps/macos/tools/generate_project.py and the generated Xcode
# project), and the release notes .github/release-notes/<tag>.md exist and have no TODO left (draft
# them with scripts/release-notes.sh). Writes version, base and prerelease to $GITHUB_OUTPUT when set.
#
# usage: scripts/release/check-release.sh v0.1.0 | v0.1.0-rc.1
set -eu

tag=${1:?usage: check-release.sh <tag>}
fail() {
	echo "check-release: $*" >&2
	exit 1
}

cd "$(dirname "$0")/../.."

version=${tag#v}
case "$tag" in
v[0-9]*.[0-9]*.[0-9]*-rc.[0-9]*) prerelease=true ;;
v[0-9]*.[0-9]*.[0-9]*) prerelease=false ;;
*) fail "$tag is not a release tag (vX.Y.Z or vX.Y.Z-rc.N)" ;;
esac
base=${version%%-*}
printf '%s\n' "$base" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || fail "$tag is not a release tag (vX.Y.Z or vX.Y.Z-rc.N)"

app=$(scripts/release/app-version.sh) || fail "the app's version could not be read (see above)"
[ "$app" = "$base" ] || fail "the macOS app has version '$app' (apps/macos/tools/generate_project.py), the tag says $base"

notes=".github/release-notes/$tag.md"
[ -f "$notes" ] || fail "$notes is missing; draft it with scripts/release-notes.sh $tag > $notes"
if grep -n "TODO" "$notes" >&2; then fail "$notes still has TODO placeholders"; fi
head -n 1 "$notes" | grep -q "^# Supercy $tag — " || fail "$notes does not start with '# Supercy $tag — <theme>'"

echo "check-release: $tag OK (version $base, prerelease $prerelease)"
if [ -n "${GITHUB_OUTPUT:-}" ]; then
	{
		echo "version=$version"
		echo "base=$base"
		echo "prerelease=$prerelease"
	} >>"$GITHUB_OUTPUT"
fi
