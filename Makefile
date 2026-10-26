# Common tasks. Every Rust check runs in Docker (scripts/docker-test.sh); nothing is built on the host.

.PHONY: all check privacy test fmt clippy hooks

all: check

check: privacy test

privacy:
	scripts/check-privacy.sh

# fmt + clippy -D warnings + cargo test, in a Linux container.
test:
	scripts/docker-test.sh

fmt:
	scripts/docker-test.sh fmt

clippy:
	scripts/docker-test.sh clippy

# Use the repository's hooks: privacy check before commit, Conventional Commits on the message.
hooks:
	git config core.hooksPath scripts/hooks
