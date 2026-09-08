# M2a11 existing scheduler owner coverage — 2026-09-08

Status: Implemented with 18 new private checks and 6 independent public boundaries passing. The final focused suite passes 151 checks; the serial full workspace passes 2,069 with 0 failures and 31 ignored. Independent coordinator release review passed; see [review](coordinator/review.md). Opaque component candidates only; complete saves, production installation, M2c execution replacement and host adoption remain pending.

| Existing authority | Exact V1 policy |
|---|---|
| NpcScheduler.order | Ordered ActorId vector, including duplicate weighted slots and historical absent IDs; 100,000 supported slots. No reconstruction from Engine idle mode. |
| round_robin_index | Checked fixed-width cursor; zero for empty order, otherwise less than order.len(). Priority selections do not change it. |
| priority_handoffs, player_reactions | Two ordered FIFO deques, each unique and mutually disjoint; 25,000 rows each. A flying actor may be queued again because later input arrived. |
| minimum_delay_seconds, maximum_backoff_seconds | Actual normalized scheduler values, independent of original configuration. Nonnegative finite values and explicit positive-infinity Never; cap at least one and at least delay. |
| next_turn_at | Exact nonnegative finite historical future, including signed zero, or Never for positive infinity. No capture-relative rebasing or horizon normalization. |
| provider_failures | Exact counter through i32::MAX-1, leaving one safe increment and signed exponent subtraction. All-future failure horizons remain pending. |
| running | Exact flag. Closed schedulers still apply an existing completion; decoding must not equate closed with discarded work. |
| submitted | Required nullable exact actor notification. Standalone callers may retain it. Complete Engine boundary must agree with the consumed Novelty stamp; no clearing during decode. |
| retry_work | Unique actor map of exact semantic OperationId and presence_epoch, through 256 rows. Historical absent/replaced actors survive decode and retire through ordinary poll. |
| in_flight | Required nullable exact actor, incarnation, opaque full-u64 request ID, semantic root, original PlayerReaction/Handoff/Idle lane, drained_events, presented, and exact original prompt. Each input buffer has 64 rows and each string/prompt has a 65,536-byte supported bound. |
| held_result | Required nullable Completion with matching execution request, exact success or independent error kind/detail, and finite supported nonnegative duration (signed zero retained). Success has a 400,000-byte supported bound, deliberately no 100,000-scalar provider-success predicate at decode. |
| Engine.config.turn_delay_seconds, maximum_backoff_seconds | Strict {bits:u64} original IEEE values, including NaN payloads, signaling patterns, infinities and signed zero. Never run normalization over loaded actual scheduler pacing. |
| Engine.config.player_id | Exact immutable identity bound to the supplied context and existing borrowed body. |

SchedulerCheckpointContext borrows live World or an unadopted BackboneCandidate plus CommandLedgerDtoV1. Validation uses the TURN producer, issued/high-water identity and either protected obligation or known command-zero receipt. Known committed roots remain admissible for the ordinary replay guard. Distinct roots across flight/retry work are required. No temporary World or ledger candidate is constructed, no historical subject must remain current, and full protected-root owner equality stays with the envelope.

Closed remote records name every existing scheduler/flight/retry field, so adding a runtime field without an explicit persistence decision fails the generated constructor. Candidate getters expose read-only existing scheduler behavior and exact original Engine configuration. Private tests install covered fields only in independently prepared, deliberately scrambled test owners and require canonical equality before ordinary continuation. No public Clone or public partial installation was added for persistence.

Floor deferral occurs before provider-size validation. A held string above 100,000 Unicode scalars but within 400,000 bytes is exact saved authority; ordinary continuation must produce its original provider-failure behavior. Larger unsupported raw values refuse capture without truncation or invented failure conversion. Drained and presented prefixes remain outside live inbox/history while held; terminal application graduates or restores them once, ahead of later arrivals only on failure.

Full M2c must replace unfinished execution with exactly one load-specific retry of the original resolved input, including empty-inbox idle turns, with runtime-generation fencing. Current flight does not separately store the derived output-token budget; exact input receipts must bind that option and immutable content before resubmission. Held terminal values owe no provider request. Seated knowledge, receipt ownership, social notification equality, Floor/SpeechRouter interrupted-draft transformation, remaining Engine cadence/configuration, complete manifests and host publication remain pending. This cut is not complete Engine continuation or a full supported save.
