# Morning report

`game-studio report [--date YYYY-MM-DD] [--out FILE]` writes one screen (40 lines at most) per game, default
`reports/<date>.md` in the game. It is built from files and `gh`, never from a model's summary.

Sources, all from `game-studio.json`:

- `night_logs`: the night's log files (first failing line per ticket).
- `verify_json`: the last verify result.
- The `night/<date>` branch: `git log main..night/<date>` (commits and their size).
- Issue labels in `tickets.repo`: `done-local`, `blocked`, and the human label.

A missing source is skipped with one line saying so.

Sections:

| Section | What |
|---|---|
| Ready to review | Issue, commit, verify line |
| Failed | Issue and the first failing line (at most 3 lines each) |
| Waiting on a human | Top 3 human tickets |
| Escalate to Claude | Blocked twice, or a diff over 300 lines (at most 3): each is worth a `game-studio gate-packet` |

Read it top to bottom; act on "Waiting on a human" first, then pick at most three "Escalate" items for review sessions.
