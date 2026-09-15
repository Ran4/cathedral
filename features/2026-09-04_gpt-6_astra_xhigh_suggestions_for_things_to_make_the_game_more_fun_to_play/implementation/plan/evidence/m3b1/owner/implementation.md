# M3b1 implementation boundary

M3b1 supplies executable real-file preparation and actual LocalEngine disposal
transport. M3b2 remains required for complete App adoption. No wire version,
profile cap, typed cap or 1 GiB aggregate cap changes. No unsafe Send.

## Preparation and return ownership

`CheckpointPreparation` is Linux-gated like M3a storage. It owns one serial
worker, one preparation slot and one independent retirement slot. Its disjoint
4 MiB Running sublease covers the explicit 2 MiB thread stack and 2 MiB fixed
control/TLS/diagnostic/wrapper allowance. The persistent service charge has an
actual named scope; it does not account for the live Engine, ECS, immutable
recipe, candidate, old generation, provider/device runtime, or global log queue.
Those allocations require separate lifetime charges, without double counting.
A 512 MiB Running reservation is only the established minimum/trusted caller
contract, not an introspected heap census. No two-world residency proof is made.

`reserve_load` accepts the actual same-budget M3a `LoadedCheckpoint`. It admits
recipe storage BEFORE `PreparationPermit::submit(builder)` constructs owned
captures. Inputs already captured by the caller need their own existing charge;
the API cannot validate arbitrary Rust closure allocation bounds. The production
immutable startup recipe and its actual shared cache accounting belong to M3b2.

Worker `CompleteCheckpointInput::prepare_hydration` admits assets, then decode
admits full definition/diagnostic scratch before invoking the factory. It runs
exact installed role manifest comparison and the existing all-sixteen typed
owner/agreement/root validator. Raw bytes and resolver scratch are disposed
before delivery. `Admitted<DecodedHydration>` is opaque Send. Its construction
performs exhaustive fixed-count moves into the non-Send Engine, without seeding,
polling, service submission or raw/typed decoding on the host.

`PreparedDelivery` retains the SAME preparation return entitlement across
Decoded/Hydrated/Prepared/Bound stages, failed preparation/binding, cancellation,
service close and service destruction. Cancelling never needs the retirement
slot. `is_current` prevents further construction/preparation/binding after
cancel/close. Drop detaches actual Send services, disposes only empty host
forwarders, then returns an opaque `CandidateDisposal` with its original Load
leases. No large graph Drop fallback exists on the coordinator thread. Worker
abandonment waits for every outstanding delivery/permit; it never joins in Drop.

Retained M2c methods return the actual admitted graph on error. A partially
prepared failed candidate is disposal-only, not a rolled-back retry. Legacy
consuming APIs remain available and may synchronously drop on error; legacy
bind_services still propagates factory panic. New retained bind catches it.
Factory/builder errors and panic payloads are disposed under their admitted
charges; retained diagnostics copy at most4096 UTF-8 bytes into bounded capacity.
Cancelled factory captures and returned services are destroyed under their
recipe/candidate charges even when their destructors panic. Arbitrary provider
or destructor behavior has no universal frame bound.

## Actual old-host retirement

`LocalEngine` now owns `ForwardingServices` separately from the Engine's erased
trait adapters. The real fake cognition staging uses Arc<Mutex<FakeCognition>>,
so it moves with the old services; the Engine itself remains non-Send.

`LocalEngine::retire_to_worker` receives the real EngineGuard, BridgeHandle,
BridgeInbox and an already admitted RetirementPermit. It refuses while the whole
bundle remains intact if startup, generation or pin checks fail. After preflight
it pins actual mailbox/command/publication lifetimes, fences without draining,
extracts actual Send services, exhaustively splits Engine state from its known
empty adapters and infallibly submits all old owners to the reserved slot.
The old actual receivers, PromptLog, backend handle/session guard, fake staging,
command endpoint and publication accounting move with that payload.

Worker disposal explicitly drains callbacks, commands and publications before
endpoint destruction. Crossbeam disconnect alone would retain queued payloads
behind surviving senders. Command and callback publication mutexes serialize
fence with enqueue, preventing new queue entries after the fence/drain boundary.
The real PromptLog writer fence and its destructor run on the worker. External
mailbox receivers/senders/jobs and their queued charges retain an acyclic shared
RetirementLease. Bridge command clones and externally held PublicationAllocation
Arcs similarly pin the cohort. Shared pins own no channels.

`owners_disposed` means transported owners were destroyed. `released` is only
true after the last tracked lease owner disappears; an unconsumed retirement
result continues to occupy the one service slot. The worker keeps its lease
outside catch_unwind so destructor panic payloads die before its charge releases.
The original Running charge cannot be released/promoted based solely on these
booleans until M3b2 reconciles actual cohort ownership and remaining allocations.

## Named M3b2 closure

- Build one immutable production definition recipe at startup from installed
  assets/config/image/roles; admit captures and any shared navigation caches.
- Bound and charge real live Engine/ECS/services/producers and global prompt
  queue retention, including exchanges that outlive the PromptLog handle. Tokio
  shutdown_background and detached STT/TTS Drops do not prove all runtime work or
  external allocation destruction. Do not equate transport completion with that.
- Promote candidate/Running/Retiring cohorts with actual lifetime transfer;
  preserve persistent3 MiB storage and4 MiB preparation overhead across loads.
- Extend PreparedDelivery with an atomic whole-host consume/adopt operation,
  retaining its original cancellation disposal entitlement through ECS staging.
- Final-bind saved accepted logical time, virtual/fixed clocks, residual and debt
  without counting preparation/offline time. Clear old teleports/input, restore
  both physical samples and controller velocity/view/jump/ground/flight.
- Adopt all HostCandidate ownership, mirror/UI/readables/custody/vermin cues,
  bindings and generation together. Explicit deferred commands and transform
  propagation barriers are required; ordinary propagation already ran before
  HostCaptureSet in PostUpdate and the chain uses chain_ignore_deferred.
- Bevy0.19 message counters cannot be assigned through public APIs. Implement a
  bounded documented cursor rebase preserving semantic IDs and unread ordering.
- Preserve already-attached save IDs/results across world replacement. Uncaptured
  old-world save intent must not attach another world's capture.
- Durable creation/fsync of storage-root ancestors at startup, real renderer-free
  App integration, nonzero residual/debt/custody/readable fixtures, same-image
  continuation, failure/cancel/staging coverage and separate coordinator phase
  samples. This cut does not claim whole-App or renderer/frame acceptance.

## Verified storage predecessor correction

The initial concurrent workspace exposed an existing directory-lock lifetime
defect. A deterministic duplicate-descriptor regression reproduced it before
the fix. Store now owns explicit LOCK_UN in Drop and establishes that guard
before its fallible startup sync, so joined disposal does not leave a lock
behind an inherited descriptor. Live concurrent writers are still refused.
See storage-lock-lifetime.md for exact before/after evidence and limits on
attributing the original concurrent failure.

The second concurrent workspace exposed an M3b1 failed-binding field-order
regression. Failed actual services now precede both asset and service leases in
PreparedContinuation, preserving their charge through disposal. Both the legacy
failure/panic test and a retained returned-candidate lifetime regression cover
the corrected order; service-drop-order.md preserves the failure provenance.
