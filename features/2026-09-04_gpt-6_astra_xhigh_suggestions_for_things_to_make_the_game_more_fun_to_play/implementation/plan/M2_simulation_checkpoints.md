Status: In progress (2026-09-08). M2a1–M2a4 private components are implemented and reviewed; the complete M2a envelope and remaining owners, M2b capture/hydration, M2c pending-work restoration and M2d continuation remain pending.

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

Continuation witnesses cover scheduled/custom climate, stable lightning identity, inherited wetness, drying residue, config-forced/disabled/unanchored/wrapped states and old-slope bell obligations after a scale change. [Admission](evidence/m2a4/ADMISSION.md) adds an explicit 64 KiB sampling allowance to raw-input admission; no limits increase. The forced-storm probe is not a universal scheduled-climate or full-save latency bound. Existing office/forced-lightning catch-up allocation and all-consumer effective-rate/accounting horizons remain full-envelope gates. M2a4 passes final focused/public and full workspace verification (1,928 tests); coordinator release review and commit remain pending; no M2a5 or M2b begins before its accepted commit.

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
