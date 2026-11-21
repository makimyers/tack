---
name: onboard
description: Start a session from the truth (git, the "What's left" lines on GitHub epics, docs/MEMORY.md, the newest handoff), then a six-line confirm-back and a paste-ready multitree block. Read-only. End the session with /wrap.
argument-hint: "[optional: the task, epic line or issue number for this session]"
---

Onboard onto this project. **Read-only until the owner says go:** no edits, commits, pushes, or
issue, label or board changes, and no plans before step 7. Run from the project root (`cd "$(git rev-parse --show-toplevel)"`).
Task from the owner, if any: the text after `/onboard` in their message.

## Rules (they hold all session)

- **Truth lives in three places:** git (code, status), the "What's left" lines in the GitHub epic
  bodies (the work), and `docs/MEMORY.md` (facts learned). Everything else (handoffs, the Next file
  `docs/roadmap/next.json`, "Where it stands" notes, branch names in docs, your memory) is a view: when it
  disagrees with a source, the source wins and the view gets fixed.
- An epic is an issue with the epic label. Its body has `## What's left`, in three groups: the owner's
  name, `Claude can build`, `Waiting on others`. `(must-have)` = launch fails without it: do those
  first. `(was #N)` = the closed ticket the line came from: read it before building the line.
- New work is one line on the right epic, never a new ticket (`/capture`). Ticketed work exists only
  where `/breakdown` made it or the owner asked.
- Nothing runs on a schedule: every command runs when the owner types it.
- Close with proof (commit, check output, CI run, live link). Never claim a result you did not run.
- GitHub reads: `gh api` (REST). Never `gh issue list` or `gh project item-list`: about 100 GraphQL
  points a page, against a 5,000/hour budget.
- `CLAUDE.md`/`AGENTS.md` and the docs it names win over this skill.

Shell variables don't survive between tool calls: start every snippet that uses them with
```bash
R=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null)
```
**No GitHub** (`R` empty, or no `gh`): skip
step 2 and the `gh` reads in step 5, work from git, `docs/MEMORY.md` and the newest handoff, and say
in one line what was skipped.

## 1. Git

```bash
git fetch -q || echo "fetch failed (offline?): remote state below may be stale"
git status -sb | head -25                     # line 1: branch...upstream [ahead n, behind n]
git rev-list --count HEAD --not --remotes     # commits on no remote: non-zero = say so loudly
git log --oneline -12
D=$(git symbolic-ref -q --short refs/remotes/origin/HEAD); echo "default branch: ${D:-unknown}"
git for-each-ref refs/heads ${D:+--no-merged=$D} --sort=-committerdate --format='%(refname:short) %(committerdate:short) %(upstream:track)' | head -8
```
Behind upstream = someone pushed from elsewhere: pull only after go.

## 2. Roadmap: read it

```bash
gh api "repos/$R/issues?state=open&labels=epic&per_page=100" --jq '.[] | "#\(.number) \(.title)", ((.body // "") | split("\n")[] | select(test("^\\*\\*Where it stands|^- \\[ \\]")) | "  \(.)")'
cat docs/roadmap/next.json 2>/dev/null || echo "no Next file"
```
It prints the open epics with "Where it stands" and their open "What's left" lines, then the Next
list. `gh` fails: quote its one line, carry on with git. Don't re-query GitHub for what it already
printed.

## 3. Work in flight

```bash
git worktree list | grep '/\.worktrees/' || echo "no onetree/multitree run"
for p in docs/*/task_plan.md; do [ -f "$p" ] || continue; d=${p%/task_plan.md}; n=$(grep -ciE 'stuck|todo|\[ \]' "$d/progress.md" 2>/dev/null); echo "$d: ${n:-no progress.md} open/stuck marks"; done
```
A worktree under `.worktrees/` is a running `onetree`/`multitree` loop; a plan folder with open or
stuck phases is unfinished. A loop owns those lines and its landing branch: don't build them here.

## 4. Read, in order

1. `CLAUDE.md`/`AGENTS.md` (already loaded) and every doc it says to read first: note the build
   checks, the places not to build, and what only a person can do.
2. `docs/MEMORY.md`, in full (none: "no MEMORY.md yet").
3. The newest handoff, in full: `ls docs/handoff 2>/dev/null | grep '\.md$' | sort | tail -1` (none:
   "no handoff yet"). "What's important" first. "What's left" is a candidate list: check each item
   against git and its epic, since a handoff is a snapshot.

## 5. Pick the work

- **Task given:** find its epic line or issue (`gh api -X GET search/issues -f q="repo:$R is:issue is:open <words>" --jq '.items[] | "#\(.number) \(.title)"'`).
- **No task:** must-have lines first, then `Claude can build` lines, in Next-file order. Skip
  `Waiting on others`, lines a loop owns (step 3), `parked` and `human-only` work. `area:decision`
  = the owner's call: prepare options, never decide. `blocked`: read its last comment.
- **For your top 3:** read the epic body, and the `(was #N)` ticket or the issue, with comments.
  They name what a line hides ("Depends on #", "blocked by", CONFLICT, unticked boxes):
  ```bash
  n=<issue>; gh api repos/$R/issues/$n --jq '"#\(.number) \(.title) [\(.state)]\n\(.body)"'
  gh api repos/$R/issues/$n/comments --jq '.[-3:][] | "-- \(.user.login) \(.created_at[:10]): \(.body)"'
  ```

## 6. Reconcile

List where the handoff, `docs/MEMORY.md`, the Next file or an epic's "Where it stands" disagrees with
git or GitHub: a line called open that is ticked, "pushed" work that isn't, a fact the code
contradicts. Report only; fix it in the first commit after go, or /wrap does.

## 7. Confirm back (6 lines max), the multitree block, then stop

Plain words, the action first:
```
Do first: <one line or #n, why now>. Go?   (blocked on a decision: Decide first: #n <the question>)
Where: <n> open lines, <m> must-have (<name them>) · <phase/date from MEMORY or the epics>
Next up: #e <line> · #e <line> · #e <line>   (or: your task → epic #e)
In flight: <branch>: <n> uncommitted, <n> unpushed, <n> behind · loops/plan folders running | none
Watch out: <MEMORY/handoff point that affects it> (<handoff file> | no handoff)
Drift: <a doc says X; git/GitHub says Y> | none found   · skipped: <what, if No GitHub>
```
When you suggested lines to build and the `multitree` skill is installed, add ONE
fenced block the owner can paste into a new session:
```text
/multitree
Build these "What's left" lines, one task each. Read each epic body and its (was #N) ticket first.
- #<epic>: <line text>
- #<epic>: <line text>
```
Then stop. No status page, no report page. Wait for go, or a different pick.

## After go

- Before the first code change, run the build checks once as a baseline; quote the exit codes.
- Never fake or claim what only a person can do (sign-ins, secrets, live data or DNS, deploying,
  anything `CLAUDE.md`/`AGENTS.md` lists): do the verifiable part, give the owner the exact steps.
- Git: additive commits. Never force-push, rebase or amend pushed commits, push the default branch,
  or delete branches. Push only when the owner says so (/wrap asks once).
- New idea mid-task: `/capture <idea>`, then carry on. Too big for one pull request: `/breakdown #<epic>`.
- Session end: `/wrap`. If the owner forgets and you changed state, remind them once.
