Discussion about going from 500 to 1500 characters with no performance loss:

Claude said:

Rendering-wise you're in decent shape: Bevy already frustum-culls the NPCs for you, and because all 500 share three mesh handles and five materials they batch into a handful of instanced draws. The thing that will not survive tripling is the per-frame CPU work, which is O(N) several times over and runs every
frame even though your NPCs never move. That, plus shadow-map casters and 2 UI nodes per NPC, is what stands between you and 1500.

How NPCs are rendered

reconcile_actor_views (src/smart_actors/actors.rs:145) projects the WorldMirror snapshot into ECS entities. Each NPC gets a root plus three mesh children — a Capsule3d body, a UV sphere head, a cone "nose" — plus three empty anchor transforms, plus two separate UI entities (a name label Text node and a
thinking-indicator Text node, actors.rs:283 and actors.rs:307). That's ~9 entities per NPC: about 4,500 today, ~13,500 at 1500.

They're spawned once and never despawned. NPC positions are immutable — spatial_update explicitly rejects moving anyone but the player (crates/cathedral-sim/src/engine.rs:826) — and they're spread across the full city (x −508→492, z −668→482), so only ~12 sit within 100 m of the cathedral origin.

What culling you actually have

Frustum culling: yes, automatic. The meshes get an Aabb, Bevy's check_visibility runs, and anything outside the camera frustum is skipped in the main pass. Someone behind you is free.

Occlusion culling: no. An NPC standing behind a wall but inside the frustum is still transformed and rasterized — only the depth test kills the pixels. Bevy 0.19 does ship GPU two-phase occlusion culling (OcclusionCulling on the view + a DepthPrepass), but your camera at src/controller.rs:336 is a plain
Camera3d with no prepass, so it's off.

LOD / distance culling: none. No VisibilityRange, no impostors. A capsule at 800 m costs the same as one at 3 m.

Shadows are where "you can't see them but they still render" really bites. Your directional light has maximum_distance: 520.0 with default cascades (src/scene.rs:1106). Shadow casters are culled against the light's frusta, not the camera's — so every NPC within that 520 m volume is drawn into the shadow map
regardless of walls, buildings, or facing. At 1500 that's ~4,500 capsule/sphere/cone draws per cascade for shadows a player can't resolve anyway.

The real bottleneck: O(N) CPU every frame

Every frame you walk, the player's position bumps world_revision (crates/cathedral-sim/src/world.rs:251), and that triggers this chain over the entire cast:

1. public_snapshot() clones all 501 actors with fresh String allocations each (world.rs:257).
2. WorldSnapshot::from(&PublicSnapshot) clones the whole thing again (src/smart_actors/model.rs:217).
3. ValidatedSnapshot::new builds a 501-entry HashMap of cloned ActorIds and validates every one (model.rs:476).
4. reconcile_actor_views builds a Vec + HashSet + HashMap over all actors, then writes Transform on all 500 roots unconditionally (actors.rs:176) — which marks every one dirty and forces full transform propagation and AABB/visibility recompute for the whole cast.
5. position_actor_name_labels and update_thinking_indicators each rebuild a fresh HashMap of all 500 anchor positions, separately, every frame (actors.rs:348, actors.rs:389).
6. update_actor_focus collects and sorts all 500 actor records every frame (src/smart_actors/targeting.rs:99).

That's five-plus full passes with per-actor heap allocation per frame, for characters that are statues. It scales linearly with your NPC count and it's nearly all waste.

What I'd change to get 1500 at today's cost

Roughly in order of leverage per line of code:

- Guard the Transform write in reconcile_actor_views — if transform.translation != new. Change detection is the whole point, and you're currently defeating it for the entire cast every frame.
- Don't rebuild the snapshot when only the player moved. The game is the source of the player's position; making it round-trip through a full 501-actor clone-and-validate is the single biggest avoidable cost while walking.
- Pool the UI nodes. You only ever show ≤20 name labels (MAX_VISIBLE_NAME_LABELS) and exactly one thinking indicator, yet you keep one Text node per NPC in the taffy layout tree — 3,000 hidden nodes at 1500. Pool 20 labels and 1 indicator and reassign them to the nearest actors.
- Add VisibilityRange to the actor roots (fade out around 120–150 m). Cheap, ~10 lines, and with your fog nobody will notice.
- Cut the shadow cost — NotShadowCaster on NPCs, or a much shorter cascade maximum_distance.
- Cache the anchor-position map instead of rebuilding it 2–3× per frame in different systems, and give update_actor_focus / characters_within a spatial index instead of linear-scan-and-sort.
- Optionally enable DepthPrepass + OcclusionCulling on the player camera. A dense city of pinched streets is close to the ideal case for it, but measure — it costs you a prepass.

One non-rendering caveat worth flagging: the scheduler runs a single turn at a time round-robin over all LLM actors (crates/cathedral-sim/src/scheduler.rs:682), so tripling the cast means each NPC's autonomous turn comes around 3× less often. background_turn_order already weights by significance, so it may not
matter much, but at 1500 the ambient tier will effectively go quiet.
