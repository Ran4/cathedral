# Performance: 1500 actors, and they walk

Status: superseded — see `findings.md` (2026-07-23), the measured overnight
session. Scorecard against this document: items 1, 2 and 8 had already
landed before that night; 9 and the spirit of 5/10 landed during it (as
change-detection gating and per-view culling rather than the exact shapes
proposed here); 4 (the shared spatial grid), 3 (flatten the hierarchy) and
5 (pool the UI nodes) remain the standing plan for 1500. The night's biggest
wins — the wet-material invalidation bug, the city-wide batch AABBs, the
interior shadow views — were things this document never suspected.

Status then: proposed. No code written yet.

## Goal

Two changes at once, at today's frame cost:

- the cast grows from 500 to ~1500;
- the cast stops being statues. NPCs walk the streets, and their behaviour gets richer than
  `position_m` + `facing_yaw`.

This document is written for that end state. An earlier version of it optimised for a stationary cast — that
advice is void, because most of it was cashing in on the fact that nothing ever changed. Half of it stops
paying the moment someone takes a step, and one item (`NotShadowCaster` on NPCs) is actively wrong once
figures move. What follows only contains work that survives movement.

## Where the cost is today

**Rendering is not the problem yet.** Bevy frustum-culls the NPCs automatically, and because all 500 share
three mesh handles and five materials they batch into a handful of instanced draws. Nobody behind you costs
anything.

**The per-frame CPU work is the problem**, and it is O(N) several times over — today for a cast that never
moves, which is what makes it absurd rather than merely expensive. Walking forward bumps `world_revision`,
and that fires this chain across the entire cast, every frame:

1. `public_snapshot()` (`crates/cathedral-sim/src/world.rs:257`) clones all 501 actors, allocating a fresh
   `String` per actor for `name_for_player`.
2. `WorldSnapshot::from(&PublicSnapshot)` (`src/smart_actors/model.rs:217`) clones the whole thing again.
3. `ValidatedSnapshot::new` (`model.rs:476`) builds a 501-entry `HashMap` of cloned `ActorId`s and validates
   every entry.
4. `reconcile_actor_views` (`src/smart_actors/actors.rs:145`) rebuilds a `Vec` + `HashSet` + `HashMap` over
   all actors, then writes `Transform` on all 500 roots unconditionally (`actors.rs:173-187`) — marking every
   one dirty and forcing full transform propagation and AABB/visibility recompute for the whole cast.
5. `position_actor_name_labels` (`actors.rs:348`) and `update_thinking_indicators` (`actors.rs:389`) each
   rebuild a separate, identical `HashMap` of all 500 anchor positions, from scratch, every frame.
6. `update_actor_focus` (`src/smart_actors/targeting.rs:99`) collects and sorts all 500 actor records every
   frame to answer "who am I looking at".

**Entity count.** Each NPC is ~9 entities: a root, three primitive meshes (capsule body, sphere head, cone
nose), three empty anchor transforms, and two UI text nodes (name label `actors.rs:283`, thinking indicator
`actors.rs:307`). About 4,500 today; ~13,500 at 1500.

**Culling.** Frustum: yes, free. Occlusion: none — the camera at `src/controller.rs:336` is a plain `Camera3d`
with no depth prepass, so Bevy 0.19's two-phase GPU occlusion culling is off. Distance/LOD: none — a capsule at
800 m costs the same as one at 3 m. Shadows: the directional light's cascade `maximum_distance` is 520 m
(`src/scene.rs:1105-1109`), and shadow casters are culled against the *light's* frusta, not the camera's, so
every NPC inside that volume is drawn into the shadow map regardless of walls or facing.

**Movement is currently forbidden in the sim.** `spatial_update` rejects any update that moves someone other
than the player (`crates/cathedral-sim/src/engine.rs:948`). Lifting that guard is what turns the O(N) chain
above from a walking-only annoyance into the steady-state cost of the world existing — which is why item 1
below is a prerequisite for movement, not a follow-up to it.

## The plan

Ordered by leverage. Items 1–4 are load-bearing; the rest are cheap wins that stack on top.

### 1. Split the projection into a hot channel and a cold channel

The single biggest cost, and the one that gets worse rather than better with movement. Today any position
change invalidates the whole public projection, which is then cloned three times with per-actor heap
allocation.

Split it:

- **Hot** — `(ActorId, position_m, facing_yaw)`, and whatever else changes at movement rate. A flat, dense
  array. Copying it is a `memcpy`; it allocates nothing and validates nothing. Rebuilt whenever anyone moves,
  which will be constantly.
- **Cold** — `name_for_player`, the knows-set, `appearance`, inventory, everything else. Rebuilt only when
  one of those actually changes, which is rare and event-driven.

`ValidatedSnapshot` only needs to re-validate the cold channel; the hot channel is structurally valid by
construction (it is indices into a cast whose membership did not change).

Do not build the intermediate "special-case the player so a player step doesn't rebuild the world" version.
It is a smaller change, but it is the wrong shape and you would rip it out the week NPCs start walking.

### 2. Move on a fixed tick; interpolate in the render

Do not let movement cost scale with framerate. Advance NPC positions in the sim on a fixed tick — 10–20 Hz is
plenty for a walking crowd — and have Bevy interpolate between the last two ticks for the render transform.

This decouples the two budgets. Steering, collision and pathing get a stable per-second cost that does not
triple when someone runs the game at 165 fps, and the render side gets smooth motion regardless of tick rate.
It also means the hot channel above is produced at tick rate, not frame rate.

### 3. Flatten the actor hierarchy

Statues never propagate their transforms, so nine entities each was free. Walkers propagate all of them, every
tick: at 1500 NPCs that is ~9,000 `GlobalTransform` updates plus the AABB and visibility recompute that
follows.

- **Delete the three anchor children** (name, speech, offer — `actors.rs:263-278`). They exist purely to hold a
  Y offset. Compute `root.translation + Vec3::Y * NAME_ANCHOR_Y` at the point of use and 4,500 transform nodes
  disappear at zero cost to behaviour.
- **Merge body, head and nose into one mesh** at asset-build time. Three shared primitives were a convenient
  way to author a figure; they are three children to propagate and three draws to batch. One combined mesh per
  outfit keeps the instancing property (still a handful of shared handles) and cuts the child count to one.

Root + one mesh child = 2 entities per NPC. 3,000 at 1500, against 13,500 today.

### 4. One shared spatial grid, rebuilt per movement tick

Right now three systems each answer "who is near this point" by brute force — two `HashMap` rebuilds over the
whole cast (`actors.rs:348`, `actors.rs:389`) and a collect-and-sort (`targeting.rs:99`) — every frame. With
movement, `characters_within`, the attention gating and the who-can-hear-whom checks lose their stable answers
too, so demand for those queries goes up at the same time.

Build a uniform grid once per movement tick, keyed by cell, holding `(ActorId, Vec3)`. Re-bucket only the
actors that crossed a cell boundary — O(moved), not O(N). Every consumer reads it:

- nearest-N name labels;
- the thinking indicator's in-range check;
- `update_actor_focus` (a cone query against the local cells, not a sort of the whole cast);
- the sim's proximity/audibility queries.

### 5. Pool the UI nodes

You display at most `MAX_VISIBLE_NAME_LABELS` (20) names and exactly one thinking indicator, but every NPC owns
a permanent hidden `Text` node for each — 3,000 hidden nodes in taffy's layout tree at 1500, which taffy still
walks.

Keep a pool of 20 label entities and 1 indicator, and reassign them each frame to the nearest actors from the
grid in item 4. UI cost becomes constant in cast size. Movement makes this *more* valuable, not less: the
nearest-20 set churns constantly once people walk, and a pool is exactly the structure that wants to be
reassigned.

### 6. LOD the simulation, not just the render

Distance-cull the *behaviour*, which is where the money goes once steering exists:

- **Near** (say < 60 m): full steering, collision, avoidance, per-tick pathing.
- **Mid**: follow the path, no avoidance, coarser tick.
- **Far**: advance along the path analytically at a slow interval, or don't simulate until the player
  approaches.

Nobody can distinguish a steered walker from a waypoint-slider at 200 m through the fog. This tiering should
reuse the same neighbourhood notion the LLM attention gating already has
(`crates/cathedral-sim/src/attention.rs`), so there is one answer to "is this actor near the player" and not
three.

### 7. REMOVED

(this section used to talk about not having any gait etc; fuck that, we want realism!)

### 8. Distance culling and shadows

- **`VisibilityRange` on the actor roots**, fading out around 120–150 m. Ten lines, and the fog already hides
  that range. Unaffected by movement — the fade band handles actors walking across the boundary.
- **Shorten the shadow cascade `maximum_distance`** (`src/scene.rs:1107`) so only nearby actors are shadow
  casters. Do **not** reach for `NotShadowCaster` on NPCs: a moving shadow is most of what makes a figure read
  as alive, and killing it is a real aesthetic loss now that they move. Shortening the cascade keeps the
  shadows where they are legible and drops the 400 m crowd from the shadow pass.

### 9. Guard the Transform write

Compare before assigning in `reconcile_actor_views` (`actors.rs:176-181`) — today it writes translation,
rotation and scale unconditionally, which marks the component changed whether or not the value differs.

Demoted from its former place at the top of the list: once NPCs walk, a walking NPC's transform genuinely does
differ every frame and the propagation is real work you have to pay. The guard now only buys you whoever is
currently standing still — but a float compare is cheaper than the write it avoids, and in a crowd a decent
fraction is always idle, so it stays worth the three lines.

### 10. Optional: `DepthPrepass` + `OcclusionCulling`

A dense city of pinched streets, with most of the world behind a wall, is close to the ideal case for GPU
two-phase occlusion culling — and it gets better with movement, since the cast is no longer a fixed set you
could have hand-tuned around. But it costs a depth prepass every frame whether or not it saves anything.
Measure before adopting.

## Explicitly rejected

- **`NotShadowCaster` on NPCs.** Correct advice for statues, wrong once they move. See item 8.
- **Skinned character animation as the default rig.** See item 7.
- **Special-casing the player's position in the snapshot.** The right fix is the hot/cold split (item 1); the
  special case is a smaller change of the wrong shape.

## Caveat: the scheduler, which none of this fixes

The scheduler runs a single LLM turn at a time, round-robin over the cast
(`crates/cathedral-sim/src/scheduler.rs:682`), so tripling the cast means each NPC's autonomous turn comes
around 3× less often. `background_turn_order` already weights by significance, but at 1500 the ambient tier
will effectively go quiet. That is a design problem, not a rendering one: the work above buys the frames, it
does not buy a city that still feels alive at 3× the cast. See
`features/gate_idle_cognition_on_novelty.md`, which is attacking the same budget from the other end.

## Suggested order of work

1. Hot/cold projection split (item 1) — must land before the `spatial_update` guard at `engine.rs:948` is
   lifted.
2. Fixed movement tick + render interpolation (item 2).
3. Flatten the hierarchy (item 3) and add the shared grid (item 4) — these two together are what make the
   per-tick cost of a moving cast linear-with-a-small-constant instead of linear-with-several.
4. UI pooling (5), `VisibilityRange` and the shadow cascade (8), the Transform guard (9) — cheap, independent,
   land whenever.
5. Simulation LOD (6) and the gait (7) once movement actually exists and there is something to profile.
