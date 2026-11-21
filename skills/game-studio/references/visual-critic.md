# Visual critic

A second pair of eyes on a visual change: does the player see what the change is meant to show? The game owns the
poses, questions, pictures and verdict file. This reference owns who reviews, what evidence they need, and when review
stops. It does not judge fun; only a human playing does.

## When

A change whose point is what the player sees: a landmark, a cue, a readable route, a lighting or colour pass. Run it
after the change builds and its deterministic checks pass. A critic never replaces a test: a high score cannot
override an invisible target, a blocked route or a failed check.

## 1. Builder: capture matched pairs

- Pick 2 to 6 player poses that answer the design questions. Record for each: position, camera yaw and pitch, field of
  view, resolution, HUD on or off. Use the real gameplay camera, not a free camera, unless the pose says so.
- Capture **before** from the unmodified build and **after** from the change, with the same script and poses. Keep the
  capture script in the game repo so a later round reuses it unchanged.
- Label pictures neutrally (`before/2_turn.png`, `after/2_turn.png`). No captions saying what to notice.
- Capture once per round, after the change is final; tune with numbers, not pictures. Render on the GPU (see
  `godot.md`, "Fast pictures"): software rendering makes each capture several times slower.

## 2. Builder: write the packet

One markdown file in the game repo (for example `docs/verify/<topic>/critic-packet.md`) holding:

| Section | Holds |
|---|---|
| Goal | What a player should notice or understand, in one or two lines, from the game's brief |
| Pictures | A table: pose name, before path, after path, absolute paths |
| Camera | Each pose's position, yaw, pitch, field of view, resolution and HUD state; the capture command |
| Reference | The game's look rules or reference file, if it has one |
| Questions | The design questions, each with the answer it requires |
| Checks | What the deterministic checks already prove (route tests, gates), so the critic does not re-judge them |

Leave out the builder's opinion, preferred verdict and scores.

## 3. Pick the critic

- One fresh, image-capable reviewer that did not build the change and will not edit the game: a new subagent on the
  repo's allowed model, with tools to read files and, if the packet allows, run the capture script. Follow the
  repo's and the harness's model rules; do not change routes or send pictures to a new service to get one.
- If none is available, the builder runs the same steps as a **self-check**, says so in the verdict's first line, and
  marks independent review **OPEN** in the report. Never write that a separate reviewer saw the pictures when none did.

## 4. Critic: the brief

Give the critic the packet path and these rules, nothing else from the builder's conversation:

1. Open every picture with an image-reading tool before answering. If a picture will not open, or a before and after
   pair is not the same pose, stop and return **insufficient evidence** naming the pair.
2. Answer each packet question with references to the picture paths and where in the frame (left third, top
   centre). Separate what you see (**SEEN**) from what you expect a player to do (**hypothesis**).
3. If one view is ambiguous, you may ask for, or take with the packet's capture script, **one** extra ordinary player
   view. Say which.
4. Score each after view 0 to 10 for readability of the thing it must show, and the before view for comparison.
5. List what got worse: clutter, hidden pickups, things that look solid but are not (or the reverse), blocked or
   misleading routes.
6. End with: the **single most important fix**, and a disposition: **pass**, **revise** or **insufficient evidence**.

The critic writes its verdict to `<packet folder>/critic-round<N>.md` or returns it for the builder to save verbatim.

## 5. Rounds and stopping

- Round 1 reviews the first implementation.
- On **revise**: fix only the top issue, recapture the same poses with the same script, and run round 2 with a fresh
  critic and the same packet plus the round 1 verdict.
- Stop after round 2. There is no pass by exhaustion: keep the best version whose checks pass, and report what is
  still unresolved.
- On **insufficient evidence**: fix the evidence (the capture, not the game) and rerun that round.

## 6. Report

In the game's report: the critic's identity (fresh subagent, model) or the self-check fallback, each round's
disposition and scores, the top fix and whether it was made, and what stays open. Whether the change feels good to
discover is the human playtester's call, never the critic's.
