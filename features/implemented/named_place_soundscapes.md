# Named-place soundscapes, the two civic bells, hens and bees

Implemented 2026-07-22. The second `features/more_sounds/` pass: the remaining
18 shortlisted sounds, taking `src/soundscape.rs` from 35 routes to **53**.
Every asset lives in `assets/sounds/soundscape/`, and both workbench manifests
(`more_sounds.json`, `sounds_to_implement.json`) now carry
`implemented_in_game: true` for all 53.

## 1. What went in

| Kind | Sounds |
|---|---|
| Named-place ambience beds | 14 loops: `gradine`, `wickmarket` (close-down), `coswalds_yard`, `tallage`, `marens_green`, `drapers_reach`, `tenterhook_lane`, `cinder_row`, `the_cut`, `gaunt_passage`, `hungry_ox`, `old_sluice`, `skinners_court`, `seven_lofts` |
| Civic bells | the Scold (`bellstand_tower`), Maren Smallvoice (`saint_marens_church`) |
| Urban nature | hens in five domestic yards, bees at the Wickmarket honey pitch |

## 2. Named-place beds bind to the shipped area map

The new mechanism is `AREA_BEDS` plus `AreaBedGeometry`, which resolves each
bed's `area_id` out of `assets/world/areas.json` — the same file the sim parses
— embedded with `include_str!` at startup. Audio geography therefore cannot
drift from simulation geography, and a test asserts every id still resolves.

A bed's emitter is the **point of its area nearest the listener**
(`BedBox::nearest_point` is a clamp, so it returns the listener itself when they
are inside). Three things follow for free:

- the bed loses its direction the moment you are in the place, which is what a
  room tone wants (the Lanthorn nave bed already worked this way);
- a bed can own a 1 km five-box corridor (`the_cut`) or a 12 m passage
  (`gaunt_passage`) without either shouting from a centroid;
- height counts, so flying over Gaunt Passage is not standing in it.

Two independent distances: `spill_m` (how far outside the boxes the bed may be
*demanded* at all, with the usual activation hysteresis) and the descriptor's
`radius_m` (how the sound attenuates). A test enforces `spill_m < radius_m`, so
the loop selector's own radius check can never reject a bed the spill test
admitted.

`AreaBedSchedule` is one variant per authored place — not a shared clock
window. Two hand-overs are asserted for every (weekday, office) pair:

- the Wickmarket **crowd** bed and the **close-down** bed are never both live;
- Maren's Green's fish-arrival bed hands over to the Lowmarket fish market.

`area_bed_gain_scale` carries the rest: market days fill the Tallage, Maren's
Green and the Seven Lofts; a downpour thins the `open_air` pitches (but never
Draper's Reach, where the rain *is* the bed); the Hungry Ox scales with how
many people are actually in it.

`AreaBed::muffles` is the declarative version of "each numbered furnace a
muffled source": standing in Cinder Row drops the `CinderFurnace` point source
to `MUFFLED_BEHIND_BED_GAIN`, because from the street the glass-house draw is
behind a screened bay.

`MAX_LIVE_LOOPS` went 8 → 10 so a place bed never wins its slot by evicting the
market, loom or furnace detail the place is made of.

## 3. Bells are patterns, not recordings

Per `lore/second_sun/design/06` §1, the store holds **one stroke per bell** and
every peal is assembled at runtime. `BellPattern::plan` gives the count, the
interval and the tower; `schedule_bell_pattern` queues the strokes with 20–40 ms
of seeded jitter each, so the bronze sounds pulled by hands. The jitter never
touches playback *speed* — retuning a bell the player is expected to count would
be a worse lie than a metronome.

- **Scold curfew** — 9 strokes at 3 s, rung from the Bellstand on the clock's
  edge into the Snuffing, after Evenblow's seventh office has finished plus
  `DUSK_GRACE_SECONDS`. Nine because the greatest office rings seven: a counted
  curfew can never be misread as a counted hour.
- **Scold summons** — 5 strokes at 1.15 s. Too quick to be counted as an hour.
- **Name-knell** — one stroke per year of the life, at the canonical 3 s.

Radii are the canonical ones (`design/06` §2): the knell carries 300 m across
the Reed Ward, the Scold 500 m over the eastern city, both below the Lanthorn's
own voice in scale.

Only the curfew has a real trigger today. The summons and the knell wait on the
proclamation and burial transactions the sim does not model, so they ship
behind the `SoundscapeCue::CivicBell` seam and the new drive action
(`bell curfew | summons | knell <years>`). **Not done:** the canonical
completion *percept* ("the name-knell from Saint Maren's: N strokes"). That
belongs to the funeral transaction that computes the count, not to a render
layer whose whole contract is never to fill an actor inbox.

## 4. Hens and bees

The bees are one 12 m source at Clemence Skep's honey pitch — the north-row
Wickmarket stall beside the Vell wax stand, since canon has the wax-house marry
the honey-seller. Covering the pots quiets them well before a storm sends them
in: `static_emitter_weather_gain` scales on precipitation, and
`wildlife_suppressed` finishes the job.

The hens are five yards in five different wards, taken from real door points in
`homes.json`: never the swept Gradine, never inside a market square, and more
than 100 m apart, so two yards can never be heard at once.

## 5. Two bugs the drive run found

Both were found by scripting the game, not by reading it:

1. **The curfew rang twice a night.** The Snuffing runs 21:00–02:00, so the day
   number changes *inside* the office; a `(day, office)` edge detector fired
   again at midnight. `CivicBellState::observed_office` is now the office alone.
2. **The Hungry Ox was always empty.** Its workplace nav node sits about a metre
   inside its own box and the round's tavern leash is 8 m, so strict
   containment reported an empty tavern while five people worked it.
   `AreaBedGeometry::occupied_by` allows one leash of slack.

## 6. Verifying

```sh
CATHEDRAL_DRIVE='tp -305 3 200 0; sleep 3; tp -228 3 26 0; sleep 3; \
  tp -130 3 205 0; sleep 3; weather rain 0.7; tp 120 3 260 0; sleep 4; \
  weather timeline; bell knell 17; sleep 4; bell summons; sleep 3; quit' cargo run
```

Each bed logs `[soundscape] bed in: <area> (<gain>)` / `bed out: <area>`, and
each peal logs `[bell] <label>: <n> strokes at <interval>s` — both to stdout and
to `logs/latest_session/logs.jsonl`. `key KeyT` twice puts the clock at 60×,
which is how to reach an hour-gated bed or the daily curfew without waiting.
