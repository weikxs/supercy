#!/usr/bin/env bash
# Runs the Rust checks inside a Linux container. All Supercy builds and tests run
# in Docker: nothing is built or installed on the host.
#
# The repository is mounted read-only and copied into the container, so the host
# tree is never written; build output, the crate cache and installed tools live in
# named volumes. Only the write steps (lock, format, bindings-write, golden-write)
# mount it read-write. The check steps run scripts/check.sh in the container, the
# same script CI runs on its Linux runners.
#
# Usage: scripts/docker-test.sh [step]
#   all (default)   fmt, clippy, deny, audit, test, bindings
#   fmt | clippy | deny | audit | test | bindings   one check
#   bench           performance budgets (release build, ignored perf_ tests)
#   cover           line coverage, fails below COVER_MIN
#   fuzz            every fuzz target for FUZZ_SECONDS each (nightly image)
#   lock            write Cargo.lock files after a dependency change
#   format          run cargo fmt on the sources
#   bindings-write  regenerate the committed Swift bindings
#   golden-write    regenerate the annotation golden images (crates/cy-annotate/tests/golden)
#   icons           render design/icons/*.svg to the app's PNGs (Debian image with librsvg)
set -euo pipefail

readonly IMAGE="${SUPERCY_RUST_IMAGE:-rust:1.95-slim}"
readonly NIGHTLY_IMAGE="${SUPERCY_NIGHTLY_IMAGE:-rustlang/rust:nightly}"
readonly ICONS_IMAGE="${SUPERCY_ICONS_IMAGE:-debian:bookworm-slim}"
readonly CPUS=4
readonly MEMORY=5g
readonly TARGET_VOLUME="supercy-target"
readonly REGISTRY_VOLUME="supercy-cargo-registry"
readonly TOOLS_VOLUME="supercy-tools"
readonly NIGHTLY_TOOLS_VOLUME="supercy-tools-nightly"
readonly COVER_MIN=80
# Seconds per fuzz target: short in the harness, longer on demand (FUZZ_SECONDS=600).
readonly FUZZ_SECONDS="${FUZZ_SECONDS:-30}"
# Do not start a container when the host is busy: at most 70% of its CPUs.
readonly MAX_LOAD_PERCENT=70

repo_root=$(git rev-parse --show-toplevel)
step=${1:-all}

case "$step" in
  all | fmt | clippy | deny | audit | test | bindings | bench | cover | fuzz | lock | format | bindings-write | golden-write | icons) ;;
  *)
    echo "usage: $0 [all|fmt|clippy|deny|audit|test|bindings|bench|cover|fuzz|lock|format|bindings-write|golden-write|icons]" >&2
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

mount_mode=ro
image=$IMAGE
tools_volume=$TOOLS_VOLUME
case "$step" in
  lock | format | bindings-write | golden-write) mount_mode=rw ;;
  icons)
    mount_mode=rw
    image=$ICONS_IMAGE
    ;;
  fuzz)
    image=$NIGHTLY_IMAGE
    tools_volume=$NIGHTLY_TOOLS_VOLUME
    ;;
esac

# The script that runs inside the container. Cargo.lock must be committed (--locked).
read -r -d '' inner <<'INNER' || true
set -eu
export TMPDIR=/tmp/supercy-test CARGO_INSTALL_ROOT=/tools PATH="/tools/bin:$PATH"
mkdir -p "$TMPDIR" /work

# Swift bindings: build the library, then generate from its metadata (UniFFI library mode).
generate_bindings() {
  cargo build -p cy-ffi --locked --quiet
  rm -rf /tmp/bindings
  cargo run -p cy-bindgen --locked --quiet -- generate --library /target/debug/libcy_ffi.so \
    --language swift --out-dir /tmp/bindings --no-format >/dev/null
}

case "$STEP" in
  icons)
    apt-get update -qq >/dev/null && apt-get install -y -qq --no-install-recommends librsvg2-bin imagemagick >/dev/null
    sh /src/design/icons/render.sh; exit 0 ;;
  lock)
    cd /src && cargo metadata --format-version 1 >/dev/null
    cd /src/fuzz && cargo +1.95.0 metadata --format-version 1 >/dev/null
    echo "Cargo.lock files updated"; exit 0 ;;
  format) cd /src && cargo fmt --all && (cd fuzz && cargo +1.95.0 fmt --all) && echo "sources formatted"; exit 0 ;;
  bindings-write)
    cd /src && generate_bindings
    rm -rf /src/bindings/swift && mkdir -p /src/bindings && cp -r /tmp/bindings /src/bindings/swift
    echo "Swift bindings written to bindings/swift"; exit 0 ;;
  golden-write)
    cd /src && SUPERCY_UPDATE_GOLDEN=1 cargo test -p cy-annotate --locked --test golden --quiet
    echo "golden images written to crates/cy-annotate/tests/golden"; exit 0 ;;
esac

tar -C /src --exclude=./target --exclude=./.git --exclude=./fuzz/target -cf - . | tar -C /work -xf -
cd /work
rustup component add rustfmt clippy >/dev/null 2>&1 || true
# The checks themselves are scripts/check.sh, which CI runs natively on its Linux runners.
exec bash scripts/check.sh "$STEP"
INNER

exec docker run --rm \
  --cpus="$CPUS" --memory="$MEMORY" \
  -v "$repo_root:/src:$mount_mode" \
  -v "$TARGET_VOLUME:/target" \
  -v "$REGISTRY_VOLUME:/usr/local/cargo/registry" \
  -v "$tools_volume:/tools" \
  -e COVER_MIN="$COVER_MIN" \
  -e FUZZ_SECONDS="$FUZZ_SECONDS" \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_TERM_COLOR=always \
  -e STEP="$step" \
  "$image" sh -c "$inner"
