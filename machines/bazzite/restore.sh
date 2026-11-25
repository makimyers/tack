#!/bin/sh
# Restore Maki's Bazzite setup using the checked-in pins.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
. "$here/versions.env"
tack_home=${TACK_HOME:-$HOME/.tack}
export PATH="$HOME/.local/bin:$PATH"
command -v cargo >/dev/null || { echo "Install Rust before restoring Tack" >&2; exit 1; }

sh "$here/dependencies.sh" sync
sh "$here/t3-fork.sh"

if [ "$(pi --version 2>/dev/null || true)" != "$PI_VERSION" ] || [ ! -x "$HOME/.local/bin/pi" ]; then
  npm install -g --prefix "$HOME/.local" --ignore-scripts --no-audit --no-fund "@earendil-works/pi-coding-agent@$PI_VERSION"
fi

mkdir -p "$tack_home"
if [ ! -e "$tack_home/tack.toml" ]; then
  cp "$here/tack.toml" "$tack_home/tack.toml"
fi
sh "$root/install.sh"
ln -sfnT "$here/dependencies.sh" "$tack_home/bin/check-dependencies"
tack on --pi superpowers games \
  3d-essentials animation-system audio-system camera-system event-bus \
  gdscript-advanced gdscript-patterns godot-brainstorming godot-code-review godot-debugging \
  godot-optimization godot-testing godot-ui hud-system input-handling math-essentials \
  particles-vfx physics-system player-controller rendering-pipeline resource-pattern \
  save-load scene-organization shader-basics state-machine tween-animation using-godot-prompter

# 4. The Tack app-menu launcher.
mkdir -p "$HOME/.local/share/applications"
cp "$here/tack.desktop" "$HOME/.local/share/applications/tack.desktop"
command -v kbuildsycoca6 >/dev/null && kbuildsycoca6 >/dev/null 2>&1 || true

# 5. pi's MCP servers (Godot AI, loaded only when a task needs it). An existing file is kept.
if [ -f "$HOME/.pi/agent/mcp.json" ]; then
  echo "kept ~/.pi/agent/mcp.json (compare with $here/pi-mcp.json)"
else
  mkdir -p "$HOME/.pi/agent"
  cp "$here/pi-mcp.json" "$HOME/.pi/agent/mcp.json"
fi

cat <<EOF

If the machine is fresh, finish the host and model setup:
  1. Install and sign in to Claude Code if you use it.
  2. Build the ROCm SDK and Avalon if tack doctor flags them:
     cd ~/Projects/fork-strata && sh setup.sh
     ~/Projects/fork-avalon/local/build.sh
  3. Download the pinned Avalon weights if tack doctor flags them (~90 GiB):
     uvx --from 'huggingface_hub[hf_xet]' hf download $AVALON_MODEL --revision $AVALON_REVISION --local-dir ~/Avalon-models/Qwen3.8-Flash-Next-Avalon-GGUF --include '*.gguf'
     uvx --from 'huggingface_hub[hf_xet]' hf download $AVALON_PROJECTOR mmproj-Qwen3.8-Flash-Next-BF16.gguf --revision $AVALON_PROJECTOR_REVISION --local-dir ~/Avalon-models/Qwen3.8-Flash-Next-Avalon-GGUF
  4. Install Godot 4.7.2 (Flathub org.godotengine.Godot), then open game-brilliant/game.
     Configure Claude Code from the Godot AI panel if you use that integration.
EOF
tack doctor
