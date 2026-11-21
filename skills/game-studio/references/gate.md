# Claude review gates

Claude (Opus) time is scarce, so it judges; local models execute.

## The weekly pattern

1. Monday: plan the week (tickets written to `references/tickets.md`, each passing `game-studio lint-ticket`).
2. Tuesday to Thursday: up to three review gates, one packet each.
3. Friday: design review (play the build, then re-spec or cut).
4. Every other job runs locally at night; done is the game's verify command.
5. A gate ends in one answer: approve, re-spec or fix.

## Using a packet

`game-studio gate-packet <branch> [--issue N] [--out FILE]` writes one file (default
`docs/game-studio/gate/<branch>.md` in the game) holding everything the review needs:

- `## Ask`: the decision wanted (approve, re-spec or fix).
- `## Ticket`: the issue body.
- `## Verify`: the last verify result.
- `## Diff stat` and `## Diff` (cut at 400 lines, with a note of what was cut).
- `## Screenshots`: paths of changed images, to open if the change is visual.

Open the packet, read it once, answer the Ask. Explore the repo only when the packet cannot answer a question, and say
which question. The packet never holds `.env` files, keys, or anything outside the game's repo.
