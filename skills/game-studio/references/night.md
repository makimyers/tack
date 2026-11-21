# Night runs and model routes

The night runner is the DeepSeek Harness (dsh), headless, one ticket at a time, on the game's own supervisor script.
Done is the game's `verify` command, never the model's word.

## Before each ticket

1. `game-studio lint-ticket <issue>`: a failing ticket gets `blocked` and a "needs spec" comment; skip it.
2. `game-studio route --labels <the ticket's labels>`: prints the ult route (`manifest.toml`) and model id.
   - `routes.code` (default Avalon: fast, 256K context, text only) for code tickets.
   - `routes.vision` (default Strata: reads pictures) when the labels include `needs-vision`.
3. `game-studio route --labels ... --check`: asks the route's `/v1/models` whether that model is loaded. If not,
   refuse to start. Never start, stop or switch a model server: someone may be using it.

## During and after

- Work on a `night/<date>` branch. Never merge it; a person reviews in the morning.
- One GPU job at a time: code, then assets, then vision, never together.
- Logs go to `game-studio.json`'s `night_logs` folder. At the end: `game-studio report`.

## Golden images

`baselines` in `game-studio.json` is the golden-image folder. An agent that replaces a golden screenshot can turn any
visual regression green, so only a human changes them.

- `game-studio guard install`: adds a `commit-msg` rule to the game's hooks (it respects `core.hooksPath` and appends
  to an existing hook). A commit touching the baselines folder is rejected unless its message has a
  `Baseline-approved-by: <name>` line.
- `game-studio guard status`: says whether the rule is installed.
- A failing screenshot gate is reported, never "fixed" by copying the new image over the old.
