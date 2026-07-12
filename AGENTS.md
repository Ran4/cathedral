If i refer to screenshots,
they're written to:

screenshots/session_X/cathedral_screenshot_YYYY-MM-DD_HH_MM_SS.png
screenshots/cathedral_screenshot_latest.png <- this is usually what I'll talk about

## Agent drive mode (verify changes without xdotool)

To verify a change in the running game, do NOT use xdotool/XTEST (winit never
sees synthetic core events). Instead script the game from the inside with
`CATHEDRAL_DRIVE`, a `;`-separated action list. The game runs windowed
1280x720, executes the actions (~0.5 s apart), prints a `[drive] 3.2s key
Escape` evidence line per action to stdout, and exits on its own — no `pkill`
needed.

Actions: `key <KeyCode>` (e.g. `Escape`, `KeyZ`, `F5`), `click <Name
substring>` (case-insensitive match on UI `Name`), `shot <name>` (PNG to
`screenshots/drive/<name>.png`), `sleep <seconds>`, `wait-online` (until the
actor sidecar is ready; 30 s timeout), `quit`. Without `quit` the game exits
~2 s after the last action; a watchdog aborts after `CATHEDRAL_DRIVE_TIMEOUT`
seconds (default 60).

Example — open the settings menu, switch the STT pill, close it, exit:

```sh
CATHEDRAL_DRIVE='wait-online; key Escape; shot menu_open; click Local Canary-Qwen; shot stt_switched; key Escape; shot menu_closed; quit' cargo run
```

`CATHEDRAL_SHOT=<name> cargo run` is shorthand for a single
`sleep 2; shot <name>; quit`. For runs without network/API keys set
`smart_actors.fake_backend: true` in config.ron. NOTE: drive scripts exercise
the real handlers, so e.g. clicking a backend pill persists to config.ron —
back it up first if that matters.
