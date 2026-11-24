#!/bin/sh
# Rebuild Maki's Bazzite desktop setup (RX 7900 XTX) from GitHub. Safe to re-run: every step skips
# what is already there. The slow and manual steps are printed at the end, not run.
#   sh ~/Projects/tack/machines/bazzite/restore.sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
# tack must be built from this repo: the upstream release binary lacks [local_model] and the T3 buttons.
command -v cargo >/dev/null || { echo "install Rust first: curl https://sh.rustup.rs -sSf | sh"; exit 1; }
P="$HOME/Projects"
mkdir -p "$P"

# 1. The repos this setup uses (all on github.com/makimyers).
for repo in tack fork-avalon fork-strata tool-model-deck tool-claude-setup tool-multitree game-brilliant; do
  if [ -d "$P/$repo/.git" ]; then
    echo "have $repo"
  else
    git clone "git@github.com:makimyers/$repo.git" "$P/$repo"
  fi
done

# 2. tack itself: skills, guard, rules, pi, T3 Code, rtk.
(cd "$P/tack" && ./install.sh)

# 3. This machine's local model (Avalon long, 256K), then tack wires pi and T3 Code's buttons to it.
cp "$here/tack.toml" "$HOME/.tack/tack.toml"
tack install

# 4. The Tack app-menu launcher.
mkdir -p "$HOME/.local/share/applications"
cp "$here/tack.desktop" "$HOME/.local/share/applications/tack.desktop"
command -v kbuildsycoca6 >/dev/null && kbuildsycoca6 >/dev/null 2>&1 || true

# 5. pi's MCP servers (Godot AI, loaded only when a task needs it). An existing file is kept.
if [ -f "$HOME/.pi/agent/mcp.json" ]; then
  echo "kept ~/.pi/agent/mcp.json (compare with $here/pi-mcp.json)"
else
  cp "$here/pi-mcp.json" "$HOME/.pi/agent/mcp.json"
fi

cat <<'EOF'

Done with the quick part. Still to do by hand (slow or needs you):
  1. Claude Code: install it from https://claude.com/product/claude-code, then `claude auth login`.
  2. ROCm SDK for the Avalon build:  cd ~/Projects/fork-strata && sh setup.sh
  3. Build Avalon (10-20 min):       ~/Projects/fork-avalon/local/build.sh
  4. Avalon weights (~90 GiB):
     uvx --from 'huggingface_hub[hf_xet]' hf download freriadoc/Qwen3.8-Flash-Next-Avalon-GGUF \
       --local-dir ~/Avalon-models/Qwen3.8-Flash-Next-Avalon-GGUF --exclude '*.jpg'
  5. Godot 4.7.2 (Flathub org.godotengine.Godot), open game-brilliant/game, then in the Godot AI
     panel press Configure next to Claude Code.
  6. T3 Code fork (pi model name, reply speed; ~5 min): sh ~/Projects/tack/machines/bazzite/t3-fork.sh
  7. Check:  tack doctor  &&  tack model on  &&  tack model status
EOF
