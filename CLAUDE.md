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

`config.ron: smart_actors.extra_ambient_npcs` (0..=20000, default 0,
`CATHEDRAL_EXTRA_NPCS=n` for one run) requests extra persistent ambient
residents. Generation lives in `crates/cathedral-sim/src/crowd.rs`; their local
routine lives in `round/residents.rs`. They have six-character ids
(`x00000`…), personal identities, inventories and knowledge, and remain
interactable strangers. They have no authored character file.

**Generated residents have no job by default.** Occupation, title and rank
are null; housing and livelihood are separate choices. About three quarters
belong to supported households with nearby doors, while the existing
approximate hardship quarter has local support and no settled household
door. Ordinary residents use varied civilian clothes. A generated worker
requires an explicit occupation-and-workplace override through the generation
API; shipped hosts provide no worker overrides. Authored actors, including
authored ambient workers, retain their jobs and daily rounds.

Residents start at individually reserved standing spots beside building
frontages, courts and square edges. These positions are baked into
`navigation.json: resident_places`, with verified local routes and protection
for walls, doors, service areas and through-routes. Allocation balances
patches and household doors and excludes existing bodies. The generator
reports **requested, placed and unplaced** counts; the request ceiling is not
a promise that 20,000 people fit. Exhausting safe capacity leaves excess
actors unspawned. The current bake places all 1,000/2,000 requests and 9,072
of a 20,000 request after accounting for the authored starting bodies.

A resident normally lingers for 45–150 elapsed simulation seconds and may
then change position within the same frontage. Optional walks use exact
destinations, reservations and a 15 m routed budget, with at most 10% of
eligible residents admitted at once and one optional departure per small
patch. Arrival starts another dwell. A failed route or unavailable place
means waiting. Residents do not inherit the occupational recall radius,
generic wander roll or a city-wide workplace commute.

Household or neighbourhood provision satisfies needs locally at meal
offices, including breakfast at the Kindling. Hunger remains real, held food
can still be eaten, and missed meals have bounded local recovery. This is
abstract hearth support: it creates no saleable items, vendor transactions
or wages. Default residents do not join public food or water queues or take
the cast's well-keeper and vendor posts.

At night residents rest at safe places beside their household or familiar
frontage, with individual morning/evening timing. A home connector is an
access route, not a shared stopping coordinate. There is no authoritative
indoors state: residents remain present bodies, and resting is reported as
an outdoor proxy. The generated routine does not roll an evening destination
at a distant tavern or ward sign.

Standing residents retain existing speech, curiosity, gestures and
interaction rules. Explicit travel, conversation, weather and custody use
the simulation's ownership rules. Their presence does not add an LLM
scheduler lane: stage capacity and the existing cognition slots still bound
provider work.

Local rain shelter uses individually reserved positions under actual nearby
roofs. Residents without available local cover remain exposed; a household
assignment does not count as being indoors. Street lanes use measured
clearance and checked connections. Invalid imported starts for mobile authored
actors are corrected to nearby safe ground before the world is first shown;
their jobs, rounds and destinations remain intact.

`cathedral-headless --trace-motion` measures actual displacement and its
cause separately for generated and authored populations, including queues,
hunger and the full present-population denominator. It uses 0.05-second
watch-clock polls so movement, needs and calendar time advance consistently.
The older `at_post` census is not a stationary count. The ambient residents
two-day probes at 1,000 and 2,000 residents average about 99.06% stationary in
settled daytime samples, with the full populations present. Mean pump/report
cost is 4.52/8.97 ms per normal poll; the earlier one-day baseline was
2.90/6.55 ms. The [implemented spec](features/implemented/ambient_residents.md)
and [acceptance evidence](features/implemented/ambient_residents_evidence/m4.md)
record geometry, behaviour and frame measurements, including that CPU tradeoff.

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
