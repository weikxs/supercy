#!/usr/bin/env bash
# Runs the Rust checks (fmt, clippy -D warnings, tests) inside a Linux container.
# All Supercy tests run in Docker: nothing is built or installed on the host.
#
# The repository is mounted read-only and copied into the container, so the host
# tree is never written; build output and the crate cache live in named volumes.
#
# Usage: scripts/docker-test.sh            # fmt + clippy + test
#        scripts/docker-test.sh test       # one step: fmt | clippy | test
set -euo pipefail

readonly IMAGE="${SUPERCY_RUST_IMAGE:-rust:1.95-slim}"
readonly CPUS=4
readonly MEMORY=5g
readonly TARGET_VOLUME="supercy-target"
readonly REGISTRY_VOLUME="supercy-cargo-registry"
# Do not start a container when the host is busy: at most 70% of its CPUs.
readonly MAX_LOAD_PERCENT=70

repo_root=$(git rev-parse --show-toplevel)
step=${1:-all}

case "$step" in
  all | fmt | clippy | test) ;;
  *)
    echo "usage: $0 [all|fmt|clippy|test]" >&2
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
read -r -d '' inner <<'INNER' || true
set -eu
export TMPDIR=/tmp/supercy-test
mkdir -p "$TMPDIR" /work
tar -C /src --exclude=./target --exclude=./.git -cf - . | tar -C /work -xf -
cd /work
rustup component add rustfmt clippy >/dev/null 2>&1 || true
run_fmt() { cargo fmt --all --check; }
run_clippy() { cargo clippy --workspace --all-targets --locked -- -D warnings; }
run_test() { cargo test --workspace --locked; }
case "$STEP" in
  all) run_fmt; run_clippy; run_test ;;
  fmt) run_fmt ;;
  clippy) run_clippy ;;
  test) run_test ;;
esac
INNER

exec docker run --rm \
  --cpus="$CPUS" --memory="$MEMORY" \
  -v "$repo_root:/src:ro" \
  -v "$TARGET_VOLUME:/target" \
  -v "$REGISTRY_VOLUME:/usr/local/cargo/registry" \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_TERM_COLOR=always \
  -e STEP="$step" \
  "$IMAGE" sh -c "$inner"
