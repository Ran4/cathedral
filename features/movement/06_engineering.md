# Engineering: how it rides the existing machine

`features/performance_improvements.md` — *"Performance: 1500 actors, and they walk"* — already did most
of this thinking, and it did it well. This file does not restate it. It says which parts of it are
**hard prerequisites** for movement, what movement adds, and what movement gets wrong if you skip them.

---

## 1. The two prerequisites

The perf doc's own ordering is right and its reasoning is right:

> **Movement is currently forbidden in the sim.** [...] Lifting that guard is what turns the O(N)
> chain above from a walking-only annoyance into the steady-state cost of the world existing — **which
> is why item 1 below is a prerequisite for movement, not a follow-up to it.**

### Item 1 — the hot/cold split

Today, *any* position change bumps `world_revision` (`world.rs:251-253`), and that fires this chain
across all 501 actors, **every frame**:

1. `public_snapshot()` clones all 501 actors, allocating a fresh `String` per actor for
   `name_for_player` (`world.rs:257`);
2. `WorldSnapshot::from(&PublicSnapshot)` clones it again (`model.rs:217`);
3. `ValidatedSnapshot::new` builds a 501-entry `HashMap` of cloned `ActorId`s and validates every
   entry (`model.rs:476`);
4. `reconcile_actor_views` writes `Transform` on all 500 roots unconditionally (`actors.rs:173-187`),
   marking every one dirty and forcing full transform propagation for the whole cast.

The player walking already pays this. **Five hundred NPCs walking would pay it continuously, forever.**

- **Hot**: `(ActorId, position_m, facing_yaw)` — plus, for us, `gait_phase` and `speed` (§5). A flat
  dense array. A `memcpy`. Allocates nothing, validates nothing. Produced **at tick rate, not frame
  rate.**
- **Cold**: names, `knows`, `appearance_key`, inventory. Rebuilt only when one of those changes, which
  is rare and event-driven.

### Item 2 — fixed tick, interpolate in the render

> *"Advance NPC positions in the sim on a fixed tick — 10–20 Hz is plenty for a walking crowd — and
> have Bevy interpolate between the last two ticks."*

**And the pattern is already in the repo, for the player.** `src/controller.rs` runs
`fixed_player_movement` in `FixedUpdate` at `FIXED_HZ = 120.0`, keeps a `PhysicalPosition { previous,
current }` (`controller.rs:301-305`), and lerps by `overstep_fraction()` in `interpolate_player`
(`controller.rs:533-541`). Do exactly that for NPCs, at 20 Hz.

**Take 20 Hz, not 10.** At 1.2 m/s, a 20 Hz tick moves someone 6 cm — invisible after interpolation.
10 Hz moves them 12 cm, which is also fine for walking, but 20 Hz gives local avoidance enough
resolution not to look mushy, and the cost difference is nothing (§6).

---

## 2. The guard

`crates/cathedral-sim/src/engine.rs:967-979` rejects any `SpatialUpdate` naming a non-player actor:

```rust
// Only the player moves. NPC positions are static world state; a client
// that tries to move one is confused, and letting it would silently
// rewrite the cast's geometry (`server.py:924-929`).
```

**Do not lift this guard. Keep it, and honour it.** The reasoning in that comment stays true: the
*host* must not be able to move an NPC. What changes is that the *sim* now moves them itself, on its own
tick, through a path that has nothing to do with `spatial_seq` (which exists to sequence the player's
client-authoritative position stream, and coupling NPC motion to it would be a category error).

So the new mutation path is a sibling, not a replacement:

```rust
impl World {
    /// Advance every mover by one fixed movement tick. The sim's own hand on
    /// the cast — not `update_positions`, which sequences the *player's*
    /// client-authoritative stream and must stay exactly as strict as it is.
    pub fn step_movement(&mut self, dt: f64, nav: &NavGraph) -> MovementDelta { /* … */ }
}
```

`MovementDelta` is the hot channel: the ids that actually moved, and where to.

**And mind the facing rule.** `update_positions` deliberately does *not* bump `world_revision` for a
facing-only change (`world.rs:251-253`), because a snapshot always reads current facing. That is fine
today. Once the hot channel exists, facing must ride it — otherwise an NPC who *turns to look at you*
and does not move their feet will never reach the renderer, and the sound-witness cone will be
unlearnable. `actors.rs:177-180` already flags this:

> *"The render is the only place the player can read the sound witness rule from: if the sim thinks an
> NPC faces away and the body faces the player, the rule is unlearnable."*

---

## 3. Determinism

`cathedral-sim` is pure, offline and deterministic, and its whole test suite depends on that. Movement
must not break it.

**Fixed tick with an accumulator, never `dt` straight from the frame.** A variable `dt` makes the sim's
float trajectory depend on the framerate, and a test that steps 0.1 s at a time would land somewhere
different from a game at 1/144 s. Accumulate `now - last_tick` and step fixed 50 ms slices. Then a
test, the headless runner and the game all produce **the same city**.

**No RNG.** Follow `attention.rs:683-699`, which rolls curiosity as a pure hash of
`(salt, actor_id, context, visit)` and explains exactly why: *"the engine polls at 60 Hz, and a
re-drawn 20% is a certainty within a frame."* Every "random" choice in the ladder — which well, which
wander target, whether to take the probability gate — is `hash(actor_id, decision_epoch, salt)`.

**No clock.** `now: f64` keeps coming in through `Engine::poll`. The `WorldClock` is a *projection* of
it ([01](01_the_clock.md) §2), not a new source of time.

The payoff is a test you can actually write:

```rust
#[test]
fn sibbe_reaches_the_oven_before_the_kindling() {
    let mut engine = fixture_city();               // offline, fake cognition
    engine.run_until(Office::Kindling);
    assert_eq!(engine.area_of("p001p"), Some("bell_and_sluice_bakehouse"));
}
```

That is the loop this feature gets iterated in, and it is why the sim is the mover.

---

## 4. LOD — one notion of "near", not three

`performance_improvements.md` item 6 is emphatic, and correct:

> This tiering should reuse the same neighbourhood notion the LLM attention gating already has
> (`crates/cathedral-sim/src/attention.rs`), so **there is one answer to "is this actor near the
> player" and not three.**

| tier | radius | ladder | steering | tick |
|---|---|---|---|---|
| **stage** | < 32 m — `DEFAULT_STAGE_RADIUS_M`, the existing constant | 1–3 s | full: avoidance, corridor clamp, gait | 20 Hz |
| **near** | < 150 m — where `VisibilityRange` fades them out anyway | 3–8 s | follow the route; no avoidance | 20 Hz |
| **far** | ≥ 150 m | 10–30 s | advance along the route | **2 Hz** |

Far-tier actors are also the only ones eligible for the **Long Errand** fiat ([01](01_the_clock.md)
§6): an actor beyond 150 m on a route longer than 250 m may be advanced by the clock rather than by
their feet. **The 150 m guard is what makes it invisible** — you cannot watch someone teleport,
because at 150 m they are already fading out.

---

## 5. The gait — protect the instancing

`performance_improvements.md` item 7, and it is right:

> Resist skinned meshes: skinning is what breaks the property that makes 1500 NPCs affordable in the
> first place (three shared mesh handles, five materials, a handful of instanced draws). [...] Fake it
> instead — **bob the root, swing the merged primitives sinusoidally off a phase seeded per actor.**

An NPC today is a capsule, a sphere and a cone (`actors.rs:112-117`). glTF and `AnimationPlayer` are
available with no `Cargo.toml` change — Bevy's `3d` feature pulls `bevy_gltf` and `gltf_animation`
transitively — and it would be a trap.

So the hot channel carries **two extra floats per actor**:

- `speed` — how fast they are actually going, which drives the gait's frequency and amplitude;
- `gait_phase` — accumulated in the sim (`phase += speed * dt * k`), so that it is *continuous* across
  ticks and does not reset when someone stops and starts.

Bevy reads them and applies a sinusoidal bob + lean + arm-swing to the single merged mesh. Zero
skeletons, zero extra draws, and the instancing property survives.

**Two floats × 500 actors = 4 KB per tick.** The hot channel is 500 × ~24 bytes ≈ **12 KB per tick, a
memcpy**.

---

## 6. Budget, at 500 NPCs and 20 Hz

| | |
|---|---|
| integrate + steer | 500 × ~200 ns = **100 µs/tick** → 2 ms/s ≈ **0.2% of one core** |
| ladder | ~140 evaluations/s across the whole cast (mean 3.5 s cadence) — rounds to zero |
| A* | 5–20 µs per query, **capped at 8 per tick** through a request queue, so a mass repath cannot spike a frame |
| neighbour grid | rebuilt per tick, re-bucketing only movers — O(moved), per perf doc item 4 |
| hot channel | **12 KB memcpy per tick** |
| `characters_within` | today an **O(n) brute-force scan** (`world.rs:132-140`) with no spatial index; it is called per prompt render and once per poll. The grid from perf-doc item 4 is what stops it becoming O(n²) once everything moves. |

**Movement is not where the money goes.** Frames are cheap. Tokens are not, and that is
[05_the_llm_seam.md](05_the_llm_seam.md) §5.

---

## 7. Small things that will bite

- **`MAX_ACTORS = 1_024`** (`src/smart_actors/model.rs:24`). Fine at 500, trips at the perf doc's 1,500.
- **Twenty golden prompt fixtures** regenerate on any sheet change (`tests/golden_prompts.rs`). Change
  the sheet **once** ([05](05_the_llm_seam.md) §3), not four times across four milestones.
- **`CollisionWorld` has no broadphase** — `move_aabb` is brute force over 3,000+ colliders
  (`controller.rs:675-720`). Do not give NPCs the player's character controller. They do not need it:
  the corridor clamp plus the walkable bitset makes it geometrically impossible for them to enter a
  wall, and that is cheaper *and* more reliable than sweeping.
- **NPCs have no colliders and should not get any** (`targeting.rs:1-5` says so deliberately). Actor-vs-actor
  separation is the steering layer's job, on stage only.
- **`update_actor_focus` collects and sorts all 500 actors every frame** (`targeting.rs:99`) to answer
  "who am I looking at". It is wrong today and it is *loud* once they move. It is perf-doc item 4's
  first customer.
- **The three anchor children per NPC** (name, speech, offer) exist only to hold a Y offset
  (`actors.rs:263-278`). Statues never propagate them; walkers propagate all of them, every tick.
  Perf-doc item 3 deletes them, and movement is what makes it urgent.
