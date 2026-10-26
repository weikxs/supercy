#!/usr/bin/env bash
# Runs the Rust checks (fmt, clippy -D warnings, tests) inside a Linux container.
# All Supercy tests run in Docker: nothing is built or installed on the host.
#
# The repository is mounted read-only and copied into the container, so the host
# tree is never written; build output and the crate cache live in named volumes.
#
# Usage: scripts/docker-test.sh            # fmt + clippy + licenses + test
#        scripts/docker-test.sh test       # one step: fmt | clippy | licenses | test
#        scripts/docker-test.sh bench      # performance budgets (release build, ignored perf_ tests)
#        scripts/docker-test.sh cover      # line coverage, fails below COVER_MIN
#        scripts/docker-test.sh lock       # write Cargo.lock after a dependency change (mounts the repo read-write)
#        scripts/docker-test.sh format     # run cargo fmt on the sources (mounts the repo read-write)
set -euo pipefail

readonly IMAGE="${SUPERCY_RUST_IMAGE:-rust:1.95-slim}"
readonly CPUS=4
readonly MEMORY=5g
readonly TARGET_VOLUME="supercy-target"
readonly REGISTRY_VOLUME="supercy-cargo-registry"
readonly TOOLS_VOLUME="supercy-tools"
readonly COVER_MIN=80
# Do not start a container when the host is busy: at most 70% of its CPUs.
readonly MAX_LOAD_PERCENT=70

repo_root=$(git rev-parse --show-toplevel)
step=${1:-all}

case "$step" in
  all | fmt | clippy | licenses | test | bench | cover | lock | format) ;;
  *)
    echo "usage: $0 [all|fmt|clippy|licenses|test|bench|cover|lock|format]" >&2
    exit 64
    ;;
esac

if ! command -v docker >/dev/null 2>&1; then
  echo "docker-test: Docker is not installed or not on PATH" >&2
  exit 1
fi

# One container at a time.
if [[ -n "$(docker ps -q)" ]]; then
  echo "docker-test: another container is running; wait for it to finish (docker ps)" >&2
  exit 1
fi

# Refuse to start while the host's one-minute load is at or above the limit.
host_load() {
  if [[ "$(uname -s)" == "Darwin" ]]; then
    sysctl -n vm.loadavg | awk '{print $2}'
  else
    cut -d' ' -f1 /proc/loadavg
  fi
}
host_cpus() {
  if [[ "$(uname -s)" == "Darwin" ]]; then
    sysctl -n hw.ncpu
  else
    nproc
  fi
}
load=$(host_load)
cpus=$(host_cpus)
if awk -v l="$load" -v c="$cpus" -v p="$MAX_LOAD_PERCENT" 'BEGIN { exit !(l * 100 >= c * p) }'; then
  echo "docker-test: host load $load is at or above ${MAX_LOAD_PERCENT}% of $cpus CPUs; try again later" >&2
  exit 1
fi

# The script that runs inside the container. Cargo.lock must be committed (--locked).
# Only lock and format write to the repository; every other step sees it read-only.
mount_mode=ro
case "$step" in
  lock | format) mount_mode=rw ;;
esac

# The script that runs inside the container. Cargo.lock must be committed (--locked).
read -r -d '' inner <<'INNER' || true
set -eu
export TMPDIR=/tmp/supercy-test
mkdir -p "$TMPDIR" /work
rustup component add rustfmt clippy >/dev/null 2>&1 || true
case "$STEP" in
  lock) cd /src && cargo metadata --format-version 1 >/dev/null && echo "Cargo.lock updated"; exit 0 ;;
  format) cd /src && cargo fmt --all && echo "sources formatted"; exit 0 ;;
esac
tar -C /src --exclude=./target --exclude=./.git -cf - . | tar -C /work -xf -
cd /work
run_fmt() { cargo fmt --all --check; }
run_clippy() { cargo clippy --workspace --all-targets --locked -- -D warnings; }
run_test() { cargo test --workspace --locked; }
run_bench() { cargo test --workspace --release --locked -- --ignored --nocapture perf_; }
# Every dependency must offer a permissive license (docs/development/standards.md, section 6).
run_licenses() {
  allowed='MIT|Apache-2\.0|BSD-2-Clause|BSD-3-Clause|ISC|Zlib|Unicode-3\.0|Unicode-DFS-2016|0BSD'
  cargo tree --workspace --locked -e normal,build --prefix none --format '{p} | {l}' | sort -u >/tmp/licenses.txt
  bad=$(grep -v ' (/work' /tmp/licenses.txt | grep -Ev "\| .*($allowed)" || true)
  if [ -n "$bad" ]; then
    echo "licenses: dependencies outside the allowlist:" >&2
    echo "$bad" >&2
    exit 1
  fi
  echo "licenses: $(grep -vc ' (/work' /tmp/licenses.txt) dependencies, all permissive"
}
run_cover() {
  export CARGO_INSTALL_ROOT=/tools PATH="/tools/bin:$PATH"
  rustup component add llvm-tools-preview >/dev/null
  command -v cargo-llvm-cov >/dev/null || cargo install --locked cargo-llvm-cov
  cargo llvm-cov --workspace --locked --fail-under-lines "$COVER_MIN" --summary-only
}
case "$STEP" in
  all) run_fmt; run_clippy; run_licenses; run_test ;;
  fmt) run_fmt ;;
  clippy) run_clippy ;;
  licenses) run_licenses ;;
  test) run_test ;;
  bench) run_bench ;;
  cover) run_cover ;;
esac
INNER

exec docker run --rm \
  --cpus="$CPUS" --memory="$MEMORY" \
  -v "$repo_root:/src:$mount_mode" \
  -v "$TARGET_VOLUME:/target" \
  -v "$REGISTRY_VOLUME:/usr/local/cargo/registry" \
  -v "$TOOLS_VOLUME:/tools" \
  -e COVER_MIN="$COVER_MIN" \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_TERM_COLOR=always \
  -e STEP="$step" \
  "$IMAGE" sh -c "$inner"
