# 3. World time and day/night

Day/night is both a visual system and a behavioral clock. Treating it as only a rotating sun would leave schedules unable to reason about time. Treating it as only a schedule number would make the world visually contradict its inhabitants.

## 3.1 Clock representation

Use integer calendar units to avoid long-session floating-point drift:

```rust
pub const GAME_SECONDS_PER_DAY: u32 = 86_400;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct WorldTime {
    pub day: u32,
    pub second_of_day: u32,
    pub subsecond_nanos: u32,
}

#[derive(Clone, Copy)]
pub struct GameInstant {
    pub day: u32,
    pub second_of_day: u32,
}
```

Internally, a single integer total tick count is also reasonable. The requirements are exact wraparound, easy ordering across midnight, serialization, and deterministic advancement. Display converts to hour/minute only at the edge.

The clock should start at a configured or saved value, such as day 1 at 08:00. Do not randomly offset time on every boot. Deterministic start time makes screenshots, drive tests, and behavior failures reproducible. A seeded random starting phase can remain an explicit new-game option later.

## 3.2 Calendar scale

Sea Game's full day is 720 real seconds (12 minutes). That is effective for a compact 2D play space where the visual cycle should be obvious quickly. The Cathedral-City spans roughly 1.2 × 1.0 km and uses first-person human walking speed, so copying the number literally creates severe schedule compression.

At a 1.3 m/s pedestrian speed:

| Calendar day length | Calendar scale | 250 m walk in real time | Same walk in game time |
|---:|---:|---:|---:|
| 12 real min | 120× | 3.2 min | 6.4 game hr |
| 24 real min | 60× | 3.2 min | 3.2 game hr |
| 48 real min | 30× | 3.2 min | 1.6 game hr |
| 72 real min | 20× | 3.2 min | 1.1 game hr |

Recommendation:

- default initial playtest: **48 real minutes per game day** (30× calendar scale);
- debug preset: 4 real minutes per game day;
- configuration range: allow at least 12–120 minutes and a frozen clock;
- keep physical walking at plausible real speed;
- build most assigned home/work pairs within a local ward or roughly 100–250 m;
- reserve cross-city travel for roles that can plausibly spend hours doing it;
- use virtual interiors and explicit abstract links where the player cannot observe the interior portion of a journey.

The default must be tuned through play. The architecture should not bake in 48 minutes, but it should have a documented default so schedules and lighting are tested against one consistent assumption.

## 3.3 Physical time versus calendar time

Calendar acceleration must not automatically accelerate locomotion, conversation leases, turning, animation, or sound. Those use physical simulation seconds.

Schedule planning converts route ETA into calendar time:

```text
physical ETA = route_length / expected_physical_speed
calendar travel duration = physical ETA * calendar_scale
departure time = desired arrival - calendar travel duration - buffer
```

For actors whose route is not known when the daily agenda is built, use a cached travel-time matrix between named zones/portals or a conservative Euclidean estimate. Refine the departure when a route is acquired. If the actor is already late, the policy should select among:

- arrive late at normal speed;
- choose a nearer fallback activity;
- use a valid offstage/portal shortcut;
- skip an optional activity;
- record a routine failure.

Never silently multiply visible speed to satisfy the clock.

## 3.4 Schedule windows, not exact instants

Schedules should be tolerant intervals:

```rust
pub struct ScheduleWindow {
    pub earliest_start: TimeOfDay,
    pub preferred_start: TimeOfDay,
    pub latest_start: TimeOfDay,
    pub minimum_duration: GameDuration,
    pub end: ScheduleEnd,
}
```

Daily materialization applies seeded jitter and expected travel. This prevents an entire occupation from leaving home on the same tick. It also handles interruption: a five-minute player conversation should make an actor slightly late, not invalidate their whole day.

Use overnight windows explicitly. `22:00–06:00` cannot be represented as a naive same-day ordered pair. Compile it to absolute `GameInstant`s for the current agenda day.

## 3.5 Day phases and continuous solar curves

Discrete phases are useful to behavior; continuous curves are necessary for visuals.

Initial semantic phases:

- `Night`
- `PreDawn`
- `Dawn`
- `Day`
- `Dusk`
- `Evening`

An initial fixed timetable might be:

| Phase | Start | Typical behavior signal |
|---|---:|---|
| PreDawn | 04:30 | bakers, porters, watch shift preparation |
| Dawn | 05:30 | lamps extinguish, early workers leave home |
| Day | 07:00 | markets and normal work fully active |
| Dusk | 18:00 | close work, lamplighters start rounds |
| Evening | 19:30 | taverns/social activity, night watch |
| Night | 22:00 | most residents sleep/offstage |

These are data, not hardcoded conditionals. Seasonal day length can later alter solar elevation and phase events without rewriting schedules.

Bevy receives a normalized solar sample or the clock snapshot and evaluates smooth curves:

- sun azimuth and elevation;
- directional light rotation;
- illuminance, including zero/near-zero below the horizon;
- sun color temperature across dawn/day/dusk;
- sky/atmosphere response;
- global ambient brightness and color;
- fog color/density/exposure adjustments;
- optional moon direction and low shadowless contribution;
- star visibility if a star layer is added.

Never switch from full noon lux to darkness at a phase boundary. Behavior events are discrete; visual light is continuous.

## 3.6 Coordinate convention and sun path

Before implementation, record world north/east in a small test and in code comments. The current city plan and area data use `x/z`; choose one convention and use it in:

- solar azimuth;
- maps/debug overlays;
- terms such as east gate or western ward;
- future wind/weather.

A simple initial sun model is sufficient:

1. map solar-day fraction to an azimuth sweep;
2. map daylight fraction to a sine-like elevation curve;
3. use a configurable maximum elevation;
4. place the directional light by rotation only, as appropriate for an infinite sun;
5. clamp atmosphere inputs around/below the horizon to avoid numerical artifacts.

Add reference screenshots at pre-dawn, sunrise, noon, sunset, evening, and midnight. Lighting is too subjective to validate only numerically.

## 3.7 Darkness and gameplay readability

A physically dark medieval city may be atmospheric but unplayable. Establish explicit readability goals:

- the player can navigate major streets at night using moon/sky contribution and practical lights;
- alleys remain meaningfully darker without becoming featureless black;
- faces within conversation range remain readable near an appropriate light source;
- exposure changes do not flash when crossing a phase boundary;
- daytime interior/candle lights do not flatten the scene;
- shadows from hundreds of lamp lights are not required.

Use a layered solution:

- low moon/sky ambient baseline;
- emissive lamp/candle materials visible at distance;
- actual point/spot lights only within a renderer budget around the player;
- no shadows for most street lamps;
- a small shadow-casting priority set if measurement allows it;
- exposure/tonemapping tuned at all keyframes.

The semantic state can say 120 lamps are lit while Bevy instantiates only the nearest useful light entities. Visual light culling must not change whether a lamp is considered lit for behavior.

## 3.8 Lamp affordances and lamplighter routines

Current fixtures do not provide a complete street-lamp network. Add authored/generated lamp affordances with:

- stable lamp ID;
- fixture position and service interaction point;
- district/route group;
- normal on/off phase;
- semantic lit state;
- optional fuel/maintenance fields reserved for later;
- renderer light class and priority.

At dusk, a lamplighter receives a route through a bounded group of lamps. At each lamp, the actor reserves the service spot, performs a short `LightLamp` activity, and changes semantic state. At dawn, the reverse activity extinguishes them.

Reliability rule: atmosphere must not depend on flawless path execution. Each lamp has automatic fallback deadlines:

- if not manually lit by the end of a dusk grace window, turn it on and log `lamp_fallback_lit`;
- if not extinguished by the end of a dawn grace window, turn it off and log the equivalent;
- preserve visible lamplighter behavior and diagnostics without punishing the whole scene for one stuck actor.

This is deliberate stagecraft. A later fuel economy can make failures meaningful when the rest of the game supports them.

## 3.9 Other clock-driven roles

Day/night becomes convincing when it changes who occupies the streets:

- watch shifts trade off at gates and patrol posts;
- bell ringers perform phase/hour activities;
- bakers and food provision begin before normal market hours;
- porters and servants fetch, deliver, and prepare early;
- market sellers set up, tend, and pack down;
- clergy attend recurring offices;
- tavern roles become more active in evening;
- most households transition through home portals at night;
- impoverished or unhoused characters use shelters, arcades, fires, or other appropriate spots instead of fictional private homes.

These are schedule templates plus affordances, not special per-frame systems. Bells, lamps, and opening hours emit world events that package conditions can consume.

## 3.10 Pausing and focus behavior

Recommended policy:

- world/calendar time and physical actor simulation pause while the Esc settings menu is actively open;
- they also pause when the game is deliberately suspended or loses focus, unless a future option says otherwise;
- audio/provider workers may finish and their messages may queue;
- host monotonic time continues so network timeouts remain sane;
- when unpausing, do not feed the wall-clock gap into simulation;
- a debug free-camera mode may choose to pause or run the world explicitly.

This avoids returning from settings to find that a day passed or every NPC completed an invisible route. If the existing UX intends Esc to be non-pausing multiplayer-like behavior, make that an explicit product decision and add a true pause command for tests and photo mode.

## 3.11 Fast-forward and debugging

Do not implement unrestricted high-speed visible simulation by simply increasing frame delta. Provide two concepts:

- **calendar speed**: changes the day/night and schedule clock;
- **headless catch-up**: advances a controlled simulation loop in many fixed steps without rendering.

Debug controls/configuration should support:

- set absolute day/time;
- choose 0×, 1×, debug-fast, and configured calendar scales;
- step one physical simulation tick;
- step to next schedule/phase event in headless mode;
- display phase, game time, physical tick, and pending wakes;
- force dawn/dusk lighting for art tuning without mutating actor agendas, clearly labeled as visual override.

Jumping from noon to midnight in a running semantic world needs a policy. The safe default is to rebuild agendas and reconcile actors through explicit catch-up events, not just rotate the sun while leaving everyone at work. A visual-only override is separate.

## 3.12 Clock message and renderer smoothing

The clock need not emit a message every render frame. A `WorldClockSnapshot` can carry:

```rust
pub struct WorldClockSnapshot {
    pub world_time: WorldTime,
    pub calendar_scale: f32,
    pub simulation_tick: u64,
    pub paused: bool,
}
```

Bevy extrapolates the visual clock between samples using the known scale and clamps when paused. Periodic authoritative samples remove drift. Discrete `DayPhaseChanged` events drive semantics and diagnostics; they should not be inferred independently by Bevy from a slightly extrapolated clock.

## 3.13 UI

The minimum player-facing UI is intentionally small:

- optional hour/minute display;
- day number or named day only if useful;
- no developer timing fields outside debug mode.

The debug overlay should be richer:

- game day and exact time;
- calendar scale and pause state;
- semantic phase;
- sun elevation;
- lit/total lamp count;
- actors by activity and presence;
- schedule wakes in the next game hour.

## 3.14 Save/load and time discontinuities

Persist world time and calendar configuration. On load:

- do not advance by real time spent outside the game by default;
- rebuild the next phase boundary and agenda wake indices;
- validate that every actor's current activity is still valid at the loaded time;
- discard transient visual interpolation state;
- replan transient routes if the navigation asset changed;
- prevent duplicate dawn/dusk/day-boundary events by persisting the last processed absolute event instant or deriving it from the agenda.

If an “offline passage of time” feature is ever desired, implement it as an explicit summarized simulation mode with clear economic and narrative rules. It should not emerge accidentally from comparing wall-clock timestamps.

## 3.15 Day/night acceptance criteria

1. Given a start time and delta sequence, `WorldTime` is bit-for-bit deterministic.
2. Midnight, overnight windows, pause/unpause, and large host stalls do not double-fire or skip semantic events.
3. The sun and atmosphere transition continuously through six reference times.
4. A debug four-minute day and a normal 48-minute day use the same semantic code.
5. Visible pedestrian speeds remain plausible at every calendar scale.
6. Schedule departures account for expected physical route time.
7. Lamp semantics remain correct even when a lamplighter route fails.
8. The city is navigable and visually readable at night within explicit renderer light budgets.
9. Headless mode can advance at least several days without Bevy and produce the same phase and schedule outcomes.
