<!--
  Release notes template, in the layout of the other weikxs projects (see
  https://github.com/weikxs/otpkeep/releases/tag/v0.1.0); keep every release in
  the same order and voice.

  One file per release: .github/release-notes/vX.Y.Z.md (a pre-release such as
  vX.Y.Z-rc.1 has its own file too). The release workflow publishes it, word
  for word, as the GitHub release page, and stops when the file is missing or
  still contains TODO. Draft one from the changelog:

      scripts/release-notes.sh vX.Y.Z > .github/release-notes/vX.Y.Z.md

  Voice: product communication for people deciding whether to install or
  upgrade, not a developer changelog (CHANGELOG.md is that). Speak to "you",
  say what the reader notices, not how it was built. No commit hashes, author
  emails, real names, companies or local paths; examples use demo data
  (docs/development/standards.md, section 4) and never real clipboard
  content. State no performance figure that was not measured for this release.

  Sections marked "(only when …)" are left out when they have nothing to say.
  Every other section is always there. Delete these comments when done.
-->

# Supercy vX.Y.Z — One-line theme

Released on `YYYY-MM-DD`.

> Supercy is a lightweight menu-bar clipboard manager for macOS: what you copy is a shortcut away, searchable
> (Chinese by pinyin too), encrypted on your Mac and never sent anywhere.

## Summary

<!-- Two or three sentences: what this release is, what changes for you, which systems it runs on. -->

## Who this release is for

<!--
  One or two sentences naming the person it helps most, specifically, then an
  "If you have ever:" list of 3–5 real moments, then one sentence on how this
  release changes them.
-->

## Before you upgrade

<!-- (only when you must act) Changes to the stored history, settings or shortcuts, steps to run after upgrading, in order. -->

## What you can do now

<!--
  3–6 short paragraphs, one per capability. Each starts with a bold verb
  phrase and ends with the moment you notice it.
-->

## What's new

<!--
  Grouped by theme, one "###" per theme (for example: History and search,
  Pins, Privacy, Screenshots and annotation, Settings). Each item: a bold lead,
  what it does, and its boundary (what it does not do).
-->

### Theme

- **Lead.** What it does, and what it does not do.

## Bug fixes

<!-- (only when there are fixes) One line each, in the reader's words: what went wrong, and that it no longer does. -->

## Privacy and security

<!--
  Always present, even when nothing changed. Keep the promises, then add this
  release's security fixes with their severity (and advisory ID).
-->

- **Nothing leaves your Mac.** No account, no cloud, no telemetry: your clipboard history stays on this computer.
- **Concealed copies are never stored.** Content that password managers mark as concealed or transient is dropped
  before it reaches the history.
- **Your history is encrypted on disk,** with a key kept in your login keychain.

## Install

- Script (fully automatic): `curl -fsSL https://raw.githubusercontent.com/weikxs/supercy/main/install.sh | sh`
- Homebrew: `brew install --cask weikxs/tap/supercy`, or `brew upgrade --cask supercy`
- The disk image below; check it against `checksums.txt`

| File | Platform |
|---|---|
| `supercy_X.Y.Z_darwin_universal.dmg` | macOS 14 (Sonoma) or newer, Apple silicon and Intel |

## Compatibility

- macOS 14 (Sonoma) or newer on Apple silicon and Intel.
- Supercy is available for macOS only; there is no Windows or Linux version.
- English and Simplified Chinese, following the system language.

## What's coming next

<!-- (only when there is a plan to share) One or two themes of the next release, without dates. -->

## Notes

- The macOS app is not signed or notarized by Apple. Homebrew and the install script handle this for you. If you
  open the disk image by hand, open Supercy once, then choose **Open Anyway** in System Settings → Privacy &
  Security, or run `xattr -dr com.apple.quarantine /Applications/Supercy.app`. See
  [SECURITY.md](https://github.com/weikxs/supercy/blob/vX.Y.Z/SECURITY.md#release-packages-and-macos).
- **Full changelog:** [vA.B.C...vX.Y.Z](https://github.com/weikxs/supercy/compare/vA.B.C...vX.Y.Z) ·
  [CHANGELOG.md](https://github.com/weikxs/supercy/blob/vX.Y.Z/CHANGELOG.md)
- Documentation: [README](https://github.com/weikxs/supercy#readme) ·
  [简体中文](https://github.com/weikxs/supercy/blob/main/README.zh-CN.md)
- Feedback and problems: [issues](https://github.com/weikxs/supercy/issues).
