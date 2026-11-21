#!/bin/sh
# Run the offline test suite and `tack install --skip-tools` in a Linux container (podman or docker).
# Repo is mounted read-only; the build uses a container-local target dir.
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
# node:22 (Debian) already has git, curl and gcc, which tack's prerequisite check and the build need.
image="${TACK_TEST_IMAGE:-docker.io/library/node:22}"

if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  rt=docker
elif command -v podman >/dev/null 2>&1; then
  rt=podman
  if ! podman info >/dev/null 2>&1; then
    echo "podman machine is not running; starting it (podman machine start)"
    podman machine start
  fi
else
  echo "no container runtime answers" >&2
  echo "fix: install podman (brew install podman && podman machine init && podman machine start)" >&2
  exit 1
fi
echo "runtime: $rt, image: $image"

# shellcheck disable=SC2016 # expands inside the container
"$rt" run --rm -v "$repo:/src:ro" -w /src -e CARGO_TARGET_DIR=/tmp/target "$image" sh -euc '
  node --version
  curl -fsSL https://sh.rustup.rs | sh -s -- -y -q --profile minimal >/dev/null
  . "$HOME/.cargo/env"
  cargo test
  cargo build --release
  export HOME=$(mktemp -d)
  /tmp/target/release/tack install --skip-tools
  /tmp/target/release/tack list
'
