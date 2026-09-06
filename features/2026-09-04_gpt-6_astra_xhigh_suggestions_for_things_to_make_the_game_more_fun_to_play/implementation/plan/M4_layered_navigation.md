Status: Planned (2026-09-05).

# M4 — Places on more than one floor

Provide real connected interiors and upstairs movement as a city capability. The final quest's shortcut must use it without special movement code.

## Entry

M3 is accepted. New spatial state and stable identities must participate in the checkpoint from their first release.

[SPATIAL_PROOF_PROTOCOL](SPATIAL_PROOF_PROTOCOL.md) defines traversal-certificate coverage, temporal progression and the distinction between developer fairness checks and character knowledge.

## Verified starting point

`nav/mod.rs` schema 1 stores XZ nodes, uses `WALK_Y = 0.91`, and snaps destinations without floor identity. `World::step_movement` also flattens paths. Several round helpers assume the same plane. The street bake keeps the largest connected ground component.

`src/controller.rs` already sweeps a standing AABB against boxes and convex vertical prisms, but has no general step/slope policy. Most building footprints are solid colliders; façade doors and many stairs are decorative. The Lanthorn has actual interior geometry. The Cut separately adds a host-side ground lift to player/NPC presentation, which must not become a second source of floor height.

## Implementation slices

### M4a — Shared spatial source

Define stable spaces, walkable surfaces, entrances and transitions. Preserve the outdoor street graph as a cheap layer and add authored interior surfaces and stair/portal connections. Avoid a full-city voxel rewrite.

Use one source for structural geometry, player collision, NPC traversal and perception occluders. A pure shared module/crate is acceptable if it avoids Bevy dependencies in the sim. Keep rendering detail separate from authoritative walls, floors and openings.

Rooms belong to a subordinate spatial namespace. The current `AreaMap` rejects overlapping boxes, so nested rooms must not be inserted as overlapping ordinary city areas without an explicit schema migration. A citizen can be in a named ward/building and a particular room at the same time.

Give each entrance an authored stable ID. Current named-place snapping and hashed building-door selection can point to different locations; a floor-qualified doorway must remain the same doorway after a bake renumbers nodes.

M4 also owns the physical portal state and revision API: opening/obstruction geometry, affected routes and the shared collision/occlusion projection. M5 observes these states, initially driven by fixture operations. M6 adds the production lock/key/permission semantics and validated mechanical writers. All three use one authoritative physical opening; M6 must not replace it with a competing door state.

Ship the versioned spatial declarations, loader and fixture data here. M13 will compose and document module schemas, not introduce their authorability for the first time.

### M4b — Traversal and support

Add surface-qualified nearest-point projection, legal segment queries, route search and endpoint approach validation. A point directly above another room must not snap downstairs. Closed portals remain in the topology even when unavailable; baking only the currently open connected component would delete rooms needed later.

Implement usable stairs/ramps, headroom, landings and a bounded player step policy. Make NPC stepping preserve elevation and consume remaining distance across waypoints, rather than dropping the remainder at every corner. Define the movement metric used on slopes and record it in route measurements.

Make separation and crowd avoidance surface aware. People on different floors must not repel each other through a slab. Generalise narrow-passage ownership into a reusable crossing/reservation mechanism with bounded waits and rerouting.

Retain operation-wide progress/retry state across route replacement; two opposing escorts need deterministic yielding rather than endlessly renewed crossing timeouts. Repair movement-faithful development advancement: spend all accepted physical spans in ordered substeps and report dropped spans. The current coarse headless clock watcher cannot generate a faithful late-start world without this work.

Reconcile Cut lift and other cosmetic offsets: authoritative feet and eye/source positions must match the displayed body. Remove double lifts only when their replacement has passed the existing ground-route tests.

### M4c — Host and bake agreement

For an authored interior building, replace the filled footprint with actual shell, openings and supports. Preserve abstract buildings elsewhere; the feature does not require furnishing all 1,101 homes.

Extend navigation export, collision export, place/door resolution, map/debug overlays and bake validation together. Break the current feedback loop in which a baked hashed door choice controls the renderer: authored portals are inputs to both.

Migrate scene-derived references through stable IDs. Content manifests distinguish geometry versions. An incompatible old save is rejected before adoption unless an explicit migration provides safe replacement anchors without losing referenced objects or activities.

## Development slice

Create a small non-quest building with two rooms sharing XZ on different floors, one stair, two street entrances and a second upstairs connection. An ordinary NPC must travel between them while the player can walk the same route. Include a blocked portal and an alternate route. No Corin/Lise/Odo IDs or case evidence appear in this fixture.

## Measurement contract

An observed route duration is an upper bound on what that particular attempt achieved. A sparse graph's shortest path is not proof of the fastest path through the player's collision space.

Provide tools to capture actual traversed polylines, surface IDs, access revisions, door/action durations and interruptions. Public impossibility analysis must conservatively include the permitted controller's free walking, corner cuts, jumps and other usable connections. Since player speed caps horizontal movement, dividing a sloped 3D path length by 12 m/s is not automatically a valid lower bound.

Implement a bounded permissive route approximation for developer certification, separate from ordinary NPC routing. Every usable physical route must map into it and every edge cost must underestimate actual time. Include mandatory approach regions, boundary exit/re-entry, roofs/drops and directional transitions. Uncertain coverage adds a permissive shortcut or makes certification unavailable; it cannot silently delete an alternative. A valid but weak lower bound may honestly fail to prove impossibility.

## Tests

- Stacked rooms, wrong-floor destinations, stair ascent/descent, ceiling contact and unreachable endpoints.
- Player/NPC agreement at doors, walls, slab boundaries, stairs and narrow passages.
- Waypoint density does not arbitrarily slow an unobstructed route; elapsed motion remains speed bounded.
- Crowd avoidance distinguishes floors and cannot push an actor through a wall or off a landing.
- Ground-only navigation, home assignment, gates, road parties, wells and ordinary markets remain reachable.
- Bake reproducibility and stable entrance IDs survive node renumbering.
- Save/load on stairs, at a reserved crossing and during a route preserves coherent support and ownership.
- Hidden-window drive captures show real traversal, not developer flight or render-only height offsets.
- Regular/jittered polling and large requested advances spend equal accepted movement spans, preserving route/object/custody results within declared tolerance.
- A usable controller shortcut absent from the ordinary nav graph weakens or invalidates the certificate rather than being ignored.

## Completion gate

The independent building is traversable by player and NPC under the same structural rules. Shared queries can answer support, route and occlusion geometry. The street city still works. M5 receives the spatial-query contract; M6 receives stable physical portals; M14 receives survey tools rather than a claimed solved site.
