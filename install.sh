#!/bin/sh
# tack bootstrap: builds from a checkout, or downloads the release binary, then runs `tack install`.
# Usage: curl -fsSL https://raw.githubusercontent.com/makidatacity/tack/main/install.sh | sh
set -eu

TACK_REPO="${TACK_REPO:-makidatacity/tack}"

die() {
  echo "tack: $1" >&2
  [ $# -lt 2 ] || echo "fix: $2" >&2
  exit 1
}

# Run from a checkout (not piped, where $0 is just `sh`): build it.
if [ -f "$0" ] && [ "$(basename "$0")" = install.sh ]; then
  here=$(cd "$(dirname "$0")" && pwd)
  if [ -f "$here/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
    echo "building tack from $here"
    (cd "$here" && cargo build --release)
    exec "$here/target/release/tack" install "$@"
  fi
fi

case "$(uname -s)" in
  Darwin) os=macos ;;
  Linux) os=linux ;;
  *) die "unsupported OS $(uname -s); tack runs on Apple Silicon Macs and Linux (x86_64, aarch64)" ;;
esac
cpu=$(uname -m)
# A Rosetta shell on Apple Silicon reports x86_64.
if [ "$os" = macos ] && [ "$(sysctl -in sysctl.proc_translated 2>/dev/null)" = 1 ]; then
  cpu=arm64
fi
case "$cpu" in
  arm64 | aarch64) arch=$([ "$os" = macos ] && echo arm64 || echo aarch64) ;;
  x86_64 | amd64)
    [ "$os" = linux ] || die "Intel Macs are not supported: T3 Code ships no runtime for them" "use an Apple Silicon Mac or Linux"
    arch=x86_64
    ;;
  *) die "unsupported CPU $cpu" ;;
esac
asset="tack-$os-$arch"

command -v curl >/dev/null 2>&1 || die "curl not found" "install curl with your package manager, then re-run"
if command -v sha256sum >/dev/null 2>&1; then
  sha() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  die "no sha256sum or shasum found" "install coreutils, then re-run"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

base="https://github.com/$TACK_REPO/releases/latest/download"
fetch() {
  code=$(curl --proto '=https' --tlsv1.2 -sSL -o "$tmp/$1" -w '%{http_code}' "$base/$1") || die "download of $base/$1 failed" "check your network, then re-run"
  case "$code" in
    200) ;;
    404) die "no release asset $1 at $base (the repo $TACK_REPO has no published release yet)" "clone the repo and run ./install.sh from it, or set TACK_REPO=owner/repo" ;;
    *) die "download of $base/$1 returned HTTP $code" "re-run later" ;;
  esac
}

echo "downloading $asset from $TACK_REPO"
fetch "$asset"
fetch SHA256SUMS

want=$(awk -v a="$asset" '$2 == a || $2 == "*"a { print $1 }' "$tmp/SHA256SUMS")
[ -n "$want" ] || die "SHA256SUMS has no entry for $asset" "report this at https://github.com/$TACK_REPO/issues"
got=$(sha "$tmp/$asset")
[ "$want" = "$got" ] || die "checksum mismatch for $asset (want $want, got $got); refusing to run it" "re-run; if it repeats, report it"

chmod +x "$tmp/$asset"
"$tmp/$asset" install "$@"
