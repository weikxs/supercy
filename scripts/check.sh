#!/usr/bin/env bash
# Runs the Rust checks on this Linux machine, with the toolchain of rust-toolchain.toml. The same
# script runs inside the container of scripts/docker-test.sh (locally) and directly on the GitHub
# runners (CI, Full, Release), so both give the same answers. On a developer's machine use
# scripts/docker-test.sh or the make targets instead: this script installs the cargo tools it needs.
#
# Usage: scripts/check.sh [step...]
#   all (default)   fmt, clippy, deny, audit, test, bindings
#   fmt | clippy | deny | audit | test | bindings   one check
#   bench           performance budgets (release build, ignored perf_ tests)
#   cover           line coverage, fails below COVER_MIN (default 80)
#   fuzz            every fuzz target for FUZZ_SECONDS each (default 30; needs a nightly toolchain)
#   tools           install the cargo tools the checks use, without running them
set -euo pipefail

# The cargo tools, at versions known to build with the pinned toolchain.
readonly CARGO_DENY_VERSION=0.20.2
readonly CARGO_AUDIT_VERSION=0.22.2
readonly CARGO_LLVM_COV_VERSION=0.9.1
readonly CARGO_FUZZ_VERSION=0.13.2
# Unmaintained (not a vulnerability), build time only, via uniffi -> askama; see deny.toml.
readonly AUDIT_IGNORE=RUSTSEC-2026-0374
readonly COVER_MIN="${COVER_MIN:-80}"
readonly FUZZ_SECONDS="${FUZZ_SECONDS:-30}"

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
# Tests write only under /tmp/supercy-test (docs/development/standards.md, section 4).
export TMPDIR=/tmp/supercy-test
mkdir -p "$TMPDIR"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
# The stable toolchain of the workspace; fuzz/ has its own (nightly) toolchain file.
stable=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)

install_tool() {
  command -v "$1" >/dev/null || cargo install --locked --version "$2" "$1"
}
# The advisory database is fetched with git, which slim container images lack.
need_git() {
  command -v git >/dev/null || { apt-get update -qq && apt-get install -y -qq git >/dev/null; }
}

# Swift bindings: build the library, then generate from its metadata (UniFFI library mode).
generate_bindings() {
  local out=$1 library
  case "$(uname -s)" in
    Darwin) library="$target_dir/debug/libcy_ffi.dylib" ;;
    *) library="$target_dir/debug/libcy_ffi.so" ;;
  esac
  cargo build -p cy-ffi --locked --quiet
  rm -rf "$out"
  cargo run -p cy-bindgen --locked --quiet -- generate --library "$library" \
    --language swift --out-dir "$out" --no-format >/dev/null
}

run_fmt() { cargo fmt --all --check && (cd fuzz && cargo "+$stable" fmt --all --check); }
run_clippy() { cargo clippy --workspace --all-targets --locked -- -D warnings; }
run_test() { cargo test --workspace --locked; }
run_bench() { cargo test --workspace --release --locked -- --ignored --nocapture perf_; }
run_deny() {
  need_git
  install_tool cargo-deny "$CARGO_DENY_VERSION"
  cargo deny --locked check
}
run_audit() {
  need_git
  install_tool cargo-audit "$CARGO_AUDIT_VERSION"
  cargo audit --ignore "$AUDIT_IGNORE"
}
run_bindings() {
  local out="$TMPDIR/bindings"
  generate_bindings "$out"
  if ! diff -r "$out" bindings/swift; then
    echo "bindings: bindings/swift is out of date; run make bindings" >&2
    exit 1
  fi
  echo "bindings: bindings/swift is up to date"
}
run_cover() {
  rustup component add llvm-tools-preview >/dev/null
  install_tool cargo-llvm-cov "$CARGO_LLVM_COV_VERSION"
  cargo llvm-cov --workspace --locked --fail-under-lines "$COVER_MIN" --summary-only
}
run_fuzz() {
  install_tool cargo-fuzz "$CARGO_FUZZ_VERSION"
  local target log
  cd fuzz
  for target in $(cargo fuzz list); do
    echo "fuzz: $target for ${FUZZ_SECONDS}s"
    log="$TMPDIR/fuzz-$target.log"
    if ! cargo fuzz run "$target" -- -max_total_time="$FUZZ_SECONDS" -rss_limit_mb=2048 >"$log" 2>&1; then
      tail -n 40 "$log" >&2
      echo "fuzz: $target failed" >&2
      exit 1
    fi
    grep -E '^Done [0-9]+ runs' "$log" || tail -n 3 "$log"
  done
  cd "$root"
}
run_tools() {
  install_tool cargo-deny "$CARGO_DENY_VERSION"
  install_tool cargo-audit "$CARGO_AUDIT_VERSION"
  install_tool cargo-llvm-cov "$CARGO_LLVM_COV_VERSION"
}

run_step() {
  case "$1" in
    all) run_fmt; run_clippy; run_deny; run_audit; run_test; run_bindings ;;
    fmt) run_fmt ;;
    clippy) run_clippy ;;
    deny) run_deny ;;
    audit) run_audit ;;
    test) run_test ;;
    bindings) run_bindings ;;
    bench) run_bench ;;
    cover) run_cover ;;
    fuzz) run_fuzz ;;
    tools) run_tools ;;
    *)
      echo "usage: $0 [all|fmt|clippy|deny|audit|test|bindings|bench|cover|fuzz|tools]..." >&2
      exit 64
      ;;
  esac
}

if [[ $# -eq 0 ]]; then set -- all; fi
for step in "$@"; do
  case "$step" in
    all | fmt | clippy | deny | audit | test | bindings | bench | cover | fuzz | tools) ;;
    *) run_step "$step" ;;
  esac
done
for step in "$@"; do
  run_step "$step"
done
