---
name: multitree
description: Turn one document of mixed work into a plan on disk and execute it, in one of two modes. Use when the user hands over a file (or pastes a list) holding several separate pieces of work and wants it split up and done - "batch this", "run all of these", "multitree this", "/multitree", "/multitree here", "/multitree one". The split is identical in both modes; only execution differs. Wraps multitree-prompt (the loop prompt) and the `onetree` script (one worktree).
---

# multitree

One input document -> a plan on disk -> executed. Two modes, one split.

| Mode | Command | Execution | Right when |
|---|---|---|---|
| here | `/multitree here` | sequential, in the current session, commit per tranche | small edits, you want to watch, no orientation tax |
| one | `/multitree one` | ONE worktree, loop over all phases, unattended, `onetree` | many small-to-medium tasks, you want to walk away |

If the user's intent is ambiguous, assume `one` - it is right far more often.

Steps 1-5 are the same in both modes. Only step 7 differs.

## The cost model - read before recommending a mode

Every fresh agent session pays a fixed orientation cost before it does any work: the
repo's CLAUDE.md / AGENTS.md and the docs it points at, plus finding its way around. On a
repo with heavy onboarding docs that cost is large compared with a few-line diff.

So: **isolate work in a worktree when the work is much bigger than the orientation.**
Practically - more than ~15 minutes of real agent work, or the user wants to walk away.
Below that, `here` beats a worktree.

A loop and a worktree are different things and get chosen separately. A loop buys
persistence (keeps going, state in files) and costs nothing extra. A worktree buys
isolation and costs a session. `one` mode is a loop with one isolation; `here` is
neither.

## Read these first, every run

1. The `multitree-prompt` skill - for `one` mode. Do not write loop prompts from memory.
   `onetree` is `scripts/onetree` in this skill's folder (bash); its header lists the env
   settings. Run it as `bash <this skill's folder>/scripts/onetree "<task>"`.
2. The repo's own agent instructions (`CLAUDE.md`, `AGENTS.md`, `CONTRIBUTING.md`) - the
   gates (build / typecheck / test commands), the branch model, anything human-only.

## Procedure

**1. Establish the file inventory before splitting anything.**

```bash
git ls-files | head -400
```

A task naming a path that does not exist is worse than no task. Every path you write comes
from this inventory or is a genuinely new file you are creating. When the input says "make
it like X", find X in the repo and cite it as `file:line` - that reference is what turns a
subjective task into a checkable one.

**2. Copy every referenced asset into the repo and rewrite the links.**

Images pasted from a notes app live outside the repo (`~/Downloads/...`, an `Attachments/`
folder). Copy them to `docs/<slug>/assets/` and rewrite the markdown links to the new
paths. An unattended loop cannot read a clipboard, a chat attachment, a URL behind a
login, or anything outside the worktree.

**3. Split into a plan on disk** at `docs/<slug>/` (prefer an existing convention dir if
the repo has one for this topic):

- `task_plan.md` - the phases, in dependency order, grouped into tranches
- `findings.md` - what you learned from the repo while splitting: the real file paths, the
  reference implementations, the anchors
- `progress.md` - the state file the loop appends to; starts empty with the phase list
- `assets/` - the copied images
- `RALPH.md` - the loop prompt (`one` mode only, step 6)

One phase = one landable commit = one thing reviewable in a sitting. If a paragraph in the
input is nine pieces of work, it is nine phases. If nine bullets are one edit to one
file, they are one phase.

**Group phases into tranches by file ownership, not by input order.** Phases that touch
the same file belong in the same tranche so they land as one coherent diff and cannot
conflict with each other.

Each phase carries:

- a `title` in Conventional Commits shape, <= ~60 chars - it becomes the commit message
- `files:` - every path it is expected to touch
- `verify:` - the command or grep that proves it is done, or `human` if there isn't one
- `ref:` - the committed asset path, when there is a reference image
- the intent, the acceptance criteria, and `file:line` anchors for the reference
  implementation

**4. Set `verify:` honestly, and prefer "match X" over judgement.**

Anything whose acceptance criterion is "reuse the same component or token as X" is
machine-checkable: assert the import, assert the token, assert the old thing is gone.

`human` is correct only when the criterion really is a human eye - a size increase with no
target value, a novel visual treatment, an alignment judgement. For those, prefer the
N-attempts shape: produce 2-3 candidates from the same base and stop for a pick.

**5. Record open questions as gates with shipping defaults, and do not stall on them.**

An unattended loop asks nobody. Every fork gets a written default in `task_plan.md`
(`H1`, `H2`, ...) naming what ships if nobody answers, chosen from what the repo already
does. A gate without a default is a stall. Surface the list to the user when you present
the plan.

**6. Write the loop prompt** (`one` only) by following the `multitree-prompt` skill
against the plan: one `RALPH.md` for the whole plan.

**7. Commit, then execute in the requested mode.**

Commit the plan, the prompts and the assets first. `onetree` cuts worktrees from HEAD, so
anything uncommitted is invisible to the loop and anything untracked it leaves behind gets
squashed into its commit.

Check for a collision before launching `one` mode: if the user has uncommitted
work in the same files the plan targets, the loop starts from a HEAD that lacks it and
will re-solve the same problems differently. Say so and let them commit first.

- **here** - work the tranches in order in this session. Run the repo's gates at the end
  of each tranche, commit, and report. No worktree, no `RALPH.md`, no wave plan.

- **one** - one worktree, unattended:

  ```bash
  bash <this skill's folder>/scripts/onetree "<slug>: execute docs/<slug>/RALPH.md - phases 1-N of task_plan.md"
  ```

  The launch string becomes the landed commit message, so keep it a short pointer.
  `onetree` squashes the whole plan into one commit on the current branch when the loop
  exits, then pushes it (never `main`; `ONETREE_NO_PUSH=1` to skip).

## When no mode was given

If invoked as plain "batch this", do steps 1-5, then recommend a mode from what the split
actually produced and ask for a go. Recommend `one` when the user wants to walk away and
`here` when they want to watch.

Never choose the execution model before seeing the tasks.

## What this pipeline does not do

- **It does not merge to `main`.** Landing goes to the current branch. Shipping is human-only.
- **It does not judge design.** See `verify: human`.
- **It does not do human-only work** - provisioning, migrations against a live DB, auth
  providers, secrets, deploys. Split so the build-verifiable part is a phase and the human
  step is an `H<n>` gate with a default.

## Output discipline

- one line naming the plan directory and the phase count
- the tranche list: phase titles only, grouped, with `human` ones marked
- the `H<n>` gates and their defaults, one line each
- the single command to run, or - in `here` mode - just get on with it

Do not narrate each phase you wrote, restate the frontmatter, or explain the mode table
back to the user.
