---
name: speakit
description: Read the previous reply aloud as a natural spoken script through the Mac's audio. Use when the user types /speakit, or says "speak it", "speakit", "speak", "talk", or "read that out".
---

# speakit

Turn your last text reply into something worth listening to, then play it out loud on this Mac.

## Steps

1. **Source:** your most recent substantive reply in this conversation (not tool output). If the user names something else ("speak the table part"), use that.
2. **Rewrite as a spoken script:**
   - Open with the bottom line in one or two sentences.
   - Free-flowing and conversational, like explaining it to a colleague. Contractions fine. Direct, not hyped. No "great question", no sign-off.
   - Keep every fact that drives a decision; drop the rest. Aim for 150–350 words (about 1–2.5 minutes). If the source is long, summarise and say "ask me for the detail on any part".
   - No markdown, bullets, tables, emoji, code, URLs, file paths or issue numbers. Say what they are in words ("a command that starts LM Studio's server").
   - Spell symbols out: "$100" → "a hundred dollars", "64GB" → "sixty-four gigs", "✅ / ❌" → "allowed / not allowed", "e.g." → "for example".
   - Signpost out loud: "Three things." "First…" "Second…". Short sentences.
   - End with the one next action.
3. **Save:** write the script with a quoted heredoc so nothing expands:
   ```bash
   cat > "$TMPDIR/speakit.txt" <<'EOF'
   ...script...
   EOF
   ```
4. **Play** in the background so it doesn't block (dsh: a background job or a trailing `&`; Claude Code: Bash `run_in_background: true`). Local Voxtral voice, streamed so it starts talking straight away:
   ```bash
   pkill -f mlx_audio.tts.generate; killall say afplay 2>/dev/null
   HF_HUB_OFFLINE=1 mlx_audio.tts.generate --model mlx-community/Voxtral-4B-TTS-2603-mlx-4bit \
     --voice "${SPEAKIT_VOICE:-casual_female}" --stream --play \
     --output_path "$TMPDIR" --file_prefix speakit < "$TMPDIR/speakit.txt"
   ```
   If that fails (mlx-audio or the model missing on this machine), fall back to `/usr/bin/say -r 190 -f "$TMPDIR/speakit.txt"`. Setup on a new Mac: `uv tool install 'mlx-audio[tts]'`, then run it once without `HF_HUB_OFFLINE` to download the model.
5. **Reply** with one line only: `Speaking (~N min). Stop: pkill -f mlx_audio`. Don't paste the script unless asked.

## Variants

- "again" / "replay" → re-run step 4 on the saved file.
- "slower" / "faster" → add `--speed 0.9` / `--speed 1.15`.
- "voice <name>" → `SPEAKIT_VOICE=<name>`. Options: casual_female (default), casual_male, neutral_female, neutral_male, cheerful_female.
- "stop" → `pkill -f mlx_audio.tts.generate`.
- "save it" → drop `--stream --play`, add `--join_audio --audio_format wav --output_path ~/Downloads`.
