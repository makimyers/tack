---
name: breakdown
description: Split an epic, or a big goal, into small independent GitHub tickets filed in waves under the roadmap's rules, then hand the ready ones to a builder (a coding agent on GitHub by @-mention, this session, a /goal-loop or onetree run, or a person). Re-run it on the same epic to release the next wave as tickets close.
argument-hint: "<epic #n, or the goal in your words>"
---

Break this work into tickets an agent can finish in one pull request each, file them, then release
the ones that can start now. Input: the text after `/breakdown` in the message: an epic number (`#12`
or `12`) or a goal in plain words. Empty: ask only "Break down what?".

Plan before you file; file before you release. The split decides how often parallel pull requests
collide, so run this step on the strongest model you have; the tickets can run on any model.
Facts only: what you read in the code, the epic and the project rules, and the owner's words.

Run from the project root. Shell variables don't survive between tool calls. Start each snippet with:
```bash
R=$(gh repo view --json nameWithOwner -q .nameWithOwner)
```
Reads: `gh api` (REST). Never `gh issue list` or `gh project item-list` (about 100 GraphQL points a
page). `R` empty (no GitHub repo): say so and stop.

## 1. Mode

```bash
gh api "repos/$R/milestones?state=open&per_page=100" --jq '.[] | "\(.title) due \(.due_on // "-" | .[:10]) \(.closed_issues)/\(.open_issues + .closed_issues)"'     # phases
gh api "repos/$R/issues?state=open&labels=epic&per_page=100" --jq '.[] | "#\(.number) \(.title)", ((.body // "") | split("\n")[] | select(test("^\\*\\*Where it stands|^- \\[ \\]")) | "  \(.)")'     # epics
gh api "repos/$R/labels?per_page=100" --paginate --jq '[.[].name] | join(", ")'
n=<epic>; gh api "repos/$R/issues/$n/sub_issues?per_page=100" --jq '.[] | "#\(.number) \(.state) \(.title)"'
```
- **An epic with open tickets:** release only (step 5). The owner asked to add work to it: plan only
  the new part (steps 2-4), then step 5.
- **An epic with none, or a goal in words:** steps 2-5. A goal: search for its epic first
  (`gh api -X GET search/issues -f q="repo:$R is:issue is:open label:epic <key words>" --jq '.items[] | "#\(.number) \(.title)"'`).
  None fits: create it, in the next free `Epic N:` number, and use its number from here on:
  ```bash
  gh issue create -R "$R" -t "Epic <N>: <the outcome>" -l epic -F - <<'EOF'
  **Where it stands (<D Mon>):** Being broken down into tickets.

  <the goal in the owner's words, quoted and dated>
  EOF
  ```

## 2. Read

The epic's body and latest comments; `CLAUDE.md`/`AGENTS.md` and the docs they name (build checks,
places not to build); then the code the goal touches, only enough to name each ticket's files (`ls`,
`grep`, entry points). Don't start building.

## 3. Plan the tickets (file nothing yet)

Each ticket:
- **One outcome, one pull request**, reviewable in about 15 minutes: roughly 300 changed lines at
  most, inside one module or along one existing chain. Bigger: split it. A redesign that spans
  several modules or changes what callers expect is the owner's call: file it with `area:decision`,
  not the agent label.
- **Touches:** the files or folders it will change (your best reading of step 2).
- **Done when:** 1-3 boxes, each a check someone can run: a test name, a command and its expected
  output, a page that shows X. UI changes: before and after screenshots.
- **Depends on:** the tickets whose code or interface it needs merged first.

**Waves.** Wave 1 = tickets with no dependency; wave n = tickets whose dependencies are all in earlier
waves. Within one wave no two tickets touch the same file: move one to a later wave (it now depends
on the other) or merge the two. Shared groundwork (a type, a schema, a config key) is an early ticket
of its own. Keep it to 3-15 tickets; more means the goal is several epics: plan the first.

## 4. File them, wave 1 first, so later bodies can cite real numbers

Milestone: the epic's phase (as in /capture: the phase most of its tickets are in, else the current
one). Labels, only from the list in step 1, never new ones: the agent label on tickets an agent
builds; an `area:*` that fits. No week label or assignee unless the owner named one. Never `blocked`
for a later wave: it means stuck on something external. The
`Depends on` line is what holds a ticket back.
```bash
gh issue create -R "$R" -t "<outcome, under 70 characters>" -m "<phase>" -l "<label>,<label>" -F - <<'EOF'
<what and why, one or two lines.> Part of #<epic>.

**Touches:** `path/a`, `path/b`
**Depends on:** #a, #b

**Done when**
- [ ] <a check someone can run>
EOF
```
Leave out the `Depends on` line in wave 1. Then make each one a sub-issue of the epic, by numeric id,
not number: `gh api -X POST repos/$R/issues/<epic>/sub_issues -F sub_issue_id=$(gh api repos/$R/issues/<n> --jq .id)`.
Last, rewrite the epic's `**Where it stands (D Mon):**` paragraph (as in /wrap step 5): "Broken into
#a-#z in <w> waves; wave 1 (#a, #b) ready."

## 5. Release the ready tickets

```bash
n=<epic>; gh api "repos/$R/issues/$n/sub_issues?per_page=100" --jq '.[] | "#\(.number) \(.state) \(.state_reason // "-") [\([.labels[].name] | join(","))] assignees:\(.assignees | length) \(.title)\n  \((.body // "") | split("\n") | map(select(test("depends on"; "i"))) | join(" "))"'
```
Sort each open ticket from that output into: ready (every `Depends on` ticket closed as completed,
not started), started (assigned or labelled `in-progress`), waiting (on which tickets), held
(`blocked`, `parked` or another deferred label: leave it) or stuck (a dependency not found or closed as
not planned or duplicate: re-plan that ticket now, as in steps 3-4). A dependency that is not a
sub-issue: `gh api repos/$R/issues/<n> --jq '"\(.state) \(.state_reason)"'`. Then, by whether the
owner named a coding agent's GitHub login (`CLAUDE.md`/`AGENTS.md` or the owner's words):
- **Named:** on each ready agent-labelled ticket, comment from your own GitHub login, which the agent
  takes as an explicit request, then mark it `in-progress` so the next run skips it:
  ```bash
  gh issue comment <n> -R "$R" -F - <<'EOF'
  <@login> please build this ticket: one pull request whose description says `Closes #<n>`, changes kept to the files under **Touches**, and every **Done when** box met.
  EOF
  gh issue edit <n> -R "$R" --add-label in-progress    # label missing: gh label create in-progress -R "$R"
  ```
- **Not set:** the ready list is the queue. Build them here one at a time (each as its own branch
  and checks), or list them as `- [ ]` items of a plan for /goal-loop or the `multitree` skill.

Later waves start when their dependencies close. Run `/breakdown #<epic>` again after merges.
Nothing runs on a schedule.

## 6. Reply (5 lines max)

```
Epic #<e> <title>: <n> tickets in <w> waves (#a-#z).
Ready: #a <title> · #b <title> → dispatched to <mention> | build next here | queue for /goal-loop
Waiting: #c on #a · #d on #a, #b
Stuck or for the owner: #x <why> | none
Next wave: re-run /breakdown #<e> after merges.
```
