---
name: chat-table
description: 'Show a small, scannable markdown table in the chat reply instead of paragraphs or long lists. Use when the user asks "where are we", for a summary, status, overview, comparison or options, or when a reply covers 3+ things that share 2+ attributes (projects, tasks, options, findings). Not for one fact, one action or code.'
---

# Chat table

The user has ADHD and reads tables faster than prose. When a reply covers several things with the same attributes, show one small table in the chat.

## When

- "Where are we", "summarise", "status", "overview", "compare", "what are my options".
- 3 or more items that share 2 or more attributes.
- Not for: a single answer, a single next step, code, or anything that reads better as one sentence.

## Shape

1. **Line 1: the answer in one plain sentence.** The table backs it up; it doesn't replace it.
2. **One table**, GitHub markdown:
   - At most 7 rows and 5 columns. More items: show the 7 that matter most, then one line "and N more" (say where they are).
   - First column: the thing (project, task, option), in the user's own words.
   - **Last column: Next action** whenever the rows are things that can move (projects, tasks, options): one concrete step and who does it ("You: click yes", "Claude: review the report"). Never leave it blank; write "None" if nothing is next.
   - Cells: 8 words or fewer, plain words, no jargon or file paths unless the user needs to open one.
   - A status column may use 🟢 🟡 🔴, with what each means in the header or one line under the table.
   - Order rows by what matters most to the user (closest to done, biggest, most urgent), and say the order if it isn't obvious.
3. **After the table: at most one line**, usually `Next: <one action>`.

No second table, no notes section, no recap under the table. If detail is needed, it goes in a file and the reply links it.

## Example

Nothing is ready to build yet; the co-op mountain game is closest.

| Product | Research says | Status | Next action |
|---|---|---|---|
| Co-op mountain game | People buy online co-op chaos | 🟡 buyer partly proven | Claude: design a cheap test |
| UNPLUG IT | Short horror median: 31 reviews | 🟡 | Claude: profile it |
| Rustiton | iPhone covered; Android small | 🔴 parked | None |

Next: say yes or no to the co-op test design.
