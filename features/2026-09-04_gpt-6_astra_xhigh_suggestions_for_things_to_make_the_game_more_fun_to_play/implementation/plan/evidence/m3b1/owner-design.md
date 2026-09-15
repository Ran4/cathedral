# M3b reconciliation and proposed sequential boundary

2026-09-15; approved by root for implementation with delivery-slot, infallible retirement, mailbox-pin and pre-admitted factory requirements. Accepted source is
M3a 5559d26654faba677971ceeef66f0869495e6489. No Cargo has run in this cut yet.

## Why two cuts

M3b1 will implement real bounded preparation and retired-owner transport; M3b2,
assigned explicitly to the next sequential fresh owner, will implement the
production App coordinator, shared-cohort promotion, inactive ECS staging and
whole-host adoption. M3b remains incomplete until both are accepted. M3b1's
executable endpoint is a real M3a slot loaded, fully validated/decoded on a worker,
constructed and continuation-prepared without a poll on its host thread; actual
old LocalEngine/backend owners are dismantled and disposed by the same bounded
worker. This is materially more than a declaration or test-only Engine adoption.
There is deliberately no M3b1 claim of restoration into an App.

## Worker preparation

`checkpoint/complete/hydration.rs` currently does all work synchronously in
hydrate_observed. `engine/complete_checkpoint.rs::decode_components` returns the
full private ValidatedOwners graph. `world/checkpoint.rs::BackboneCandidate::hydrate`
moves already-built BTree/PlaceRegistry indexes; `engine/hydration.rs::construct`
exhaustively moves the other owners and creates only fixed roots, an empty router,
four inert service objects and the at-most-eight speech-action index.

Add an admitted `DecodedHydration` whose private fields are ValidatedOwners,
HydrationAssets, saved boundary/lineage, chosen generation, cost and the existing
asset lease. Assert Send in a compile-checked test. Do not make Engine Send. Split
the existing API into worker-eligible decode/raw disposal/retention and a
host-thread construct method; the old synchronous hydrate wrapper delegates to
both so existing M2 users and all sixteen-owner contracts remain exercised.

Add a direct CompleteCheckpointInput preparation entry point that admits resolver
scratch and a disjoint asset/factory allowance before invoking the factory. It
uses the same complete framing, exact installed manifest and shared-root/numeric
validation, retains the decoded owner graph instead of throwing it away, drops
resolver/wire/raw input on the worker, then shrinks to retained typed storage.
Refactor a common validated-decode helper rather than duplicating the closed
owner-validation logic or decoding an unvalidated candidate by assumption.
The existing structural and typed caps remain; no extra full DTO copies.

The backend `CheckpointPreparation` service has one explicitly bounded serial
worker and one retained preparation operation (queued, active or unread result).
It consumes the original LoadedCheckpoint and carries its exact SlotReference
through preparation. Its factory is Send and is invoked only after admission;
captured factory state is bounded small immutable configuration/definition input,
not an Engine, World or Bevy borrow. Rejection returns the untouched input/factory.
M3b2 installs an immutable startup definition recipe using original parsed input
roles/configuration; it must not use the current test helper that borrows Engine.
For M3b1 the production service consumes a caller-supplied admitted factory;
real-file tests construct independent immutable recipes before the running world.

Cancellation before or during expensive worker phases marks the request obsolete,
but its slot and lease remain until the worker actually disposes it. No replacement
thread starts to bypass a slow phase. Once a completed value is transferred to the
host, explicit return-for-disposal uses the same slot/worker rather than dropping
the typed graph in a coordinator frame. Shutdown is a signal with no join; unread
values are disposed by the worker on receiver abandonment. Terminal status is
preserved while the handle is retained. Factory errors/panics and validation
failures dispose assets/raw/typed values before releasing their leases.

## Actual retirement, including service destructors

An exhaustive private Engine destructure separates Send domain owners from the
four non-Send trait objects. The resulting opaque domain-disposal value exposes
no World access, poll or reseeding route. Its actual owner bundle, including
transcript, scheduler, router, Round, nav/assets and publication caches, travels
to the worker under an independently admitted RetiringGeneration reservation.
Compile-time Send checking verifies the concrete field split.

The production LocalEngine services currently erase Send and fake cognition uses
Rc<RefCell<FakeCognition>>. Simply dropping those boxes on the main thread is not
an acceptable solution: SttEngine/TtsEngine own workers; their Drop paths may do
IO. Introduce host forwarding adapters whose concrete service storage is held by
a separate owner and can be taken as Box<dyn Trait + Send> at retirement. Engine
keeps ordinary non-Send traits and forwarding semantics. Fake staging likewise
retains its real data until the owner moves it. The main-thread adapter teardown
then drops only empty/shared lightweight wrappers; real service destruction is
on the retirement worker. Preserve actual fake and production callback behavior.

LocalEngine's retirement method first reserves/accepts the retirement slot while
the old world is still intact. Only after this succeeds does it fence the bridge
and backend generation, detach the Engine's domain/service owners, and move the
old completions/commands/events/fake staging and BackendsHandle onto the worker.
PromptLog must travel too: its Drop calls flush(), which currently waits without
a timeout. SessionDir::drop recursively removes files. Neither runs on the
adoption thread. Retired command/event queues and backend callback records are
disposed on the worker; immutable old endpoints remain retired while numeric
request IDs are reused in the positive-control fresh generation.

One retained retirement operation is independent of the one preparation operation,
but both share the same worker and unchanged global four-cohort budget. A blocked
retirement can delay another preparation; the old running App remains responsive.
Retirement is not declared complete until payload/services/queues/SessionDir are
gone. Completion status itself is bounded fixed metadata.

## Accounting and promotion boundary

M3b1 adds a disjoint persistent Running overhead lease covering its explicit
worker stack, bounded queue/control objects and fixed scratch. Exact bound will
be source-derived before allocation; provisional 4 MiB is 2 MiB stack plus 2 MiB
control/runtime scratch (not a heap estimate for world/assets/factories). This is
additional to M3a's persistent 3 MiB. Factory assets and retirement payloads use
separate admitted bounds, including spare capacity and last-shared-owner storage.

M3b1 retirement requires a trusted caller bound for the actual surrendered Engine,
services/channels/log/session and shared assets. Tests use independently bounded
constructed histories plus actual capacity witnesses and retain the original
Running owner through disposal. This conservatively overlaps some original
charge; it does not claim an arbitrary long-lived played world fits a fixed
number. No charge is shrunk because a handle was merely fenced or queued.

Atomic cohort promotion belongs to M3b2. Current Reservation stores its Cohort
directly, so renaming a Slot behind it breaks later drop accounting. M3b2 needs
stable cohort-group identity and an atomic checked transition that transfers
persistent Running children (storage/preparation workers and actually shared
static assets), demotes only the disjoint old generation and promotes the fully
prepared candidate. All surviving subordinate leases follow their real group.
The 512 MiB minimum Running requirement remains, and two 512 MiB minima plus
service overhead cannot coexist under 1 GiB. Therefore the old-runtime charge
must be based on its actual disjoint retained owners after static/shared ownership
stays Running; duplicating both minima or shrinking by wishful release is invalid.
The existing prompt-log writer's global unbounded queue also needs explicit
production admission in M3b2; a moved PromptLog does not own that entire queue.

## M3b2 concrete adoption responsibilities

The next owner wires one App coordinator to durable root creation/parent fsync,
M3a storage and M3b1 preparation. Save operations survive load; uncaptured save
intent is tagged with source-world identity and is explicitly cancelled at load,
while attached payload/results keep their original operation identity.

Build inactive generation projections incrementally, with bounded per-frame
staging/despawn queues and actual allocation accounting. Include mirror and
movement bindings, doors/items/lamps/marks/weather/custody, host soundscape/vermin
semantic cursors, saved unread/readable/pending/draft consumers. Stage the complete
initial publication through a dedicated no-poll API before enabling submissions.
Never reconstruct private authority from PublicSnapshot or drop private owners.

At one exclusive boundary after the prior ordinary poll/capture and before new
input/fixed progression, recheck current request and staged readiness, perform
checked admission transition, fence old endpoints, and infallibly swap all owned
resources/controller/time/projections. Bind host origin at that final boundary.
Restore previous/current body, velocity/yaw/pitch/ground/jump/flight; reset only
render interpolation, not physical residual. Restore accepted elapsed, ordinary
debt, fixed timestep/elapsed/delta/overstep and virtual clock policy separately.
Clear/fence old TeleportPlayer and ephemeral ControllerInput; saved drafts remain
unsent. Reconcile host message-count/cursors with an explicit bounded rebase that
preserves semantic identities and unread order (Bevy's counters are private).

The current PostUpdate smart-actor chain is after TransformSystems::Propagate
and uses chain_ignore_deferred. Therefore explicit deferred-entity application
and transform propagation are required before focus/presentation/input consumers
see the new generation. Normal propagation cannot be assumed to have run later.
Old ECS entities retire incrementally, and domain/service disposal uses M3b1.
No failure before this final barrier changes the old whole bundle.

## M3b1 executable verification and evidence

- Real M3a writer/load, independently built immutable installed definitions,
  worker decode, host construction, M2c preparation/binding; compare all sixteen
  actual hydrated/prepared categories and saved host/lineage/logical boundaries.
  Fresh same-image child writer/reader covers the real file/service path.
- Delay worker phases while ordinary fake old-world polls continue; saved instant
  stays exact (final host-origin binding itself remains M3b2).
- Malformed/incompatible definitions, generation reuse, admission pressure and
  factory panic before/between phases leave the running world unchanged; prove
  owner-before-lease release on every refusal/cancellation/disposal path.
- Repeated cancellation with a save retained and with one actual retired LocalEngine
  blocked on disposal; no extra load/retiring cohort and no lost save terminal.
- Actual LocalEngine retirement moves services, PromptLog, session and queued
  completion payloads; test thread IDs/drop gates prove their actual destructor
  ownership. Old callback families remain inert with reused numeric IDs and a
  live fresh positive control. No provider/device/audio invocation.
- Separate raw timing arrays for worker validation/decode/raw disposal, host
  construction/preparation/service detachment, retirement enqueue, callback
  disposal and later full disposal; no aggregate pointer-swap-only claim.

Use existing component-inputs-v2 command-start/result maps, exact env, original
logs and mtime-zero gzip. Preserve failed/development attempts. Root owns public
tests/review and final commit. M3b1 will cede source/Cargo before that review;
M3b2 whole-App/frame/renderer acceptance remains explicitly open.
