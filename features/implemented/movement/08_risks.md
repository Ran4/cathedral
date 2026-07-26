# Risks, gaps, and the decisions I need from you

---

## 1. Three decisions that change the shape of the work

### (a) How long is a game day?

**Decided: 1 game day = 1 real hour (24×), NPC walking speed 1.8 m/s.** (The speed was re-cut from the
old 1.2 m/s; the lore's one pedestrian timing in `04_routes_and_sightlines.md` — west doors to Tally
Bridge, ~360 m on the ground — now reads *"roughly three to four minutes"* to match.) The arithmetic:

| 1 game day = | C | 60 m — the ward well | 200 m — across your ward | 500 m — across town |
|---|---|---|---|---|
| 24 real min | 60× | 33 game min | 1 h 51 m | 4 h 38 m |
| **60 real min (chosen)** | **24×** | **13 game min** | **44 game min** | **1 h 51 m** |

**60 real minutes is Skyrim's number.** Local life reads perfectly, a full day/night cycle fits in a
sitting, and the cross-town cost is handled by content (people live where they work — and
`planning_ward` says they already do) plus the bounded **Long Errand** rule ([01](01_the_clock.md) §6).
It is one number in `config.ron` (`seconds_per_day: 3600.0`); M0's `T` key still lets you watch a whole
game day in a minute.

### (b) The player walks at 8 m/s

`WALK_SPEED = 8.0` (`controller.rs:43`), `RUN_SPEED = 12.0`. That is **six times a human walk**. It
was surely chosen because the city is a kilometre across and walking it at 1.4 m/s is tedious.

Two consequences once NPCs move at 1.8 m/s:

- **They will still read as slow next to you.** The player outpaces them more than four to one.
- **You cross the 20 m hearing radius in 2.5 seconds.** You can outrun a conversation by accident.

I would drop the player's **walk** to ~4 m/s and leave the **run** at 8–12, so that walking beside
someone is possible and running across town is still fast. But this is your game's feel and I am not
going to change it in a movement plan. Flagging it because movement is what makes it visible.

### (c) Buildings have no interiors

Every building is a solid extruded prism. Only the Lanthorn has an inside. So "go home and sleep"
means "walk to your door and vanish".

That is fine — it is what Fallout does, and it is a large *win* at night (the rendered crowd collapses
at curfew, which is also exactly what curfew should feel like). But it needs an explicit `Indoors`
state ([02](02_navigation.md) §7), and it has one ugly consequence:

> **You will hear people talking inside houses.**

The sim has no occlusion. `Sight::line_of_sight` always returns `true` (AGENTS.md, "Known gaps"), and
hearing is pure 3D distance within 20 m. Today nobody notices, because everybody is outdoors. The
moment half the cast is behind a wall, they will.

Minimum fix: **an `Indoors` actor neither hears nor is heard by an outdoor actor**, unless within a few
metres of the door. A special case, not a raycast. It will do, and it is honest about what it is.

---

## 2. The one thing that must not be skipped

**`context_hash` must count only *settled* neighbours, and it must land before the first NPC walks.**

`attention.rs::context_hash` gates idle turns on the set of ids within 20 m. Once people walk, that set
churns constantly, `require_news` is satisfied nearly always, and the round-robin stops skipping. The
scheduler then runs at its hard ceiling — one turn per second, ~3,600 an hour — for as long as the game
is open. Today, standing alone in a field costs **zero calls**.

The fix is one sentence — *a man crossing the square does not make you think; a man who stops in front
of you does* — and it is a **better rule than the current one even without movement**. But if it ships
after the walkers do, there will be a build in which the token bill silently multiplies and nobody
knows why.

Full detail: [05_the_llm_seam.md](05_the_llm_seam.md) §5.1.

---

## 3. Bugs this feature would expose (that are already there)

The first one is not hypothetical — I found it while writing this, and it is the reason
[M1](07_milestones.md#m1--the-bake) exists.

| | |
|---|---|
| **106 buildings have a front door you cannot walk to** (`city/mod.rs:920-923`) | The door edge is `stable_hash(&building.id) % polygon.len()` — a pure FNV-1a hash that takes no account of what is on the other side of that wall. I reimplemented the rule and tested it ([`door_probe.py`](door_probe.py)): **2,154 of 2,566 doors reachable (83.9%); 106 (4.1%) have no walkable ground within six metres.** It has never mattered because nothing in the game walks up to a door. Fix: pick the edge by reachability, tie-broken by `stable_hash` — deterministic, still varied, takes it to 99.9%. **Changes the rendered geometry too, and should.** |
| **`CharacterState::inbox` is unbounded** (`character.rs:87`) | Only `recent_history` is capped (32). The inbox drains on prompt render — and under the stage gate, an ambient NPC in a far ward **may never be prompted**. Their inbox grows for the whole session. Latent today because so little happens; movement makes things happen. **Bound it.** |
| **No sound occlusion** | See §1(c). |
| **`characters_within` is an O(n) brute-force scan** (`world.rs:132-140`) | No spatial index. Called per prompt render and once per poll. Fine for statues; becomes the hot loop when everything moves. Perf-doc item 4's shared grid is the fix. |
| **`update_actor_focus` sorts all 500 actors every frame** (`targeting.rs:99`) | To answer "who am I looking at". Wrong today, loud once they move. |
| **`MAX_ACTORS = 1_024`** (`model.rs:24`) | Fine at 500; trips at the perf doc's 1,500 target. |

---

## 4. Content gaps

These need a person, not a compiler.

| Gap | Size | Notes |
|---|---|---|
| **`occupation_id` → workplace area** | ~150 lines | `occupations.json`'s `lore_locations` are **prose** (*"The Tallage"*, *"Ward wells"*, *"Households throughout Ombreval"*). ~40 of 65 map cleanly onto existing areas. Must be authored once by hand. **This is the only genuinely new content the feature needs.** |
| **Five trades have no site at all** | small | `baker` — *"Bakery site not fixed"*; also `smith`, `brewer`, `bellfounder`, `executioner`. Pick from the 434 `workshop` / 56 `industrial` footprints, or let them work from home (which for a medieval baker is *correct*). |
| **The Bell and Ladle is not an area** | one entry | Named in **4 of the 20 authored routes** and it does not exist in `areas.json`. Add it, at the Bellstand. |
| **The 20 routes join by name, not id** | one afternoon | `05_dramatis_personae.md` uses slugs (`havise-dorn`); the sheets use 5-char ids (`ak3vd`). Join once, and **assert all twenty resolve** — a silent mis-join gives the Praelucent a fuller's day. |
| **Diffuse workplaces** | design | `domestic_servant` (45 people — the largest occupation), `scavenger`, `sanitation_worker`, `messenger`. Their workplace *is* the street. They want a **circuit** Leg, not a post. House → well → market → house is the most-walked route in Ombreval and it needs its own shape. |

---

## 5. Things I decided, and what I rejected

| Decision | Rejected alternative | Why |
|---|---|---|
| **The sim moves NPCs** | Bevy moves them and pushes `SpatialUpdate` (the `Vec<SpatialActorUpdate>` shape already allows it) | Breaks `cathedral-headless`, breaks deterministic tests, and makes the sim's authority over hearing/stage/perception depend on the renderer being right. `engine.rs:967-979` forbids it, deliberately, and the reason it gives is still true. |
| **Bake from the cadastral plan** | Bake from `CollisionWorld` | `CollisionWorld` cannot exist without Bevy, and the sim must navigate headless. They agree by construction (the colliders are *built from* the plan) — and a test in the game crate can prove it. |
| **Hand-rolled 2D nav** | `vleue_navigator` / `oxidized_navigation` / `bevy_landmass` | All Bevy-coupled. The sim's `Cargo.toml` says *"Deliberately frozen dependency set: no bevy, no tokio, no network, no clock reads, no file reads."* And the problem is 2D on a flat plane over a static world — the easiest possible case. |
| **A priority ladder** | A utility/GOAP/behaviour-tree system | seagame proves 17 `if`s produce genuinely divergent lives. A ladder is debuggable (*"why is he doing that?"* → read down the list) and extensible by adding a rung, not by retuning a weight matrix. |
| **The clock in the sheet, not the inbox** | Emit the office bell as a percept | 7 offices × 500 NPCs = 3,500 inbox lines a game day, into an unbounded inbox. `lore/second_sun/design/06` §5 already ruled: *"The offices are a clock, not events."* |
| **No RNG** | `rand` in the sim | The sim is deterministic by decree. `attention.rs:683-699` already established the idiom: pure hashes of `(salt, actor, context)`, never fresh draws. |
| **Fake gait** | Skinned glTF characters | `performance_improvements.md` item 7. Skinning breaks the instancing that makes 500 (and 1,500) affordable. |
| **Schedule off the bells** | Schedule off the light | `features/50_cool_suggestions.md` #6, *The Rose Meridian* — *"actors schedule their day around"* the light — is in the **NoWay** tier. Bell-driven scheduling is not. |

---

## 6. Open questions I could not answer from the repo

1. **Does the cast need to grow?** `performance_improvements.md` targets 1,500. A city of 5,000
   (the lore's post-Hammering population) showing 500 is already a big abstraction. Movement makes the
   500 *feel* like more, because the same person is in different places. It might be enough.

2. **What happens at the walls?** `features/food_and_items/08_near_countryside.md` proposes
   spawning arrivals at the gates — *"Actors arriving through a gate would have grounded destinations,
   kin, cargo, and news instead of being generic travelers from nowhere."* The five gates are already
   areas. That is a natural M8 and it makes the walls feel like the edge of the play space rather than
   the edge of existence. Out of scope here, but the Round schema should not make it awkward.

3. **The countergait.** If you add a day/night cycle you have implicitly promised the second sun —
   which walks *backwards* (dawn-showing → strong hour → the Passing). **Two of the twenty authored
   routes already peg themselves to it** (Dorn's *"the crossing at the strong hour"*, Ferrant's *"the
   nave at the strong hour"*). Keep the sun's angle a pure function of `WorldTime` so a second one is
   trivial to add, and do not forget you owe it.

4. **Ward politics as a schedule modulator.** `features/lore_ward_politics.md` says election results
   change *"gate hours, stall licences, **watch routes**, shoring orders, well repairs"* — i.e. it is
   *designed* to modulate exactly the things this system owns, and `planning_ward` is the join key. I
   have not built for it, but I have not built against it.

---

## 7. The acceptance test for the whole design

> **Turn cognition off — `fake_backend: true`, or just pull the API key — and the city still gets up in
> the morning, walks to the well, works, goes to the market on Highmarket, and is indoors by the
> Snuffing. It just does it in silence.**

If the city stops moving when the LLM stops answering, the layering is wrong and something has climbed
from L4 down into L2 where it does not belong.
