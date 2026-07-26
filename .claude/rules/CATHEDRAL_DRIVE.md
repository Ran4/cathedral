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
- `seize <officer>` / `seize <officer> -> <target>` — stage an arrest
  (`features/law_and_order.md` M4): the named officer takes somebody in charge
  for the nearest station, defaulting to the player. Both handles resolve by
  display name first (may contain spaces) then by actor id, exactly as `status`
  does — hence the explicit `->`, since `seize Havise Ashe` is one handle and
  not two. Every judgement above `seize` is
  deliberately an LLM's, so a scripted run cannot otherwise reach one; this is
  the only way to look at the tether, the grab reflex and the strain meter. It
  goes through the same code a real seizure does, minus that verb's
  preconditions, and prints `[smart actors] <officer> takes <who> in charge for
  <station>`. The officer is first placed at arm's reach of the target, because
  custody whose escort is more than 20 m off is released on the next poll — so
  seizing from across the city would show nothing at all. A handle matching
  nobody is logged and skipped — not a fault.
- `commit [<name-or-id>]` — finish the escort at the Stone House
  (`features/law_and_order.md` M5), defaulting to the player. `seize` alone only
  ever shows the walk, and custody commits on *arrival*, so without this a
  scripted run can never see the inside of the gaol — not the booking, not the
  posted fee, not the bell you are told you go at, and not what walking out of
  it costs. It forces the Stone House rather than whatever posting the seizure
  picked (that is the thing being looked at), then runs the same
  `Custody::commit` and arrival announcement a real arrival does. Refused, with
  a logged reason, if nobody has them in charge — `seize` first. Prints
  `[smart actors] <who> is committed to The Stone House`.

  The whole scene, including the fifth door out:

  ```sh
  CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE='wait-online; \
    tp 44.5 1 -207.2 90; key Quote; seize Ede Clove; sleep 2; commit; \
    sleep 2; shot cell; hold KeyW 3; sleep 3; shot walked_out; quit' cargo run
  ```

  `key Quote` turns flying back off after the `tp` — **flying is not custody**,
  so a still-flying player is never tethered and never walks out of anything.
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
`CATHEDRAL_DRIVE_RES=1920x1080` (drive window resolution),
`CATHEDRAL_NO_ACTORS=1` / `CATHEDRAL_NO_WEATHER=1` (ablation). NOTE: drive scripts exercise
the real handlers, so e.g. clicking a backend pill persists to config.ron —
back it up first if that matters.
