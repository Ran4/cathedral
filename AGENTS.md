# The Cathedral-City of Impossible Light

A first-person, procedural cathedral-city inspired by the monumental engraving
in `docs/reference_image.png`. The scene is assembled entirely in Rust with
Bevy 0.19 and uses original generated material artwork for cathedral limestone,
weathered city plaster, half-timber infill, dark fieldstone, terracotta and
slate roofs, and the rose window.

The cathedral opens into a roughly 1.2 × 1.0 km fortified medieval city. Most
streets pinch and change width between independently offset façades; each block
contains a 4.6 m route that doglegs twice, lateral alleys, projecting upper
floors, covered passages, small courts, and frequent overhead bridges.
Those dense quarters open selectively into five town squares, markets, a canal
and bridges, secondary churches and towers, and the cathedral's ceremonial
forecourt. For developer playtesting, "flying" support makes the full skyline explorable.

## Smart actors

NPCs are LLM-driven "smart actors". The authoritative simulation is a Python
sidecar (`prompt_playgound/server.py`) that the game launches itself via `uv`
and that owns world state, prompts, and action parsing; the Bevy side
(`src/smart_actors/`) is only a non-blocking projection. Rust and Python speak
a version-1 JSON-lines protocol over the child's stdin/stdout
(`src/smart_actors/bridge.rs`); Bevy reconciles a `WorldMirror` from
authoritative snapshots and events, requests a resync on any sequence gap, and
blocks player commands until the replacement snapshot lands.

The sidecar reports three independent capabilities at handshake: LLM cognition,
player speech-to-text (cloud OpenAI gpt-4o-transcribe or local Canary-Qwen),
and NPC voices (local streaming Pocket TTS, cloud OpenAI, or off). Each
degrades independently — a missing API key never takes the others down. The
Esc settings menu switches STT/TTS backends at runtime and persists the choice
to `config.ron`; the X key cycles the NPC voice backend.

Everything is configured in `config.ron` under `smart_actors: (...)`. For runs
without network or API keys, set `fake_backend: true` — a deterministic
offline mode also used by the integration tests. Python-side details (domain
model, actions, prompt format, the terminal prototype) live in
`prompt_playgound/AGENTS.md`.

----------

If i refer to screenshots,
they're written to:

screenshots/session_X/cathedral_screenshot_YYYY-MM-DD_HH_MM_SS.png
screenshots/cathedral_screenshot_latest.png <- this is usually what I'll talk about

## Agent drive mode

To verify a change in a running game, see .claude/rules/CATHEDRAL_DRIVE.md
(do NOT use xdotool/XTEST (winit never sees synthetic core events).
