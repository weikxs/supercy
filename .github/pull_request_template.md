## What and why

<!-- What this changes and why. Link the issue: "Fixes #123". A feature needs a maintainer's go-ahead on its issue. -->

## How it was tested

<!-- Commands you ran (make check, make macos-test), the macOS version and CPU you checked on, and what you checked by hand. -->

## Use of AI

<!-- If an AI tool wrote a substantial part of this change, say which part and what it did. Otherwise write "none". -->

## Checklist

- [ ] It starts from an issue; a new feature had a maintainer's go-ahead
- [ ] One purpose: no unrelated fixes, refactors or features mixed in
- [ ] Commit subjects follow [Conventional Commits](https://www.conventionalcommits.org/), no tool attribution trailers
- [ ] Tests written first; `make check` passes (and `make macos-test` when the macOS app changed)
- [ ] I understand and have verified every line, including any AI-assisted part
- [ ] `CHANGELOG.md`, README (English and Chinese) and in-app texts (both languages) updated if behavior changed
- [ ] Only demo data is used: no real clipboard contents, personal paths, accounts, tokens or private links
