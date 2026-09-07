Status: Historical read-only audit (2026-09-05), reconciled against current source in BASELINE_RECONCILIATION (2026-09-07). Findings below are not implemented fixes.

# What the 2026-09-05 code provided

[M0’s current-source reconciliation](BASELINE_RECONCILIATION.md) supersedes the dated knowledge/prerequisite state and hearsay-seizure gap below. The existing clock, save, geometry, authority and escort limitations remain. Current ownership is in [PERSISTENCE_INVENTORY](PERSISTENCE_INVENTORY.md).

The audit used working-tree source at HEAD `f56a2c306ec1523fc569c7c2de232b8c90d4d4ef`, including uncommitted knowledge M1 changes. HEAD alone does not describe that working tree. Existing unrelated modifications were retained.

During this planning session, concurrent repository work committed the knowledge goldens as `18bd26d` and M1 as `0504d29`; further pollen/M2 source edits appeared. The final feature README read reports M2 implemented and M3–M5 pending. These are external/concurrent changes, not implementation performed by this plan. Treat this document as a dated audit and reconcile the final state at M0.

`cargo test -p cathedral-sim --tests` completed successfully during this audit. That establishes the current deterministic test baseline; it does not certify any new capability in M1–M19. No live-provider acceptance was run for this plan.

## A. Engine and persistence

| Verified source | Finding | Consequence for the plan |
|---|---|---|
| `crates/cathedral-sim/src/engine.rs` — `Engine`, `poll`, `new` | World state, rounds, clock/weather, scheduler, speech floor/router and Night Office have separate mutable owners | M2 needs explicit whole-engine hydration rather than a `World`-only save |
| `snapshot.rs` — `PublicSnapshot` | Explicitly omits private knowledge, memories, goals, inboxes and other operational state | A public snapshot cannot become the checkpoint format |
| `round.rs` — `Round`, `Townsperson` | Queues, market/production progress, household settlements, road parties, lamps and polling anchors live here | Reseeding the round after a load would change future behaviour and duplicate daily work |
| `scheduler.rs` — `InFlight`, `NpcScheduler` | A submitted prompt drains inbox/history into in-flight buffers; a result can be held before application | Restore either the result or its obligation, never both; preserve input provenance |
| `speech_router.rs`, `floor.rs` | Active capture/jobs, presented speech waits and separate foreground/background pacing | Restore semantic speech/drafts and discard old external handles under a new generation |
| `night.rs` — `NightOffice` | Owed/in-flight reflection, per-subject last day and scheduling anchors | A reload must not grant another reflection or erase an already settled memory |
| `clock.rs` — `WorldClock` | Calendar time derives from `now` and an epoch; many services retain process-style deadlines | Rebase all relevant anchors together when a new host/engine starts |
| `src/smart_actors/local_engine.rs` — `LocalEngine` | Non-`Send` engine with backend completion and command channels | Capture a plain value for worker IO; do not move the engine to a thread or reuse old-world callbacks |
| `src/controller.rs` — `PlayerController` | Velocity, view, grounded/coyote/jump state and flight live in the host | Saving only the sim player position cannot restore movement coherently |
| Host pump/position synchronization | Pump currently precedes throttled/interpolated player synchronization | M1/M3 need an accepted authoritative physical sample and command watermark, not merely a same-frame capture |
| `attention.rs`, `night.rs`, `scheduler.rs` | Novelty has a behavioral visit salt; Night stamps at queue time; generic idle requeue drops idle work | Save exact salts, explicit owed-day duty state and load-specific retry obligations |
| `src/city/vermin.rs` | Swarm cursors generate `WorldSound` and NPC percepts outside Engine | Preserve/move these semantic cursors; visual rebuilding cannot repeat announcements |
| `src/smart_actors/custody.rs` | Player strain and its reported latch generate escape/struggle commands | Persist them with the input watermark or move them into sim authority |
| `src/soundscape.rs`, backend `runtime.rs` | Independent cue edge detectors; backend destructor may wait 500 ms | Explicit presentation continuation and budgeted retirement belong in M3 |

See [PERSISTENCE_INVENTORY.md](PERSISTENCE_INVENTORY.md) for the field groups and [CHECKPOINT_PROTOCOL.md](CHECKPOINT_PROTOCOL.md) for the capture/adoption/default-pending-work policies.

## B. Time and activity integration

`Engine::poll` already advances office checks, weather, movement, notices/custody and the round outside ordinary conversation-floor gating. This is a useful base for the user's never-pause requirement. UI input capture and a particular NPC's conversation courtesy are distinct from pausing the whole city.

There is not yet a shared long-running activity/resource/duty service for evidence comparison, meetings, retrieval and guard search. `Round` has several specialised duties and `TravelIntent` is useful, but new systems must not each overwrite the same actor's route independently.

Movement advances in 0.05 s slices with a bounded catch-up. Current movement code can drop excess backlog, and each waypoint can discard leftover distance in a slice. A formal timed reconstruction cannot assume `distance / speed` equals actual traversal time or silently ignore a dropped span. M1/M4/M8 must define and test the relevant timing policy.

`lib.rs` allows eight movement slices, only 0.4 seconds. The ordinary headless clock watcher selects `seconds_per_day / 200` (18 seconds at the ordinary day); its optional water/census/food three-second cap still exceeds movement catch-up. The opening source comments wrongly said 3.2 seconds. Concurrent pollen work corrected those comments and added `--trace-pollen --pollen-step` with a 0.4-second default. That narrower measurement mode does not establish a repaired general late-world runner. M0 can use an `Engine::poll` harness with accepted increments no larger than 0.05 seconds, then reconcile the actual CLI before M1/M4 work.

Normal host time also needs reconciliation: the player/controller setup caps Bevy virtual delta at 100 ms and the local engine uses the generic `Time` elapsed value. A long frame can therefore discard wall duration before it reaches the sim. Simply switching the engine to real time would instead let calendar advancement outrun dropped movement. M1 must choose one coherent accepted-time/debt policy and expose overload measurements.

## C. Geometry and navigation

| Verified source | Finding | Consequence |
|---|---|---|
| `nav/mod.rs` | Schema 1 nodes are `[x,z]`; `WALK_Y = 0.91`; snapping/routes are planar | Add surface-qualified layered navigation, not only Y in render transforms |
| `world.rs` — `step_movement` | Writes the fixed walk height and advances XZ motion | NPC movement itself must traverse the new elevations |
| `round.rs` route/leash/home helpers | Several helpers recreate planar paths/positions | Audit all route consumers; changing nav nodes alone is insufficient |
| `scripts/bake_navigation.py` | Quarter-metre ground walkability from collision footprints, radius erosion, largest component | Do not erase interior floors or closed-but-reachable rooms from the baked topology |
| `src/city/mod.rs` — `build_buildings` | Most building footprints remain solid despite visual doors/windows | Author real shells/openings for selected interiors |
| `src/scene.rs` | Lanthorn is an existing real interior exception | Reuse collision/structural lessons without assuming all other houses are enterable |
| `build_yard_stairs`, `build_open_balconies` | Stairs are largely scenery; landings can support flying-player perches | These are not current walking/NPC upstairs routes |
| `src/controller.rs` — `CollisionWorld`, `move_aabb` | Useful swept 3D collision; no general stair/step/slope support policy | M4 must add usable traversal, headroom and body-support tests |
| `CutMarginProfile::ground_lift`, `actors.rs` | Cut height is applied separately to host movement/rendering | Reconcile physical eye/feet heights and avoid double lifts |
| `DynamicBarrier`, `src/city/gates.rs` | Freight gates have host-owned dynamic barriers; ordinary nav does not carry portal state | Move shared physical portal state into the spatial foundation and project it consistently |
| `CollisionWorld::nearest_ray_hit` | Static geometry only | Dynamic doors/barriers must also affect interaction/perception rays |
| `areas.rs` / `AreaMap` | Overlapping area interiors are rejected | Add subordinate rooms/surfaces rather than inserting nested boxes into the current flat area schema |

The committed navigation data inspected by the spatial audit contains 3,972 nodes, 4,008 edges and 1,101 doors. Those counts describe the current artefact and must not become permanent constants in new content.

### Current Tallage anchors

| Feature | Audited current reference |
|---|---|
| Tallage named place | Approximately `(-213.5, 63)` |
| Weigh-beam | `tallage_weighbeam`, approximately `(-214.2, 45.5)` |
| Tally Bridge | Upper passage between toll-house and warehouse over the dry Cut, centre approximately `(-213.5, 73.5)` |
| Pawnshop | `named_copp_shop`, centre approximately `(-188.8, 33.1)`, rendered three levels |
| Pawnshop façade door | Node 650, approximately `(-189.625, 47.375)` |
| Pawnshop named-place destination | Separately snapped node 484, approximately `(-196.375, 27.125)` |

The door/place mismatch is concrete evidence for stable authored entrance IDs. It is not evidence that either existing node is the intended final case entrance.

![Existing Tallage source-data illustration](evidence/tallage_existing.png)

This generated plan uses the current cadastral footprints and ground graph, north `+x` and east `-z`. The two pawnshop destinations are 21.35 m apart in a straight line. [site_audit.json](evidence/site_audit.json) records exact coordinates and source hashes; [render_site_audit.py](evidence/render_site_audit.py) regenerates the illustration. It is not a screenshot or a plan of proposed new interiors.

No validated comparison room, case hoist yard, loft rack or private desk was found. No source audit proves the proposed private/public timing inequality in the current city. The user has authorised the necessary district redesign.

## D. Perception and knowledge

`Sight` defaults to true and is stored by the engine, but current gameplay does not consume it. `perception::sees` checks horizontal facing only. Important consumers use neighbourhood distance for `you_see`, person-follow eligibility and continual `last_seen` refresh. Some witness paths similarly treat proximity as plain sight.

Speech and sound recipients use stable Euclidean-distance sets. Walls, slabs, doors and windows do not yet produce the local acoustic distinctions the mystery needs. Sound events already distinguish recipients and witnesses, which is a useful seam for the new queries.

The final voice audit followed `interaction.rs::poll_microphone_events`, `speech_router.rs::resolve_transcription`, `EngineCommand::PlayerSay` and `actions.rs::say`. The host samples player location at recording completion; the router applies ordinary speech as soon as transcription resolves and selects recipients then. Enter chat also immediately produces ordinary speech. Consequently, a later confirmation cannot prevent every socially consequential mis-transcription or erase what listeners already heard. M5 owns utterance-time acoustic coverage; M10 confirmation protects formal actions and keeps explicit unsent dictation separate.

The host also exposes the complete recipient count through `mod.rs`, `speech.rs` and `hud.rs::show_player_transcript_delivery`, displaying “nobody nearby” or “heard by N nearby people.” M5/M10 must remove this unearned audience knowledge. A private-action rejection can leak the same information even without a name: paired-world privacy tests therefore hold UI/commit outcomes identical when only an undetected listener differs, while retaining actual hearing consequences.

`src/smart_actors/hands.rs` renders selected held/offer props and animates transfers after ledger commitment; `actors.rs` interpolates motion behind sim samples. A seen actor/held item is therefore not proof that the player saw that particular object. M5/M7 need compatible observable descriptors and temporal coverage. `targeting.rs` is another consumer to audit. `weather.rs` already supplies visibility data, while `city/trade_props.rs` supplies decorative beam motion rather than an authoritative hoist timing mechanism. [SPATIAL_PROOF_PROTOCOL](SPATIAL_PROOF_PROTOCOL.md) owns these distinctions.

The knowledge feature's final 2026-09-05 status read reports M0 measured and M1/M2 implemented, with M3–M5 pending. Its propagation changes advanced during this audit; the complete journal/consequence feature is still a prerequisite. Reconcile its actual accepted interfaces at M0 rather than treating the opening M1 structure as final.

Durable historical evidence must not use a current-holder predicate as though it were a historical observation. `FactSource::ItemWith` can become false when an item moves; “I saw it with this person yesterday” needs its actual historical receipt. Current sealed source predicates are not omniscient evidence the player or an officer may consult.

Knowledge M5 intentionally includes lower-confidence social consequences such as hearsay-based summons and refusals. M11 must preserve that allegation/intake behaviour while separating it from authority to search/arrest. The new process must not erase the just-shipped gossip mechanic by treating every institutional action as already requiring a full criminal proof bundle.

## E. Law and custody

The law audit inspected `notices.rs`, `custody.rs`, `actions.rs`, `round.rs`, `engine.rs` and existing test seams. It did not implement fixes.

| Current behaviour | Why it is insufficient for this request | Owner |
|---|---|---|
| `WardNotice` contains prose, people/item refs, one summons and a warrant boolean | No structured allegations, evidence decision, scope or disposition history | M11 |
| Broad law occupations share all notice authority | Notaries/revenue workers get powers that should be distinct capabilities | M11 |
| `notices::carries` is true for every law actor | Officers gain city-wide knowledge without delivery/briefing | M11/M12 |
| `notices::confront` queues one proximity percept | No deterministic assignment/search and no guaranteed off-stage action | M12 |
| `issue_warrants` prioritises the accused | This does not dispatch a guard | M12 |
| `fresh_own_notice` checks raiser and age | A freshly repeated report can masquerade as the officer's witnessed breach | M11 |
| `Notices::expire` removes warranted notices after twenty game days | Conflicts with durable order authority and the module's own explicit-discharge comments | M11 |
| `take_into_charge` inserts custody before a fallible route-budget check | Failure can leave a custody mutation behind | M1/M12 shared action validation |
| Named seizure authority filters a first-found warrant | A second explicitly requested valid warrant can be rejected | M11/M12 |
| `follow_escorts` places an NPC behind the officer directly | Initial snaps and corner/wall crossing are possible; it is not route following | M12 |
| Return/settlement can erase the named notice | Property restitution could erase assault proceedings if charges remain conflated | M11 |
| Settlement and physical release are not one controlled disposition | A cleared matter can remain physically held or a different matter can be mishandled | M11/M12 |
| Ordinary arrest release uses short real-time/next-bell ceilings | Not a lasting case disposition; a still-live warrant could cause repeated arrest loops | M11 |
| Confiscation transfers the notice's one named held item | No scoped property search or formal evidence handling | M6/M7/M11/M12 |

Useful existing machinery remains: ordinary guard posts/rounds, `TravelIntent`, route budgets, custody ownership, keeper relationships, confined-character guards in the ladder, notifications, deterministic struggle calculations and player tether presentation. The plan extends those through shared services rather than adding a separate quest police system.

The second review found missing authority beyond those helpers: capacity is a city-wide four-new-arrest safeguard excluding eight authored inmates, not per-station occupancy; current Stone House staff all go home at Snuffing; booking's item transfer can fail silently; and the existing canon requires gate/night watch to deliver prisoner and written cause to Stone House by the next office bell. [LAW_PROTOCOL](LAW_PROTOCOL.md) assigns real briefing cadence, fair dispatch, post coverage, station reservations, multiple hold grounds, booking and release egress to M8/M11/M12.

## F. Object identity and casting

Whole non-stackable transfers preserve `ItemId`. Whole non-stackable `restamp_metadata` also preserves it, so getting a unique quantity-one paper wet is not currently an identity-loss bug by itself.

Swallowing is different: the original item is consumed and `GutEntry` retains kind/metadata without the original ID. Digestion later creates stock with a new ID. Keys and small evidence specimens need exact identity or explicit durable lineage through this path. Inventory restock provenance is not an evidence custody record.

The seven cast tiers are: Corin/Lise major; Odo/Warin/Averil/Gile minor; Mott ambient. `lore/characters/AGENTS.md` forbids individually required ambient quest references. Mott must be promoted to a stable tier; this plan chooses major. The minor definitions permit stable supporting figures, so the other four are not automatically required to become majors.

## G. GDD consistency repairs

The planning review found these defects in the original illustrative model/content:

- The reference private journey includes a three-second departure delay omitted from the “fits 32 s” execution check.
- The promised physical no-confession solution does not satisfy the original theft recipe.
- E15 is granted as though making an alibi statement already proved a contradiction.
- A next-day retrieval does not contradict the previous day's counter sightings.
- The private admission path names an independently verified nonpublic detail without authoring that detail.
- An alternative Warin clearance through the same identified assailant is promised but unmodelled.
- The late locked-desk search lacks a complete non-circular authority path.
- Deferral, delayed retrieval, reward fulfilment and the household tools lack sufficient executable detail.
- A 75–100 minute walkthrough cannot end at the original 72.5-minute review under never-pausing gameplay.

[CASE_CONTRACT.md](CASE_CONTRACT.md) records the proposed repairs. The GDD/model remain unchanged until content reconciliation; their old passing validator is not acceptance evidence for the corrected implementation.

## Audit limits

One hidden-window Bevy survey was attempted with `CATHEDRAL_HEADLESS=1`, fake cognition and a 120-second watchdog. Compilation succeeded, but the renderer reported a software-only adapter and the run timed out at its first screenshot request. `logs/session_777_2026-09-05_11_43_46/screenshots/` remained empty. This is **no visual acceptance evidence**. The run also logged cursor-confinement and audio-loader errors; they were not investigated as part of the source-data illustration. [REVIEW_LOG](REVIEW_LOG.md) records the command and result without presenting the diagram as a rendered survey.

Static source and existing tests establish current implementation facts. They do not prove the proposed district's route feasibility, live-provider reliability, human enjoyment, full save performance or new guard behaviour. Those have explicit later gates. Recheck this inventory when the in-progress knowledge feature lands and before each affected milestone begins.
