# Contributing to Supercy

Thanks for helping! This page is the short version; the full rules are in
[docs/development/standards.md](docs/development/standards.md).

> Supercy has no license yet: all rights are reserved (see [LICENSE](LICENSE)). You may read the source and
> propose changes, but please open an issue to discuss before starting any larger work.

## Set up

Requirements:

| For | Needs |
|---|---|
| The Rust core (`crates/`) | Docker, Make and Bash: every Rust check runs in a Linux container, nothing is built or installed on your machine |
| The macOS app (`apps/macos`) | A Mac with macOS 14 or newer, Xcode, Python 3, and the Rust toolchain of `rust-toolchain.toml` |

```sh
git clone https://github.com/weikxs/supercy.git
cd supercy
make hooks         # install the pre-commit and commit-msg hooks
make ci            # what CI runs: privacy, Xcode project, fmt, clippy, deny, audit, tests, bindings, coverage
make check         # all of that, plus the performance budgets and a short fuzzing run
make macos-test    # on a Mac: build the app and run its unit tests, isolated under /tmp/supercy
```

The [README](README.md#development) lists every target. Running the app you build records your real clipboard;
use `make macos-trial` to try it with everything kept under `/tmp/supercy`.

## Workflow

1. Open or pick an issue, and say you are working on it.
2. Create a branch from `main`: `feat/<topic>`, `fix/<topic>`, `docs/<topic>`, …
3. Write a failing test first, then the code that makes it pass. Decisions belong in the Rust core, where they are
   tested on Linux; the macOS app only talks to the system and draws the interface.
4. Keep the change focused. Update `CHANGELOG.md` under **Unreleased** for anything users notice.
5. Run `make check` (and `make macos-test` when the app changed), then open a pull request using the template.

`main` is protected: every change arrives through a pull request, squash-merged once CI's `check` job is green.
Nobody bypasses this, maintainers included.

## Commit messages

[Conventional Commits 1.0](https://www.conventionalcommits.org/en/v1.0.0/), enforced by the `commit-msg` hook:

```text
feat(search): match pinyin initials in mixed mode
fix(macos): keep the selection when the history panel reloads
```

Commits are authored by you only; do not add tool or bot attribution trailers.

## Test data

Supercy sees everything its user copies, so test data matters more than usual. Use demo data only: text such as
`Hello from Supercy`, hosts such as `example.com`, accounts such as `demo@example.com`, apps such as
`com.example.editor`, home paths written as `~/…`, and "secrets" generated inside the test.
Tests never read the real clipboard, keychain or data folder. **Never put real clipboard contents, accounts,
tokens or personal paths in the repository, an issue or a pull request.** `make privacy` blocks the common cases.

## Security

Report vulnerabilities privately; see [SECURITY.md](SECURITY.md).
