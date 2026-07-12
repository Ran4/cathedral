# Agent drive mode — in-process verification hooks

## Why

Claude (and CI) can't reliably verify changes by poking at the running game
from outside:

- `xdotool key --window` uses XSendEvent; winit reads keyboard via XInput2 and
  never sees synthetic core events. Keys are silently dropped.
- XTEST injection needs window focus, a cooperating WM, and steals the user's
  pointer/keyboard.
- The only screenshot path is a human pressing F5 (`screenshot.rs`).
- The game never exits on its own, so every run needs a `pkill`.

`~/src/rust/play/quakeclone` solves this with env-var hooks baked into the
game (`QC_SHOT`, `QC_GALLERY`, `QC_AUTOTEST`, …): the game drives itself,
screenshots itself via Bevy's `Screenshot` API, and exits. No focus, no WM,
no synthetic input. This spec ports that idea here.

## What

One env var, `CATHEDRAL_DRIVE`, holding a `;`-separated script of actions.
When it is set:

- The window is forced to windowed 1280x720 regardless of `config.ron`
  (quakeclone's "preview" mode — small, fast, WM-friendly).
- Actions run in order, spaced ~0.5 s apart by default (always at least one
  frame apart).
- Every action logs to stdout when it fires: `[drive] 3.2s key Escape` — the
  log is the evidence trail.
- When the env var is absent there is **zero** behavior change.

### Actions

| Action | Meaning |
|---|---|
| `key <KeyCode>` | Inject one press+release of a Bevy `KeyCode` (e.g. `Escape`, `KeyZ`, `KeyX`, `F5`) into `ButtonInput<KeyCode>`. All existing keybindings work unchanged — menu, mic toggle, screenshot key, everything. |
| `click <name substring>` | Find the UI entity whose `Name` contains the substring (case-insensitive) and set `Interaction::Pressed` for one frame. Lets scripts click `Continue` or `Pill: Local Pocket TTS` without coordinates or a pointer. No match → log a warning, continue. |
| `shot <name>` | Screenshot the primary window to `screenshots/drive/<name>.png` (create the dir; overwrite silently). Reuses the same `Screenshot::primary_window()` + `save_to_disk` machinery as F5. |
| `sleep <seconds>` | Extra delay before the next action (fractional ok). |
| `wait-online` | Pause the script until `SmartActorRuntime::interactions_enabled()` (actor sidecar connected and ready). Timeout after 30 s → log an error and continue anyway. |
| `quit` | Graceful `AppExit::Success`. **Not** `std::process::exit` — the bridge must get its normal `Shutdown` so the Python sidecar dies with the game. |

If the script ends without `quit`, quit automatically ~2 s after the last
action. A watchdog aborts with a nonzero exit code if the whole run exceeds
`CATHEDRAL_DRIVE_TIMEOUT` seconds (default 60), so hung runs can't strand a
background process.

### Shorthand

`CATHEDRAL_SHOT=<name>` (mirroring quakeclone's `QC_SHOT`) is sugar for
`CATHEDRAL_DRIVE="sleep 2; shot <name>; quit"`.

### Example

```sh
CATHEDRAL_DRIVE='wait-online; key Escape; shot menu_open; click Local Canary-Qwen; shot stt_switched; key Escape; shot menu_closed; quit' cargo run
```

produces three PNGs under `screenshots/drive/` showing the settings menu
opening, the STT pill switching, and the menu closed again — then exits 0
with the sidecar shut down.

For runs without API keys / network (CI), combine with
`smart_actors.fake_backend: true` in the config.

## Implementation notes

- New top-level module (e.g. `src/drive.rs`) with a plugin that `main.rs`
  adds only when `CATHEDRAL_DRIVE`/`CATHEDRAL_SHOT` is set. The windowed
  override happens in `main.rs` where the `Window` is built.
- Script is parsed once at startup; a parse error should fail fast with a
  clear message naming the bad token, not run half a script.
- Key injection: call `ButtonInput<KeyCode>::press` in `PreUpdate` **after**
  Bevy's input systems (which clear `just_pressed` each frame); release the
  key the following frame. Physical `KeyCode` is enough — the F5 screenshot
  path checks physical keys.
- Click injection: write `Interaction::Pressed` after `ui_focus_system` runs
  in `PreUpdate` so it isn't immediately overwritten; the real focus system
  naturally resets it to `None` the next frame, which also produces the
  `Changed<Interaction>` transitions the handlers expect.
- `shot` must wait for the GPU readback to finish before the script advances
  to `quit`, or the PNG never lands (screenshot saving is async).
- Unit-test the parser and the action scheduler (pure logic, no renderer).
  The end-to-end proof is running the example script above.

## Docs

Add a short section to `AGENTS.md`: how to run a drive script, where the
PNGs land, and one copy-pasteable example (the menu one above). That is what
makes future Claude sessions actually find and use this instead of falling
back to xdotool.

## Non-goals

- No pointer movement / pixel-coordinate clicks, no gamepad, no input
  recording/replay, no headless (GPU-less) rendering.
- Not a general test framework — just enough to run, look, and leave.
