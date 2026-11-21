#!/bin/sh
# Cross-build static Linux binaries (musl) from macOS into dist/, linking with rustup's bundled rust-lld.
set -eu

cd "$(dirname "$0")/.."

host=$(rustc -vV | sed -n 's/^host: //p')
lld="$(rustc --print sysroot)/lib/rustlib/$host/bin/rust-lld"
if [ ! -x "$lld" ]; then
  echo "rust-lld not found at $lld" >&2
  echo "fix: rustup component add llvm-tools  (or install cross: cargo install cross)" >&2
  exit 1
fi

mkdir -p dist
for arch in x86_64 aarch64; do
  triple="$arch-unknown-linux-musl"
  if ! rustup target list --installed | grep -qx "$triple"; then
    echo "missing target $triple" >&2
    echo "fix: rustup target add $triple" >&2
    exit 1
  fi
  var="CARGO_TARGET_$(echo "$triple" | tr 'a-z-' 'A-Z_')_LINKER"
  env "$var=$lld" RUSTFLAGS="-C link-self-contained=yes" \
    cargo build --release --target "$triple"
  cp "target/$triple/release/tack" "dist/tack-linux-$arch"
  echo "built dist/tack-linux-$arch"
done
