# Common tasks. Every Rust check runs in Docker (scripts/docker-test.sh); nothing is built on the host.
# `make ci` is what CI's `check` job runs on every pull request; `make check` adds the benchmarks and
# fuzzing. Run `make check` before every pull request.

# Where the Rust checks run: in a container (default), or directly on a CI runner, which installs
# the pinned toolchain itself (RUN=scripts/check.sh). Both take the same step names.
RUN ?= scripts/docker-test.sh

.PHONY: all check ci privacy xcode-project test fmt clippy deny audit bench cover fuzz format lock bindings golden \
        hooks macos-project macos-test macos-measure macos-app macos-trial macos-trial-clean

all: check

# Everything: what CI runs, plus the performance budgets and a short fuzzing run.
check: ci bench fuzz

# What CI's `check` job runs on every pull request.
ci: privacy xcode-project test cover

privacy:
	scripts/check-privacy.sh

# The generated Xcode project matches the Swift sources (needs python3, writes nothing).
xcode-project:
	scripts/check-xcode-project.sh

# fmt, clippy -D warnings, cargo deny, cargo audit, cargo test and the bindings check (Linux).
test:
	$(RUN)

fmt:
	$(RUN) fmt

clippy:
	$(RUN) clippy

# Licenses, advisories, sources (cargo deny) and known vulnerabilities (cargo audit).
deny:
	$(RUN) deny

audit:
	$(RUN) audit

# Each fuzz target for FUZZ_SECONDS (default 30) in a nightly container.
fuzz:
	$(RUN) fuzz

# Search performance and memory budgets, release build.
bench:
	$(RUN) bench

# Line coverage; fails below 80%.
cover:
	$(RUN) cover

# Write changes: format the sources, or update Cargo.lock after changing dependencies.
format:
	scripts/docker-test.sh format

lock:
	scripts/docker-test.sh lock

# Regenerate the annotation golden images after an intended rendering change; review them first.
golden:
	scripts/docker-test.sh golden-write

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

# Manual check with the real clipboard, everything kept under /tmp/supercy; then remove it all.
macos-trial:
	apps/macos/scripts/trial.sh

macos-trial-clean:
	apps/macos/scripts/trial.sh clean

# Use the repository's hooks: privacy check before commit, Conventional Commits on the message.
hooks:
	git config core.hooksPath scripts/hooks
