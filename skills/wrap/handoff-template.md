# Handoff: /wrap step 8

The story of THIS session that git, the epics and `docs/MEMORY.md` can't tell on their own. It cites
them; it doesn't replace them. Run from the project root, after steps 1-7.

## 1. Gather the facts (never write from memory)

```bash
R=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null)
S=$(date +%F)     # session start day; began before today? set S=YYYY-MM-DD by hand
git status -sb | head -3; git rev-list --count HEAD --not --remotes
git log --since="$S 00:00" --all --no-merges --format='%h %ad %an %s' --date=short | head -60
[ -n "$R" ] && gh api "repos/$R/issues?state=closed&since=${S}T00:00:00Z&per_page=100" --jq '.[] | select(.pull_request|not) | select(.closed_at >= "'"$S"'") | "#\(.number) \(.state_reason) \(.title)"'
ls docs/handoff 2>/dev/null | grep '\.md$' | sort | tail -1     # the previous handoff: read it in full
git log --since="$S 00:00" -p --format='--- %h %s' -- docs/MEMORY.md | head -80
```
Open lines per epic: the epic listing from step 5. From the conversation: the owner's
rulings (word for word, dated), anything changed live and how it was verified, checks and their
numbers, defaults you chose, what failed or is half-done. A fact you can't trace to git, an issue, a
command or the owner's words goes under "Unverified".

## 2. Write `docs/handoff/YYYY-MM-DD-<topic>.md`

Sections, in this order:

1. **Summary**: 4-6 sentences, ending with the state the session leaves.
2. **Where we are**: a table: branch, unpushed count, CI; open "What's left" lines and must-have lines
   per epic; lines ticked and issues closed this session; live changes.
3. **Where we want to be**: the next dates (from `docs/MEMORY.md` or the epics), the open must-have
   lines with owner and date each.
4. **What we did, and why**: one `###` per epic touched (`Epic #e <title>`; the rest under `### Other`).
   Per item: what changed, why (the reason, or the owner's ruling quoted), the line or `#n`, sha(s),
   how it was verified, what is still open on it.
5. **What's important**, numbered: pending decisions (`#n` + the question in plain words), risks,
   traps (steps that break in the wrong order, tool quirks, live state the code depends on). Copy
   durable traps into `docs/MEMORY.md` if they aren't there.
6. **What's left, by owner**: checklists for the owner, the agent, and anyone else named. Quickest
   first. Each item names its epic line (`#e`, `was #N`) and the exact next step (a command, a file, a
   question). Carry over every still-open item from the previous handoff.
7. **Where things are**: the docs, folders and files this session created or changed.
8. **Unverified**: everything you couldn't confirm, or "none".

Plain sentences; tables where they help. Cite `#n`, shas and `path:line` so every claim can be
checked. No secrets. Link epics; don't copy their lines.

## 3. Check it

```bash
f=docs/handoff/<file>.md
for p in $(grep -oE '`[^` *<>]+/[^` *<>]+`' "$f" | tr -d '`' | grep -v '://' | sed 's/:.*//' | sort -u); do [ -e "$p" ] || echo "MISSING $p"; done
for c in $(grep -owE '[0-9a-f]{7,40}' "$f" | grep '[a-f]' | sort -u); do git cat-file -e "$c^{commit}" 2>/dev/null || echo "NO COMMIT $c"; done
```
Fix every hit, or say in the doc why it's expected (e.g. a file the next session creates).

## 4. Retire the previous handoff, commit

- Its open items are now in yours, so archive it:
  `mkdir -p docs/handoff/archive && git mv docs/handoff/<old>.md docs/handoff/archive/`.
  `docs/handoff/` keeps one file: the newest.
- `git add docs/handoff && git commit -m "docs(handoff): <YYYY-MM-DD> <topic>"`. No AI trailer. On the
  default branch, only if `CLAUDE.md`/`AGENTS.md` allows commits there (as in step 7).
- Push only under step 3's yes.
