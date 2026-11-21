# Tools: where every command comes from

Read from upstream on 2026-11-20 (`gh api repos/<o>/<r>/contents/...`, `npm view`, `pip index
versions`, `brew info`). Every command in `manifest/tools.toml` is listed here with the upstream line
it comes from. Piped installers (`curl ... | sh`) are never encoded as steps; they appear only in
`manual`, the one line doctor prints as the fix when no install step can run. `hint` is for notes
after an install (sign in, a manual wiring step).

## pi (base, required)

Upstream: https://github.com/earendil-works/pi. Pinned `@earendil-works/pi-coding-agent@1.6.0`
(npm `latest`, engines `node >=22.19.0`).

- install: `README.md`: "npm install -g --ignore-scripts @earendil-works/pi-coding-agent".
- check: `packages/coding-agent/docs/cli.md` lists `-v`, `--version`. `min_version = "0.84.3"` from
  t3code `docs/user/providers-pi.md`: "T3 Code supports Pi 0.84.3 or newer."
- hint: `README.md`: "run `/login` inside Pi".
- manual: the pi.dev `install.sh` route (piped, so never a step).
- Not confirmed: the exact `pi --version` output text (assumed to contain `N.N.N`).

## t3 (base, required)

Upstream: https://github.com/pingdotgg/t3code. Pinned `t3@0.1.11` (npm `latest`, GitHub release
`v0.1.11`; package has no install scripts).

- install: upstream documents `npx t3@latest` and `npx t3 install` (`docs/user/install.md`), not
  `npm install -g t3`. The npm package declares `"bin": {"t3": "bin/t3.mjs"}`, so a global install
  gives the `t3` command. The brief asked for the npm route; the documented CLI route is
  `curl -fsSL https://t3.codes/install.sh | sh` (piped, not encoded; it is `manual`).
- check: `scripts/npm-launcher/t3.mjs` runs the downloaded runtime with `--version` to validate it.
  On first run the launcher downloads the runtime into `~/.t3/runtime/versions/<ver>/`, so the first
  `check` needs network. Supported: darwin-arm64, linux-arm64, linux-x64 only (no Intel Mac).
- Pi provider: `docs/user/providers-pi.md` step 3: "Open T3 Code Settings, enable Pi, and refresh the
  provider." `docs/user/install.md`: "Open **Settings → Providers** ... and enable the provider you
  want." No CLI. Settings live in `~/.t3/userdata/settings.json` (`docs/user/managed-policy.md`), but
  the key for enabling a provider is not documented, so tack does not write it: clicks are in `hint`.
  A machine-wide `policy.json` exists but needs root; not used.

## claude (base, detect only)

Upstream: https://claude.com/product/claude-code. No install step. From t3code `README.md`:
"install Claude Code and run `claude auth login`": the install half is `manual`, sign-in is `hint`.

## rtk (toggle, default claude + pi)

Upstream: https://github.com/rtk-ai/rtk. Release `v1.2.1` (tag commit `d0f0399a`); Homebrew core
`rtk` stable `1.2.1`.

- install 1: `README.md`: "brew install rtk" (macOS, and Linux with Homebrew). Brew cannot pin.
- install 2: `docs/guide/getting-started/installation.md`: "cargo install --git
  https://github.com/rtk-ai/rtk --branch master rtk". tack swaps `--branch master` for
  `--tag v1.2.1` to pin. Plain `cargo install rtk` installs the unrelated Rust Type Kit.
- check: `installation.md`: "rtk --version   # should print: rtk x.y.z".
- manual: a link to `docs/guide/getting-started/installation.md` (no brew and no cargo).
- enable claude: `rtk init -g --auto-patch`. `README.md`: "rtk init -g # Claude Code";
  `docs/usage/FEATURES.md`: "rtk init -g --auto-patch # Non-interactive (CI/CD)". Without
  `--auto-patch`, init asks before patching `settings.json`. With it, the proxy and delegation
  offers are skipped (`src/hooks/init/mod.rs`). On disk: hook + `settings.json` entry + `RTK.md`.
- enable pi: `rtk init -g --agent pi --auto-patch`. `supported-agents.md`: "Creates ...
  `~/.pi/agent/extensions/rtk.ts`"; "use `--auto-patch` to approve without prompting". That file is
  rtk's pi `marker`.
- disable claude: `installation.md`: "rtk init -g --uninstall # remove hook, RTK.md, and
  settings.json entry".
- disable pi: `supported-agents.md`: "rtk init --uninstall --agent pi --global". Refuses to delete a
  modified extension file.
- Not confirmed: rtk `--auto-patch` also skips the telemetry prompt (telemetry is off by default,
  `docs/guide/resources/telemetry.md`).

## tgrep (toggle, pi only, default off)

Upstream: https://github.com/microsoft/tgrep. Release `v1.2.8`; Homebrew core `tgrep` stable `1.2.8`.

- install: `README.md`: "brew install tgrep" (Linux, macOS). Other documented routes need a clone
  (`cargo install --path tgrep-cli --locked`) or a shell pipeline over release assets, so no argv
  step: on a machine without brew the tool is manual.
- check: clap `version` attribute in `tgrep-cli/src/main.rs`, so `tgrep --version` exists.
- manual: the `v1.2.8` release page (prebuilt assets; no brew).
- Agents: `README.md`: "To install MCP search tools and startup hooks for Codex or pi, run `bash
  install-agent.sh` from this checkout". Claude Code is not supported, so `harnesses = ["pi"]`.
- pi wiring is manual (in `hint`): it runs `scripts/agent/install.py` from a checkout, and argv
  steps cannot name a checkout path. `scripts/agent/README.md`: "./install-agent.sh install --agent
  pi ..." with `--scope user` writing `~/.pi/agent/extensions/tgrep.ts`. Undo: "./install-agent.sh
  uninstall --agent codex,pi ...". So `tack on tgrep` installs the binary and prints the hint;
  `enable`/`disable` are empty.

## context-mode (toggle, default off)

Upstream: https://github.com/mksglu/context-mode. npm `context-mode@2.2.1`, engines
`node >=24.15.0`.

- No install step: each harness installs the plugin itself. `check` is `node --version` with
  `min_version = "24.15.0"` because `README.md`: "The plugin needs Node 24.15.0 or later". This is
  above tack's own Node 22.19 prerequisite. `manual` says to upgrade Node.
- enable claude: `README.md`: "claude plugin marketplace add mksglu/context-mode", "claude plugin
  install context-mode@context-mode". Not pinned (no documented ref syntax).
- disable claude: `README.md` "# remove": "claude plugin marketplace remove context-mode".
- enable pi: `README.md`: "pi install npm:context-mode"; pinned with `@2.2.1` per pi
  `docs/packages.md`: "Versioned npm specifications are pinned."
- disable pi: `README.md`: "pi remove npm:context-mode". tack passes the same pinned source it
  installed.
- Not confirmed: that `marketplace add` exits 0 when the marketplace already exists (re-run of
  `on`); that `pi remove` needs the exact pinned source string.

## skillevaluator (toggle, default off)

Upstream: https://github.com/NVIDIA/SkillEvaluator. PyPI `skillevaluator` 1.0.1 (latest; release
`v1.0.1`).

- install: `README.md`: "uv tool install --python 3.13 skillevaluator", pinned `==1.0.1`. The
  `pip install skillevaluator` route is `manual` only (system pip may refuse a global install).
- check: `src/skillevaluator/cli.py` has `@click.version_option`, so `--version` exists.
- Standalone CLI, not wired into a harness: `harnesses = ["claude", "pi"]`, `default = []`, no
  `enable`/`disable`. `tack on skillevaluator` installs it; `off` only flips state.
