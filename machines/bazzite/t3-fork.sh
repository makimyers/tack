#!/bin/sh
# Build Maki's T3 Code fork (github.com/makimyers/t3code, local clone ~/Projects/fork-t3code) and
# install it as the `t3` that `tack open` starts. The stock npm `t3` has none of the fork's changes
# (pi model name, reply speed in tok/s).
#   sh ~/Projects/tack/machines/bazzite/t3-fork.sh [version]     (default 0.1.11-maki.1)
# Restart T3 afterwards: close the Tack window, click Tack. Back to stock: npm install -g t3@0.1.11
# (the fork may have migrated the database further; a copy is kept in ~/.t3/backup-<date>/).
set -eu

V=${1:-0.1.11-maki.1}
SRC="$HOME/Projects/fork-t3code"
NODE_V=v26.13.1   # the fork's package.json engines; Node SEA executables need this exact version
NODE_DIR="$HOME/.local/opt/node-$NODE_V-linux-x64"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

[ -d "$SRC/.git" ] || git clone git@github.com:makimyers/t3code.git "$SRC"

# Tools the release script expects from an Ubuntu CI runner.
if [ ! -x "$NODE_DIR/bin/node" ]; then
  mkdir -p "$HOME/.local/opt"
  f="node-$NODE_V-linux-x64.tar.xz"
  (cd "$WORK" && curl -fsSLO "https://nodejs.org/dist/$NODE_V/$f" &&
    curl -fsSL "https://nodejs.org/dist/$NODE_V/SHASUMS256.txt" | grep " $f\$" | sha256sum -c - &&
    tar -xf "$f" -C "$HOME/.local/opt")
fi
command -v patchelf >/dev/null || brew install patchelf
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
vp i
vp run --filter t3 build
node apps/server/scripts/cli.ts build-exe
cargo build --locked --release --manifest-path native/resource-monitor/Cargo.toml
mkdir -p apps/server/dist/resource-monitor/linux-x64
cp native/resource-monitor/target/release/t3-resource-monitor apps/server/dist/resource-monitor/linux-x64/
node scripts/build-cli-archive.ts --platform linux --arch x64 --version "$V" --output-dir "$WORK/cli"
node scripts/build-npm-launcher.ts --version "$V" --output-dir "$WORK/launcher"
archive="$WORK/cli/t3-$V-linux-x64.tar.gz"
node scripts/smoke-cli-archive.ts --archive "$archive" \
  --expect-version "$(node -p 'require("./apps/server/package.json").version')"

# Back up the live database (read-only), then install the runtime where the launcher looks for it.
backup="$HOME/.t3/backup-$(date +%F-%H%M)"
mkdir -p "$backup"
sqlite3 "file:$HOME/.t3/userdata/statev2.sqlite?mode=ro" ".backup $backup/statev2.sqlite"
dest="$HOME/.t3/runtime/versions/$V"
rm -rf "$dest.staging"
mkdir -p "$dest.staging"
tar -xzf "$archive" -C "$dest.staging" --strip-components=1
echo "$V" > "$dest.staging/.install-complete"
rm -rf "$dest"
mv "$dest.staging" "$dest"
npm install -g "$WORK/launcher/t3.tgz"
echo "installed T3 Code fork $V ($(git -C "$SRC" log -1 --format='%h %s')). Restart: close the Tack window, click Tack."
