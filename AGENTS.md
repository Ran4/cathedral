# The Cathedral-City of Impossible Light

A first-person, procedural cathedral-city inspired by the monumental engraving in `docs/reference_image.png`.
The scene is assembled entirely in Rust with Bevy 0.19 and uses original generated material artwork for
cathedral limestone, weathered city plaster, half-timber infill, dark fieldstone, terracotta and slate roofs,
and the rose window.

The cathedral opens into a roughly 840 × 700 m fortified medieval city. Most streets pinch and change width
between independently offset façades; each block contains a 4.6 m route that doglegs twice, lateral alleys,
projecting upper floors, covered passages, small courts, and frequent overhead bridges. Those dense quarters
open selectively into five town squares, markets, a canal and bridges, secondary churches and towers, and the
cathedral's ceremonial forecourt. For developer playtesting, "flying" support makes the full skyline
explorable.

## Smart actors

NPCs are LLM-driven "smart actors". The authoritative simulation is **`crates/cathedral-sim`**: a pure,
IO-free Rust crate that owns world state, the prompt format, the action parser and the NPC turn scheduler. It
runs **in-process**, pumped once per frame by `src/smart_actors/local_engine.rs`; the rest of
`src/smart_actors/` is a non-blocking projection of it (there is no sidecar and no wire — the engine hands the
game typed `cathedral_sim::EngineMessage`s, and `model::WorldMirror` projects the snapshots the ECS reads).

Everything impure — the provider HTTP client, the speech workers, the prompt archive, the private audio
directory — lives in **`crates/cathedral-backends`**. The sim calls it through the `Cognition` /
`Transcription` / `Tts` traits and gets results back as plain values, so the sim itself has no clock, no
threads and no filesystem. Domain details (the world model, the action verbs, the turn loop, the "unknown
people" rule) are in `crates/cathedral-sim/AGENTS.md`.

Three capabilities are probed at startup and reported independently: LLM cognition, player speech-to-text
(cloud OpenAI gpt-4o-transcribe or local Canary-Qwen), and NPC voices (streaming local Pocket TTS, streaming
cloud OpenAI, or off). Each degrades on its own — a missing API key never takes the others down. The Esc settings
menu switches STT/TTS backends at runtime and persists the choice to `config.ron`; the X key cycles the NPC
voice backend.

LLM turns are spent only on NPCs the player can see, hear or talk to: the idle rotation is gated on the
player's neighborhood (`crates/cathedral-sim/src/attention.rs`), while speech and sounds still reach anyone,
anywhere. `config.ron: smart_actors.idle_cognition.mode` switches between `"stage"` and the old city-wide
clock (`"all"`) without a rebuild.

Once a game day the cast also **sleeps on it** — the Night Office
(`crates/cathedral-sim/src/night.rs`). At their own bedtime a Major reflects on the day and may settle a
memory, change what they are set on, and move one leg of tomorrow with `set_round`; the Minors are batched
one prompt per ward at the curfew, which returns a mood every Minor of that ward then carries; the ambient
cast's evenings are re-rolled in code for no tokens at all. ~38 provider calls a game day, on a **second**
cognition lane (`Cognition::request_night`, its own capacity of one) that yields absolutely to the player —
it never submits while anyone is on stage with you, while a line is being presented, while the microphone is
open, or while a reply is owed, and a night it runs out of drops silently. `config.ron:
smart_actors.night_office` turns each tier off; `cathedral-headless --night-office` runs one in a terminal.

Everything is configured in `config.ron` under `smart_actors: (...)`; secrets stay in `prompt_playgound/.env`
(real environment variables win over it). For runs without network or API keys, set `fake_backend: true` — a
deterministic offline mode also used by the integration tests.

The two local speech models still run as `uv` subprocesses; `prompt_playgound/` is now nothing but those two
workers and their `.env`.

### The crowd knob

`config.ron: smart_actors.extra_ambient_npcs` (0..=20000, default 0, `CATHEDRAL_EXTRA_NPCS=n` for one run)
generates that many extra ambient citizens and spreads them over the walkable graph
(`crates/cathedral-sim/src/crowd.rs`). They are **not cast**: six-character ids (`x00000`…, so they cannot
shadow a five-character lore id), no authored sheet, no bed in `homes.json`, strangers to the player, and
barred from the one civic post the round hands to whoever is standing nearest — the well curbs stay the
cast's. Everything else about them is ordinary: an occupation, a ward, a daily round, a purse, a walk.

They cost no tokens by existing — the stage cap and the single in-flight cognition slot bound the spend
however many people are about — but they do change who is *nearest*, and therefore who the idle rotation
picks. What they cost is frames: measured at 1280x720, p50 frame time 7.2 ms at 0, 16.8 ms at 2000, 36 ms at
5000, 204 ms at 20000. Past ~2000 the dominant cost is the engine pump (the sim), not the puppets.
`cargo run -p cathedral-backends --bin cathedral-headless -- --extra-ambient n` measures that half alone.

### Running the sim without Bevy

The whole cast plays out headlessly, which is the fastest way to change the
prompt, the scheduler or an action verb and see what it does:

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- --fake -t 6    # offline, instant
cargo run -p cathedral-backends --bin cathedral-headless -- -t 10 -v       # live provider, full prompts
cargo run -p cathedral-backends --bin cathedral-headless -- --fake -t 6 --stage  # gate idle turns on proximity
cargo run -p cathedral-backends --bin cathedral-headless -- --one-shot FILE  # send one file, print the reply
# a whole game night in a few seconds: 30 majors, 8 wards, the ambient roll
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --night-office --start-office waning --seconds-per-day 300 --watch-clock 0.6
```

stdout is the transcript, the final world state and the run cost in USD;
diagnostics (and, with `-v`, the prompts and raw replies) go to stderr.

## Agent drive mode

To verify a change in a running game, see .claude/rules/CATHEDRAL_DRIVE.md (do NOT use xdotool/XTEST (winit
never sees synthetic core events).

Run those scripts with **`CATHEDRAL_HEADLESS=1`**: the window is created but never mapped, so the game renders
and screenshots exactly as usual while nothing appears on screen, takes the focus, grabs the pointer or makes
a sound. Somebody is usually working at that desktop.

```sh
CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 \
  CATHEDRAL_DRIVE='wait-online; tp 0 40 200 180 -12; sleep 2; shot skyline; quit' cargo run
```

## Logs

Automatically written:

```
logs/
    latest_session  # symlinks on game start to e.g. session_46_2026-07-13_10_28_52
    session_<n>_<YYYY-MM-DD_HH_MM_SS>/   # run counter, local start time
        logs.jsonl
        screenshots/
        prompts/
```

See `.claude/rules/LOGS_FOLDER.md` for more info.

## Lore

Extensive lore, in markdown format (as well as inspiration images generated by
scripts/generate_lore_inspiration_images.py) is found at `lore/`.

Note: most of the things that is part of the lore isn't part of the game in any way!

## Backlog

Found in `features/`, see features/AGENTS.md

----------

## Session logs

Every game start creates a session directory (the counter lives in `cathedral_meta.json`) and repoints the
`logs/latest_session` symlink at it — that's usually what I'll talk about:

```
logs/
    latest_session -> session_35_2026-07-13_10_12_02
    session_35_2026-07-13_10_12_02/        # session 35, started 2026-07-13 10:12:02
        logs.jsonl                         # structured logs, one JSON object per line:
                                           #   game (source "rust"), actor-engine diagnostics
                                           #   ("engine"), speech-worker stderr ("stt"/"tts"),
                                           #   drive evidence lines ("drive")
        screenshots/
            cathedral_screenshot_2026-07-13_10_14_31__00.png   # __nn counts up within a second
            <name>.png                     # named drive-mode `shot` captures
        prompts/                           # every LLM exchange, written by the engine's host
            2026-07-13_10_12_45__00__k0fb1__Ilse_prompt.md     # Prompt / Answer / Meta sections
            2026-07-13_10_12_45__00__k0fb1__Ilse_prompt.json   # same data: {prompt, answer, meta}
```

If I refer to screenshots, F5 captures are the timestamped files in
`logs/latest_session/screenshots/`.
