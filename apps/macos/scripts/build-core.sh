#!/usr/bin/env bash
# Builds the Rust core as a static library for Apple silicon (decision D-6), into the
# repository's target directory. Runs with the real HOME so cargo finds its toolchain.
set -euo pipefail
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
. "$(dirname "$0")/common.sh"

cd "$repo_root"
CARGO_BUILD_JOBS=$BUILD_JOBS cargo build -p cy-ffi --release --locked --target aarch64-apple-darwin
ls -l "$repo_root/target/aarch64-apple-darwin/release/libcy_ffi.a"
