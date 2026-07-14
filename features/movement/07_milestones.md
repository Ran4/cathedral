# Milestones

Eight of them. Each ships something, and each one has a way to *see* it work with the tools this repo
already has — `cathedral-headless` for the sim, `CATHEDRAL_DRIVE` for the game.

The ordering has one rule: **M1 is where the risk is, and it depends on nothing.** Do it early, or do
it first, but do not discover the navigation problems while you are also debugging the sim.

---

## M0 — The Clock

**Ships.** `WorldClock` in the sim. The seven offices. The week. The sun moves. The bell rings its
ordinal. A HUD readout. A `T` key that cycles the debug time-scale.

**Nobody moves.**

**Touches.** `crates/cathedral-sim/src/clock.rs` (new), `EngineConfig`, `config.ron`,
`src/scene.rs:1096` (the one `DirectionalLight`), `src/smart_actors/hud.rs`.

**Why first.** It is independent of everything, it is small, and it makes the whole rest of the plan
*visible* — you can watch a day pass before a single NPC has taken a step. It also derisks the
prettiest part (does a rotating sun through Bevy's `Atmosphere` actually look good at dusk? find out
now, not in M7).

**How you know:**

```sh
# The sun goes down and the HUD agrees.
CATHEDRAL_DRIVE='sleep 2; shot dawn; key KeyT; key KeyT; sleep 25; shot dusk; quit' cargo run
```

```sh
# Two whole game days in two real minutes, every office printed as it rings.
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --seconds-per-day 60 -t 120
```

Expect seven bells per minute of wall-clock, in order, wrapping correctly at the Snuffing.

---

## M1 — The bake

**Ships.** `scripts/bake_navigation.py` → `assets/world/navigation.json` + the walkable bitset. **The
door fix** — pick the door edge by reachability, tie-broken by `stable_hash`, in *both* the bake and
`add_facade_openings` so the visible door and the nav door are the same door. A debug overlay (an
F-key) that draws the graph and the walkable surface. The full test suite from
[02_navigation.md](02_navigation.md) §8.

**Still nobody moves.**

**Why this is the risky one, and it is not hypothetical.** Everything downstream assumes you can get
from A to B. I ran the probe, and the answer was *mostly* yes — 65.9 ha walkable, 99.9% of it in one
connected component — **but 106 buildings have a front door with no walkable ground within six
metres of it** ([`door_probe.py`](door_probe.py)). `stable_hash(id) % polygon.len()` picks the door
edge without looking at what is on the other side of the wall.

That is a real defect in shipped code, and *only a walking NPC would ever have found it*. Find the
rest of them here, with no sim changes to bisect against and no NPCs to blame.

**How you know:**

```sh
cargo test -p cathedral-sim navigation
```

Every test in [02](02_navigation.md) §8 green, including the ones the lore already wrote for us in
`lore/places/04_routes_and_sightlines.md` §"Route acceptance tests for later builds" — *"walk
Wickmarket to Tallage through the Needle and by a handcart alternative"*, *"walk the full dry Cut from
Chain Bridge to Old Sluice"*.

And look at it:

```sh
CATHEDRAL_DRIVE='sleep 2; key F7; shot navgraph; quit' cargo run
```

---

## M2 — One NPC walks

**Ships.** The hot/cold snapshot split (perf-doc item 1). The fixed 20 Hz movement tick + render
interpolation (item 2). `World::step_movement`. One hard-coded actor pacing between two named places,
forever.

**Touches.** This is the big structural one:
`crates/cathedral-sim/src/{world,engine,nav/*}.rs`, `src/smart_actors/{model,actors}.rs`.

**And the novelty fix ships here, not later.** [05](05_the_llm_seam.md) §5.1 —
`context_hash` must only count *settled* neighbours before anybody starts walking past anybody. If it
does not land in M2, M3's token bill will be mysterious.

**How you know:**

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- --fake -t 60 --trace-positions
```

Their `position_m` advances, ~1.2 m per second, along a plausible street; they arrive; they turn
around. And in the game:

```sh
CATHEDRAL_DRIVE='wait-online; shot t0; sleep 20; shot t20; quit' cargo run
```

Two screenshots. They are somewhere else in the second one, and they got there by walking, not by
teleporting.

---

## M3 — The water round  ⭐ *the vertical slice*

**Ships.** Needs (thirst first). `Cues`. The ladder, with rungs 2, 6, 11 and 12 only. Routes to the
nine public water sources. A queue at the curb. `draw_water` / `chain_windlass` / `pour_trough` /
`pail_clatter` flipped to `actor_emittable`. And home again.

**Why this one.** Because it is already 80% built and nobody noticed — the wells are areas, the queue
aprons are *already walkable geometry* (`src/city/water.rs:14-16`), the four sounds are already in
`catalog.toml` under a comment that says *"flip `actor_emittable` and a keeper can work the curb"*, the
queue rules are authored in `lore/wells_and_water.md`, and `domestic_servant` is the single largest
occupation in the city with `lore_locations: [..., "Markets and wells", ...]`.

It exercises **the entire stack**: clock → need → cue → ladder → route → walk → queue → act → **sound**
→ and the sound is heard by the LLM layer, so an NPC standing nearby can be asked about it.

Zero tokens. Fully audible. Entirely lore.

**How you know:** stand thirty metres from Chain Well and *listen*. You hear the windlass. You walk
over and there is a queue, and a keeper, and household vessels going before trade vessels because the
lore says they do. You ask the woman at the front what she is doing, and she tells you, because the
`draw_water` percept is in her recent history.

```sh
CATHEDRAL_DRIVE='wait-online; sleep 40; shot chain_well; quit' cargo run
```

---

## M4 — The Round

**Ships.** The home-binding bake (`assets/world/homes.json`). The occupation → workplace mapping
(`assets/world/workplaces.json` — the ~150 lines of new content this feature actually needs). The 65
occupation templates. The twenty authored routes, transcribed. Market days. **Curfew.**

**How you know.** Two ways, and take both.

```sh
# A full game day, headless, and read the city.
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --seconds-per-day 120 -t 130 --census-by-area
```

At the Kindling the bakehouses and the Moorings should be occupied and the streets nearly empty. At
High Wick the squares should be full. At the Snuffing the streets should empty *again*, and the
Hungry Ox and the Bell and Ladle should not.

```sh
# And watch it. Stand in the Wickmarket and let curfew happen around you.
CATHEDRAL_DRIVE='wait-online; shot noon; sleep 90; shot curfew; quit' cargo run
```

**The real test is the boatmen.** Give the gates hours (`open: Dayspring, shut: Snuffing`). Run a
week. Somebody will be **gate-caught** — shut outside the wall at the Snuffing, sleeping in an
outlodge, his household eating without him. Nobody wrote that; it happened. If it happens, everything
under it works.

---

## M5 — `go_to`

**Ships.** The verb. `ActionErrorCode::{UnknownPlace, NoRoute, TooFar}`. The intent, with expiry. The
refusal. The arrival percept. The `turn.j2` change (deleting the paragraph that says *"if what you want
to do has no verb here (like walking somewhere)…"*). The sheet's three additions —
`the_hour`, `places_you_know`, `moving`. **All twenty golden fixtures regenerated.**

**Change the sheet once.** It is the most expensive small change in the codebase.

**How you know:**

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- -t 10 -v
```

An NPC issues `go_to`, the prompt log shows it, and their position starts moving toward the right
place. Then, in the game: **tell someone to meet you at the Gradine. Walk there. They are there.**

---

## M6 — The Night Office

**Ships.** The second cognition lane (one in flight; yields absolutely to the player; drops silently).
Individual reflection for the 31 Majors, at their own bedtimes. Ward-batched reflection for the 120
Minors. Code-rolled Rounds for the 350 Ambients. The `set_round` verb.

**39 provider calls per game day.**

**How you know.** Play an evening, sleep the night, come back in the morning: **an NPC's goal has
changed because of something that happened to them yesterday**, and their day is slightly different
for it. And the harder test — watch the foreground latency while a Night Office is running. If the
player's reply is ever slower because a Major was reflecting, the lane is wrong.

---

## M7 — Crowds and gait

**Ships.** Lane offsets (or you get a conga line). Local avoidance on stage. The gait — bob and swing,
phase-seeded, no skeletons (perf-doc item 7). The Needle's one-person claim. The lamplighter's dusk
round. `VisibilityRange`. The shadow cascade.

**How you know.** It looks like a city. And `features/50_cool_suggestions.md` #21 becomes true:

> One dedicated smart actor **walks a dusk route** through the five squares lighting lanterns one by
> one [...] **delay him with conversation and a whole quarter stays dark longer.**

---

## The dependency graph

```
M0 clock ─────────────┐
                      ├──> M3 water ──> M4 round ──> M6 night office
M1 bake ──> M2 walk ──┘                    │
             │                             └──> M7 crowds
             └──> M5 go_to
```

M0 and M1 are independent of each other and of everything else. **Both can start today, in parallel,
by two people who never speak.**
