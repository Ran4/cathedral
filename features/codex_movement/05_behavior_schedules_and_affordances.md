# 5. Behavior, schedules, and affordances

A living city needs two apparently conflicting qualities:

- **legibility**: a watchman should patrol at night and a seller should attend a market;
- **variation**: people should not leave in a synchronized wave, select the same idle point, or repeat one loop forever.

Use an ordered package model for legibility and bounded utility/weighted selection inside a package for variation. This borrows the strongest idea from Fallout-style AI packages—conditions and schedules choose the currently valid class of behavior—without copying engine-specific implementation. The GECK documentation for the [ordered AI package list](https://geckwiki.com/index.php/AI_Packages_Tab) and [sandbox package](https://geckwiki.com/index.php/Sandbox_Package) is a useful conceptual reference: scheduled/conditional packages determine broad action, while nearby valid objects and idle opportunities make sandbox time feel less scripted.

## 5.1 The hierarchy

Recommended evaluator order:

1. **transition safety** — finish/abort an atomic portal or other non-interruptible transition;
2. **urgent reaction** — future danger, severe condition, quest-critical directive, or explicit engine event;
3. **player engagement** — targeted speech, conversation lease, pending offer/handoff;
4. **structured directive** — validated quest/LLM/player agreement with priority and expiry;
5. **essential routine** — sleep/rest threshold, assigned watch shift, role-critical operation;
6. **scheduled routine** — work, worship, meal, market, home, patrol, service;
7. **personal motive** — a structured longer-running preference or opportunity;
8. **opportunistic activity** — fetch water, visit, browse, socialize, tavern, observe;
9. **local sandbox** — use an appropriate nearby idle/affordance;
10. **safe fallback** — wait, return home, or remain at the current valid spot.

“First valid package” is easier to reason about than one giant global utility calculation. Within `OpportunisticSocial`, for example, utility can score three taverns and two acquaintances. It cannot outscore an assigned night-watch shift because it belongs to a lower package.

The exact order is data, with a few engine-enforced safety constraints. Quest systems may insert a directive at a known tier; arbitrary content cannot outrank a conversation by using an enormous float.

## 5.2 Package definition

```rust
pub struct BehaviorPackage {
    pub id: PackageTemplateId,
    pub tier: PackageTier,
    pub conditions: Vec<BehaviorCondition>,
    pub intent_factory: IntentFactory,
    pub destination_query: Option<AffordanceQueryTemplate>,
    pub duration: DurationPolicy,
    pub interruption: InterruptionPolicy,
    pub failure: FailurePolicy,
    pub cooldown: CooldownPolicy,
}
```

Conditions should be a bounded typed expression language, not arbitrary Rust closures in content and not prose:

```rust
pub enum BehaviorCondition {
    TimeInWindow(ScheduleSlotId),
    PhaseIs(DayPhase),
    HasRole(RoleTag),
    NeedAbove(NeedKind, Threshold),
    NeedBelow(NeedKind, Threshold),
    HasDirective(DirectiveKind),
    CurrentPlace(PlacePredicate),
    SpotAvailable(AffordanceQueryId),
    EventFlag(WorldEventKind),
    NotOnCooldown(ActivityKind),
    ActorFlag(ActorFlag),
}
```

Keep boolean composition shallow and validate it offline. If content starts requiring a programming language, the package taxonomy is probably missing a concept.

## 5.3 Evaluation and scoring

On a decision wake:

1. evaluate package tiers in deterministic order;
2. discard packages whose conditions are invalid;
3. for the first tier with viable packages, build bounded intent candidates;
4. query indexed affordances rather than scanning every spot;
5. score candidates;
6. use stable seeded weighted choice among near-best candidates;
7. reserve the chosen slot;
8. instantiate an intent or apply the package's failure policy.

Example destination score:

```text
score = role_affinity
      + personal_preference
      + relationship_opportunity
      + novelty_bonus
      + schedule_fit
      + route_safety
      - travel_time_cost
      - crowding_cost
      - repetition_penalty
      - lateness_risk
```

Normalize and clamp each term. Deterministic weighted choice among candidates within, for example, 10% of the best score avoids always choosing one mathematically optimal bench. A strong invalidity condition—closed, disconnected, full, wrong role—removes a candidate rather than applying a huge negative magic number.

## 5.4 Daily agenda versus live activity

A `DailyAgenda` is a set of intended windows, not a script of exact coordinates:

```rust
pub struct DailyAgenda {
    pub for_day: u32,
    pub entries: Vec<AgendaEntry>,
    pub generated_from: ScheduleTemplateId,
}

pub struct AgendaEntry {
    pub id: AgendaEntryId,
    pub window: AbsoluteScheduleWindow,
    pub activity: PlannedActivity,
    pub importance: AgendaImportance,
    pub destination: PlannedDestination,
    pub completion: AgendaCompletion,
}
```

At the day boundary or before the actor wakes, deterministic agenda materialization:

- resolves weekday/phase variants;
- applies actor-specific shift and seeded time jitter;
- includes known role assignments and household obligations;
- estimates travel lead time;
- respects spot capacity/shift allocation;
- leaves optional slots open for personal initiative;
- records why a required slot could not be assigned.

Live package evaluation adapts the agenda to interruption and world state. It may leave early, arrive late, skip optional leisure, choose a fallback work site, or resume after conversation. The agenda is not a command queue that must execute blindly.

## 5.5 Schedule templates

Templates should use role families rather than one file per character. Suggested first families:

| Template family | Typical day shape | Important affordances |
|---|---|---|
| market seller | home → setup → tend stall → meal break → tend → pack → home/tavern | home portal, assigned stall, market storage, meal/social spots |
| workshop craft | home → workshop shift → errand/meal → workshop → home | assigned work slot, supply/delivery spots |
| day laborer | shelter/home → hiring square → assigned labor or wait → meal → work/social/home | hiring point, generic labor spots, cheap meal/shelter |
| porter/cargo | depot/gate → pickup/dropoff circuit → break → circuit → home | cargo nodes, gates, warehouses, route chain |
| domestic servant | household/work portal → fetch/deliver/service loop → break → household | household portal, well, market, service entrances |
| food provision/baker | very early workplace → market/delivery → rest/home | oven/work portal, stall, delivery anchors |
| watch day shift | muster → post/patrol loop → break → handoff → home | guard posts, patrol route, gatehouse |
| watch night shift | evening muster → patrol/post loop → handoff → home | night patrol, gatehouse, watch fire |
| lamplighter | daytime ordinary role/rest → dusk lamp route → night/home → dawn route if assigned | lamp route and service slots |
| clergy/religious | home/cloister → offices/service → duties/visits → service → rest | church portals, prayer/service spots |
| bell ringer | home/other work with timed bell duties | tower portal, bell interaction |
| tavern worker | later start → preparation → evening service → close → home/interior | tavern portal/service positions |
| civic/guild | home → office/hall → meal/errand → office → home | civic/guild portals and public desk |
| messenger | dispatch point → bounded deliveries → return/wait | dispatch, destination portals |
| child/dependent | household-local activity → errands/play/learning → household | home, safe local sandbox, family-linked spots |
| infirm/limited mobility | local home/interior and nearby social/service routine | accessible portal, close resting spots |
| unhoused/begging | shelter/covered night spot → water/meal → begging/social circuits → shelter | shelter, arcade, alms spot, well, fire |
| unemployed/pauper | local search/hiring/alms/errands/social, with substantial idle variance | hiring, alms, market edge, shelter |
| prisoner/restricted | assigned interior/court/guarded location only | restricted portal/yard |

This table is a starting taxonomy. Compile the many authored occupation strings into role tags and one or more template candidates offline. Preserve the raw occupation for lore and prompts.

## 5.6 Assignment should reflect the authored person

The generator should consider:

- planning ward/district;
- occupation and workplace-compatible building use;
- household/family links where known;
- socioeconomic status and plausible home/shelter type;
- age and mobility;
- faction or institutional role;
- authored conditions;
- authored significance;
- illegal or night work where explicitly present;
- bespoke overrides for named story characters.

It should not infer moral worth, personality, or story outcomes from poverty, disability, gender, or other descriptors. Use these fields only for concrete placement/mobility needs supported by authored context.

Generation output is reviewed data with stable IDs, not runtime guessing. A report should flag:

- actors without a home/shelter/interior policy;
- roles without a compatible work assignment;
- commutes above thresholds;
- overloaded buildings/spots;
- inaccessible assignments;
- assignments outside the intended ward;
- mobility profiles with inaccessible required links;
- major characters still using generic fallbacks.

## 5.7 Affordances: what a place lets someone do

A coordinate is not a destination. An affordance says what can happen, where the actor stands, when it is available, and how many actors fit.

```rust
pub struct ActivitySpot {
    pub id: ActivitySpotId,
    pub place: Option<PlaceId>,
    pub position: NavLocation,
    pub facing: FacingPolicy,
    pub tags: TagSet<ActivityTag>,
    pub slots: Vec<ActivitySlot>,
    pub opening: AvailabilitySchedule,
    pub role_filter: RoleFilter,
    pub social_group: Option<SocialGroupLayout>,
    pub portal: Option<PortalId>,
    pub source: SpotSource,
}
```

Useful initial tags:

- `HomePortal`, `Shelter`, `SleepInterior`;
- `Work`, `Workshop`, `StallSeller`, `StallCustomer`, `Office`, `Labor`;
- `Well`, `Cistern`, `Delivery`, `Storage`, `Cargo`;
- `GuardPost`, `PatrolNode`, `Muster`;
- `LampService`, `BellDuty`;
- `Prayer`, `Service`, `ChurchPortal`;
- `Meal`, `Tavern`, `Social`, `Observe`, `Rest`, `Beg`, `Play`;
- `PortalEntry`, `PortalExit`, `WaitNearPortal`.

Each slot has an arrival region and preferred pose/facing. A bench might have two rest slots. A well might have two use slots and three waiting slots. A market stall separates seller and customer sides. These small layouts make crowds look intentional.

## 5.8 Building-derived affordances

The city plan's building uses can bootstrap many spots:

- choose a reachable street-facing portal point at a building edge;
- create virtual interior IDs for residential, trade, workshop, tavern, lodging, guild, civic, storage, and ecclesiastical uses;
- create a limited number of exterior work/customer/service positions based on use and footprint;
- add generic rest/social/observe positions around sites and squares;
- add explicit authored spots for story-significant places.

Generation must be geometric, validated, and deterministic:

1. find candidate boundary edges facing reachable open space;
2. reject edges near corners, obstacles, or another incompatible portal;
3. sample approach/exit slots onto navigation;
4. confirm a short route from the street component;
5. orient facing toward the façade or activity object;
6. store source building ID and generation version.

Do not place spots at building centroids. That sends actors through walls and makes portals visually meaningless.

## 5.9 Capacity, opening hours, and reservations

Every destination query must consider:

- activity-specific capacity;
- current reservations and occupants;
- opening schedule;
- role/faction/access restrictions;
- portal or nav connectivity;
- expected arrival before closing;
- crowd density near the arrival region;
- whether a paired/group activity has the required participants.

Capacity is not only a performance optimization. It is part of believable behavior. If a tavern is full, an actor waits, chooses another, or returns home; thirty people do not stand at one point.

Reservations should distinguish roles at one place. A stall's seller slot remains assigned to its seller while customer slots turn over. A church service may admit many standing attendees but only one bell-ringer slot.

## 5.10 Local sandbox behavior

Sandbox is the default between obligations, not random wandering across the city. Given a radius or semantic area, the actor queries valid spots based on profile and phase:

- rest or observe in a square;
- browse a nearby market;
- visit a tavern in evening;
- draw water during an errand window;
- stand near a workplace before a shift;
- return toward home as night approaches;
- take a short neighborhood walk using a reachable loop;
- socialize nonverbally with an available known actor;
- wait at an appropriate edge rather than in a traffic lane.

Use repetition history:

```rust
pub struct ActivityHistory {
    pub recent_kinds: RingBuffer<ActivityKind>,
    pub recent_spots: RingBuffer<ActivitySpotId>,
    pub cooldowns: BTreeMap<CooldownKey, GameInstant>,
}
```

Novelty weighting should decay rather than permanently ban a favorite place. Personal preferences can make one tavern or church genuinely habitual.

Pure “wander to a random nav point” should be the final fallback and tightly local. Even then, choose a safe arrival region outside traffic choke points and attach a reason such as `TakeAir` or `NeighborhoodWalk`.

## 5.11 Personal initiative without an LLM

People can have “their own idea of what to do” through a small structured motive system:

```rust
pub enum PersonalMotive {
    EarnWage,
    SeekWork,
    MaintainHousehold,
    PracticeFaith,
    Socialize,
    VisitPerson(ActorId),
    VisitPlace(PlaceId),
    Procure(ActivityTag),
    Deliver(DeliveryRef),
    Patrol(RouteId),
    Observe(TopicOrPlace),
    Rest,
}
```

An actor has persistent motive weights derived from authored profile and optional overrides. Daily agenda generation allocates open windows to a subset. Live opportunity queries realize them only when world support exists.

Example: an observant, curious minor character with a `VisitPlace(churchyard)` preference may choose that location in a free afternoon window. That choice is code-driven, seeded, and understandable in diagnostics. An optional nightly reflection could later change the preference after a meaningful event, but it is not needed for the behavior to exist.

Do not parse the existing prose `goal` to manufacture a motive at runtime. A one-time reviewed content compiler may suggest mappings, but runtime uses explicit structured fields.

## 5.12 Lightweight needs

Needs are useful only when the world can satisfy them visibly. Initial recommendation:

- `fatigue`: rises during active waking time, falls during sleep/rest;
- `social_pressure`: optional, slow, creates a preference rather than an emergency;
- `meal_window`: represented primarily by schedule, not a starvation meter;
- injury/illness/mobility: static or event-driven modifiers, not a generic decaying bar.

Avoid adding hunger, thirst, bladder, cleanliness, and full morale in the first movement delivery. Without food ownership, beds, toilets, pricing, and consequences, these produce repetitive pantomime and hidden failure states.

Fatigue rules:

- normal sleep package becomes valid in the actor's schedule window;
- extreme fatigue can promote rest/sleep above optional work;
- night alone does not force every role home;
- sleep occurs in a valid interior/shelter/spot assignment;
- missing sleep assignment is a content validation error, not a reason to lie down in the street.

## 5.13 Work as visible activity

“Work” should not mean standing motionless for eight game hours. Each role template defines a small activity loop at or near the work place:

- seller alternates tend, arrange, wait, short customer interaction pose;
- porter performs pickup, route, dropoff, rest;
- watch alternates patrol and post;
- workshop actor alternates several exterior/interior work poses or remains virtual inside;
- servant performs bounded errands between household, market, well, and delivery points;
- clergy alternate service/prayer/interior duty/short visits;
- laborer uses one of several generic work sites and rest points.

The first visual implementation can use timed poses and movement rather than item animation. Activity names should be honest: if there is no hammer/tool animation, `WorkAtBench` can face and idle at the bench without claiming a simulated product was created.

Routine loops should not emit semantic memories. Aggregate counts such as “completed 4 deliveries” can enter the journal only if reflection or diagnostics need them.

## 5.14 Patrols and route circuits

Patrol is not repeated point-to-point destination selection. Define:

```rust
pub struct PatrolRoute {
    pub id: PatrolRouteId,
    pub nodes: Vec<PatrolNode>,
    pub mode: PatrolMode,
    pub dwell: DurationRange,
    pub shift_assignment: Option<ShiftId>,
}
```

Modes include loop, ping-pong, and seeded branch at junctions. Nodes may be posts, lamps, gates, or observation spots. Route validation precomputes connectivity and length. Shift assignments stagger start node/direction so four watchmen do not overlap exactly.

Lamplighter circuits use task-bearing nodes and semantic completion state. Watch circuits use observation dwell and may react to future incidents. Messengers use generated delivery sequences, not endless fixed patrols.

## 5.15 Social activity without fabricated conversation

Code-driven socializing can:

- reserve compatible positions in a social group layout;
- face nearby participants;
- play gesture/listening/idling visuals;
- choose a short curated non-semantic bark by role/phase, subject to audio limits;
- satisfy lightweight social preference;
- end cleanly when a participant's schedule or player interaction preempts it.

It should not invent detailed dialogue, promises, memories, secrets, or relationship changes without cognition or authored content. A silent animated exchange can communicate city life without asserting facts.

Pair/group rendezvous needs a handshake:

1. initiator proposes a group spot and time-limited invitation to known/eligible nearby actors;
2. participants accept only if their higher-tier packages permit;
3. capacity is reserved atomically;
4. participants travel independently;
5. a wait timeout handles absence;
6. the group activity starts with whoever satisfies the minimum size or cancels.

Do not make one actor chase another's live coordinate indefinitely.

## 5.16 Household coordination

Household data can improve plausibility without simulating domestic life deeply:

- family members may share a home portal/interior;
- dependent actors can be assigned local schedules related to a caregiver's household, not necessarily physically tethered;
- servants can share a work-household portal distinct from their home;
- joint worship/market visits are optional rendezvous entries;
- wake/sleep jitter prevents a household from emerging in one exact pile.

Avoid assuming all authored family relations imply co-residence. Use an explicit generated/authored `HouseholdId` and validation report.

## 5.17 Interruptions and resume policy

Every package declares one:

- `ResumeIfStillValid`: most travel/work after a brief greeting;
- `Reevaluate`: optional leisure or a full destination after a long interruption;
- `Cancel`: short sandbox actions;
- `CompleteBeforeInterrupt`: rare atomic transition;
- `SuspendUntil(Event)`: waiting for a paired actor or offer resolution.

Preemption matrix examples:

| Current | Incoming | Policy |
|---|---|---|
| walk to work | player speaks | decelerate, attention lease, then resume if window/slot valid |
| tend stall | schedule meal | finish short pose, reserve stall through break if assigned, take meal |
| local sandbox | work window | cancel and depart with travel lead |
| sleep interior | ordinary ambient sound | remain asleep unless sound is explicitly urgent |
| portal transition | player speaks | finish the sub-second/short transition, then engage if still reachable |
| night patrol | optional social opportunity | ignore lower tier |
| any routine | urgent future danger | release/suspend reservations and react |

This table should become tests, not only documentation.

## 5.18 Failure and fallback policies

Packages must say what failure means:

- no compatible spot: use role-specific fallback or safe idle;
- spot full: choose next scored spot, wait nearby if permitted, or skip optional activity;
- disconnected route: flag content/navigation issue and choose same-component fallback;
- arrival after closing: cancel or switch to another package;
- repeated stuck recovery: release destination and re-evaluate;
- portal unavailable: wait within a bounded queue or choose fallback;
- scheduled work missing: record a diagnostic; do not roam cross-city seeking any generic work point;
- required home missing: use explicitly assigned shelter fallback and keep a validation failure visible.

Fallbacks should degrade plausibly, not hide authoring defects. The debug overlay must show `reason` and last failures.

## 5.19 Relationship to LLM turns

Routine activity should enter prompts as concise context only when a prompt is already warranted:

```text
You are walking to your assigned stall in Wickmarket and expect to arrive late.
```

The behavior layer does not ask the LLM whether to take the next route corner. The LLM may influence a structured directive or motive through the bounded system described in the next chapter. Code validates priority, destination knowledge, capability, travel window, and expiry.

## 5.20 Behavior acceptance criteria

1. With all cognition disabled, every eligible actor produces a deterministic agenda and plausible routine.
2. Actors with different roles occupy visibly different places and times.
3. Start-time jitter and capacity prevent synchronized waves and point piles.
4. A named actor can be followed through a complete home/work/leisure cycle.
5. Conversation interrupts and resumes according to explicit policy.
6. No package scans all actors or all affordances every decision.
7. The same seed and input events produce the same chosen packages/spots regardless of frame rate and sim LOD.
8. Missing assignments and unreachable destinations generate actionable validation errors.
9. Routine behavior does not create LLM requests, semantic dialogue, or unbounded memories.
10. Night, dawn, daytime, and evening population distributions differ in role-appropriate ways.
