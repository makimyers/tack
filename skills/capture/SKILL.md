---
name: capture
description: Park a passing idea (task, bug, question, owner decision, "later" thought) as one line in the right epic's "What's left" list, in under two minutes, then carry straight on with the current work. A separate GitHub issue only when the user asks for one.
argument-hint: "<the idea, in your words>"
---

Park this as one "What's left" line on its epic, then carry on with the work in hand. The idea is the
text after `/capture` in the message. Empty: the latest tangent or "later" item in this conversation
(none obvious: ask only "Capture what?"). Several ideas: one line each. File it, don't work it: look
into it only as far as placing it needs. Leave ticking, the Next file, "Where it stands",
`docs/MEMORY.md` and git to /wrap. Facts only: what you saw (`path:line`, #n, a command's output) and
the owner's words, quoted and dated.

Run from the project root. Shell variables don't survive between tool calls. Start each snippet with:
```bash
R=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null)
```
`R` empty or `gh` failing: say so in one line and print the drafted line and the epic it belongs on.

## 1. Find the epic, and dedupe

Reads: `gh api` (REST). Never `gh issue list` or `gh project item-list` (about 100 GraphQL points a
page).
```bash
gh api "repos/$R/issues?state=open&labels=epic&per_page=100" --jq '.[] | "#\(.number) \(.title)", ((.body // "") | split("\n")[] | select(test("^\\*\\*Where it stands|^- \\[ \\]")) | "  \(.)")'
```
It lists the open epics with their open "What's left" lines.
- **Epic:** the one whose objective the idea serves. None fits: the parked/after-launch epic if there
  is one, else tell the owner in one line and print the drafted line; don't create an epic.
- **Already there** (an open line says the same): add the new detail to that line instead, or skip
  it if nothing is new; reply `Already on #<e>: <line>. Back to <current task>.`

## 2. Write the line

Group (as headed in the epic body): the owner's name for what only the owner can do or decide,
`Claude can build` for build work, `Waiting on others` for what waits on a named person. Format:
```
- [ ] <Name if waiting>: <what, in plain words> (<D Mon>)
```
Add `(must-have)` only when the owner says launch fails without it; `(see #n)` when an issue holds
the detail. No backticks or `$` in the line.

Edit the body from a fresh copy, one epic per call:
```bash
f=$(mktemp); gh api repos/$R/issues/<epic> --jq .body > "$f"; echo "$f"    # add the line under its group in that file, then:
gh api -X PATCH repos/$R/issues/<epic> -F body=@"$f" --jq .updated_at
```
No `## What's left` heading or no such group: add it, in that order of groups, after "Where it stands".

## 3. A separate ticket, only when asked

Only when the owner asks for an issue (or `/breakdown` territory: work too big for one line, then
say "run /breakdown #<e>" instead). Title: the outcome in plain words, under 70 characters, no
backticks or `$`. Body in a quoted heredoc:
```bash
gh issue create -R "$R" -t "<title>" -F - <<'EOF'
<Where it came up: `path:line`, #n, or the owner's words, quoted and dated.>

**Done when**
- [ ] <a result someone can check>
EOF
gh api -X POST repos/$R/issues/<epic>/sub_issues -F sub_issue_id=$(gh api repos/$R/issues/<n> --jq .id)
```
Labels only from the repo's existing ones (`gh api "repos/$R/labels?per_page=100" --jq '[.[].name] | join(", ")'`).

## 4. Reply

Two lines max, then resume the previous work where it stopped; don't wait for an answer:
`Captured on #<e> (<group>): <line>. Back to <current task>.` (or `Opened #<n> under #<e>.`)
