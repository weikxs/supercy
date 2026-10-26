# Common tasks. Every Rust check runs in Docker (scripts/docker-test.sh); nothing is built on the host.

.PHONY: all check privacy test fmt clippy licenses bench cover format lock hooks

all: check

check: privacy test bench cover

privacy:
	scripts/check-privacy.sh

# fmt + clippy -D warnings + cargo test, in a Linux container.
test:
	scripts/docker-test.sh

fmt:
	scripts/docker-test.sh fmt

clippy:
	scripts/docker-test.sh clippy

# Every dependency must offer a permissive license.
licenses:
	scripts/docker-test.sh licenses

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

# Use the repository's hooks: privacy check before commit, Conventional Commits on the message.
hooks:
	git config core.hooksPath scripts/hooks
