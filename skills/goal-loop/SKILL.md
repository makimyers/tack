---
name: goal-loop
description: "Work through a task list to verified completion in ONE session, one task per round, like /goal but done only when the checks pass, never on the model's word. Use when the user types /goal-loop <handoff folder or plan file> [--check \"<command>\"], or asks to run handoff tasks, a plan or a checklist until done in this session. One agent, nothing in parallel. For unattended overnight runs with a fresh context per item, use the multitree skill instead."
---

# goal-loop

Finish a list of tasks in this session, one task per round, and stop only when
every task's own check passes, in one session and one agent, so it suits a local
model that can only run one agent at a time.

## Input

`/goal-loop <target> [--check "<command>"]`

- **A handoff folder** (the `review-handoff` layout): `tasks/NN-*.md`, each with
  a `## Verify` section, plus a `progress.md` ledger. Follow `HANDOFF.md` and,
  if present, `EXECUTOR_PROMPT.md` rules for every task.
- **A plan file** with `- [ ]` items. The check is `--check`, or `./verify.sh`
  when that exists. A check passes when it exits 0 (for `verify.sh`, when its
  last line is `VERIFY exit=0`). The ledger is `progress.md` next to the plan.

No target given: use `handoff/` if it exists, otherwise ask for one.

## Start (the user's turn)

1. Read the ledger. Rows are lines that start with a date. If any row says
   `| blocked |`, stop and report it; change nothing.
2. Count the tasks not yet done: N. If N is 0, say so and stop.
3. Start the goal:
   - **DeepSeek Harness** (you have `create_goal`): call it with the objective
     `Every task in <target> is done: its checks pass and it has a done row in <ledger>. One task per round, rules in the goal-loop skill.`
     and `max_goal_rounds` = 2 × N + 2.
   - **Claude Code** (no goal tool): work through the tasks in this turn. If the
     session may end first, tell the user to type exactly
     `/goal Every task in <target> is done and its checks pass; follow the goal-loop skill, one task at a time.`
4. Do the first round now.

## Each round: one task

1. Take the lowest-numbered task without a `| done |` row (plan: the first
   `- [ ]` item). Read only that task.
2. Do exactly what it says. Do not redesign, and respect its "Do not" list.
3. Run its checks in order and compare the output literally. Close enough is a
   failure.
4. **Pass:** append `<YYYY-MM-DD> | <id> | done | <what the check reported>` to
   the ledger (plan: also change that item to `- [x]`).
5. **Fail:** run the failing check once more. If it fails again, append
   `<YYYY-MM-DD> | <id> | blocked | <command>` and, under "Blocked detail", the
   command and its exact output. Then go to Blocked.
6. DeepSeek Harness: end your turn here; the goal driver starts the next round
   with a fresh step. Claude Code: carry on with the next task.

## Finish

- **All done:** run the last check again: the last task's Verify commands,
  `--check`, or `./verify.sh`. If it passes, mark the goal complete (DeepSeek:
  `get_goal` for the id and revision, then `update_goal` with `complete`) and
  report the tasks done and the numbers the checks gave.
- **Blocked:** DeepSeek's `update_goal` refuses `blocked` until the same problem
  has lasted 3 rounds. In each following round, re-run the failing check
  without editing anything, then call `update_goal` with `blocked` and the exact
  reason once it is accepted. Claude Code: stop and report.

## Rules

- Never change a check, threshold, expected count or test to make it pass.
- Never commit, push, merge or switch branches unless the task says to.
- One goal and one task at a time. Never start subagents or parallel runs.
- Keep each round short: read only the current task and the files it names.
- If your tool calls come back as plain text instead of running, you are on a
  route without tools (for example a `run_code`-only route). Stop and tell the
  user to pick a model route that has `read`, `edit` and `bash`.
