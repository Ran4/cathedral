---
name: generate_explanation_video
description: This skill should be used when the user wants to create a narrated, animated "explainer"/"walkthrough"/"explanation" video about part of this project — a spec/feature document, a piece of lore, or a subsystem of the game code. Produces a 1080p MP4 with British-storyteller voiceover and hand-drawn SVG motion graphics, timed to the narration.
---

# Generate an explanation video

Produce a narrated, animated explainer video (1080p MP4) about any part of the
project — a `features/**` spec, a `lore/**` document, or a slice of the game
code. The reference build that this skill was distilled from is committed at
`architecture/movies/the_supply_chain.mp4` (a ~11 min walkthrough of
`features/food_and_items/07_the_supply_chain.md`).

The output is **not** edited video. It is a self-contained HTML page whose every
frame is a pure function of a timeline position, screenshotted frame-by-frame
through headless Chromium and muxed with ElevenLabs narration. That design is
what makes it deterministic, resumable, and QA-able frame by frame.

## The two load-bearing decisions

1. **Audio first.** Synthesize the narration, then measure each clip's exact
   duration with `ffprobe`. Those measured durations lay out the global timeline
   (`timings.json`). The visuals are timed to the voice, never the reverse — so
   nothing is ever out of sync with what's being said.
2. **Every frame is `renderAt(t)`.** The animation exposes one pure function:
   given a timestamp, it deterministically positions every element. No
   `requestAnimationFrame`, no wall clock, no hidden state. This is why frames
   can be rendered in any order, why the render is resumable, and why the
   **probe/QA loop** below works.

## Prerequisites (already present on this machine)

- `ffmpeg` / `ffprobe`, `uv` (runs the Python scripts as inline-dependency
  scripts), and Playwright's cached Chromium (`uv run` pulls `playwright`).
- API keys in the repo-root `.env`: `ELEVENLABS_API_KEY` (voice) and
  `OPENAI_API_KEY`. `tts.py` auto-discovers `.env` by walking up from the CWD,
  or set `CATHEDRAL_ENV=/path/to/.env`.
- No cairo/pango — **manim is not available**; this HTML→Chromium→ffmpeg
  pipeline is the supported path. Fonts: DejaVu (Sans / Serif / Sans Mono),
  Liberation, Noto — the design uses DejaVu.

## Files in `assets/` (the reusable harness)

Copy these into a working directory (use the session scratchpad, not `/tmp`
directly). Reuse the first four **verbatim**; author the last two per video.

| file | role | change per video? |
|---|---|---|
| `engine.js` | timeline framework: easing, `A()`/`window_()`/`pulse()`, dom+svg helpers, `glyph()`, `defScene()`, the `renderAt(t)` render loop and crossfades | no |
| `render.py` | Playwright→ffmpeg frame renderer; `--probe` keyframe mode | no |
| `tts.py` | ElevenLabs synthesis + `ffprobe` duration measurement → `timings.json` | rarely (voice/model) |
| `mix.py` | lays each clip at its start, mixes, levels, fades, muxes onto the video | no |
| `anim.html` | page shell + full design system (colors, type, `.code`, chrome). Edit the `<title>`, the two `#foot-label`s, and the `<script src="scenes*.js">` list | light edits |
| `scenes.example.js` + `narration.example.json` | a 3-scene template (title / flow / code) showing the patterns | **replace** |

## Procedure

### 1. Read the source and design the arc

Read the whole document/subsystem. Then write `narration.json`: an array of
scenes `{id, title, text}`. Guidelines that made the reference work:

- **Order by argument, not by section number.** Lead with the problem, then the
  mechanism, then the consequences. (The reference reorders the spec into:
  cheats → chain → presence → boundary → transforms → money → milestones.)
- **~15–40 s of speech per scene**; 15–25 scenes for a feature-sized doc.
- **`text` is spoken aloud** — full sentences, natural rhythm, **no** markdown,
  code punctuation, bullet fragments, or symbols the voice would mangle. Spell
  out `→` as "becomes", `u32::MAX` as "the u32 maximum", etc.
- `title` is the short chrome label shown top-left during the scene.
- Keep `id`s stable and prefixed (`s01_…`) — they key both audio and scenes.

Pull exact names from the code, don't paraphrase them (grep the enums, ids,
place names, office/weekday vocabulary). Wrong identifiers read as sloppy.

### 2. Synthesize audio and lock the timeline

```sh
cd <workdir> && uv run tts.py
```

Writes `audio/<id>.mp3` for each scene (idempotent — existing clips are skipped)
and `timings.json` (per-scene measured `dur` + computed `start`/`end`, plus
`total`). Neighbour-aware prosody is on (`previous_text`/`next_text`). Voice is
"George" (British storyteller); change `VOICE`/`MODEL` in `tts.py` if wanted —
[list voices](https://api.elevenlabs.io/v1/voices) with the key first.

### 3. Author the scenes

Create `scenes.js` (split into `scenes2.js`, … if it gets large; list each in
`anim.html`). One `defScene(id, build)` per narration id. `build(root)` runs
once and returns `{ update(lt) }`, where `lt` is seconds since the scene's
narration began. **Everything visual must be a pure function of `lt`.** Use the
`assets/scenes.example.js` patterns (fade-in with `rise()`, arrows that draw on
with `A()`, a travelling token, monospaced `.code` blocks with `.add`/`.del`
diff spans). Define project-specific `glyph()` kinds for recurring nouns
(the reference draws grain sacks, flour, loaves, wool, spark coins, people).

Timing tip: read the scene's `text` and place reveals at the fraction of the
clip where the narration reaches that idea (clip length is `dur` in
`timings.json`).

### 4. Probe before you render (do not skip)

You cannot watch video, and a full render is ~15 min. So screenshot one keyframe
per scene as PNGs you *can* inspect:

```sh
uv run render.py --probe                    # one frame ~72% through each scene
uv run render.py --probe --probe-at 40 210  # or specific timestamps
```

Read every probe PNG. Hunt for **layout collisions** (overlapping blocks — the
most common defect), text overrun past the 1920×1080 safe area, and **stray SVG
artifacts** (an arrow drawn at zero length shows a floating arrowhead — gate it
with `opacity` when its progress is 0). Fix, re-probe, and only then render. In
the reference build this loop caught eight collisions before the first render.

### 5. Render, mix, deliver

```sh
uv run render.py            # 1920x1080 @ 30fps -> video_only.mp4 (silent)
uv run mix.py               # timeline audio -> narration.wav, then mux -> the_supply_chain.mp4
```

`render.py` pipes JPEG frames straight into `libx264` (nothing hits disk) and
prints progress; it aborts on any page/console error. Rename the output as
appropriate. If a render is interrupted, re-run — it is deterministic.

Deliver with `SendUserFile` (`display: 'render'`). **The chat upload limit is
30 MiB**; if the master exceeds it, re-encode a smaller copy *for delivery only*
and keep the master:

```sh
ffmpeg -y -i master.mp4 -c:v libx264 -preset slow -crf 24 -c:a aac -b:a 160k \
  -movflags +faststart delivery.mp4
```

### 6. (If asked) commit

Videos are large binaries → **git LFS**. `.gitattributes` already routes
`*.mp4 *.png *.mp3 *.wav` to LFS. Put the file under `architecture/movies/`,
`git add`, confirm `git lfs status` shows it under LFS (staged blob is a ~130 B
pointer), then commit. Follow the repo's commit-message trailer convention.

## Adapting the source type

- **Spec / feature doc** — the reference case. The doc already argues a design;
  re-sequence it and lift exact identifiers.
- **Lore** (`lore/**`) — favour mood over structure: a warmer script, fewer code
  blocks, more `glyph()`/scene-painting. Drop the `.code` scenes.
- **Game code** — first map the subsystem (grep the module, its public types,
  the call seam). Narrate *what it does and why*, showing real `struct`/`fn`
  signatures in `.code` blocks and pointing at the one field that matters rather
  than reading the whole thing.

## Scope & cost notes

- ElevenLabs charges per character synthesized (~cents for an 11-min script).
  Re-running `tts.py` reuses existing clips — delete a clip to re-voice just
  that scene.
- Match length to the ask: a feature doc → 8–12 min; a quick overview → 3–5 min
  (fewer scenes, tighter script) from the same harness.
- The working directory (audio, frames, wav) is large and ephemeral. Only the
  final `.mp4` (and, if wanted, `narration.json` + `scenes*.js`) is worth
  keeping.
