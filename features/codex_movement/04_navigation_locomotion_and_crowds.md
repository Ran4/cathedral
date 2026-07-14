# 4. Navigation, locomotion, and crowds

Navigation is the highest technical-risk portion of the feature. It has to represent a dense kilometer-scale procedural city, narrow alleys, plazas, walls, gates, covered passages, bridges, and eventually portals, while remaining usable in the pure headless simulation.

The recommendation is to put a small project-owned navigation API inside `cathedral-sim`, load a versioned checked-in navigation artifact, and keep the underlying path/avoidance implementation replaceable. An offline bake should derive walkable topology from canonical world geometry. Do not make a Bevy ECS plugin the sole owner of routes.

## 4.1 Requirements

The street-level navigation system must:

- return routes that agree with player collision geometry;
- cover roads, squares, courtyards, alleys, gates, churchyards, markets, and valid open ground;
- retain height/topology through bridges, stairs, ramps, and passages;
- support stable links to virtual-interior portals;
- reject or recover from unreachable destinations;
- run deterministically enough for headless behavior tests;
- handle approximately 500 actors without 500 route searches in one frame;
- expose route length/ETA before or during schedule planning;
- provide current route position for cold actors without simulating a render frame;
- allow active actors to avoid one another and the player;
- provide diagnostics that make bad bakes and stuck actors visible.

It does not initially need combat cover, dynamic destruction, arbitrary jumping, vehicles, or physically simulated doors.

## 4.2 Why road centerlines are not the topology

The existing 49 road polylines are useful semantic data but fail as a complete graph:

- most endpoints are not close enough to form a connected endpoint graph;
- many actor spawns are tens of meters from a centerline;
- plazas and broad open areas require free movement, not only lane following;
- alleys and covered passages may be represented by building offsets/collision rather than a named road;
- bridge and elevation topology cannot be recovered reliably from a flat centerline list;
- arbitrary connectors from every spawn would cross obstacles unless separately validated.

Keep roads as optional route-cost metadata: actors can prefer named thoroughfares, avoid a dangerous district, or describe the street they are on. Use a navigable surface for actual reachability.

## 4.3 Canonical bake geometry

Create an intermediate `NavigationBakeInput` exported from the same procedural/cadastral definitions used to build the city:

```rust
pub struct NavigationBakeInput {
    pub schema_version: u32,
    pub source_hash: [u8; 32],
    pub bounds: Aabb2,
    pub walkable_surfaces: Vec<SurfaceMesh>,
    pub obstacle_footprints: Vec<ObstaclePrism>,
    pub excluded_regions: Vec<TaggedPolygon>,
    pub explicit_links: Vec<BakeLink>,
    pub portals: Vec<PortalBakeRecord>,
    pub semantic_regions: Vec<RegionAnnotation>,
}
```

The export should include stable source IDs for diagnostics: `building:1234`, `cathedral:north_transept`, `wall:east_17`, `fixture:well_04`, and so on. When a nav polygon is unexpectedly absent, the debug tooling can report which obstacle created the exclusion.

The canonical layer must cover bespoke colliders in addition to cadastral building footprints. Refactoring does not require the renderer and controller to consume identical low-level triangle arrays, but they must consume the same declared bounds and exclusions.

Initial street-level assumptions should be explicit:

- ground and authored raised walkways are walkable surfaces;
- building footprints, city walls, substantial fixtures, water, steep/non-walkable surfaces, and collision prisms are excluded;
- decorative geometry that does not block the player does not block navigation;
- doorways into unavailable interiors terminate at portals;
- no route is allowed through an implied interior merely because a footprint has a gap;
- the cathedral interior is included only if its collision/walkable data can be exported reliably.

## 4.4 Offline bake, checked-in asset

Runtime generation of a kilometer-scale navmesh would increase startup time, introduce platform variation, and make failures harder to review. Prefer an offline command:

```sh
cargo run -p cathedral-tools --bin bake-navigation -- \
  --world ombreval \
  --out assets/navigation/ombreval.nav.ron
```

Binary output may be appropriate after profiling, but begin with a debuggable/versioned representation or provide a companion JSON/RON report.

The bake produces:

- polygon vertices and adjacency;
- area/region IDs and height data;
- link records;
- connected-component IDs;
- clearance/profile metadata;
- source geometry hash;
- bake parameter hash;
- summary statistics and validation failures.

Check the artifact into the repository, just like generated world assets. CI regenerates it in verification mode and fails if output or hashes differ unexpectedly. A developer command should generate an overhead SVG/PNG or simple geometry dump colored by connected component.

Startup behavior:

1. load the nav asset;
2. compare its source hash and supported schema version;
3. validate required spawn/portal/activity anchors;
4. fail loudly in development if stale;
5. in a release build, either disable movement with a clear diagnostic or use a deliberately supported fallback—never silently route through mismatched geometry.

## 4.5 Candidate implementation and mandatory spike

At the time of this design:

- [`landmass`](https://docs.rs/landmass/) is an engine-agnostic Rust navigation library that provides pathfinding, path simplification, steering, and avoidance;
- [`bevy_landmass` 0.12.0](https://docs.rs/crate/bevy_landmass/0.12.0) has a Bevy 0.19-compatible release and is useful as an integration/reference prototype, but a Bevy-only owner would conflict with headless simulation authority;
- [`rerecast`](https://docs.rs/rerecast/) provides a Rust Recast-style generation path suitable for investigating an offline bake;
- the corresponding high-level Bevy Recast integration has not consistently matched this project's current Bevy version, so it should not be made a prerequisite.

Recommended spike:

1. export one representative district containing a square, a narrow alley, a covered passage, a gate/bridge, irregular building footprints, and at least 20 current spawns;
2. bake it with raw `rerecast` or another offline method;
3. load the result into a pure test executable;
4. query a fixed matrix of routes;
5. advance 50 agents with the engine-agnostic `landmass` core;
6. compare output against a small custom polygon A* + corridor follower if adapter friction is high;
7. render a Bevy debug overlay, but keep the authoritative update outside Bevy;
8. measure determinism, memory, path latency, narrow-passage behavior, and dependency/version friction.

Do not commit the entire feature to a library before this spike. `landmass` currently uses a different `glam` version from the workspace, so the adapter should use explicit project vector types/conversions at the boundary rather than leaking third-party vector types through domain APIs.

The acceptance decision is not “did the demo move?” It is:

- can it load project-owned topology without Bevy?
- can it route through the actual hard cases?
- can it deterministically update enough active agents?
- can we diagnose failures?
- is the dependency upgrade path tolerable?

## 4.6 Bake profiles and resolution

Start with one pedestrian connectivity profile approximately matching the existing actor body radius. A tentative matrix for the spike:

| Parameter | Values to test | Reason |
|---|---|---|
| agent radius | 0.35, 0.40, 0.45 m | narrow passages versus collision safety |
| cell size | 0.12, 0.16, 0.20 m | fidelity versus bake size/time |
| step height | match player controller, plus conservative variant | stairs/curbs |
| max slope | match authored walkable surfaces | avoid false ramps |
| tile size | 32, 64, 96 m | city-scale update/query locality |

These are test values, not requirements. Record the final parameters in the asset header. A nominal 1.2 m alley cannot comfortably support two 0.4 m-radius adults side by side; navigation should preserve it as a single-file connection rather than delete it or pretend it is a broad corridor.

One mesh may be insufficient later for carts, large carried loads, children, or mobility aids. Keep `NavigationProfileId` in APIs now, even if only `Pedestrian` exists.

## 4.7 Navigation asset model

The simulation-facing representation should be stable and minimal:

```rust
pub struct NavigationAsset {
    pub version: NavigationAssetVersion,
    pub geometry_hash: GeometryHash,
    pub profiles: Vec<NavigationProfile>,
    pub regions: Vec<NavRegion>,
    pub polygons: Vec<NavPolygon>,
    pub links: Vec<NavLink>,
    pub semantic_areas: Vec<SemanticAreaRef>,
}

pub enum NavLinkKind {
    Walk,
    Stair,
    Gate,
    NarrowPassage,
    Portal,
    FutureDoor,
}
```

Links have traversal cost, directionality, clearance/profile restrictions, and optional availability conditions. A permanently open arch is a normal link. A virtual interior portal is a semantic transition. A future locked gate can toggle availability and invalidate routes through a topology revision.

Every activity spot stores or can resolve to:

- a precise interaction point;
- the containing or nearest nav polygon;
- an acceptable arrival radius/region;
- facing at the activity;
- its component/profile validation result.

Avoid snapping the actor to the exact center of a point on arrival. Define an arrival region and let reservation slots choose distinct positions.

## 4.8 Route service API

Behavior should ask for outcomes, not call a third-party library:

```rust
pub struct RouteRequest {
    pub actor: ActorId,
    pub profile: NavigationProfileId,
    pub start: NavLocation,
    pub goal: GoalRegion,
    pub policy: RoutePolicy,
    pub topology_revision: u64,
}

pub enum RouteResult {
    Found(Route),
    AlreadyAtGoal,
    StartOffMesh { nearest: Option<NavLocation> },
    GoalOffMesh { nearest: Option<NavLocation> },
    Disconnected,
    TemporarilyBlocked,
    BudgetDeferred,
}
```

`RoutePolicy` may include semantic cost modifiers such as prefer main roads, avoid a closed/dangerous area, allow portals, or choose a patrol loop. Keep these bounded and data-driven; an LLM cannot inject arbitrary cost code.

The route contains a corridor/poly sequence plus simplified steering corners and cumulative segment lengths. Cumulative length supports ETA and analytic cold progress. It records the topology revision so movement can detect a stale route after future gate changes.

## 4.9 Route budgets and caching

Dawn can make hundreds of actors want routes at once. Explicitly budget route work:

- stagger agenda start times and wake events first;
- cap new path searches per simulation tick or by measured CPU duration;
- prioritize active/interacting actors, then soon-late scheduled actors, then cold optional activities;
- leave deferred actors in a visible `AwaitingRoute`/preparation activity rather than inventing a path;
- cache routes between stable portals/areas and common work anchors;
- cache only topology-dependent corridors, not actor-specific avoidance;
- invalidate by topology/profile/policy revision.

An initial budget might be 4–8 new routes per 100 ms tick, subject to measurement. The plan should test the worst deliberate burst: all actors re-agenda after a debug time jump.

For daily departure estimates, use a coarser portal/area travel-time cache so agenda generation does not require a full actor route for every slot.

## 4.10 Route execution

Each fixed physical step:

1. materialize the actor's current corridor position;
2. advance or prune passed route corners;
3. compute a preferred velocity toward a look-ahead point;
4. limit acceleration, speed, and angular turn rate by mobility profile;
5. in the active band, run local avoidance using nearby agents and dynamic obstacles;
6. integrate displacement conservatively;
7. project/correct onto the permitted corridor or nav surface;
8. detect arrival, lack of progress, or route invalidation;
9. update authoritative position/facing/velocity;
10. emit a spatial update only if useful.

The actor should decelerate near an arrival region. Facing during a performance comes from the affordance slot; facing while walking follows smoothed horizontal velocity. Preserve the last meaningful facing while nearly stationary.

Avoid updating vertical position from an arbitrary terrain raycast in Bevy. The route/nav surface should provide height, or a deterministic simulation-side height query should. Visual feet can adjust locally within a small non-authoritative range later.

## 4.11 Speeds and mobility

Use normalized structured mobility data rather than parsing conditions every tick:

```rust
pub struct MobilityProfile {
    pub preferred_speed_mps: f32,
    pub maximum_speed_mps: f32,
    pub acceleration_mps2: f32,
    pub turn_rate_rad_s: f32,
    pub avoidance_radius_m: f32,
    pub can_use: NavigationCapabilities,
}
```

Tentative ordinary ranges:

- relaxed walking: 0.9–1.2 m/s;
- purposeful walking: 1.2–1.5 m/s;
- elderly, injured, encumbered, or young actors: authored/generated reductions;
- urgent running: deferred until animation and gameplay justify it.

Do not expose precise medical condition strings to movement logic. The population compilation step assigns a profile or modifiers. Authored overrides handle characters for whom the default would be insensitive or implausible.

## 4.12 Local avoidance and crowd behavior

Pathfinding avoids static geometry; local avoidance handles other moving bodies. Active-band neighbor queries should use a spatial index and include:

- nearby NPC agents;
- the player as a dynamic obstacle/agent;
- temporarily occupied activity slots;
- future door queues or large movable obstacles.

Goals:

- agents do not overlap or walk through the player;
- opposite flows negotiate plazas and ordinary streets;
- narrow passages become single-file naturally;
- a stopped conversation does not block an entire square;
- actors do not oscillate indefinitely at a doorway.

Avoid hard NPC-player collision as the first solution. In a first-person narrow city, several hard capsules can trap the player. Prefer soft avoidance:

- NPCs steer around and yield to the player;
- the player may gently displace an uncommitted NPC over time, with nav projection;
- performers in fixed slots can resist more but still yield under escape rules;
- never apply an instantaneous visible shove or let an NPC be pushed through a wall.

Measure the chosen avoidance library in 1.2–2 m corridors. General crowd algorithms often look good in open plazas and fail in exactly these medieval choke points.

## 4.13 Choke points, lanes, and queues

Some problems are semantic rather than geometric:

- a well has one or two usable interaction positions;
- a stall has a seller side and customer positions;
- a gatehouse may require directional lane priority;
- a lamp service point must not be occupied by a socializer;
- a doorway needs a short transition reservation so actors do not deadlock face-to-face.

Represent these through affordance slots, portal transition locks, and optional narrow-link traversal reservations. Do not globally reserve every nav polygon; that would serialize the whole crowd.

For a narrow link:

1. actors approach a waiting region;
2. a deterministic short lease grants traversal direction;
3. a small convoy may follow in the same direction;
4. the lease flips after the link clears or a fairness deadline;
5. stuck recovery can revoke it.

Only add this machinery to links that demonstrate chronic avoidance deadlock in tests.

## 4.14 Active, warm, and cold execution

### Active

Near the player, run the full fixed-step movement and local avoidance. Send spatial samples around 10 Hz. This band should include any actor in conversation/hearing interaction even if distance thresholds would otherwise demote them.

### Warm

In the broader neighborhood, route-follow at a lower neighbor-query rate. Agents can use simplified separation or ignore mutual avoidance in low density. Sample positions less frequently. They still arrive at the same semantic time within tolerance.

### Cold

Far from the player, retain route start time, cumulative distances, speed profile, and pauses. Compute progress analytically or in coarse event steps. Schedule the next event at a corner requiring special handling, portal, or arrival. Do not run all 500 agents at 10 Hz merely to increment vectors.

Cold actors still require a coherent `position_at` result for global speech/sound or sudden player movement. Promotion:

1. calculate exact analytic route position at the current physical sim instant;
2. validate it on the corridor;
3. insert into the active spatial index;
4. resolve overlaps using a bounded correction and brief easing;
5. never restart or reroll the journey.

An actor in a virtual interior is event-driven, not a cold walker.

## 4.15 Stuck detection and recovery

Stuck actors will occur. Treat recovery as a designed state machine with diagnostics:

```text
normal travel
  -> no meaningful progress for N seconds
  -> clear local avoidance target / short wait
  -> reproject to current nav polygon and replan
  -> choose alternate arrival slot or route policy
  -> return to last safe route point (only if not visible)
  -> cancel activity and select fallback
```

Track:

- distance progress over a rolling physical-time window;
- repeated steering direction reversals;
- time at the same route corner;
- route topology mismatch;
- crowd density and narrow-link ownership;
- recovery attempts and last reason.

Teleportation policy:

- never visibly teleport an active actor as routine recovery;
- a tiny projection correction can be visually eased;
- a cold/offstage actor may move to a validated portal or last-safe point only under an explicit logged recovery rule;
- repeated failure cancels the intent and records a material failure if narratively relevant;
- debug builds can pause/highlight rather than hide systematic geometry defects.

## 4.16 Virtual interior transitions

For homes and inaccessible workplaces:

1. route to a portal approach slot;
2. reserve the portal transition briefly;
3. play a short wait/door-facing transition;
4. change `SpatialPresence::Present` to `Interior { place, portal }`;
5. remove/hide the Bevy actor view and street sound emitter;
6. perform sleep/work/visit as event-driven interior activity;
7. at departure, reserve the exit, become present at a validated exit slot, and route onward.

The actor does not continue to occupy a fake coordinate behind the wall. Perception can later model sound leaking through a portal explicitly; until then, street actors cannot hear or exchange items with an interior actor.

If the player can enter a particular interior, that place can later become its own navigation region connected by the same portal abstraction rather than changing the schedule model.

## 4.17 Dynamic topology

The first milestone can use immutable street topology. Still, route APIs should carry a topology revision so future systems can handle:

- city gates closing at night;
- doors opening/locking;
- temporary market stalls narrowing a path;
- festival barriers;
- hazards, floods, fire, or construction;
- quest-based closures.

Prefer cost/availability overlays and link toggles over rebaking the whole city at runtime. Large new obstacles may require tiled rebuilds later, but should not complicate the initial implementation.

Market stalls are an important choice: if their current visual geometry blocks the player, bake a conservative permanent obstacle or export a matching runtime topology modifier. Do not allow a seller to route through their own solid stall.

## 4.18 Visual locomotion projection

Bevy derives visual state from interpolated velocity:

- `Stopped`, `Starting`, `Walking`, `Stopping`, `TurningInPlace`;
- smooth root yaw with a maximum visual turn rate;
- procedural body bob and subtle lateral sway keyed to traveled distance;
- optional capsule/body lean under acceleration;
- preserve head/attention facing separately if the actor looks at the player while torso settles;
- suppress bob while sliding due to a correction.

Key gait phase to accumulated rendered travel distance, not wall-clock animation alone, to reduce foot sliding. When full rigs arrive, the same locomotion parameters can drive blend trees.

Footstep audio:

- near-player actors only;
- surface class from nav polygon if available;
- cap concurrent footsteps globally;
- spatialize from the interpolated render position;
- no semantic event or memory per footstep;
- no footsteps for cold/offstage actors.

## 4.19 Debugging tools

Movement without spatial visualization is painful to debug. Add toggles for:

- navmesh polygons colored by connected component/area;
- obstacle footprints and clearance;
- explicit links and portals;
- actor current corridor, corners, desired velocity, actual velocity;
- avoidance radius and neighbors;
- destination arrival region and reservation slot;
- active/warm/cold band;
- stuck timer and recovery stage;
- rejected route reason;
- actor speed, ETA, lateness, and topology revision.

Headless commands should dump:

- route between two named spots/coordinates;
- nearest nav location for every actor spawn;
- disconnected anchors by component;
- the N longest common commutes;
- movement invariant failures as JSON.

## 4.20 Navigation validation gates

The full city artifact is acceptable only if:

1. Every street-present actor spawn projects within a small documented tolerance or has an intentional portal/interior assignment.
2. Every required activity and portal arrival region projects to the correct profile.
3. All major public areas expected to connect belong to the intended component, with explicit exceptions.
4. Fixed route fixtures cover every gate, bridge, narrow passage, market, and cathedral approach.
5. Sample routes never intersect exported collision prisms beyond numerical tolerance.
6. The same bake inputs produce identical hashes/artifacts in CI-supported environments, or differences are normalized and reviewed.
7. Path latency and memory fit budgets at full scale.
8. A dense crowd test can traverse representative narrow streets without chronic deadlock.
9. A 24-game-hour headless run reports no unrecovered active actor stuck longer than the threshold.
10. Rendering at 10 Hz spatial input produces no visible routine teleportation in the drive scenarios.
