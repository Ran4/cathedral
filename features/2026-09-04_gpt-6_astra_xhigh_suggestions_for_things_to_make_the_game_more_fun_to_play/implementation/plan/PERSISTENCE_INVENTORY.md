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
| Water | Sources, queues, current service, serving/sound deadlines and per-citizen state | S for occupancy/progress; R for immutable source geometry. Do not refill thirst, reorder a queue or duplicate its service sound |
| Markets | Stalls, counters, bindings, market errands, closed visits and stock plans | S for live sessions/reservations/bindings/visit IDs; R for immutable definitions from the exact manifest |
| Production | Plans, start counters, `production_last_game_days`, eligibility samples, stall-watch state | S. Preserve integrated progress and once-per-office/day starts; validate active jobs against inventory reservations |
| Household economy | Reserves, Watch/settlement day markers, poverty streaks and accounting totals | S. Reloading before/after a day boundary must not pay or redistribute twice |
| Road parties | Members, phases, cargo, routes, departure/re-entry state, departure grace and cash totals | S. Stable membership/presence epochs and cargo survive; departed actors do not respawn from the original seed |
| Weather reactions | Shelter intents, lightning reflex deadlines | S with correct elapsed/calendar basis; R static shelter definitions |
| Service refusal | `chalk_refused_until` and similar backoff state | S. Otherwise reload lets actors immediately repeat a refused operation |
| Lamps | Lit state, targets, night day, Belwyn selections and revision | S. R host lights; do not reroll tonight's dark lamp or relight every post |
| Calendar anchors | Processed calendar cursors replacing unsafe `last_office_now` interpretation, `last_game_days`, `seeded` and other once-per-crossing marks | S in their explicit time domain. A scale change cannot reinterpret the past through a new slope; hydration is not a second `Round::seed` |
| Worksites and resolved data | Worksites, resolved trade/production specs, counter groups | R only if exact definitions and stable bindings reproduce them; dynamic changes are S |
| Traces/scratch | `food_log`, `observed_cart_loads`, `departed_this_tick`, `ladder_scratch` | T or explicit flush boundary. An undelivered departure is a domain effect and cannot be dropped with scratch data |

### Current generated-resident controller (2026-09-07)

`Round.residents` in `round/residents.rs` is an additional authoritative service. Generated sheets come from `generate_ambient(nav, count, first_index, occupied, overrides)`, including patch/spot/housing assignment; `first_index` affects stable actor IDs and `overrides` contains worker declarations. Preserve the generator/content manifest separately from any world seed. M2 owns its private DTO together with Round; it cannot be recreated by placing the same number of new people.

| Exact field group | Time basis and policy | Validation / continuation obligation |
|---|---|---|
| `Residents.people`, per-resident `patch`, `preferred_spot`, `spot`, `target`, `phase`, `retiring`, `support_was_eligible`, `missed_meal`, `recovery_remaining` | S. `recovery_remaining` is elapsed duration; eligibility/missed-meal are semantic interval state | Preserve actual interrupted/returning/sheltering state and household support. Save stable patch/spot IDs or explicitly remap indexes against the exact resident-place manifest. |
| `Resident.dwell_until`, `epoch`, `weather_retry_until` | S elapsed deadlines; `epoch` is opaque deterministic sequence | Rebase deadlines, not epoch. Loading cannot reroll a lingering citizen or reset a failed path's recovery budget. |
| `Resident.weather { slot, intent, arrived }`; `WeatherShelterIntent { shelter, target, release_threshold, below_since_days, release_after_days }` | S active claim and hysteresis; suffix-days values use calendar time | Exact shelter/slot binding, preserved arrival and release hysteresis; invalid/missing compatible shelters reject hydration. |
| `Residents.reservations`; `SpotReservations.owners`, `.actors` including occupied/target claims | S exclusive reservations; R `.positions` from accepted resident-place geometry | Bidirectional owner/actor agreement, no duplicate occupancy or destination, present compatible actor. A target claim cannot become occupied early at restore. |
| `Residents.order`, `cursor`, `optional`, `departing` | S fairness/admission/departure sets and cursor | Preserve bounded optional movement admission and fairness. Pending departure releases its reservations exactly once. |
| `Residents.changed`, `local_shelters`; `CharacterState.resident` | R changed-publication set/status from saved controller; R nearby static shelter index | Dedicated full restored publication, no new behavior tick or knowledge grant. `ResidentStatus` remaining dwell is a projection, not the source deadline. |
| `Townsperson.motion_cause` | S diagnostic attribution, or explicit reset to unknown without changing the route | Retain if runtime evidence/inspection depends on the cause; never infer a newly started route from rebuilding a display. |

M2 must continue a world while someone lingers, another owns an optional route, another waits to return, and a weather shelter is occupied. Compare control/restored reservation ownership, displacement, needs/support, next decision and fairness. This is additional to the older occupational queue/production/road-party tests.

## Engine time, scheduling and external work

| Source | State | Policy |
|---|---|---|
| `clock.rs` — `WorldClock` | Seconds/day, epoch mapping, scale, brightness | S as a logical clock descriptor. Rebind to a new host origin without changing saved calendar time |
| `weather.rs` — `WeatherTimeline` | Timeline seed/config, forced overrides, residue, strike sequence and processed anchors | S for future-affecting values. R derived sample/visual state from the restored timeline |
| `Engine` | `last_clock_now`, `movement_now`, movement accumulator residual, `next_round_tick_at`, weather anchors, `last_player_sound_at` | S/rebase consistently; no duplicate calendar edges or missing fixed-step progress. Convert semantic `NEG_INFINITY` into DTO `Never`, as for knowledge/marks sweep cursors |
| `Engine` | Owed bell strokes and sequence | S for owed semantic/audio events, or explicitly mark presentation already committed; do not ring a new civic event twice |
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
| `Round.pollen_due` | R from `next_pollen` using exact finite/deadline ordering and valid Never policy | No missing/duplicate due actor; discard only entries proven stale by the saved authoritative deadline. |
| `Knowledge.occasions`, `Occasion { subject,from,at_game_days,offered }`, `raises` | S occasions and `(day,office,count)` quota | Offered occasion belongs with scheduler's pending prompt; an external retry does not restore already spent quota. |
| `Knowledge.player_learned`, `LearnedHow { word,at,place,from,hops,tellings,wards,wards_seen,mouths_seen,unattributed_seen }`, `receipts_revision` | S recorded words/acquisition and distinct-mouth bookkeeping; `at` game days | Maintain max 64 ordinary receipts and dedupe across A→B→A tellings. An unknown source stays unknown after load. Future retained history follows M5's archive extension, not silent pinning of this cap. |
| `Knowledge.seated`, `player_stage_stir`, `player_stage_tellings`, `hearsay_raised` | S pending reheat seats and deterministic dedupe | At an asserted fully completed prompt boundary `seated` may be absent; otherwise save actor/key inputs with that pending request. Never emit another notice from the same fact/wrong subject or count an already accepted stage telling again. |
| `Round.knowledge_refused_until`, `knowledge_refused_buyers`; `Engine.door_shut_until` | S refusal-until on game days, experienced-buyer set, door diagnostic deadline on game days | Maintain first-refusal experience and alternative-stall selection; stale IDs pruned only under ordinary policy. Distinguish from elapsed `chalk_refused_until`. |
| `Engine.last_ward_heat`, `last_journal`, `last_journal_receipts`, `last_journal_at` | R publication caches from saved receipts/standing; restart publication cadence on adoption | No new receipt/news to force a message. Publish the complete current learned view to the new host. |
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
