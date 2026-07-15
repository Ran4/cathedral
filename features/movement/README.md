# Movement: the city gets up in the morning

Status: proposed. No code written. This folder is the plan.

| | |
|---|---|
| **The brief** | `features/movement.md` |
| **The engineering prerequisite** | `features/performance_improvements.md` — *"Performance: 1500 actors, and they walk"* |
| **This folder** | the design the brief asked for |

---

## 1. The one-paragraph version

The city already has everything a movement system needs except the movement system. It has a
validated cadastral plan with 2,566 building footprints, 49 street centrelines and 69 place
anchors. It has 70 named areas that the NPC prompt already speaks in prose. It has 500 authored
characters with a ward, a trade and a spawn point. It has nine wells with walkable queue aprons and
four unused water sounds waiting in the catalog with a comment that says *"flip `actor_emittable`
and a keeper can work the curb"*. It has a 600 m town bell nobody rings because there is no clock.
And it has, in `lore/second_sun/05_dramatis_personae.md`, twenty hand-written daily routes for the
twenty characters who matter most.

What it does not have is a clock, a walkable surface, and a reason for anyone to take a step. This
plan adds those three things, in that order, and then lets the existing content do the work.

---

## 2. The thesis: this is the layer the attention gate already asked for

`AGENTS.md` says it, at the end of the section on idle cognition:

> **Known consequence.** With the gate in and nothing behind it, the city outside the stage stops
> moving: no errands, no autonomous movement, no gossip. That is the accepted trade, and it promotes
> the non-LLM behavior layer from a nice-to-have to a dependency.

That non-LLM behaviour layer is this document. The gate was not a compromise that movement has to
work around — it is a hole that movement was always going to fill. The gate says *"only six people
near the player may think"*; movement says *"everyone else still lives, they just don't need a model
to do it."*

The brief asks for the same thing from the other end: *"we need a solution that's mostly
code-driven."* Same layer. Two people arriving at it from opposite directions is a good sign.

---

## 3. The architectural claim

**The LLM is not the brain. It is a voice, a memory, and an occasional change of mind.**

Everything an NPC does with their *body* — where they go, when, and why — is code. The LLM keeps
exactly the powers it has today (speech, memory, goals, item exchange) and gains exactly two more:

- it may **redirect** a body, by naming a place (`go_to {"place": "wickmarket"}`), which the code then
  carries out and which **expires**;
- once a day, at that character's own bedtime, it may **rewrite its own agenda** (the Night Office).

This is the shape `~/seagame` arrived at, and the single most important property it has is this:
**if the LLM has said nothing, the body carries on by itself.** In seagame, `tryExecuteCommand`
preempts the behaviour ladder — and if the command queue is empty, the ladder runs. An LLM call that
takes three seconds, fails, or never arrives costs nothing at all, because nobody was waiting for it.

Five layers, bottom to top. Each one is useful without the one above it, which is what makes the
milestones in §8 shippable one at a time.

| | Layer | Owns | LLM? |
|---|---|---|---|
| **L0** | **The Clock** | the seven offices, the week, the sun | no |
| **L1** | **The Body** | the walkable surface, routes, steering, the gait | no |
| **L2** | **The Ladder** | needs → conditions → first-match-wins behaviour | no |
| **L3** | **The Round** | home, work, and the daily itinerary | no |
| **L4** | **The Mind** | `go_to`; the Night Office | **yes** |

Deep dives: [01_the_clock.md](01_the_clock.md), [02_navigation.md](02_navigation.md),
[03_the_ladder.md](03_the_ladder.md), [04_the_round.md](04_the_round.md),
[05_the_llm_seam.md](05_the_llm_seam.md).

---

## 4. The load-bearing decision: the sim moves people, not Bevy

The tempting design is the wrong one. `EngineCommand::SpatialUpdate` already takes
`updates: Vec<SpatialActorUpdate>` — a *list*. Bevy could walk everyone with a character controller
and push the whole cast through it every tick. It would work. It is wrong, and the codebase already
says so, at `crates/cathedral-sim/src/engine.rs:967-979`:

```rust
// Only the player moves. NPC positions are static world state; a client
// that tries to move one is confused, and letting it would silently
// rewrite the cast's geometry (`server.py:924-929`).
```

**The sim owns NPC position.** Four reasons, in order of force:

1. **Everything social keys on position.** Hearing (20 m), the stage (32 m), `you_see`, the sound
   witness cone, `context_hash`. If the host owned position, the sim's authority over its own rules
   would depend on the renderer being right about geometry. That is not a seam you want.
2. **`cathedral-headless` is the stated fastest iteration loop** — AGENTS.md: *"the fastest way to
   change the prompt, the scheduler or an action verb and see what it does."* A daily round you
   cannot run headless is a daily round you cannot iterate on.
3. **`cargo test -p cathedral-sim` is offline and deterministic.** *"Does Sibbe Vell reach the oven
   before the Kindling bell?"* should be a unit test, and with the sim as the mover it is one.
4. **The data pattern already exists.** The sim takes `areas.json`, `catalog.toml` and `seed.json` as
   `&str` — *"The loaders take `&str`, never a path: the host reads the file."* A baked navigation
   graph is one more of those. Nothing about pathfinding requires Bevy: it is 2D geometry on a flat
   plane, and `glam` is already a dependency.

The cost is that pathfinding, steering and local avoidance must be written in pure Rust inside
`cathedral-sim`. That is about 700 lines of arithmetic with no IO, no clock and no threads. It is
worth it.

Bevy's job is unchanged and already written: `reconcile_actor_views`
(`src/smart_actors/actors.rs:173-187`) already writes `transform.translation` and
`transform.rotation` from the snapshot on every frame. **The render channel exists. The moment the
sim's positions change, the bodies move.** What Bevy adds is interpolation between movement ticks, a
fake gait, and the sun.

---

## 5. What the lore already decided, and we should not re-decide

An unusual amount of this design is not a design choice. It is already written down.

**The day has seven offices** (`lore/core_lore/trade_and_daily_life.md`, and again in
`lore/second_sun/11_glossary_and_naming.md`):

> 1. **the Watch** — deep night; 2. **the Kindling** — before light; 3. **Dayspring** — sunrise;
> 4. **High Wick** — noon; 5. **the Waning** — mid-afternoon; 6. **Lamplight** — sunset;
> 7. **the Snuffing** — curfew.

**The week has three shapes**: Bellday (holy day, 1st), Highmarket (3rd — the Wickmarket and
Coswald's Yard), Lowmarket (6th — the Tallage and Maren's Green). Market day decides *where the
crowd is*.

**The bells are already spec'd as mechanics** (`lore/second_sun/design/06_the_sound_of_the_city.md`
§3): each office rings its own ordinal at 3 s intervals — the Watch one stroke, the Snuffing seven —
*"A player anywhere in the city learns the hour by counting."*

**And the same document already solved the worst cost problem in this plan** (§5):

> **The offices are a clock, not events.** Seven percepts per actor per day would be token waste:
> Evenblow instead updates the **scene-header time-of-day** every actor already receives (*"the last
> office rung was the Waning"*). **Only *deviations* from the daily round are events.**

That is exactly right and it is not obvious. It is spelled out in
[05_the_llm_seam.md](05_the_llm_seam.md) §2, because it is the difference between a bell that costs
nothing and a bell that costs 3,500 inbox lines a day.

**Walking speed is set at 1.8 m/s** — a brisk, purposeful medieval pace. The lore's one pedestrian
timing was re-cut to match. `lore/places/04_routes_and_sightlines.md`:

> The outdoor part from west doors to Tally Bridge is roughly **three to four minutes**.

West doors ≈ `(0, 81)`, Tally Bridge ≈ `(-305, 105)` — about 310 m as the crow flies, more on the
ground. At 1.8 m/s that ~360 m walk is ~3½ minutes. (It was first written at 1.2 m/s — the slower
amble "six to seven minutes" implied; movement takes the brisker pace so cross-town trips read better
under the 24× clock, and the lore follows the number.)

**Twenty daily routes are already authored.** `lore/second_sun/05_dramatis_personae.md` gives every
major character a `route:` line, and they all have the same shape — two to four legs, each pegged to
an office or a market day, each naming an area that exists in `areas.json`:

> **Verger Dunstan Pike** — *unbars the west doors for the dawn-showing; rounds all day; locks at
> dusk; then the Bell and Ladle*
>
> **Mistress Jonet Sparr** — *furnace lit before dawn; workshop all day; the Lanthorn only when the
> Fabric summons, never alone*
>
> **Wyn Alder** — *the Moorings yard before dawn; the fish market; walking pilgrims to the Old Sluice
> for a penny; the nave on Bellday*

That is the schema for L3, discovered rather than invented. See [04_the_round.md](04_the_round.md).

---

## 6. The walkable surface: I baked it, and it works

Rather than assert that a navmesh is feasible, I baked one. The script is
[`walkable_probe.py`](walkable_probe.py) in this folder; the output is
[`walkable_probe.png`](walkable_probe.png). Real numbers, from the shipped
`lore/places/ombreval_buildings.json`:

| | |
|---|---|
| Grid at 0.25 m | 4125 × 4705 = 19.4 M cells → **2.43 MB as a bitset** |
| Inside the wall | 111.9 ha |
| Minus 2,566 buildings + 91 fixtures | 71.3 ha |
| **Road cells covered by a building or fixture** | **5,622 m²** ← the "roads win" rule is not optional |
| Free, roads carved back in | 71.8 ha (64.2% of intramural) |
| Eroded by the 0.35 m agent radius | **65.9 ha walkable** |
| Connected components | 2,744 |
| **Largest component** | **65.8 ha — 99.9% of all walkable area** |
| Everything else | 2,743 slivers totalling ~40 m² |
| Buildings with ≥1 reachable door *candidate* | 2,564 / 2,566 (99.9%) |
| **Buildings whose *actual* front door is reachable** | **2,154 / 2,566 — 83.9%** ← a real bug |

Four things fall out of that, and they are the whole navigation design:

**The city is one connected region.** 99.9% of the walkable area is a single component. Drop the
slivers and there is nothing to worry about.

**"Roads win" is mandatory.** 5,622 m² of road sits underneath a building or fixture footprint —
the malt-house *over* Malt Passage, the three bridge upper-stores, the covered Draper's Reach, and
the sixty market stalls standing in the street. Subtract building footprints naively and you wall
off the city's covered passages. Roads must be carved back in *after* the buildings are subtracted.

**Interior anchors must resolve to their door.** Fifteen of the 69 named places came back
unreachable or far. Every single one is an interior — The Lanthorn, the chapter house, the
toll-house, the bonded warehouse, the Ilvane Chapel, the glaziers' guildhall, the masons' lodge, the
gates. That is correct, not a bug: their anchor is *inside* a footprint. The bake needs an explicit
rule that an interior anchor routes to its building's entrance.

**And 106 buildings have a front door you cannot walk to.** `src/city/mod.rs:920-923` gives every
building exactly one door, on the edge chosen by `stable_hash(&building.id) % polygon.len()` — a pure
FNV-1a hash, taking no account of what is on the other side of that wall. So I implemented the real
rule and tested the real doors ([`door_probe.py`](door_probe.py)):

```
THE ACTUAL stable_hash-chosen doors, 2566 buildings:
  reachable (<=1.0 m to walkable)   2154  (83.9%)
  far       (1-6 m)                  306  (11.9%)
  blocked   (no walkable within 6m)  106  (4.1%)
```

The hash cheerfully puts a front door on the edge facing a 30 cm gap between two houses. Nobody has
noticed because nobody walks. **The fix is to pick the door edge by *reachability*, tie-broken by
`stable_hash`** — deterministic, still varied, and it takes the number to 99.9% (that is what the
"≥1 candidate" row above is measuring). It changes the rendered geometry too, and it should: a door
opening onto a sealed gap is a bug in the *render* as well, it just has not mattered yet.

This is the whole argument for doing the bake first. It is a real defect, in shipped code, that only
a walking NPC would ever have found.

Full design in [02_navigation.md](02_navigation.md).

---

## 7. The three things that could blow the budget, and their fixes

This is the part I would want reviewed hardest. Movement's real cost is not frames — the perf doc
has the frames. It is tokens, and there are exactly three ways it goes wrong.

### 7.1 The novelty gate churns — **prerequisite, not follow-up**

`attention.rs::context_hash` decides whether an on-stage actor gets an idle turn by hashing **the set
of ids within 20 m**. The doc comment at `attention.rs:390-397` is explicit that positions were
rejected as a key because *"a neighbour's every step would otherwise be news."*

Movement re-opens that wound through the id set. Once people walk, the 20 m membership churns
constantly, `require_news` is satisfied almost always, and the round-robin stops skipping. Today,
standing alone in a field costs zero calls. After movement, with no fix, the scheduler runs at its
ceiling — one turn per second, ~3,600 an hour — forever.

**The fix is one sentence of semantics:**

> A man crossing the square does not make you think. A man who *stops* in front of you does.

Include an actor in `context_hash` only if they are **settled**: speed below ~0.15 m/s, or within
20 m for ≥ 3 s. Passers-by never enter the hash. Someone who stops near you is genuinely news, which
is exactly the meaning you want — and it is a better rule than the one we have now, independent of
movement.

**This must land before the `spatial_update` guard is lifted.** It is not an optimisation.

### 7.2 The bell would spam 500 inboxes — *already solved in the lore*

`town_bell` is audible at **600 m** (`assets/sounds/catalog.toml:38`), which is most of the city. The
sound path delivers a percept to every recipient. Seven offices × 500 NPCs = 3,500 inbox lines per
game day — and `CharacterState::inbox` is an **unbounded `Vec<String>`** (`character.rs:87`; only
`recent_history` is capped, at 32). An ambient NPC in a far ward, who under the stage gate may never
take a turn at all, would accumulate bell lines forever.

The lore's own design doc already ruled on this: *the offices are a clock, not events.* So:

- the office goes in the **sheet** — a new `you_are.the_hour` field: *"the last office rung was the
  Waning"* — where it costs nothing and never queues;
- the `town_bell` **sound** still plays, for the player's ears, and still rings from the Lanthorn;
- **no actor percept, no priority nudge, no inbox line.**

Only *deviations* are events: the Ruin (the ring rung backward — fire or flood), the name-knell, the
Scold's summons to a proclamation. Those are rare, they are meant to interrupt, and they should.

(Separately: **bound the inbox.** It is a latent leak today and movement would make it a real one.)

The good news, verified: a sound nudges **exactly one** actor into the priority lane, not all of them
— `engine.rs:1314-1327`, *"Exactly one nudge per sound: the turn stream is global and single."* So
even if we did emit the bell as a percept, it would not cause 500 turns. It would just fill 500
inboxes. Fix the inbox anyway.

### 7.3 The Night Office would starve the player

`features/quicker_response_improvements.md` names the constraint: **cognition has one global in-flight
request**, and a background actor that started thinking just before the player's transcript lands adds
its whole provider call to foreground latency.

So a nightly reflection over 500 NPCs, run through the normal scheduler, would be a disaster: 500
sequential calls at ~1 s minimum delay each is eight minutes of exclusive scheduler time during which
the player cannot be answered.

The brief already guessed the right shape (*"one huge llm prompt that updates them all once per
day"*). Concretely:

| tier | count | Night Office |
|---|---|---|
| **Major** | 31 | individual reflection, staggered across the game-night |
| **Minor** | 120 | **batched by ward** — one prompt per ward per night (8 prompts) |
| **Ambient** | 350 | **no LLM at all.** Agenda re-rolled in code from occupation + seed |

31 + 8 = **39 calls per game day** — about 39 an hour at the default clock, trickled through the
quiet hours in a lane that never submits while the floor is busy, while the player is composing, or
while anyone is on stage. Details in [05_the_llm_seam.md](05_the_llm_seam.md) §4.

---

## 8. The first vertical slice should be water

Not because it is easy, but because **it is already 80% built and nobody has noticed.**

- **Nine named public water sources**, every one already an area in `areas.json`: `ford_well`,
  `chain_well`, `bitter_well`, `three_curb`, `lodge_well`, `shambles_well`, `slate_cistern`,
  `tenter_cistern`, `reed_cistern`, `step_cistern`, `seven_lofts_tanks`.
- **The queue space is already walkable geometry.** `src/city/water.rs:14-16`: *"the collision follows
  the stonework, not the footprint: the curb, posts, troughs and vault stop you, while **the queue
  space, apron and roof shelter stay walkable**."* Someone built the place for a queue to stand in.
- **Four water sounds already in the catalog** — `draw_water`, `chain_windlass`, `pour_trough`,
  `pail_clatter` — with this comment sitting above them:

  > *"These are world sounds like the bell, not actor choices: the sim has no water items, actions or
  > source state yet, so nothing in it can decide to draw a bucket. The percepts are written for the
  > day it can — **flip `actor_emittable` and a keeper can work the curb.**"*

- **The queue has authored rules.** `lore/wells_and_water.md`: *"A household vessel takes precedence
  over a trade vessel in an ordinary queue. Bulk users draw by a turn-list."* And the keeper *"controls
  the queue, and can close a suspect source."*
- **The most common journey in the city is already lore.** `domestic_servant` is the single largest
  occupation (45 of 500), and its `lore_locations` are *"Households throughout Ombreval, **Markets and
  wells**, Merchant and clerical houses."* A servant's morning trip to the ward well and back is the
  most-walked route in Ombreval, and it was written down before anyone could walk it.

So **M3 is the water round**, and it exercises the entire stack end to end: clock → need → condition
→ ladder → route → walk → queue → act → *sound* → and the sound is heard by the LLM layer, which
means an NPC can be asked about it. It is audible, it is visible, it is lore-perfect, and it costs
zero tokens.

---

## 9. Milestones

Each one is shippable, and each one has a way to *see* that it works using the tools this repo
already has (`cathedral-headless`, `CATHEDRAL_DRIVE`). Full recipes in
[07_milestones.md](07_milestones.md).

| | | Ships | How you know |
|---|---|---|---|
| **M0 ✅** | **The Clock** *(implemented)* | seven offices, the week, the sun moves, the bell rings, a HUD readout, a debug time-scale key. **Nobody moves.** | `CATHEDRAL_DRIVE='wait-online; key KeyT; key KeyT; sleep 20; shot dusk'` — the sun has moved, the HUD reads the office and hour |
| **M1 ✅** | **The bake** *(implemented)* | `assets/world/navigation.json` + the walkable bitset, the door fix, an F7 debug overlay, and the connectivity tests. **Still nobody moves.** | `cargo test -p cathedral-sim navigation` — every named place and every door reachable; `key F7` draws the graph |
| **M2** | **One NPC walks** | hot/cold split, fixed tick, interpolation, the guard lifted. One hard-coded actor paces between two places forever. | headless prints their position advancing; a drive `shot` finds them somewhere new |
| **M3** | **The water round** | needs → conditions → ladder → route → queue → draw → home. The vertical slice. | you can *hear* the windlass from thirty metres away, and ask the drawer about it |
| **M4** | **The Round** | homes, workplaces, the 20 authored routes, market days, and the Snuffing emptying the streets | headless `-t` across a full day prints a plausible city; stand in the Wickmarket at curfew and watch it clear |
| **M5** | **`go_to`** | the LLM verb; the prompt and sheet change; 20 golden fixtures regenerated | tell someone to meet you at the Gradine, and walk there, and they are there |
| **M6** | **The Night Office** | reflection, agenda rewriting, the second cognition lane | an NPC's goal changes overnight because of something that happened to them yesterday |
| **M7** | **Crowds and gait** | lane offsets, local avoidance, the lamplighter's round, the Needle's pinch | it looks like a city |

M0 and M1 are independent of each other and of everything else. **M1 is where the actual risk lives**
— get the bake green before touching the sim.

---

## 10. What I want a decision on

Laid out properly in [08_risks.md](08_risks.md). The three that change the shape of the work:

**(a) How long is a day? — decided: 1 real hour (24×).** At 1.8 m/s the arithmetic reads:

| 1 game day = | compression | 60 m, to the well | 200 m, across your ward | 500 m, across town |
|---|---|---|---|---|
| 24 real min | 60× | 33 game min | 1 h 51 m | 4 h 38 m |
| **60 real min (chosen)** | **24×** | **13 game min** | **44 game min** | **1 h 51 m** |

The chosen **1 game day = 1 real hour** is Skyrim's number: local life reads perfectly and a full
day/night cycle fits one sitting. The cross-town cost is handled by content (medieval people lived
where they worked — and `planning_ward` says they still do) plus an explicit, bounded **Long Errand**
rule for the rare cross-city trip. It is one number in `config.ron` (`seconds_per_day: 3600.0`).

**(b) The player walks at 8 m/s.** `WALK_SPEED` in `controller.rs:43` — more than four times an NPC's
1.8 m/s. NPCs will still read as slow beside you, and you cross the 20 m hearing radius in 2.5
seconds. I would drop the player's walk to ~4 m/s and leave the run at 8–12, but that is a feel
question and it is yours.

**(c) Buildings have no interiors.** They are solid extruded prisms; only the Lanthorn has an inside.
So "go home and sleep" means "walk to your door and vanish". That is fine — Fallout does it, and it
is a large perf *win* at night — but it needs an explicit `Indoors` state, and it makes the sim's
existing lack of sound occlusion glaring the moment half the cast is behind a wall.

---

## 11. Files in this folder

| | |
|---|---|
| [01_the_clock.md](01_the_clock.md) | the seven offices, the week, the sun, the bells, the numbers |
| [02_navigation.md](02_navigation.md) | the bake, the graph, the probe results, what to do about doors |
| [03_the_ladder.md](03_the_ladder.md) | needs, conditions, the priority ladder — what we take from seagame and what we leave |
| [04_the_round.md](04_the_round.md) | homes, workplaces, the 20 authored routes, market days, curfew, the gate-caught boatmen |
| [05_the_llm_seam.md](05_the_llm_seam.md) | `go_to`, the Night Office, the prompt and sheet changes, the token budget |
| [06_engineering.md](06_engineering.md) | sim vs host, the hot/cold split, the tick, LOD, determinism |
| [07_milestones.md](07_milestones.md) | M0–M7, each with a verification recipe |
| [08_risks.md](08_risks.md) | open questions, content gaps, and the decisions I need from you |
| [walkable_probe.py](walkable_probe.py) | the script that produced §6's numbers — run it, it takes 40 seconds |
| [walkable_probe.png](walkable_probe.png) | the baked walkable surface of Ombreval |
| [door_probe.py](door_probe.py) | reimplements `stable_hash` and tests the *real* doors. This is the one that found the bug. |
