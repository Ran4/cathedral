# 9. Testing, observability, and budgets

Movement bugs are often emergent: the route is individually valid, the schedule is individually valid, and the reservation is individually valid, but together 40 actors deadlock at dawn. Tests therefore need three scales:

- pure unit/property tests for exact rules;
- headless full-world simulations for emergent invariants;
- Bevy drive sessions for visual and interaction quality.

Observability is part of the feature, not cleanup. Without reason codes, route overlays, and state-transition logs, a stationary NPC is indistinguishable from an intentional wait.

## 9.1 Determinism contract

Given the same:

- normalized world/actor/behavior/nav data;
- world seed;
- start time;
- physical simulation delta sequence after fixed-step accumulation;
- external commands and typed provider results;

the simulation must produce the same:

- agenda entries;
- package/spot choices;
- intent transitions;
- semantic event ordering;
- presence/arrival outcomes;
- deterministic state hash at selected ticks.

Floating-point route/avoidance details may need tolerance across platforms. Semantic outcomes and stable candidate ordering should remain exact. If a third-party avoidance library cannot guarantee cross-platform bit identity, test bounded positional equivalence while ensuring decisions never depend on tiny unordered float differences.

Critical differential test: run the same 60 physical seconds through host frame partitions of 60×1 s, 600×100 ms, and a varied frame sequence. Once fixed steps are extracted, semantic results should match. A frame stall beyond the documented catch-up clamp should produce a known diagnostic and tested alternative result.

## 9.2 Clock unit tests

Test:

- exact advancement at configured scales;
- second/minute/hour/day rollover;
- multi-day delta arithmetic;
- overnight window compilation;
- dawn/dusk/day phase boundaries fire once;
- pause/unpause and zero delta;
- host time continuing while world time pauses;
- scale change at a known instant;
- agenda boundaries around midnight;
- load immediately before/after a boundary;
- no duplicate phase event after save/load;
- large input delta is broken into bounded steps or handled by explicit catch-up policy;
- visual clock extrapolation clamps when a new paused sample arrives.

Property checks:

- advancing by `a` then `b` equals advancing by `a+b` when neither path hits a clamp/config change;
- `second_of_day < 86_400` always;
- absolute `GameInstant` ordering is transitive;
- compiling an overnight recurring window covers the intended duration exactly.

## 9.3 Navigation bake tests

### Geometry fixtures

Build small deterministic fixtures:

- open plaza;
- L-shaped obstacle;
- narrow alley at clearance threshold;
- two disconnected courtyards;
- gate link;
- bridge crossing above another surface without false connection;
- stair/ramp link;
- portal approach;
- well/fixture exclusion;
- market stall with distinct seller/customer sides.

### Full-world validation

- source hash matches canonical geometry;
- every nav vertex/value is finite and within world bounds;
- polygon adjacency is reciprocal/valid;
- links reference valid polygons and profiles;
- expected connected components match reviewed list;
- every current actor spawn and activity anchor has valid projection or explicit exception;
- route fixtures connect/disconnect as expected;
- sampled route segments do not cross exported blocking geometry;
- no polygon exists inside water/exclusion volumes;
- clearance preserves reviewed narrow passages;
- artifact regeneration is stable.

### Query tests

- start already in goal;
- start/goal slightly off mesh with allowed projection;
- start deeply off mesh rejected;
- disconnected result;
- disabled link/topology revision invalidates route;
- route length and cumulative segment lengths consistent;
- arrival region rather than point accepted;
- route policy cost changes do not create illegal connectivity;
- queued budget ordering prioritizes active/player-interacting request.

## 9.4 Locomotion unit tests

Test on pure routes:

- actor accelerates/decelerates within profile limits;
- speed never exceeds maximum tolerance;
- facing remains finite/normalized and changes within turn-rate limits;
- actor reaches arrival region without oscillating;
- no integration step crosses static obstacle/corridor boundary;
- route corner pruning works;
- fixed-step result independent of render sample rate;
- pause freezes progress and unpause does not catch up wall time;
- attention lease decelerates and later resumes;
- route invalidation triggers replan;
- progress/lack-of-progress detector fires at correct physical duration;
- each stuck recovery stage has a bounded attempt count;
- visible actor recovery never selects teleport branch;
- cold analytic position matches fixed-step position within documented tolerance on straight and segmented routes;
- warm/cold/active transition does not change intent/arrival completion;
- portal entry removes street presence exactly once;
- portal exit uses valid reserved slot.

Property/invariant checks per fixed step:

- all vectors finite;
- spatially present actor is within nav tolerance;
- offstage actor is absent from street spatial index;
- displacement is physically bounded;
- route progress is nondecreasing except explicit replan;
- reservation references are internally consistent.

## 9.5 Avoidance/crowd fixtures

Run repeatable simulations:

- two actors crossing at 90 degrees;
- two actors passing in a broad corridor;
- opposing actors in a single-file alley;
- ten actors through a gate;
- dense plaza flows toward several distinct spots;
- stopped conversation near but not inside a route;
- stopped conversation accidentally inside a choke point;
- player walks against a crowd;
- player stands in a doorway;
- portal entry/exit contention;
- actor with slower mobility in mixed flow.

Measure overlap time, minimum separation, throughput, oscillation count, mean delay, maximum stuck duration, and player trapping. Visual review remains necessary; a solver can satisfy distance metrics while looking robotic.

## 9.6 Behavior evaluator tests

Use table-driven tests for package precedence:

- player conversation outranks work;
- required watch shift outranks optional social activity;
- extreme fatigue can invoke configured essential rest policy;
- ordinary nighttime does not override a night role;
- invalid/full destination falls through to declared fallback;
- lower-tier high utility cannot outrank valid higher tier;
- cooldown/novelty changes choice only within the same allowed tier;
- stable candidate sorting makes map iteration order irrelevant;
- schedule window tolerates lateness and short interruption;
- expired directive cannot persist;
- non-interruptible portal transition completes then reevaluates;
- missed optional agenda entry does not block later required entry;
- missing required content produces explicit safe fallback and diagnostic.

Test seeded agenda generation:

- same actor/day/seed is identical;
- different actors are jittered without violating windows;
- rerunning after unrelated actor addition does not change existing actor choices if IDs/seeds remain;
- overnight sleep windows compile correctly;
- route ETA moves departure earlier;
- capacity/shift allocator never oversubscribes hard slots;
- deterministic tie-break resolves contested reservations.

## 9.7 Affordance and reservation tests

- tag/role/opening queries return only valid candidates;
- spatial index query does not scan all spots;
- seller/customer slots remain separate;
- arrival deadline expires and releases;
- conversation suspension renews within grace only;
- cancellation/completion/portal transition release all leases;
- actor cannot own incompatible duplicate reservations;
- group reservation is atomic;
- failed group rendezvous releases every participant;
- queue fairness at a narrow link;
- save/load conflict reconciliation deterministic;
- spot removed by data migration produces fallback, not dangling reference.

Include a leak assertion after every headless day: all reservations are either held by an active matching activity/intention or intentionally persistent assignments.

## 9.8 Presence and perception tests

- street actor entering interior disappears from street stage/hearing/offer queries;
- stored last exterior coordinate is never treated as current presence;
- interior actor does not hear ordinary external sound;
- exit restores stage membership at current portal coordinate;
- moving actor crosses hearing/stage boundary on authoritative tick;
- prompt location derives from simulated position, not render interpolation;
- offer application validates current distance;
- sound origin follows actor position at utterance time;
- flying player rapidly entering a cold actor's area promotes/materializes without semantic teleport;
- actor IDs/view entities remain stable through ordinary movement and intentionally recreate through offstage presence.

## 9.9 Interaction tests

With fake cognition and controllable completion:

1. target actor while walking;
2. assert attention lease immediately;
3. hold provider response for several physical seconds;
4. assert actor remains appropriately engaged, not permanently frozen and not walking away;
5. move player in/out of range;
6. complete response;
7. expire quiet interval;
8. assert resume or re-evaluation according to schedule validity.

Additional cases:

- speech arrives during portal transition;
- schedule boundary fires while conversation active;
- pending offer during work travel;
- offer expires after player leaves;
- provider error/timeout;
- fake backend disabled/no cognition;
- cognition creates valid static-place intent;
- cognition requests unknown/disconnected target;
- intent expires during another interruption;
- older prompt result arrives after a newer structured goal revision.

## 9.10 Reflection tests

- eligibility requires dirty journal and cooldown;
- sleep-time jitter spreads requests;
- game-day request/token cap;
- rolling real-time cost cap;
- time acceleration cannot bypass caps;
- stable priority/fairness ordering;
- foreground request starts/completes while background reflection is in flight;
- invalid JSON/schema/text length/reference rejected atomically;
- stale base revision merge/reject policy;
- protected memory cannot be deleted;
- structured target must be allowed/known;
- retry consumes budget and is bounded;
- disabled/no-key mode leaves behavior untouched;
- journal retention/compaction after skipped days;
- applied request not applied again after save/load;
- ambient pressure output clamped and expires, if feature exists.

Assert that a full fake-backend routine-only day generates exactly zero reflection/cognition requests when reflection is disabled.

## 9.11 Spatial protocol/projection tests

- semantic and spatial revisions advance independently;
- out-of-order/duplicate spatial batches ignored;
- presence transition resets interpolation;
- stationary actor omitted from delta remains stationary;
- interpolation stays within previous/current segment and does not overshoot walls at corners;
- extrapolation duration clamps;
- a semantic snapshot does not snap a moving render view back to an old position;
- actor despawn/reappear clears stale samples;
- mirror rebuild after full snapshot preserves newest valid spatial sample by tick;
- movement does not trigger full snapshot at 10 Hz;
- bounded allocations/message size under all-cast movement.

A renderer test scene can feed an artificial right-angle route at 2, 5, and 10 Hz and compare captured root paths/maximum visual correction.

## 9.12 Full-world headless scenarios

Extend `cathedral-headless` with deterministic simulation controls. Suggested scenarios:

### One ordinary day

Start 04:00, run through 04:00 next day. Report actors by presence/activity every game hour, late/missed agenda entries, routes, stuck recovery, reservations, and phase events.

### Multi-day soak

Run 7–30 game days in fake/no-cognition mode. Assert no unbounded queue/journal/reservation growth, no actor permanently lost, and stable performance.

### Dawn burst

Start just before major commute window. Measure decision wakes, route queue depth/latency, and arrival distribution.

### Dusk/night roles

Run lamps, watch shifts, market close, tavern increase, and household transitions. Inject one lamplighter route failure and verify fallback.

### Time-scale matrix

Run 12, 24, 48, 72 real-minute equivalent scales with identical physical duration and compare lateness/commute effects. Use results to tune the default.

### No backend

All cognition/STT/TTS unavailable; movement and city routine invariants remain green.

### Topology failure

Disable a gate link or feed a stale nav hash in a fixture. Verify clear fallback/diagnostics and no straight-line movement.

### Frame partition

Drive same fixed duration with different host delta partitions and compare semantic state hash.

Headless transcript output should remain readable; detailed movement goes to structured diagnostics or a requested verbose actor trace.

## 9.13 Bevy drive scenarios

Use the project's documented drive mode and session evidence conventions.

### A. Follow a worker

- begin before departure;
- observe portal exit;
- follow across at least two streets/doglegs;
- observe arrival and performance;
- capture positions/activity transitions in logs and screenshots.

### B. Interrupt a commute

- speak to a walking actor;
- verify deceleration, facing, speech origin, and resume;
- move away during reply and verify range behavior.

### C. Dense market arrival

- observe dawn arrivals at a square;
- check distinct slots, no central pile, and stable frame pacing;
- stand in the route and verify soft yielding/no player trap.

### D. Dusk lamplighter

- follow several lamp service actions;
- verify emissive and practical light transitions;
- force/observe fallback deadline in a test configuration.

### E. Night watch and population

- traverse several wards at night;
- see mostly offstage/home population, active watch/tavern roles, readable streets;
- verify no mass visible sleeping at façades.

### F. Rapid stage transition

- fly from one end of city toward cold actors;
- verify materialization/interpolation without obvious teleport or wrong activity;
- speak immediately after arrival.

### G. Pause/menu

- open settings across a phase boundary duration;
- return to unchanged world time/positions under chosen pause policy;
- verify provider completion does not corrupt leases.

### H. Long roam

- play/fly for at least one full configured day;
- collect session log stuck, route, contention, snapshot, light, and sim-cost summaries;
- inspect the worst reported actors rather than only the main route.

## 9.14 Structured logging

Add state-transition events, not per-step spam:

```json
{
  "source": "engine",
  "kind": "behavior_transition",
  "simulation_tick": 48120,
  "world_day": 3,
  "world_time": "05:47:20",
  "actor_id": "actor:ilse",
  "from": "interior_sleep",
  "to": "travel_work",
  "intent": "work",
  "reason": "agenda_window",
  "destination": "spot:wickmarket:stall_014:seller"
}
```

Event families:

- `world_phase_changed`, `world_day_changed`, `simulation_lagged`;
- `agenda_generated`, `package_selected`, `intent_interrupted/resumed/completed/failed`;
- `reservation_acquired/released/expired/contended`;
- `route_queued/found/failed/invalidated`;
- `actor_stuck`, `stuck_recovery_stage`, `actor_unstuck`;
- `portal_entered/exited`;
- `lamp_changed`, `lamp_fallback`;
- `reflection_queued/skipped/completed/rejected/applied`;
- periodic aggregated `living_city_metrics`.

Rate controls:

- state transitions for all actors;
- detailed route/movement trace only for selected actor IDs or on failure;
- aggregate counters every few physical seconds/game hour;
- suppress repeated identical errors with counts;
- always retain first occurrence and final recovery result.

Logs should include stable IDs and reason codes; names can be added for readability but not replace IDs.

## 9.15 Metrics

Track at minimum:

### Behavior

- actors by public activity/presence/LOD band;
- agenda entries completed/late/missed by role;
- decisions per tick and wake queue depth;
- destination query candidate counts;
- reservation contention/fallbacks;
- average activity repetition score.

### Navigation

- route requests, success/failure/deferred;
- route queue depth and latency percentiles;
- path length/ETA by role;
- stuck events/recovery stages/unrecovered duration;
- active neighbor-query counts;
- actor projection corrections;
- cold promotion corrections.

### Projection/performance

- changed actors per spatial batch;
- batch bytes/allocations and semantic snapshot rate;
- sim fixed-step duration average/p95/p99;
- route work duration;
- view reconciliation/interpolation duration;
- active actual lights and footsteps;
- game frame-time impact.

### Cognition

- foreground/background requests and latency independently;
- reflection eligible/queued/skipped reasons;
- input/output tokens and estimated cost by request kind;
- budget remaining;
- invalid/stale result count;
- journal dirty/size distribution.

## 9.16 Initial budgets

These are starting guardrails to replace with measured project baselines, not immutable requirements:

| Resource | Initial guardrail |
|---|---|
| physical sim step | 10 Hz authoritative movement |
| behavior fallback evaluation | staggered, usually no faster than every 2–10 game min per actor |
| new routes | 4–8 per physical tick, priority queued |
| active spatial output | up to 10 Hz for changed nearby actors |
| full semantic snapshots due solely to motion | zero |
| movement/behavior CPU on reference machine | target average under 1.5 ms per rendered frame equivalent, p99 under 4 ms after full rollout |
| visible route correction | no routine snap; small correction eased |
| unrecovered active stuck time | zero beyond defined failure/replan window |
| actual nearby practical lights | begin with 24–48, tune by GPU measurement |
| concurrent NPC footstep voices | begin with 8–12 |
| LLM requests caused by routine movement | exactly zero |
| reflection | default off; when enabled, e.g. 3–5 calls/game day plus token/USD caps |

CPU targets require a named reference scene/machine and baseline. CI may use regression ratios if hardware is variable. A performance failure should identify which budget—decision, route, avoidance, output, render lights—was exceeded.

## 9.17 Population-distribution checks

Headless hourly histograms are powerful content tests. Establish broad reviewed ranges after the cohort/full-cast data exists, for example:

- late night: large majority of non-night roles in residence/shelter/interior;
- pre-dawn: bakers/porters/watch transitions visible but no all-cast commute;
- market hours: assigned sellers at work and customers distributed across capacity;
- dusk: work close and home/social flow spread over a window;
- night: watch/tavern/lamplighter roles remain represented.

Do not hardcode arbitrary percentages before seeing the authored population. Store expected bands in test data so intentional schedule revisions produce reviewable updates.

## 9.18 Failure triage checklist

When an NPC is “not moving,” the inspector/log should answer in order:

1. Is the actor spatially present or inside/absent?
2. What is the active activity and intent?
3. Why did the package evaluator select it?
4. When is the next wake?
5. Does it have/reserve a destination slot?
6. Is a route queued, found, or failed?
7. Is the start/goal on nav and same component?
8. Is an attention/portal/narrow-link lease suspending it?
9. Is it intentionally waiting or performing?
10. Is progress below stuck threshold and what recovery stage is active?
11. Is sim paused or the actor cold/offstage?
12. Is movement disabled by config/stale nav data?

If tools cannot answer these without attaching a debugger, the observability milestone is incomplete.

## 9.19 Final quality gate

Before enabling full movement by default:

- all pure/unit/integration tests pass;
- full navigation validation is clean or every exception reviewed;
- 7-day headless no-backend soak passes invariants;
- dawn/dusk burst metrics stay within budgets;
- full-cast deterministic state hashes match repeated runs;
- drive scenarios A–H have recent session evidence;
- no visible systematic teleporting, wall crossing, crowd pile, or player trapping;
- clock lighting has approved reference captures;
- provider request counts prove routine movement cost is zero;
- reflection remains independently disableable and foreground-safe;
- config defaults, authoring workflow, and recovery diagnostics are documented.
