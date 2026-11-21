---
name: design-md
description: Drop a ready-made brand-inspired DESIGN.md (74 styles such as linear.app, stripe, vercel, notion, apple, airbnb) into a project so UI work follows one consistent visual system. Use when building or restyling a UI and the user names a look ("make it feel like Linear") or the project has no design system yet.
---

# design-md

The local library lives at `~/.local/share/awesome-design-md/design-md/<slug>/DESIGN.md` (VoltAgent/awesome-design-md, MIT). Each file is 7–10k tokens, so never read more than one, and never read them just to browse.

## Steps
1. If the project already has a `DESIGN.md` (or `design-system/*/MASTER.md`), that is the source of truth. Stop here and follow it.
2. If the library is missing, ask the user, then fetch it: `git clone --depth 1 https://github.com/VoltAgent/awesome-design-md ~/.local/share/awesome-design-md`.
3. List the choices: `ls ~/.local/share/awesome-design-md/design-md/`.
4. Pick the closest slug to what the user asked for. If nothing was named, choose by product type: dev tools → linear.app/vercel/raycast, docs → mintlify, consumer → airbnb/spotify, fintech → wise/revolut. State the choice.
5. Copy it in: `cp ~/.local/share/awesome-design-md/design-md/<slug>/DESIGN.md ./DESIGN.md`. Then read it once, and build the UI from its tokens (colours, type scale, spacing, radii, components).
6. Optional: run `ui-ux-pro-max` (`--domain ux` / `--stack <stack>`) for UX rules. DESIGN.md stays the visual source of truth.

## Rules
- These are interpretations "inspired by" real brands. Never ship a lookalike of a bank, payment, crypto or other trademark-sensitive brand as if it were theirs. Rename it, change the logo and wordmark, and adapt the colours for production work.
- Some files name proprietary fonts (Söhne, SF Pro…). Substitute a free font (Inter, Geist, IBM Plex…) unless the project already licenses it.
- Some files say to run `npx @google/design.md lint DESIGN.md`. Only run it if the user agrees.
- Don't use `npx getdesign`; it sends telemetry and the local copy is identical.
- To update the library, the user runs `git -C ~/.local/share/awesome-design-md pull`.
