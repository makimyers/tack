# Goal

One command that sets up, and keeps easy to manage, a coding-agent setup where local models do the
bulk of the work and a smarter model (Claude) reviews and realigns less often.

What that means for every change:

- **One install, Mac and Linux.** A single binary; nothing the owner has to wire by hand.
- **Easy to see and change.** `tack` shows what is on; `tack on` / `tack off` switches anything, per
  agent.
- **Lean prompts.** A local model must not be buried in harness text. Measure tokens before adding
  anything that every session loads.
- **Same behaviour from every model.** Short, plain, proactive replies that name what was done, the
  next step, and the goal.
- **Safe by default.** The guard runs before every tool call in every agent.
