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
(cloud OpenAI gpt-4o-transcribe or local Canary-Qwen), and NPC voices (streaming local Pocket TTS, streaming cloud OpenAI, or off). Each degrades on its own — a missing API key never takes the others down. The Esc settings
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
residents.

See `.claude/rules/THE_CROWD_KNOB.md` for more info.
