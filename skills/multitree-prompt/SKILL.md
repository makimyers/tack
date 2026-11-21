---
name: multitree-prompt
description: Write a loop prompt (RALPH.md) for an unattended `onetree` worktree run. Use when the user asks for a "loop prompt", "ralph prompt", "worktree prompt", "onetree prompt", or says something like "give me the prompt to run this" for work that has a plan on disk.
---

# multitree prompt

`onetree "task"` (`scripts/onetree` in the `multitree` skill's folder) cuts a throwaway git worktree from HEAD under `.worktrees/`, runs an agent
inside it with `"$task"` as the prompt, then on exit squashes the branch into the base
branch as ONE commit named `$task`, removes the worktree, and pushes the trunk (only if it
has an upstream; never `main`, never forced; a failed push exits 1 and prints the command
for a human).

The agent is `claude --permission-mode dontAsk --settings <guard>` by default: anything
not allowed by the guard is refused rather than prompted, Bash runs in an OS sandbox with
no network that can write only the worktree, and `git push` / `merge` / `rebase` /
`gh pr` / MCP tools are denied. With `ONETREE_AGENT` set to another CLI there is no guard -
only that agent's own permission settings apply.

Three facts drive everything below:

1. **The launch string becomes the landed commit message.** So it is a short
   Conventional-Commits-shaped pointer to the prompt file, never the prompt itself.
2. **The worktree is cut from HEAD.** Anything the loop must read has to be *committed*
   first, and it has to be *tracked*. Check `git check-ignore -v <path>` before you decide
   where the prompt lives; if `prompts/` is gitignored, the tracked copy goes in the plan
   directory.
3. **Nobody is watching.** The prompt must forbid asking questions and give every fork a
   written default.

## Procedure

1. **Confirm the plan exists on disk** and is tracked. The prompt is a driver for a plan,
   not a substitute for one. No plan -> write the plan first (see the `multitree` skill,
   step 3), then come back.
2. **Locate the tracked home for the prompt** - normally `<plan-dir>/RALPH.md`.
3. **Write the prompt** with the sections below.
4. **Commit plan + prompt** before handing over the invocation.
5. **Output**: the commit line if not already committed, then the `onetree` invocation. Nothing else.

## Required sections

| # | Section | Must contain |
|---|---|---|
| 0 | Header | Which file this is inside the worktree; the numbered read-order for every iteration (state file FIRST, then plan, then findings, then the repo's CLAUDE.md / AGENTS.md, then the one or two source files whose real content outranks the plan's summary of them); a note that `git branch --show-current` reporting `onetree-<hex>` is correct |
| 1 | Modes | Any global agent modes that are active, pinned at a level, plus the boundaries: a laziness/minimalism mode governs HOW a phase is built never WHETHER, and a terseness mode governs chat prose never artifacts. Omit if none |
| 2 | Goal | What done looks like, checkable against the filesystem |
| 3 | Completion promise | One `<promise>` block, every clause independently verifiable by a command |
| 4 | The 2-4 things most likely to go wrong | Named failure modes, especially any that NO gate in the repo can catch |
| 5 | The loop | One iteration = one phase. Pick first unblocked phase in strict order; re-read the cited lines before editing; build; run the gates and paste exit codes; update the state file; commit |
| 6 | Cross-cutting sweeps | Copy-pasteable greps with the expected result stated ("expect ZERO hits") |
| 7 | Anti-patterns | Decisions already argued, phrased as prohibitions, each with its reason |
| 8 | Stuck handling | 5 attempts -> log + `[stuck]` -> opposite approach -> next INDEPENDENT phase. Include the phase dependency map |
| 9 | Safety | Max iterations; never-run list; which ports are the loop's (`$PORT`) and which are off-limits; shared dirs like `node_modules` are symlinks so no reinstall |
| 10 | The launch contract | The shell reality of the wrapper, plus the consequences below |

## The launch-contract section, always

- `$task` becomes the landed commit message, so the prompt received will be one sentence.
  Say so, or the agent will think context was lost.
- The worktree is cut from HEAD: every file the loop reads must be committed and tracked
  first. Name the failure ("if iteration 1 cannot find the plan directory, that is what
  happened - stop and say so, do not reconstruct the plan from memory").
- `git add -A` runs at exit from the worktree root. Gitignored files are safe; **any new
  untracked artifact the loop leaves behind gets squashed in.** Scratch files go outside
  the worktree or in a gitignored path.
- Nothing prompts, so the never-run list is the last guard (and the only one for non-Claude
  agents).
- **Never ask the user a question.** Every human gate has a default the loop ships; an
  uncovered fork takes the option that changes least, gets recorded as a deviation, and is
  surfaced at the end.
- Do not switch branches, `git worktree remove`, delete the branch, or `git push` - the
  wrapper lands the work and pushes the trunk.
- If the repo root had `node_modules`, it is symlinked into the worktree. Never reinstall,
  never `rm -rf node_modules`.

## Content rules

- ASCII only. No smart quotes, em dashes, or emoji - verify with
  `LC_ALL=C grep -n '[^ -~]' <file>`.
- Every shell command copy-pasteable from a Unix terminal.
- Cite `file:line` as anchors AND say they are anchors, not truth, with an instruction to
  trust the repo and record drift when they disagree.
- State human gates as `H1..Hn` with a shipping default each. A gate with no default is a
  stall.
- Prefer a measured number over an adjective. "3.61:1, below the 4.5 floor" survives;
  "too dark" does not.

## Output discipline

After the file is written:

- ONE line naming the file.
- The fenced `onetree` invocation.
- At most three short lines of consequence, and only for something that changes what the
  user does next.
