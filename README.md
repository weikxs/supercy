# Supercy

**A lightweight menu-bar clipboard manager with screenshots, annotation and pins, and private sync between your own devices.**

[简体中文](README.zh-CN.md)

![Status](https://img.shields.io/badge/status-early%20development-lightgrey)
![License](https://img.shields.io/badge/license-not%20chosen%20yet-lightgrey)

> [!NOTE]
> Supercy is in **early development**. There is no app to download yet: this repository holds the Rust core
> (history model, filtering, search, encrypted storage, the engine and its Swift interface), the development
> standards and the test harness. The macOS app comes next.

## What it will be

- **Light:** a native menu-bar app (SwiftUI/AppKit on macOS first), no web view. Target: at most 30 MB of memory
  and no CPU use while idle.
- **History at your fingertips:** text, rich text, links, images and file references; exact, fuzzy and regular
  expression search (fuzzy search understands pinyin for Chinese); pins with their own keys; sorting by recent use,
  first copy or copy count; previews; a global shortcut; full keyboard control; paste straight back into the app
  you were using, with or without formatting; pause recording or skip the next copy.
- **More than a history:** favourites and snippets, edit an item before pasting, merge several items into one.
- **Screenshots, annotation and pins:** capture a region, a window or the whole screen with a magnifier and colour
  picker; draw rectangles, arrows, text, numbered steps, highlights, mosaic and blur; copy, save or pin the result
  as a floating, always-on-top image. Every image in the history can be annotated or pinned too, and text can be
  recognised on the device. Nothing stays in memory when you are not capturing.
- **Private by default:** content that password managers mark as concealed or transient is never stored; apps
  can be ignored; sensitive items expire; history is encrypted on disk with a key kept in the system keychain.
  No account, no cloud, no telemetry.
- **Sync between your own devices:** on the local network, end-to-end encrypted, after pairing with a QR code or
  a one-time code. No server by default.

## Roadmap

| Version | Scope |
|---|---|
| 0.1 | macOS clipboard history with the full core feature set |
| 0.2 | Screenshots, annotation and pins |
| 0.3 | Favourites, edit before paste, merge, text recognition, file references |
| 0.4 | Sync between your own devices on the local network |
| 1.0 | Release: measured resource use, privacy audit, signed builds |

## Platforms

macOS first. Windows and Linux follow; mobile comes last and can only send and receive by hand, because iOS and
Android do not let apps read the clipboard in the background.

## Development

The core is written in Rust (`crates/`); each platform has a thin native shell (`apps/`, from milestone M3).

| Crate | Purpose |
|---|---|
| `cy-model` | History items, representations, ids, hybrid logical clocks, content fingerprints |
| `cy-filter` | What is kept: concealed/transient markers, ignored apps, size limits, sensitive content |
| `cy-search` | In-memory fuzzy search with pinyin |
| `cy-store` | Encrypted SQLite history (XChaCha20-Poly1305, key in the system keystore) |
| `cy-platform` | Keystore and clock traits, with in-memory versions for tests |
| `cy-engine` | The core service: capture, deduplicate, store, search, pin, expire; no threads or timers |
| `cy-ffi` | The engine for Swift (UniFFI); generated bindings are committed in `bindings/swift` |

All checks run in a Linux container, so nothing is built or installed on your machine:

```sh
make hooks     # use the repository's git hooks
make privacy   # no local paths, secrets or real identifiers
make test      # fmt, clippy -D warnings, cargo deny, cargo audit, tests and a bindings check in Docker
make bench     # search speed and memory budgets (release build)
make cover     # line coverage, at least 80%
make fuzz      # fuzz the parsers and the capture filter (nightly container)
```

Read [docs/development/standards.md](docs/development/standards.md) before contributing.

## License

Not chosen yet. Until a license is added, all rights are reserved (see [LICENSE](LICENSE)).
