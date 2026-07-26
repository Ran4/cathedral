To verify a change in a running game do NOT use xdotool/XTEST (winit never sees synthetic core events).

Instead script the game from the inside with
`CATHEDRAL_DRIVE`, a `;`-separated action list env argument.
The game runs windowed 1280x720, executes the actions (~0.5 s apart),
prints a `[drive] 3.2s key Escape` evidence line per action to stdout,
and exits on its own — no `pkill` needed.

Actions (each fires ~0.5 s after the previous):

- `key <KeyCode>` — press a key, e.g. `key Escape`, `key KeyZ`, `key F5`.
- `hold <KeyCode> <seconds>` — hold a key down, e.g. `hold KeyW 20` walks
  forward for 20 s; the next action fires after release. Caveat: a window
  focus loss mid-hold releases all keys (Bevy clears `ButtonInput` on
  `KeyboardFocusLost`), so don't click other windows during a scripted hold.
- `type <text>` — inject text into the Enter chat box as a raw keyboard message
  (`;` cannot appear in the text).
- `click <Name substring>` — case-insensitive match on a UI element's `Name`.
- `shot <name>` — capture a PNG to `logs/latest_session/screenshots/<name>.png`.
- `sleep <seconds>` — wait.
- `wait-online` — block until the actor engine is ready (30 s timeout).
- `sound <sound_id>` — emit a catalog world sound, e.g. `sound town_bell` (the
  stand-in trigger for world causes the sim lacks).
- `bell curfew | summons | knell <years>` — ring one of the two civic bells
  from its own tower: the Scold at the Bellstand, Maren Smallvoice at Saint
  Maren's. Patterns are assembled from a single stroke, so `bell knell 17`
  really rings seventeen countable strokes three seconds apart. Each peal
  prints `[bell] smallvoice knell: 17 strokes at 3.00s` to stdout and
  `logs.jsonl`. The Scold's *curfew* also has a real trigger (the clock's
  edge into the Snuffing, once a day); this action is the stand-in for the
  funeral and proclamation transactions the sim does not model yet.
- `status <name-or-id> <kind> <value>` — set a carriage body status so a
  drunk/weary walk can be eyeballed, e.g. `status Ilse drunkenness 0.8` or
  `status p006v weariness 1`. The target is resolved by display name first (may
  contain spaces), then by the actor id the HUD shows for strangers (`id p006v`).
  Kinds are `drunkenness`, `weariness` and `urgency`; value is a `0..=1` float. A handle
  matching nobody is logged (`logs.jsonl` source `engine`, and stderr) and
  skipped — not a fault. The stand-in for the ale the sim does not model yet
  (npc_bodies M5).
- `tp <x> <y> <z> [yaw_deg [pitch_deg]]` — teleport the player and aim the view
  (yaw 0 looks toward -Z, positive pitch looks up); switches to flying so an
  elevated vantage holds for a `shot`.
- `frame <name-or-id> [distance [bearing_deg]]` — photograph a named actor:
  stand off their *front* by `distance` metres (default 2.6) at eye height and
  look back at them. The handle is a case-insensitive substring of the display
  name (spaces allowed) or an exact actor id. The bearing walks the camera
  around them — 0° dead ahead, 90° to their left, 180° behind — so one script
  captures a body from every side without knowing which way it happens to be
  turned. The cast walks, so expect ~1 m of drift per action gap: shoot from
  4 m+ and skip the `sleep`. A handle matching nobody is logged and skipped.
- `quit` — exit immediately.

Without a trailing `quit` the game exits ~2 s after the last action; a watchdog
aborts after `CATHEDRAL_DRIVE_TIMEOUT` seconds (default 60). The `[drive]` lines
are also mirrored into the session's `logs/latest_session/logs.jsonl` (source
`"drive"`).

Example — open the settings menu, switch the STT pill, close it, exit:

```sh
CATHEDRAL_DRIVE='wait-online; key Escape; shot menu_open; click Local Canary-Qwen; shot stt_switched; key Escape; shot menu_closed; quit' cargo run
```

Example — hear four named places and both bells (`[soundscape] bed in: <area>`
lines name whichever place bed is playing):

```sh
CATHEDRAL_DRIVE='tp -214 3 140 0; sleep 3; tp -155 3 16 0; sleep 3; \
  weather rain 0.7; tp 100 3 214 0; sleep 3; weather timeline; \
  bell knell 17; sleep 4; bell summons; sleep 3; quit' cargo run
```

Pressing `key KeyT` twice puts the clock at 60×, which is how to reach an
hour-gated bed (the Wickmarket at Lamplight, Maren's Green at the Kindling) or
the daily curfew without waiting out a real hour.

`CATHEDRAL_SHOT=<name> cargo run` is shorthand for a single
`sleep 2; shot <name>; quit`. For runs without network/API keys set
`smart_actors.fake_backend: true` in config.ron — or export
`CATHEDRAL_FAKE_BACKEND=1`, which forces it without editing the file.
Other perf/dev env levers: `CATHEDRAL_PERF=1` (frame-time recording +
vsync off; see `features/performance_improvements/findings.md`),
`CATHEDRAL_DRIVE_RES=1920x1080` (drive window resolution — a drive window asks
to be non-resizable, which on a tiling WM does not always get you the size you
asked for but does stop it changing between runs; check the PNG's dimensions
before comparing two shots pixel for pixel),
`CATHEDRAL_NO_ACTORS=1` / `CATHEDRAL_NO_WEATHER=1` (ablation),
`CATHEDRAL_BODY_LINEUP=1` (stand a rank of 14 puppets — 7 outfit classes × 2
builds, every headgear — on the open paving at (0, 100), in the authored rest
pose and untouched by the sim, for A/B-ing the NPC model itself; view it with
`tp 0 1.35 96.6 180`, or from behind with `tp 0 1.35 103.4 0`). NOTE: drive scripts exercise
the real handlers, so e.g. clicking a backend pill persists to config.ron —
back it up first if that matters.
