# Rats in the fish lanes and the slaughter courts

**Status:** M0 implemented 2026-07-31, M2 implemented 2026-08-01. M1 (the two sounds) is not
built — the `rat_swarm` asset M2 fires is not generated either. M3 stays parked. Coordinates in
this file are current (post-shrink) world coordinates.

Put roughly fifty ordinary rats on the ground in eight authored colonies — the Shambles, the
fish landing, the tanneries, the quiet end of the Cut — scurrying in short darts, pausing, gone
again. They are drawn and heard, never simulated: no per-rat entities, no colliders, no nav
rebake, no new sim verb, no cast additions, and an ordinary rat never costs a token. The one
crossing into the sim is a single unattributed world-sound line when a colony *boils* (§4 M2),
which is the "LOTS of rats is a percept" half of the original note.

Related, and deliberately kept separate:

- `features/adhd-new-cool-features/04_stigmergy_fields.md` §"The Vermin Layer" — a few hundred
  rats/cats/dogs running gradient rules over hidden filth fields. That is the ambitious future;
  this feature is the hand-authored floor under it. If the fields ever ship, the colony table in
  §3 becomes their seed points, not a competitor.
- `features/scents.md` — rat ground is smell ground. §3 is a ready-made anchor list for it.
- `features/more_sounds/more_sounds.json` `snd_249_rats_behind_cellar_boards` — shortlisted
  there, generated and placed here (M1).

---

## 1. The city already believes in rats

Nothing needs inventing about *where*. Measured off the shipped data, the city asserts vermin
in three separate places and shows none:

| | |
|---|---|
| Existing fauna | 11 `SoundscapeSound` species (`src/soundscape.rs:519`) — ravens, sparrows, gulls, dogs, geese, hens, bees, an alley cat, flies. **All audio. No non-NPC creature has a body anywhere in the repo.** |
| The waste piles | `FliesAtWaste` (`src/soundscape.rs:2803`), three deliberately tiny 14 m sources doc-commented *"material waste piles, not three broad district ambience beds"*: (−214.2, −255.5) at Maren's Green, (−294.2, 219.6) in the Shambles, (−296.5, −228.6) at Tanners' Slip |
| Cats with no quarry | `CAT_ROOF_ANCHORS` (`src/soundscape.rs:148`) includes *"Eelback Alley / fish lanes"* — the cat is already hunting there; nothing is under it |
| The shortlisted sound | `snd_249_rats_behind_cellar_boards`, `implemented_in_game: false`, placement note: *"grain stores, tavern cellars, Gaunt stores, brine cellar"* |
| The lore | `lore/places/03_new_places_and_infrastructure.md:157` gives the Seven Lofts *"Raised floors, rat ledges"* |

A port with a slaughter quarter, a fish market, tanneries and grain lofts has rats; ours says
so out loud in three subsystems. This feature makes the claim visible.

---

## 2. Binding decisions

### 2.1 Rats are drawn, never collided

Same trap as the Cut kerb (`features/the_cut_improvements/the_cut_kerb.md` §2.1):
`scripts/bake_navigation.py::build_walkable` erodes **every** exported collider footprint by
the human agent radius and keeps only the largest connected component, and
`solid_footprints_in_band(WALK_BAND_LO, WALK_BAND_HI)` takes anything whose `max.y >= 0.01`.
A rat with a collider would punch moving holes in the walkable surface.

**Therefore: no entry in `CollisionWorld`, ever.** `collision_footprints.json` stays
byte-identical after this feature; assert that in review. Rats are confined the other way
around — by *reading* the world: each waypoint must pass `NavData::is_walkable(x, z)` and fail
`CollisionWorld::contains_point`. The precedent for a render layer holding its own `NavData` is
`puddle_mesh` (`src/weather/render.rs:1013`), which `include_str!`s the committed bake.

### 2.2 One entity, one batched mesh, seeded motion

The chimney-smoke pattern (`src/city/smoke.rs` + `src/mesh_batch.rs`), at smaller scale: one
`Vermin` entity, one alpha-tested `StandardMaterial`, one mesh rebuilt per frame with
`write_batch_mesh`, `NoFrustumCulling` + `NotShadowCaster`, and a batch that goes empty parks
on the idle triangle for free.

Per-rat motion is a pure function of the clock, like a smoke puff: at startup each colony bakes
its rats' waypoint loops (4–8 points inside the colony radius, each validated per §2.1, all
drawn from `(colony seed, rat index)`), and per frame a rat's position is a piecewise
sprint–pause–sprint along its loop derived from elapsed time. No stored per-rat transform, no
per-rat system, deterministic forever. The only mutable state is one small
`Option<(Vec3, f32)>` scatter impulse per colony (§3).

**Not camera-facing billboards.** Smoke can billboard because it is amorphous; a ground
creature seen from one of the city's many overhead bridges (or in developer flight) must not
turn to face down. Each rat is an oriented tent of two quads — a body ridge ~0.28 m long,
~0.09 m high, rotated to its heading — plus a single tail quad. Flat dark-brown vertex colour,
no texture: at 25 cm and 2 m/s the rat reads by **motion**, not by texel.

### 2.3 An ordinary rat never reaches the sim

The catalog states the rule this feature must obey (`assets/sounds/catalog.toml`, the
`coin_clink` comment): world texture is *"heard only by the player … so thirty sales an hour in
a square never nudge an NPC into a reaction turn."* Every M0/M1 rat sound is a
`src/soundscape.rs` sound — render-side texture, never a percept, exactly like the flies and
the hens today.

The **only** sim crossing is the boil (M2), and it uses the thrice-precedented unattributed
world-sound shape (`town_bell`, `coin_clink`, `market_cry`): a catalog row with
`actor_emittable = false` and **no `seen`** — a swarm has no author — funneled through
`emit_sound(world, None, …)` (`crates/cathedral-sim/src/perception.rs:73`). The inbox
coalescing rule (*"a percept barrage cannot flush real dialogue out of the window"*, the
`… (3 times now)` counter) already bounds repeats.

Deliberately **not** a standing sheet field beside `weather`
(`prompt/mod.rs`, `YouAre.weather`): weather is a standing fact about a body's situation; a
boil is a transient event, which is what the inbox is for — and the event route leaves
`golden_prompts.rs` unbled. If rats ever become a standing fact about a *place* (the stigmergy
future), revisit.

### 2.4 Colonies are authored, not spread

The colony table in §3 is the whole population. No reproduction, no migration, no
field-following, no procedural placement — the same spirit as the fixed hand-authored cast.
`config.ron` `density` scales the per-colony counts and nothing else.

### 2.5 Config and ablation

Top-level `vermin: ( enabled: true, seed: 40, density: 1.0, swarm_percepts: true )` block —
the `weather` precedent: host-side presentation with one sim-facing switch, all
`#[serde(default)]` so existing configs keep loading. Plus a `CATHEDRAL_NO_VERMIN` env lever
beside `CATHEDRAL_NO_ACTORS` / `CATHEDRAL_NO_WEATHER` in `src/main.rs:47–60`, so a
`CATHEDRAL_PERF=1` attribution run can ablate the rebuild without touching anyone's config.

---

## 3. The colonies

| Colony | Anchor (x, z) | Rats | Active | Why here |
|---|---|---|---|---|
| The Shambles | (−294, 220) | 10 | all offices | the slaughter courts; `FliesAtWaste` key 31 is this exact pile |
| Maren's Green, landing edge | (−214, −255) | 8 | all offices | fish market spoil; flies key 30 |
| Tanners' Slip | (−296, −229) | 6 | all offices | tanning waste; flies key 32 |
| Eelback Alley | (−275, −330) | 6 | Snuffing→Kindling | the fish lanes; the alley cat already hunts these roofs |
| The Old Sluice | (−213.5, −400) | 6 | Snuffing→Kindling | the Cut's poorest, quietest reach; the dry grate |
| Gaunt Passage | (−155, 17) | 4 | Snuffing→Kindling | folklore empties the passage after dark — something should actually be in it |
| Seven Lofts skirts | (252, 234) | 4 | Snuffing→Kindling | grain; lore's "rat ledges", but also *"kept mostly at bay"* — small on purpose |
| Wickmarket | (−14, 249) | 6 | Snuffing→Kindling | market spoil after close-down |

The three all-office colonies are exactly the three authored waste piles: the flies and the
rats mark the same filth, which is the kind of agreement between subsystems the soundscape
already practices (its `AREA_MAP_SOURCE` is the sim's own `areas.json`).

The Old Sluice anchor was corrected during M0: this table first read (−213, −427), which is
inside the sluice's own shell (`areas.json: old_sluice` is solid from `z −448` to `z −406`, so
it settles no rats at all), and as built the colony stands where the row means — at the dry
grate, on the Cut's centreline (`CUT_CENTRE_X`) a few metres off the blocked dry arches at
`z −405.86`.

Numbers: colony radius 8–14 m; sprints 1.8–2.6 m/s for 0.4–1.2 s; pauses 0.5–4 s, weighted
long; a slight body bob while sprinting. Night gating reuses the clock the way
`schedule_is_active` does (`WarmDayWaste` is `brightness > 0.30`; the night colonies are its
inverse). `wildlife_suppressed` (`src/soundscape.rs:2246`) thins the visible count in heavy
rain, matching the animals going quiet. Cull the whole batch past **60 m** — a 25 cm rat is
sub-pixel long before smoke's 450 m — so the frame cost is a few dozen quads near the player
and the idle triangle everywhere else.

**Scatter:** a player (or any actor puppet) inside ~2.5 m of a rat kicks its colony's scatter
impulse — affected rats dart away for ~1.5 s, then resume their loops. This is the moment the
feature earns its keep: rats that ignore you are wallpaper; rats that flee you are alive.

---

## 4. Milestones

### M0 — the rats

`src/city/vermin.rs`, registered in `CityPlugin` beside `animate_chimney_smoke`
(`src/city/mod.rs:65`): the colony table, startup waypoint validation against nav + collision,
the batched tent-quad mesh, clock gating, the scatter impulse, the config block and
`CATHEDRAL_NO_VERMIN`. Silent.

**This alone is the feature. Everything below is optional.**

### M1 — the sound of them

Two sounds, both pure `src/soundscape.rs` texture (remember the lockstep arrays: `ALL_SOUNDS`
and `SOUND_DESCRIPTORS` index-match `SoundscapeSound`, both bump 55 → 57; assets are
`include_bytes!`-tested):

- **`snd_249_rats_behind_cellar_boards`** — generate it at last (the more_sounds workbench:
  `uv run --script features/more_sounds/server.py --open`), loop, and place three
  `StaticEmitter`s at the shortlist's own spots: the brine-rotted cellar (`brine_cellar`,
  ~(−315, −288)), the Hungry Ox tavern rear (~(−237, −338)), the Seven Lofts (~(252, 234)).
  Concealed rats, deliberately **no visual** — gnawing behind boards you cannot see is worth
  more than three more visible colonies. Quiet/dark schedule, small radius like the flies
  (gain ~0.12, radius ~12 m).
- **A new scatter one-shot** — claws-on-cobbles scurry with one squeak, ~2 s. Add a row to
  `features/more_sounds/more_sounds.json` in its style (next free number), generate, and fire
  it from the M0 scatter impulse via the `schedule_urban_nature_sounds` pattern
  (`proximity_entered` + per-colony cooldown, ~45 s), suppressed by `wildlife_suppressed`.

### M2 — the boil — **implemented 2026-08-01**

Once a game night, one colony surges: `hash(game_day, seed) % colonies` picks it, and from the
Snuffing (the curfew edge the Scold already rings) to the Kindling its count triples and its
radius doubles. Purely more of the M0 rats — plus the percept:

- Catalog row `rat_swarm` in `assets/sounds/catalog.toml`: `actor_emittable = false`, no
  `seen`, `audible_distance = 12`,
  `heard = "[Rats — far too many of them — are pouring through the street here.]"`, and an
  `sfx_prompt` so the player hears it too (playback resolves from the id by convention).
- A new `EngineCommand::WorldSound { sound_id, position_m }` beside `DebugSound`
  (`crates/cathedral-sim/src/engine.rs`), same `emit_sound(world, None, …)` funnel — a
  proper name for the non-debug path rather than shipping on the debug verb. The vermin plugin
  fires it on boil entry and re-arms while the boil holds.
- No sim state, no new verb, no attention change needed: an audible sound already takes the
  priority lane, so an NPC standing in the boil reacts on their next turn, and the coalescing
  counter keeps a long boil from flooding anyone's history.

As built, three things this section did not foresee:

- **The re-arm is half a game-hour, not "a few game-minutes."** That priority lane is not free:
  `flush_sound` hands the nearest hearer the next turn, so every repeat buys a paid provider
  call for whoever is standing in the boil. Eight game-hours at five game-minutes is ~96 nudges
  a night for one person — two and a half times the Night Office's whole daily budget — and a
  60× drive run showed exactly that (one NPC took 24 of a 28-prompt run; the rest of the cast
  starved). At 30 game-minutes a whole night costs 16, and the same run spread 19 prompts over
  eight people. `SWARM_PERCEPT_INTERVAL_MINUTES` in `src/city/vermin.rs`.
- **The boil complement is baked at startup, beside the ordinary loops**, over the doubled
  radius and through the same nav+collision validation — a boil is not the frame to run forty
  walkability probes per rat. A draw that finds no room out there falls back to the colony's own
  disc (Gaunt Passage's doubled circle is mostly building), so the count really does triple.
- **"Is this colony drawn" is now one function**, `colony_showing`, shared by `animate_vermin`
  and `trigger_vermin_scatter`: a boiling colony is out whether or not the darkness gate alone
  would show it, and the batch and the scatter sweep must never disagree about which rats exist.

Still open: `assets/sounds/rat_swarm.mp3` does not exist (it belongs to the M1 generator pass —
`uv run --script scripts/generate_sounds.py` will make it from the row's `sfx_prompt`). Until it
does, a boil logs one `bevy_asset` "Path not found" per emission and plays nothing; the percept,
the HUD line and the visual are unaffected. And a boiling colony at deep midnight is very nearly
invisible on screen — dark-brown 25 cm bodies on unlit paving — which is worth a look when M3's
cats arrive.

**Not done:** a `vermin` sheet field (§2.3 has the reason), any rat item, catching, poison, or
gameplay consequence. The boil is a sight and a sentence, nothing more.

### M3 — cats with a quarry (parked)

One line, not a system: bias the existing `AlleyCat` one-shots toward the currently boiling
colony inside `schedule_urban_nature_sounds`, so the night the Eelback boils is the night the
cats are loud over it. Anything beyond that — actual pursuit, populations, filth gradients —
belongs to `04_stigmergy_fields.md` and should not be built here.

---

## 5. Verifying

The all-office colonies need no clock work; `key KeyT` twice (60×) reaches the night ones and
the boil. `sound rat_swarm` exercises the M2 percept path through the real funnel with no boil
waiting, the moment the catalog row exists. Two lessons from the first verification pass are
baked into the script: every §3 vantage stands at a *larger* z than its anchor, so the view
must be yaw **0** (yaw 180 faces away from every colony), and at 60× with `seconds_per_day:
3600` the Snuffing is ~35 wall-seconds out from the Dayspring start, not 8. `weather clear 0`
keeps `wildlife_suppressed` from thinning the count mid-read.

```sh
CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=120 CATHEDRAL_DRIVE='wait-online; \
  weather clear 0; \
  tp -294 1.6 226 0; sleep 2; shot shambles_rats; sleep 1.5; shot shambles_rats_b; \
  tp -214 1.6 -249 0; sleep 2; shot green_rats; sleep 1.5; shot green_rats_b; \
  key KeyT; key KeyT; sleep 36; key KeyT; \
  tp -155 1.6 22 0; sleep 2; shot gaunt_rats_night; \
  sound rat_swarm; sleep 3; quit' cargo run
```

The paired `_b` shots are the point: a rat is 25 cm of flat dark brown and reads by motion, so
compare the two frames for displaced bodies rather than squinting at one. Expect the night shot
to show almost nothing — this build's night ground renders black (a same-vantage Shambles
day/dusk control proved the invisibility is the lighting, not the vermin layer); the gate and
the boil are pinned by unit tests instead.

Then check: the two shots show dark darting bodies near the waste piles; a `[vermin] boil: <colony>`
line appears in `logs/latest_session/logs.jsonl` once per game night; after `sound rat_swarm`,
the bracketed heard line shows up in `since_your_last_turn` of the next prompt under
`logs/latest_session/prompts/`; `git diff --stat assets/world/collision_footprints.json` is
empty; and a `CATHEDRAL_PERF=1` pair with/without `CATHEDRAL_NO_VERMIN` shows the batch
rebuild well under 0.2 ms.
