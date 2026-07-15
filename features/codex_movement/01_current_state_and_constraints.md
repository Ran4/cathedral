# 1. Current state and constraints

This chapter records what the repository already does and what that implies. It is not a general wishlist. The proposed architecture should fit these boundaries instead of quietly creating a second simulation alongside them.

## 1.1 The authoritative simulation already has the right home

`crates/cathedral-sim` owns the world, turn scheduling, prompt construction, action parsing, attention, and semantic state. It is deliberately pure and IO-free. Bevy pumps it through `src/smart_actors/local_engine.rs`, receives typed `EngineMessage`s, and projects snapshots into ECS through `model::WorldMirror`.

Movement should extend that contract. It should not be implemented as a Bevy-only wandering system that occasionally copies positions back to the sim. A Bevy-only design would create several immediate contradictions:

- the prompt and hearing systems could use an old position while the rendered NPC has walked away;
- offers and item handoffs could succeed or fail at the wrong distance;
- stage-gated cognition could select the wrong set of actors;
- headless simulation would still contain stationary people;
- save/replay and deterministic testing would depend on frame timing;
- a renderer hiccup could change behavior outcomes.

The invariant should therefore be:

> `cathedral-sim` owns spatial truth. Bevy may interpolate, animate, cull, and decorate it, but must never originate ordinary NPC movement truth.

## 1.2 Existing position flow assumes NPCs never move

The current code contains most of a spatial update path, but it is intentionally one-sided:

- player position and facing are sampled in Bevy and sent through `EngineCommand::SpatialUpdate`;
- `World::update_positions` is generic enough to mutate actors;
- `Engine::spatial_update` rejects non-player updates because NPCs are static;
- snapshots include actor position and facing;
- `reconcile_actor_views` sets every NPC transform directly from the latest snapshot and explicitly treats views as stationary.

This is a useful seam, not a design to reverse. The sim should start producing NPC spatial changes itself. The host should continue sending only external spatial actors—currently the player, and later perhaps networked or physics-owned objects. The projection then needs interpolation instead of exact transform replacement on every semantic snapshot.

## 1.3 One revision currently carries too many meanings

World snapshots are emitted when the world revision changes. That is sensible while position rarely changes. Once hundreds of NPCs move at 5–10 simulation updates per second, touching the same semantic revision for every spatial step would cause excessive full-world snapshot churn.

Movement needs two forms of change:

- **semantic change**: inventory, memory, goal, offer, relationship-visible state, actor presence, activity category, or other state consumers need to reconcile;
- **spatial change**: position, facing, velocity, route sample time, and perhaps gait.

The public protocol should separate them. A full semantic snapshot can remain revision-based and relatively infrequent. A compact spatial batch can carry only changed actors at a fixed or adaptive cadence. The mirror should associate both with a monotonically increasing simulation tick so late or out-of-order data can be rejected.

## 1.4 `Engine::poll(now, commands)` already uses time—but not game time

The existing `now` participates in provider request lifetimes, scheduler timing, floor control, and speech behavior. It is effectively a host monotonic clock. Reinterpreting it as “time of day” or multiplying it by a game time scale would break unrelated systems.

Add a separate `WorldTime` and a separate simulation delta. Keep these concepts distinct:

| Time | Purpose | May pause? | May accelerate? |
|---|---|---:|---:|
| Host monotonic time | backend timeouts, request aging, audio/floor coordination | normally no | no |
| Physical simulation elapsed time | locomotion, steering, short interaction leases | yes | normally 1× |
| World/calendar time | hour, day, schedules, lighting, sleep/work windows | yes | yes |

The distinction is especially important in a large city. If a 30× calendar scale also multiplies a walker's 1.3 m/s speed, people sprint unnaturally. If it does not, a long commute consumes many game hours. The solution is not to hide the mismatch; schedules, locality, offstage travel, and time-scale tuning must account for it explicitly.

## 1.5 The action language has no locomotion verb

Current actions cover speech, exchange, consumption, memory, goals, and waiting. Prompt text explicitly explains the absence of walking. This is correct for the current world.

The routine layer should not express every step as an action. A schedule can create a `TravelTo` activity directly in code. Later, LLM output can request a high-level, validated intent such as “go to Wickmarket” or “meet at this church,” but it must not provide coordinates, waypoints, velocity, animation commands, or an unbounded script.

The current prose `goal` should also remain distinct from executable intent. Parsing arbitrary natural-language goals into movement would be brittle and unsafe. A character may retain the prose goal “find out why the bells stopped,” while a structured planner chooses `Observe(BellTower)` or `Ask(PersonId)` only when those capabilities exist.

## 1.6 Stage-gated cognition is already waiting for non-LLM behavior

Idle cognition is now limited to actors the player can see, hear, or talk to. That means most of the city does not spend LLM turns and—today—also does nothing. The existing design notes identify autonomous non-LLM movement as the missing behavioral layer.

This has a helpful consequence: movement can improve the entire cast without weakening the attention gate. A code-driven actor may travel, work, sleep, or socialize anywhere. LLM cognition remains event- and attention-driven. When that actor enters the player's neighborhood, its current position, activity, and recent material events can be included in a prompt.

## 1.7 The cast is large and heterogeneous

The authored population has roughly 500 people: major, minor, and ambient characters with occupations, districts, circumstances, conditions, family relationships, descriptions, and frequently a prose goal. The data contains useful behavioral signals—watchmen, bell ringers, lamplighters, servants, porters, clergy, market trades, poverty, injury, pregnancy, age—but no stable home, workplace, schedule, mobility profile, or activity anchors.

Do not turn free-form occupation and condition strings into a permanent runtime rules engine. They are excellent inputs to an offline deterministic assignment pass. Runtime code should consume normalized fields:

- schedule template ID;
- home/interior/portal ID;
- work and fallback spot IDs or queries;
- role tags;
- mobility parameters;
- structured routine preferences;
- authored exceptions.

Major characters should support hand-authored overrides. Ambient characters can use generated assignments. The same runtime evaluator should handle both.

## 1.8 The city has rich cadastral data, but not a navigation graph

The city plan includes thousands of buildings, roads, sites, fixtures, and named places. `build_city` also registers collision boxes and convex prisms in `CollisionWorld`, including geometry that is not simply represented by road centerlines. This is valuable input, but it is not yet a canonical exportable walkable surface.

A diagnostic comparison of current actor spawns to the 49 road polylines found:

- median distance from a spawn to the nearest road centerline: about 20.4 m;
- 90th percentile: about 48.3 m;
- maximum: about 122.5 m;
- only 9 of 98 road endpoints connect to another endpoint within 10 m.

Those values make a road-centerline navigation graph unsuitable as the only traversal network. It would strand many spawns, omit plazas and courtyards, and require arbitrary long connectors. It can still provide semantic street names, route bias, or debug comparison.

The navigation input must instead represent free walkable ground minus obstacles, with explicit links for stairs, passages, gates, bridges, portals, and any other topology that cannot be inferred from a flat plane.

## 1.9 Geometry ownership needs one refactor before a reliable bake

The runtime collision world knows about obstacles, but it is private to the controller and constructed alongside rendering. The cadastral JSON does not necessarily contain every bespoke collider. Baking from only one representation risks an actor route crossing a cathedral wall that exists only in another.

Introduce a project-owned geometry description that can feed all three consumers:

```text
canonical cadastral/bespoke definitions
                 |
                 v
     WorldGeometry / BakeGeometry
         /          |          \
        v           v           v
   rendering    collision    navigation bake
```

This need not rewrite all procedural building generation. It does need an explicit export of ground bounds, obstacle footprints, walkable exclusions, portal edges, elevation regions, and stable source IDs. The navigation artifact should embed a hash of those inputs. Startup validation can then detect a stale bake instead of allowing invisible disagreement.

## 1.10 Narrow streets constrain agent assumptions

Authored roads can be very narrow. A nominal 1.2 m path leaves little clearance for two adult agents with approximately 0.4 m bodies. The bake and crowd layer must agree on radius, clearance, and which passages are one-way or effectively single-file.

The first profile should be one conservative pedestrian mesh, rather than separate meshes for every body type. Profile-specific speed and avoidance radius can vary at runtime. If later actors with carts, stretchers, or bulky loads need different connectivity, add a second navigation profile based on measured need.

## 1.11 The visual actors are simple primitives

NPC views currently use simple body/head geometry rather than full character rigs. Locomotion still needs readable feedback:

- smooth facing changes;
- velocity-based lean or body pitch;
- small procedural vertical and lateral gait;
- a stopped/turning/walking visual state;
- near-player footstep sounds with aggressive distance and concurrency limits.

This can make the first movement slice legible without blocking on skeletal animation. The simulation should expose desired/actual velocity and activity, not animation phase. Bevy can derive animation phase locally so visual detail never becomes authoritative.

## 1.12 Daylight is currently static

The scene uses a fixed directional light, bright global ambient light, static atmosphere/fog settings, and always-present local lights. There is no world clock or celestial update. Bevy's atmosphere can respond to directional light changes, so the scene has a viable extension point.

Day/night therefore spans two domains:

- the sim owns `WorldTime`, phase transitions, schedule events, and semantic lamp state;
- Bevy maps time continuously to sun direction, illuminance/color, sky/atmosphere, ambient/fog exposure, emissive appearance, and the subset of actual point/spot lights worth rendering.

The lamplighter should be able to perform a visible route, but critical illumination cannot depend on perfect AI completion. Lamps need a deterministic grace-period fallback so a path failure does not leave the playable city black.

## 1.13 No established savegame can absorb accidental assumptions

The project persists settings but does not yet expose a general world save system. Movement state should still be designed to serialize cleanly. If transient route handles, ECS entities, or wall-clock instants leak into authoritative state, later persistence will be painful.

Use stable IDs and explicit serializable values. Routes may be safely discarded and replanned on load; actor presence, position, activity intent, schedule, reservations, decision counters, world time, and semantic outcomes should have clear persistence rules now.

## 1.14 Lessons taken from Sea Game

Sea Game separates an actor's behavior state and targets from the continuous walking that executes them. Its idle behavior uses a priority ladder: urgent hazards and conditions first, needs and role behaviors next, then social or exploratory fallback. It also ties useful roles such as lamp handling to darkness.

Those are strong patterns to retain:

- decisions happen at a lower cadence than rendering;
- intention is separate from locomotion;
- urgent conditions preempt routine;
- role and world phase create recognizable behavior;
- random variation chooses among valid options rather than replacing logic.

Patterns not to transplant literally into this project include:

- rolling low-probability random choices every frame;
- scanning every possible target on each idle decision;
- choosing a fresh random time-of-day offset on every boot;
- keeping all behavior in one actor-type switch;
- introducing a deep needs/economy system before activities have world support.

With 500 actors, stable seeded choices, indexed queries, staggered wake-ups, and data-driven package templates are essential.

## 1.15 Hard architectural constraints

The rest of the plan treats the following as non-negotiable unless the core project architecture changes deliberately:

1. The simulation remains pure and headless-capable.
2. Provider, speech, filesystem, and wall-clock concerns remain outside `cathedral-sim`.
3. LLM failure cannot disable movement, schedules, or time.
4. All actor-facing coordinates have one authority.
5. Routine motion does not generate memories, inbox entries, prompts, or world revisions every step.
6. Navigation data is versioned against world geometry.
7. Behavior randomness is seeded and event-based, never dependent on render frame count.
8. Foreground player interaction outranks background reflection.
9. The system has budgets for path requests, behavior decisions, spatial messages, lights, sounds, and LLM work.
10. The disabled feature path preserves current stationary behavior while milestones are being integrated.
