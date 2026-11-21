# Bazzite desktop (Maki's PC)

RX 7900 XTX 24 GB, i5-13600K, 96 GB RAM, Bazzite (Fedora Atomic, home on /var/home). Saved 2026-11-21.

Fresh machine or after a crash: clone this repo to `~/Projects/tack`, then run

```sh
sh ~/Projects/tack/machines/bazzite/restore.sh
```

It clones the other repos, installs tack, puts this machine's settings in place and prints the slow
steps (ROCm SDK, Avalon build, 90 GiB of weights, Godot AI's one-click connect).

| File | Goes to | What |
|---|---|---|
| `tack.toml` | `~/.tack/tack.toml` | the one local model: Qwen3.8 Flash-Next, Avalon fast (128K context, port 8080) |
| `tack.desktop` | `~/.local/share/applications/` | the **Tack** app-menu item (`tack open` in a Konsole window) |
| `pi-mcp.json` | `~/.pi/agent/mcp.json` | Godot AI for pi, loaded only when a task searches for it |

## Daily use

1. Click **Tack** in the app menu. T3 Code opens in the browser; the Konsole window keeps it running.
2. Local model: at the top right of a project chat, the **Local model: status** button and its ▾ arrow
   (**on** / **off**). It prints `● ON` or `○ OFF`. On takes about 20 s and the whole GPU; switch it
   off when you game.
3. Pick the agent per chat: **Claude** (your subscription) or **pi** (the local model, free).
   To have Claude lead and the local model build, say "use the local model" (the `local-worker` skill).
4. Godot: open `game-brilliant/game` in Godot first; then Claude or pi can work in the editor (Godot AI).
5. Change skills or tools only through tack: `tack`, `tack on <name>`, `tack off <name>`, `tack doctor`.

## What lives where

| Thing | Repo (github.com/makimyers) | Notes |
|---|---|---|
| Skills, guard, rules, T3 Code + pi + Claude Code wiring, local model switch | `tack` | the authority; change things here, then `tack install` |
| Avalon fast (the local model server) | `fork-avalon` (`local/`) | `local/avalon-fast up|down|status` |
| ROCm SDK the Avalon build links | `fork-strata` (`.venv`, made by `setup.sh`) | also Strata's own models |
| Model Deck (test bench for other models) | `tool-model-deck` | tray app, autostarts |
| game-studio CLI and Godot verify kit | `tool-claude-setup` | used by the `game-studio` skill |
| multitree / onetree runs | `tool-multitree` | |
| The game (Godot 4.7.2) with the Godot AI add-on | `game-brilliant` | |

## Not in any repo (on purpose)

- Sign-ins: Claude Code (`claude auth login`), GitHub (`gh auth login`, SSH key). Never stored here.
- Claude Code's Godot AI entry: made by the Godot AI panel's **Configure** button (it pins a version and port).
- Claude plugins outside tack: headroom, claude-voice, rust-analyzer-lsp (reinstall with `claude plugin` if wanted).
- Retired on 2026-11-21 and kept only as a local backup in `~/retired-2026-11-21/`: ult (Ultimate
  Harness), dsh (DeepSeek Harness), Hermes, jcode, the old LM Studio AppImages, the godot-claude-skills plugin.
