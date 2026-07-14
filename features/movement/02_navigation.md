# L1 — The Body: navigation

The riskiest milestone and the one to do first, because it can be finished and *proved* without
touching the sim, the prompt, or a single NPC.

---

## 1. I baked it. Here are the numbers.

[`walkable_probe.py`](walkable_probe.py) in this folder rasterises the shipped cadastral plan,
erodes it by the agent radius, floods it, and reports what is actually reachable. It takes about 40
seconds. [`walkable_probe.png`](walkable_probe.png) is what it looks like.

```
world  x:[-518,513] z:[-683,493]  = 1031 x 1176 m
grid   4125 x 4705 = 19.4M cells @ 0.25 m  (2.43 MB as a bitset)

inside wall                          111.9 ha
minus 2566 buildings + 91 fixtures    71.3 ha
road cells blocked by a building/fixture: 89948  (5622 m2)   <- 'roads win' matters
free (roads win)                      71.8 ha  (64.2% of intramural)
eroded by r=0.35 m                    65.9 ha

components             2744
largest component      65.8 ha  (99.9% of walkable)
top component sizes    658446 m2, 26 m2, 3 m2, 3 m2, 2 m2, 2 m2

named places (69): 54 within 6 m of the main component
buildings with >=1 reachable door candidate: 2564/2566  (99.9%)
```

Four conclusions, and they are the design:

### The city is one connected region

2,744 components sounds alarming and is not. The largest is **99.9% of the walkable area**; the other
2,743 total about **40 m²** — slivers of gap between adjacent buildings, one and two cells wide.
Discard any component under, say, 4 m² and the answer is: **Ombreval is one room.** No island
handling, no connectivity repair, no hand-authored links.

### "Roads win" is mandatory, not an optimisation

**5,622 m² of road** lies underneath a building or fixture footprint. That is not noise. It is:

- **the malt-house over Malt Passage** (`named_malt_house` — *"The malt-house over Malt Passage"*),
- **the three bridge upper-stores** (`use == "bridge"`: the Chain Bridge upper store, the Tally Bridge
  upper passage, the Eel Bridge gallery),
- **the covered Draper's Reach** (`tier: "passage"` — the *sheltered* trade route, which
  `04_routes_and_sightlines.md` says is what *"keeps the Reach busy in rain"*),
- and **sixty market stalls** standing in the street.

Subtract building footprints naively and you brick up the covered passages, which are among the most
characterful spaces in the city — and you make the Reach, a named canonical route, impassable.

**Rule: subtract buildings and fixtures, then carve the road corridors back in.** A building over a
road is an overhead structure, by construction. That single rule is the difference between a working
bake and a broken one, and it is worth an assertion: *every road centreline must be walkable end to
end after the bake.*

### Interior anchors must resolve to a door

Fifteen of the 69 named places came back unreachable or "far". All fifteen are interiors: The
Lanthorn, the chapter house, the Tallage toll-house, the bonded warehouse, the Ilvane Chapel, the
glaziers' guildhall, the masons' lodge, Lise Copp's pawnshop, Doctor Ferrant's house, the brine
cellar, Malt Passage, the Bellstand tower, and the gates.

That is *correct*. Their `named_place_index` anchor is *inside* a footprint, because the place is
inside a building. The bake needs an explicit rule:

> An anchor that falls inside a building footprint resolves to that building's **entrance**, not to
> the anchor itself.

The one real exception is **The Lanthorn**, which is a solid polygon in the cadastral plan but has a
genuine walkable interior built by `src/scene.rs`. It needs an explicit *interior carve-in*: a list of
polygons that are re-added as walkable after the buildings are subtracted. Start with the cathedral
floor; the list will stay short, because no other building has an inside (§6).

### The doors already exist — and 106 of them open onto a wall

`src/city/mod.rs:920-923`, inside `add_facade_openings`:

```rust
if edge_index == stable_hash(&building.id) as usize % building.polygon.len() {
    let center2 = a + direction * (length * 0.5) + normal2 * 0.045;
    add_facade_panel(doors, center2, base_y + 1.25, direction, normal, 1.35, 2.5);
}
```

Every building gets **exactly one door**, on the edge picked by `stable_hash(id) % polygon.len()`
(FNV-1a, `city/mod.rs:1949-1953`), at that edge's midpoint, pushed 0.045 m out along the outward
normal. Baked into a batched mesh — no entity, no component, no data. **But the rule is four lines
and completely deterministic**, so it can be recomputed from the plan alone. That is 2,565 front
doors, free, in exactly the places the player can see them.

Except that the hash **takes no account of what is on the other side of that wall**. So I
reimplemented the real rule — FNV-1a and all — and tested the real doors
([`door_probe.py`](door_probe.py)):

```
THE ACTUAL stable_hash-chosen doors, 2566 buildings:
  reachable (<=1.0 m to walkable)   2154  (83.9%)
  far       (1-6 m)                  306  (11.9%)
  blocked   (no walkable within 6m)  106  (4.1%)
```

**One building in twenty-four has a front door with no way to stand at it.** The hash cheerfully
places it on the edge facing a 30 cm gap between two houses. It has never mattered, because nothing
in the game has ever tried to walk up to a door.

Note the difference between this and the "≥1 reachable door candidate: 2,564/2,566" figure in the
first probe. That one asked *"does this building have **any** edge you could put a reachable door
on?"* and the answer is essentially always yes. This one asks *"is the door that is **actually there**
reachable?"* and the answer is no, 106 times.

### The fix

**Pick the door edge by reachability, tie-broken by `stable_hash`.**

```
candidates = edges whose midpoint + 0.8 m outward lands on walkable ground
door_edge  = candidates[ stable_hash(id) % candidates.len() ]   // still varied, still deterministic
           // if no candidate: the building has no door, and nobody lives there
```

Still a pure function of the building id. Still deterministic. Still spreads doors around the
façades instead of putting them all on the north face. And it takes reachability from 83.9% to
**99.9%** — because the first probe already proved that essentially every building has *some* edge
that works.

**It changes the rendered geometry too, and it must.** The nav door and the visible door have to be
the same door, or NPCs will walk through blank walls and stand outside doors that are not there. And
a door opening onto a sealed gap between two houses is a bug in the *render* as well — it just has not
mattered, because nobody walks.

The two buildings that still have no reachable edge get **no door and no resident**. Which is correct;
they are enclosed.

**This is the whole argument for doing the bake first.** It is a real defect, in shipped code, and
only a walking NPC would ever have found it.

---

## 2. Source of truth: the cadastral plan, not `CollisionWorld`

There are two candidate obstacle sets, and picking wrong is expensive.

**`CollisionWorld`** (`src/controller.rs:132-280`) is *exactly* what stops the player: 3,000+ AABBs
and vertical convex prisms, assembled at scene build from buildings, walls, towers, gates, fixtures,
well curbs, bridge piers, the Bellstand stair, and the cathedral. It is the ground truth for
"solid".

**The cadastral plan** (`lore/places/ombreval_buildings.json`, `include_str!`-ed at `plan.rs:11`) is
the *source* `CollisionWorld` is built from. 1.4 MB, deterministic, seeded (`"seed": 11810721`),
validated on load, and — critically — **pure data with no Bevy in it**.

Take the plan, for one reason that outweighs everything else: **`CollisionWorld` cannot exist without
Bevy**, and the sim must be able to navigate in `cathedral-headless` and in `cargo test -p
cathedral-sim`. A navmesh you can only bake inside a running game is a navmesh you cannot unit-test.

The risk of divergence is real but small and *checkable*: since `CollisionWorld` is generated from the
plan, they agree by construction, and a test can enforce it —

> **Test.** Sample every cell of the baked walkable surface. Assert that no sampled point is inside a
> `CollisionWorld` solid. (Run it once, in the game crate's test suite, where `CollisionWorld` is
> available.)

That closes the loop: the sim bakes from the plan, the game proves the bake agrees with what actually
stops you.

**Not a third-party navmesh crate.** `vleue_navigator`, `oxidized_navigation`, `bevy_landmass` are all
good and all Bevy-coupled. The authoritative pathfinder lives in a crate whose `Cargo.toml` says
*"Deliberately frozen dependency set: no bevy, no tokio, no network, no clock reads, no file reads."*
Respect that. What we need is 2D, on a flat plane, over a static world — the easiest possible case.

---

## 3. What gets baked

One artifact, `assets/world/navigation.json` (+ a companion `.bin` for the bitset), generated by
`scripts/bake_navigation.py`, **checked in**, exactly as `ombreval_buildings.json` is generated by
`generate_top_down_map.py` and checked in. A test asserts the committed artifact matches a fresh bake,
so it cannot silently drift.

The sim loads it as a `&str` / `&[u8]` handed in by the host, exactly like `areas.json`.

### (a) The walkable bitset — 2.4 MB

0.25 m cells over the plan's bounding box, eroded by the 0.35 m agent radius (matching
`PLAYER_HALF_SIZE.x` in `controller.rs:40`, so NPCs and the player agree about what fits). Used for:

- *is this point walkable* — a single array index;
- validating the graph at bake time;
- clamping steering (§5);
- picking a wander target inside a square.

Not used for long-range pathfinding. 19.4 M cells is far too fine to A* across a kilometre.

### (b) The street graph — a few hundred nodes

`Road.points` is a **street centreline polyline** and `Road.width_m` is its **corridor width**. That is
a navigation graph that somebody already drew:

```
cut               THE CUT              tier=cut      w=20.0   ← the dry canal, a 20 m cartway
west_approach     WEST APPROACH        tier=major    w=7.0
fabric_way        FABRIC WAY           tier=major    w=7.0
river_cartway     RIVER CARTWAY        tier=major    w=8.5
drapers_reach     THE DRAPER'S REACH   tier=passage  w=5.0    ← covered
needle            THE NEEDLE           tier=alley    w=1.2    ← one person wide
eelback           EELBACK ALLEY        tier=alley    w=2.4
… 32 minor, 6 major, 4 wall_lane, 3 service
```

**But the roads are not endpoint-connected.** Of 204 distinct vertices, only **8** are shared by more
than one road. The 49 polylines *cross geometrically* — their ribbons overlap, which is why the raster
is connected — but they do not share vertices. So the bake must:

1. compute segment–segment intersections across all 49 polylines (49² × a few segments — trivial,
   one-time, deterministic) and **split the polylines at every crossing**;
2. weld coincident vertices;
3. emit a planar graph — roughly 300–500 nodes, 400–700 edges.

That is the long-haul router, and A* over it is a few microseconds.

### (c) Destinations

Three kinds of leaf node, all hung off the street graph:

- **The 69 named places.** Their `named_place_index` anchor, projected to the nearest walkable point,
  or — if the anchor is inside a footprint — to that building's door (§1).
- **The 23 sites** (squares, yards, courts, churchyards). These are *polygons*, not points: inside a
  site, movement is free, not on a centreline. Connect each site to every road that touches it.
- **The 2,565 building doors.** Each a leaf, joined to the nearest point on the nearest road by a
  short "driveway" edge, validated straight-line against the bitset.

So a route is: **door → driveway → street graph A* → driveway → door**, with free movement inside any
square it passes through. Which is, exactly, how you walk to someone's house.

---

## 4. What a route actually is

```rust
/// crates/cathedral-sim/src/nav/route.rs
pub struct Route {
    /// The polyline in world XZ, already string-pulled. Metres.
    pub points: Vec<Vec3>,
    /// Corridor half-width at each point — how far off the line it is safe to
    /// drift. Wide on the Cut (10 m), nothing in the Needle (0.6 m).
    pub half_width_m: Vec<f32>,
    /// Total length, so an ETA is a division.
    pub length_m: f32,
    /// What the actor means to do on arrival. The pathfind-then-act bridge.
    pub on_arrival: Arrival,
}

pub enum Arrival {
    Work, Sleep, DrawWater, Eat, Pray, Trade, Idle, Talk(ActorId), Stand,
}
```

`on_arrival` is lifted directly from seagame's `targetState` (`~/seagame/src/crew/movement.ts:58-121`),
and it is the load-bearing coupling in the whole behaviour system: **the decision of what to do is
taken when the walk *begins*, and remembered until the walk ends.** Nothing re-decides mid-journey,
which is both cheap and — surprisingly — the thing that makes NPCs look purposeful rather than
twitchy.

---

## 5. Following it

**Lane offsets, or you get a conga line.** Every NPC gets a stable lateral offset within the road's
`width_m`, seeded from their `ActorId` (the codebase's existing pure-hash-not-RNG idiom —
`attention.rs:683-699`). Keep to the right, jitter ±0.3 × half-width. The corridor is 4–8 m wide on
most streets; use it.

**Steering, clamped to the corridor.** Move toward the next point; clamp the resulting position to
`half_width_m` of the centreline and to the bitset. Cheap, and it makes it impossible to walk through
a wall no matter what the avoidance does.

**Local avoidance only on stage.** Reciprocal velocity obstacles over the ≤ 6 neighbours within 32 m.
Nobody at 200 m needs to avoid anybody, and at that distance nobody can tell.

**The Needle is 1.2 m wide.** It is a *one-person* alley and the lore knows it —
`04_routes_and_sightlines.md` calls it *"shortest and slow whenever someone approaches from the other
direction"*, and says it is what makes *"past the Needle"* belong to a path people actually need.
So: a one-lane corridor with a claim. If someone is in it going the other way, you wait, or you take
Cinder Row. **This is a feature that has been designed and is waiting for a mover.** It is also the
first place a naive steering implementation will deadlock, so build the claim before you build the
crowd.

---

## 6. What is *not* walkable, and why

- **Bridges.** The three `use == "bridge"` buildings are extruded solid prisms with their deck at
  y 4.25–9.0 (`city/mod.rs:707-710`). You cannot walk on them or under them. They are scenery.
  Exclude them; the Cut is crossed on the ground.
- **The wall-walk.** A paving slab at y 14.12 running the whole curtain (`city/mod.rs:1686-1694`) —
  standable, but **there is no stair up to it**. Exclude.
- **The Bellstand stair.** 14 real collider-backed steps, 3.15 m → 12.95 m (`city/mod.rs:1237-1284`).
  The only genuine vertical traversal in the city. Exclude from v1; the watch-bell tower's ringer can
  stand at the foot of it until someone wants to build multi-level nav for one staircase.
- **Building interiors.** There aren't any. Every building except the Lanthorn is a solid prism. This
  is a *large* fact and it is dealt with in §7.
- **The Serle.** Outside the wall entirely (`city/mod.rs:441-443`), and it is real water. The city has
  **no in-city canal** — the lore is emphatic (`04_routes_and_sightlines.md`: *"one Serle beyond the
  south wall and no in-city canal"*).
- **The Cut, however, is walkable.** It is a *filled* canal — dry since F.369, *"an unusually straight
  trade street"*, rendered with the `dry_cut` material and literally named *"The dry Cut"* at
  `city/mod.rs:562`. It is a 20 m-wide cartway, it is the widest walkable space in Ombreval, and it
  should be a major artery in the graph.

**So: navigation is single-level, on a flat plane, at y = 0.91.** Every one of the 500 NPCs already
spawns at exactly that height. Pathfinding is genuinely 2D. Do not build a 3D navmesh for a city that
does not have a second storey you can stand on.

---

## 7. Doors are destinations, not portals — and that is fine

You can walk *to* a door. You cannot walk *through* one. So "go home and sleep" ends with an NPC
standing on their doorstep.

The answer is Fallout's, and it is honest: **an `Indoors` state.**

```rust
/// The actor is inside a building. Rendered: not at all. Simulated: fully.
struct Indoors {
    building: BuildingId,
    /// Where they will re-emerge. Their own door.
    door: Vec3,
}
```

On arrival with `Arrival::Sleep`, the actor is marked `Indoors`, their body is hidden, and their
position is parked a metre inside the footprint. They are still in the world — still in the sim, still
schedulable, still audible if you shout — they are simply not rendered and not steered. On leaving,
they reappear at the door.

Three consequences worth stating out loud:

1. **It is a big perf win.** At the Snuffing, most of the cast goes indoors. The rendered crowd
   collapses to the watch, the tavern, and whoever has no bed. The city empties, which is *exactly*
   what curfew should feel like.
2. **It makes the missing sound occlusion glaring.** The sim has no line-of-sight —
   `Sight::line_of_sight` always returns `true` (AGENTS.md, "Known gaps"), and hearing is pure 3D
   distance. Today that means you can hear through a wall, and nobody notices because everyone is
   outdoors. The moment half the cast is behind a wall, you will hear people talking inside houses.
   Minimum fix: **an `Indoors` actor neither hears nor is heard by an outdoor actor** unless they are
   within a few metres of the door. That is a special case, not a raycast, and it will do.
3. **A hundred people have nowhere to go.** 100 characters carry the `pauper` status, 18 are
   `unhoused`, 14 have `insecure_lodging`. The lore gives them their own concern —
   *"fear of losing a sleeping place"* — and it is a gift: **an NPC with no door is an NPC who is
   still in the street at the Snuffing**, which is precisely the person the watch stops. Do not
   invent beds for them.

---

## 8. Tests — and the lore already wrote them

`lore/places/04_routes_and_sightlines.md` closes with a section called **"Route acceptance tests for
later builds"**. It was written for exactly this milestone. Adopt them verbatim as the bake's test
suite:

> - drive a notional cart from River Gate to Tallage and Maren's Green without stairs or cathedral
>   ground;
> - walk Wickmarket to Tallage through the Needle **and** by a handcart alternative;
> - walk the full dry Cut from Chain Bridge to Old Sluice;
> - …

Plus the mechanical ones:

| Test | Asserts |
|---|---|
| `every_named_place_is_reachable` | all 70 areas route from the cathedral forecourt |
| **`every_door_is_reachable`** | **≥ 2,560 of 2,566 — this fails today at 2,154 (§1). It is the test that catches the door bug, and it is the reason M1 exists.** |
| `the_door_you_see_is_the_door_you_walk_to` | the bake's door and `add_facade_openings`' door are the same point |
| `roads_are_walkable_end_to_end` | every road centreline, after roads-win |
| `the_reach_is_passable` | Draper's Reach is not bricked up by its own cloth halls |
| `malt_passage_is_passable` | the building *over* the passage does not block it |
| `graph_matches_bitset` | every graph edge's midpoint is walkable |
| `bake_is_reproducible` | fresh bake == committed artifact, byte for byte |
| `no_walkable_cell_is_solid` | (game crate) no baked cell is inside a `CollisionWorld` volume |
| `the_cut_is_walkable` | you can walk down the dry canal |

**Get all of these green before writing a single line of movement code.** M1 has no dependency on M0
and no dependency on the sim, and it is where the risk actually is.

---

## 9. Budget

At 500 NPCs and a 20 Hz movement tick:

| | |
|---|---|
| integrate + steer | 500 × ~200 ns = **100 µs/tick** = 2 ms/s ≈ 0.2% of one core |
| bitset lookup | one array index; free |
| A* over ~500 nodes | 5–20 µs, **capped at 8 queries per tick** through a request queue, so a mass repath cannot spike |
| neighbour grid | rebuilt per tick, re-bucketing only movers — O(moved), per `performance_improvements.md` item 4 |
| the hot channel | 500 × ~20 bytes = **10 KB memcpy per tick** |

Movement is not where the money goes. The money goes to tokens, and that is
[05_the_llm_seam.md](05_the_llm_seam.md).
