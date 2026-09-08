Status: Reconciled source-backed M0 inventory (2026-09-07), including shipped knowledge M5, generated residents and conversation continuity. DTO/restore implementation remains M2/M3.

# Whole-world continuation inventory

M2/M3 must preserve the state that changes future behaviour, not only what is visible. This inventory assigns an initial policy to the actual current engine and to every planned subsystem. The owning module remains responsible for its DTO, validation and continuation tests. [CHECKPOINT_PROTOCOL](CHECKPOINT_PROTOCOL.md) fixes the capture, pending-work, clock and adoption defaults behind these rows.

**S** means save authoritative state. **R** means rebuild from saved state and an exact compatible content manifest. **T** means discard a transient handle/cache while following an explicit recovery rule. Several rows contain both S and R/T components.

## Existing authoritative simulation

| Owner/source | State | Policy and restore obligation |
|---|---|---|
| `world.rs` — `World` | Actor map, roster/insertion order, public/event/spatial counters | S. Preserve stable identities and ordering; begin a new host spatial/runtime generation without accepting old updates |
| `character.rs` — `CharacterSheet` | Authored/generated identity, control, appearance, lore/static context | S or exact manifest-backed reconstruction. Generated sheets and any runtime-mutated sheet values must be reproducible; a catalog lookup by name is insufficient |
| `CharacterState` | Position, yaw, presence and `presence_epoch`, economic class | S. Preserve who is actually present/departed and distinguish later visits by the same stable ID |
| `CharacterState` | Held-item order, pockets, gut entries, urgency/developer urgency | S. Validate quantities/reservations and unique-object lineage; no duplicate digestion on load |
| `CharacterState` | Goal, memories, known people/places | S. These are private and behaviourally significant; never route them through `PublicSnapshot` |
| `CharacterState` | Inbox, pending history, recent history | S, coordinated with scheduler in-flight inputs so unread events are not lost or applied twice |
| `CharacterState` | Movement path, speed, gait, patrol, choke wait | S with route/surface/content validation. Rebuilding a route may not reset an already completed journey or move a body through new geometry |
| `CharacterState` | Needs, travel intent, round edit, statuses, gesture deadline, departure flag | S. Rebase elapsed deadlines and preserve pending edits; cosmetic gesture presentation can be rebuilt from its remaining semantic state |
| `CharacterState` | Resolved daily-round descriptions and vendor list | R only where derived from saved effective rounds/vendor bindings; never regenerate from the original schedule after a saved edit |
| `World.household_doors`, `area_adjacency` | Household door registry and ward adjacency | S dynamic household bindings; R adjacency only from exact saved-compatible area geometry. Generated citizens retain the same door and derived home description |
| `world.rs`, `item.rs` | Items, quantity, kind and metadata | S. Validate catalog/version, uniqueness, positive quantities and ownership |
| `World.offers` | Giver, target, item, quantity and creation/order state | S. Preserve an actual pending offer and its commitments; revalidate on later acceptance normally |
| `inventory.rs` | Restock shares, active transform jobs and reservations, completed job IDs/results | S. A transform completion or historical stock sweep cannot repeat after load |
| `World.needle_claim` | Narrow-crossing owner and expiry/progress | S. Validate owner and remaining duration; do not silently grant two owners by rebuilding the queue |
| `notices.rs` | Live notices, sequence, summons, warrants and delivered/confrontation state | S. M11 will migrate semantics; current expiry/settlement bugs must not be encoded as desired new behaviour |
| `custody.rs` | Records, officer/holders, confinement, station, grip, struggle attempts, release/review anchors | S. Preserve actual custody and controlling authority; host tether is a projection |
| `marks.rs` | Marks, identities, strength, authors/subjects, sweeps and per-day work | S. Rebind catalog via manifest; recreate rendered chalk without minting new marks |
| `dogs.rs` / `World.dogs` | Animals' positions, movement/behaviour state and deterministic counters | S where it changes later motion. Rebuild bodies/animation handles |
| `World.ward_moods` | Settled nightly ward mood | S. Do not ask the model to invent a replacement on load |
| `knowledge/` | Fact identities/sequence, live facts, seeded/held knowledge and views | S. Preserve identity and origin; rebuild indexes only from saved values |
| M2 `Knowledge` / private `Holding` | `next_key`, `next_sequence`, actor holding key/hops/heat-at-learn/learned-on/from/view, air `Drift` heat/hops/via/stir, `last_sweep_game_days` | S. Preserve the actual saved fact-key mapping or explicitly remap every reference. Do not reroll stir, recompute acquisition heat or reset the sweep clock |
| M2 `Fact` social context | `quiet_among`, `craft_ear`, seeded origins and claimed-source data | S as recorded on the fact. Recomputing mint-time context from today's household/trade may change later spread |
| M2 engine/round polling | `Engine.next_player_pollen_game_days`, `Round.next_pollen` and `last_game_days` | S on the calendar basis, including valid Never sentinels. Do not reseed per-person polls or reinterpret old cursors through a new calendar rate |
| M2 derived spatial/salience support | `World.area_adjacency`, salience table, `pollen::WardGrid` process cache | R from the exact accepted content/geometry/behavior manifest. If later packs can change those inputs in-process, key/invalidate caches by that identity rather than keeping the first world's `OnceLock` result. Persist explicit measurement behavior overrides when supported, not a default guessed from current config |
| Accepted knowledge M0–M5 | Air heat, hop/garble/provenance state, polling anchors, player learning, occasions, systemic consequences | S. The exact remaining field groups and time bases are reconciled below; do not hydrate only the original M1 store |
| `World.events` | Undrained domain events | Require and assert a complete event-flush capture boundary. Persist resulting committed receipts not yet consumed by the host; never silently drop an event buffer that unexpectedly remains nonempty |
| `World.spoke_this_turn` | Same-turn seizure guard | T only at a guaranteed completed-turn/transaction boundary; capture must never cut a multi-action application in half |
| World catalogs/maps | Items, facts, marks, sounds, areas, shelters, places and nav | R through an exact content manifest, except genuinely mutable place/catalog state which needs its own S record |
| World switches/config | Sound, view cone, knowledge/marks settings affecting simulation | S for world behaviour; global user device/provider preferences remain separately configured |

## `Round` — state outside `World`

`crates/cathedral-sim/src/round.rs` owns much of the actual city. Serialising the character map while reseeding this service would recreate queues, stock, housing settlements and duties incorrectly.

| Group | Actual fields/examples | Restore obligation |
|---|---|---|
| People and daily work | `people: BTreeMap<ActorId, Townsperson>`, current phase/leg, home/work bindings, next decision, leash, lag, `epoch`, `evening_seed`, `excused` and active errands | S, including private per-person progress. Keep the saved effective schedule, incarnation and own ladder timing |
| Water | Sources, queues, current service, serving/sound deadlines and per-citizen state | M2a3 S exact occupancy/progress and geometry, validated against installed definitions. Do not refill thirst, reorder a queue or duplicate its service sound |
| Markets | Stalls, counters, bindings, market errands, closed visits and stock plans | M2a3 S exact live sessions/reservations/bindings/visit IDs and resolved definitions; shared pure validation binds retained definitions to the exact manifest |
| Production | Plans, start counters, `production_last_game_days`, eligibility samples, stall-watch state | S. Preserve integrated progress and once-per-office/day starts; validate active jobs against inventory reservations |
| Household economy | Reserves, Watch/settlement day markers, poverty streaks and accounting totals | S. Reloading before/after a day boundary must not pay or redistribute twice |
| Road parties | Members, phases, cargo, routes, departure/re-entry state, departure grace and cash totals | S. Stable membership/presence epochs and cargo survive; departed actors do not respawn from the original seed |
| Weather reactions | Shelter intents, lightning reflex deadlines | S with correct elapsed/calendar basis; R static shelter definitions |
| Service refusal | `chalk_refused_until` and similar backoff state | S. Otherwise reload lets actors immediately repeat a refused operation |
| Lamps | Lit state, targets, night day, Belwyn selections and revision | S. R host lights; do not reroll tonight's dark lamp or relight every post |
| Calendar anchors | Processed calendar cursors replacing unsafe `last_office_now` interpretation, `last_game_days`, `seeded` and other once-per-crossing marks | S in their explicit time domain. A scale change cannot reinterpret the past through a new slope; hydration is not a second `Round::seed` |
| Worksites and resolved data | Worksites, resolved trade/production specs, counter groups | M2a3 S exact retained worksites/plans/trades/groups, with shared pure installed-definition validation; do not recreate a missing planner |
| Traces/scratch | `food_log`, `observed_cart_loads`, `departed_this_tick`, `ladder_scratch` | M2a3 S food log, cart-load observations and unread departure publications; only ladder scratch must be empty and is omitted. Whole Engine flush remains a later gate |

### Current generated-resident controller (2026-09-07)

`Round.residents` in `round/residents.rs` is an additional authoritative service. Generated sheets come from `generate_ambient(nav, count, first_index, occupied, overrides)`, including patch/spot/housing assignment; `first_index` affects stable actor IDs and `overrides` contains worker declarations. Preserve the generator/content manifest separately from any world seed. M2 owns its private DTO together with Round; it cannot be recreated by placing the same number of new people.

| Exact field group | Time basis and policy | Validation / continuation obligation |
|---|---|---|
| `Residents.people`, per-resident `patch`, `preferred_spot`, `spot`, `target`, `phase`, `retiring`, `support_was_eligible`, `missed_meal`, `recovery_remaining` | S. `recovery_remaining` is elapsed duration; eligibility/missed-meal are semantic interval state | Preserve actual interrupted/returning/sheltering state and household support. Save stable patch/spot IDs or explicitly remap indexes against the exact resident-place manifest. |
| `Resident.dwell_until`, `epoch`, `weather_retry_until` | S elapsed deadlines; `epoch` is opaque deterministic sequence | Rebase deadlines, not epoch. Loading cannot reroll a lingering citizen or reset a failed path's recovery budget. |
| `Resident.weather { slot, intent, arrived }`; `WeatherShelterIntent { shelter, target, release_threshold, below_since_days, release_after_days }` | S active claim and hysteresis; suffix-days values use calendar time | Exact shelter/slot binding, preserved arrival and release hysteresis; invalid/missing compatible shelters reject hydration. |
| `Residents.reservations`; `SpotReservations.owners`, `.actors` including occupied/target claims | M2a3 S exclusive reservations and `.positions`, with exact streaming validation against resident-place geometry | Bidirectional owner/actor agreement and compatible live resident. One actor may hold its occupied and destination alias; different actors cannot share either claim. A target cannot become occupied early at restore. |
| `Residents.order`, `cursor`, `optional`, `departing` | S fairness/admission/departure sets and cursor | Preserve bounded optional movement admission and fairness. Pending departure releases its reservations exactly once. |
| `Residents.changed`, `local_shelters`; `Resident.projection`; `CharacterState.resident` | M2a3 S exact publication set, shelter rows, projection sample and published status; R pure consistency validation | Validate status from the last sampled inputs, not a later interrupted controller or moved actor. Stream-validate shelter geometry; update only on the ordinary owner pass. Host initial publication remains a separate adoption policy. |
| `Townsperson.motion_cause` | M2a3 S exact diagnostic attribution | Preserve current cause separately from the last sampled resident projection cause; never infer a newly started route from rebuilding a display. |

M2 must continue a world while someone lingers, another owns an optional route, another waits to return, and a weather shelter is occupied. Compare control/restored reservation ownership, displacement, needs/support, next decision and fairness. This is additional to the older occupational queue/production/road-party tests.

## Engine time, scheduling and external work

| Source | State | Policy |
|---|---|---|
| `clock.rs` — `WorldClock` | Seconds/day, epoch/origin segment, scale, brightness | M2a1/M2a4 S exact live descriptor plus initial config-clock provenance. Accepted logical time is independent of a replacement process origin; never reinterpret the old segment through the new slope |
| `weather.rs` — `WeatherTimeline` | Config/climate, forced inherited water/anchor/revision, residue, next override revision | M2a4 S every private field. Strike identities are computed from the saved timeline, not a mutable strike sequence. Exact sampled World/Engine weather and processed cursor are S; pure recomputation validates rather than replaces them |
| `Engine` | `last_clock_days`, `last_weather_days`, `last_weather_sample`; movement/round cadence and other elapsed owners | M2a4 S exact climate/clock cursors and sampled fields at the supplied boundary. Movement cadence, host residual assembly, player sound and other temporal owners remain explicitly pending with their own sentinel policies |
| `Engine` | `bell_strokes`, `bell_seq` | M2a4 S exact future logical deadlines in order, including equal/interleaved old-slope obligations, and sequence identity. Old strokes drain even with ring/sound flags disabled; no dropping or reconstruction at decode |
| `scheduler.rs` | Order, round-robin index, priority handoffs, protected player reactions, delay/backoff/running state | S. Retain obligations and fairness; do not reset the cast to the first actor after every load |
| Scheduler in-flight request | Semantic obligation ID, actor/presence epoch, lane/fairness position, drained inbox, presented/pending history, prompt/resolved input receipt | S semantic input/obligation; T external job. Restore one load-specific retry, including an empty-inbox idle turn; generic failure/idle-requeue helpers are insufficient |
| Scheduler held completion | Finished but unapplied exact success/error completion and its input receipt | S. Hydrate a held result without a provider request; normal current action validation still applies. Do not restore drained inputs to the inbox and then also apply the completion |
| `floor.rs` | Awaited presentation IDs/deadlines, foreground/background pacing, microphone hold | Preserve owed readable presentation/semantic speech state; T old audio acknowledgements and transient recording holds. Rebase legitimate pacing without blocking the city |
| `speech_router.rs` | Streams, parked recording, transcript jobs, utterance timing, input purpose/version, captured audience coverage and backend mapping | S committed text/utterance identity and supported observation history; T live capture/socket/job handles. Interrupted recording becomes available unsent draft/status, never an automatic new `say` or public-chat fallback. M5 extends the format for temporal hearing receipts |
| `night.rs` | Duty ID, owed day, subject/incarnation, queued/submitted/completed/dropped status, input, bedtimes, retry/yield anchors, totals and ambient day guard | S. Current `last_reflected` is stamped at queue time, not completion; restore queues directly without ordinary `enqueue`. Retry only within the saved duty's restored valid window |
| `Engine` social scheduling | Warm exchanges, last player partner, novelty/context counters | S. Preserve novelty's opaque `Memory.visit` bit salt unchanged and rebase `touched_at` separately. Supersede the old “do not save novelty” comment with this behavior-backed policy |
| `Engine` presentation caches | Last snapshot/law/chalk/lamp revisions, dogs-published, ready flag | T/R. Force complete initial publication to the new host without replaying world creation |
| `Engine.transcript` | Session transcript | Preserve the player's durable readable history through an explicit transcript/journal policy; developer omniscient logs may remain session artefacts. Do not conflate the two |
| Provider traits/env | Cognition/STT/TTS/Sight handles, prompt environment, capabilities | R using current services and validated content. No sockets, threads, secrets or old job handles are serialized |

### Accepted knowledge and conversation fields (2026-09-07)

These rows supersede the earlier generic social/knowledge descriptions; they are an exhaustive grouping of `Knowledge` and `Conversation`'s fields at audited HEAD. Existing World/Round/host owner rows still apply to their other state.

| Owner / exact fields | Time basis and policy | Validation / continuation obligation |
|---|---|---|
| `Knowledge.live`, `by_id`, `next_key`, `next_sequence`, `holdings`, `air` | S canonical facts, counters and private holdings/air; R `by_id` with exact saved keys | Unique IDs/keys/sequence, compatible actor/ward references, current count caps; sealed `FactSource` exported only through private owner DTO. Fact includes seeded/own/quiet/craft context, mint calendar time and source. |
| `Holding { key,hops,heat_at_learn,learned_on,from,view }`; `Drift { heat,hops,via,stir }` | S; learned-on/mint use game days, stir is opaque roll identity | Do not turn garbled hearsay into canonical firsthand truth or reroll a carried view. Keep first-hand seeded defaults distinct from stored override. |
| `Knowledge.last_sweep_game_days`, `last_hearsay_beat_game_days`; `Engine.next_stage_hop_at`, `next_player_pollen_game_days`; `Round.next_pollen` | S sweep/player/round polling on game-day basis; stage hop on elapsed basis | Preserve Never sentinel; restore cadence without duplicate receipt/hearsay or clock-rate-dependent extra pickup. |
| `Round.pollen_due` | S exact due index, with R validation against `next_pollen` and canonical signed deadline/Never keys | Live deadlines must have their matching entry; legitimate stale re-enrollment entries remain until the ordinary pop. M2a3 preserves index cadence rather than dropping stale rows at decode. |
| `Knowledge.occasions`, `Occasion { subject,from,at_game_days,offered }`, `raises` | S occasions and `(day,office,count)` quota | Offered occasion belongs with scheduler's pending prompt; an external retry does not restore already spent quota. |
| `Knowledge.player_learned`, `LearnedHow { word,at,place,from,hops,tellings,wards,wards_seen,mouths_seen,unattributed_seen }`, `receipts_revision` | S recorded words/acquisition and distinct-mouth bookkeeping; `at` game days | Maintain max 64 ordinary receipts and dedupe across A→B→A tellings. An unknown source stays unknown after load. Future retained history follows M5's archive extension, not silent pinning of this cap. |
| `Knowledge.seated`, `player_stage_stir`, `player_stage_tellings`, `hearsay_raised` | S pending reheat seats and deterministic dedupe | At an asserted fully completed prompt boundary `seated` may be absent; otherwise save actor/key inputs with that pending request. Never emit another notice from the same fact/wrong subject or count an already accepted stage telling again. |
| `Round.knowledge_refused_until`, `knowledge_refused_buyers`; `Engine.door_shut_until` | S refusal-until on game days, experienced-buyer set, door diagnostic deadline on game days | Maintain first-refusal experience and alternative-stall selection; stale IDs pruned only under ordinary policy. Distinguish from elapsed `chalk_refused_until`. |
| `Engine.last_ward_heat`, `last_journal`, `last_journal_receipts`, `last_journal_at` | M2a5 S exact publication caches and logical/Never throttle anchor | Journal prose/standing can legitimately be stale until the ordinary one-second pass. Ward label/order/centroid are checked against exact installed derivation. Future host republishing must remain separate from simulation receipt/cadence mutation. |
| `Conversation.engagement { actor,at,reciprocal,witnesses }`, `invitation`, `focus` | S social evidence; `at`, invitation and focus since/last samples are elapsed anchors | Preserve incumbent player choice, 30-second warm interval, 10-second invitation, 0.65-second focus dwell/1-second freshness and max 128 prior witnesses. Rebase together; do not invent past dialogue for newcomers. |
| `Conversation.next_utterance`, `latest_applied_utterance` | S monotonic utterance ordering | Keep latest ≤ next. Restored stale completions cannot overwrite a newer group/name/gaze choice even when actor IDs match. Still fence external jobs with M1 runtime generation. |
| `CapturedAttention { captured_at,sequence,focus,engagement,invitation }`; `SpeechRouter.captures`, `TranscriptionTask.attention` | Preserve already committed speech attribution; T in-progress microphone/STT execution under checkpoint interrupted-draft policy | At most eight captures with 120-second expiry. Discard interrupted unsent attention/job handles rather than auto-speaking after restore; preserve counters and all earlier committed suffixes through inbox/history. |
| `SpeechSelection.witnesses` and rendered conversation suffix | S rendered percept in pending/inbox/recent history and saved in-flight prompt input | Attribution is event-time metadata, never rerendered from the newly loaded conversation partner. |
| `SpeechRouter` remaining `streams`, `parked`, `timings`, `recording_jobs`, `stream_jobs`, `tts_backends`, `next_job`, `stt_stream_grace_seconds` | T external handles/buffers; S readable committed/unsent text, input purpose and interruption status per checkpoint protocol; R provider configuration | Preserve category and user text, not a live socket. No replay of committed `say`, no automatic draft submission, new generation job IDs. Timing probes are diagnostics, not permission to keep speech active. |

`Engine.npc_exchanges`, novelty/context salts and pending floor/scheduler inputs retain their prior S policy. Backend prompt archives and omniscient transcript are not a substitute for these private semantic records. New archive/root types do not exist yet; M5/M9 extend this same inventory when they introduce them.

## Host continuation

| Source | State | Policy |
|---|---|---|
| `PlayerController` | Physical position, velocity, yaw/pitch, grounded/coyote/jump state, flight | S with a matching accepted physical sample and command watermark. New input sampling starts from that physical state |
| Host fixed/virtual clock | Fixed-step overstep/residual and origin, virtual-step policy | S residual independently from sim movement residual; bind origin at final adoption. Loading a mid-jump body must not change when its next physics tick occurs |
| M1 accepted-time coordinator | Ordinary unaccepted wall debt already accumulated at capture | S separately from accepted fixed/movement residuals. Initial publication stays at saved accepted time; later budgeted steps service debt. Offline/preparation/old-world elapsed time adds none |
| `PhysicalPosition` / render transforms | Authoritative current/previous physical sample versus interpolation | S physical value/baseline; R interpolated transforms. No interpolation from the previous world's location; do not capture throttled/interpolated `GlobalTransform` as authority |
| `LocalEngine` | Command/completion channels, fake staging, dead/start flags | R with a new runtime generation. Retire old queues; do not reuse old callbacks based only on actor IDs |
| `WorldMirror`, `MovementInbox`, actor entities | Snapshot/motion caches and render bindings | R from complete new publication. Clear old-world entities/queues consistently |
| Custody host state | Tether/grip/reflex/projection plus `strain` and `struggling_reported` | R constraints from law; S strain/latch with the command watermark or move both to sim authority. A queued threshold command must not double escape or reset progress |
| Doors, marks, lamps, weather, dogs/carts | Entities, animation progress and graphics resources | R visuals from saved semantic state; S physical door/operation progress belongs in the sim |
| `src/city/vermin.rs` | Seed/config, `announced_boil_night`, `last_percept_minutes` and any pending semantic swarm announcement | S or move semantic scheduling into sim DTOs. These generate `WorldSound`/NPC cognition; rebuilding visuals cannot reset them. R cosmetic bodies/scattering under a stated policy |
| `src/soundscape.rs` | `ScheduledSounds`, `CueCooldowns`, `WellSoundState`, `ClockSoundState`, `CivicBellState` | Preserve owed committed presentation or explicitly discard already-presented/transient cues; rebase/init clock edge detectors at the saved instant. The first new snapshot must not duplicate civic/well/curfew events |
| Chat/notebook/document UI | Draft text, input purpose, proposition version, selected learned records, interruption state, reading position, unsent evidence bundle | S as player-owned UI state where valid. Restore as drafts; never auto-submit after load, confer new knowledge or erase already committed speech |
| Speech/audio/microphone | Playing clips, buffers, device streams and callback IDs | T. Clear old presentation; restore readable committed speech and start new capture only by player intent |
| User preferences | Audio/input/display/provider settings | Keep existing preference persistence separate. Loading a world must not require its old provider key or overwrite current device preferences |
| Save service | Write queue, temporary files, selected slot and last completion | T IO handles; S slot metadata only after durable publication. A previous valid save remains until the replacement succeeds |

## Planned systems: mandatory additions at their own milestone

| Milestone | State that must join the checkpoint |
|---|---|
| M1 | Logical time origin, command identity/high-water state, semantic receipts and operation IDs |
| M4 | Surface-qualified positions/routes, dynamic topology revision, passage reservations and content/geometry version |
| M5 | Relevant observation receipts, last-seen position/time/source and retained continuity chains |
| M6 | Door/lock/bar/opening state, grants, scope/revocation, key patterns, loans and pending access operations |
| M7 | Placed/contained object location, unique identity/lineage, document version, examinations/copies/custody receipts |
| M8 | Undertakings, step receipts, actor/resource ownership, invitations, attendance, deadlines and interruptions |
| M9 | Confirmed accounts, submissions, admissibility references, findings, amendments and retained evidence links |
| M10 | Player drafts/read state, input purpose/version, interruption state and knowledge-limited presentation continuation |
| M11 | Matters, allegations, authority decisions, registry orders, issued finite mandates, acquired versions/recall tombstones, shared execution identities, independent hold deadlines/defaults, deliveries and dispositions |
| M12 | Assignments, known leads, search/escort/handover phase and ordinary duty ownership |
| M13 | Installed pack/version set, per-world seed markers and authoring compatibility manifest |
| M15–M18 | Fixed incident installation, cast strategy states, actual evidence locations, hearings, disclosure, payments, favours and fulfilment/amendment history |

## In-flight request policy to implement once

At capture, classify each outstanding request as unsubmitted intent, submitted/unfinished work, completed-but-unapplied result, or committed effect awaiting presentation. Save its semantic category and references.

At restore, preserve committed effects, restore exact held completions, and restore one retry obligation for submitted/unfinished work. Regenerate presentation without emitting world speech twice and fence all old external completions. Inboxes drained for a pending prompt are restored only if that prompt's result will not be applied. This policy belongs in shared scheduling/persistence, not in individual quests. The detailed protocol also distinguishes Night Office duties and presentation-only clocks.

## Validation obligations

Reject unsupported versions, invalid counts/coordinates, duplicate ownership, cycles, dangling IDs, impossible reservations, multiple exclusive duty owners and contradictory custody/order references before adoption. Validate content hashes and stable references before placing any restored body into geometry.

The [M0 field index](evidence/m0_baseline/owner_fields.json) records 323 named fields across 26 private owner structs. Run `uv run --cache-dir /tmp/alibi-m0-uv evidence/inventory_fields.py` from this plan directory to detect declaration changes; `--write` is an intentional rebaseline after reviewing the policies above. This selected-owner index is a review aid, not an exhaustive Rust parser or proof of a working DTO. Host/module policies above still apply outside its selected structs. Every added mutable field must be assigned a policy. Continue real behaviour across capture/restore boundaries to verify those policies; a serialization round trip alone cannot catch a schedule that silently reseeded.


## M1a accepted-time owner delta — 2026-09-07

These implemented fields supersede the original M0 `last_clock_now`/`last_office_now` names. The original M0 field capture remains historical; [the current M1a field capture](evidence/m1a/owner_fields.json) indexes the new selected-owner declarations. Complete DTO/hydration remains M2.

| Owner / fields | Disposition and continuation |
|---|---|
| `WorldClock.elapsed_origin`, `epoch_days` (new meaning), `scale`, `seconds_per_day`, `night_brightness` | S complete clock segment: `epoch_days` is calendar at accepted logical `elapsed_origin`, not calendar extrapolated to process zero. Restore the pair exactly; scale changes stamp it directly and preserve calendar bits at that instant. This owner is outside the selected 26-owner field index. |
| `Engine.last_clock_days`, `Round.last_office_days`, `NightOffice.last_office_days` | S calendar positions, never rebase against a host clock or project through a changed slope. Round stores `Option<f64>`: production seeding installs `Some`, while a virgin clock-less Round has `None`; hydration preserves this rather than reseeding. Legacy unseeded test fixtures adopt `clock.game_days(0)` at their first tick. Existing `movement_now`/round cadence remain elapsed anchors. |
| `NightOffice.last_ambient_reroll_day` | S optional absolute game day; preserves spent ambient semantic reroll independently from provider duties. |
| `timeline::AcceptedTime { elapsed, debt, wall }` inside host `LiveTime.continuation` | S logical elapsed duration, ordinary unaccepted debt, and measured live-wall duration. No process timestamp. Capture all three; adoption and offline preparation add no wall duration or debt. Their invariant is `wall = elapsed + debt` for a fresh timeline; retain any explicit initial logical-origin offset when restoring supported older origins. |
| `LiveTime.accepted_virtual` | S accepted virtual clock elapsed/origin/context, consistent with `continuation.elapsed`. This copy supplies Bevy Time after TimePlugin's wall update. Host fixed overstep remains a separate S value; it is not ordinary wall debt or the sim's movement residual. |
| `LiveTime.last_frame` | R diagnostic report, may reset on adoption; never drives next progression. |
| `LocalEngine.input_watermark` | S finite accepted incoming cohort count, including non-consequential inputs. M1b adds separate durable consequential identities; this count is not a replay ledger. |
| `LocalEngine.accepted_boundary { input_watermark, physical_sequence, position, yaw, elapsed }` | S coherent-capture witness with host body/controller/fixed residual and sim player state; R restored publication witness only after those owners match. No extra `poll` to recreate it. |
| `PlayerSpatialState.sequence` and sim `World.spatial_sequence` | S monotonic sample ordering. Every final boundary sequence exceeds every action-carried sequence in its frozen cohort; M3 must adopt host/sim together. Cached background pose/yaw may reconstruct from this sample. |
| `GateRuntime` mechanism progress | Existing authoritative physical progress is S. Its advancement now uses accepted `Time`, since it changes collision; elapsed progress cannot reset from only the current office at restore. Cosmetic projection may rebuild from it. |

`DueBoundary`, `LogicalTime`, `CalendarTime` and `ExclusiveDeadline<T>` are value adapters, not new stores. M2 validates supported time magnitudes as well as finiteness before hydration; these constructors alone do not admit arbitrary huge calendars. The 100 ms admission cap and 128-backend-result pump cap are behavior-version inputs. M1c still owns bounded callback channels/backpressure and generation fencing; the M1a finite cohort does not claim to solve all producer allocation bounds.


## M1b receipts and semantic work owner delta — 2026-09-07

These fields are implemented authority. M2 must preserve them at the M1a finite coherent boundary, not rebuild fresh command IDs from UI correlation strings. DTO/hydration and replay-after-load tests remain M2 work.

| Owner / fields | Disposition and continuation |
|---|---|
| `World.command_ledger` | S all 32 producer issued/high-water/compacted-floor values, next acceptance ordinal, recent and referenced retained entries, protected semantic roots. Preserve exact IDs, digests/version, times, typed outcomes and valid principal refs. Validate ≤4,096 recent/≤256 referenced/≤256 roots/≤32 producers, ≤1KiB actual encoded entries and ≤4MiB recent allocations, unique identities/ordinals, floors, supported counter magnitudes and owner references. |
| `CommandLedger.pending` | Transaction-local; must be empty at any completed poll/capture. A snapshot cannot cut through a ticket or partly applied provider reply. |
| `CommandLedger.updates` | R presentation notification IDs, drained by ordinary flush. Authoritative results are already stored; rebuild presentation without replaying effects. Referenced terminal records survive until the notification flush. |
| `BridgeHandle.issued: Mutex<u64>` | S HOST producer allocator, independent of the input watermark. Capture/adopt with the queued finite cohort and sim high-water; explicit HOST sends advance it after successful enqueue. Restore above every already issued/queued sequence, never allocate a colliding retry ID. Mutex itself is R. |
| `World.travel_actions`, `TravelIntent.receipt` | S actor→step binding plus optional receipt on the intent; validate mutual agreement, active owner, protected root and current subject. Terminal arrival comes only from an explicit movement signal. |
| `World.round_actions`, `RoundEdit.receipt`, `presence_epoch`, `teach_place_on_commit` | S pending actor→step binding, captured incarnation and deferred teaching policy. Preserve the exact pending edit; validate it against the actor and ledger. Never teach or mark Completed just because a save was loaded. |
| `NpcScheduler::InFlight.semantic`, `retry_work[actor] { semantic, presence_epoch }` | S semantic obligation across provider retries, separate from ephemeral execution RequestId. Preserve drained input, presented history, prompt, lane and exact held completion using the existing request protocol; successful root/step receipts prevent repeated committed history/actions. Failed external attempts are execution lifecycle, not a final semantic failure receipt. |
| `NightOffice::Due.semantic`, `Due.presence_epoch`, `Flight.semantic`, `owed_day`, `presence_epoch`, `held_result` | S allocated obligation/day/subject/incarnation and exact held success/error. Busy submit cannot recapture a new incarnation. Unallocated queued `None` duties retain subject/day; M2 duty inventory must assign/retry them once under the protocol, without generic failure on load. Terminal dropped obligations release protected roots. RequestId remains an ephemeral execution ID. |
| `SpeechRouter::TranscriptionTask.semantic`, `resolved` | S pending recording operation and captured attention/pose/task binding. `resolved` stages only synchronous completion until the outer receipt finishes and must be empty at the completed command/poll boundary. Submitted/uncommitted microphone/audio/text restores as the protocol's unsent draft; committed speech restores receipt/presentation without speaking again. STT RequestId/job IDs remain ephemeral execution IDs for M1c fencing. |
| `CommandId`, `OperationId`, `Receipt`, `Outcome`, `AffectedRef`, `LedgerEntry`, payload version 1 | S values referenced above. Canonical encoding, command SHA-256 domain tag, provider reply UTF-8 fingerprint/version 1/domain tag, producer IDs, byte/depth/step/entry limits and typed state/reason interpretation are behavior/manifest version inputs. M2 must reject unsupported versions and malformed references before adoption. |

New value types are IO-free. The memory-only hashing writer implements the serializer's sink interface and never opens files or sockets. The shared sim dependency addition is `sha2` with default features disabled and all versions pinned in Cargo.lock. This field delta also covers owners not listed by the selected 26-owner declaration index.

## M1c runtime and delivery owner delta — 2026-09-08

These fields enforce execution isolation now; checkpoint DTO/adoption is still M2/M3. A runtime generation is an ephemeral execution address, separate from the durable semantic identities and actor presence incarnations already inventoried in M1b. The [M1c evidence](evidence/m1c/README.md) records bounds and cleanup obligations.

| Owner / fields | Disposition and continuation |
|---|---|
| `EngineConfig.runtime_generation`, `LocalEngine.generation`, `AcceptedHostBoundary.generation`, process `NEXT_RUNTIME_GENERATION` | E execution identity, allocated checked and nonzero in the production host. Do not restore the old runtime number as authority. M2 preserves logical elapsed/physical sample/watermark from the accepted boundary; M3 issues a fresh generation at adoption. A restarted process has no surviving in-process producers; within one process no generation is reused. |
| `RuntimeEnvelope<T>`, command/event `InGeneration`, `PlayerIntent::InGeneration`, all speech presentation generation fields | E origin tags retained immutably through every delivery. Reject stale/nested transport before identity, pose, watermark or UI mutation. Saved uncommitted player text/recordings follow the unsent-draft protocol; do not replay stale queued messages into an adopted world. |
| `BridgeHandle`/`BridgeCommandSender.generation`, shared `active`; `BridgeInbox.generation`, `disconnect_reported` | E endpoint lifetime and lifecycle projection. M3 must create fresh endpoints; retire old producers before swapping resources. The M1b HOST issued counter remains S and must be adopted with its finite watermark, independently from these fresh endpoint fields. |
| `LocalEngine.publication_bytes`, `command_endpoint`; `PublicationAllocation.total/bytes` | E queue ownership/accounting. Complete the ordinary flush before accepting a capture boundary. Old retained envelopes remain charged until consumed/dropped, and must not be deep-cloned against one shared token. M3 owns old bundle disposal. |
| Backend mailbox `Shared.generation/active/usage/retirement`, `Queued` charge, `Lease.charge/fallback/failed`, `BackendSender.lease` | E delivery reservations. Never serialize channel/Arc/mutex state. Preserve unfinished semantic duties/results through their sim owners, then create fresh backend executions under the new generation. Reserved old terminal failures cannot become new domain outcomes. Test-only chunk latch is not runtime authority. |
| `HttpCognition` checked `next_request_id`, lane guards; STT/TTS job-bound delivery sender; realtime `SessionState.deliveries` | E per-runtime execution IDs/lifetimes. Numeric IDs may recur only behind a fresh generation fence. Realtime admitted keys/commit tombstones are old transport state; M2 preserves unsent recording/text or exact committed semantic outcome through the speech router, not a provider socket. |
| `FakeCognition.next_request_id/staged/prompts`, host `SharedCognition` generation | E bounded backend execution and diagnostic prompt history. Drain only into its fixed generation. M2 stores unfinished scheduler/Night obligations and exact held semantic results, rather than restoring old fake channel delivery. |
| `SpeechRouter.next_job` | E checked transcription execution allocation; refusal on exhaustion. Its M1b recording semantic roots, exact held results, deadlines and unfinished work retain their existing S policy. M3 binds retry execution to a fresh generation. |
| `MicrophoneService.generation`, shutdown/stopped ownership; native bounded capture/error/lifecycle queues | E device execution. Old service cannot create current PlayerIntent or SpeechPresented effects. M3 drains/drops lifecycle receivers before awaiting shutdown, and schedules filesystem/worker destruction separately. Existing enabled/backend preferences and unsent drafts follow host protocol. |
| `SpeechPresentationState.generation`, existing queues/ready WAVs/PCM, microphone suspension/ack fields, `StreamingPcmSource` fixed sample storage | E bounded presentation projections. Synchronize/reset before accepting new-generation messages. Clear old voice entities and captions without acknowledging new speech IDs or resuming a new microphone. Rebuild readable state from M3's restored publication, not an extra poll. |

No new durable domain owner is hidden in these queue counters. Existing M1a accepted time and M1b receipt/replay/semantic-obligation fields remain authoritative. Host/module policies above apply beyond the selected 26-owner index. The immediate retirement APIs retain owners; Worker/SessionDir destruction and synchronous microphone discard remain M3's explicit cleanup work, and deferred STT discard can retain disk files until session retirement.

## M1d operation/resource/duty owner delta — 2026-09-08

The [M1d record](evidence/m1d/README.md) gives the actual adapter, priority and retained-allocation bounds. M2 owns checkpoint DTOs, bounded decoding and adoption. `validate_continuation` is a pure candidate gate, not a loader or a restored-run test; it must run before a decoded kernel can be used. The generation is still the M1c execution fence, never the operation identity.

| Owner / fields | Disposition and continuation |
|---|---|
| `EngineConfig.operations: OperationConfig`, `FixtureDeclaration { id, adapter, position }`, `AdapterDeclaration { name, version }` | R immutable content/behavior manifest: at most 256 exact resource IDs (1–64 bytes, no controls), finite positions and registered `timed_fixture` version 1. Engine construction validates then compacts retained config allocations. M2 must compare declarations, refuse unknown/unavailable/version-mismatched adapters, and never replace one with an immediate-success placeholder. The v1 adapter accepts present NPCs already within one metre; it does not implement player-controller locking. |
| `World.operations: OperationKernel.active` | S at most 256 live instances, independent of the existing 256 protected semantic roots. `InstanceId(CommandId)` includes the initiating action's producer/root/step; `StepId { instance, index }` preserves that identity. V1 supports one logical step (`index = 0`); replan increments only plan revision. Future multi-step adapters require their own versioned validation. |
| `ActiveOperation.actor`, `presence_epoch`, `resource`, `adapter` | S exact stable subject, captured incarnation and resource/implementation binding. Validate present NPC, matching incarnation/declaration, unique actor and resource, legal producer/root/command step, live nonterminal receipt and protected root. An old presence incarnation cannot retain a claim after the ordinary boundary. |
| `ActiveOperation.accepted_at`, `last_observed_at`, `last_progress_at` | S `LogicalTime`, explicitly accepted elapsed seconds. Checked scalar serde rejects negative/nonfinite values; validate monotonic anchors and agreement with the receipt. Never substitute current process time, calendar time or load duration. |
| `ActiveOperation.recovery_deadline` | S `ExclusiveDeadline<LogicalTime>`, exclusive at its exact cutoff. Validate the stored representable deadline against accepted time and required work. No replan, restore, obstruction release or newer command renews it. Offline/adoption preparation adds no elapsed work. |
| `ActiveOperation.required_work`, `completed_work`, `retry_limit`, `retries_spent`, `obstruction_revision`, `plan_revision`, `obstructed`, `running` | S operation-wide continuation: measured work in seconds, ≤32 retries, checked revisions and pending obstruction. Blocked/out-of-range spans update observation time but add no work; `running` records whether positive work has begun (Accepted versus InProgress), not a promise that the current span is unblocked. Validate finite bounded work/recovery, progress below required work for an active instance, retry/plan agreement, and receipt state. |
| `OperationKernel.actor_claims`, `resource_claims` | R indexes reconstructed from active records, with exact one-to-one validation before adoption. No independent second movement owner. Validate the complete retained kernel under its conservative 2 MiB heap bound, including all strings and BTree indexes. |
| `OperationKernel.fixtures: FixtureState { declaration, completed_units }` | S completion counters, R matching immutable declarations. The actual elapsed-time adapter increments one counter before publishing Completed. Terminal operations are removed immediately; their final receipts remain in the bounded replay ledger. Loading counters/receipts never runs completion again. |
| `World.speech_actions` | R bounded shared-root ownership index derived from the SpeechRouter's at most eight accepted jobs/parked/resolved tasks. Rebuild and cross-validate semantic IDs/receipts before adoption; leaving this empty with live recording tasks would permit sibling fixture/travel cleanup to unprotect their roots. Successful, failed, silent, aborted and already-terminal result consumption release the corresponding binding through the shared helper. |
| `CommandLedger` protection / `release_finished_root` | Existing S roots now remain protected while any pending ticket, travel, schedule edit, recording or kernel instance owns them. Operation command controls invoke cleanup again after their own ticket is finished, so cancelling the last sibling cannot leak a root. Preserve owner/root agreement across all these modules. |

No new process clock, channel, IO owner, save writer or UI pause exists in the kernel. `Round.refresh_operation_needs` advances the existing shared `last_game_days` anchor at each accepted boundary while work is active, before duty validation; the later Round pass sees no duplicate decay. This reuses the existing resident/hearth support formulas and can change their sampling cadence while active. It adds no second need clock. The ordinary empty-kernel cadence remains unchanged.


## M2a1 private component owner delta — 2026-09-08

This is the first M2a review cut, not a complete save or load path. The
[coverage ledger](evidence/m2a1/OWNER_COVERAGE.md) keeps all omitted owners explicitly
pending. Existing S/R/T policies above still apply to them; they are not optional.

| Owner / fields | Implemented component policy |
|---|---|
| `CommandLedger` producers, ordinal, recent, retained, protected | S in strict v1 DTO records. Explicit arrays preserve structured IDs; duplicate identities/ordinals/roots reject before rebuilding maps. Issued and high-water remain independent; protected and recent records can legitimately predate a floor. Full u64 exhaustion restores as exhausted, without wrapping or resetting. |
| Ledger `pending`, `updates` | Must both be empty for owner export after ordinary dispatch and notification flush. Updates are not replayed; committed notifications already handed to the host remain a required complete-envelope owner. |
| Receipt principal references | Exact bounded typed historical/attempted IDs survive consumed items, removed marks and rejected absent targets. These do not assert current existence. Active owner references must separately resolve to current authority; the complete-envelope root inventory must exactly match protected roots. |
| `OperationKernel` active and fixtures | S strict v1 records, including all work/retry/recovery anchors and counters, with exact fixture declarations/adapters. R actor/resource indexes only from validated unique active records. Existing shared continuation gate now also rejects materially impossible elapsed work credit and false zero-work progress anchors. |
| `WorldClock` five fields | S exact original calendar/rate segment through `WorldClockDtoV1`; validate explicit calendar position at the same logical instant. No rounding to an office or reconstructing via normal creation. |
| `HostTimeV1` | S exact nanosecond elapsed/debt/wall and host fixed step/residual, plus sim movement residual. This is only the time component; host body/controller/custody, mechanisms, vermin, UI and presentation payloads remain pending. |
| Explicit logical/calendar anchors | S `Never` only for declared negative-infinity owner sentinels; finite logical time is nonnegative, calendar can be negative. DTO validators reject unsupported numeric magnitudes and nonfinite values. Opaque novelty bits are not time anchors and remain pending with their real owner. |
| `CompatibilityManifestV1` | Exact schema/content/geometry/behavior/generator/hash implementation identity comparison. The host's canonical manifest builder and complete asset resolver remain M2a2+/M2b work; std-hash identity must include toolchain/target/build dependencies. |
| `CheckpointBudget`, `Reservation`, `Admitted<T>` | Pure runtime admission primitives: at most one running/save/load/retiring cohort under 1 GiB shared bytes. A non-Clone Send-capable admitted component retains its charge through export/decode/encoding and disposal. These counters/tokens are transient coordinator ownership, not serialized city state. M3 must integrate all actual host/worker generations, cancellation and retirement. |

V1 component decode budgets cover input, escaped-string scratch, DTO and candidate
index allocations. Encoded size, conservative retained heap and reserved peak are
separate measurements. The real host capture/hydration/staging/frame gates remain
pending; component microbenchmarks cannot satisfy them.


## M2a2 private character/inventory/backbone delta — 2026-09-08

The [current coverage ledger](evidence/m2a2/OWNER_COVERAGE.md) supersedes only the
pending status of the following owner families. All other S/R/T rows remain
mandatory pending composition; `WorldBackboneDtoV1` is explicitly not World.

| Owner / fields | Implemented component policy |
|---|---|
| Full CharacterSheet and CharacterState | S exact separate records, including seed/history versus live ownership, generated lore/appearance, all private memories and ordered percept occurrences, perspective sets, body/gut/needs/statuses, movement/patrol/gait/choke, travel/edit/deadlines, presence epochs and economic state. No seed-loader defaults or implicit missing Options. |
| Daily-round/vendor/resident projections | S exact current values in this cut. Pending Round owner must supply the shared pure projection validator and any saved cache anchor needed to justify stale values; preserve first read and update only at the next ordinary pass. No reconstruction with Round::seed or silent empty projection. |
| Item/offer/restock/transform authority | S full kind/metadata/quantity and current holder references, promises, sorted legacy shares, ordered aggregate reservations/output plans/progress, globally unique job IDs and held completion lineage. R candidate owner/commitment validation maps under admission. Historical completion IDs may no longer exist; live inputs must be held by the producer. Source/recipe/day/slot/work compatibility still needs the saved Round planners. |
| ItemCatalog | R exact supplied immutable catalog by SHA-256 over full schema/definition content; refuse mismatch or unknown kind/invalid metadata/stackability rather than falling back. |
| PlaceRegistry | S ordered entries and saved home owner/ID bindings. R by_id, first-normal-entry by_name, home_by_owner and owner_by_home indexes. Preserve name collisions and home exclusions; no home ID rehashing. Full static nav/place/home compatibility remains a complete-manifest gate. |
| World roster/counters/reference slice | S roster order, character/inventory/place/door bindings, round/travel command ownership, world/event/spatial counters, sounds/view-cone, sampled WorldTime, Needle claim, same-turn speech marker. Reject unflushed event/dispatch/receipt updates. All other World and Engine fields remain mandatory later owners. |
| Nested allocation admission | Streaming nonallocating preflight before typed parsing/cloning, followed by an attached aggregate peak reservation. Covers small/empty records, strings/escapes, concrete inline nullable records, BTree/HashMap/Vec indexes and serde Content/error scratch. Public new DTOs do not expose Deserialize. See the bounded allocation argument and actual +2,000 component peak in the linked evidence. |

No full-envelope capture, Engine hydration, pending IO retry, host adoption or
save slot is provided by this cut. Numeric clock-format limits are not accepted
effective-rate CPU limits, and measured component CPU costs require future
host offload/incremental coordination rather than synchronous frame execution.


## M2a3 Round/component delta — 2026-09-08

[The complete owner ledger](evidence/m2a3/OWNER_COVERAGE.md) supersedes the prior pending Round rows for this covered component. Save all Round, Residents and SpotReservations authority and publication buffers; require only ladder scratch to be empty/transient. The new resident projection sample is saved authority, so the exact shared projection can be validated despite a later speech interruption or motion change. Daily/vendor prose uses the ordinary pure writers. Typed records remain strict and mandatory.

Retained recipes/worksites/stalls/source geometry bind installed definitions; dynamic legs, queues, bindings and historical markers retain their cadence. Manual transform IDs/recipes and non-`legacy_stall:` provenance remain Inventory-owned: a known spec name is not proof of generated job origin. Missing production plans/specs leave stranded jobs for the ordinary watchdog, and old jobs need not have a day counter pruned to today-1. Both pollen map and stale-capable index are saved; initial Never has signed numeric ordering that cannot busy-loop before calendar zero.

Borrowed backbone references allow candidate-to-candidate validation without seeding a World. This does not finish general World/Engine geometry/time/root ownership. The clock-format range is not an effective-rate CPU gate; accounting totals have next-pass headroom but a complete time/rate horizon still needs validation. Full envelope, other owners, capture/adoption and host coordination remain pending.

## M2a4 climate/clock component delta — 2026-09-08

[Complete coverage](evidence/m2a4/OWNER_COVERAGE.md) and [bounded admission](evidence/m2a4/ADMISSION.md) now cover private WeatherTimeline, sampled World climate and the Engine clock/weather/bell component. World.current_time and sounds_enabled are checked copies of M2a2 backbone authority. World.current_weather and Engine.last_weather_sample remain exact saved values, validated against the private timeline rather than silently refreshed. Bare World None values, config-forced/null anchors, disabled overrides, inherited water/residue and revision wrap retain their actual owner meaning.

Full ordered resolved sound/ambient, area and shelter definitions and the exact original navigation inputs are bound as supplied context. This is not complete geography/catalog resolver validation: place/home/patrol/worksite/generated bindings, knowledge area-adjacency, downstream catalogs and all other World/Engine owners remain mandatory. Movement/round cadence, host debt/residual assembly and scheduling/speech duties are not included in an apparently complete envelope. No constructor or seeding runs during component decode/candidate conversion.

The narrower supported WeatherClimate numeric policy is explicit; accepting a finite clock descriptor does not certify an effective rate. Office and forced-lightning catch-up enumerate before truncation in unchanged ordinary source. Full-envelope temporal/accounting horizons and Running/retiring coexistence must be resolved before adoption. M2a2 plus Round already exceed the unchanged 1 GiB ceiling under naive simultaneous standalone reservations; the new small climate component does not resolve that composition problem.

## M2a5 knowledge/geography component delta — 2026-09-08

[Complete field coverage](evidence/m2a5/OWNER_COVERAGE.md) and [admission](evidence/m2a5/ADMISSION.md) cover all private Knowledge fields, World knowledge switches/area-adjacency and the Engine pollen/journal/ward-heat/door-timer component. Exact `by_id` rows are saved and checked against the sparse live keys, not regenerated by catalog seed. Holdings and air are deep-owned in an admitted cohort; sealed provenance has only private wire adapters and generic malformed-source errors.

Area keys bind the exact ordered installed AreaMap. Catalog and salience bind full resolved definitions, including unseeded pack rows; compiled homes/constants bind the ward grid. Default bare-World adjacency remains default, while initialized adjacency is checked against the exact pure derivation. World time is a checked backbone consistency copy. Household doors remain M2a2 authority with the M2a3 owner agreement; M2a5 fingerprints the supplied copy after finite-point/identity validation. General geography still requires complete place/home/patrol/worksite/generated and downstream resolver composition; no full World manifest is claimed.

Player receipts and seated keys survive fact invalidation. Absent historical mouths and stale sealed references are preserved until ordinary source truth checks. A receipt permits 64 distinct mouths plus one unattributed telling; receipt count remains 64. Sweep/stir/revision wrap and offered/expired occasions preserve actual owner semantics. Door deadlines use signed calendar days and Stage-only pruning; each caller binds immutable Engine.player_id. Journal cache state and its logical throttle are saved, rather than re-created during decode. CrossingTally is the headless runner's diagnostic host state; future host reset/new-run policy remains explicit and outside this component.

Conversation/CapturedAttention, pending provider/request authority, scheduler/Night, remaining Engine config/cadence, law/custody/notices/marks/animals and complete envelope adoption remain pending. No constructor/reseed/partial production install exists. Existing allocation and host-frame limits remain unchanged, and naive simultaneous component composition is already over 1 GiB before this cut. M2b must establish actual phase lifetimes, Running/retiring admission and full temporal/accounting horizons.
