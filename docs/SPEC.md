# tack — spec

`tack` is one Rust binary that installs and manages a coding-agent setup on Apple Silicon Macs and
Linux (x86_64, aarch64). Intel Macs are not supported: T3 Code ships no runtime for them.

- **View:** T3 Code (`t3`), one window for every agent.
- **Harnesses:** pi (any model, local models included) and Claude Code (Claude models).
- **Shared kit:** skills, a guard, and a few token-saving tools, each switchable per harness.

Owner's goals, in order: one command to install; trivially easy to run and manage; lean (nothing
always-on that a small local model must read unless it earns its place); safe to re-run.

Non-goals for v1: Windows, Codex/dsh/OpenCode wiring, a TUI or desktop app, model/provider
configuration (the owner sets pi's models himself), owning CLAUDE.md / AGENTS.md beyond the
`rules` block (see 3).

## 1. Commands

| Command | Does |
|---|---|
| `tack` | Same as `tack list`, plus one "next step" line. |
| `tack install [--skip-tools]` | Idempotent. Copies itself to `$TACK_HOME/bin/tack`, links it into `~/.local/bin`, checks prerequisites, installs base tools, sets T3 Code's provider defaults (below), fetches the git skill sources that have a skill on, applies every component's on/off state (defaults for components not yet in state), and removes tack's links for skills that are off or no longer in the catalogue. A tool's enable steps run only when it is on and not yet wired (see 3.3). `--skip-tools` skips every `[[tool]]` install/enable step (used by tests and for a skills-only run). |
| `tack list [--all]` | Table: name, Claude on/off, pi on/off, short description. Groups collapsed to one row with a count unless `--all`. Then one line of base tools with versions. Before the first install it starts with `not installed yet; this is what tack install will turn on`. |
| `tack on <name>... [--claude] [--pi]` | Turn a component or a whole group on. No flag = every harness the component supports. |
| `tack off <name>... [--claude] [--pi]` | Reverse of `on`. A tool's disable steps run only if tack wired it; otherwise only the state flips. |
| `tack doctor [--json]` | Checks prerequisites, base tools, links, hook, extension. Each failure prints the cause and one `fix:` line. Exit 1 if any check fails. |
| `tack open` | Starts T3 Code (`t3`). |
| `tack uninstall` | Refuses (exit 2) unless `$TACK_HOME` holds `bin/tack` or `state.toml`. Runs no tool `disable` steps: prints one line naming the tools still wired and `tack off <names>` to unwire them first. Then removes every link, the Claude hook and the pi extension that tack created, and `$TACK_HOME`. Never uninstalls pi, T3 Code, Claude Code or other tools; prints that they stay. |
| `tack guard --harness <claude\|pi>` | Hidden. Hook entry point: reads one tool call as JSON on stdin, exits 0 (allow) or 2 (deny, reason on stderr). |

T3 Code provider defaults (also with `--skip-tools`): in `$T3CODE_HOME/userdata/settings.json`
(created with its folders if missing), add `providerInstances.codex` =
`{"driver": "codex", "enabled": false}` and `providerInstances.pi` =
`{"driver": "pi", "enabled": true, "config": {"launchArgs": "--exclude-tools mcp__t3-code__*,mcp__t3_code__*"}}`
(the launch argument keeps T3 Code's own tools, about 16,000 prompt tokens, out of pi sessions),
each only when no entry has that id or that driver; every other key, entry and the key order stay. Atomic
write that keeps the file mode. Prints `t3: Codex off, pi on (restart T3 Code to apply)` naming only
what changed, nothing when nothing did. Not valid JSON, or `providerInstances` not an object: prints
`warn: <path> is not valid T3 Code settings; left alone` and install carries on. `uninstall` leaves
the file alone.

Exit codes: 0 ok, 1 a check or step failed, 2 usage error or missing prerequisite.
A broken `state.toml` prints the cause and `fix: mv <path> <path>.bak && tack install`, exit 1;
`doctor` reports it as a failed check and `uninstall` still runs.
Output: plain short lines, no colour codes when stdout is not a terminal. Every failure names the cause
and one command that fixes it.

Prerequisites (checked by `install` and `doctor`, never installed by tack): `git`, Node.js ≥ 22.19
with `npm`. If one is missing, print the exact command for this OS and exit 2.

## 2. Layout on disk

All paths derive from `$HOME`, so tests run against a temp home.

| Path | What |
|---|---|
| `$TACK_HOME` (default `~/.tack`) | Everything tack owns. |
| `$TACK_HOME/bin/tack` | The installed binary. Hooks and the pi extension call this absolute path. |
| `$TACK_HOME/store/<id>-<rev12>/` | One checkout per git skill source at its pinned commit. `<id>` is the source URL as a slug, at most its last 60 characters. |
| `$TACK_HOME/store/own/` | Skills embedded in the binary, written out on install. |
| `$TACK_HOME/state.toml` | On/off per component per harness, and which tools tack wired. |
| `$TACK_HOME/backup/` | Copy of any file tack edits, taken before the first edit. |
| `~/.claude/skills/<name>` | Symlink into the store when the skill is on for Claude Code. (`$CLAUDE_CONFIG_DIR` overrides `~/.claude`.) |
| `~/.pi/agent/skills/<name>` | Symlink into the store when the skill is on for pi. (A pi folder moved with `$PI_CODING_AGENT_DIR` is not supported.) |
| `~/.claude/settings.json` | Gets one PreToolUse hook entry that runs `tack guard --harness claude`. |
| `~/.t3/userdata/settings.json` | T3 Code settings; install adds the Codex-off and pi-on provider entries (see 1). (`$T3CODE_HOME` overrides `~/.t3`.) |
| `~/.pi/agent/extensions/tack-guard.ts` | pi extension that calls `tack guard --harness pi` on every tool call. |
| `~/.claude/CLAUDE.md`, `~/.pi/agent/AGENTS.md` | Get one `rules` block each (see 3). |
| `~/.pi/agent/extensions/tack-rules.ts` | pi extension that adds a short rules reminder before each run, while `rules` is on for pi. |

Ownership rule: tack only ever removes or replaces a symlink whose target is inside `$TACK_HOME`, a
hook entry whose command contains `tack guard`, the extension files it wrote, and the text between
its rules markers. Anything else with
the same name is left alone and shown as `exists (not tack's)` in `list` and `doctor`.

## 3. Components

Three kinds. All share one namespace and one on/off state shape.

1. **skill** — a directory containing `SKILL.md`. On = a symlink in that harness's skills dir.
2. **tool** — an external program installed and wired by argv commands from the manifest.
3. **builtin** — `guard` (hook + extension, implemented in Rust) and `rules`.

`rules` writes one block, `<!-- tack:rules:start -->`, the text of `assets/rules.md` (embedded),
`<!-- tack:rules:end -->`, into `~/.claude/CLAUDE.md` and `~/.pi/agent/AGENTS.md`. On replaces the
block between the markers, or appends it after one blank line (creating the file). Off and
`uninstall` remove the block and that blank line, and delete the file if only whitespace is left.
`install` rewrites the block when on. Text outside the markers is never changed. Missing, unpaired
or repeated markers: `warn: <path> has a damaged tack rules block; left alone`. `doctor` warns
(fix: `tack install`) when the block is missing or out of date.

For pi, `rules` also writes `assets/pi/tack-rules.ts` (embedded) to
`~/.pi/agent/extensions/tack-rules.ts`, like the guard extension: install rewrites it, off and
`uninstall` remove it, `doctor` warns when it is missing or differs. Claude Code gets the block only.

The manifest is embedded in the binary from `manifest/skills.toml` and `manifest/tools.toml`
(`include_str!`). An optional `$TACK_HOME/tack.toml` with the same schema adds or overrides entries by
name. Own skills live in `skills/` in the repo and are embedded with `include_dir`.

### 3.1 `manifest/skills.toml`

```toml
[[skill]]
name = "goal-loop"              # unique; also the link name
description = "Keeps working until the checks pass"   # <= 60 chars, shown in `tack list`
source = "own"                  # "own" (embedded under skills/) or an https git URL
path = "goal-loop"              # dir that holds SKILL.md: relative to skills/ (own) or the repo root (git)
rev = ""                        # full 40-char commit SHA; required when source is a URL
harnesses = ["claude", "pi"]    # where it may be turned on
default = ["claude", "pi"]      # where it is on after a first install; [] = off
group = ""                      # optional; `tack on <group>` toggles every member
```

### 3.2 `manifest/tools.toml`

```toml
[[tool]]
name = "rtk"
description = "Shrinks command output before the model reads it"
harnesses = ["claude", "pi"]    # [] = base tool: installed, never toggled (pi, t3, claude)
default = ["claude", "pi"]      # for base tools: [] and `base = true`
base = false                    # true = part of the base line in `list`; cannot be turned off
required = false                # true = `tack install` exits 1 if it cannot be installed
check = ["rtk", "--version"]    # argv; exit 0 = present
min_version = ""                # optional "N.N.N"; compared with the first N.N.N in check's stdout
hint = ""                       # optional; one line printed after tack installs the tool
manual = ""                     # optional; one line, a command or URL, to install it by hand

[[tool.install]]                # first step whose `os` and `needs` hold is used; no match = manual
os = "any"                      # "any" | "macos" | "linux"
needs = ["brew"]                # commands that must be on PATH
run = [["brew", "install", "rtk"]]   # argv lists, run in order, no shell

[tool.enable]                   # optional, per harness: argv lists run by `on` (and `install`, once)
claude = [["rtk", "init", "-g"]]
pi = [["rtk", "init", "-g", "--agent", "pi"]]

[tool.disable]                  # optional, per harness: argv lists run by `off`, only if wired
claude = []
pi = []

[tool.marker]                   # optional, per harness: a file the enable steps create, relative
pi = "extensions/rtk.ts"        # to that harness's config folder (~/.claude, ~/.pi/agent)
```

Commands are argv arrays and are never passed through a shell. When no `install` step can run and
`check` fails, `doctor` prints `manual` as the `fix:` line (this is how Claude Code is handled:
detect, never install, tell the owner how to install and sign in).

### 3.3 `state.toml`

```toml
[enabled.goal-loop]
claude = true
pi = true
```

`install` adds entries only for components missing from state (using `default`); it never flips an
existing entry. `on`/`off` update state, then apply.

```toml
[wired.rtk]
claude = true
```

`wired` records the tools whose enable steps tack ran for a harness: set after enable steps succeed,
cleared after disable steps succeed. `install` runs a tool's enable steps only when it is on and not
yet wired (a wired tool only gets its program checked); `on` always runs them; `off` runs disable
steps only when wired. A tool with no enable steps for a harness is never wired.

A declared `marker` that is missing makes a wired tool count as not wired for that harness:
`install` runs its enable steps again and `doctor` warns (`fix: tack install`). Tools without a
marker rely on `wired` alone. A marker that is absolute or contains `..` fails manifest loading.

### 3.4 The catalogue (what ships)

On by default: `guard`, `rules`, `goal-loop`, `onboard`, `wrap`, `capture`, `breakdown`,
`multitree`, `i-have-adhd`, the default members of the `mattpocock` group, `rtk`.
Claude Code only, on: `one-thing`, `security-audit`, the `superpowers` group (pi:
`tack on superpowers --pi`).
Off until toggled: `context-mode`, `web-scrape`, `impeccable`, `skillevaluator`, `tgrep`.
Base tools: `pi` (required, ≥ 0.84.3), `t3` (required), `claude` (detect only).

## 4. Guard

Pattern-matching deny list, not a sandbox (say so in the README). Rules live in `src/guard.rs`.

Deny when a tool call:

1. Reads, writes or lists a credential path: `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.config/gh`,
   `~/.netrc`, `~/.npmrc`, `~/.docker/config.json`, `~/.kube/config`, harness credential files
   (`~/.claude/.credentials.json`, `~/.pi/agent/auth.json`, `~/.codex/auth.json`), or a `.env` /
   `.env.*` file (allowed: `.env.example`, `.env.sample`, `.env.template`). On macOS these compare
   case-insensitively (`~/.SSH`, `.ENV`). Glob `pattern` and Grep `glob` resolve against the
   tool's `path` (else the cwd); Grep `pattern` is a regex and is not checked.
2. Writes (any file tool other than read / glob / grep / ls / find) under `$TACK_HOME` (else
   `~/.tack`) or to the pi extension `~/.pi/agent/extensions/tack-guard.ts`. Reading them is allowed.
3. Runs a shell command that names any path from 1 or 2 (except running the tack binary itself as
   the command word), reads the macOS keychain (`security find-generic-password`,
   `find-internet-password`, `dump-keychain`), force-pushes (`git push` with `--force`, `-f`,
   `--force-with-lease` or a `+refspec`), or runs `rm -r` on `/`, `~` or `$HOME`.
   - Leading `VAR=x` words and wrappers (`sudo`, `doas`, `env`, `command`, `exec`, `nohup`, `time`,
     `nice`, `timeout`, `xargs`, `rtk`; rtk's runners `proxy`, `bypass`, `run`, `err`, `summary`, `test` are skipped like wrappers;
     after `rtk` every tail of the command is checked as a command, and rtk's shell strings (the
     value of `-c` / `--command` / `--flag=value`, and a lone word after a runner) are checked as
     command lines; text arguments of the wrapped command are left alone. Program names are
     compared without case on macOS) with their flags (and the values of sudo's `--user`, `--group`,
     `--host`, `--prompt`, `--chdir`) are skipped; for git, global options and the values of `-C`,
     `-c`, `--git-dir`, `--work-tree`, `--namespace`.
   - `eval`, `sh -c`, `env -S` / `--split-string`, every heredoc or here-string body (whoever reads
     it), `((...))`, and `$(...)` / backticks (also inside double quotes and unquoted heredocs) are
     checked as commands, nested at most 8 deep; deeper nesting is denied. `#` at a word start
     comments out the rest of the line.
   - Env-file names (not credential or tack paths) are text in `echo` / `printf` arguments (unless a
     later segment of the same pipeline runs `xargs` or a shell), heredoc bodies not fed to a shell,
     the grep / rg pattern, and `git rm --cached`, `git check-ignore`, `git ls-files`. Redirect
     targets are always paths.

Out of reach (pattern matching, not a sandbox): paths built at run time (`cat $DIR/id_rsa`,
`cd ~; cat .ssh/id_rsa`), variables other than `$HOME` and `$TACK_HOME`, globs that expand to a
protected path, encoded or obfuscated strings, scripts written to disk and run later, interpreters
(`python -c`), removing a parent of a protected path (`rm -rf ~/.pi`), and programs that read a
secret by themselves. `~/.claude/settings.json` is deliberately not protected (the owner edits it
through the agent), so an agent can remove the Claude hook.

Input shapes: Claude Code PreToolUse JSON (`tool_name`, `tool_input` with `command`, `file_path`,
`path`, `pattern`, `glob`, `notebook_path`); pi sends `{ "tool": ..., "input": {...} }` from the
extension. Unparseable stdin = deny (fail closed). Resolve `~`, `$HOME`, `$TACK_HOME` and relative
`..` before matching. Exit 0 allow; exit 2 deny with `Blocked by tack guard: <reason>` on stderr.
The Claude hook command is `<tack> guard --harness claude || exit 2` (hooks run through bash), so a
missing binary or crash also blocks; the pi extension blocks on any non-zero exit or spawn error.
A hook that times out still lets the call through in Claude Code.

## 5. Code layout and who owns what

Single crate, binary `tack`. Dependencies are fixed in `Cargo.toml`; add one only if you must, and say
so in your report.

| Stream | Branch / worktree | Owns (nobody else edits these) |
|---|---|---|
| core | `feat/core` | `src/main.rs`, `src/manifest.rs`, `src/state.rs`, `src/store.rs`, `src/link.rs`, `src/run.rs`, `src/components.rs`, `src/doctor.rs`, `tests/` |
| guard | `feat/guard` | `src/guard.rs`, `src/claude.rs`, `src/pi.rs`, `assets/pi/tack-guard.ts` |
| skills | `feat/skills` | `manifest/skills.toml`, `skills/**`, `docs/SKILLS.md` |
| tools | `feat/tools` | `manifest/tools.toml`, `docs/TOOLS.md` |
| ship | `feat/ship` | `install.sh`, `Install.command`, `.github/workflows/release.yml`, `verify.sh`, `README.md`, `LICENSE`, `scripts/**` |

`src/paths.rs` is written and frozen: use it, do not change it. If you need something it lacks, say so
in your report.

Interfaces the core stream calls (the guard stream implements them; stubs exist in the skeleton):

```rust
// src/guard.rs
pub fn run(harness: Harness, stdin: &str, home: &Path) -> (i32, String);   // (exit code, stderr text)
// src/claude.rs
pub fn install_hook(paths: &Paths) -> anyhow::Result<bool>;   // Ok(true) = file changed
pub fn remove_hook(paths: &Paths) -> anyhow::Result<bool>;
pub fn hook_installed(paths: &Paths) -> bool;
// src/pi.rs
pub fn install_extension(paths: &Paths) -> anyhow::Result<bool>;
pub fn remove_extension(paths: &Paths) -> anyhow::Result<bool>;
pub fn extension_installed(paths: &Paths) -> bool;
```

## 6. Working rules for every build agent

- Work only inside your own worktree, with absolute paths. Do not edit files another stream owns.
- The older harness repo this project replaced is **read-only reference**. Never edit it.
- Never write to the real `~/.claude`, `~/.pi`, `~/.tack`, or `~/.local/bin`. Tests use a temp `HOME`.
- Install nothing on this machine: no `npm i -g`, `brew install`, `cargo install`, `pip install`,
  `curl | sh`. To learn how an upstream tool is installed, read its README
  (`gh api repos/<owner>/<repo>/readme -H "Accept: application/vnd.github.raw"`). Plain `curl` is
  blocked by a hook here; use `gh api` or the context-mode fetch tools.
- `cargo test` must pass with no network. Tests that need external commands use tiny shim scripts on a
  temp `PATH`; tests that need a git source create a local repo and use its path as the URL.
- If a command is refused with "Blocked by the ult guard" or similar, that is final. Do not route
  around it. Keep secret-looking fixture strings (such as credential paths) in Rust test source, not
  in shell command lines.
- Smallest complete change. No options, abstractions or hardening this spec does not ask for; note
  ideas as follow-ups in your report. Comment only non-obvious "why". Mark a deliberate shortcut with
  a `shortcut:` comment.
- Before you finish: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  (ship/skills/tools streams: the checks named in your brief).
- Commit on your branch in your worktree. Conventional Commits subject (≤ 60 chars). **No
  `Co-Authored-By` line and no AI attribution.** Never push, never merge, never switch branches.
- Final report, under 300 words: what exists now, deviations from this spec and why, open risks,
  and a last line `Verified: <command> -> <result>` naming a command you ran after your last change.
