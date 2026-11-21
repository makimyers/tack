---
name: podcast-me
description: Use when the user says "podcast me" or asks to turn dense material (a research report, a plan, a long answer, a document) into a one-episode podcast, a script to listen to, something to read aloud with text-to-speech, or an audio version they can listen to without feeling overwhelmed.
---

# Podcast me

Turn dense material into a one-episode, single-narrator podcast the user can listen to (or read aloud with
text-to-speech) without feeling overwhelmed.

## What to work from

- If the user names or attaches something, use that.
- Otherwise use the most substantial content in the current conversation (a research report, a plan, a long answer).
- If there's nothing to work from, ask in one line what the episode should cover.
- If a source is a long research report whose text you haven't seen, read it first.

## Output

- Reply in plain text in the chat. Don't create files, artifacts or audio unless the user asks for them.
- Aim for about 8–12 minutes spoken (roughly 1,200–1,800 words). Go shorter if the material is thin. Never pad.
- Give it a short title in bold, then the script.

## Structure

1. Warm open (2–3 sentences). Address the user by name if you know it. Tell them they don't need to take notes.
2. Promise. Say exactly what they'll know by the end, as a small number of things ("By the end of this, you'll
   know four things..."). Then deliver those things.
3. 4–6 parts. Give each a bold heading in the form "Part one: plain-English title". Each part makes one main point.
4. Close. Give one sentence to remember ("If you remember one thing from this, make it this..."). Then give the
   single first action they should take. End with "Thanks for listening."

## Voice and pacing

- Write for the ear. Use short sentences, one idea per sentence, and plain English. Use "you" and "I" naturally.
- Give the bottom line early, then the reasons behind it.
- Use light signposting: "Here's the part that matters." "There's one catch." "Let's talk money, briefly."
- Tell one concrete story or example per part where possible, such as a named person, company or case. Don't
  stack several.
- Be warm but honest. Say bad news plainly, and say good news plainly too. No hype and no flattery.
- Connect the material to the user's own situation when it changes the advice.

## Numbers

- Keep only the numbers that change a decision. Drop the rest.
- Round them and say them as a person would: "about forty to sixty thousand dollars a month", not "$40–60K/mo".
- Use at most two or three numbers in a row. Spread them out.
- Label estimates as estimates, briefly: "That's my best guess, not a hard statistic."
- Keep key dates and thresholds exact where they matter.

## Avoid

- Tables, code blocks, citations, URLs and footnotes.
- Long bullet lists. A short list of up to 4 items is fine only for things the listener would want to tick off:
  timeline milestones, admin steps, or stop/pivot checkpoints.
- Jargon without a one-line plain explanation.
- Re-covering everything in the source. Pick what matters most and leave out the rest, even if it's interesting.
- Saying "in this podcast" repeatedly, meta-commentary about the source, or describing how you made the script.

## If they want audio

If the user asks for an actual audio file, offer to generate one and confirm the voice and accent first. Script
and accent should match: British English for UK users, for example. Otherwise stay in text.

On a Mac, the local Voxtral voice (the one `speakit` uses) records it offline on the GPU.

- Voices: casual_female, casual_male, neutral_female, neutral_male, cheerful_female. Write the finished script
  (title and part headings included) to `script.md`, then:
  `HF_HUB_OFFLINE=1 mlx_audio.tts.generate --model mlx-community/Voxtral-4B-TTS-2603-mlx-4bit --voice <voice> --join_audio --audio_format wav --output_path . --file_prefix <title> < script.md`
- Save the WAV in the working folder unless they name another place, and give them its path.
- If `mlx_audio.tts.generate` does not exist, no voice tool is installed. Say so (setup: `uv tool install
  'mlx-audio[tts]'`) and give them the script as text.
