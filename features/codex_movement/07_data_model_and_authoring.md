# 7. Data model and authoring

Movement will fail at content scale if every detail is encoded in Rust conditionals. It will also fail if runtime code repeatedly guesses from lore prose. The content pipeline should compile rich authored sources into normalized, validated simulation data with stable IDs.

## 7.1 Three kinds of data

Keep these domains separate:

### Authored lore

The existing character and place records remain human-facing canonical material: names, descriptions, occupations, conditions, relationships, prose goals, ward associations, and history.

### Compiled world behavior data

Generated and reviewed data binds lore to simulation concepts:

- actor behavior profile;
- household/home/shelter assignment;
- workplace/role/shift assignment;
- schedule template;
- mobility profile;
- patrol/lamp/delivery route;
- activity spot/portal assignments;
- authored overrides.

This should live in a clear generated/runtime asset namespace, not be smuggled into arbitrary description strings.

### Runtime/save state

Session state records the current day, presence, position, intent, activity, needs, reservations, journal, memories, and deterministic counters. It must not rewrite lore source files when the actor changes a goal during play.

## 7.2 Stable identifiers

Use opaque stable IDs for all cross-file/domain references:

```rust
ActorId
PlaceId
BuildingId
HouseholdId
ActivitySpotId
PortalId
PatrolRouteId
ScheduleTemplateId
RoleId
NavigationProfileId
```

String-backed IDs are fine in authored RON/JSON and can compile to dense indices at load. IDs must not derive solely from array order or Bevy entity values. Generation should preserve an existing ID whenever its source identity remains.

Names are labels, not keys. There can be repeated occupations, changed spellings, and future duplicate personal names.

## 7.3 Suggested asset layout

One possible layout:

```text
assets/simulation/ombreval/
  behavior_schema.ron             # version metadata
  actors.ron                      # normalized assignments and overrides
  households.ron
  activity_spots.ron
  portals.ron
  patrol_routes.ron
  schedule_templates.ron
  role_templates.ron
  lamp_routes.ron
  navigation.nav                  # checked-in compiled artifact
  navigation_report.json
  validation_report.json

tools/world_compile/
  occupation_mapping.ron
  building_use_mapping.ron
  assignment_rules.ron
```

The exact root can follow existing asset conventions. The important points are:

- generated output is recognizable;
- hand-authored overrides are not overwritten silently;
- schema/generator versions are embedded;
- a single validation command covers cross-file references;
- diffs are reviewable or accompanied by a deterministic report.

If the team prefers behavior bindings next to lore records, add a separate structured field with schema tooling. Do not mix transient runtime state into lore.

## 7.4 Actor definition

Proposed normalized definition:

```rust
pub struct ActorBehaviorDefinition {
    pub actor: ActorId,
    pub significance: SignificanceTier,
    pub roles: Vec<RoleId>,
    pub schedule: ScheduleTemplateId,
    pub home: ResidenceAssignment,
    pub work: Vec<WorkAssignment>,
    pub household: Option<HouseholdId>,
    pub mobility: MobilityProfileId,
    pub preferences: BehaviorPreferences,
    pub default_motives: Vec<WeightedMotive>,
    pub authored_overrides: ActorBehaviorOverrides,
}

pub enum ResidenceAssignment {
    HouseholdInterior { household: HouseholdId, portal: PortalId },
    Lodging { place: PlaceId, portal: PortalId },
    Institution { place: PlaceId, portal: PortalId },
    Shelter { spot: ActivitySpotId },
    NoFixedResidence { night_query: AffordanceQueryId },
}

pub struct WorkAssignment {
    pub role: RoleId,
    pub place: Option<PlaceId>,
    pub primary_spot: Option<ActivitySpotId>,
    pub shift: Option<ShiftTemplateId>,
    pub route: Option<PatrolRouteId>,
}
```

`NoFixedResidence` is an explicit behavioral reality, not missing data. “No residence assignment at all” is an error.

## 7.5 Runtime actor state

Extend the sim's character state through a composed runtime structure rather than bloating prompt-facing fields:

```rust
pub struct ActorSimulationState {
    pub presence: SpatialPresence,
    pub kinematics: Kinematics,
    pub activity: Activity,
    pub intent: Option<Intent>,
    pub suspended_intents: SmallVec<[SuspendedIntent; 2]>,
    pub agenda: DailyAgenda,
    pub needs: LightweightNeeds,
    pub reservations: ActorReservationRefs,
    pub decision_state: DecisionState,
    pub journal: ExperienceJournal,
    pub structured_goal: StructuredGoalState,
}

pub struct Kinematics {
    pub position: Vec3,
    pub facing: Vec3,
    pub velocity: Vec3,
    pub movement_profile: MobilityProfileId,
    pub last_safe_nav_location: Option<NavLocation>,
}
```

For an interior actor, the `Kinematics.position` can retain the last portal position as historical data, but APIs must use `presence` and refuse to treat it as a current street coordinate.

Route library objects, spatial-index handles, and provider request handles live in runtime caches keyed by stable actor ID; they are not serialized directly.

## 7.6 Public snapshot additions

Consumers need enough state to project actors without exposing internal planner machinery:

```rust
pub struct ActorSnapshot {
    // existing fields...
    pub presence: SpatialPresenceSnapshot,
    pub activity: PublicActivity,
}

pub enum PublicActivity {
    Idle,
    Walking { purpose: PublicPurpose, destination: Option<PlaceId> },
    Working { kind: PublicWorkKind },
    Sleeping,
    Socializing,
    Conversing,
    Waiting,
    Other,
}
```

Keep detailed route, package score, private motive, and exact schedule in debug messages rather than normal snapshots. A public purpose helps animation, debug UI, and contextual prompts without leaking hidden internal plans to unrelated UI.

Spatial batches carry high-rate movement as defined earlier. Presence changes belong in both protocols carefully: the semantic snapshot tells consumers that the actor is interior; the spatial batch immediately removes/resets the render projection. Use simulation tick ordering to resolve whichever arrives first.

## 7.7 Affordance data

Example RON-like activity spot:

```ron
(
    id: "spot:wickmarket:stall_014:seller",
    source: (building: "building:1042", generator: "stall_spots_v2"),
    place: Some("place:wickmarket"),
    nav_profile: "pedestrian",
    position: (x: 184.2, y: 0.0, z: -92.6),
    arrival_radius: 0.45,
    facing: (toward: "building:1042"),
    tags: ["work", "market", "stall_seller"],
    slots: [
        (id: "seller", capacity: 1, role: Some("market_seller")),
    ],
    opening: (template: "market_day_hours"),
)
```

Generated coordinates should be rounded/normalized consistently so harmless floating-point variations do not create huge diffs. Store the source/generator version for traceability.

## 7.8 Portal data

```rust
pub struct PortalDefinition {
    pub id: PortalId,
    pub exterior_spot: ActivitySpotId,
    pub interior: PlaceId,
    pub capacity: u8,
    pub transition_duration: PhysicalDuration,
    pub availability: AvailabilitySchedule,
    pub access: AccessPolicy,
    pub nav_link: NavLinkId,
}
```

Portal semantics distinguish:

- residence entrance;
- workplace/service entrance;
- public building entrance;
- shelter transition;
- future real interior connection.

Multiple households may share a tenement portal while retaining separate interior/household identity. A tavern may have one public and one service portal. Accessibility restrictions and future locks belong here, not in pathfinding coordinates.

## 7.9 Schedule template data

Example:

```ron
(
    id: "schedule:market_seller:standard",
    day_variants: {
        "ordinary": [
            (
                id: "setup",
                window: (earliest: "05:35", preferred: "06:00", latest: "06:40"),
                activity: (work: "market_setup"),
                destination: (assignment: "primary_work"),
                importance: "required",
                jitter_minutes: (-12, 12),
            ),
            (
                id: "sell",
                window: (earliest: "06:15", preferred: "06:30", latest: "08:00"),
                activity: (work: "tend_stall"),
                end: (phase: "dusk", offset_minutes: -60),
                importance: "required",
            ),
            (
                id: "evening",
                window: (earliest: "18:00", preferred: "19:00", latest: "21:30"),
                activity: (optional: "home_or_social"),
                importance: "optional",
            ),
            (
                id: "sleep",
                window: (earliest: "21:30", preferred: "22:30", latest: "00:30"),
                activity: "sleep",
                destination: "residence",
                importance: "essential",
            ),
        ],
    },
)
```

Compile text times to validated seconds-of-day and overnight absolute windows. Validate overlapping required entries and impossible minimum durations. Content can reference phase offsets to remain compatible with later seasonal light changes.

## 7.10 Package template format

Do not build a completely general expression interpreter. A constrained RON schema could be:

```ron
(
    id: "package:scheduled_work",
    tier: "scheduled_routine",
    conditions: [
        (agenda_entry_active: true),
        (not: (attention_lease: "player")),
    ],
    intent: (from_agenda: true),
    interruption: "resume_if_still_valid",
    failure: ["alternate_assigned_spot", "wait_nearby", "safe_fallback"],
)
```

Package kinds and condition variants are Rust enums. Unknown fields/variants fail load. All referenced IDs are validated. Major-character overrides can disable a template or insert a typed directive package without embedding code.

## 7.11 Occupation and role compilation

Create a reviewed mapping table:

```ron
{
    "watchman": ["watch", "armed_public_role"],
    "lamplighter": ["lamplighter", "municipal_service"],
    "domestic servant": ["household_service", "errand_worker"],
    "baker": ["food_provision", "early_shift"],
    // ...
}
```

Mapping reports every unmatched occupation. A role assignment may incorporate title/faction/building context through explicit rules, but manual overrides win. Keep mapping versioned so a changed category produces a reviewable actor diff.

Do not use fuzzy string matching during gameplay.

## 7.12 Household and workplace assignment algorithm

An offline deterministic assignment pass can solve a constrained matching problem:

### Candidate generation

- residential/lodging/institution/shelter buildings in compatible ward;
- occupation-compatible work buildings/sites;
- capacity estimated from footprint/use and authored limits;
- accessible portal and nav component;
- family/household constraints;
- status/context constraints where explicitly modeled.

### Cost

```text
assignment cost = commute travel time
                + ward mismatch penalty
                + capacity pressure
                + role/building mismatch
                + household split penalty
                + accessibility penalty
                + authored preference penalties
```

### Solve

Use deterministic stable ordering and a min-cost/greedy-with-repair strategy appropriate to scale. Exact global optimality matters less than transparent constraints and good reports. Preserve existing assignments unless a relevant input changed, to avoid noisy asset churn.

### Review outputs

- capacity utilization by building/spot;
- commute percentiles by role and ward;
- top 50 longest routes;
- unmatched actors;
- fallback assignments;
- household splits;
- major-character generic assignments;
- unreachable portals.

The runtime loads results; it does not rerun the solver.

## 7.13 Activity spot generation

Generators should be small versioned passes:

- building portal generator;
- market stall seller/customer slots;
- square edge observe/social/rest spots;
- well/cistern interaction and waiting spots;
- guard/gate posts;
- lamp service points and circuits;
- church public/service/prayer spots;
- workshop façade/work slots;
- shelter/arcade night spots;
- generic delivery nodes.

Each pass declares source inputs, generation version, and validation. Hand-authored spot files can supplement or override generated records by stable ID. If a generator would move an overridden spot, it reports the discrepancy instead of overwriting silently.

## 7.14 World events

Behavior and reflection need typed events with different retention:

```rust
pub struct WorldEvent {
    pub id: WorldEventId,
    pub at: GameInstant,
    pub location: EventLocation,
    pub kind: WorldEventKind,
    pub participants: SmallVec<[ActorId; 4]>,
    pub materiality: Materiality,
    pub visibility: EventVisibility,
}
```

Routine examples:

- phase changed;
- work shift opened/closed;
- lamp changed state;
- actor arrived/completed activity;
- portal transition;
- reservation contention.

Material examples:

- player agreement;
- offer outcome;
- important route/goal failure;
- future incident witnessed.

Routine events feed behavior/diagnostics and expire quickly. Only material perceived events enter journals. Movement steps are spatial samples, not world events.

## 7.15 Configuration

Add a clear top-level simulation section rather than hiding time and movement among provider settings:

```ron
simulation: (
    enabled: true,
    world_seed: 12345,
    clock: (
        start_day: 1,
        start_time: "08:00",
        real_minutes_per_day: 48.0,
        pause_in_menu: true,
    ),
    movement: (
        enabled: true,
        fixed_step_ms: 100,
        active_radius_m: 100.0,
        warm_radius_m: 300.0,
        max_routes_per_tick: 6,
        spatial_sample_hz: 10.0,
    ),
    behavior: (
        enabled: true,
        fallback_check_game_minutes: 5,
    ),
)
```

Reflection remains under smart-actor/backend configuration because it controls cognition cost. Validate ranges at startup and print effective values once. Real-minutes-per-day changes calendar scale, not physical movement speed.

Feature rollout flags should allow:

- clock visuals only;
- one-actor allowlist;
- navigation debug without behavior;
- behavior decisions with movement frozen;
- full cast;
- reflection independently.

Avoid a permanent maze of flags. Remove milestone-only flags after stabilization, retaining useful debug and accessibility controls.

## 7.16 Save schema

Even before a general save UI, derive/implement serialization for authoritative state where feasible. Proposed save fragment:

```rust
pub struct MovementSaveV1 {
    pub schema_version: u32,
    pub world_data_hash: WorldDataHash,
    pub navigation_hash: NavigationHash,
    pub world_seed: u64,
    pub time: WorldTime,
    pub actors: Vec<ActorMovementSaveV1>,
    pub reservations: Vec<ReservationSaveV1>,
    pub reflection_budget: ReflectionBudgetSaveV1,
}
```

Per actor persist:

- presence and validated position/facing;
- active/suspended structured intent;
- current performance and remaining semantic duration;
- agenda day/entries/completion;
- needs;
- decision sequence and cooldowns;
- structured motive/directives;
- material journal/reflection revision;
- reservation references.

Do not persist:

- Bevy entity IDs;
- interpolation samples;
- spatial index handles;
- third-party nav query objects;
- raw provider in-flight handles;
- local avoidance neighbors;
- animation phase (optional cosmetic only).

Routes can be persisted as semantic start/goal/progress if useful, but should be rebuilt against current topology on load. Preserve enough progress/time to place the actor plausibly.

## 7.17 Migration rules

On load:

1. check save schema and migrate typed versions;
2. compare world and navigation hashes;
3. resolve stable actor/place/spot IDs;
4. retain semantic state where IDs still exist;
5. reproject present positions onto valid nav within a small tolerance;
6. discard/replan routes;
7. rebuild reservations deterministically, resolving conflicts by saved lease/priority/actor ID;
8. replace removed destinations using declared fallback;
9. preserve journal/memories unless their actor references no longer exist;
10. emit a migration report.

If a present actor is now disconnected or deeply inside an obstacle, use the actor's home/portal/last-safe location under an explicit migration recovery. Never silently place everyone at the origin.

## 7.18 Validation command

Provide one developer-facing command that runs all content checks without Bevy:

```sh
cargo run -p cathedral-tools --bin validate-living-city -- --strict
```

Checks:

- schema and reference integrity;
- every actor has role, schedule, mobility, and residence policy;
- required work assignments exist;
- every spot/portal samples onto correct nav component;
- every required agenda entry has a feasible destination;
- capacity and shift oversubscription;
- commute limits and route availability;
- patrol/lamp circuits are connected;
- schedule conflicts/overnight correctness;
- package failure chains terminate in a safe fallback;
- generated files match source/generator hashes;
- major-character override coverage report;
- no duplicate IDs or order-dependent generation.

Warnings should be grouped and capped in terminal output with a complete JSON report. Strict CI promotes selected warnings to errors.

## 7.19 Authoring workflow

Recommended workflow for adding or changing an actor/place:

1. edit canonical lore/place/cadastral data;
2. run behavior/world compilation;
3. inspect assignment diff and validation summary;
4. add an override if the generic assignment conflicts with story intent;
5. rebake navigation only if geometry/topology changed;
6. run headless day simulation for affected actors/ward;
7. use Bevy debug overlay for approach/facing/visual fit;
8. commit source, reviewed generated data, and reports/artifacts required by project policy.

Do not ask content authors to hand-calculate coordinates when a generator can place/validate the spot. Do allow them to choose semantic relationships: “Ilse works at stall 14 on the seller slot” is a durable override.

## 7.20 Data acceptance criteria

1. Runtime behavior never depends on fuzzy parsing of lore strings.
2. All cross-references use stable IDs and validate before play.
3. Generated actor assignments are deterministic and reviewable.
4. Every cast member has an explicit residence/night policy, including no-fixed-residence cases.
5. All required destinations are nav-validated and capacity-aware.
6. Authored overrides survive regeneration.
7. Public snapshots expose activity/presence without leaking planner internals.
8. Authoritative movement state has a clear serialization and migration policy.
9. One headless validation command reports schema, navigation, schedule, assignment, and capacity errors.
10. No LLM output ever modifies canonical lore assets directly.
