# 2. Target simulation architecture

## 2.1 Ownership model

The target design keeps one authoritative actor state in `cathedral-sim` and creates two projections of it:

- a semantic projection (`WorldMirror`) for names, inventory, goals, activity, interaction state, and presence;
- a spatial projection for smoothly rendered positions, facing, velocity, and gait state.

The renderer is allowed to be ahead of the last spatial sample only through bounded interpolation/extrapolation. It is never allowed to commit that estimate back as NPC truth.

```text
Bevy input / frame delta                 Provider results / speech
          |                                        |
          v                                        v
   HostCommand queue ----------------> Engine::poll(host_now, commands)
                                              |
               +------------------------------+--------------------+
               |                              |                    |
               v                              v                    v
      fixed simulation steps          cognition scheduler      message drain
               |                              |                    |
               v                              |                    |
       authoritative World <------------------+                    |
               |                                                   |
               +---------- semantic snapshot ----------------------+
               +---------- spatial delta batch --------------------+
               +---------- diagnostic events ----------------------+
```

The host supplies elapsed physical simulation time as data. The sim does not call a clock. The headless binary supplies the same command using virtual deterministic deltas.

## 2.2 Separate time domains

Add explicit inputs rather than overloading the existing `now`:

```rust
pub struct PollInput {
    /// Monotonic host time used for backend/floor/request lifetimes.
    pub host_now: HostInstant,
    /// Elapsed playable time since the previous pump, zero while world-paused.
    pub simulation_delta: Duration,
    pub commands: Vec<EngineCommand>,
}
```

If changing the public `poll` signature immediately is undesirable, introduce an `EngineCommand::AdvanceSimulation { delta }` first and fold it into a later API cleanup. The critical requirement is that tests can supply the delta explicitly.

Within the engine:

- `simulation_delta` feeds a fixed-step accumulator;
- each fixed physical step advances locomotion and short-lived gameplay leases;
- the world clock advances by `simulation_delta * calendar_scale`;
- `host_now` continues to age provider and speech work even when the world is paused.

The fixed physical step should begin at 100 ms (10 Hz). It is frequent enough for authoritative pedestrian motion when Bevy interpolates, and cheap enough for 500 actors. This is a configuration constant for measurement, not a permanent promise.

Use a maximum number of catch-up steps per host frame. When a debugger stop or frame stall exceeds the bound:

1. run at most the configured catch-up budget;
2. retain or intentionally clamp the remainder according to a documented policy;
3. emit a `simulation_lagged` diagnostic;
4. never take one giant steering step through obstacles.

For normal focus loss, pause the playable simulation rather than accumulating a huge delta.

## 2.3 World state additions

At a high level, the simulation needs these new resources:

```rust
pub struct World {
    // existing semantic state...
    pub time: WorldTime,
    pub navigation: NavigationWorld,
    pub affordances: AffordanceRegistry,
    pub reservations: ReservationBook,
    pub behavior: BehaviorRuntime,
    pub movement: MovementRuntime,
    pub world_seed: u64,
    pub simulation_tick: u64,
    pub semantic_revision: u64,
    pub spatial_revision: u64,
}
```

Some of these may live in `Engine` rather than the serialized domain `World`; the exact placement should follow the crate's existing encapsulation. The conceptual ownership is more important:

- `WorldTime` is semantic and serializable;
- actor intentions, presence, schedule state, and reservations are semantic and serializable;
- a computed route can be transient and rebuilt after load;
- navigation topology is immutable/versioned world data;
- steering scratch buffers and route cache entries are runtime accelerators;
- deterministic decision counters are semantic if save/load must reproduce future choices.

## 2.4 Actor runtime state

Do not encode movement as a growing set of booleans. Give every actor a small orthogonal set of states:

```rust
pub struct ActorRuntime {
    pub presence: SpatialPresence,
    pub kinematics: Kinematics,
    pub activity: Activity,
    pub active_intent: Option<Intent>,
    pub agenda: DailyAgenda,
    pub needs: LightweightNeeds,
    pub attention_lease: Option<AttentionLease>,
    pub decision: DecisionRuntime,
}

pub enum SpatialPresence {
    Present {
        position: Vec3,
        facing: Vec3,
    },
    Interior {
        place: PlaceId,
        entry_portal: PortalId,
        since: GameInstant,
    },
    Absent {
        reason: AbsenceReason,
    },
}

pub enum Activity {
    Idle { until_check: SimInstant },
    AcquiringDestination { intent_id: IntentId },
    AwaitingRoute { request_id: RouteRequestId },
    Travelling(TravelActivity),
    Performing(PerformanceActivity),
    Conversing(ConversationActivity),
    Waiting(WaitingActivity),
    TransitioningPortal(PortalTransition),
    Recovering(StuckRecovery),
}
```

`Activity` answers “what is happening now?” `Intent` answers “why are we doing it?” A route answers “how do we reach it?” Keeping those separate makes interruption and resumption tractable.

A `TravelActivity` should include stable route/corridor identifiers, destination, progress, current segment, actual and desired velocity, expected arrival, failure count, and a resumption policy. It should not contain a Bevy entity or animation handle.

## 2.5 Intents and lifecycle

An intent is an executable, structured objective selected by schedule, package evaluator, authored story state, or validated cognition:

```rust
pub struct Intent {
    pub id: IntentId,
    pub source: IntentSource,
    pub kind: IntentKind,
    pub priority: Priority,
    pub created_at: GameInstant,
    pub validity: IntentValidity,
    pub interruptibility: Interruptibility,
    pub resume: ResumePolicy,
    pub reason: ReasonCode,
}
```

Examples of `IntentKind` are `Work`, `Sleep`, `EatMeal`, `Socialize`, `AttendService`, `Patrol`, `LightLamps`, `FetchWater`, `WaitFor`, and `Visit`. Each kind contains typed references or an affordance query, not prose.

Lifecycle:

1. A wake event asks the package evaluator for the first valid/highest-scoring intent.
2. The actor tries to reserve an appropriate affordance.
3. If already within its interaction region, it begins performing.
4. Otherwise it requests a route and enters `Travelling` when one is available.
5. Arrival begins a timed or condition-based performance.
6. Completion emits a compact material or routine event and schedules the next wake.
7. Interruption either suspends, cancels, or replaces the intent according to policy.
8. Invalid destinations, repeated route failure, closed hours, or expired windows force re-evaluation.

No step needs an LLM response.

## 2.6 Event-driven decisions, fixed-step execution

Behavior selection should not run for every actor on every 100 ms step. An actor becomes decision-ready when:

- a scheduled window opens or closes;
- its current activity completes;
- an urgent condition changes;
- a destination becomes invalid;
- a reservation is revoked;
- a player interaction starts or its attention lease ends;
- a relevant world event is delivered;
- its staggered fallback check is due.

Use a time-ordered wake queue keyed by physical or game time as appropriate. A small fallback reevaluation—roughly every 2–10 game minutes, with deterministic jitter—catches missed changes and resembles the robustness of package reevaluation without becoming a per-frame scan.

Locomotion, by contrast, advances on fixed physical steps. It follows the existing route and should not re-run high-level decisions unless it encounters a trigger.

## 2.7 Stable randomness

Random variation makes routines less synchronized but can easily destroy determinism. Never roll from a shared RNG based on frame order. Derive a local seed from stable facts:

```text
hash(world_seed, actor_id, game_day, decision_kind, decision_sequence)
```

Use it for:

- daily start-time jitter;
- selecting among equally valid work or leisure spots;
- sandbox dwell duration;
- route variation when multiple paths have similar cost;
- repetition avoidance.

Increment `decision_sequence` only when a semantic choice is actually made. Simulation LOD, render frame rate, path queue timing, and map iteration order must not change it. Sort candidate IDs before weighted choice.

## 2.8 Semantic and spatial output protocols

Recommended message split:

```rust
pub enum EngineMessage {
    // existing variants...
    WorldSnapshot(WorldSnapshot),
    ActorSpatialBatch(ActorSpatialBatch),
    WorldClockChanged(WorldClockSnapshot),
    BehaviorDiagnostic(BehaviorDiagnostic),
}

pub struct ActorSpatialBatch {
    pub simulation_tick: u64,
    pub sample_time: SimInstant,
    pub updates: Vec<ActorSpatialUpdate>,
}

pub struct ActorSpatialUpdate {
    pub actor: ActorId,
    pub presence: SpatialPresenceSnapshot,
    pub position: Vec3,
    pub facing: Vec3,
    pub velocity: Vec3,
    pub locomotion: LocomotionSnapshot,
}
```

Possible optimizations should follow measurement:

- omit unchanged/stationary actors;
- quantize velocity or position only if fidelity remains acceptable;
- send active-neighborhood actors at 10 Hz and cold actors on meaningful samples;
- use a dense stable actor index internally while keeping IDs at API boundaries.

Do not prematurely create a binary wire protocol; the engine is in process. The important optimization is avoiding a complete semantic snapshot and full view reconciliation on every movement step.

Bevy stores the last two authoritative samples per actor:

```rust
struct RenderMotion {
    previous: SpatialSample,
    current: SpatialSample,
    received_at: Instant,
}
```

It renders a short interpolation delay behind the simulation. Extrapolation, if used, must be brief and clamped. A presence transition, nav correction, or large discontinuity resets interpolation intentionally.

## 2.9 Position-dependent systems must read the same truth

The following existing or future systems must query the simulated/current actor position, not the render transform:

- hearing and speech delivery;
- stage/attention membership;
- visibility approximations used by cognition gating;
- offers, handoffs, and proximity actions;
- sound origins;
- conversation facing and attention leases;
- occupancy and crowd queries;
- event witnesses;
- prompt location descriptions.

For cold actors represented analytically along a route, `MovementRuntime::position_at(actor, sim_instant)` must materialize a deterministic position before a spatial query. The optimization may avoid updating a stored vector ten times a second, but it cannot make position unknowable.

Offstage/interior actors are not “very far away.” They have a distinct presence state. Street perception excludes them unless a future same-interior model explicitly connects those spaces.

## 2.10 Interaction leases

When player interaction targets a moving NPC, the system should acquire an attention lease immediately, before an LLM reply exists:

1. suspend interruptible travel/performance;
2. decelerate rather than snap to rest;
3. turn toward the player when spatially present;
4. retain the reservation for a short grace period if appropriate;
5. keep the NPC engaged while targeted speech, an offer, or a response is pending;
6. expire after a configured quiet interval, then resume or re-plan.

This prevents a schedule transition from making the target walk away while a provider request is in flight. It also ensures the semantics work with the fake backend.

High-priority non-interruptible states should be rare and explicit: a dangerous portal transition, a future combat reaction, or a scripted critical action. “Going to work” should not make a character refuse to stop for the player.

## 2.11 Reservation ownership

Reservations are authoritative simulation state, not navigation state. They prevent crowd piles and allow schedule validity checks.

Rules:

- acquire before committing to a capacity-limited destination;
- apply deterministic tie-breaking;
- give reservations an arrival deadline;
- renew while travelling if progress is reasonable;
- release on cancellation, prolonged conversation, portal transition, completion, or load migration;
- allow a short suspended grace period so a greeting does not lose a job slot instantly;
- expose diagnostics for leaks and chronic contention.

A route to a full stall is not a navigation failure. It is a destination-selection failure and should cause package re-evaluation or fallback.

## 2.12 Sim LOD is execution detail, not different behavior

Use three spatial detail bands as an initial model:

- **active**: near the player or otherwise interaction-relevant; full steering/local avoidance at 10 Hz, frequent spatial samples;
- **warm**: in the broader neighborhood; lower-frequency neighbor queries and spatial samples, route progress remains exact;
- **cold**: far away/offscreen; analytic progress along an already computed route, event-driven activity completion, no per-step crowd avoidance.

Transitions must not reroll decisions, reset dwell time, skip semantic events, or teleport an actor. Promoting a cold traveler computes its current route position, resolves nearby occupancy, and eases any small correction. If the exact path has become invalid due to future dynamic obstacles, it enters the same replan path an active actor would.

Avoidance quality may differ by band. Schedule and intent outcomes should not.

## 2.13 Typical end-to-end sequence

At 05:43, a market seller's agenda wake fires:

1. The schedule package for “set up market” is valid.
2. The actor has no urgent condition or conversation lease.
3. The evaluator creates `Intent::Work` against the actor's assigned stall.
4. The reservation book grants the stall's seller slot until an arrival deadline.
5. The route service queues a path from the actor's home portal to the stall approach point.
6. A path budget allows the request on the next tick.
7. The actor exits its virtual home, becomes spatially present at the portal, and follows the route.
8. Spatial batches update the renderer; the semantic snapshot changes only for presence/activity.
9. The player speaks at 05:51. An attention lease suspends travel and the actor faces the player.
10. The response ends. The stall reservation is still valid, so the actor resumes from its current position.
11. On arrival, the actor releases route data, occupies the stall, and performs `TendStall` until the next schedule boundary or interruption.

Only step 9–10 may involve an LLM, and only because the player initiated a conversation. Everything else is deterministic simulation.

## 2.14 Suggested module boundaries

Exact names can follow local conventions, but the dependencies should remain acyclic:

```text
cathedral-sim/src/
  time.rs                 WorldTime, calendar arithmetic, phase events
  behavior/
    mod.rs                evaluator API and wake queue
    package.rs            conditions, priorities, templates
    agenda.rs             daily materialization
    intent.rs             typed intent lifecycle
    needs.rs              deliberately small need set
  navigation/
    mod.rs                replaceable route/steering interface
    asset.rs              versioned nav data loader types
    query.rs              route requests and errors
    movement.rs           fixed-step route execution
    avoidance.rs          active-band local steering adapter
  affordance/
    mod.rs                registry/query index
    reservation.rs        capacity and leases
    portal.rs             virtual interior transitions
  reflection/
    journal.rs            compact material experiences
    scheduler.rs          optional background budget lane
    schema.rs             validated output
```

Avoid making `behavior` depend directly on Bevy, authored JSON parsing, an HTTP client, or renderer state. Authoring compilation belongs in a build/offline layer or backends host; the sim consumes normalized data.
