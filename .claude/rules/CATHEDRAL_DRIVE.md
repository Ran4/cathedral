To verify a change in a running game do NOT use xdotool/XTEST (winit never sees synthetic core events).

Instead script the game from the inside with
`CATHEDRAL_DRIVE`, a `;`-separated action list env argument.
The game runs windowed 1280x720, executes the actions (~0.5 s apart),
prints a `[drive] 3.2s key Escape` evidence line per action to stdout,
and exits on its own — no `pkill` needed.

Actions: `key <KeyCode>` (e.g. `Escape`, `KeyZ`, `F5`), `type <text>` (inject
text into the Enter chat box as a raw keyboard message; `;` cannot appear in
the text), `click <Name substring>` (case-insensitive match on UI `Name`),
`shot <name>` (PNG to
`logs/latest_session/screenshots/<name>.png`), `sleep <seconds>`, `wait-online` (until the
actor engine is ready; 30 s timeout), `sound <sound_id>` (emit a catalog
world sound, e.g. `sound town_bell` — the stand-in trigger for world causes
the sim lacks), `tp <x> <y> <z> [yaw_deg [pitch_deg]]` (teleport the player
and aim the view — yaw 0 looks toward -Z, positive pitch looks up; switches
to flying so an elevated vantage holds for a `shot`), `quit`. Without `quit` the game exits
~2 s after the last action; a watchdog aborts after `CATHEDRAL_DRIVE_TIMEOUT`
seconds (default 60). The `[drive]` lines are also mirrored into the
session's `logs/latest_session/logs.jsonl` (source `"drive"`).

Example — open the settings menu, switch the STT pill, close it, exit:

```sh
CATHEDRAL_DRIVE='wait-online; key Escape; shot menu_open; click Local Canary-Qwen; shot stt_switched; key Escape; shot menu_closed; quit' cargo run
```

`CATHEDRAL_SHOT=<name> cargo run` is shorthand for a single
`sleep 2; shot <name>; quit`. For runs without network/API keys set
`smart_actors.fake_backend: true` in config.ron. NOTE: drive scripts exercise
the real handlers, so e.g. clicking a backend pill persists to config.ron —
back it up first if that matters.
