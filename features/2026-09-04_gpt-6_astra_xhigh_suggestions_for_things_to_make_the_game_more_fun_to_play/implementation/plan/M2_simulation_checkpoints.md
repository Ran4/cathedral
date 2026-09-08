Status: In progress (2026-09-08). M2a1–M2a10 private components are implemented and reviewed. The complete M2a envelope and remaining owners, M2b capture/hydration, M2c pending-work restoration and M2d continuation remain pending.

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
