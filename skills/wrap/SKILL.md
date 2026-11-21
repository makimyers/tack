---
name: wrap
description: End the session in one command so the next agent starts from the truth. Ticks the "What's left" lines that got done on GitHub epics, closes with proof, turns new work into epic lines (never new tickets), rewrites the Next file and stale "Where it stands" notes, updates docs/MEMORY.md, commits the docs, and last writes the handoff (docs/handoff/YYYY-MM-DD-<topic>.md) that /onboard reads first.
argument-hint: "[optional: a 1-3 word topic for the handoff]"
---

Wrap up this session. Steps in order; outward actions (pushes, issue or epic edits) only where a step
says so. Run from the project root. Write only what comes from git, an issue, a command you ran, or
the owner's words (quoted, dated); mark anything else "unverified" or leave it out. Truth lives in
git, the "What's left" lines on the epics and `docs/MEMORY.md`; everything else is a view.

**Unattended** (a loop or plan prompt with nobody to answer: an `onetree`/`multitree` worktree, a
/goal-loop round): never ask, never push; see the last section.

Shell variables don't survive between tool calls. Start each snippet that needs them with:
```bash
R=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null)
D=$(git symbolic-ref -q --short refs/remotes/origin/HEAD); D=${D#origin/}      # the default branch
```
**No GitHub** (`R` empty, or no `gh`): skip the CI part of step 3 and steps 4-5's GitHub writes,
still do 6-8, and say in one line what was skipped.

## 1. What happened

```bash
git status -sb | head -25
git log --oneline --since=midnight     # this session's commits; started earlier? --since="YYYY-MM-DD HH:MM"
git rev-list --count HEAD --not --remotes
[ -n "$R" ] && gh api "repos/$R/issues?state=open&labels=epic&per_page=100" --jq '.[] | "#\(.number) \(.title)", ((.body // "") | split("\n")[] | select(test("^\\*\\*Where it stands|^- \\[ \\]")) | "  \(.)")'
```
From the conversation and those commits, list: epic lines and issues worked on, finished (how
verified), half-done (what's left), new work found, decisions (the owner's words + date), lessons a
future agent would trip over.

## 2. Checks (only if code changed)

```bash
{ git status --porcelain | cut -c4-; git log --since=midnight --name-only --format=; } | sort -u | grep -vE '^(docs/|\.claude/|\.agents/)|\.md$' | head -5
```
Nothing listed = "checks: skipped (docs only)". Else run `CLAUDE.md`/`AGENTS.md`'s checks (none named:
say so), each as `( <check> 2>&1; echo "exit $?" ) | tail -8`; quote every exit code. Red: say so and
tick or close nothing it covers. A UI change must also be seen working in the running app.

## 3. Push: ask once

Never push the default branch, force, rebase or amend pushed commits, or delete branches. Unpushed
commits on a non-default branch: ask once, "Push `<branch>` (<n> commits)?". The answer covers the docs
and handoff commits below too. Yes: `git fetch -q`; behind its upstream: `git merge --no-edit '@{u}'`
(conflict: `git merge --abort`, push nothing, tell the owner); then `git push -u origin HEAD`. No, or
the default branch: put "(unpushed)" after every sha you cite.

Pushed, and the repo has `.github/workflows/`: cite each run on HEAD as `CI <conclusion>: <url>`. A red
run beats green local checks.
```bash
gh api "repos/$R/actions/runs?head_sha=$(git rev-parse HEAD)" --jq '.workflow_runs[] | "\(.id) \(.name): \(.status) \(.conclusion // "-") \(.html_url)"'
```

## 4. GitHub: the epic lines

Reads: `gh api` (REST). Never `gh issue list` or `gh project item-list`. Text with backticks or `$`
never goes inside `"..."`: use a quoted heredoc, `gh issue comment <n> -R "$R" -F - <<'EOF'`.
Edit an epic body only from a fresh copy, one epic per call:
```bash
f=$(mktemp); gh api repos/$R/issues/<epic> --jq .body > "$f"; echo "$f"    # edit that file, then:
gh api -X PATCH repos/$R/issues/<epic> -F body=@"$f" --jq .updated_at
```
- **A line done and verified:** change its `- [ ]` to `- [x]` (that line only) and comment the proof on
  the epic: `Done: <line>. <sha>. Checks: <result>. <CI line | live link>`.
- **A ticket finished and verified:** tick its "Done when" boxes, then
  `gh issue close <n> -R "$R" -r completed -c "Done in <sha>: <what>. Checks: <result>. <CI line>"`.
  Folded into an epic line or withdrawn: `-r "not planned"` plus where it went or why.
- **Half-done:** one comment (done, next, any blocker) on the epic or ticket; take `in-progress` off if
  work stopped.
- **A ticket box waits on a person or another line:** add it as a line on the right epic (keep
  `(must-have)`, the owner, any date), strike it on the ticket as `~~box~~ → moved to #<epic>`, and
  close the ticket if every box left is done.
- **New work:** never a new ticket. ONE line in the right group of the right epic's `## What's left`:
  `- [ ] <Name if waiting>: <what, in plain words> (<D Mon>)`, `(must-have)` only on the owner's word.
- **`blocked`:** label plus a comment naming exactly what it waits on; off when it arrives.
Never tick or close what isn't verified.

## 5. Where it stands, and the Next file (No GitHub: skip, say so)

1. **"Where it stands":** for every epic whose lines moved, and every open epic whose note is older
   than 7 days, rewrite the one `**Where it stands (D Mon):** ...` paragraph (today's date, current
   facts, numbers inline; ends at a blank line; none yet: add it at the top), as in step 4.
2. **Next file** (`docs/roadmap/next.json`; missing: create it): about 20 open epics or tickets, never a
   closed one. Epics with an open must-have line first (what gates others on top), then ticketed work,
   then the rest. `why` names the lines to do first; `updatedAt` = today:
   `{ "updatedAt": "YYYY-MM-DD", "items": [{ "issue": 12, "why": "<lines to do first, one line>", "who": "<who>" }] }`
3. Re-run step 1's epic listing: no closed rows in Next; touched epics show today's date. Wrong?
   Fix GitHub or the Next file.

## 6. docs/MEMORY.md

Durable facts and gotchas only, never status or task lists. Missing: create it. One line per lesson
under the right heading, newest first: `- <fact> [<sha | #n | command | owner's words>; YYYY-MM-DD]`.
Delete lines this session proved false or superseded (git keeps history). About 60 lines at most; no secrets.

## 7. Commit the docs

Stage only what this wrap changed. Review every hit of
`git diff --cached | grep -E '^\+' | grep -iE 'secret|passw|token|api[_-]?key|private key'`, then
`git commit -m "docs: wrap $(date +%F) <topic>"` with no Co-Authored-By or other AI trailer. On the
default branch: commit unless `CLAUDE.md`/`AGENTS.md` forbids commits there (then leave the edits
uncommitted and say so). Push only under step 3's yes.

## 8. Handoff, always, last

Follow `handoff-template.md` in this skill's folder: it writes `docs/handoff/YYYY-MM-DD-<topic>.md`
(topic: the text after `/wrap`, else 1-3 words), checks every cited path and sha exists, archives the
previous handoff and commits. Push only under step 3's yes.

## Summary (10 lines max)

Plain words, the next action first:
```
Next: <the owner's one action (answer #n <question> | push <branch>) — else: next session starts with #e <line>>
Lines: ticked #e … · added #e … · closed #… · commented #…
Checks: <check> exit <n> … · CI <conclusion> <url> | skipped (docs only)
Git: <branch> pushed | <n> unpushed (why)
Roadmap: Next top 5 #… · notes rewritten #… · MEMORY +<n> / −<n> lines
Handoff: docs/handoff/<file>.md   · skipped: <what, if No GitHub>
```

## Unattended

Never ask, never push: the loop's runner lands and pushes the branch. Worktree shas vanish in the
squash, so cite the plan and phase instead (`Done in <plan dir> phase <n>, lands as one squash
commit`). Steps 1, 2 (once, on the final tree), 4, 6 and 7 as written; skip 3, 5 and 8 (no Next
rewrite, no handoff in a loop). Any red check: comment, tick and close nothing. A phase that needs a
person: comment the exact steps, leave the line open. Commit as
`docs: wrap $(date +%F) <plan> (unattended)`; end with the summary.
