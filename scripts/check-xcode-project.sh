#!/usr/bin/env bash
# Fails when apps/macos/Supercy.xcodeproj is not what apps/macos/tools/generate_project.py makes
# from the current source tree (a Swift file was added or removed without regenerating it).
# The generator runs on a copy, so the working tree is never written. Needs only python3, so it
# runs on Linux too.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
readonly APP="$root/apps/macos"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/apps/macos"
for part in tools Sources Tests Resources Supercy.xcodeproj; do
  cp -R "$APP/$part" "$work/apps/macos/"
done
python3 "$work/apps/macos/tools/generate_project.py" >/dev/null

if ! diff -r "$work/apps/macos/Supercy.xcodeproj" "$APP/Supercy.xcodeproj"; then
  echo "xcode-project: apps/macos/Supercy.xcodeproj is out of date; run make macos-project and commit it" >&2
  exit 1
fi
echo "xcode-project: apps/macos/Supercy.xcodeproj is up to date"
