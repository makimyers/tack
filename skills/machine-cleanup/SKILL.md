---
name: machine-cleanup
description: 'Find and clear large/unused files on the user''s Fedora-style machine (Silverblue layout, home on /var/home). Use whenever the user asks to free disk space, find big files, clean caches, or "clear up my computer". Contains the known disk hogs, exact paths, safe delete commands, and the rules for what is recreatable vs real data.'
---

# Machine cleanup playbook

Goal: recover disk space without destroying anything the user values. Home dir is `$HOME` (Silverblue keeps it under /var/home). Disk was 79% used (731G/929G) as of 2026-10-28.

## Ground rules

1. **Never delete without the user approving that specific item.** Scan read-only first, show a table (path, size, what it is, can-it-come-back), then act on their picks.
2. Classify every candidate as **recreatable** (models re-download from HF, caches auto-refill, venvs rebuild, swapfile rebuilds) vs **real data** (Trash = already-deleted data, project folders, session databases, `sessiondata.img`). Only push recreatable items; ask twice for real data.
3. Files under `~/.cache/*` are always safe to delete; the tool re-downloads.
4. `/tmp` is read-only in the agent sandbox — write scratch files to the workspace or `/var/tmp`, not `/tmp`.
5. Root is needed for: the swapfile (root-owned), `/var/lib/flatpak`, `journalctl --vacuum`. Do not run those yourself: hand the user the command to run as root.

## How to rescan (run these, they're the proven route)

```bash
df -h /; du -xh --max-depth=2 $HOME 2>/dev/null | sort -hr | head -50
du -xh --max-depth=1 $HOME/.local/share $HOME/.cache $HOME/.var/app 2>/dev/null | sort -hr | head -30
find $HOME -xdev -type f -size +2G -printf '%s\t%p\n' | sort -nr | head -25 | awk -F'\t' '{printf "%.1fG\t%s\n",$1/1073741824,$2}'
du -sh ~/.local/share/Trash; swapon --show; free -h
flatpak list --app --columns=name,size
podman system df; journalctl --disk-usage; du -sh /var/lib/flatpak
```

## Known disk hogs (snapshot 2026-10-28, sizes will drift)

| Where | Size | What | Recreatable? | Clean command |
|---|---|---|---|---|
| `~/.local/share/Trash/files` | 49G | Already-deleted files (big zips, Windows .exe installers) | No (intentional) | Empty Trash in Files app |
| `~/swapfile` | 48G | Emergency disk swap, root-owned, priority −1, usually 0B used (zram 16G does the real work). Created 2026-08-15 when downloading MiniMax-H3 video models. | Yes, in 30s | as root: `swapoff ~/swapfile && rm ~/swapfile`; rebuild, as root: `fallocate -l 48G ~/swapfile && mkswap ~/swapfile && swapon ~/swapfile` |
| `~/.var/app/ai.lmstudio.lm-studio/.lmstudio/models` | ~86G | GGUF LLMs (each 12–20G). Delete ones not actively run. | Yes — LM Studio re-downloads | Delete dir per model, or use LM Studio UI |
| `~/ComfyUI/models` | ~94G | MiniMax-H3 video + music3 models, stored as *duplicate quants* (GGUF Q5_K_M + w4a8 safetensors of the same model). Keep one per model. | Yes — HF re-download | `rm` the redundant quant files |
| `~/.local/share/Steam` | 34G | Installed games | Yes | Uninstall inside Steam |
| `~/.cache/speech.cpp` | 21G | `speech` CLI (TTS) model cache, Drama-35B gguf | Yes — `speech` re-downloads | `rm -rf ~/.cache/speech.cpp` |
| `~/autodev/.forge` | 21G | z-image-turbo image-gen runtime models | Yes — autodev re-fetches | Delete runtime dir if dormant |
| `~/.config/Claude/vm_bundles` | 20G | Claude Desktop sandbox VM: `rootfs.img` (recreates) + `sessiondata.img` (**may hold user files — check first**) | Partially | Delete rootfs only unless user confirms |
| Project `.venv`s (e.g. `~/YuE`, `~/minimax-music3`) | 15–18G each | PyTorch duplicated per venv | Yes | `rm -rf .venv` then `pip install -r requirements.txt` |
| `~/.codex` | 23G | Session logs (jsonl >30 days old = history only), `visualizations/` | Logs only | Prune `~/.codex/sessions` by mtime |
| `~/.cache` rest | ~11G | uv, playwright, huggingface, codex-runtimes, comgr | Yes, automatic | `uv cache clean`; `rm -rf ~/.cache/<name>` |
| `~/.local/share/opencode/opencode.db` | 6G | OpenCode session history DB | Logs only | Don't `rm` — archive/trim, app may need schema intact |
| Flatpak store `/var/lib/flatpak` | 20G | Apps + orphaned runtimes | Yes | `flatpak uninstall --unused` (as root for the system store) |
| Podman `~/.local/share/containers` | 31G | Mostly *active* images; only stopped containers reclaimable | Containers yes, images user-decides | `podman container prune` (check `podman ps -a` first) |
| Toolchains (Unity/Hub 9G, Android/Sdk 6G, `~/.gradle` 4G, swiftly 3.5G) | ~22G | SDKs, painful to reinstall | Painfully | Keep unless user dropped that stack |

## What NOT to touch

- Anything under `~/Projects` except `node_modules`, `.venv`, build dirs (`dist/`, `.next/`, `Library/` for Godot).
- `~/ComfyUI` workflows, `~/.config` app settings, `~/.dsh`, `~/.claude`.
- The 48G swapfile *while* something big is loading (check `free -h` — if RAM nearly full, keep swap).

## Finish like this

Re-run `df -h /` and report G recovered, plus which items the user declined so a future run doesn't re-ask.
