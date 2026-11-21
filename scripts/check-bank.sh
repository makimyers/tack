#!/bin/sh
# Proves every `bank` skill is off after a fresh install and can be switched on.
# Runs the release binary in a throwaway HOME (needs network to fetch third-party skills).
set -u

cd "$(dirname "$0")/.." || exit 1
cargo build --release --quiet || exit 1
home=$(mktemp -d)
trap 'rm -rf "$home"' EXIT
tack="$PWD/target/release/tack"
# Unset overrides so nothing can land in the real ~/.tack, ~/.claude or ~/.t3.
run() { env -u TACK_HOME -u CLAUDE_CONFIG_DIR -u T3CODE_HOME HOME="$home" PATH="$home/.local/bin:$PATH" "$tack" "$@"; }

names=$(awk '/^\[\[skill\]\]/ { n = "" } /^name = / { n = $3 } /^group = "bank"/ { print n }' manifest/skills.toml | tr -d '"')
count=$(echo "$names" | wc -l | tr -d ' ')
[ -n "$names" ] || { echo "FAIL  no bank skills in manifest"; exit 1; }

run install --skip-tools >"$home/install.log" 2>&1 || { echo "FAIL  tack install"; tail -n 20 "$home/install.log"; exit 1; }

fail=0
for n in $names; do
  for dir in "$home/.claude/skills" "$home/.pi/agent/skills"; do
    if [ -e "$dir/$n" ] || [ -L "$dir/$n" ]; then
      echo "FAIL  $n linked after a fresh install: $dir/$n"
      fail=1
    fi
  done
done

row=$(run list | grep '^bank ')
case "$row" in
  *"group of $count skills"*) echo "ok    list: $row" ;;
  *) echo "FAIL  list row for bank: '$row' (want group of $count skills)"; fail=1 ;;
esac
all=$(run list --all)
for n in $names; do
  # list cuts names longer than 32 columns, so match on a prefix.
  echo "$all" | grep -q "^$(echo "$n" | cut -c1-30)" || { echo "FAIL  list --all has no row for $n"; fail=1; }
done

for n in $names; do
  if ! out=$(run on "$n" 2>&1); then
    echo "FAIL  tack on $n: $out"
    fail=1
    continue
  fi
  good=1
  for dir in "$home/.claude/skills" "$home/.pi/agent/skills"; do
    if ! { [ -L "$dir/$n" ] && [ -f "$dir/$n/SKILL.md" ]; }; then
      echo "FAIL  $n: no linked SKILL.md at $dir/$n"
      good=0
      fail=1
    fi
  done
  [ "$good" = 1 ] && echo "ok    $n"
done

[ "$fail" = 0 ] && echo "ok    $count bank skills off by default, each switches on"
exit "$fail"
