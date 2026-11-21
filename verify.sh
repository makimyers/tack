#!/bin/sh
# Project oracle: one line per step, exits non-zero on the first failure.
# Set VERIFY_ONLINE=1 to also smoke-test the release binary (fetches third-party skills).
set -u

cd "$(dirname "$0")" || exit 1
log=$(mktemp)
trap 'rm -f "$log"' EXIT

step() {
  name=$1
  shift
  if "$@" >"$log" 2>&1; then
    echo "ok    $name"
  else
    echo "FAIL  $name"
    tail -n 30 "$log"
    exit 1
  fi
}

step "cargo fmt --check" cargo fmt --check
step "cargo clippy" cargo clippy --all-targets -- -D warnings
step "cargo test" cargo test
step "sh -n install.sh" sh -n install.sh
if command -v shellcheck >/dev/null 2>&1; then
  step "shellcheck" shellcheck install.sh Install.command verify.sh scripts/*.sh
else
  echo "skip  shellcheck (not installed)"
fi

if [ "${VERIFY_ONLINE:-}" != 1 ]; then
  echo "skip  smoke run (needs network for third-party skills; set VERIFY_ONLINE=1)"
  exit 0
fi

step "cargo build --release" cargo build --release
home=$(mktemp -d)
trap 'rm -f "$log"; rm -rf "$home"' EXIT
tack="$PWD/target/release/tack"
# Unset overrides so nothing can land in the real ~/.tack, ~/.claude or ~/.t3.
run() { env -u TACK_HOME -u CLAUDE_CONFIG_DIR -u T3CODE_HOME HOME="$home" PATH="$home/.local/bin:$PATH" "$tack" "$@"; }

step "tack install --skip-tools" run install --skip-tools
step "tack list" run list
step "tack off goal-loop --pi" run off goal-loop --pi
step "tack on goal-loop --pi" run on goal-loop --pi
# Exit 1 is expected: --skip-tools left the base tools uninstalled, so doctor reports them.
doctor() { run doctor; [ $? -le 1 ]; }
step "tack doctor (exit 0 or 1)" doctor
step "tack uninstall" run uninstall
no_links() {
  left=$(find "$home" -type l 2>/dev/null | while read -r l; do
    # Leading paren: bash 3.2 (macOS sh) misparses an unbalanced `)` inside $( ).
    case "$(readlink "$l")" in ("$home/.tack"*) echo "$l" ;; esac
  done)
  [ -z "$left" ] || { echo "links into .tack remain: $left"; return 1; }
}
step "no links into .tack remain" no_links
