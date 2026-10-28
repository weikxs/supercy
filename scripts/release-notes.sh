#!/bin/sh
# Drafts the release notes of one version from its section of CHANGELOG.md,
# in the layout of .github/release-notes/TEMPLATE.md:
#
#   scripts/release-notes.sh v0.2.0 > .github/release-notes/v0.2.0.md
#
# The draft fills in what the changelog knows (date, summary, upgrading
# steps, new features, changes, fixes, security, downloads, links) and
# leaves TODO comments where a person writes: the theme, who the release is
# for and what you can do now. Sections with nothing to say are left out.
# The release workflow publishes the edited file, not this draft. A
# pre-release (v0.2.0-rc.1) is drafted from its version's section (0.2.0).
# Fails when the changelog has no section for the version.
#
# usage: scripts/release-notes.sh <tag> [CHANGELOG.md]
set -eu

tag=${1:?usage: release-notes.sh <tag> [changelog]}
changelog=${2:-CHANGELOG.md}
version=${tag#v}
repo=https://github.com/weikxs/supercy

# trim drops leading and trailing blank lines.
trim() { sed -e '/./,$!d' | sed -e ':a' -e '/^\n*$/{$d;N;ba' -e '}'; }

# A pre-release without a section of its own uses its version's section.
if ! grep -q "^## \[$version\]" "$changelog"; then version=${version%%-*}; fi

# section prints the lines under "### $1" in this version's section.
section() {
	awk -v version="$version" -v want="$1" '
		/^## \[/ { inside = index($0, "## [" version "]") == 1; next }
		!inside { next }
		/^### / { current = substr($0, 5); next }
		current == want { print }
	' "$changelog" | trim
}

# block prints a heading and the CHANGELOG category, unless it is empty.
block() {
	body=$(section "$2")
	if [ -n "$body" ]; then printf '%s\n%s\n\n' "$1" "$body"; fi
}

heading=$(grep -m1 "^## \[$version\]" "$changelog" || true)
if [ -z "$heading" ]; then
	echo "release-notes: CHANGELOG has no section for $version" >&2
	exit 1
fi
date=${heading##* - }
case "$date" in [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]) ;; *) date=TODO ;; esac
summary=$(awk -v version="$version" '
	/^## \[/ { inside = index($0, "## [" version "]") == 1; next }
	inside && /^### / { exit }
	inside { print }
' "$changelog" | trim)
# The version released before this one: the next section below it.
previous=$(awk -v version="$version" '
	/^## \[/ { if (found) { sub(/^## \[/, ""); sub(/\].*/, ""); print; exit } if (index($0, "## [" version "]") == 1) found = 1 }
' "$changelog")
files=${tag#v}
upgrading=$(section Upgrading)
removed=$(section Removed)
deprecated=$(section Deprecated)

cat <<EOF
# Supercy $tag — <!-- TODO: one-line theme -->

Released on \`$date\`.

> Supercy is a lightweight menu-bar clipboard manager for macOS: what you copy is a shortcut away, searchable
> (Chinese by pinyin too), encrypted on your Mac and never sent anywhere.

## Summary

${summary:-<!-- TODO: two or three sentences: what this release is about and what changes for you. -->}

## Who this release is for

<!-- TODO: the person this release helps most, specifically. -->

EOF
if [ -n "$upgrading$removed$deprecated" ]; then
	printf '## Before you upgrade\n\n'
	if [ -n "$upgrading" ]; then printf '%s\n\n' "$upgrading"; fi
	block "### Removed" Removed
	block "### Deprecated" Deprecated
fi
cat <<EOF
## What you can do now

<!-- TODO: 3–6 short paragraphs, one per capability, from the reader's side. -->

EOF
if [ -n "$(section Added)$(section Changed)" ]; then
	printf "## What's new\n\n"
	block "### New" Added
	block "### Changed" Changed
fi
block "## Bug fixes" Fixed
cat <<EOF
## Privacy and security

EOF
security=$(section Security)
if [ -n "$security" ]; then printf '%s\n' "$security"; fi
cat <<EOF
- **Nothing leaves your Mac.** No account, no cloud, no telemetry: your clipboard history stays on this computer.
- **Concealed copies are never stored.** Content that password managers mark as concealed or transient is dropped
  before it reaches the history.
- **Your history is encrypted on disk,** with a key kept in your login keychain.

## Install

- Script (fully automatic): \`curl -fsSL https://raw.githubusercontent.com/weikxs/supercy/main/install.sh | sh\`
- Homebrew: \`brew install --cask weikxs/tap/supercy\`, or \`brew upgrade --cask supercy\`
- The disk image below; check it against \`checksums.txt\`

| File | Platform |
|---|---|
| \`supercy_${files}_darwin_universal.dmg\` | macOS 14 (Sonoma) or newer, Apple silicon and Intel |

## Compatibility

- macOS 14 (Sonoma) or newer on Apple silicon and Intel.
- Supercy is available for macOS only; there is no Windows or Linux version.
- English and Simplified Chinese, following the system language.

## Notes

- The macOS app is not signed or notarized by Apple. Homebrew and the install script handle this for you. If you
  open the disk image by hand, open Supercy once, then choose **Open Anyway** in System Settings → Privacy &
  Security, or run \`xattr -dr com.apple.quarantine /Applications/Supercy.app\`. See
  [SECURITY.md]($repo/blob/$tag/SECURITY.md#release-packages-and-macos).
EOF
if [ -n "$previous" ]; then
	printf -- '- **Full changelog:** [v%s...%s](%s/compare/v%s...%s) · [CHANGELOG.md](%s/blob/%s/CHANGELOG.md)\n' \
		"$previous" "$tag" "$repo" "$previous" "$tag" "$repo" "$tag"
else
	printf -- '- **Full changelog:** [CHANGELOG.md](%s/blob/%s/CHANGELOG.md)\n' "$repo" "$tag"
fi
cat <<EOF
- Documentation: [README]($repo#readme) · [简体中文]($repo/blob/main/README.zh-CN.md)
- Feedback and problems: [issues]($repo/issues).
EOF
