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
to change `fullscreen`, the window dimensions, title, or whether windowed mode
can be resized. If the file cannot be read or parsed, the app safely uses the
same fullscreen defaults.

## Controls

- `W A S D` — walk (8 m/s)
- Hold `Shift` — run (12 m/s)
- Mouse — look
- `Space` — jump
- `F` — toggle gravity-free flight
- `Space` / `Ctrl` — rise / descend while flying
- `Esc` — release the mouse
- Left click — recapture the mouse

Walking uses acceleration, friction, air control, gravity, collision, coyote
time, and buffered jumping. Flying disables gravity but deliberately keeps
collision enabled so the architecture still has physical presence.
