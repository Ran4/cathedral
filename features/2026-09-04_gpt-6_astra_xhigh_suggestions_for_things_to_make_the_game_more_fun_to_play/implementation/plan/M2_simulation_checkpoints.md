Status: In progress (2026-09-15). M2a1–M2a16 complete capture/validation and M2b quarantined hydration are implemented and reviewed. M2c pending-work restoration and M2d continuation remain.

# M2 — Complete simulation checkpoints

Restore an authoritative city, not a picture of one. This milestone implements pure export, validation and hydration; M3 supplies files and application controls.

## Entry

M1 is accepted and the persistence inventory names every current mutable subsystem. [CHECKPOINT_PROTOCOL](CHECKPOINT_PROTOCOL.md) specifies capture, pending-work and adoption defaults. Do not begin by deriving serialization on `PublicSnapshot`: that type deliberately omits private and operational state.

## Implementation slices

### M2a — Versioned DTOs and state inventory

Define an explicit checkpoint envelope with schema version, world identity, content/geometry/behavior/generator manifest, simulation/calendar position, private domain payload and host-continuation requirements. Use existing serde facilities first; measure before introducing compression or a different encoding. Represent valid “never processed” infinity sentinels as explicit DTO variants; reject invalid numeric values without rejecting virgin worlds.

Each owning module exports and validates its own DTO. Include `World`, `CharacterState`, inventory reservations/jobs, `Round`, clock/weather, knowledge, law/custody, marks, animals, scheduling and night work. Shared immutable catalogs are referenced through an exact manifest and resolved before hydration. Rebuild indexes only where their derivation is explicit and deterministic.

Admit capture/hydration work through the global count/byte budgets in [RUNTIME_BUDGETS](RUNTIME_BUDGETS.md). Measure extraction and index building as well as encoding; a plain DTO is not automatically cheap to obtain. Establish small versioned supported-save fixtures here so M3 can exercise the real file/adoption path against them.

Every field has one policy: saved authoritative state, re-derived state, or discarded transient presentation/IO state with a stated continuation rule. “Private field” is not a reason for omission. An automated field inventory may support review, but behavioural continuation tests are the proof.

#### M2a1 review cut — 2026-09-08

The coordinator accepted a smaller coherent first cut after source reconciliation:
strict `CommandLedgerDtoV1` and `OperationKernelDtoV1` owner exports/validation,
exact compatibility components, `WorldClockDtoV1`, explicit Never/time values,
`HostTimeV1` and attached count/byte admission. These are actual private owner
payloads, not an empty complete-save envelope. The [owner coverage ledger](evidence/m2a1/OWNER_COVERAGE.md)
names covered fields and all remaining M2a owners; [evidence/handoff](evidence/m2a1/README.md)
records the API boundaries and verification.

The five v1 supported **component** fixtures cannot be loaded as a saved city.
M2a2+ must compose every remaining owner into the exact versioned envelope and
supply small complete supported saves before M2a is accepted. The first cut does
not implement capture, Engine hydration, external retries, adoption or storage.

#### M2a2 review cut — 2026-09-08

`CharacterDtoV1`, `InventoryDtoV1` and `WorldBackboneDtoV1` preserve the real
private records, with strict mandatory nullable fields, exact catalog binding,
reconstructed registry indexes and aggregate admission before extraction or
parsing. This cut also fixes duplicate active transform job-ID admission.
The [current owner coverage ledger](evidence/m2a2/OWNER_COVERAGE.md) names every
covered family and the exact pending Round/projection/reference contracts;
[admission evidence](evidence/m2a2/ADMISSION.md) distinguishes bytes, conservative
expansion, retained peaks and the measured component phases.

Three additional supported component fixtures cover empty, live/private and
historically completed inventory state. Behavioral witnesses cover quantity
commitments, merge/consumption/completion replay, ordered private buffers,
movement/gait, and ordinary gut formation after restoring the covered body.
These do not establish whole-Engine continuation: Round, law, knowledge,
scheduler/IO and complete World/Engine/host composition remain mandatory work.
The proposed next coherent owner cut is M2a3 Round/residents/production/household/
road-party state against the actor/item/place foundation, after M2a2 acceptance.

The measured M2a1 ledger export/encode p99 of 7.186/11.884 ms already rules out
synchronous host-frame placement. M2a2 also preserves conservative cohort peaks
until disposal; the +2,000 diagnostic reserves 813,374,372 shared bytes excluding
Running. Complete-envelope coexistence and an effective clock-rate CPU limit
remain unproved. M3 must supply bounded offload/incremental coordination; no
component API is represented as satisfying a frame or complete-save budget.

#### M2a3 review cut — 2026-09-08

`RoundDtoV1` preserves the whole Round/resident/market/production/household/road-party component. Its opaque candidate validates against an unadopted M2a2 backbone and exact installed geometry/catalog definitions, without Round::seed or Engine::new. [Owner coverage](evidence/m2a3/OWNER_COVERAGE.md) records every field family, shared projection sampling/writer contracts and remaining composition. [Admission](evidence/m2a3/ADMISSION.md) includes raw-input admission plus explicit definition scratch and the failed whole-envelope coexistence inference.

Supported fixtures and continuation witnesses cover atomic water service, held/rebound market sessions, paused and manual/stranded production, household once-per-day markers, road departures/trips, signed-calendar pollen and resident fairness/support/weather/claim/projection state. This cut corrects nonpositive-calendar pollen ordering. Complete effective clock-rate/accounting horizon validation, all other private owners and full capture/hydration remain pending. M2a3 passes focused/public and full workspace verification (1,912 tests), plus independently audited release component and ordinary CPU measurements. [Coordinator review](evidence/m2a3/coordinator/review.md) accepts this component cut and records its unchanged allocation/frame limits. M2a4 private clock/weather and sampled World climate/context is next after commit; complete M2a and M2b remain pending.

#### M2a4 review cut — 2026-09-08

`WeatherTimelineDtoV1`, `WorldClimateDtoV1` and `EngineClimateDtoV1` preserve the exact private weather timeline, sampled World climate and clock/bell component. The live and initial clock reuse M2a1's descriptor; initial provenance is validated at its own origin, while the live segment and both processed cursors bind the saved boundary. [Owner coverage](evidence/m2a4/OWNER_COVERAGE.md) distinguishes World time/sound consistency copies from backbone ownership and lists all pending Engine cadence/configuration/scheduling fields. Candidate validation borrows unadopted backbone references and exact nav/shelter/area/sound definitions without seeding or partial production adoption.

Continuation witnesses cover scheduled/custom climate, stable lightning identity, inherited wetness, drying residue, config-forced/disabled/unanchored/wrapped states and old-slope bell obligations after a scale change. [Admission](evidence/m2a4/ADMISSION.md) adds an explicit 64 KiB sampling allowance to raw-input admission; no limits increase. The forced-storm probe is not a universal scheduled-climate or full-save latency bound. Existing office/forced-lightning catch-up allocation and all-consumer effective-rate/accounting horizons remain full-envelope gates. M2a4 passes final focused/public and full workspace verification (1,928 tests); [Coordinator review](evidence/m2a4/coordinator/review.md) accepts the component and completed release measurements. M2a4 is committed; M2a5 knowledge/facts/pollen and geography/area-adjacency context is reviewed below. Complete M2a and M2b remain pending.

#### M2a5 review cut — 2026-09-08

`KnowledgeDtoV1`, `WorldKnowledgeDtoV1` and `EngineKnowledgeDtoV1` preserve private facts, sparse stable handles, holdings/air, provenance, occasions/receipts/seated/dedupe state and the Engine's pollen/journal/ward-heat/door timers. [Coverage](evidence/m2a5/OWNER_COVERAGE.md) lists every owned field and the exact historical-reference policy. Context borrows either World or an unadopted backbone and binds ordered areas, full fact catalog/salience, compiled ward inputs and household-door authority. Exact initialized adjacency and immutable cached centroids are checked without seeding or warming a global singleton.

[Admission](evidence/m2a5/ADMISSION.md) retains lexical raw-input charges plus a combined 4,456,448 B allowance for sequential geography, adjacency and home-definition work. The active probe fills six stored holdings per actual actor and keeps historical receipts, seated occasions and publication caches; it makes no complete-save or synchronous-frame claim. Continuation uses a deliberately scrambled covered component in an independently prepared Engine, with immediate canonical equality before subsequent ordinary polls. Final focused/public and full workspace verification pass (1,949 tests); [verification](evidence/m2a5/verification.json) binds exact raw evidence to the final source. [Coordinator review](evidence/m2a5/coordinator/review.md) accepts the component and its 3,600 audited release samples. Export p99 is 5.201/18.358 ms; host integration still requires bounded offload or incremental coordination. M2a5 is committed as `61e3e1a`; M2a6 existing law/custody/notices is implemented and accepted below. Complete M2a/M2b and all other owners remain pending.

### M2b — Capture boundary and hydrate path

Capture after the ordinary complete simulation transaction/poll boundary, with a matching authoritative player physical sample, controller/fixed-step residual and a known command high-water mark. Assert the sim accepted that sample. Include committed readable receipts still awaiting host consumption. Do not capture half an item transfer or between custody insertion and its event. Do not wait for actors to finish an escort or for a provider call to return; never issue a save-only extra poll.

Construct the restored engine through a dedicated hydration path. `Engine::new` currently seeds rounds, knowledge, inmates and other authored state; running normal seeding over a checkpoint would duplicate or reset the city. Rebind immutable assets and services without replaying world creation.

Validate all IDs, item ownership/quantities, reservations, references, state variants, numerical values and time relationships before making the candidate usable. A corrupt reference must identify the failing subsystem and leave the current engine untouched.

Implemented and reviewed (2026-09-15): an admitted complete
LoadCandidate prepares a separate same-budget asset lease before its host factory
and decodes one fully checked private owner graph. Exhaustive World/Engine
literals move those actual owners without ordinary creation. The public
HydratedEngine remains quarantined, retaining typed speech interruption,
accepted cognition input and complete host continuation owners. It exposes narrow
observations and same-budget streamed hashes of the16 actual new owner categories;
no raw envelope remains and no Engine/World mutation, extraction or poll is public.
Original parsed WorldSeed, fresh PromptEnv and distinct config/World asset roles
bind through a fresh resolver. Saved lineage and logical/calendar state remain
exact; a supplied nonzero execution generation must differ from the saved fence.
The [owner design](evidence/m2b/owner-design.md) records construction, admission,
disposal, immutable-cache and same-image fixture contracts. Full M2c/M2d and M3
adoption/offload remain separate gates; synchronous hydration and the existing
non-Send Engine do not establish host-frame acceptance.

### M2c — Pending cognition and speech

Separate committed actions from unfinished requests. A submitted NPC prompt may have drained its inbox; preserve its input receipt and the obligation to respond. The default is exact held-result restoration for completed-but-unapplied success/error completions, and exactly one load-specific retry for submitted/unfinished work. Do not both restore the drained inbox and apply its saved completion. Preserve newer arrivals separately and retain lane/fairness identity. The existing idle-retry and provider-failure helpers are not valid substitutes: one drops idle work, the other creates false failure/backoff effects.

External sockets, HTTP jobs, audio handles and raw microphone capture are not engine state. Outstanding provider jobs belong to the old runtime generation. Completed, committed speech remains in transcript/knowledge receipts; audio can be regenerated or replaced with readable text without saying the line to the world a second time.

An unfinished player recording is not silently turned into a new utterance after load. Preserve any available text draft, report that the recording was interrupted, and require a new intentional submission. This does not prevent saving while the microphone is open.

Preserve Night Office duty identity, owed day, subject/incarnation and queued/submitted/completed/dropped state. Its current queue-time `last_reflected` stamp is not a completion marker. Hydrate queues directly, preserve the ambient once-per-day guard and retry only inside the restored valid window. Reload must not grant a second reflection, duplicate a settled memory or use an old result on the replacement world.

### M2d — Deterministic continuation

Build fixtures that run to a chosen boundary, capture, hydrate into fresh services, then execute the same subsequent inputs as an uninterrupted control. Compare canonical authoritative state and committed domain events, excluding declared transient presentation counters. Use fake or recorded cognition completions with controlled timing. Exact equality covers retained/recorded completions; intentional re-rendering of unfinished work instead proves obligation conservation and exactly-once application, since a live provider need not reproduce the unsaved future.

Test boundaries during a food transformation, market queue, road-party departure, warm conversation, pending offer, custody escort, weather transition, bell sequence, knowledge propagation and night reflection. Later milestones must add their own continuation cases before acceptance.

## Time and identity requirements

- Preserve logical simulation time; bind it to a new host origin.
- Preserve calendar position/rate and every “last processed” edge, so daily work does not repeat.
- Keep stable actor/item/fact/order/operation IDs and ID allocation counters.
- Create a new runtime generation and reject old callbacks.
- Revalidate references against the exact content manifest; never guess replacements from display names.
- Preserve accepted but unfinished work and completed step IDs without accepting unsaved future effects.

## Tests

- Round trip a populated world with all present subsystems enabled.
- Continue across every boundary listed above and compare with the control run.
- Inject duplicate holdings, dangling references, invalid quantities, non-finite coordinates, bad time anchors and unsupported versions; all fail before adoption.
- Feed late provider/speech callbacks from the discarded engine; all are inert.
- Restore with cognition unavailable; deterministic city activity and previously committed consequences continue.
- Capture at default cast, 2,000 extra citizens and 20,000 stress citizens; record capture CPU time, payload size and allocations separately from disk time.

## Completion gate

No current authoritative subsystem is missing from the inventory or continuation tests. Hydration does not call ordinary seed paths. Capture never waits on external services. The sim crate remains IO-free. M3 receives a validated checkpoint value and an explicit host-state contract.


#### M2a6 review cut — 2026-09-08

Strict private Notices and Custody records, mandatory WorldLaw composition and exact Engine law-publication cache are implemented. [Owner coverage](evidence/m2a6/OWNER_COVERAGE.md) records every field and separates active holder/escort bindings from historical served names, settled/expired notice links and frozen Station copies. World.spoke_this_turn remains saved backbone authority for both seizure and seated-fact reheat; full pending-reply agreement is still required. No expiry, release, seed or partial production adoption runs during decoding.

[Admission](evidence/m2a6/ADMISSION.md) adds a fixed 4 MiB borrowed registry/reference allowance without raising any global cap. Raw charges remain attached through candidates. Fixtures and component-only Engine continuation preserve exact private state, immediate canonical bytes, ordinary grip/sentence/notice behavior and cached publication cadence. The active six-phase `alibi_law_cost` probe uses actual authored/+2,000 placement and retains served, unissued summons, warrant, grip/closing and historical links. Owner full workspace verification passes (1,962 tests; 0 failures), with frozen-source developer smokes and lossless raw evidence. [Coordinator review](evidence/m2a6/coordinator/review.md) accepts five supplemental public boundaries, the unchanged owner workspace, admission/source audits and 3,600 release phase samples. Export p99 is 1.063/4.490 ms; populated capture requires bounded offload or incremental host coordination. Marks/catalog/switches and Engine chalk cache are the next coherent cut. Full M2a/M2b composition, all-consumer horizons and synchronous host placement remain unaccepted.


#### M2a7 review cut — 2026-09-08

Strict WorldMarks/EngineMarks components preserve the complete existing private Marks store, sparse identities, raw strengths/strokes, historical references, sweep/day clocks, switches and exact chalk publication cache. Initial Engine marks config copies remain distinct from later World state. [Coverage](evidence/m2a7/OWNER_COVERAGE.md) records orphan-anchor cleanup, spent-day scrub windows, calendar rewind semantics and exact IEEE scale storage. The immutable resolved catalog, registry and shelter context binds a World or unadopted backbone; no constructor or partial adoption is exposed.

[Admission](evidence/m2a7/ADMISSION.md) retains original raw charges and the 4 MiB context allowance under unchanged caps. New component fixtures, independent public boundaries and private World/Engine/ordinary Round continuation tests exercise the real medium; the frozen-source full workspace passes 1,986 tests with no failures and 23 intentional ignores. Exact archives and source hashes are recorded in the [reviewed handoff](evidence/m2a7/README.md). [Coordinator acceptance](evidence/m2a7/coordinator/review.md) includes all six public boundaries, unchanged ordinary source, admission/log audits and 3,600 release phase samples. Authored/populated export p99 is 1.110/4.411 ms; the populated cost requires bounded offload or incremental host coordination. Dogs, ward moods/Night and remaining Engine/scheduler/speech/pending owners remain separate cuts. Full-envelope lifetime admission, capture/hydration and host offload remain mandatory; M2 is not complete.


#### M2a8 design cut — 2026-09-08 (implemented; owner verification passed)

This next coherent component owns the existing ordered `World.dogs` vector and every Dog field: identity, authored/runtime prose, coat/build, base/leash, position/yaw/speed/gait, exact remaining path, signed rest seconds and wrapping decision epoch. Dogs remain separate from Characters. V1 uses closed explicit records and admitted opaque read-only candidates. Load does not call seed_pack, drift_path, route finding or ordinary ticks. DogView remains presentation only.

The Engine component adds exact `dogs_published`, immutable player binding, independent exact `config.nav` binding and the shared human/dog `movement_now` elapsed anchor. The anchor uses LogicalTime in the existing accepted elapsed coordinate system, preserving its exact float without subtraction/re-addition. This is its sole component owner; full Engine composition must use the same value for humans, dogs and custody sampling, agree with the host physical/residual boundary and separately restore Round cadence. Nav=None and historical backwards/coarse debug callers preclude a universal residual<TICK or movement_now<=boundary rule here. World.nav and config.nav can differ after public World edits and bind separately; neither can be silently substituted.

Supported dog identity is unique and valid within the ordered vector, with no requirement to be authored, prefixed dog_, or absent from actor names. Finite numeric fields retain signed and unnormalized values; points use the existing city coordinate bound. Do not infer empty-path implies zero-speed: arrival leaves the last positive speed for the next stop publication. Turn-in-place and partial acceleration retain exact paths; failed drift retries and signed rest timing retain the epoch, including u64::MAX, because ordinary stepping wraps it. The format supports at most 25,000 dogs and 65,536 waypoints per dog under the unchanged 128MiB expanded/encoded and 1 GiB aggregate limits; those format ceilings are not the measured ten-dog workload.

Borrowed context checks precede allocating identity-validation scratch. NavData is immutable and constructor-validated; bind its existing exact original JSON+bitset SHA fingerprint without reparsing or warming route caches. Player refs validate against live World or an unadopted BackboneCandidate. Explicit raw-input admission and retained leases follow M2a1–7, including whitespace/escape charges, mandatory nullable nav fields, strict duplicates/unknown fields and failure release. Closed-layout and sparse-container evidence will be recorded before review.

Private continuation installs only these covered fields into independently prepared, deliberately scrambled owners, asserts immediate canonical equality, then exercises resting, moving, turning, arrival/stop, retry/wrapped next drift and exact initial/deduped publication without extra cognition/revision effects. New golden World/Engine fixtures only; prior component fixtures remain unchanged. The six-phase alibi_animals_cost probe uses actual authored 520/+2,000 placement 2,520, real navigation and ordinary dog ticks/publication; tests cover rare/max cases outside the timed workload.

Complete envelope/build/target/toolchain/DefaultHasher binding, shared owner equality, Engine hydration, pending work and independent host initial republish remain future gates. Naive backbone+Round Save+Load already totals 1,256,093,444 B before Running, above 1 GiB: this cut cannot establish full phase/lifetime admission or synchronous host acceptance, and increases no global cap.


#### M2a8 coordinator review — 2026-09-08

Strict WorldAnimals/EngineAnimals components preserve all existing private Dog authority, exact independent World/Engine navigation bindings, initial publication state and the shared human/dog elapsed slice anchor. [Coverage](evidence/m2a8/OWNER_COVERAGE.md) records every field and its reachability policy. Resting, planted turns, partial acceleration, arrival's positive-speed empty path, failed retries, signed edits and wrapping MAX epochs continue without seeding or rerouting. Read-only candidates resolve against World or an unadopted backbone; no production partial adoption exists.

[Admission](evidence/m2a8/ADMISSION.md) proves the concrete 200-byte Dog, closed container growth and bounded sorted borrowed-ID index within a 1 MiB working allowance, retaining original raw charges under unchanged limits. The final 841-file source passes focused 104/public 6/layout 2 and serial full-workspace **2,004 tests, zero failures, 27 intentional ignores**. [Handoff](evidence/m2a8/README.md) and [verification](evidence/m2a8/verification.json) retain every raw command archive and source manifest. Two debug smokes demonstrate the real ten-dog living pack on authored 520/+2,000 placement 2,520; [Coordinator acceptance](evidence/m2a8/coordinator/review.md) includes source/admission/log audits and 3,600 release phase samples. Authored/populated export p99 is 0.030/0.032 ms; full-envelope and host adoption remain pending.

World and Engine navigation copies remain independently bound, and future complete Engine composition must share movement_now with human/custody movement and the host accepted residual. Dedicated initial host republish must not alter dogs_published or spend a simulation poll. Full envelope/build/target/toolchain/DefaultHasher compatibility, all-consumer numeric/temporal safety, phase/lifetime admission, capture/hydration and pending external work remain mandatory. Ward moods/Night and remaining Engine configuration/cadence/attention/conversation/scheduler/speech owners are next separate work; full M2 is not complete.


#### M2a9 implementation cut — 2026-09-08

Implement the existing NightOffice owner, independent World.ward_moods and original Engine.config.night_office composition as opaque admitted components. Preserve the ordered queued duties including admission identity/incarnation/owed day, exact submitted prompt/request/root, optional exact held success/error/duration, queue-time daily stamps, resolved historical bedtimes, office and ambient guards, pacing/yield anchors, seeded/config flags and both run counters. No constructor reseeding, ordinary polling or production adoption belongs in decoding.

Bind the exact live clock and Night semantic roots against borrowed World or unadopted backbone/ledger context. Do not require historical subjects to remain present or clean up obsolete duties on decode. Resolved bedtimes remain their saved authority rather than being recreated from Round. Positive-infinite pacing reached through a tiny positive clock scale must survive as an explicit Never anchor, including after the clock changes back; preserve finite future anchors exactly. Supported count/text/counter headroom and sparse-layout bounds must be explicit, under the unchanged global admission ceilings.

Add only new Night component fixtures. Prove immediate canonical equality after private field installation into independently prepared, scrambled owners, then continue ordinary queued Busy/admission, held-result capacity release, stale incarnation and obsolete day, queue-time guard, ambient daily work, clock pacing, ward mood/round/mark/receipt behavior. Completed successes/errors and unfinished exact prompts remain obligations; full M2c still owes held-once restoration and exactly one load-specific unfinished retry with runtime-generation fencing.

Measure six component phases on actual 520/2,520 actors with real navigation and ordinary Night work, separately from max-supported shapes and rare test-only conditions. Complete-envelope ownership/time horizons, Running/Save/Load/retiring lifetimes, M2b capture/hydration, M2c pending adoption and host/renderer acceptance remain pending. Earlier backbone+Round naive Save+Load is already 1,256,093,444 B, above 1 GiB before Running; this cut must not raise any cap.


#### M2a9 coordinator review — 2026-09-08

Strict NightOffice/WorldNight/EngineNight components preserve ordered duties, queue-time daily guards, historical bedtimes, exact submitted input/root/incarnation, held success/error, pacing and counters, independent World ward moods and original Engine Night flags. [Coverage](evidence/m2a9/OWNER_COVERAGE.md) and [admission](evidence/m2a9/ADMISSION.md) document exact clock/root binding and closed layout/scratch bounds under unchanged limits. Read-only candidates do not seed, rerender prompts, prune old work or partially adopt production state.

The final 852-file source passes focused 119/public 6/layout 1 and serial full-workspace **2,025 tests, zero failures, 29 intentional ignores**. The first workspace link failure during disk exhaustion and every earlier development failure remain in the [exact archive record](evidence/m2a9/commands.json); the unchanged-source retry succeeds after disk recovery. [Coordinator acceptance](evidence/m2a9/coordinator/review.md) includes source/admission/log audits, actual held/deferred and ambient continuation, plus all 3,600 release phase samples. Authored/populated export p99 is 0.048/0.036 ms. Both modes reserve 8,864,336 B for Save+Load excluding Running.

Full M2c must consume completed saved success/error exactly once without another provider request and retry unfinished submitted input once with its saved obligation and runtime-generation fence. Complete owner/root/manifest agreement, all-consumer horizons, actual cohort lifetimes, capture/hydration and host initial publication remain pending. Earlier naive backbone+Round Save+Load alone still exceeds 1 GiB, so complete integration must solve phase/lifetime admission and host scheduling without increasing caps. Conversation/attention and remaining Engine/scheduler/speech owners are subsequent cuts; complete M2 is not accepted.


#### M2a10 design cut — 2026-09-08

Preserve all existing Conversation, WarmExchanges and Novelty authority, plus the original Engine idle_mode, StageConfig, idle_requires_news and CuriosityConfig values. These decide whom the player is addressing, which people hold their place during an exchange, and whether an on-stage person has already seen the current context. They are save state even though old attention module comments call them derived. Update those comments without changing ordinary behavior. Add closed explicit V1 records, opaque admitted read-only component candidates and Engine composition. Do not add a partial production installation path or seed/recompute state on decode.

Conversation retains optional engagement(actor,at,reciprocal,witnesses), independent invitation, partial focus(actor,since,last), next_utterance and latest_applied_utterance. Preserve capped witness history (128) and saturating u64 tokens, with latest<=next as the accepted inventory gate. Absence, stale focus/engagement/invitations, and historical actor IDs are retained until ordinary consumers act. Do not invent target-is-witness or current-roster membership requirements. Warm pairs are canonical unordered distinct endpoints and exact last-line anchors; no decode-time pruning. State an explicit supported pair count independent of the current cast. Novelty preserves map key and Memory(context Option<u64>,visit u64,touched_at): context digest and visit salt are opaque, including any u64 visit bits, never recomputed or reinterpreted as a timestamp. Strict duplicates, unknown fields, required nullable fields, count/ID/time bounds and canonical tuple identity apply before adoption.

Retain the accepted logical coordinate rather than subtracting and re-adding a host origin. Derive chronology gates from actual source and the explicit supported time domain. Focus methods can retain a since older than a backwards last supplied sample; do not silently normalize historical state. Engine immutable player identity must agree with borrowed World or an unadopted backbone. Exact full content/build/target/toolchain binding remains the full envelope's responsibility, especially for opaque DefaultHasher contexts. Save raw original Stage radius/Curiosity scale bits (NaN payloads, infinities and signed zero included) because their ordinary consumers apply deliberate fallback/clamp behavior. Preserve max_actors 0 and usize::MAX under a checked fixed-width representation; comparison against actual stage size is not an allocation capacity. Preserve actual scheduler order in its later owner rather than recomputing from initial mode.

CapturedAttention held by pending microphone/STT work belongs to the later speech interrupted-draft policy: this leg does not keep a live recording or add a speculative standalone capture DTO. Conversation token continuation may use controlled captured inputs in tests. Existing committed speech suffixes already belong to inbox/history/pending prompt authority; never rerender them from the loaded partner. SpeechRouter/floor/scheduler/external jobs and all remaining Engine cadence/config fields remain pending owner cuts.

Prove raw-input admission before clone/decode, retain charges through candidates, and document sparse-map/container/string layouts and scratch lifetimes under unchanged 128MiB encoded/expanded, depth64 and 1GiB shared ceilings. Prefer streamed borrowed records and direct unique collection visitors rather than whole intermediate JSON values or copied Worlds. Choose the smallest justified owner working allowance and show measured concrete layouts. This does not resolve the existing whole-save peak excess or synchronous host capture budget.

Private tests restore into independently prepared, deliberately scrambled social owners and require immediate canonical equality before ordinary continuation. Include partial focus dwell/freshness, reciprocal incumbent versus unsolicited invitation, witness/newcomer suffix, old token versus newer same-time group choice, MAX token saturation, historical IDs, unordered warm-pair alias/30s expiry, Novelty 60s inclusive expiry, opaque context/visit, submission-time news and later inbox arrivals, raw Engine config edge values, strict/duplicate/missing/raw-padding refusals and format maxima. New component fixtures only; previous fixture bytes stay unchanged. Coordinator supplies independent public boundaries.

Measure six release phases on actual authored 520 and +2000 successfully placed 2520 Engines with real nav/content. Use ordinary speech and scheduling operations to establish nonempty conversation/warmth/novelty, preserve semantic counts and meaningful input/output witnesses across repetitions. Maximum maps and rare edge cases are separate deterministic tests. Freeze source/assets before final verification, retain exact original stdout/stderr with source-at-start manifests and honest failure history. Coordinator reviews source/admission/evidence, runs release measurements and commits this leg. Complete M2b capture/hydration, M2c pending-work adoption and M3/host acceptance remain pending.


#### M2a10 coordinator review — 2026-09-08

Strict Conversation/WarmExchanges/Novelty/EngineSocial components preserve independent partner/invitation/focus, historical warmth, prior witnesses, utterance counters, exact opaque context/visit salts and original idle configuration. [Coverage](evidence/m2a10/OWNER_COVERAGE.md) and [admission](evidence/m2a10/ADMISSION.md) bind exact fields and player identity without pruning or recomputation, under unchanged caps.

[Coordinator acceptance](evidence/m2a10/coordinator/review.md) verifies the frozen 895-path unified input scope, 133 focused/six public and **2,045 workspace passes**, private continuation and four new fixtures, complete archived failures, and all 3,600 release phase samples. Authored/populated export p99 is 0.004/0.005 ms; Save+Load reservations are 257,802/293,942 B excluding Running. All seven historical datasets pass the amended auditor, with four intended negative refusals.

Scheduler/Floor/SpeechRouter, remaining Engine configuration/cadence and host state, complete owner/root/manifest assembly, all-consumer horizons, actual cohort lifetimes, capture/hydration and generation-fenced pending-work restoration remain pending. Initial idle mode cannot rebuild scheduler fairness. Interrupted unsent attention is a later draft-policy transformation; this component introduces no active STT restoration or automatic speech. Complete M2 is not accepted.

M2a10 [tail review](evidence/m2a10/coordinator/tail_latency_audit.json) retains a slow third populated run: pooled populated export p99 is 3.577735 ms, maximum export 13.120826 ms and maximum decode 55.358842 ms. The smaller figures above are medians of per-run percentiles; they do not establish synchronous host-frame acceptance.

#### M2a11 design cut — 2026-09-08

After M2a10 commit `2a5115060b8dc8731d93ca14d0a015d72898f981`, the next sequential owner is `NpcScheduler`, composed with Engine's original turn-delay/backoff configuration and immutable player binding. This is another strict read-only admitted component. Production adoption, external resubmission, Floor/SpeechRouter transformation and the complete envelope remain later work.

Preserve every existing scheduler field: exact weighted `order`, `round_robin_index`, both ordered priority/protected queues, normalized minimum delay and maximum backoff, `next_turn_at`, failure count, running flag, submission notification, retry-work map, flight and held result. A flight retains actor/incarnation, execution request, semantic operation, original lane, exact drained inbox, exact presented history and prompt. Retry work retains actor/semantic/incarnation. Reuse the existing strict completion/error owner representation while retaining exact success/error/detail/duration values. Required nullable fields cannot disappear from the wire.

Order duplicates are legitimate weights; do not deduplicate or reconstruct order from initial idle mode. Queues must be individually unique and disjoint from each other. The flight actor may have been queued again during its outstanding request, so it need not be absent from the queues. Valid historical IDs may be absent, departed or changed in the current World; ordinary scheduler checks still retire them. A successful-submit notification is exact standalone authority; its relation to Engine's consumed Novelty stamp is an explicit complete-boundary agreement, not an excuse to clear it during decoding.

Use explicit supported limits of 100,000 weighted order slots, 25,000 entries per lane and 256 retry rows, under the existing global aggregate ceilings. Cursor is zero for empty order and otherwise within it. The two saved input buffers each use the existing 64-entry bound and the admitted 65,536-byte per-text limit. Prompts use the existing 64 KiB supported canary; refuse an unsupported exact prompt rather than truncating or rerendering it. These are format admission limits, not claims that arbitrary direct public edits are inherently bounded by the current roster. Validate distinct semantic roots across flight/retry records against borrowed live or saved ledger state with `TURN_PRODUCER`; known committed root receipts remain admissible for ordinary replay suppression. Complete protected-root owner equality remains the envelope's responsibility.

Scheduler floor deferral happens before provider-reply size validation. Therefore, a held successful string can still represent an invalid oversized provider answer. Admit exact held strings up to an explicit 400,000-byte component limit without applying the normal 100,000-scalar success predicate during decode. A restored oversized held answer must later take its ordinary failure path. Larger unsupported raw answers refuse capture without truncation or invented error conversion. Held completion requires a matching flight request; absence of a held result does not mean the submitted semantic obligation vanished. Never restore drained input and also apply that result.

The normalized delay is nonnegative and maximum backoff is at least one and at least the delay. Reachable positive infinity is explicit; finite historical future pacing remains exact even beyond the ordinary logical-boundary horizon. Boundary comparison is bitwise, including signed zero. Preserve the two original Engine configuration floats independently as strict raw IEEE bits; do not rerun constructor normalization over actual scheduler state. Failure-count admission must reserve the next failure increment and avoid the existing signed `i32` exponent boundary, not merely the final `u32` overflow. Establish source-based local headroom and document that all-future consumer safety remains pending. No broad scheduler behavior change is included without a concrete reviewed defect.

Typed decoding follows nonallocating raw lexical admission; raw padding/escape charges remain attached through candidate conversion. Prove sparse tree/deque/vector layouts, malformed overcount/string shapes, exact cloning and sequential borrowed-ledger/lane/root scratch. A proposed 4 MiB owner working allowance must be justified by actual retained layouts and lifetimes; caps remain 128 MiB E/J, depth 64 and 1 GiB shared. No temporary World, geometry/index rebuild, constructor, provider-failure helper, idle requeue or ordinary poll runs during decode/candidate validation. Do not introduce a public scheduler clone or partial installation solely for checkpoint testing.

Private tests must scramble covered state before installation and demand canonical equality before ordinary continuation. Verify weighted fairness, protected/handoff FIFO and suppression, an actor queued again while flying, Busy/failure courtesy, running=false with a completed flight, once-only held success/error/oversized-answer application, stale incarnations, exact pending prompt/drained/presented separation and post-submission arrivals. Keep new fixtures separate from every historical fixture. Root supplies independent public boundary tests and release review. The probe uses actual authored and +2,000 successfully placed populations, ordinary commands/scripted provider values and real floor or receipt deferral to populate its measured state; private mutations must not manufacture the timed boundary. Preserve all timing samples and report long tails explicitly.

Full M2c must replace unfinished execution with exactly one load-specific retry of the original resolved input, including an empty-inbox idle turn, without a generic failure/rerender/requeue. Existing flight does not separately store the derived provider output-token budget; the complete input-receipt design must account for that original option and immutable content binding before resubmission. Held terminal values require no provider request. Runtime generation fencing, seated knowledge/receipt agreement, complete host publication and actual cohort lifetimes remain mandatory later gates.

#### M2a11 coordinator acceptance — 2026-09-08

The [independent review](evidence/m2a11/coordinator/review.md) accepts exact existing scheduler and Engine configuration components against 907 frozen inputs, 2,069 workspace passes, 151 focused passes and six public boundaries. Four new fixtures preserve all historical bytes. All 3,600 release phase samples pass source/binary/counter/admission audits; the corrected authored/+2,000 setup uses 136 bounded ordinary polls with no discarded physical time. authored export pooled p99 0.077532 ms, observed maximum 0.093167 ms; populated export pooled p99 0.186618 ms, observed maximum 0.209647 ms. All tails are retained in the [performance record](evidence/m2a11/performance/README.md).

Floor/SpeechRouter, remaining Engine fields, complete envelope/manifest/root agreement, all-consumer horizons, full capture/hydration, generation-fenced M2c resubmission and actual cohort lifetimes remain pending. Current saved flight retains the exact prompt but the derived provider output budget still needs a complete resolved-input receipt before retry. No synchronous host-frame or complete-save claim is made, and no byte cap increases.

#### M2a12 design cut — 2026-09-08

After the accepted M2a11 scheduler commit `e68d57fcc795b401b10169c0ccb9ab00b21e1556`, preserve the existing ConversationFloor and remaining Engine cadence/publication/configuration state as strict admitted read-only components. This closes these field owners; it does not adopt an Engine, restore a microphone, replay audio or establish the complete save envelope. SpeechRouter interruption and owed readable presentation remain the next separate owner.

Floor retains the exact insertion-ordered awaited speech IDs, deadlines and protected-reaction scopes, independent foreground/background reading/beat deadlines and player-hold deadline. Preserve the existing 32-row limit, unique exact SpeechEventId strings and supported finite nonnegative or positive-infinite pacing values without sorting, pruning, subtracting a host origin or replacing future historical anchors. SpeechEventId is an unvalidated string newtype, unlike ActorId: state its supported byte limit explicitly, and do not invent an actor-ID or speech-prefix predicate. Capture boundary equality is bitwise, including signed zero. Unsupported values refuse capture; no truncation or repair.

Prove acquire/refresh insertion order, trim-before-refresh at capacity, last-same-scope acknowledgement's 0.4-second beat, duplicate/unknown acknowledgement no-op, exact-deadline purge without beat, independent background versus protected-player pacing, unvoiced reading holds without awaiting IDs, and rolling microphone hold. Private continuation must scramble covered fields, restore without a constructor, assert immediate canonical equality and then compare ordinary behavior. Exact saved floor authority is not the final load policy: old audio acknowledgements and interrupted player holds must later be transformed together with saved speech obligations and readable presentation, without reapplying a committed say.

Engine continuity composition owns last_snapshot_revision (i64), last_player_sound_at (including the initial negative-infinity sentinel), next_round_tick_at, lamp_revision_sent (u64), startup_diagnostics, ready_emitted and actual mutable tts_selected. It also preserves the remaining stored EngineConfig fake_mode, sounds_enabled, view_cone_degrees, sound_cooldown_seconds, tts_selected, tts_startup_message and stt_stream_grace_seconds, with immutable player binding. Stored configuration is the value currently in Engine: its constructor already clamps sound cooldown and floors STT grace. Do not reconstruct lost pre-constructor input or normalize the stored floats again. Use exact raw float bits where ordinary consumers support them; keep initial config TTS selection distinct from later runtime selection. Bind live World or an unadopted backbone. Derive validation from actual consumers and document complete-owner/horizon agreements still pending.

Keep publication cursors, startup diagnostics and ready flag exact. Future M3 initial host publication is an explicit presentation operation; resetting these caches or polling to emit Ready would alter simulation continuation. Runtime capabilities/services/generation/path are rebound separately. Engine.transcript is the existing omniscient session artifact, not the player's durable readable history; this cut must not claim to supply the later transcript/journal policy.

Use mandatory closed V1 records, strict duplicate/unknown/missing/nullable refusal, raw lexical admission before allocations, attached leases and explicit count/text limits. Prove concrete layouts and sequential scratch against the smallest justified working allowance under unchanged 128 MiB E/J, depth 64 and shared 1 GiB ceilings. No IO, clock, thread, seeding, ordinary polling or public partial adoption in production decoding. Add new fixtures only; preserve all historical fixture bytes. Root supplies independent public boundary tests after the API proposal.

Measure the six component phases on actual authored 520 and successfully placed +2,000/2,520 populations using real content/navigation and ordinary commands/scripted TTS acknowledgements to populate relevant floor/cadence/publication state. Setup uses bounded ordinary steps of at most 50 ms and no discarded physical-time diagnostics. Record meaningful primary inputs and outputs, exact semantic counts and every raw timing sample; maximum shapes and rare state are separate tests. Freeze the complete component-inputs-v2 source set before final focused/public/workspace verification, retain exact originals and development failures, then cede Cargo for root release review and commit.

Full M2 still requires SpeechRouter transformation, exact manifest/content/build/target/toolchain binding, full Engine/World owner/root agreement, all-consumer horizons, capture/hydration and generation-fenced pending work. The earlier naive backbone+Round Save+Load peak exceeds 1 GiB before Running; actual phase/lifetime integration must solve that without raising caps. Component timings do not establish host-frame acceptance.

#### M2a12 coordinator acceptance — 2026-09-09

The [independent review](evidence/m2a12/coordinator/review.md) accepts Floor and Engine continuity against 917 frozen inputs, 2,090 workspace passes, 166 focused passes and six public boundaries. New fixtures leave all historical bytes intact. The ordinary authored/+2,000 setup uses bounded polls with zero discarded physical time. All 3,600 release phase samples pass provenance/counter/admission audits. authored export pooled p99 0.002510 ms, observed maximum 0.003357 ms; populated export pooled p99 0.003217 ms, observed maximum 0.003654 ms. See the [full performance record](evidence/m2a12/performance/README.md).

SpeechRouter interruption and readable presentation, complete manifest/root/owner agreement, full capture/hydration, pending-work execution replacement and actual phase/lifetimes remain pending. Saved Floor holds are not a promise to resume old audio or microphone activity. No cap increase or synchronous host-frame claim is made.

#### M2a13 design cut — 2026-09-09

After accepted M2a12 commit `c035a430da9d23132bcf9f8138e27eedca3401bf`,  implement the SpeechRouter persistence owner for
load interruption as strict admitted read-only component values. This closes the
sim-owned recording/draft input to M2c; it is not a whole Engine adoption or a host
microphone/audio restore. The owner must first propose the exact wire/API and an
exhaustive field-disposition table for root review before writing production code.
The choice is a semantic interrupted-input checkpoint, not a promise to continue
old streaming sessions. Existing authoritative state and ordinary behavior must
remain unchanged until complete M2c composition owns the interruption transition.

Preserve every accepted recording obligation and its exact optional CommandId,
original purpose, request correlation and available user text without guessing
that a repeated basename denotes the same semantic submission. Current router
input purpose is public player speech; do not invent already-existing proposition
versions/selections, which belong to future/host owners. Preserve available text
from completed but unconsumed streams even before PlayerRecording acceptance.
Preserve exact owner configuration and clearly distinguish it from Engine stored
config; standalone SpeechRouter constructor/default do not apply Engine clamps.

At the accepted complete outer-command/poll boundary, synchronous resolved staging
must be empty. Refuse a mid-command snapshot instead of applying or dropping a
pending result. Bind exact accepted semantic roots/receipts against borrowed live
World or unadopted backbone+ledger references, with explicit terminal-receipt
reachability policy. The combined accepted jobs/parked/resolved bound is eight;
stream/capture bounds are independently eight. Do not impose unsupported basename
uniqueness, assume execution stream_jobs are bounded by active streams, or require
queue-time TTS mappings to match still-awaited Floor IDs. Audit actual consumers.

Assign raw audio/sockets/backend handles, stream execution IDs, captured unsent
attention, timing diagnostics and TTS delivery mappings explicit transient/rebind
policies. Preserve all previously committed attribution via prior saved owners;
never recalculate its suffix from restored current attention. An interrupted input
is an unsent draft/status requiring a new intentional submission. Do not invoke
normal resolve_transcription (it applies say), ordinary abort (it leaves accepted
jobs), submit_batch, poll, provider callbacks or service availability while
capturing/decoding/preparing these values. No public partial Engine install.

M2c must later consume this validated value together with saved Floor/ledger/shared
roots to terminalize only interrupted accepted obligations, release their owner
bindings correctly, discard old microphone holds and execution mappings, and
publish drafts/status without replaying a say. That complete transformation and
new-generation callback rejection need integration evidence before full M2.
Committed NPC/player speech already applied to World must retain readable owed
presentation from the actual pending event/message/host text owner; router TTS
backend mappings contain no text and do not certify that separate owner complete.

Use mandatory closed V1 records with explicit supported counts/text/number limits,
strict duplicate/missing/unknown/nullable refusal, raw lexical admission before
allocation, attached leases and a proved working allowance. Do not raise 128 MiB
encoded/expanded, depth64 or shared1GiB limits. Preserve historical component
fixtures. Bound memory for malformed/max/sparse inputs from concrete layouts and
actual installed allocator/serde/container behavior; any projection copies remain
charged throughout their lifetime. Independent root public boundaries follow the
approved API proposal.

Use controlled pure STT/TTS/Cognition services and ordinary Engine commands to
exercise active microphone, available unsent stream text, parked and batch-pending
accepted recordings, reused basenames, exact semantic receipts and already
committed voiced/readable speech. Rare/max and mid-command refusal stay separate
private tests. The authored520/+2000allplaced2520 six-phase component probe uses
real installed content/nav and <=50ms ordinary steps with no physical-time discard.
Preserve primary submitted inputs and committed outputs plus draft/root counts,
not only Debug fingerprints. Exact originals, final complete v2 source freeze,
focused/public/serial workspace and independently audited release trials remain
the commit gate. No audio/device/window activity or external provider calls.

Complete manifest/content/build/target/toolchain equality, full owner/root/horizon
agreement, Engine capture/hydration, original pending cognition output budgets,
initial host publication and actual Running/Save/Load/retiring lifetimes remain
pending. The existing naive backbone+Round sum exceeds1GiB before Running; solve
phase/lifetimes in full composition rather than increasing caps.

The reviewed M2a13 API proposal uses a semantic interrupted-input projection:
separate ordered capture basenames, stream text rows and accepted recording rows,
plus exact router grace bits. Accepted rows preserve their source, optional exact
CommandId and retained receipt, request correlation, basename, accepted pose,
backend and parked deadline provenance. They do not merge by basename or operation
root. Engine composition adds immutable player identity with character-map
membership, not a present/in-city predicate. Candidates expose borrowed records
only; they cannot create an active SpeechRouter. Old execution mappings, unsent
attention and diagnostic timing fields have explicit transient policies.

Live capture checks the exact `World.speech_actions` owner index; saved-context
validation derives that index from accepted rows and validates every receipt and
protected root against the unadopted ledger. Terminal receipts remain terminal.
The public-speech purpose records the owning subsystem's classification; it does
not independently prove the original ledger payload digest. Current tasks omit
the submitted spatial sequence and may retain an accepted pose different from
the original payload. Complete semantic owner/category agreement must address
that distinction without fabricating original input values. Receipt float equality
is bitwise, and raw grace/deadline records use explicit `{bits:u64}` values.

#### M2a13 coordinator acceptance — 2026-09-09

The [independent review](evidence/m2a13/coordinator/review.md) accepts interrupted speech projections against 930 frozen inputs, 2,113 workspace passes, 183 focused passes and six public boundaries. New fixtures leave all historical bytes intact. Ordinary authored/+2,000 setup uses 17 bounded polls with zero discarded physical time. All 3,600 release phase samples pass provenance/counter/admission audits. authored export pooled p99 0.009243 ms, observed maximum 0.010273 ms; populated export pooled p99 0.008516 ms, observed maximum 0.009428 ms. See the [full performance record](evidence/m2a13/performance/README.md).

Complete manifest/root/category agreement, original cognition output budgets, full capture/hydration, atomic Floor/ledger interruption and host draft/readable publication remain pending. The candidate cannot restore an active microphone or automatically submit speech. Actual phase/lifetimes must resolve the earlier complete-save peak excess without a cap increase; no synchronous host-frame claim is made.

#### M2a14 design cut — 2026-09-09

After accepted M2a13 commit `3aed5c264462e6e8c84c0d0e880e82057f47741d`, close the missing resolved cognition
input authority for both NpcScheduler and NightOffice before complete hydration.
Each currently retains its submitted prompt and semantic work identity, but both
derive an output-token budget from the then-current subject at submission and do
not retain that resolved argument. Exact retries cannot reconstruct this input
from a later mutable actor, a current default or a reformatted prompt.

Retain the exact accepted request method/lane and Option<u32> budget alongside
the original prompt and its existing actor/subject/incarnation/semantic identity.
None is an intentional provider-default argument, distinct from missing legacy
authority. Capture only successful submission: a Busy attempt is not an accepted
execution receipt, and subsequent ordinary retries resolve their own new inputs
under existing behavior. Held success/error still belongs to its matching flight
and never needs another provider call.

The owner must propose the precise runtime field, versioned wire/API and legacy
component policy before production edits. Preserve all historical V1 fixture
bytes and honest prior component scope. Do not silently fill a missing budget,
rewrite old V1 meaning, or claim an old component alone supplies complete input.
Prefer a narrow explicit version or separately bound resolved-input owner over
duplicating the entire scheduler/Night codec. Any added state must have complete
creation/removal/failure/held/invalidated lifecycle ownership and admission proof.
The complete envelope must refuse unsupported missing exact submitted input
before adoption, while allowing explicit valid absence for a nonflying lane.

Use closed mandatory records, exact borrowed live/unadopted component/root
bindings and existing raw admission/retained leases. Fixed scalar options must
not justify raising E/J, depth or shared byte caps. Do not add external IO,
clocks, threads, ordinary polls, prompt rerendering, generic provider failures or
partial production hydration to decode/validate. Existing ordinary provider
requests, budgets, fairness, Busy handling and held application must remain
behaviorally unchanged apart from retaining the resolved input authority.

Private and independent public evidence must observe original submitted prompt
and budget, then change actor significance/lore/config or remove/change the
subject after acceptance, proving the saved argument remains exact. Cover both
scheduler and person/ward Night lanes, None versus Some, Busy, flight cleanup,
held success/error, strict missing/unknown/duplicate/ref-binding refusals and the
legacy policy. Use meaningful ordinary commands and controlled pure services.
Add new fixtures only, freeze complete inputs, retain failures and verify focused
and workspace behavior before review and commit. Measurement must exercise the
new authority through real accepted requests; reuse existing measurement
infrastructure where it truthfully measures this change rather than inventing
another large population simulation.

This cut supplies exact input for later M2c execution replacement. Complete
Engine/World/host assembly, once-only load retry/adoption, host readable draft
publication, all-consumer horizons, compatibility manifest and actual cohort
lifetimes remain required work. No complete save or host-frame claim follows
from retaining this missing argument alone.

The reviewed design adds an inline `AcceptedOutputBudget` to each existing
scheduler/Night flight: explicit `MissingLegacy` or `Accepted(Option<u32>)`.
Successful submission installs it; moving or retiring the flight carries or
discards it. Existing V1 export/copy and decode intentionally omit this newly
introduced authority and produce `MissingLegacy`, preserving their historical
wire scope. A new mandatory field cannot be smuggled into that closed old wire.

`EngineCognitionInputsDtoV1` is a separate admitted read-only component with
mandatory version, boundary, player and nullable scheduler/Night rows. Each
present row preserves the exact request method, optional budget, prompt and
existing flight identity/lane or Night subject/day/incarnation. The two prompts
are each bounded by 65,536 bytes and duplicated only in the admitted projection;
Running retains its existing single prompt. A live context compares the known
accepted argument exactly. An unadopted context derives both old owner contexts
from one backbone, ledger, boundary and exact supplied saved clock, then binds
every row to its legacy flight. Presence must match; absent budget authority is
never interpreted as an intentional null argument. Both boundary values are
compared bitwise, including where the older Night wrapper used numeric equality.

Context construction only borrows. The proposed 4 MiB working charge must cover
sequential old-owner validation before projection checks, with proof of the new
inline flight layout and prompt/container allocations. New public records expose
Serialize and borrowed getters only; private decoders enforce mandatory nullable
fields and strict string tags. Opt-in cognition-input modes in existing probes
may reuse their ordinary setup, while preserving default historical workloads
and retaining full actual submitted method/prompt/budget witnesses in the new
mode. Full execution replacement and complete adoption remain later work.

#### M2a14 coordinator acceptance — 2026-09-09

The [independent review](evidence/m2a14/coordinator/review.md) accepts exact cognition input authority against 938 frozen inputs, 2,129 workspace passes, 193 focused passes and six public boundaries. All 7,200 release phase samples pass; historical defaults and fixture bytes remain unchanged. scheduler authored export pooled p99 0.041565 ms; scheduler populated export pooled p99 0.054141 ms; night authored export pooled p99 0.023191 ms; night populated export pooled p99 0.023130 ms. Complete envelope/root agreement, full capture/hydration, once-only pending-work adoption, host owner restoration and actual phase/lifetime admission remain pending. See the [performance record](evidence/m2a14/performance/README.md).

#### M2a15 design cut — 2026-09-09

This leg owns the existing application's continuation authority and readable
presentation contract. M2a14 is accepted at `e4653ed`; this is the next sequential
ESPFEIT owner. Implement a strict admitted read-only host component with actual
owner extraction, exact field policies, new supported component fixtures and
renderer-free behavioral evidence. Complete-envelope assembly and final host
adoption remain M2b/M3; a host component cannot claim complete city restoration.

Before changing production source, reconcile all host command-producing resources
and player-readable owners from current code, propose the concrete schema/API and
identify any additional authoritative fields. The coordinator reviews that bounded
handoff while independently reading consumers. The owner's inventory is the basis
for implementation; old comments calling all host state a projection are not proof.
Do not invent future notebook/proposition/document fields that are absent today.

Capture authority includes the exact PlayerController dynamics/view, both
PhysicalPosition samples, actual LiveTime accepted/debt owner and fixed residual,
and LocalEngine accepted physical sequence/input watermark/boundary. Bind host
and sim accepted pose/yaw/elapsed/sequence bitwise where their types permit exact
agreement. Render-interpolated Transform is not body authority. Classify currently
held device inputs separately with an explicit release/re-sample policy; never
replay an old mouse delta or manufacture an intentional submission. Preserve
physical previous/current values in the read-only format and reserve interpolation
reset for the final publication policy.

PlayerCustodyState strain and struggling_reported generate future semantic commands.
Preserve them with the queued/accepted command boundary. Its sampled custody view
and notices need an explicit source-backed binding to the saved sim law/publication
state, accounting for host consumer lag. No independent second law authority or
silent progress reset is permitted. Prove next ordinary tether/strain behavior and
once-only threshold delivery against control state.

The physical definition binding also includes the actual optional CutMarginProfile:
its ordered rectangles/feather flags, ramps and stairs, plus the ground-height
algorithm/constants. The fixed controller applies this virtual floor after the
collision sweep, changing height, vertical velocity and grounded/coyote state;
CollisionWorld or navigation identity alone does not cover it. A prepared host
with a different profile must reject the candidate before continuation.

The prepared definition context also binds gate/vermin owner presence and the
installed vermin seed, percept flag and density bits. Keeping an opaque geometry
hash while replacing required continuation with null is not a valid checkpoint;
an absent owner cannot acquire spurious continuation either. Controller motion
limits use the actual solver/setter constants, with an accepted-lifetime bound
for the falling velocity axis. Quickbar selection must retain room for its next
ordinary wheel increment. These checks reject unsupported finite values before
their consumers run, without changing ordinary motion constants.

Vermin seed/config/colony/nav definition identity, swarm_percepts,
announced_boil_night and last_percept_minutes affect WorldSound commands. Preserve
exact meaningful gates, including last-percept stamps retained when try_send fails.
Cosmetic rat poses may rebuild under a stated policy, without new swarm percepts.
Reconcile every soundscape resource: accepted bell scheduling sends semantic Knell
or CivicPeal once, so CueCooldowns and civic/clock gates cannot all be discarded as
sound effects. Preserve semantic gate authority and explicitly classify scheduled
strokes/PCM/handles and purely cosmetic timers as discarded or rebuilt at the saved
accepted calendar. Initial restored clock publication must not create a new peal
or replay an already delivered flour/curfew/well cue.

Chat open/buffer/character cursor and visible journal open/scroll position are
player state. Preserve exact unsent Unicode text; submission validation happens
only on new intentional input. Distinguish editor modifiers/blink/device state.
Journal resolved rows are sampled readable data, with exact source/publishing
semantics; an open journal does not lose its position during re-publication.

Owed committed speech must come from actual PresentSpeech/event/host consumers,
not Engine transcript or TTS request maps. Capture already-consumed pending
subtitle/bubble/player-transcript receipts and readable messages still queued at
the host boundary. Reconcile actual ECS bubble text/anchor lifetime and sequence
tracking. If current retained data loses original fields, add a bounded actual
receipt at ordinary presentation acceptance; do not reconstruct names/plain text
by splitting a formatted label. Preserve exact event identity, words, attribution,
anchor and readable progress. Bound supported rows by the existing presentation
limits without silently dropping saved work. Define remaining minimum readable
time when old audio is interrupted, keep generation identity distinct from durable
event identity, and never execute a say or revive old audio/STT/TTS on decode.
If the additional original-receipt byte budget is exhausted, keep ordinary
dialogue display and audio behavior unchanged. Decline capture while a surviving
readable owner lacks that receipt; do not acknowledge and skip new dialogue
because checkpoint bookkeeping could not retain it. Distinguish an unavailable
committed player receipt from an ordinary provisional transcript, and clear that
unavailability only when the owning caption is replaced, expires or is discarded.
Inventory HUD/player-transcript/timed-readable outcomes and separate preferences
and rebuildable service/mirror state. Existing queue overflow policy is not a
license for a checkpoint decoder to discard admitted rows.

The sim crate stays pure: no Bevy, filesystem, device, clock reads, threads or
network. If its existing admitted codec is reused, define closed pure host data
and a supported borrowed input/context API there; do not expose unchecked
Admitted constructors or blanket Deserialize escape hatches. Host adapters own
actual resource/ECS observation. Preflight/lease admission must precede cloning
strings, collecting rows or constructing validation indexes. Retain raw input
and candidate charges, strict unknown/duplicate/missing fields and explicit
nullable values. Exact installed definitions/config/ordered catalogs must bind
without seeding or warming side-effectful caches. Use source-supported limits
under unchanged 128 MiB encoded/expanded, depth 64 and shared 1 GiB ceilings.

This cut exports and validates completed host boundaries and exposes read-only
candidates; no live partial resource adoption, save-only poll, provider wait,
input acceptance freeze lasting across frames, user-facing save controls or disk
publication. Tests may install covered fields privately into scrambled prepared
owners to prove their next ordinary behavior, with immediate canonical equality
before stepping. The capture contract must honestly reject an incoherent boundary
without draining or advancing the live host to make the test pass. All emitted
readable obligations through watermark H need explicit conservation whether
already consumed by presentation or still waiting in the bridge.

The actual chat continuation test exposed an ordinary input-order regression:
PreUpdate creates a pose-bearing intent, the completed physical sample advances
its sequence, and later forwarding leaves that intent stale at the next poll.
Repair sequence assignment at the ordinary forwarding boundary for unaccepted
pose-bearing intents, preserving their captured position, words and targets.
Apply the policy to every affected intent family. Keep sim stale-sequence checks
and already-accepted command identities intact. The checkpoint retains unread
intent state without forwarding it; a normal subsequent update must still commit
a valid typed line exactly once.

Verification must cover virgin/active owners, jumps with nonzero residual and
saved ordinary debt, custody threshold on either side of queue acceptance, vermin
repeat before/after deadline including full-queue stamp behavior, semantic peal
once-only gates, open/Unicode drafts and journal scroll, multiple queued readable
lines/bubbles and partially elapsed visible minimum, old audio interrupted without
repeated domain effects, and corrupted boundary/reference/limit/numeric inputs.
Use real owner systems with MinimalPlugins/no renderer/device/provider; do not
reprobe the known unavailable GPU. Timed probes should exercise meaningful actual
host state through ordinary paths, retain primary boundary/event/input witnesses,
and measure preflight/export/encode/decode/validation/drop separately. Rare/max
supported shapes belong in separate tests. Coordinator owns release runs; owner
owns serial Cargo checks, exact original logs and final source-at-start freeze.

Complete assembly still owes manifest/build/target/toolchain/DefaultHasher,
category/shared-root equality, all-consumer numeric/time horizons, full hydration,
M2c exact once-only retry/interruption and dedicated M3 publication without a poll.
The naive backbone+Round Save+Load bound of 1,256,093,444 bytes already exceeds
1 GiB before Running. This leg must neither raise the cap nor claim it solves
actual full-cohort lifetimes or host frame scheduling.

#### M2a15 coordinator review — 2026-09-14

The admitted read-only host component and actual capture adapter are accepted.
They preserve body/time/debt/residual authority, installed physical and semantic
definitions, command/consumer fences, pending intentional input, UI state and
exact surviving readable obligations at the completed ordinary boundary. Capture
does not poll, drain input or wait for later worker arrivals. Ordinary unaccepted
chat forwarding now stamps a current spatial identity while preserving captured
words/pose/target. Original-receipt saturation preserves normal dialogue/audio
and refuses capture until missing readable authority expires or is replaced.

Two new supported host component fixtures preserve initial and active states;
three writer processes agree exactly and the ordinary loader checks canonical
decoded/candidate bytes at actual compatible CityPlugin boundaries. They remain
component fixtures, not saved cities. [Coordinator acceptance](evidence/m2a15/coordinator/review.md)
records the final 950-input source, **2,157 workspace passes, zero failures,
38 ignored**, eleven independent public cases and exact source/admission/archive
audits, including all historical failed commands.

Both actual-city release smokes passed. [Measurements](evidence/m2a15/performance/README.md)
retain 3,600 phase samples across six processes, with all 2,000 requested additions
placed in the populated workload. Both modes encode 6,275 bytes and retain
10,162,498 bytes of simultaneous component Save plus Load admission, excluding
Running. Export p99 is 5.634495/10.245549 ms authored/populated; every preflight
and export sample exceeds 2 ms. The unchanged frame target therefore still needs
bounded offload or incremental host coordination. Complete-envelope ownership,
hydration, retry/interruption adoption, full generation lifetimes and publication
remain mandatory work. M2 is not complete.

#### M2a16 next design cut — 2026-09-14

Begin only after M2a15 is accepted and committed. Compose the existing owners
into one closed versioned envelope and admitted read-only complete candidate.
This is the remaining M2a composition cut; dedicated production hydration is
M2b, external-work adoption M2c, full continuation M2d and application publication
M3. The sequential fresh-context owner first reconciles the actual field inventory,
wire/admission design and independent public test seam with the coordinator.

Capture one ordinary completed boundary with ledger, operation kernel, World
backbone, Round, climate/clock, knowledge, law, marks, animals, social/floor,
Engine continuity, scheduler, Night, interrupted speech, exact cognition inputs
and host authority together. Explicitly reconcile World.events and any remaining
fields against current consumers. The existing backbone exporter rejects
unflushed events; prove what the actual ordinary boundary guarantees rather
than draining or polling solely to make capture eligible. Validate category and
root agreement, saved versus later inbox work, player/config/calendar/physical
identity and every consumer's numeric and temporal headroom. Independently valid
components from different boundaries are not a valid complete save.

World/lineage identity is stable across replacement runtime generations and is
distinct from content identity. Its initial creation belongs to the host; the
simulation remains free of filesystem, OS randomness, clock reads, devices and threads.
Compatibility binds actual installed parsed objects and effective overrides in
their distinct roles, ordered generator/geometry/behavior inputs and exact
implementation/toolchain/target/build configuration. Git HEAD alone cannot
identify a dirty build, and today's files cannot attest to earlier loaded
objects. Legitimately distinct World and Engine definition copies remain distinct.
Specify supported-fixture compatibility honestly: fixture bytes must not hash
themselves through a build manifest, and saved build identities must not be
silently replaced with current identities to pass a loader test.

Use one shared admission budget with real retained and temporary lifetimes.
Independent budgets for component exporters are not aggregate admission. The
four cohort count limits, 1 GiB shared ceiling, authored 64 MiB/populated 128 MiB
encoded limits and 128 MiB expansion ceiling remain unchanged. The historical
naive backbone plus Round simultaneous Save/Load charge exceeds 1 GiB before
Running; their combined generic expansion estimate also exceeds 128 MiB. A
different closed full-envelope layout may use a new proved allocation model,
but cannot merely reduce charges or omit raw buffers, indexes, cloned definitions
or surviving owners. Internal subordinate leases retain their cohort slot until
the last owner drops, and reserve growth before allocating. Public unchecked
extraction or constructors are not an acceptable composition API.

Measure an actual complete authored/+2,000 placed workload at one boundary,
including preflight, extraction, encoding, raw decode, validation/index building,
candidate ownership and disposal. Distinguish Running/Save/Load coexistence from
process RSS, and preserve all cold and tail samples. Support small complete
versioned fixture payloads plus corruption, incompatible-mixture, raw-admission
and cancellation/lifetime cases. Do not seed an Engine/World or install partial
live state to validate a candidate. This cut must not imply that synchronous
capture meets the M3 frame budget or that component timings establish complete
save/load performance.

#### M2a16 coordinator acceptance — 2026-09-15

The complete closed envelope, ordinary-boundary capture and admitted immutable
candidate are accepted. All sixteen saved owners are validated together against
actual installed definition/build/image roles; root, publication, physical/time,
configuration and next-consumer agreements are checked without seeding or a
save-only poll. [Coordinator review](evidence/m2a16/coordinator/review.md) records
2,178 workspace passes, twelve independent public cases and all preserved failed
attempts. Three fresh writers reproduce each complete supported fixture; the
creating executable accepts exact bytes and an incompatible image refuses.

[Release evidence](evidence/m2a16/performance/README.md) retains2,400 end-to-end
and 7,800 stage samples from actual 520/2520-character hosts. Shared peaks including
Running remain669,365,203/793,082,795 bytes; populated expansion has only 866,545
bytes headroom. Capture p99 is 76.099/225.320 ms, so synchronous host placement
remains unaccepted. M2b dedicated hydration is next; M2c external obligations,
M2d continuation and M3 file/ECS/service publication remain mandatory.


#### M2b coordinator acceptance — 2026-09-15

Dedicated admitted hydration is accepted. It reconstructs actual World/Engine
owners without normal seeding or a save-only poll, rebinds fresh immutable asset
roles, and preserves lineage, time and pending authority in a protected wrapper.
[Independent review](evidence/m2b/coordinator/review.md) records 2,191 workspace
passes, six public cases, ordinary nonempty pending/root witnesses, exact memory
and failure lifetimes, and all original failed/development command records.

[Release measurements](evidence/m2b/performance/README.md) retain 600 hydrations
with all sixteen owner hashes equal, across actual 520/2,520-character hosts.
Hydration p99 is 39.981/119.324 ms; shared peaks including Running are
726,174,363/843,676,620 bytes. The populated expansion retains 604,401 bytes of
headroom under the unchanged cap. Three fresh writers agree for each supported
fixture, compatible readers hydrate exact bytes, and different images refuse.

M2c joint retry/held/interruption preparation is next. M2d full continuation and
M3 file/ECS/service adoption, worker coordination and actual retiring lifetimes
remain mandatory. Synchronous host-frame acceptance is not claimed.
