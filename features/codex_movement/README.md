# Living City: movement, daily life, and world time

Status: design plan, not implementation

This folder proposes a complete movement and daily-life system for the Cathedral-City. It is deliberately larger than a pathfinding feature. If NPCs merely select random points and walk between them, the city will look busy for a few minutes and then feel arbitrary. The intended result is a city in which people appear to have obligations, habits, places they belong, reasons to stop, and believable responses when the player interrupts them.

The touchstones are:

- the legibility of Fallout 3/New Vegas NPC routines: a character goes to work, eats, sleeps, patrols, or relaxes because an ordered package is currently valid;
- the useful separation in `~/seagame` between slow behavior decisions and continuous path execution;
- the existing Cathedral architecture: `cathedral-sim` is authoritative and pure, while Bevy is a non-blocking projection;
- a strict cost rule: ordinary city life must consume zero LLM tokens.

The recommendation is a layered simulation:

1. A deterministic world clock determines calendar time and day phase.
2. Structured schedules and ordered behavior packages decide what an actor is trying to do.
3. An affordance registry turns intentions such as “work at a market stall” into reservable destinations.
4. A navigation service computes a route on a checked-in city navigation asset.
5. The pure simulation advances actors and owns their true positions.
6. Bevy receives spatial deltas, interpolates transforms, drives simple gait animation, and changes the sky and lights.
7. Optional LLM reflection can revise high-level goals or memories under a hard budget, but it cannot be required for waking, working, walking, or sleeping.

That split gives us a living city in fake-backend and no-key runs, deterministic headless tests, correct hearing and conversation ranges, and a clean path to richer cognition later.

## The core product promise

When this work is complete, a player should be able to do all of the following:

- wait near a market before dawn and watch sellers, porters, servants, watchmen, and early trades arrive for different reasons;
- follow one named person across several streets to a plausible work site;
- speak to that person while they are walking, see them stop and face the player, and see them resume or re-plan afterward;
- return at night and find most residents at home or offstage, taverns and watch routes active, and lamps illuminating useful streets;
- run the same cast headlessly and obtain the same schedule, decisions, destinations, and semantic outcomes from the same seed;
- disable every LLM backend and retain all of the above;
- enable reflection without allowing it to delay a player-facing reply or exceed a configured request/token/cost budget.

“Plausible” matters more than “maximally simulated.” We should not begin by implementing metabolism, a complete medieval economy, or a fully interactive interior for every building. The first system should model the facts that are visible to the player: where people go, when they go, what kind of activity they perform, how they share space, and how they handle interruptions.

## Recommended decisions at a glance

| Question | Recommendation |
|---|---|
| Who owns NPC position? | `cathedral-sim`, including route progress and spatial presence. |
| Who owns rendering and animation? | Bevy, as interpolation and visual projection only. |
| Is Bevy's frame clock the game clock? | No. Add an explicit, serializable `WorldTime`; keep provider monotonic time separate. |
| Does the LLM choose every action? | No. Ordered code/data-driven packages choose normal activity. |
| Does the LLM produce coordinates or paths? | Never. It may request a validated high-level intent from a fixed vocabulary. |
| Is the road polyline data enough for navigation? | No. It is too disconnected and many spawns are far from it. Bake walkable free space from canonical obstacle geometry. |
| Where does navigation execute? | In or immediately beneath the pure simulation, not solely in Bevy. |
| Navigation implementation | Spike project-owned nav data plus the engine-agnostic `landmass` core; use an offline Recast-family bake if it survives validation. Keep the interface replaceable. |
| What happens inside unavailable buildings? | Use explicit virtual interiors: walk to a portal, become offstage in a named place, then emerge at the portal. |
| How many needs initially? | Very few. Schedules and role affordances first; fatigue/rest and lightweight social pressure only if they improve visible behavior. |
| How do 500 actors scale? | Staggered decisions, path budgets, reservations, spatial deltas, and active/warm/cold simulation detail. |
| Default day length | Begin playtesting at 48 real minutes per game day; expose configuration and a short debug cycle. Do not blindly copy Sea Game's 12-minute day into a 1.2 km first-person city. |
| Daily LLM call for every NPC? | No. Dirty/eventful named actors only, heavily capped and spread over sleep periods; aggregate ambient trends at most. |
| Are routines written in prose? | No. Stable structured templates plus authored overrides. Prose goals remain narrative context. |

## Why this is not “just use a navmesh”

Pathfinding answers only one question: how can an actor reach a point? A convincing city also needs answers to:

- Why that point?
- Why now?
- Is the destination open and is there room?
- What should the actor visibly do there?
- What can interrupt the activity?
- What happens if the route is unavailable?
- Does the actor occupy a street, an inaccessible interior, or no spatial stage at all?
- When should a distant actor be simulated precisely?
- How does a moving actor remain audible, interactable, and promptable?
- How does the system avoid 500 simultaneous wake-ups, replans, or reflection calls?

The plan treats these as separate layers with stable contracts. That separation is what will keep later additions—doors, interiors, quests, crime, weather, carts, festivals, combat—from turning movement into an untestable switch statement.

## Document map

1. [Current state and constraints](01_current_state_and_constraints.md) records the repo evidence and the gaps the design must respect.
2. [Target simulation architecture](02_target_simulation_architecture.md) defines ownership, clocks, ticks, state machines, messages, and data flow.
3. [World time and day/night](03_world_time_and_day_night.md) specifies the clock, schedule semantics, sun/sky/light behavior, pausing, and time-scale tradeoffs.
4. [Navigation, locomotion, and crowds](04_navigation_locomotion_and_crowds.md) covers geometry, baking, routing, steering, LOD, stuck recovery, animation, and player collision.
5. [Behavior, schedules, and affordances](05_behavior_schedules_and_affordances.md) describes the Fallout-like package model, individual ideas, occupation templates, destinations, capacities, and virtual interiors.
6. [Interaction, goals, and LLM reflection](06_interaction_goals_and_llm.md) integrates conversation with movement and defines a bounded optional cognition layer.
7. [Data model and authoring](07_data_model_and_authoring.md) proposes concrete Rust types, authored data, generation, validation, persistence, and migration.
8. [Delivery sequence](08_delivery_sequence.md) breaks implementation into reviewable vertical milestones with feature flags and explicit exit criteria.
9. [Testing, observability, and budgets](09_testing_observability_and_budgets.md) defines deterministic tests, drive scenarios, diagnostics, performance targets, and cost gates.
10. [Risks and decisions](10_risks_and_decisions.md) collects open decisions, failure modes, tradeoffs, and recommended resolutions.

The documents intentionally repeat a few invariants where violating them would be particularly damaging. The most important are: simulation authority stays in `cathedral-sim`; routine behavior has no LLM dependency; wall time is not world time; and rendered transforms are never fed back as NPC truth.

## A compact model of the finished system

```text
authoring + generated assignments
  schedules, roles, spots, portals, nav asset, world seed
                         |
                         v
                  cathedral-sim
  +------------------------------------------------------+
  | WorldTime -> agenda -> ordered package evaluator     |
  |                         |                            |
  |                         v                            |
  |                 intent / activity                    |
  |                         |                            |
  |       reservations -> destination -> route           |
  |                                      |               |
  |                                      v               |
  |                             locomotion / presence    |
  |                                      |               |
  |          perception, speech, offers, snapshots       |
  +------------------------------------------------------+
          | semantic messages       | spatial deltas
          v                         v
      WorldMirror              render interpolation
                                      |
                                      v
                        Bevy transform / gait / audio

WorldTime -> sun, atmosphere, ambient light, lamps, HUD

material events -> experience journal -> optional reflection
                                      -> validated high-level updates
```

The behavior evaluator does not run every frame. It wakes on meaningful events or a staggered cadence. Locomotion advances on a fixed simulation step. Rendering advances every frame by interpolating between authoritative samples. This is the same broad idea that makes Sea Game's slow decisions plus continuous walking effective, but expressed in the Cathedral's pure-simulation architecture and scaled for a much larger, first-person world.

## Scope boundaries

This proposal includes:

- street-level pedestrian navigation;
- actor schedules, routine selection, destination selection, and occupancy;
- daily phases, a visible clock, celestial/ambient lighting changes, and lamp behavior;
- conversation interruption and resumption;
- optional structured reflection and high-level LLM intent;
- virtual interiors for unavailable spaces;
- deterministic simulation, spatial streaming, and performance levels of detail;
- the authoring and test infrastructure needed to keep all of that reliable.

It does not require in the first delivery:

- combat navigation or tactical cover;
- mounted actors, carts, boats, or animal-specific navigation;
- dynamically destructible navigation geometry;
- player-enterable interiors for all homes;
- a complete hunger/thirst/economy simulation;
- motion-matched skeletal animation;
- seasons, weather, lunar phases, or a calendar of festivals;
- an LLM-written plan for every person every morning.

The types and boundaries should leave room for these, but none should block a convincing first living-city slice.

## Definition of success

The feature is successful when movement communicates character and place without drawing attention to the machinery. A player should notice that the lamplighter is making rounds, that sellers disperse after market, and that the watch takes over after dusk. They should not notice synchronized decision ticks, destination piles, impossible doorways, foot sliding, sudden teleportation, or NPCs forgetting a conversation because their schedule fired.

Technically, success means:

- every spatially present actor has one authoritative simulation position;
- navigation never depends on the camera/render world;
- important gameplay queries use the current simulated position;
- routine decisions and path execution are deterministic under a fixed seed;
- no normal routine creates an LLM request;
- foreground conversation always outranks reflection work;
- all bursts—dawn schedules, path planning, snapshots, lamps, and reflection—are explicitly budgeted;
- the headless executable can advance multiple game days and assert the same invariants as the game.

That is the foundation on which richer personal lives can be added safely.
