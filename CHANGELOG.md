# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing has been released yet. This section records what exists on `main` so far.

### Added

- A macOS menu-bar app (macOS 14 or newer) that keeps a history of what you copy: text, rich text, links, images
  and references to copied files. It watches the clipboard only while recording is not paused.
- A keyboard-first history panel, opened with a global shortcut (⇧⌘V by default): exact, fuzzy (with pinyin for
  Chinese), regular-expression and mixed search with highlighted matches, previews and image thumbnails; paste
  into the app you were using, with or without formatting, copy only, delete or clear.
- Sorting by last use, first copy or copy count, with pinned items at the top or the bottom.
- Pins with their own letter keys, kept in the order they were pinned; move one by dragging it or with ⌥⌘↑ and
  ⌥⌘↓. Pinned items can be exported to a JSON file and imported again; the file is not encrypted, and the
  settings say so.
- Ignore rules: clipboard types, regular expressions, ignored apps, an allow-list mode, and switches for text,
  images and files. Recording can be paused for a while or until resumed, and the next copy can be skipped.
- Content that password managers mark as concealed or transient, and auto-generated content, is never stored.
  Private keys, tokens, card numbers and password-like text are marked sensitive and expire.
- The history is stored encrypted on disk (SQLite, XChaCha20-Poly1305), with its key in the login keychain.
  Deleted, evicted and expired items are overwritten on disk at once.
- The history panel asks macOS to keep it out of screenshots, screen recordings and screen sharing.
- Settings for the shortcuts, storage, ignore rules, appearance (including the menu bar icon style) and
  permissions, in English and Simplified Chinese, following the system language.
- The core of the coming screenshot feature: annotation documents with ten shape kinds, undo and redo, rendering
  and export to PNG and JPEG. The macOS app does not use it yet.
- For development: every Rust check in a Linux container (`make check`), a trial mode that keeps all its data
  under `/tmp/supercy` (`make macos-trial`), and an isolated measurement mode (`make macos-measure`).
- Continuous integration for pull requests, an on-demand full run with the macOS app build and tests, and a
  release workflow that builds, installs and starts a universal macOS disk image before publishing it with
  checksums. An install script (`install.sh`) for macOS.

[Unreleased]: https://github.com/weikxs/supercy/commits/main
