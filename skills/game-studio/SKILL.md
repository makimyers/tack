---
name: game-studio
description: "Game studio mode for this game folder (it has game-studio.json). Use for any work on the game: tickets, verify, night runs and model routes, the morning report, review packets, asset licences, golden images, Godot gates. Routes each job to a game-studio subcommand or a reference file; the game's verify command is the only definition of done."
---

# game-studio

Read `game-studio.json` in the repo root first: it names this game's verify command, ticket repo, night logs, golden
images, licence files and model routes. Nothing game-specific lives in this skill. Run `game-studio --help` for every
subcommand. If `game-studio` is not on PATH, it is `~/Projects/tool-claude-setup/scripts/game-studio/game-studio`;
the Godot kit it copies is in `~/Projects/tool-claude-setup/templates/game-studio/godot`.

## Daytime Game Mode

Open the game folder in T3 Code (Claude for judgement and review; pi on the local model, or the `local-worker` skill, for bulk code). Start in the desired worktree. Use its verifier. The owner starts the playtest and capture, then shares the local session.md. Use the `playcap-review` skill to review the remarks and their pictures. The owner judges feel.

## Jobs

| Job | Do this | Read |
|---|---|---|
| Is it done? | Run the game's `verify` command. Its exit code is the only definition of done. | `references/godot.md` (Godot gates) |
| Write or check a ticket | `game-studio lint-ticket <issue>` before any agent works on it | `references/tickets.md` |
| Night run, which model | `game-studio route --labels a,b` (and `--check` before starting) | `references/night.md` |
| Morning report | `game-studio report` | `references/report.md` |
| Claude review gate | `game-studio gate-packet <branch> --issue N`, then review that one file | `references/gate.md` |
| Does the player see it? (visual change) | Capture matched before/after player views, write a packet, have one fresh critic review it, at most two rounds | `references/visual-critic.md` |
| Asset licences | `game-studio licences` (unknown generator = FAIL) | `references/licences.md` |
| Golden images | `game-studio guard status`; only a human re-baselines | `references/night.md` |
| Game feel, juice, UI | Turn on `gamedev-mode` for that session only and use its `game-feel` and `game-ui-ux` skills | |

## Rules

- Deterministic checks beat model judgment. A model saying "looks good" is not a pass.
- Pictures cost time: tune with printed numbers, render once per change on the GPU, never on llvmpipe (`references/godot.md`, "Fast pictures"). Agents do not play the game; the owner does.
- Agents never re-baseline golden images: a commit touching the baselines folder needs a `Baseline-approved-by: <name>` line written by a human.
- One GPU job at a time. Never start, stop or switch a model server; if the route's model is not loaded, stop and say so.
- Local models first (code tickets on `routes.code`, picture tickets on `routes.vision`); hosted models only for text work.
- No global MCP servers. A runtime or editor MCP is for a playtest or daytime session, never the night run.
