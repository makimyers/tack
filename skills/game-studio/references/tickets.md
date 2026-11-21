# Tickets

A local model succeeds when the ticket is precise. Every agent ticket is a GitHub issue in `game-studio.json`'s
`tickets.repo` with the agent label (`tickets.agent_label`), one `verify:*` label and these six sections, each
non-empty:

```markdown
## Goal
One or two sentences: what the player or the game gets.

## Files
- `path/to/file.gd`: what changes there. Name every file the agent may touch.

## Acceptance
- Observable results, each one checkable (a number, a line of output, a scene that loads).

## Verify
```bash
./tools/verify/all.sh
```
The command(s) whose exit code decides done, and what they print on a pass.

## Out of scope
What the agent must not touch or "improve" (other systems, baselines, thresholds).

## Blocked by
Issue numbers this waits on, or "none".
```

Labels:

- `verify:<gate>` (e.g. `verify:unit`, `verify:smoke`, `verify:shots`): the gate that proves the ticket. Exactly one is enough.
- `needs-vision`: the ticket needs pictures, so `game-studio route` sends it to `routes.vision`.
- The human label (`tickets.human_label`, e.g. `human-only`): no agent touches it.

Check before a run: `game-studio lint-ticket <issue>` (or `--fixture FILE.json` for a saved `gh issue view --json`
result). Exit 1 lists each missing piece; the night run marks such a ticket `blocked` with a "needs spec" comment.

Writing tips: one behaviour per ticket; name the test that should go from red to green; give numbers, not adjectives
("jump apex 3.2 tiles", not "jump feels better").
