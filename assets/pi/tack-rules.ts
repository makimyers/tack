// tack-rules: written by tack. `tack install` rewrites this file; `tack off rules --pi` removes it.
// A local model drifts from rules that sit only in the system prompt, so each prompt carries a
// short reminder of the reply rules in AGENTS.md. It is stored in the session but not displayed.
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

const REMINDER =
  "Reply rules: plain human words, as few lines as you can. " +
  "End every reply with two lines: `Done: <what you just finished>` and " +
  "`Next: <the one next step toward the goal, never none>`. " +
  "If you have not read GOAL.md in this session, read it before you start.";

export default function (pi: ExtensionAPI) {
  pi.on("before_agent_start", (event) => {
    if (event.messageType !== "user") return undefined;
    return { message: { customType: "tack-rules", content: REMINDER, display: false } };
  });
}
