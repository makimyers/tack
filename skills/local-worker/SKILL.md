---
name: local-worker
description: Hand coding work to the owner's local model (pi on the tack local model) and check it cheaply, so the local model does the typing AND the testing and Claude spends as few steps as possible. Use for work with a clear done-check (a feature from a spec, tests, a pattern across files, boilerplate, a small game or tool), when the user asks to "use the local model / Qwen / pi", or to save Claude usage. Not for design decisions, unclear specs or anything outward-facing.
---

You are the lead; the local model is a fast, free, weaker worker. It writes the code AND proves it
works; you write the brief and judge the proof. Every tool call you make costs the owner's Claude
usage, the worker's cost nothing, so push the work down.

## Budget (the reason this skill exists)

The 2026-11-21 Orb Drift test took Claude 99 steps for a 239-line game: 21 browser probes, 18 shell
commands, 10 hand-offs. Claude writing it alone would have cost less. Aim for this instead:

- **Big tasks, few hand-offs.** One brief per feature, not per function: a small game or tool is 1–3
  briefs, not 5. The worker handles a few hundred lines per brief well.
- **The worker tests, you read.** Every brief tells it to write and run its own check (tests, a
  headless screenshot script, a log of numbers) and to save evidence to files. You read those files.
- **At most 2 tool calls per review:** `git diff --stat` plus one look (the test output, or one
  screenshot with Read). No browser poking, no step-by-step probing.
- **Feedback goes back as a brief,** never as your own edits, unless the fix is one line.
- **Stop rule:** a task wrong twice → do it yourself; two tasks wrong in a session → stop delegating
  and tell the owner.

Target: under 20 Claude steps for a small game, under 10 for a feature.

## 1. Is it on?

`tack model status`. `● … ON`: go. `○ … OFF`: `tack model on` (about 20 s, takes the whole GPU; say so in
one line, and ask first if the owner is gaming or rendering). No `[local_model]`: do the work yourself.

## 2. Write the brief

The worker sees none of this conversation, so the brief stands alone:

- the goal, and the exact files to create or change (paths from the repo root);
- the rules it must keep (style, names, libraries and versions, what not to touch);
- the check it must build and run, and where to save the evidence: `CHECK.md` with the commands and
  their output, plus screenshots under `shots/` for anything visual (it can write a headless
  Chromium/Playwright or Godot render script for that);
- the ending: "Change only those files plus CHECK.md and shots/. Do not create GOAL.md or other notes.
  Fix what your check finds. Reply with one line: what you built and whether the check passed."

## 3. Run it

From the project root, on a clean git tree (commit first, so the diff is easy to read):

```
timeout 900 pi -p --no-session --no-mcp --model tack-local/<model id> "<brief>" < /dev/null > .worker.log 2>&1
```

Keep `< /dev/null` (with stdin open pi waits silently: two 10-minute hangs in the Orb Drift test).
`--no-mcp` keeps it lean; drop it only when the task needs an MCP server (Godot AI for an editor task).
`<model id>` is the `model` in tack's `[local_model]`. Run it in the background and wait for the finish
notice instead of polling. One hand-off at a time: the server answers one request at a time.

## 4. Review (2 calls)

1. `git status --short && git diff --stat && tail -5 .worker.log && cat CHECK.md`
2. One look at the evidence that matters most: the failing line, or one screenshot with Read.

Read the diff only where the stat or the check looks wrong. Good: commit it. Faults: list them in a
numbered brief and hand it back (step 3). Tell the owner in one line what the worker built and what
you sent back.
