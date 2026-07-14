# 8. Delivery sequence

The feature should ship as vertical slices that remain testable in headless mode. Avoid a long branch that adds a clock, navmesh, data compiler, crowd system, and 500 schedules before anything moves on screen.

The critical dependency graph is:

```text
current-state metrics
       |
       +------> world clock ------> schedules ------> day/night roles
       |
       +------> canonical geometry -> nav bake -> route service
                                             |           |
                                             +-----------+
                                                     |
                                              one-NPC slice
                                                     |
                                     affordances + package evaluator
                                                     |
                                      cast rollout + simulation LOD
                                                     |
                                  lamps / patrol / virtual interiors
                                                     |
                              optional reflection and richer directives
```

Clock and navigation research can proceed independently, but the first real routine needs both.

## 8.1 Milestone 0: baselines and decision spikes

### Purpose

Create measurements and resolve navigation dependencies before domain types harden around an unsuitable library.

### Work

- Add a read-only headless/report tool for current actor spawn, road, building, collision/exportable geometry, and area statistics.
- Record current game frame time, actor projection cost, snapshot frequency/size, and headless fake-backend throughput.
- Define the canonical coordinate convention and representative movement test district.
- Export the representative district into `NavigationBakeInput` without changing gameplay.
- Spike `rerecast` (or alternative bake) plus engine-agnostic `landmass` consumption.
- Build a minimal project-owned `NavigationQuery` interface and a trivial fake implementation for tests.
- Compare library adapter versus small polygon A*/corridor code on the fixed route matrix.
- Record dependency versions, duplicate `glam` cost/conversion boundary, deterministic output, path latency, and narrow-alley results.
- Decide the first pedestrian radius/resolution/tile parameters from evidence.

### Deliverables

- short ADR under this folder or project ADR location selecting the navigation stack;
- fixed route fixture data and expected reachability;
- overhead connected-component visualization;
- baseline JSON committed or attached according to repository norms;
- no gameplay movement yet.

### Exit gate

The spike routes through a square, narrow alley, passage, and gate in pure/headless code; reports disconnected and off-mesh cases; and has a credible path to full-city bake. If no candidate passes, stop and improve geometry/export rather than building behavior on straight-line movement.

## 8.2 Milestone 1: explicit world clock

### Purpose

Introduce deterministic calendar semantics without coupling them to movement or provider timing.

### Likely code areas

- `crates/cathedral-sim/src/time.rs` (new);
- engine poll/input plumbing;
- world snapshot/message types;
- `src/smart_actors/local_engine.rs` physical delta input;
- mirror resource for clock state;
- `src/scene.rs` or a new day/night scene module;
- `config.ron` schema/defaults;
- headless CLI arguments.

### Work

- Add `WorldTime`, `GameInstant`, exact advancement, day-phase events, and fixed-step simulation accumulator.
- Preserve host monotonic `now` semantics.
- Add pause command/state and menu/focus policy.
- Add `WorldClockSnapshot` and renderer extrapolation.
- Add config for start time and real minutes per day.
- Add headless flags such as `--start-time`, `--real-minutes-per-day`, `--sim-seconds`, and `--game-days`.
- Update sun direction/color/illuminance, atmosphere, ambient, fog, and a minimal HUD/debug clock.
- Capture reference lighting screenshots at six times.

### Feature flag

`simulation.clock.enabled`, default on only after visual/reference approval. A visual freeze/noon option remains for screenshot comparison.

### Exit gate

- pure clock boundary tests pass;
- provider/floor timing tests remain unchanged;
- headless and Bevy agree after the same deltas;
- pause does not catch up wall time;
- the debug four-minute cycle and normal cycle transition continuously;
- no NPC behavior changes yet.

## 8.3 Milestone 2: canonical geometry and full navigation artifact

### Purpose

Make routing agree with the actual world before actors depend on it.

### Likely code areas

- shared city/world geometry types, likely under `src/city` initially or a small non-Bevy crate if needed by tools;
- controller collision construction;
- city/cathedral bespoke geometry exporters;
- new tooling crate/binary;
- versioned navigation asset loader in `cathedral-sim` or a data crate;
- debug render plugin.

### Work

- Define `WorldGeometry`/`NavigationBakeInput` and stable source IDs.
- Refactor collision registration enough to export every movement-relevant obstacle and surface.
- Generate full-city tiled navigation artifact.
- Add explicit links for gates, passages, stairs/bridges, and initial portals.
- Generate connected-component and clearance reports.
- Sample all 500 spawns and all preliminary activity anchors.
- Add startup source-hash validation.
- Add Bevy nav overlay and headless route query command.
- Add fixed regression routes through every distinct topology feature.

### Feature flag

Navigation loads and visualizes, but actors remain stationary.

### Exit gate

- required public city surface is connected as intended;
- all actor street spawns either validate or receive an explicit interior/portal correction;
- regression paths avoid collision geometry;
- artifact regeneration is deterministic/reviewable;
- startup catches stale data;
- full asset memory and route latency fit preliminary budgets.

## 8.4 Milestone 3: one-actor end-to-end locomotion

### Purpose

Prove simulation authority, spatial messaging, interpolation, interaction stop/resume, and headless parity before building schedules.

### Work in `cathedral-sim`

- Add movement fixed-step loop and simulation tick.
- Add present/interior-compatible `SpatialPresence` shape, initially using only present.
- Add route request queue/budget and `TravelActivity`.
- Add kinematics, route following, facing, arrival, and stuck diagnostics.
- Separate semantic and spatial revisions/messages.
- Ensure perception/range/stage queries use current simulated actor position.
- Remove the assumption that only the player can move from the appropriate engine path; keep external spatial commands restricted to host-owned actors.
- Add an internal/debug command to assign `TravelToSpot` to an allowlisted NPC.

### Work in Bevy

- Extend `WorldMirror`/projection protocol.
- Replace exact per-snapshot NPC transform assignment with buffered spatial interpolation.
- Hide/remove view on future presence transition shape.
- Derive simple start/walk/stop visual gait and smooth facing.
- Add route/velocity/debug overlay.
- Treat player as a soft dynamic avoidance body in the active band.

### Work in headless

- command one actor to a named spot;
- print route, position samples, arrival, and invariant result;
- ensure exact same final semantic state under different host-frame delta partitions.

### Allowlist

Select one minor or test-only actor in a representative, well-understood area. Do not choose a major story actor for the first slice. Add 2–3 destinations and a loop command.

### Exit gate

- actor follows nav around obstacles in Bevy and headless;
- render path is smooth at 10 Hz samples;
- hearing/stage/offer distance follows new position;
- targeted speech decelerates/stops/faces and later resumes without provider dependency;
- debug stall creates recovery rather than visible teleport;
- no full semantic snapshot per movement tick;
- stationary feature-disabled path remains unchanged.

## 8.5 Milestone 4: affordance and behavior vertical slice

### Purpose

Replace debug destinations with believable code-driven daily lives for a small reviewed cohort.

### Cohort

Use roughly 12–20 actors across distinct roles and geometry:

- market seller;
- porter;
- domestic servant;
- day/night watch;
- lamplighter (semantic route can precede final lights);
- clergy/bell ringer;
- tavern worker;
- laborer/unemployed actor;
- actor with limited mobility;
- household pair;
- one no-fixed-residence/shelter policy.

### Work

- Add stable spot, portal, schedule, role, mobility, and reservation schemas.
- Generate/author the cohort's homes, work sites, public spots, and portal approaches.
- Implement daily agenda materialization and stable jitter.
- Implement ordered package evaluator, wake queue, destination scoring, reservations, and basic failure chains.
- Implement activities: sleep/interior, work/perform, travel, wait, local sandbox, patrol.
- Implement virtual interior entry/exit and renderer presence changes.
- Add fatigue only if schedule/sleep needs it; avoid broader needs.
- Add behavior diagnostics and actor inspector.
- Run multi-day headless fake/no-cognition tests.

### Exit gate

- each cohort actor completes a plausible 24-hour routine;
- routines remain intact with all LLM providers disabled;
- spots do not overfill;
- no actor sleeps at a placeholder street coordinate;
- interaction interruption works in every activity category;
- same seed produces identical agenda/choice hash;
- reviewers can inspect why each actor selected its activity.

## 8.6 Milestone 5: content compiler and cast rollout

### Purpose

Scale reviewed systems from the cohort to all 500 actors without synchronized bursts or data guessing.

### Work

- Add reviewed occupation-to-role and building-use mappings.
- Generate building portals, households/residences, workplace assignments, public affordances, and schedule templates.
- Add hand-authored override file for major/story actors.
- Implement assignment capacity and commute reports.
- Add active/warm/cold execution, route cache, analytic cold progress, and band promotion.
- Add spatial message sampling by band.
- Tune route and decision budgets under dawn/time-jump bursts.
- Add destination crowding score and any demonstrated narrow-link arbitration.
- Validate all actors, anchors, patrols, and fallbacks.

### Rollout ladder

Enable by deterministic percentage or actor allowlist:

1. cohort only;
2. one ward;
3. 25% of cast;
4. 50%;
5. all minor/ambient;
6. all actors after major overrides reviewed.

At each rung, compare performance, stuck rate, commute distributions, snapshot volume, and population density by phase.

### Exit gate

- all 500 actors have explicit residence/night policy, mobility, schedule, and fallback;
- full-day headless run satisfies invariants and determinism hash;
- active actor movement remains smooth during worst crowd concentrations;
- path/decision/snapshot budgets prevent frame spikes;
- no chronic destination piles or component failures;
- major character assignments have been reviewed.

## 8.7 Milestone 6: day/night city roles and practical lighting

### Purpose

Make the clock materially change the streets and complete the visible daily cycle.

### Work

- Generate/author lamp fixtures, service spots, route groups, and renderer priorities.
- Implement lamplighter dusk/dawn circuits and automatic grace fallback.
- Implement watch shift/muster/patrol handoff.
- Add bell phase/hour activities and distance-budgeted bell audio.
- Tune early work, market setup/close, tavern evening, clergy offices, and sleep dispersal.
- Add emissive lamp state and nearby actual light pooling/culling.
- Tune moon/ambient/readability at night.
- Add six drive scenarios and reference screenshots/log assertions.

### Exit gate

- population/activity histograms differ plausibly across phases;
- lamps transition even when individual routes fail;
- lamp/light budget remains bounded;
- night navigation is readable but distinct;
- no dawn/dusk synchronized CPU or route spike;
- watch, market, tavern, and household routines are visibly recognizable.

## 8.8 Milestone 7: richer interaction and structured directives

### Purpose

Allow conversations to influence high-level behavior without exposing pathfinding to the LLM.

### Work

- Include activity/time/destination context in prompts.
- Add validated `adopt_intent` action with a very small target vocabulary.
- Add intent results/failures to later prompt context.
- Harden offer/handoff dual leases.
- Add optional static-place rendezvous only after capacity/wait timeouts work.
- Add material experience journal with debug summaries, no live reflection required.
- Exercise slow/live/fake provider while actors move and schedules transition.

### Exit gate

- player can ask an actor to visit/wait at a known supported place;
- invalid/unknown/unreachable targets fail without world corruption;
- directive is interruptible/expiring and does not permanently replace routine;
- provider latency cannot strand movement state;
- journals contain material events, not route spam.

## 8.9 Milestone 8: optional reflection lane

### Purpose

Add slow personal adaptation only after normal life is already convincing.

### Work

- Define typed `ReflectionRequest`/`ReflectionResult` and validation.
- Add separate non-blocking backend lane or proven cancellation.
- Implement eligibility, dirty scores, sleep-time spread, budgets, and cost ledger.
- Implement deterministic fake results and stale revision tests.
- Apply bounded memory/narrative-goal/structured-motive updates atomically.
- Archive prompt kind, cost, validation, and application metadata.
- Evaluate one optional aggregate city-pressure prompt behind a separate flag only if code-generated pressures prove useful and a live model demonstrably improves them.

### Default

Disabled or extremely conservative until cost/value evidence is reviewed.

### Exit gate

- reflection never delays player-facing cognition;
- hard request/token/USD budgets hold under accelerated days;
- invalid output has no partial effect;
- no-key behavior is unchanged;
- reviewed prompt samples show meaningful changes rather than generic diary prose.

## 8.10 Milestone 9: persistence and hardening

If a general save system lands earlier, persistence work moves into each milestone. Otherwise, before declaring the living city complete:

- serialize/migrate world time, actor presence/activity/agenda/needs/directives/journal and deterministic counters;
- replan routes on load and reconcile reservations;
- handle nav/world asset hash changes;
- add long-running headless soak tests across many days;
- add session-log dashboards/report scripts for stuck, fallback, contention, and budget patterns;
- remove obsolete rollout flags and debug shortcuts;
- document content author workflow.

### Exit gate

A save made during travel, sleep interior, conversation suspension, and lamp route loads plausibly; no duplicated day-boundary/reflection events occur; asset migration failures are explicit.

## 8.11 Suggested pull-request boundaries

Keep review scopes coherent:

1. clock domain and tests;
2. clock projection/daylight visual changes;
3. geometry export and nav tool/asset;
4. pure route API and regression fixtures;
5. spatial protocol and one-actor locomotion;
6. Bevy interpolation/gait/debug overlay;
7. attention lease and moving interaction integration;
8. affordance/reservation/portal primitives;
9. package/agenda engine and cohort data;
10. assignment compiler and full cast data;
11. LOD/performance rollout;
12. lamps/night roles;
13. structured cognition intents/journal;
14. optional reflection lane.

Some boundaries can merge if changes are small, but avoid combining generated full-cast data with core movement logic in one review.

## 8.12 Feature-disabled and fallback behavior

During rollout:

- `movement.enabled = false`: actors remain at authored spawn, current behavior preserved;
- clock enabled, behavior disabled: lighting cycles, actors remain stationary;
- behavior enabled, movement disabled (debug only): decisions/agenda visible but travel does not execute; never a normal player mode;
- nav asset invalid: development startup fails; release logs prominently and disables movement rather than straight-line fallback;
- no cognition backend: routine life unchanged;
- reflection unavailable: journals remain/compact under retention policy;
- one actor repeatedly stuck: cancel/fallback that actor, do not disable entire simulation.

Straight-line movement through geometry is not an acceptable fallback.

## 8.13 Change discipline in the current architecture

Likely high-risk edits deserve focused review:

- removing the engine's “only player moves” assumption without allowing Bevy to author NPC positions;
- separating spatial and semantic revisions without losing snapshot reconciliation;
- making offstage presence explicit in perception and view reconciliation;
- introducing fixed simulation delta without changing provider `now` behavior;
- exporting collision geometry without subtly changing player collision;
- adding dependencies with different `glam`/Bevy versions;
- changing prompt format only when activity context is actually available.

For each, add a regression test before or in the same change.

## 8.14 What not to implement first

Avoid these tempting shortcuts:

- random destination wandering for all 500 before navigation validation;
- using renderer transforms as authoritative and sending them back at 10 Hz;
- a full city nav bake sourced only from road polylines;
- parsing prose goals/occupations every decision;
- per-frame behavior rolls;
- one point destination without slots/capacity;
- hard NPC capsules that can trap the player;
- turning on hundreds of shadow-casting lights;
- daily LLM calls before ordinary schedules are useful;
- copying a 12-minute day without commute measurement;
- hiding stuck actors with visible teleportation;
- making every routine event a memory or prompt entry.

Each can create an impressive early demo while locking the system into the wrong authority or cost model.

## 8.15 Completion definition

The overall effort is complete when:

- time, navigation, behavior, and motion run authoritatively in headless simulation;
- Bevy visually projects the same state smoothly;
- all actors have validated routines and destinations;
- interaction remains correct while actors move;
- day/night visibly and behaviorally changes the city;
- performance and cognition costs obey explicit budgets;
- multi-day tests and drive sessions do not reveal systematic stuck, piling, synchronization, or state-loss failures;
- optional LLM layers enrich goals without becoming infrastructure.
