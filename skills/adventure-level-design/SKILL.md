---
name: adventure-level-design
description: Level-design lens for adventure, climbing and exploration games (Zelda-like journeys, Peak-style climbs, Portal-style "aha" puzzles). Use for any game design, level design, level brief, playtest review or ticket that shapes where the player goes and why: making a level more puzzly, harder, more of an adventure, or worth exploring.
---

# Adventure level design

The core idea: **let the player see the goal, form a theory about the place, and try it.** The fun comes from
figuring out how to reach a destination and then pulling off the move, not from the destination itself.

Apply this to every design answer, level brief, ticket and playtest review on the game. A brief that cannot say
which of the ten principles below it serves is not ready.

## The ten principles

| # | Principle | What it looks like on a mountain climb |
|---|---|---|
| 1 | **The player sees somewhere they want to go.** A landmark is a goal they chose themselves. | Show the goal (a star) through a notch in the ridge. It disappears behind the cliff on approach and reappears from higher up. |
| 2 | **The terrain itself is a clue.** Looking closely changes the plan. | The face looks sheer from below. A few steps sideways, daylight reveals a narrow cleft with two rests. |
| 3 | **A safe encounter teaches one rule.** Learn by doing. | Early: jump sideways into a crack with solid ground below. Later: the same shape across a real drop, recognised without a sign. |
| 4 | **Difficulty asks for judgment.** Understand the move and still have to commit. | The next hold is in reach, but can the shelf above it be reached on this much stamina? A sideways traverse offers a longer, sure rest. |
| 5 | **One obstacle, several plans.** Each answer feels like the player's own. | A thrower watches a gap: cross after its shot, knock it off from a side perch, or slip through the cleft beneath. Each uses a move the player already has. |
| 6 | **A puzzle makes the player reconsider what they saw.** "Oh, *that's* what that was for." | Pass under a stone arch on the way up; from a high shelf, the top of that arch is the landing for a glide to the goal. |
| 7 | **Curiosity pays off.** Small irregularities invite investigation. | A dry ledge glimpsed through a break in a waterfall leads to a sheltered way up. |
| 8 | **A mistake creates a new situation.** A miss asks for a quick decision, not a reset. | Miss the high grab, fall past a lower crack, catch it and scramble back by a rougher line. Costs height; the player keeps playing. |
| 9 | **Quiet stretches give hard ones meaning.** Safety lets the player take in the place. | After a tight cleft, a broad silent shoulder where camp is visible far below. Give that view room before the next hazard. |
| 10 | **Reaching a place changes what the player can do.** The summit is worth more than the pickup. | From the goal shelf, glide back through the notch seen from camp. The cliff that took effort to understand becomes a playground on the way down. |

## Rules for a design pass

- **Destination before climb.** Give the player something they want to reach before asking them to climb toward it.
  Signs that name routes are not a reason; a visible goal is.
- **Puzzle = route discovery.** Put the goal where the obvious line fails (a wall taller than one stamina bar) and
  a less obvious line works ("I can get there if I go sideways first"). Leave the discovery to the player.
- **Difficulty = execution after discovery.** Once the route is spotted, the move still needs timing or commitment.
  Danger must be readable enough to plan around: if the player asks "why is that dangerous?", it is not readable.
- **Misses stay fresh.** A lower shelf catches a fall so the next attempt starts while the lesson is fresh.
- **Surprise on the way.** Round a blind corner into a perch, a view, a secret. Build around the moments playtests
  show the owner already loves (discovery found unprompted).
- **Shape before art.** Keep blocks; shape one recognisable ridge, a skyline notch and a shelf under an overhang.
  Shapes that make a place recognisable also make the climb readable.
- **Guard the puzzle.** Park mechanics that bypass route-finding (free building) until the core climb works; keep
  ones that change which route is best (e.g. carrying cargo up a known climb).
- **Small and deep.** Improve one climb (camp to goal) fully before widening. Keep the mountain the same size.

## How to judge it in a playtest

Two moments decide whether a climb works:
1. Does the player **pause because they've spotted a route**?
2. After a miss, **do they know what they want to try differently**?

Also look for: unprompted detours toward a framed landmark, "oh, that's what that was for" remarks, and how long
the player looks at the view on quiet stretches. Cite the playtest timestamp for each claim
(use `playcap-review` for the session).

## Writing a level brief

Each brief covers one small area and names: the goal the player sees and from where; the obvious line and why it
fails; the hidden line and the clue that reveals it; the execution test; what catches a miss; the quiet beat and
the payoff after; which principles (by number) it serves; and the playtest moment that will show it worked.
The game's verify command still defines done for the code; the owner judges feel.
