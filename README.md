# tack

One binary that installs and manages a coding-agent setup on Apple Silicon Macs and Linux (x86_64,
aarch64). Intel Macs are not supported: T3 Code ships no runtime for them.
T3 Code is the window, pi and Claude Code are the agents, and a shared kit of skills, a guard and a
few token-saving tools can be switched on or off per agent.

## Install

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/makidatacity/tack/main/install.sh | sh
```

Or, from a clone, double-click `Install.command` in Finder (or run `./install.sh`). From a clone with
Rust installed it builds tack itself; otherwise it downloads the release binary and checks its
SHA-256 before running it.

You need `git` and Node.js 22.19 or newer. tack checks for both and prints the command to install
whatever is missing. Re-running the install is safe: it never flips a choice you made.

## Everyday commands

`tack` shows what is on, per agent, then the next step:

```
name             claude pi     description
guard            on     on     Blocks secret reads, force-push and rm -rf ~
rules            on     on     Always-on reply rules: short, plain, goal-first
goal-loop        on     on     Keeps working a task list until the checks pass
...
superpowers      on     off    group of 15 skills
mattpocock       8/39   8/39   group of 39 skills
...
context-mode     off    off    Keeps big tool output out of the context window
base: pi 0.80.3 (needs 0.84.3+), t3 missing, claude 2.1.327
next: tack doctor
```

Before the first install it starts with `not installed yet; this is what tack install will turn on`.
`tack list --all` shows each group member on its own row.

`tack on` / `tack off` switch a component or a whole group. With no flag it applies to every agent
the component supports:

```
$ tack off goal-loop --pi
goal-loop: off for pi
$ tack on goal-loop --pi
goal-loop: on for pi
```

`tack doctor` prints only problems, each with one `fix:` line, then a count (exit 1 if anything
fails). `tack doctor --json` gives every check as JSON.

```
fail: pi: pi 0.80.3 (needs 0.84.3+)
  fix: tack install
fail: t3: t3 missing
  fix: tack install
warn: rtk: rtk missing
  fix: tack install
doctor: 56 ok, 1 warn, 2 fail
```

`tack open` starts T3 Code.

## On by default

- Both agents: `guard`, `rules`, `goal-loop`, `onboard`, `wrap`, `capture`, `breakdown`,
  `multitree`, `i-have-adhd`, the default members of the `mattpocock` group, `rtk`.
- Claude Code only: `one-thing`, `security-audit`, the `superpowers` group (it adds
  about 1,200 prompt tokens in pi; `tack on superpowers --pi` turns it on there).
- Off until you turn them on: `context-mode`, `web-scrape`, `impeccable`, `skillevaluator`, `tgrep`
  (pi only; installed through Homebrew, wiring it into pi is a manual step that `tack on tgrep`
  prints).
- The `bank` group: 20 extra skills (API and database design, Python, e2e tests, UI/UX, read-aloud,
  ...), off everywhere and never fetched while off, so they cost nothing. `tack list --all` names
  them; `tack on <name>` switches one on.
- T3 Code shows Claude Code and pi; Codex is switched off. pi sessions started from T3 Code run
  without T3 Code's own tools (previews, issues, starting other threads): they add about 16,000
  prompt tokens, too much for a local model. To get them back, clear pi's "Launch arguments" in
  T3 Code's provider settings.
- Base tools: `pi`, `t3` (installed by tack), `claude` (detected only; install it and sign in
  yourself).

## A local model in pi

Name one local model in `~/.tack/tack.toml` and tack makes it pi's default and starts it:

```toml
[local_model]
name = "Qwen3.8 Flash-Next · Avalon fast"
up = ["/path/to/avalon-fast", "up"]   # starts the server and returns; safe when it is already up
base_url = "http://127.0.0.1:8080/v1"
model = "qwen3.8-flash-next-avalon"   # the id the server answers with on /v1/models
context = 131072
```

`tack install` adds a `tack-local` provider to `~/.pi/agent/models.json` and makes it pi's default
in `~/.pi/agent/settings.json` (other providers and settings stay; each file is copied to
`~/.tack/backup/` before tack's first edit). `tack open` runs `up` in the background, logging to
`~/.tack/local-model.log`, then starts T3 Code, whose pi provider uses that default.

Without a `[local_model]`, set pi's model yourself. For LM Studio, add a provider with
`"baseUrl": "http://127.0.0.1:1234/v1"` and the id `lms ls` prints, and set the model's context
length in LM Studio to at least 32,000: LM Studio's own default (8,192 seen) runs out after a few
steps. With tack's defaults pi sends about 3,000 tokens before your first message.

## How toggling works

A skill that is on is a symlink from `~/.claude/skills/<name>` or `~/.pi/agent/skills/<name>` into
`~/.tack/store/`. Off removes the link; your choices live in `~/.tack/state.toml`.

tack only removes links that point into `~/.tack`. A file or link of the same name that is not
tack's is left alone and shown as `exists (not tack's)`.

## The guard

The guard runs before every tool call in Claude Code (a PreToolUse hook in
`~/.claude/settings.json`) and in pi (`~/.pi/agent/extensions/tack-guard.ts`). It blocks:

- reading, writing or listing credential paths (`~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.config/gh`,
  `~/.netrc`, `~/.npmrc`, Docker and kube configs, agent login files) and `.env` files
  (`.env.example`, `.env.sample` and `.env.template` are allowed);
- shell commands that name those paths, read the macOS keychain, force-push, or `rm -rf` `/`, `~`
  or `$HOME`;
- writes to tack's own files (`~/.tack` and the pi extension), so an agent cannot switch the guard
  off there. `~/.claude/settings.json` is not protected, because you edit it through the agent.

If the tack binary is missing or crashes, both agents block the call rather than let it through.

It is a pattern-matching deny list, not a sandbox. A command that hides a path (built from
variables, encoded, or run through a script file) gets past it. Treat it as a seatbelt against
mistakes, not a defence against a hostile model.

## Always-on rules

`rules` puts one marked block with the text of `assets/rules.md` into `~/.claude/CLAUDE.md` and
`~/.pi/agent/AGENTS.md`, so both agents reply the same way in every session. Your own text in those
files is never touched; `tack install` refreshes the block. Switch it off with `tack off rules`
(or `tack off rules --pi` for one agent). In pi it also installs `extensions/tack-rules.ts`, which
adds a short reminder before each run, since a local model follows AGENTS.md alone only half the time.

## Uninstall

```
$ tack uninstall
removed 63 skill links, the guard hook and extension, the rules blocks, and ~/.tack
pi, T3 Code, Claude Code and other tools stay installed
```

Uninstall never undoes tool wiring (such as rtk's hook), since that could strip setup you did
yourself. If tack wired a tool, uninstall names it first; run `tack off <names>` before uninstalling
to unwire them.

## Publish a release

1. Build the binaries into `dist/`: `cargo build --release`, copy `target/release/tack` to
   `dist/tack-macos-arm64`, run `scripts/build-linux.sh`, then write `dist/SHA256SUMS` with
   `shasum -a 256`.
2. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. Publish: `gh release create vX.Y.Z dist/tack-* dist/SHA256SUMS`. The curl one-liner above
   always installs the latest release.

## Development

- `./verify.sh` runs fmt, clippy, tests and shellcheck. `VERIFY_ONLINE=1 ./verify.sh` also does an
  install / toggle / uninstall smoke run against a temp `HOME`.
- `scripts/build-linux.sh` cross-builds the Linux binaries into `dist/` with rustup's `rust-lld`.
- `scripts/test-linux.sh` runs the tests and an install in a Linux container (podman or docker).

MIT licensed.
