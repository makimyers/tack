#!/bin/sh
# Verify patch reconstruction and drift handling without GitHub or a real home.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf -- "$tmp"' EXIT

src="$tmp/upstream"
profile="$tmp/profile"
export TACK_PROJECTS_DIR="$tmp/projects"
mkdir -p "$src" "$profile"
cp "$here/dependencies.sh" "$here/versions.env" "$profile/"
git -C "$src" init -q
git -C "$src" config user.name Fixture
git -C "$src" config user.email fixture@localhost
printf 'base\n' > "$src/file"
git -C "$src" add file
git -C "$src" commit -qm base
base=$(git -C "$src" rev-parse HEAD)
printf 'patched\n' > "$src/file"
git -C "$src" add file
tree=$(git -C "$src" write-tree)
git -C "$src" diff --cached --binary > "$profile/fixture.patch"

printf 'fixture file://%s %s %s fixture.patch\n' "$src" "$base" "$tree" > "$profile/dependencies.lock"
sh "$profile/dependencies.sh" sync fixture
sh "$profile/dependencies.sh" check fixture
sh "$profile/dependencies.sh" sync fixture
test "$(cat "$TACK_PROJECTS_DIR/fixture/file")" = patched
printf 'local work\n' > "$TACK_PROJECTS_DIR/fixture/file"
if sh "$profile/dependencies.sh" sync fixture; then
  echo "sync accepted a dirty worktree" >&2
  exit 1
fi
test "$(cat "$TACK_PROJECTS_DIR/fixture/file")" = 'local work'

printf 'bad-tree file://%s %s %s fixture.patch\n' "$src" "$base" "$base" >> "$profile/dependencies.lock"
if sh "$profile/dependencies.sh" sync bad-tree; then
  echo "sync accepted a bad expected tree" >&2
  exit 1
fi
test ! -e "$TACK_PROJECTS_DIR/bad-tree"
test -z "$(find "$TACK_PROJECTS_DIR" -maxdepth 1 -name '.bad-tree.*' -print)"
