# Skills: where they came from

The catalogue is `manifest/skills.toml` (schema: `docs/SPEC.md` 3.1). 86 entries:

| Group / source | Entries | On by default |
|---|---|---|
| own (`skills/`) | 9 | 7 both harnesses; `one-thing` Claude only; `web-scrape` off |
| `bank` (own and third-party, see Bank below) | 20 | none |
| `superpowers` (obra/superpowers) | 15 | all, Claude Code only (`tack on superpowers --pi` for pi) |
| `mattpocock` (mattpocock/skills) | 39 | 8, both harnesses (`grill-me`, `grilling`, `to-spec`, `to-tickets`, `triage`, `domain-modeling`, `handoff`, `setup-matt-pocock-skills`); 31 listed, off |
| ayghri/i-have-adhd | 1 | both |
| cloudflare/security-audit-skill | 1 (`security-audit`) | Claude only |
| pbakaus/impeccable | 1 | off |

## Own skills

Ported from `ultimate-harness/toolset/skills/` (the newest copies; the `tool-roadmap-main` and
`tool-multitree-main` copies are older). Goal of every change: no dependency on the `ult` binary,
`~/.ult`, `ULT_*` variables, ult modules, `manifest.toml` or hooks. Each skill needs only `git`, `gh`
and a POSIX shell (`onetree` needs bash).

The old roadmap module (`roadmap-status`, `roadmap-ready`, `roadmap-serve`, Node, ~700 lines plus a web
page) is not carried. Its config `tools/roadmap/roadmap.config.json` is no longer read: the repo comes
from `gh repo view`, the epic label is `epic`, the Next file is `docs/roadmap/next.json`.

**goal-loop**: unchanged.

**onboard**
- `R` from `gh repo view` only; dropped `B=$ULT_HOME/bin` and the `node -p` config read.
- Step 2: `roadmap-status --init` / `roadmap-status` replaced by one `gh api` listing of open `epic`
  issues with their "Where it stands" and open `- [ ]` lines, plus `cat docs/roadmap/next.json`.
- Cut `roadmap-serve --open` (the browser page) and the roadmap URL and "first run: created" parts of
  the confirm-back; description and read-only rule updated to match.
- The paste-ready `/multitree` block is gated on the `multitree` skill being installed, not on
  `~/.ult/bin/multitree`.

**wrap** (`SKILL.md`, `handoff-template.md`)
- `R` from `gh repo view`; dropped `ULT_UNATTENDED=1` as an unattended signal.
- `roadmap-status` calls replaced by the same epic listing as onboard; step 5 is skipped when there is
  no GitHub (was: no roadmap config); the Next file path is fixed at `docs/roadmap/next.json`.
- Removed "never the page" (no page any more).

**capture**
- `R` from `gh repo view`; `roadmap-status` replaced by the epic listing (the old no-config fallback
  listed epic titles only; the new one also shows their open lines).

**breakdown**
- `R` from `gh repo view`; "no roadmap config: stop" became "no GitHub repo: stop".
- Step 1: `roadmap-status` replaced by `gh api` listings of milestones (phases) and epics; epic label
  fixed to `epic`.
- Step 5: `roadmap-ready [--dispatch]` rewritten as plain commands: one `gh api` listing of the
  epic's sub-issues with state, labels, assignees and `Depends on` line; the agent sorts them into
  ready/started/waiting/held/stuck using the old script's rules; dispatch is `gh issue comment` (same
  text as before, minus the hidden marker) plus `gh issue edit --add-label in-progress`. The coding
  agent's login comes from `CLAUDE.md`/`AGENTS.md` or the owner (was `agent.mention` in the config).
  Board (Projects v2) Status handling is cut.
- Removed dashboard mentions.

**multitree**
- `onetree` (190-line bash) brought in as `scripts/onetree`, run as
  `bash <skill folder>/scripts/onetree` (exec bits may not survive embedding). Its agent is now plain
  `claude` instead of `ult run claude --`; comments updated.
- `multi` mode cut: the `multitree` script is a 435-line Python program, over the port limit. Removed
  the mode row, `tasks/NN.md`, `.multitree.json`, wave and conflict notes.
- `/onetree` and `/multitree-here` were ult commands; the table now says `/multitree one` and
  `/multitree here`.

**multitree-prompt**
- Agent described as `claude ...` not `ult run claude -- ...`; says where `onetree` lives.
- Cut the "one task in a `multitree` batch" section and `tasks/NN-RALPH.md` (multi mode is gone).

**one-thing**: unchanged.

**web-scrape**
- `scrapling` no longer from ult's module / `~/.ult/bin`; all CLIs (`yt-dlp`, `moli`, `lightpanda`,
  `scrapling`, `gh`) are listed as prerequisites the user installs; a missing one is skipped.
- `LIGHTPANDA_DISABLE_TELEMETRY` is now an instruction, not a claim that ult set it.
- Removed "all tools are already installed" and the `~/.ult` storage note.

## Third-party skills (fetched at install, pinned)

| Repo | Pinned rev | Paths |
|---|---|---|
| `obra/superpowers` | `22f0b463e7a5e579904dd6211a520321035526f7` | `skills/<name>` |
| `mattpocock/skills` | `abeeaf95660ba9e8b7c753eb38f441ca707a8823` | `skills/{engineering,in-progress,misc,productivity}/<name>` |
| `ayghri/i-have-adhd` | `1ecea1993b7fce257ffbd686c070948b5e07eda6` | `skills/i-have-adhd` |
| `cloudflare/security-audit-skill` | `c1c8a8c1471069fb0e188eeaff69b8e8db6564a8` | `skills/security-audit` |
| `pbakaus/impeccable` | `b49177d32788f2dc585c42831d845dacf1ebed2c` | `.claude/skills/impeccable` |

Names equal each skill's directory and frontmatter name; no collisions, so nothing is prefixed.
Notes:
- mattpocock's `skills/in-progress/*` skills are listed (off) like the rest.
- `i-have-adhd` also ships a `.cursor/` copy; the generic `skills/` one is used.
- impeccable keeps one copy per harness (`.claude/`, `.pi/`, `.agents/`, ...). The manifest allows one
  path, so pi also links the `.claude/` copy (same file list as `.pi/`).
- mattpocock's `code-review` shares its name with a Claude Code built-in; it is off by default.

## Does not fit

Nothing: every listed repo keeps one directory per skill.

## Bank

The `bank` group: skills the old `ult` setup carried, kept here off everywhere (`default = []`). An
off third-party skill is never fetched and nothing is linked, so the bank costs no prompt tokens
until `tack on <name>`. `scripts/check-bank.sh` proves each one fetches and links in a temp HOME.

Own skills were copied from `ultimate-harness/toolset/skills/`. Third-party skills are fetched from
their own repo at the commit the old setup pinned (short SHAs resolved with `gh api`); where the old
setup pinned two commits of one repo, the newer is used for all of that repo's skills.

| Name | Source | Licence | What was changed |
|---|---|---|---|
| `chat-table` | own | (owner) | unchanged |
| `design-md` | own | (owner); library VoltAgent/awesome-design-md MIT | added a step: if `~/.local/share/awesome-design-md` is missing, ask, then `git clone` it (ult no longer sets it up) |
| `machine-cleanup` | own | (owner) | unchanged (written for a Fedora Silverblue machine) |
| `podcast-me` | own | (owner) | dropped the ult voice module route (`~/.local/share/claude-voice`); kept the `mlx_audio` route and the text fallback, with its setup line |
| `speakit` | own | (owner) | unchanged (`mlx_audio`, falls back to macOS `say`) |
| `api-design`, `database-migrations`, `postgres-patterns`, `python-patterns`, `python-testing` | `affaan-m/ECC` @ `b38be6a7f6350964d59328b52f0624e1d1d552d1`, `skills/<name>` | MIT | none (ult had four at the older `4eb71d9`) |
| `deprecation-and-migration`, `observability-and-instrumentation`, `performance-optimization`, `source-driven-development` | `addyosmani/agent-skills` @ `301b02b12bff98e4688e6984af6556802322bbde`, `skills/<name>` | MIT | none (ult had two at the older `1be8e34`) |
| `e2e`, `create-verification-skill` | `e2e-inc/e2e` (was `tester-army/e2e`) @ `a9c3de9f0ca5eca2e112ff596271c5fabad0f0e7`, `skills/<name>` | Apache-2.0 | none; runs the CLI with `npx e2e` |
| `reverse-engineer-anything` | `morluto/rea` @ tag `rea-agents-6.4.0` = `a56f7bb668ab23fec8d33f7af353ac7ed2fe1895`, `.agents/skills/reverse-engineer-anything` | MIT | none; runs the CLI with `npx -y rea-agents@6.4.0`; no MCP server registered |
| `ui-ux-pro-max` | `nextlevelbuilder/ui-ux-pro-max-skill` @ `dd9a1b4cf9f67ca2e326f646d93e101b2778db30`, `.claude/skills/ui-ux-pro-max` | MIT | none. ult had rewritten the script path; upstream writes `${CLAUDE_PLUGIN_ROOT}/.claude/skills/ui-ux-pro-max/scripts/search.py`, which means `scripts/search.py` in the skill's own folder |
| `modern-web-guidance`, `chrome-extensions` | `GoogleChrome/modern-web-guidance` @ `4dd968349e54ca45a05dd5c818fb035c9054b022`, `skills/<name>` | Apache-2.0 | none (ult unpacked the tarball of the same commit); search runs with `npx modern-web-guidance` |

## Left out

| Name | Reason |
|---|---|
| `cheap-research` | Runs on `~/Projects/business-hq/tools/models/run_model.py` (not on this machine) and checks reports with `lr`, an ult module. |
| `research-locally` | Is a front end for `lr`, ult's local-research module, on a local-model route (`ULT_LOCAL_BASE`, swh / Strata). |
| `game-studio` | Every job runs the `game-studio` CLI and Godot kit that only ult's `game-studio` module installed (`$ULT_HOME/...`). |
