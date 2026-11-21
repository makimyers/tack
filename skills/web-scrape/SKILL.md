---
name: web-scrape
description: Fetch web content the normal route can't handle, cheapest tool first. Covers YouTube transcripts and metadata (yt-dlp), GitHub (gh), RSS/Atom feeds, JavaScript-rendered pages (Moli, then Lightpanda, then Scrapling's Chromium), pages that block simple clients (Scrapling stealth), and bulk or CSS-selector extraction across many pages. Use when a task names YouTube/GitHub/a feed, or after ctx_fetch_and_index / WebSearch failed or returned an empty JS shell.
---

# web-scrape: escalation ladder

Local CLIs on PATH: `yt-dlp`, `moli`, `lightpanda`, `scrapling` and `gh`. They are prerequisites the user installs; this skill installs nothing. A rung whose CLI is missing: say which one, skip to the next rung. There are **no MCP servers** for these, on purpose: CLI plus file output is cheaper per use. Always write output to a file under `/tmp/scrape/`, never to stdout. Then read only what you need: `grep`, `ctx_index` + `ctx_search` (context-mode), or `head -c 2000`.

## Ladder (stop at the first rung that works)
1. **Static pages:** context-mode `ctx_fetch_and_index(url)` + `ctx_search`.
2. **Platforms:**
   - **YouTube:** video info and subtitles, no download:
     `yt-dlp --skip-download --write-auto-sub --write-sub --sub-langs "en.*" --sub-format vtt -o "/tmp/scrape/%(id)s" "<url>"`
     Add `--print "%(title)s | %(channel)s | %(duration_string)s"` for metadata only. Then strip timestamps from the `.vtt` with `sed`/`grep` before reading it.
   - **GitHub:** `gh repo view owner/repo`, `gh search repos "<q>" --limit 10`, `gh issue list -R owner/repo --search "<q>"`, `gh api repos/owner/repo/contents/<path>`. These need `gh auth login`, done by the user. Without it, use `ctx_fetch_and_index` on `https://raw.githubusercontent.com/...` or `https://api.github.com/...` (public, rate-limited).
   - **RSS/Atom:** `ctx_fetch_and_index(url)` on the feed URL, then `ctx_search`. Feeds are plain XML, so no browser is needed.
3. **JavaScript-rendered pages:** **Moli** (Rust headless browser, 0.2–4 s). In our 14-URL test it found 23/24 key phrases vs Lightpanda's 15/24:
   - Main text only (smallest output): `moli fetch --obey-robots --eval '(document.querySelector("main")||document.body).innerText' "<url>" > /tmp/scrape/<name>.txt`
   - Markdown with links: `moli fetch --obey-robots --dump markdown --strip-mode ui "<url>" > /tmp/scrape/<name>.md`. Plain `--dump markdown` keeps site navigation (2–5× larger).
   - Wait for content: `--wait-selector "<css>"`; slow pages: `--timeout 45000` (ms).
   - Agent-friendly structure: `--dump semantic_tree_text`. Screenshot/PDF: `--dump screenshot` / `--dump pdf` (add `--layout`).
   - If Moli errors or returns an empty shell, try **Lightpanda** (Zig, ~0.2 s, low memory):
     `lightpanda fetch --obey-robots --dump markdown --strip-mode clutter --dump-max-bytes 200000 --wait-until networkidle "<url>" > /tmp/scrape/<name>.md`
     Add `--dump-selector "<css>"` for one part, or `--json url1 url2 …` for several URLs. Its clutter stripping can drop real content (headings, list items); fixed `Lightpanda/1.0` user agent.
   - Both have no stealth; bot-protected sites block them. If output is still empty or wrong, go to rung 4.
4. **Real Chromium:** **Scrapling** (wrap in `timeout 120`; its Cloudflare solver can overrun):
   `scrapling extract fetch "<url>" /tmp/scrape/<name>.md -s "<css>"` (~4 s)
   - Static HTML that blocks simple clients: `scrapling extract get "<url>" /tmp/scrape/<name>.md --ai-targeted -s "<css>"`. Needs no browser.
   - `extract fetch` needs a browser that is downloaded once. If it reports a missing browser, ask the user to run `scrapling install` (about 580 MB).
5. **Bot-protected pages:** `scrapling extract fetch … --solve-cloudflare` (`fetch` is Scrapling's stealthy fetcher). Only for legitimate access (see rules) — and say that you used it.

The output format follows the extension: `.md` is markdown, `.txt` is text, `.html` is raw HTML. `-s` / `--dump-selector` keep output small; check the structure first with a cheap `.html` fetch plus `grep` if you need to.

## Interactive browsing (forms, clicks, multi-step)
Write a short Playwright/Puppeteer script against a local CDP server instead of adding an MCP server:
`lightpanda serve --host 127.0.0.1 --port 9222 &`, then `chromium.connectOverCDP("ws://127.0.0.1:9222")`. Kill it when done. **Never** bind `0.0.0.0`: CDP has no authentication. For full-fidelity UI testing use the `e2e` skill (real Playwright browsers) instead.

## Rules
- Respect robots.txt, site terms, rate limits and logins. Moli and Lightpanda ignore robots.txt unless you pass `--obey-robots`, so always pass it. When crawling with Lightpanda, add `--http-nav-delay 1000`; with Moli, fetch one URL at a time.
- No scraping behind someone else's paywall or auth, and no personal-data harvesting.
- Stealth and Cloudflare-bypass features are only for sites the user owns or is allowed to access. Ask if unsure.
- Don't pass cookies or credentials unless the user supplies them for that site.
- Telemetry: run Lightpanda with `LIGHTPANDA_DISABLE_TELEMETRY=true` set.
- Don't `pip install`, `npm i`, or download browser binaries; the one-time `scrapling install` is the user's to run.
