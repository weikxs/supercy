# Security Policy

Supercy is a clipboard manager: it sees everything you copy, including passwords, tokens and private messages, and
keeps a history of it. A weakness in Supercy can expose exactly that, so we treat security reports as the highest
priority and handle them privately.

## Supported versions

No version has been released yet. Until 1.0, only the latest commit on `main` and the latest release receive
security fixes.

## Reporting a vulnerability

**Do not open a public issue, and never paste real clipboard contents anywhere.** Report privately through
[GitHub Security Advisories](https://github.com/weikxs/supercy/security/advisories/new).

Please include:

- the macOS version and CPU, and the Supercy version or commit;
- steps to reproduce, with demo content (such as `Hello from Supercy` or a generated `demo-password-123`) and with
  personal paths, app names and accounts replaced by placeholders;
- the impact you expect (for example, concealed content from a password manager reaching the history, history
  readable without its key, or clipboard content showing up in a log).

## What to expect

| Step | Target |
|---|---|
| Acknowledge the report | within 3 days |
| Initial assessment | within 7 days |
| Fix for confirmed critical issues | as soon as possible, normally within 30 days |

We credit reporters in the release notes unless they prefer to stay anonymous.

## What Supercy promises

- Content that password managers mark as concealed or transient is never stored.
- The history is encrypted on disk; its key is kept in your login keychain.
- Nothing you copy leaves your Mac: there is no account, no cloud and no telemetry.

A way around any of these is a vulnerability; please report it as above.

## Release packages and macOS

Supercy is released for macOS only. Every release publishes `checksums.txt` with the SHA-256 of its disk image; the
install script refuses a disk image that does not match it. Packages are built by GitHub Actions from this
repository's source, never on a developer's machine.

The macOS app is not signed with an Apple Developer ID or notarized by Apple. `checksums.txt` lets you confirm a
download is the released file. The Homebrew cask and the install script check the checksum, then remove the
quarantine attribute from the installed `Supercy.app` (`xattr -dr com.apple.quarantine`); otherwise macOS would
refuse to open it the first time.

If you install the disk image by hand:

1. Check it: `shasum -a 256 supercy_<version>_darwin_universal.dmg` must match its line in `checksums.txt`.
2. Copy `Supercy.app` into Applications and open it. When macOS says it cannot verify the app, click **Done**.
3. In System Settings → Privacy & Security, under Security, click **Open Anyway** next to "Supercy was blocked",
   then open Supercy again and choose **Open**. On macOS 14, Control-click the app and choose **Open** also
   works.

Instead of steps 2–3 you can run `xattr -dr com.apple.quarantine /Applications/Supercy.app`. macOS remembers the
choice until you replace the app by hand.
