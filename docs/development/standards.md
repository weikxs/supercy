# Development Standards

These rules apply to every change, by anyone, including the maintainers. **MUST** rules are enforced by tooling
or review; **SHOULD** rules need a reason in the pull request when broken. Where a tool enforces a rule, the
rule says which; "review" means a maintainer checks it.

Supercy is a clipboard manager: it sees everything its user copies, including passwords, tokens and private
messages. Privacy (§4) and security (§5) rules therefore come before every other consideration.

## 1. Standards we follow

| Area | Standard |
|---|---|
| Versioning | [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) |
| Commit messages | [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) |
| Changelog | [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/) |
| Rust style | [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/), `rustfmt`, `clippy` (pedantic) with `-D warnings` |
| Swift style | Swift API Design Guidelines; warnings treated as errors (from milestone M3) |
| Shell style | POSIX `sh` or `bash`, clean under [ShellCheck](https://www.shellcheck.net) |
| Clipboard markers | [nspasteboard.org](http://nspasteboard.org) concealed / transient / auto-generated types on macOS; the platform equivalents elsewhere |
| User interface | [WCAG 2.2](https://www.w3.org/TR/WCAG22/) level AA; the platform's own human interface guidelines |
| Community | [Contributor Covenant 2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/) |
| License | Not chosen yet; all rights reserved until a license file is added (see `LICENSE`) |

## 2. Contribution process

- **Issue first.** Every change starts from an issue. New features, changes to the storage format, the sync
  protocol, the FFI boundary, shortcuts or what is captured need a maintainer's go-ahead **before** code is
  written.
- **Bug reports** say the Supercy version, the system and CPU, what you did, what you expected and what
  happened — with demo clipboard content only (§4), never real copied data.
- **One pull request, one purpose.** Do not mix a fix, a refactor and a feature; split them.
- **Review:** every change reaches `main` through a pull request with green required checks once the
  repository is published. Nobody bypasses the checks.

## 3. AI-assisted contributions (MUST)

AI tools may help write code, tests and docs. The person who commits the change is its author and is
responsible for every line.

- **Understand it.** You must be able to explain the change, its edge cases and side effects in your own words.
- **Verify it.** Run the tests and check the behavior yourself. AI output that "looks right" is not evidence.
- **No autonomous agents** open issues or pull requests; a person does, from their own account.
- **Commits carry no tool or bot attribution** (no `Co-Authored-By` for tools, no "generated with" lines): the
  human author is the author. Enforced by the `commit-msg` hook.
- Never paste real clipboard history, secrets or private paths into an AI tool's prompt for this project.

## 4. Privacy and test data (MUST)

Enforced by `make privacy` (pre-commit hook) and review.

- **Never commit real secrets, account names, emails, organization names, hostnames or local paths** — not in
  code, tests, fixtures, docs, screenshots, release notes, commit messages or issues.
- Use only demo data:

  | Kind | Demo values |
  |---|---|
  | Copied text | `Hello from Supercy`, `示例文本`, `lorem ipsum`, generated strings |
  | URLs | `https://example.com/…`, `https://demo.example/…` ([RFC 2606](https://www.rfc-editor.org/rfc/rfc2606)) |
  | Accounts | `alex@example.com`, `demo@example.com`, `demo-user` |
  | "Secrets" in filter tests | Strings generated at random inside the test, or obviously fake ones such as `demo-password-123` |
  | Source apps | `com.example.editor`, `com.example.passwords`, `Demo Editor` |
  | Device names | `Demo-Mac`, `Demo-PC`, `Demo-Phone` |
  | Hosts | `example.com`, `*.example`; LAN addresses from `192.168.0.0/24` |
  | Images | Images generated inside the test (solid colors, gradients) |

- **Tests never touch real user data.** All test files live under `/tmp/supercy-test/` inside the test
  container. Tests never read the real system clipboard, never read or write the real data directory and never
  use the real system keystore: they use an in-memory clipboard, an in-memory keystore and a temporary data
  directory.
- Each contributor may add their own real identifiers, one regular expression per line, to the git-ignored
  `.privacy-denylist`.

## 5. Security (MUST)

- **Concealed and transient content is never stored**, not even for a moment on disk: content carrying
  `org.nspasteboard.ConcealedType` or `org.nspasteboard.TransientType` on macOS, and the platform equivalents
  on Windows and Linux, is dropped before it reaches storage. Covered by tests.
- **Nothing leaves the device except to paired devices**, end-to-end encrypted. There is no telemetry, no
  analytics, no crash upload, no account and no default server.
- **History is encrypted at rest.** The data key lives in the operating system's keystore; it never touches the
  disk in plain form.
- Keys, session secrets, pairing codes and clipboard content are wrapped in types that zero their memory on
  drop (`zeroize`) where practical, and never implement a `Debug` or `Display` that shows the value.
- Never write clipboard content, previews, keys or codes to logs, error messages or crash output.
- Use the cryptography crates already chosen for the project. Do not design new primitives or protocols;
  protocol changes need a design review.
- Compare secrets, tokens and confirmation values in constant time.
- Every parser of external input (network messages, stored records, imported files, clipboard payloads) has a
  size limit, never panics, and gets a fuzz target before the first release.
- Network: listen only while sync is enabled or a pairing window is open, only on private addresses of
  physical interfaces.
- `unsafe` is forbidden in the workspace; the FFI crate is the single exception, and only for generated
  binding code.

## 6. Supply chain

| Rule | Level | Enforced by |
|---|---|---|
| `Cargo.lock` is committed; checks run with `--locked` | MUST | `scripts/docker-test.sh` |
| A new dependency needs a reason in the pull request: maintenance, size, permissive license (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode); any other license needs a recorded decision | MUST | `make licenses` (part of `make test`), review |
| Known vulnerabilities in dependencies are checked | MUST | `cargo audit` (planned; not yet in the container) |
| Release packages are built in CI only, never on a developer's machine, and published with SHA-256 checksums | MUST | release workflow (later) |

## 7. Code

- **Layout:** Rust crates live in `crates/`, one responsibility each; platform shells live in `apps/`
  (`apps/macos`, later `apps/windows`, `apps/linux`). Dependencies point one way only: shells → FFI → services
  → storage, search, filter → model.
- **Thin shells:** a platform shell only talks to the system (clipboard, hotkeys, windows, paste, keychain) and
  draws the interface. Every decision (what to keep, dedupe, search, expire, sync) lives in the Rust core, so it
  is tested on Linux.
- **Size:** functions under 50 lines; files under 400 lines (800 hard limit); nesting at most 4 levels.
- **Pure core:** model, filter, search and merge logic take values and return values. File system, keystore,
  clock, network and clipboard sit behind traits, so the core is testable without I/O.
- **Errors:** no `unwrap`, `expect` or `panic!` outside tests (enforced by clippy lints). Library crates define
  error enums with `thiserror`; user-facing messages say what went wrong and what to do next, and are translated.
- **Immutability:** prefer returning new values over mutating inputs; no global mutable state.
- **No magic values:** name constants for intervals, limits, sizes and ports; user-tunable defaults are defined
  in one place.
- **Comments:** doc comments on every public item. Inline comments explain *why*, not *what*.
- **Logging:** `tracing`, off by default beyond warnings; clipboard content is never logged.

## 8. Resource budget (MUST)

Supercy must stay as light as the lightest menu-bar clipboard managers. Measured on the reference machine with
the default history size; a change that breaks a budget needs a recorded decision.

| Measure | Budget |
|---|---|
| Resident memory (idle, default history) | ≤ 30 MB |
| CPU while idle | ≈ 0 — no work besides the clipboard change-count check |
| Background activity | No polling except the clipboard change-count check; no timers while sync is off |

## 9. Tests (MUST)

- **Test first:** write a failing test, make it pass, then refactor.
- **Coverage:** at least 80% of lines across the Rust workspace (`make cover` fails below it); core crates aim at 90%.
- **Budgets:** search speed and index memory have release-mode benchmarks with hard limits (`make bench`).
- **Style:** descriptive test names; Arrange–Act–Assert; property tests (`proptest`) for parsers and merge.
- **All tests run in Docker** (`make test`, `scripts/docker-test.sh`): `cargo fmt --check`, `clippy -D warnings`
  and `cargo test` in a Linux container with a CPU and memory limit. The Rust core must be fully testable on
  Linux.
- **Never install anything on the developer's machine.** At most one container at a time, none started while
  the machine is at 70% CPU or more (the script checks both).
- **Isolation:** no network outside loopback and in-memory test networks; no real home directory, clipboard or
  keystore. Tests wait for conditions, never for a fixed time that a slow machine may miss.
- **Platform shells** cannot be tested in a Linux container; how each shell is verified is written in the
  development plan and agreed before the shell is built.

## 10. Git workflow

- **Branching:** GitHub Flow once published. `main` is always releasable.
- **Commits:** Conventional Commits, subject at most 72 characters, imperative, enforced by the `commit-msg` hook
  (`make hooks`). One logical change per commit; the body says *why*.
- Commits are authored by the contributor's own account, with no tool or bot attribution trailers.
- **Never** force-push to `main` or rewrite published history.

## 11. Versioning and changelog

- SemVer. Before 1.0, minor versions may contain breaking changes; they are called out in the changelog.
- The storage format and the sync protocol have their own version numbers; a change to either needs migration
  tests and a changelog entry.
- Every user-visible change adds a line under **Unreleased** in `CHANGELOG.md` (from the first release).

## 12. Documentation

- Public docs are in English; `README.md` has a Simplified Chinese translation in `README.zh-CN.md`, updated in
  the same change.
- Statements in the docs must be true of the code: a supported platform is built and tested, a privacy promise
  is enforced by a test or a rule above.
- Product requirements, designs and planning are internal and are not published (`docs/prd/` is git-ignored).

## 13. Definition of done

- [ ] tests were written first and pass in Docker (`make test`);
- [ ] no real identities, local paths or secrets (`make privacy`);
- [ ] no clipboard content can reach logs, errors or anything outside the device except paired devices;
- [ ] the resource budget (§8) still holds for changes that touch capture, storage, search or sync;
- [ ] errors are handled and messages are actionable, in both languages;
- [ ] README (both languages) is updated where relevant;
- [ ] the author understands and has verified every line, including AI-assisted parts.
