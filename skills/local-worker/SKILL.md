---
name: local-worker
description: Hand a well-specified coding task to the owner's local model (pi on the tack local model) and check its work, instead of doing the bulk yourself. Use for mechanical or bulk work with a clear done-check (write tests, apply a pattern across files, boilerplate, a refactor with a spec, a first draft to review), when the user asks to "use the local model / Qwen / pi", or to save Claude usage. Not for design decisions, unclear specs or anything outward-facing.
---

You are the lead; the local model is a fast, free, weaker worker. You write the brief, it does the
typing, you check the result. Never pass its work on unchecked.

## 1. Is it on?

Run `tack model status`. `● … ON` means go. `○ … OFF`: switch it on with `tack model on` (it loads
in about 20 seconds and takes the whole GPU, so say in one line that you did it). If the owner is
gaming or rendering, ask first. No `[local_model]` set: do the work yourself.

## 2. Write the brief

One task per call, small enough to finish in one go (one file or one pattern). The worker sees none
of this conversation, so the brief must stand alone:

- the goal in one sentence, and the exact files to touch (paths from the repo root);
- the rules it must keep (style, names, what not to change);
- the done-check: a command that must pass (`python3 -m pytest -q tests/test_x.py`, `cargo test x`);
- end with: "Change only those files. Do not create GOAL.md or other notes. Run the check, then
  reply with one line: what you changed and the check's result."

## 3. Run it

From the project root, on a clean git tree (commit or stash first, so its diff is easy to read):

```
pi -p --no-session --no-mcp --model tack-local/<model id> "<brief>"
```

`--no-mcp` keeps the worker lean and stops a hang: without it the first call in the 2026-11-21 test sat
10 minutes with no output while pi waited on an MCP server. Drop it only when the task needs one
(e.g. Godot AI for an editor task), and then give the call a `timeout 600`.

`<model id>` is the `model` in tack's `[local_model]` (`tack model status` names it). A short task
takes 10–60 s; run longer ones in the background with output to a log file, keep working, and read
the log when it finishes. Several independent tasks: run them one after another, not in parallel
(the server answers one request at a time).

## 4. Check it

- `git status` and `git diff`: only the files you named changed; revert anything else
  (`git checkout -- <file>`, delete stray files).
- Run the done-check yourself. Its "passed" line is not proof; yours is.
- Read the diff like a code review: wrong logic, weakened tests, skipped cases.

Good: keep it. Small fault: fix it yourself. Wrong twice on the same task: do it yourself and stop
delegating that kind of task this session. Tell the owner in one line what the local model did and
what you fixed.
