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
forecourt. Flight makes the full skyline explorable.

## Run

```sh
cargo run --release
```

The first build is large because Bevy and its renderer compile from source.

## Configuration

The app starts in borderless fullscreen by default. Edit [config.ron](config.ron)
to change the window or smart-actor sidecar settings. Smart actors start through
`uv` and degrade to an offline HUD state if Python, a provider key, a microphone,
or a speech service is unavailable; the cathedral remains playable. Set
`smart_actors.enabled` to `false` to disable the sidecar entirely. If the file
cannot be read or parsed, the app safely uses the same defaults.

Set `smart_actors.fake_backend` to `true` for the deterministic offline cast:
it uses no provider credentials or network calls while exercising the same
persistent process, protocol, world rules, HUD, and interaction paths.

## Controls

- `W A S D` — walk (8 m/s)
- Hold `Shift` — run (12 m/s)
- Mouse — look
- `Space` — jump
- `F` — toggle gravity-free flight
- `Space` / `Ctrl` — rise / descend while flying
- `Esc` — release the mouse
- Left click — recapture the mouse
- `F5` / `´` — save a PNG to
  `screenshots/session_<session>/cathedral_screenshot_<timestamp>.png` and
  overwrite `screenshots/cathedral_screenshot_latest.png`
- `V` — toggle the microphone on/off (on by default); speech is heard openly
  by every actor within 20 m. Recognized speech appears in tiny text near the
  bottom with a nearby-recipient count
- `Z` — toggle player transcription between the configured cloud model and
  local `nvidia/canary-qwen-2.5b` in FP16. The first local utterance installs
  the isolated NeMo environment and downloads about 5 GB of model weights;
  later utterances reuse the GPU-resident model
- Mouse wheel / `1`–`9` — select an inventory item
- Right click — offer the selected item to the focused actor
- `Y` / `N` — accept or decline the active incoming offer
- `R` — retract the selected item's pending offer

Each game start increments `session` in `cathedral_meta.json` once. Every
screenshot taken during that run uses the resulting session directory.

Walking uses acceleration, friction, air control, gravity, collision, coyote
time, and buffered jumping. Flying disables gravity but deliberately keeps
collision enabled so the architecture still has physical presence.

## Tests

All smart-actor tests are offline; the fake sidecar never contacts a provider:

```sh
cargo fmt --check
cargo clippy --all-targets --offline -- -D warnings
cargo test --offline
uv run --with openai --with python-dotenv \
  python -m unittest discover -s prompt_playgound/tests -v
```
