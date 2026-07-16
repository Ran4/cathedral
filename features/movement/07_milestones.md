# Milestones

Eight of them. Each ships something, and each one has a way to *see* it work with the tools this repo
already has — `cathedral-headless` for the sim, `CATHEDRAL_DRIVE` for the game.

The ordering has one rule: **M1 is where the risk is, and it depends on nothing.** Do it early, or do
it first, but do not discover the navigation problems while you are also debugging the sim.

---

## M0 — The Clock  ✅ *implemented*

**Ships.** `WorldClock` in the sim. The seven offices. The week. The sun moves. The bell rings its
ordinal. A HUD readout. A `T` key that cycles the debug time-scale.

**Nobody moves.**

**Status — done and verified.** `crates/cathedral-sim/src/clock.rs` (pure, 15 unit tests);
`EngineConfig.clock` + `EngineMessage::Clock` (republished every poll) + `EngineCommand::CycleTimeScale`;
the bell is a *player-only* `Sound` (no NPC percept, no nudge — the offices are a clock, not events);
`src/smart_actors/clock.rs` projects it into `WorldClockState`, rotates the `Sun`, writes the HUD, and
cycles the scale on `T`. Configured under `smart_actors.clock` in `config.ron`. **The clock reaches the
NPC** through the sheet's `you_are.the_hour` (§7 — a field, not a percept; `Option`, so the frozen golden
fixtures stay byte-identical). **The office and time-scale are mirrored into `logs.jsonl`** on change
(source `"clock"`), so a `CATHEDRAL_DRIVE` script can assert on the clock in text, not only by screenshot
(§6).

*Note on the verification recipe below:* drive `sleep`s are wall-clock, but the game clock advances in
`Time<Virtual>` (what the engine's `now` reads), which lags wall-clock when debug frames are slow — so a
real-time sleep advances game-time less than `scale × sleep`. Cycle up to 60× and/or set
`clock.start_office` to reach a target hour. Working recipes:

```sh
# In-game: cycle to 60×, watch the sun cross the sky, read the HUD.
CATHEDRAL_DRIVE='wait-online; shot morning; key KeyT; key KeyT; sleep 20; shot later; quit' cargo run
# Start at dusk to see the dark end of the cycle.
#   (set smart_actors.clock.start_office: "lamplight" in config.ron first)
```

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
# Two whole game days, every office printed as its bell rings. `--watch-clock`
# steps the clock deterministically (it takes game *days*, not turns — `-t` is a
# turn count, so it would not run "two days"), so no bell's line is ever skipped.
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --seconds-per-day 60 --watch-clock 2
```

Expect all seven offices printed in order each game day — fourteen across the two days — wrapping
correctly from the Snuffing back to the Watch.

---

## M1 — The bake  ✅ *implemented*

**Ships.** `scripts/bake_navigation.py` → `assets/world/navigation.json` + the walkable bitset. **The
door fix** — pick the door edge by reachability, tie-broken by `stable_hash`, in *both* the bake and
`add_facade_openings` so the visible door and the nav door are the same door. A debug overlay (an
F-key) that draws the graph and the walkable surface. The full test suite from
[02_navigation.md](02_navigation.md) §8.

**Still nobody moves.**

**Status — done and verified.** `scripts/bake_navigation.py` produces `assets/world/navigation.json`
(the street graph — ~9k nodes, 68 places, 23 sites, 2,562 doors) and `navigation.bin` (the 2.4 MB
walkable bitset). `crates/cathedral-sim/src/nav/` loads them (`NavData::from_parts`) and does
walkability, `nearest_node`, A*/Dijkstra routing. The door fix lives in `choose_door_edge` (bake) and
`add_facade_openings` (consumes the baked edge via `cathedral_sim::door_edges_from_json`).

**Load-bearing change vs. the original plan.** The walkable surface is baked as the *exact complement of
`CollisionWorld` at walking height*, not re-derived from the plan — the divergence the plan hoped was
small (§2) is not: the curtain-wall thickness, tower boxes and gatehouses alone are ~19 ha. So a game
test `export_collision_footprints` (ignored) writes `assets/world/collision_footprints.json` — every
collider footprint crossing y = 0.91 — and the bake subtracts exactly those. This makes **"roads win"
automatic**: an overhead structure's collider starts above head height, so it is absent from the export
and the covered way beneath (Malt Passage, the Cut under its bridges) stays open with no special case.
Regenerate that file whenever scene collision changes, then re-bake.

**How you know (all green):**

```sh
cargo test -p cathedral-sim navigation      # 11 tests: reachability, doors, graph↔bitset, roads/Cut/Reach/Malt
cargo test --bins no_walkable_cell_is_solid the_door_you_see   # the two game-crate closes-the-loop tests
CATHEDRAL_DRIVE='sleep 3; key F7; shot navgraph; quit' cargo run   # look at it
```

**Touches.** `scripts/bake_navigation.py` (new), `crates/cathedral-sim/src/nav/*` (new),
`assets/world/{navigation.json,navigation.bin,collision_footprints.json}` (new),
`src/city/mod.rs` (door fix + the two closes-the-loop tests + the collision exporter),
`src/controller.rs` (`CollisionWorld::contains_point` + footprint export), `src/nav_overlay.rs` (new, F7).

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

## M2 — One NPC walks  ✅ *implemented*

**Ships.** The hot/cold snapshot split (perf-doc item 1). The fixed 20 Hz movement tick + render
interpolation (item 2). `World::step_movement`. One hard-coded actor pacing between two named places,
forever.

**Status — done and verified.** The sim owns the walk: `NavData` is now wired into the engine
(`EngineConfig.nav: Option<Arc<NavData>>`, `None` by default so the frozen fixtures are untouched);
`CharacterState.movement: Option<Movement>` carries the remaining polyline, speed and a continuous
gait phase (`character.rs`); `World::step_movement` advances every mover one fixed slice **without
bumping `world_revision`** — positions ride a new *hot* channel, `EngineMessage::Movement { moved:
Vec<ActorMotion> }`, exactly as the `Clock` message does (that IS the hot/cold split — the cold
name/knows snapshot only rebuilds on a real revision change). The engine drives a **fixed 20 Hz
accumulator** (`MOVEMENT_TICK_SECONDS = 0.05`, `WALK_SPEED_MPS = 1.8`) in `poll`, before the stage
block, so `context_hash`/`characters_within` read current positions. The host consumes the hot channel
into a `MovementInbox` resource and interpolates each mover's `Transform` prev→current over the 50 ms
tick window in `drive_npc_bodies` (`src/smart_actors/actors.rs`), ordered *after* `reconcile_actor_views`
so it owns the mover's transform — no second `Time<Fixed>` schedule, so it never fights the player's
120 Hz controller. **The §5.1 novelty fix shipped here:** `context_hash` counts a neighbour only if
`is_settled()` (speed < `SETTLED_SPEED_MPS = 0.15`), so a man crossing the square is not news at every
step but a man who stops is. The hard-coded pacer was `p0012` (a market-seller) walking the west
forecourt flagstones between "Tenterhook Lane" and the "Seraph statue" — visible from the player's
spawn — set up in `Engine::new::seed_pacing_actor` (diagnostic-and-skip if the actor/place/route does
not resolve; never panics). *Since retired:* M4's round subsumed the bring-up artifact — `p0012` is now
enrolled like everyone else (`seed_pacing_actor` is gone), and the `Movement.patrol` ping-pong mechanism
survives only as test scaffolding. *Note:* in-game **visible** travel per real second trails the nominal
1.8 m/s because the engine runs on `Time<Virtual>`, which lags wall-clock under a heavy frame — the
same lag the clock/sun already have (M0 note above); the sim itself moves at a verified 1.8 m/s.

**Touches.** This is the big structural one:
`crates/cathedral-sim/src/{world,engine,character,attention,nav/mod,lib}.rs`,
`crates/cathedral-backends/src/bin/cathedral_headless.rs`, `src/smart_actors/{model,actors,mod,local_engine}.rs`.

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

## M3 — The water round  ✅ *implemented* — ⭐ *the vertical slice*

**Ships.** Needs (thirst first), on the dynamic `statuses` axis. The ladder, with rungs 2, 6, 11 and 12 only. Routes to the
nine public water sources. A queue at the curb. `draw_water` / `chain_windlass` / `pour_trough` /
`pail_clatter` flipped to `actor_emittable`. And home again.

**Status — done and verified.** The whole non-LLM behaviour layer lives in one new pure module,
`crates/cathedral-sim/src/round.rs`, driven once per poll right after `tick_movement` and gated on
`config.nav` (so a world without a nav graph — every frozen fixture — stays inert and byte-identical).
`CharacterState` gained the dynamic `statuses` axis as `needs: Needs { thirst }` (0–255, high = satisfied;
never rendered in M3, so the 20 golden prompts are untouched); `Movement.patrol` became `Option` (`None`
means the behaviour ladder owns the arrival, not the M2 ping-pong). `WorldClock::game_days` exposes game
time so thirst decays by the *game* clock (the `T` key speeds it up with the sun). The **nine** public
drinking sources are resolved from the nav graph (`lore/wells_and_water.md` puts the Shambles *work* well and
`Seven Lofts fire tanks` outside the ward drinking list, so both are excluded); each takes a **keeper** from
the nearest idle *ambient* townsperson (so no named character is pinned to a curb) and every water-drawing
character (`domestic_servant` households plus `cloth_worker` / `garment_worker` trade vessels) is bound to
their nearest staffed source. The ladder is a flat first-match cascade — **rung 2** parched → the well now;
**rung 6** thirsty → the well if its queue is short; **rung 11** the social pull → drift to a known, settled
neighbour; **rung 12** wander within a leash of home. A queue forms at the curb with **household vessels ahead
of trade vessels**, the keeper works the gear so the windlass is *heard*, the draw refills thirst, and the
drawer walks **home again**. Determinism is preserved: no RNG, every choice a pure hash of
`(salt, actor_id, epoch)`; the round path never bumps `world_revision` (positions ride the hot channel exactly
as M2's do).

**The sound model is the load-bearing subtlety, and an adversarial review caught the trap.** The windlass is
emitted **exactly like the bell — an unattributed world sound heard only by the player** — because a sound
event with an NPC recipient hands the scheduler's single, *proximity-ungated* priority slot to that NPC
(`flush_sound`), and ten curbs clanking every few seconds would pin that slot forever and fire back-to-back
LLM turns at off-stage ambient NPCs: a continuous token bill on people the player never meets, and the exact
opposite of M3's "zero tokens". So the well sound nudges nobody; the affordance the milestone actually wants —
*"ask the woman what she is doing… the percept is in her recent history"* — is served instead by the **drawer
remembering their own draw** (`remember_percept`, no sound, no nudge) as their turn begins.

**Two further deliberate departures, both documented in the code:** (1) the four water sounds gained a `seen`
line — used now for the *drawer's own* memory of the act ("You drew water at the well.") — but kept
`actor_emittable = false`: *flipping* it would list them as `make_sound` verb choices in every prompt and
regenerate all twenty golden fixtures, which is M5's sheet change, not M3's. (2) trade drawers (fullers/dyers)
are enrolled alongside the household servants so the household-before-trade precedence is *observable live*,
not only in a unit test.

**Verification.** `cargo test -p cathedral-sim round` — five new tests including the end-to-end slice (a
parched servant walks to the well, queues, draws — remembering it, and with the windlass a nudge-free world
sound — and goes home); the full suite stays green (378), the M1 door tests unaffected. Headless on the real
500-strong cast shows the city breathe — ~69 drawers, the thirsty count rising and falling in waves as the
whole cast cycles through the staffed wells, each ringing the right sound. The game runs the round in-engine
(`[smart actors] water round: 9 sources, 8 staffed, 69 drawers` — one well's ward has no ambient nearby to
keep it, which is allowed) with no regression.

*Note on hearing it in-game:* the player spawns ~107 m from the nearest well and `CATHEDRAL_DRIVE`'s `key`
action injects a single-frame tap (it cannot walk the player 100 m to a curb), so the *audibility* is proven
headless — every draw reports its recipient count (`N heard`), i.e. that many NPCs got the percept — rather
than by an automated screenshot. Stand thirty metres from a staffed well in a real session and you hear the
windlass.

```sh
# Watch the whole city fetch water, turn-free, over ~0.15 game days.
cargo run -p cathedral-backends --bin cathedral-headless -- --fake --watch-clock 0.15 --trace-water
```

**Why this one.** Because it is already 80% built and nobody noticed — the wells are areas, the queue
aprons are *already walkable geometry* (`src/city/water.rs:14-16`), the four sounds are already in
`catalog.toml` under a comment that says *"flip `actor_emittable` and a keeper can work the curb"*, the
queue rules are authored in `lore/wells_and_water.md`, and `domestic_servant` is the single largest
occupation in the city with `lore_locations: [..., "Markets and wells", ...]`.

It exercises **the entire stack**: clock → need → ladder → route → walk → queue → act → **sound**
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

## M4 — The Round  ✅ *implemented*

**Ships.** The home-binding bake (`scripts/bake_homes.py` → `assets/world/homes.json`). The
occupation → workplace mapping and the 65 occupation templates and the twenty authored routes, all in
one authored `assets/world/rounds.json`. Market days. **Curfew.**

**Status — done and verified.** M4 generalises the M3 water round in place: `WaterRound` became
`Round` (`crates/cathedral-sim/src/round.rs`), which now enrols the **whole LLM cast** — each with a
**home** (baked), a **workplace** (the nearest of the occupation's candidate nav places to home), and
a **round** of office-pegged legs (the 19 authored routes for the majors that resolve to a 5-char id;
the 65 occupation templates, grouped into eight archetypes, for everyone else). Water is now just two
rungs of one flat ladder. The ladder each decision epoch is **curfew (5) → parched (2) → thirsty (6)
→ the round (9) → social (11) → wander (12)**, first match wins. Market-day legs (`only_on`) move the
crowd to **all four** market squares — two market-trader archetypes split the trades so Highmarket
fills the Wickmarket *and* Coswald's Yard and Lowmarket the Tallage *and* Maren's Green, per
`trade_and_daily_life.md` — and **Bellday closes the generic trades and fills the nave** (the
day-worker/market templates lie in at the Kindling and pray at The Lanthorn from Dayspring through
the Waning; the wharf joins at Dayspring after its before-dawn work, per Wyn Alder's route; night
trades, clerics and the anchoress are the exceptions); the curfew rung sends the housed home at the Snuffing
while the night trades (a `curfew_exempt` archetype flag: tavern, watch, lamplighter) keep their
posts, and the ~100 homeless (a homeless circumstance → no `homes.json` entry) are left in the street,
exactly as the lore intends. The **whole cast** means the well keepers and the old M2 pacer too: a
keeper's round is their well (the curb from the Kindling, on a stride-short leash, so the source stays
staffed through the working day; never water-bound themselves) and the housed among them go home at the
Snuffing like anyone else — the queues and the windlass only need `WaterSource::keeper` to be assigned,
not the keeper's body at the curb at 2 a.m.

**Two load-bearing subtleties, both caught during bring-up.** (1) The **home-binding bake falls back
across wards** — the literal "nearest unclaimed residential in my ward" runs out of houses in five of
eight wards (Weigh/Reed/Wick), so `bake_homes.py` binds ward-local where it can (290 of 400) and to
the nearest unclaimed residential city-wide otherwise (110). (2) The **wander/social leash is anchored
at the current post, not home** — anchoring it at home yanked every worker who reached the Wickmarket
straight back to his own door, so nobody ever settled anywhere during the day.

**The content and the round are embedded** (`include_str!`) exactly as the game host embeds the nav
graph, so both hosts get M4 with no wiring, and the layer stays **inert without a nav graph** (the
frozen golden prompts pass `nav: None`, so nobody is enrolled and the 20 fixtures are byte-identical).
Pure and deterministic: every choice is a hash of `(salt, actor_id, epoch)`, positions ride the hot
channel and never bump `world_revision`.

**How you know.**

```sh
# A full game day, headless, and read the city as each office rings.
cargo run -p cathedral-backends --bin cathedral-headless -- \
    --fake --start-office watch --watch-clock 1 --census-by-area
```

`[census]` samples *within* each office (not at the bell, when the whole cast has just re-routed). A
real run: at the **Kindling** the Alder Moorings and the taverns are occupied and the housed are still
abed; through **Dayspring** the posts climb past 200 as the streets empty; at **High Wick** the
squares are full (the Wickmarket ~45, Coswald's Yard ~30, the Tallage ~20); at the **Snuffing** the
streets clear again — *home* climbs past 240 — while the **Hungry Ox (27) and the Bell and Ladle
(~18) do not empty**, exactly as the milestone asks.

```sh
# And watch it. Stand in the Wickmarket and let curfew happen around you.
CATHEDRAL_DRIVE='wait-online; shot noon; sleep 90; shot curfew; quit' cargo run
```

**The boatmen, and what is deferred.** The wharf archetype gives the boat trades a Moorings day, but
the **gate-caught** emergent test needs `outer_wharves` and gate-hours geometry the baked graph does
not yet reach outside the wall (there is no nav node there), so it is noted as follow-up rather than
claimed. `hunger`/`fatigue` needs and an `Indoors` render-hide are likewise out of M4's ship list —
the round's office-pegged legs plus the curfew rung already empty the streets without them.

---

## M5 — `go_to`  ✅ *implemented*

**Ships.** The verbs go_to and related verbs.
The go_to intent, with route-derived expiry (a multiple of the route's expected travel time in
*real* seconds — never a flat span of game time; see [05](05_the_llm_seam.md) §2). The intent's
ladder rung sits between thirsty (6) and the round (9) — curfew and parched preempt it, deliberately.
The person target gated on sight, degrading to last-seen position. The arrival percept **and the
lapse percept** (an expired or need-preempted intent tells its owner so), both granting the
priority nudge off stage. Validation errors `unknown_place` / `no_route`
— **`too_far` is deferred** until the hunger need exists (05 §2; reserve the enum variant now).
**`tell_way`** — the knowledge-transfer verb ([05](05_the_llm_seam.md) §3): speaker must hold the
id, target in earshot, writes the id into the target's `places_you_know`, one inbox line. It ships
here rather than later because the golden fixtures byte-diff the full rendered prompt, verb list
included — deferring it would mean regenerating all twenty fixtures twice.
The `turn.j2` change (deleting the paragraph that says *"if what you want
to do has no verb here (like walking somewhere)…"*). The sheet's three additions —
`the_hour`, `places_you_know`, `moving`. **All twenty golden fixtures regenerated.**

**Change the sheet once.** It is the most expensive small change in the codebase.

**Status — done and verified.** The verbs are three new `match` arms in
`actions.rs` (`go_to` / `stop` / `tell_way`); the intent lives on
`CharacterState.intent` (`TravelIntent`: target, route-derived `budget_seconds`,
deadline stamped by the round's first tick — the action layer has no clock) and
the wayfinding whitelist on `CharacterState.places_known`, both plain sim state.
The registry behind the opaque handles is new: **`scripts/bake_places.py` →
`assets/world/places.json`** (68 nav places classified into the eight wards by
the authored `00_city_plan.md` bounds, + 8 walkable ward anchors, each under a
baked `pl_xxxx` id) loaded by **`crates/cathedral-sim/src/places.rs`** into
`World.places`, with the 401 baked homes registered at seed time as
"<Name>'s house" entries. `Round::seed` hands out the whitelists — the 7
`kind: "major"` places and all 8 wards to *everyone* (the legal first step),
plus own-ward places, own home, workplace + route legs, and the homes of
everyone in `knows` — and the engine now sets `World.nav` so `go_to` prices and
validates its route at intent time (`unknown_place` / `no_route`; `TooFar` is
reserved, unraised, until hunger exists). The ladder gained **rung 8** between
thirsty (6) and the round (9), and a per-poll `tick_intents` pass owns the
endings: arrival ("You have arrived at The Wickmarket."), route-budget expiry,
pressing-rung preemption ("The curfew/Thirst turned you back before you reached
…"), the person-follow (re-pathed each poll while visible — with the final
off-graph stride appended to the routed path, or a follow stalls on the nearest
node — degrading to last-seen on lost sight, catching up at 2 m), and every
ending returns a **nudge** the engine feeds to `scheduler.prioritize`, the same
priority lane an addressed `say` uses — `tell_way` also hands its receiver that
lane, so the ask-the-way chain runs off stage. The errand rung is **exempt from
the conversation hold** (play-tested: holding it left a "meet me at the
Gradine" walker standing ~20–30 s after their goodbye — the exchange's warm
memory — which reads as broken, not polite): leaving is the character's own
decision, made in the same reply as the goodbye, and a **fresh errand sets off
the very tick the round sees it** (`tick_intents` lays the first walk on the
deadline-stamping tick — even the ladder's 1–6 s cadence read as hesitation);
a fresh addressed line still stops them to answer, with the resume going
through the ladder cadence so the pause is the answer's beat, and `stop {}` is
the way the mind chooses to stay. The sheet
gained `places_you_know` (after `you_are`; sorted by name; `place_id` keys) and
`moving` on `you_see` people (`!is_settled()`); `the_hour` had landed with M0.
The four water sounds flipped `actor_emittable` (the M3 deferral), `turn.j2`
lost the "like walking somewhere" clause and teaches the three verbs, and **all
20 golden fixtures were regenerated once** — the documented path is the
`#[ignore]`d `regenerate_golden_fixtures` test. Verified: 40 round tests incl.
the full errand / follow / lapse / preemption / tell_way loop end-to-end on the
committed graph, engine-level nudge wiring, sheet rendering, 423 sim tests
green, headless run seeds `wayfinding: 477 places in the registry (401 homes)`.

**How you know:**

```sh
cargo run -p cathedral-backends --bin cathedral-headless -- -t 10 -v
```

An NPC issues `go_to`, the prompt log shows it, and their position starts moving toward the right
place. Then, in the game: **tell someone to meet you at the Gradine. Walk there. They are there.**

And the knowledge loop, headless: give one NPC a goal that names a place only a *second* NPC holds
the id for. The first walks coarse (a ward id everyone has), asks; the second answers with `say` +
`tell_way`; the first — nudged, holding the id — sets off and arrives. Learn → walk → ask → be told
→ walk, end-to-end, with `--fake` or live.

---

## M6 — The Night Office

**Ships.** The second cognition lane (one in flight; yields absolutely to the player; drops silently).
Individual reflection for the 31 Majors, at their own bedtimes. Ward-batched reflection for the 120
Minors. Code-rolled Rounds for the 350 Ambients. The `set_round` verb.

Roughly **39 provider calls per game day.**

**How you know.** Play an evening, sleep the night, come back in the morning: **an NPC's goal has
changed because of something that happened to them yesterday**, and their day is slightly different
for it. And the harder test — watch the foreground latency while a Night Office is running. If the
player's reply is ever slower because a Major was reflecting, the lane is wrong.

---

## M7 — Crowds and gait

**Ships.** Lane offsets (or you get a conga line). Local avoidance on stage..
The Needle's one-person claim. The lamplighter's dusk round. `VisibilityRange`. The shadow cascade.

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
