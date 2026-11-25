#!/bin/sh
# Build the pinned T3 fork and install it as the launcher used by Tack.
# Close T3 before rebuilding. A matching installed build is reused.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
. "$here/versions.env"
SRC="${TACK_PROJECTS_DIR:-$HOME/Projects}/fork-t3code"
sh "$here/dependencies.sh" sync fork-t3code

tree=$(git -C "$SRC" rev-parse 'HEAD^{tree}')
versions="$HOME/.t3/runtime/versions"
dest="$versions/$T3_VERSION"
mkdir -p "$versions"
exec 9>"$versions/.tack-build.lock"
flock -n 9 || { echo "A T3 build is already installing" >&2; exit 1; }
if T3CODE_HOME="$HOME/.t3" sh "$here/dependencies.sh" check t3-runtime >/dev/null; then
  echo "T3 $T3_VERSION already matches the lock"
  exit 0
fi

NODE_DIR="$HOME/.local/opt/node-$NODE_VERSION-linux-x64"
WORK=$(mktemp -d)
stage=
rollback=false
cleanup() {
  if [ -n "$stage" ]; then
    if [ "$rollback" = true ]; then
      if [ ! -d "$stage/build" ]; then mv -T "$dest" "$stage/build"; fi
      if [ -d "$stage/previous" ]; then mv -T "$stage/previous" "$dest"; fi
    fi
    rm -rf -- "$stage"
  fi
  rm -rf -- "$WORK"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

# Tools the release script expects from an Ubuntu CI runner.
if [ ! -x "$NODE_DIR/bin/node" ]; then
  mkdir -p "$HOME/.local/opt"
  f="node-$NODE_VERSION-linux-x64.tar.xz"
  (cd "$WORK" && curl -fsSLO "https://nodejs.org/dist/$NODE_VERSION/$f" &&
    curl -fsSL "https://nodejs.org/dist/$NODE_VERSION/SHASUMS256.txt" | grep " $f\$" | sha256sum -c - &&
    tar -xf "$f" -C "$HOME/.local/opt")
fi
command -v patchelf >/dev/null || { echo "Install patchelf before building T3" >&2; exit 1; }
mkdir -p "$WORK/shim"
# build-cli-archive.ts names libatomic's Ubuntu package via dpkg; on Fedora answer for it.
cat > "$WORK/shim/dpkg-query" <<'EOF'
#!/bin/sh
case "$1" in
  -S) echo "libatomic-fedora: $2" ;;
  -W) rpm -q --qf '%{VERSION}-%{RELEASE} gcc %{VERSION}-%{RELEASE}' libatomic ;;
esac
EOF
chmod +x "$WORK/shim/dpkg-query"

cd "$SRC"
export PATH="$WORK/shim:$NODE_DIR/bin:$SRC/node_modules/.bin:$PATH"
test "$(node -p 'require("./apps/server/package.json").version')" = "$T3_VERSION"
npm install --prefix "$WORK/tools" --ignore-scripts --no-audit --no-fund "@pnpm/exe.linux-x64@$PNPM_VERSION"
ln -s "$WORK/tools/node_modules/@pnpm/exe.linux-x64/pnpm" "$WORK/shim/pnpm"
test "$(pnpm --version)" = "$PNPM_VERSION"
pnpm install --frozen-lockfile
vp run --filter t3 build
node apps/server/scripts/cli.ts build-exe
cargo build --locked --release --manifest-path native/resource-monitor/Cargo.toml
mkdir -p apps/server/dist/resource-monitor/linux-x64
cp native/resource-monitor/target/release/t3-resource-monitor apps/server/dist/resource-monitor/linux-x64/
node scripts/build-cli-archive.ts --platform linux --arch x64 --version "$T3_VERSION" --output-dir "$WORK/cli"
node scripts/build-npm-launcher.ts --version "$T3_VERSION" --output-dir "$WORK/launcher"
archive="$WORK/cli/t3-$T3_VERSION-linux-x64.tar.gz"
node scripts/smoke-cli-archive.ts --archive "$archive" \
  --expect-version "$(node -p 'require("./apps/server/package.json").version')"

# Back up existing threads without opening the user's data for writes.
database="$HOME/.t3/userdata/statev2.sqlite"
if [ -f "$database" ]; then
  backup=$(mktemp -d "$HOME/.t3/backup-$(date +%F-%H%M).XXXXXX")
  sqlite3 -readonly "$database" ".backup \"$backup/statev2.sqlite\""
fi

stage=$(mktemp -d "$versions/.stage.XXXXXX")
mkdir "$stage/build"
tar -xzf "$archive" -C "$stage/build" --strip-components=1
printf '%s\n' "$T3_VERSION" > "$stage/build/.install-complete"
printf '%s\n' "$tree" > "$stage/build/.source-tree"

rollback=true
if [ -d "$dest" ]; then mv -T "$dest" "$stage/previous"; fi
mv -T "$stage/build" "$dest"
npm install -g --prefix "$HOME/.local" --ignore-scripts --no-audit --no-fund "$WORK/launcher/t3.tgz"
T3CODE_HOME="$HOME/.t3" sh "$here/dependencies.sh" check t3-runtime
rollback=false
echo "Installed T3 $T3_VERSION from locked source tree $tree"
