# Common tasks. Every Rust check runs in Docker (scripts/docker-test.sh); nothing is built on the host.

.PHONY: all check privacy test fmt clippy deny audit bench cover fuzz format lock bindings hooks \
        macos-project macos-test macos-measure macos-app

all: check

check: privacy test bench cover fuzz

privacy:
	scripts/check-privacy.sh

# fmt + clippy -D warnings + cargo test, in a Linux container.
test:
	scripts/docker-test.sh

fmt:
	scripts/docker-test.sh fmt

clippy:
	scripts/docker-test.sh clippy

# Licenses, advisories, sources (cargo deny) and known vulnerabilities (cargo audit).
deny:
	scripts/docker-test.sh deny

audit:
	scripts/docker-test.sh audit

# Each fuzz target for FUZZ_SECONDS (default 30) in a nightly container.
fuzz:
	scripts/docker-test.sh fuzz

# Search performance and memory budgets, release build.
bench:
	scripts/docker-test.sh bench

# Line coverage; fails below 80%.
cover:
	scripts/docker-test.sh cover

# Write changes: format the sources, or update Cargo.lock after changing dependencies.
format:
	scripts/docker-test.sh format

lock:
	scripts/docker-test.sh lock

# Regenerate the committed Swift bindings after changing the FFI interface.
bindings:
	scripts/docker-test.sh bindings-write

# macOS app (decision D-1): built and tested on this Mac, isolated under /tmp/supercy.
macos-project:
	python3 apps/macos/tools/generate_project.py

macos-test:
	apps/macos/scripts/test.sh

# Idle memory and CPU of the app in --isolated mode (never reads the real clipboard).
macos-measure:
	apps/macos/scripts/measure.sh

# Builds Supercy.app into /tmp/supercy-build for a manual check (does not run it).
macos-app:
	apps/macos/scripts/build-app.sh

# Use the repository's hooks: privacy check before commit, Conventional Commits on the message.
hooks:
	git config core.hooksPath scripts/hooks
