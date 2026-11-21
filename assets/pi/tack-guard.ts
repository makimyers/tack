// tack-guard: written by tack. `tack install` rewrites this file and `tack uninstall` removes it.
// Runs `tack guard --harness pi` on every tool call and blocks the call unless it exits 0.
import { spawnSync } from "node:child_process";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

const TACK_BIN: string = "__TACK_BIN__";

export default function (pi: ExtensionAPI) {
  pi.on("tool_call", async (event, ctx) => {
    const res = spawnSync(TACK_BIN, ["guard", "--harness", "pi"], {
      input: JSON.stringify({ tool: event.toolName, input: event.input, cwd: ctx.cwd }),
      cwd: ctx.cwd,
      encoding: "utf8",
      timeout: 10_000,
    });
    if (res.error) {
      return { block: true, reason: `tack guard could not run ${TACK_BIN}: ${res.error.message}` };
    }
    if (res.status === 0) return undefined;
    return { block: true, reason: res.stderr.trim() || `tack guard exited with ${res.status ?? res.signal}` };
  });
}
