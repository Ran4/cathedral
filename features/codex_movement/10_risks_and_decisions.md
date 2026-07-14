# 10. Risks, alternatives, and decisions

This chapter separates firm architectural recommendations from values that need a spike or playtest. It is intended to prevent implementation convenience from silently deciding product behavior.

## 10.1 Decision register

| ID | Decision | Status | Rationale / validation |
|---|---|---|---|
| D1 | `cathedral-sim` owns NPC movement and presence | Recommended firm | Required for headless authority, perception, offers, and deterministic behavior. |
| D2 | Bevy interpolates spatial deltas and owns visual gait only | Recommended firm | Avoids snapshot snapping and renderer-as-simulation split brain. |
| D3 | Host monotonic, physical simulation, and world calendar time are distinct | Recommended firm | Provider timeouts, plausible speed, pause, and accelerated days have different semantics. |
| D4 | Routine behavior uses ordered packages with bounded scoring inside a tier | Recommended firm | Predictable obligations plus variation; avoids global utility surprises. |
| D5 | Existing prose goals are not parsed as executable movement | Recommended firm | Typed intent is validateable; prose is not. |
| D6 | Road polylines are semantic metadata, not the sole nav graph | Evidence-backed firm | Current topology/spawn-distance diagnostics show insufficient connectivity/coverage. |
| D7 | Navigation is baked offline from canonical walkable/obstacle geometry | Recommended, spike details | Aligns collision/nav, avoids startup cost; exact bake tool needs proof. |
| D8 | Use project-owned nav interfaces; spike `landmass` core + Recast-family bake | Pending Milestone 0 | Best current fit for pure sim, but dependency/determinism/narrow-city behavior must pass. |
| D9 | Begin with one pedestrian profile | Recommended initial | Simpler validation; add profiles only for demonstrated connectivity differences. |
| D10 | Unavailable buildings use explicit virtual interiors/portals | Recommended firm for first full routine | More honest and scalable than sleeping at façades or fake coordinates. |
| D11 | Start time is deterministic/configured, not randomly phased on each boot | Recommended firm | Reproducible sessions/tests; random new-game option can be explicit. |
| D12 | Initial normal day length is 48 real minutes; debug cycle is separate | Playtest recommendation | A 30× calendar is less hostile to commutes than Sea Game's exact 12 min, while still visible in a session. |
| D13 | Physical walking speed does not scale with calendar speed | Recommended firm | Prevents visible sprinting; schedules/local assignments account for travel. |
| D14 | NPC-player interaction creates a movement attention lease before LLM completion | Recommended firm | Provider latency cannot make target walk away. |
| D15 | Full semantic revisions are separate from spatial updates | Recommended firm | 500 moving actors make full snapshot-per-step wasteful. |
| D16 | Active/warm/cold LOD changes execution detail, not choices/outcomes | Recommended firm | Performance without distance-dependent personality. |
| D17 | Destination spots have capacity and reservations | Recommended firm | Prevents point piles and makes places meaningful. |
| D18 | Initial needs are minimal | Recommended firm | Avoids unsupported survival pantomime; schedules/roles create more value first. |
| D19 | Routine movement emits no LLM calls or per-step memories | Recommended firm | Cost, prompt noise, and architecture. |
| D20 | Reflection is dirty/event-driven, globally capped, and never shares a blocking foreground lane | Recommended firm | Preserves responsiveness and cost control. |
| D21 | Ambient cast has no individual daily reflection | Recommended firm | 350 calls/day is unjustifiable; aggregate pressure is optional experiment. |
| D22 | Lamps have visible service routines plus automatic grace fallback | Recommended firm | Role remains meaningful; navigation failure cannot black out city. |
| D23 | NPCs are soft obstacles to the player initially | Playtest recommendation | Avoid player trapping in narrow streets while retaining physical presence. |
| D24 | Dynamic topology is deferred but route revisions/links are designed now | Recommended | Gates/doors will need it; full runtime rebake need not block first slice. |

## 10.2 Why not Bevy-owned movement?

An ECS movement plugin would be convenient for transforms, spatial queries, and nav integration. It is the wrong authority here because:

- the pure sim would still see stationary actors unless positions were fed back;
- ordering between Bevy frames, engine polling, speech, and prompt snapshots would become semantic;
- headless runs would need a second movement implementation or Bevy dependency;
- rendered interpolation/correction could leak into gameplay;
- save/replay would inherit ECS handles and frame time.

It is fine for Bevy to host debug visualization or adapt a library during the spike. The final route/movement result must be available to `cathedral-sim` without a Bevy world.

## 10.3 Why not direct straight-line walking first?

Straight-line movement might prove the snapshot protocol quickly, but should be confined to a tiny obstacle-free unit fixture. Enabling it in the real city would:

- walk through walls and stalls;
- create misleading behavior data built around impossible destinations;
- force later handling of actors stranded inside geometry;
- produce visual regressions that obscure projection work.

The one-actor vertical slice should wait for at least a representative validated nav region. Protocol tests can use a fake route independent of world geometry.

## 10.4 Why not use only a road graph?

A centerline graph is inexpensive and readable, but the measured road data is sparse/disconnected relative to actor spawns and open areas. Expanding it into lanes and arbitrary connectors becomes a custom navmesh by another name, without reliable collision agreement.

Road graph metadata remains valuable for:

- named-route narration;
- preference for major streets;
- high-level portal/area travel-time estimates;
- patrol authoring;
- debugging.

It should annotate, not define, all walkability.

## 10.5 Why packages instead of a full planner?

A general GOAP/HTN planner can create multi-step plans but requires a complete, consistent action model with preconditions/effects. The current world lacks many systemic effects—production, hunger, doors, transactions, interior objects. A planner would either fail often or invent shallow tokens for unsupported actions.

Ordered packages fit visible daily life:

- schedule says work;
- affordance resolves a work spot;
- route reaches it;
- performance lasts until a condition/end;
- interruptions have explicit policy.

Structured motives and directives can later invoke small task graphs (`deliver A to B`, `meet at place`) without replacing the routine evaluator. If gameplay grows a robust action/effect model, an HTN layer can occupy one package tier.

## 10.6 Why not pure global utility AI?

Global utility is flexible but hard to debug and tune at population scale. A tiny weight change can make every actor prefer a nearby tavern over work. Float ties and candidate sets can change behavior unexpectedly.

Bounded scoring is still useful for:

- which valid tavern;
- which idle marker;
- whether to socialize or observe in an optional window;
- which route policy among near-equal paths.

Hard tier order protects obligations and interaction. Diagnostics can say “scheduled work tier won; stall 14 scored highest among three work spots.”

## 10.7 The time-scale dilemma

There is no perfect accelerated clock in a physically walkable kilometer-scale city.

### Very short day (12 minutes)

Pros:

- player sees a full cycle quickly;
- day/night roles change during short tests;
- directly resembles Sea Game's pacing.

Cons:

- even a local walk consumes hours of schedule time;
- frequent dawn/dusk route bursts;
- sleep/work phases are very brief in real time;
- conversations can consume large fractions of a shift;

### Longer day (48–72 minutes)

Pros:

- commutes and interruptions fit schedules better;
- phases feel less like rapid weather;
- crowd transitions spread naturally.

Cons:

- a normal play session may not show a full cycle;
- lighting/role features are less immediately visible;
- testing needs time controls/headless catch-up.

Recommendation: retain Sea Game's clear phase curve and role coupling, but start at 48 minutes plus a separate 4-minute debug preset. Playtest at 24/48/72 after real home/work assignment. Do not decide from visual preference alone; compare lateness, missed shifts, and how often the player observes a transition.

An alternative is player wait/sleep fast-forward. That should eventually advance the headless sim in controlled fixed steps or summarized offstage mode, not run visible actors at 30× speed.

## 10.8 Virtual interiors tradeoff

Pros:

- plausible sleeping/indoor work without building hundreds of rooms;
- reduces visible/rendered population at night;
- clean street perception semantics;
- future real interiors can attach to the same portals;
- offstage behavior is cheap.

Cons:

- actors visibly disappear at a doorway that may not animate/open;
- player cannot follow them;
- shared interior conversation and sound are deferred;
- portal capacity can create queues;
- every actor needs a credible portal assignment.

Alternatives are worse for the first full city:

- leave sleeping actors motionless outside homes;
- keep them present behind solid walls at fake coordinates;
- never let anyone go home;
- build all interiors before movement.

Mitigate the disappearance with a short facing/door transition, solid façade-aligned portal placement, and future door animation. UI/debug should clearly report interior presence.

## 10.9 Navigation dependency risk

Risks:

- third-party crate version drifts from Bevy/workspace `glam`;
- avoidance is nondeterministic or poor in narrow corridors;
- Recast bake output varies across platforms;
- memory footprint is too high at required cell resolution;
- elevation/bridges create false connections;
- library wants ECS ownership or runtime mesh input.

Mitigation:

- project-owned domain API and asset schema;
- dependency types confined to adapters;
- representative spike before integration;
- checked-in artifact and normalized hash/report;
- custom fixed route matrix;
- ability to keep baked polygons while replacing steering/avoidance;
- no behavior content stores opaque third-party polygon handles without stable remapping.

Trigger to abandon candidate: cannot represent/debug the representative topology, cannot run headlessly, chronic narrow-alley deadlock, or unreasonable asset/query cost.

## 10.10 Canonical geometry refactor risk

Collision/render generation is already complex. Refactoring it can accidentally change the player world.

Mitigation:

- begin with an export layer around existing registered collision primitives;
- snapshot counts/bounds/hashes before and after;
- add collision regression fixtures and drive routes;
- refactor one geometry family at a time;
- preserve unrelated visual generation;
- compare nav overlay with collision debug overlay;
- require source IDs for unmatched/bespoke shapes.

Do not bake solely from rendered triangles; decorative detail can make surfaces unwalkable and does not necessarily match controller collision.

## 10.11 Population authoring risk

Automatically assigning 500 homes and jobs can create culturally/story-inappropriate outcomes or noisy diffs.

Mitigation:

- deterministic rules with explicit reports;
- ward, occupation, family, status, and authored institution constraints;
- never infer story facts from vague prose at runtime;
- protect major-character override file;
- review longest commutes, fallbacks, and major assignments;
- preserve assignments across regeneration where inputs unchanged;
- represent no-fixed-residence and institutions explicitly rather than “fixing” everyone into a house.

Generated content should be treated as content requiring review, not an unquestioned truth.

## 10.12 Crowd and player-trapping risk

The city intentionally has pinched 4.6 m routes, narrower alleys, projections, and passages. Hard collision among many capsules can make the player unable to move.

Mitigation:

- active local avoidance treats player as high-priority body;
- NPCs yield and choose side before contact;
- soft push escape under sustained player motion;
- activity spots stay outside traffic corridors;
- cap density/capacity in small areas;
- narrow-link queue/lease only when demonstrated;
- explicit “player stationary in doorway” fixture;
- despawn/offstage is never used as an immediate collision cheat near camera.

Playtest third-person debug overhead even though the game is first-person; flow problems are easier to see.

## 10.13 Snapshot/projection risk

Separating spatial and semantic messages introduces ordering complexity:

- old semantic snapshot could contain an older position;
- presence change and spatial sample could arrive in either sequence;
- view reconcile might despawn/recreate and lose interpolation;
- dropped/late sample could extrapolate through a wall.

Mitigation:

- simulation tick/revision on both protocols;
- position removed from semantic reconciliation authority or explicitly versioned;
- clear mirror merge rules with tests;
- short interpolation delay and clamped extrapolation;
- presence transitions reset samples;
- debug displays authoritative versus rendered positions;
- no network complexity is assumed, but protocol remains robust.

## 10.14 LOD correctness risk

Cold simulation can accidentally become teleportation or change behavior:

- actor makes fewer avoidance delays and arrives earlier;
- promotion resolves to a different route point;
- event ordering changes because cold actor sleeps until arrival;
- a global sound needs a position between samples.

Mitigation:

- analytic progress uses route distance and physical speed, not direct start/end interpolation;
- schedule semantic arrival at computed physical time;
- position materialization API for queries;
- promotion does not reroll choice;
- active/warm/cold differential tests;
- avoidance delay is treated as local visual delay unless it becomes materially large, at which point ETA/activity updates coherently;
- collect promotion correction and arrival-difference metrics.

If exact analytic parity proves too complex, 500 simple route followers at low Hz may already fit budget. Measure before committing to elaborate cold representation.

## 10.15 Day/night rendering risk

Potential failures:

- physically plausible lux values produce black or blown-out gameplay under current tone mapping;
- atmosphere misbehaves with sun below horizon;
- hundreds of point lights destroy GPU performance;
- lamp state changes pop visibly;
- shadow quality flickers as lights are culled;
- current always-on interior lights flatten daylight.

Mitigation:

- six reference-time captures;
- smooth curves with tested horizon clamping;
- semantic lit state separate from renderer light budget;
- emissive distant lamps, small nearby actual-light pool;
- fade lights in/out on culling and state changes;
- disable most lamp shadows;
- tune fog/ambient/exposure together, not sun alone;
- track active light count/GPU frame time.

## 10.16 Reflection cost and responsiveness risk

Even “once daily” is frequent when a game day is under an hour. Background work may also starve foreground turns if it shares the only worker.

Mitigation:

- no per-actor ambient reflection;
- dirty/eventful named actors only;
- hard game-day and rolling-real-time request/token/USD caps;
- spread across sleep, no midnight burst;
- independent lane/cancellation or do not implement;
- default disabled/conservative;
- prompt archive review for actual value;
- routine behavior unaffected by skipped reflection.

Trigger to remove/disable live reflection: outputs are generic, rarely influence visible behavior, create latency, or consume more cost than player-facing dialogue for comparable sessions.

## 10.17 Content-prompt feedback risk

An LLM may suggest a motive unsupported by world content, then repeatedly reflect on its failure.

Mitigation:

- prompt contains only whitelisted kinds/known targets;
- output schema caps priority/expiry;
- behavior failure does not cause immediate retry call;
- journal coalesces repeated same failure;
- structured motive has attempt/cooldown/abandon rules;
- next reflection sees factual outcome and may revise;
- narrative goal may remain aspirational without forcing executable action.

## 10.18 Simulation burst risk

Dawn, dusk, midnight agenda generation, debug time jumps, topology changes, and load can all wake many actors.

Mitigation:

- deterministic schedule jitter;
- time-ordered wake queue;
- bounded agenda generation batches if necessary;
- route budgets and priority;
- reflection spread;
- lamp renderer pool rather than spawn-all;
- explicit time-jump reconciliation mode;
- burst metrics/test scenarios;
- never do an unbounded all-actor scan inside each actor update.

Some all-cast operations once per game day are acceptable if measured and staged across physical frames. They should be named and budgeted.

## 10.19 Persistence risk

Without persistence, types may acquire transient handles that later cannot be saved. With premature persistence, every experimental state becomes a compatibility promise.

Mitigation:

- stable serializable domain types from the start;
- keep route/avoidance/ECS handles in caches;
- version schemas;
- persist enough semantic travel state but replan routes;
- do not promise backward-compatible public save format until a save system exists;
- build migration fixtures once the first format is used outside tests.

## 10.20 Behavioral uncanny-valley risk

Movement can make the city feel worse if:

- everyone walks constantly;
- routes repeat visibly;
- actors never acknowledge one another;
- workers stand at blank walls;
- schedules are too synchronized;
- people vanish at implausible façade points;
- NPCs ignore time and player interruption;
- “socializing” asserts conversations that never happened.

Mitigation:

- meaningful dwell time is normal;
- role-specific work loops and local sandbox;
- novelty/cooldown and route variations;
- spot facing/pose review;
- seeded jitter;
- portal placement/transition animation;
- attention leases;
- nonverbal/curated ambient social presentation without semantic claims;
- follow-one-character drive reviews, not only crowd overhead.

## 10.21 Open product questions and when they matter

### Before Milestone 1 default-on

- Should Esc pause the world, and should focus loss pause it?
- Is 48 real minutes a reasonable initial normal day, or should normal play target 24/72?
- Should the player-facing HUD always show time or only debug/menu?

Recommended defaults are pause in menu/focus loss, 48 minutes, and a restrained optional clock.

### Before Milestone 2 acceptance

- Is the cathedral interior part of the first street nav region?
- Which bridges/elevated passages are currently physically traversable by the player?
- Are any gates intended to close in the first release?
- What actor radius preserves desired alleys without wall clipping?

Answer with collision/debug overlay and route fixtures, not lore inference.

### Before full-cast data generation

- Which major characters require specific homes/workplaces/routes?
- Which institutional residents sleep inside their institution?
- Which actors explicitly have no fixed residence or restricted movement?
- How dense should streets remain at night?
- Are children/very old actors represented with distinct enough movement profiles to merit authoring?

Generate a report first; use authored overrides for story intent.

### Before practical lighting

- How many lamp fixtures/routes should exist, and which streets are deliberately unlit?
- Should moonlight be always available for readability or vary later?
- Are bells hourly, phase-only, or schedule-specific?

### Before reflection enablement

- What dollar/token budget per real play hour is acceptable?
- What evidence qualifies an event as reflection-worthy?
- Are live reflection changes considered ephemeral save state or curated lore?
- Does aggregate ambient pressure add noticeable value over seeded code?

Default answer to the last question should remain “code-only until a comparison proves otherwise.”

## 10.22 Deferred capabilities and compatibility hooks

Design hooks now, but defer implementation:

- real building interiors connected through existing `PortalId`;
- gates/doors and dynamic nav link availability;
- weather modifying package validity and route costs;
- festivals/events adding temporary agenda templates and affordances;
- crime/guard reaction as urgent package tier;
- quests issuing typed directives;
- carts/animals through `NavigationProfileId`;
- following/escorting through a moving-target activity;
- combat/tactical navigation as a separate layer;
- player wait/sleep summarized fast-forward;
- seasonal solar phase data;
- skeletal locomotion driven by existing kinematic snapshot;
- economic production/consumption feeding motives.

Do not add placeholder implementations that pretend these systems exist. Preserve stable extension points only.

## 10.23 Independent sources examined for this plan

Local project evidence:

- `crates/cathedral-sim` engine, world, attention, action, prompt, scheduler, and domain state;
- `src/smart_actors` host bridge, mirror, actor view reconciliation, and local engine pump;
- `src/city` plan/build/collision generation and cadastral building/place/road/fixture data;
- `src/scene.rs` current atmosphere, directional/ambient/interior lighting;
- authored population/lore schema and cast data;
- `cathedral-headless` deterministic/fake execution path;
- `~/seagame` world clock and actor behavior/path execution code.

External implementation references:

- [GECK ordered AI package list](https://geckwiki.com/index.php/AI_Packages_Tab) and [sandbox package](https://geckwiki.com/index.php/Sandbox_Package);
- [`landmass` engine-agnostic navigation](https://docs.rs/landmass/);
- [`bevy_landmass` 0.12.0 integration](https://docs.rs/crate/bevy_landmass/0.12.0);
- [`rerecast` navigation mesh generation](https://docs.rs/rerecast/).

These references inform patterns and spike candidates. None should override the Cathedral's pure-simulation authority or substitute for tests against its actual geometry.

## 10.24 Final recommendation

Implement the living city as a deterministic simulation feature, not as ambient animation:

- explicit accelerated calendar plus natural physical time;
- offline-validated free-space navigation;
- typed intent/activity/presence states;
- ordered package routines and capacity-aware affordances;
- local authored/generated homes, work, patrols, shelters, and portals;
- fixed-step authoritative locomotion with Bevy interpolation;
- active/warm/cold execution budgets;
- conversation leases and current-position gameplay checks;
- day/night roles, lamps, and visual lighting;
- optional slow reflection under an independent hard-capped lane.

The shortest credible path is: clock, representative nav spike, full nav artifact, one actor end to end, a diverse cohort, then all-cast generation/LOD. LLM reflection belongs after that foundation has already made the city feel alive.
