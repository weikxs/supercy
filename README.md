# Supercy

**A lightweight menu-bar clipboard manager with screenshots, annotation and pins, and private sync between your own devices.**

[简体中文](README.zh-CN.md)

![Status](https://img.shields.io/badge/status-early%20development-lightgrey)
![License](https://img.shields.io/badge/license-not%20chosen%20yet-lightgrey)

> [!NOTE]
> Supercy is in **early development**. There is no release to download yet. The macOS clipboard app (0.1 scope)
> builds from source and is being checked by hand; screenshots, annotation and pins come next.

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
| 1.0 | Release: measured resource use, privacy audit |

## Platforms

macOS first. Windows and Linux follow; mobile comes last and can only send and receive by hand, because iOS and
Android do not let apps read the clipboard in the background.

## Install

No release has been published yet; until then, build Supercy from source as in [Development](#development).
Releases will be published on the [Releases page](https://github.com/weikxs/supercy/releases), built by GitHub
Actions from this repository, for macOS only: one disk image for Apple silicon and Intel Macs with macOS 14
(Sonoma) or newer. There is no Windows or Linux version.

### Install script, fully automatic

```sh
curl -fsSL https://raw.githubusercontent.com/weikxs/supercy/main/install.sh | sh
```

The script downloads the disk image, checks it against the release's `checksums.txt` and copies `Supercy.app` into
`/Applications` (or `~/Applications`), without the "downloaded from the internet" prompt. Nothing else is changed.
Run it again to upgrade. Options go after `sh -s --`, for example `sh -s -- --version v0.1.0`:

| Option | Meaning |
|---|---|
| `--version <tag>` | install this release instead of the latest |
| `--dir <folder>` | where `Supercy.app` goes |

### Homebrew

```sh
brew install --cask weikxs/tap/supercy
brew upgrade --cask supercy            # later, to upgrade
```

### Disk image

Download `supercy_<version>_darwin_universal.dmg` from the
[latest release](https://github.com/weikxs/supercy/releases/latest) and check it against `checksums.txt`
(`shasum -a 256 -c checksums.txt --ignore-missing`).

The macOS app is not signed or notarized by Apple. The script and Homebrew handle that for you. If you install the
disk image yourself, open Supercy once, then choose **Open Anyway** in System Settings → Privacy & Security and
open it again — or run `xattr -dr com.apple.quarantine /Applications/Supercy.app`. Details in
[SECURITY.md](SECURITY.md#release-packages-and-macos).

To paste straight into the app you were using, Supercy asks for Accessibility permission (System Settings →
Privacy & Security → Accessibility).

### Uninstall

Quit Supercy, then delete `Supercy.app` (or run `brew uninstall --cask supercy`). Your history stays in
`~/Library/Application Support/Supercy`, with its key in your login keychain (`io.github.weikxs.Supercy`); delete
both to remove it for good.

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
make ci        # what CI runs on every pull request: privacy, Xcode project, test and cover below
make check     # everything: make ci, bench and fuzz; run it before every pull request
make privacy   # no local paths, secrets or real identifiers
make test      # fmt, clippy -D warnings, cargo deny, cargo audit, tests and a bindings check in Docker
make bench     # search speed and memory budgets (release build)
make cover     # line coverage, at least 80%
make fuzz      # fuzz the parsers and the capture filter (nightly container)
```

The macOS app (`apps/macos`, Xcode project generated by `make macos-project`) is built and tested on the Mac,
isolated under `/tmp/supercy`:

```sh
make macos-test      # build the Rust library and the app, run the unit tests
make macos-measure   # idle memory and CPU, with an in-memory clipboard (yours is never read)
make macos-app       # build Supercy.app into /tmp/supercy-build
```

Running the built app records your real clipboard into `~/Library/Application Support/Supercy` (encrypted) and
keeps its key in your login keychain. To try it without that, use the trial: it reads your real clipboard and
registers ⇧⌘V like the app, but keeps everything under `/tmp/supercy`, holds the key in memory only and never
registers a login item:

```sh
make macos-trial         # build if needed and start the trial (each start begins empty)
make macos-trial-clean   # quit it and delete /tmp/supercy
```

Icons are drawn as SVG in `design/icons` and rendered in a container (`scripts/docker-test.sh icons`); see
[docs/design/icons.md](docs/design/icons.md).

### Continuous integration and releases

- **Pull requests:** CI's `check` job runs `make ci` on Linux, natively, with the same `scripts/check.sh` that
  runs in the container on your machine. A change to documentation alone runs only the privacy check.
- **`main` is protected:** changes arrive only through pull requests, squash-merged, with the `check` job green.
  Nobody bypasses this, maintainers included.
- **Full run** (Actions → Full → Run workflow, by hand before a release): everything CI runs, the performance
  budgets, fuzzing, the macOS app build with its unit tests, and the universal release build installed and
  started.
- **Release:** pushing a tag `vX.Y.Z` checks the version and the release notes
  (`.github/release-notes/vX.Y.Z.md`), runs every check, builds the universal disk image, installs and starts it
  on fresh Apple silicon and Intel Macs, and only then publishes it with `checksums.txt`.

Read [docs/development/standards.md](docs/development/standards.md) before contributing.

## Contributing

Contributions are welcome; please open an issue first. Read [CONTRIBUTING.md](CONTRIBUTING.md) and the
[code of conduct](CODE_OF_CONDUCT.md), run `make check`, and use
[Conventional Commits](https://www.conventionalcommits.org/) (`make hooks` checks them). Never put real clipboard
contents in an issue or a pull request. Notable changes are recorded in the [changelog](CHANGELOG.md).

## Security

Supercy sees everything you copy. Please report vulnerabilities privately as described in
[SECURITY.md](SECURITY.md), never in public issues.

## License

Not chosen yet. Until a license is added, all rights are reserved (see [LICENSE](LICENSE)).
