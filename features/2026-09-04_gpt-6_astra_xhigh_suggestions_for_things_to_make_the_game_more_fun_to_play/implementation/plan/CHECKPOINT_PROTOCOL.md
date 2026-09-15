Status: M1 identities/time/generation fences, M2 complete capture/continuation and M3a backend slot storage are implemented and reviewed (2026-09-15). M3b application adoption, M3c controls and M3d host-frame acceptance remain.

# Capturing and resuming one coherent city

This specifies the defaults behind [M2](M2_simulation_checkpoints.md), [M3](M3_save_load_application.md) and the [field inventory](PERSISTENCE_INVENTORY.md). It closes choices that otherwise tend to become omissions during implementation. Use these defaults unless source reconciliation demonstrates a better equivalent and the continuation evidence is updated.

## 1. Identity and clocks

Keep durable semantic identity distinct from external execution identity. A pending operation or owed response retains its ID through saving; an HTTP execution receives a new `(runtime_generation, request_id)`. Actor `presence_epoch` remains independently necessary. A restored actor may have the same stable ID as a departed or previous-world actor without being the intended recipient of an old result.

Use three explicit time domains: logical elapsed simulation time for physical work, calendar position for offices and appointments, and host monotonic time for IO/presentation scheduling. Record calendar crossing cursors in calendar units. Changing scale must not reinterpret a prior host timestamp through a newly sloped clock and invent a missed bedtime, production sweep or bell.

Both Night Office and Round need their own processed-calendar cursor. A Round tick skipped by its 20 Hz cadence must remain safe when a command changes scale in the intervening poll. Test 1×→60× and 60×→1× just after bedtime and across midnight, including the ambient nightly reroll. The current `last_office_now` names conceal this risk; renaming a float is insufficient.

Encode semantic absence explicitly. Existing `NEG_INFINITY` values meaning “never sounded/swept” become DTO variants such as `Never | At(LogicalTime)`; valid virgin worlds must save. Reject NaN, infinite coordinates and invalid numeric times after this conversion. Do not depend on JSON preserving IEEE infinity.

The novelty meeting salt currently derived from `now.to_bits()` is an opaque behavioral seed. Preserve those bits without rebasing them. Save its context and separately rebase its expiry anchor. This deliberately supersedes the old source comment saying novelty should not be saved: dropping it changes future prompt eligibility and dialogue.

The compatibility manifest includes the checkpoint schema, installed content/geometry, behavior rules and generator/hash algorithm versions. Present `DefaultHasher` use means asset hashes alone do not promise identical future procedural choices. The first supported save version may reject incompatible versions; later migrations are explicit, pure, versioned transforms. They never fill missing authority by calling world seeding.

## 2. One ordinary capture boundary

Saving must observe a boundary already used in normal play. Do not run an extra engine poll for a save: a zero-time poll can still spend a turn or consume an obligation.

1. Complete the current host fixed physics step and input sampling. Capture the authoritative `PhysicalPosition.current`, controller dynamics/view, custody strain/latch and the fixed-step residual. Render-interpolated `GlobalTransform` is not the saved body position.
2. Freeze a finite incoming command cohort through watermark **H**. Assign a final physical sample sequence greater than any action-carried position sample within this cohort.
3. Process the ordinary poll using a documented ordering of deadline advancement and FIFO commands. Accept that final physical sample in the same boundary so an older action-carried pose cannot leave the sim player behind the host body.
4. Complete ordinary domain-event flush. Assert the accepted sim player pose, yaw and physical sample identity match the host sample. If the sample is rejected, reconcile the boundary before claiming capture; never save a split body.
5. Capture before another physics tick. Include **H**, receipts, semantic obligations and readable committed presentation emitted by this poll even if host presentation has not consumed it. Inputs after **H** belong to the continuing timeline after the checkpoint.

M1a now uses this boundary on every production local pump: fixed steps finish, Update produces custody/input work, then PostUpdate freezes the input cohort and appends the final physical sample before the ordinary Engine poll finishes. Render-interpolated/throttled updates cannot leave the sim behind. Input collected later in PostUpdate retains its existing next-frame latency. `AcceptedHostBoundary` records the verified pose/sequence/elapsed/watermark; M2 captures it without adding a poll and M3 restores it alongside controller dynamics. Renderer-free fixtures lacking `PhysicalPosition` use their explicit Transform as the body; production ControllerPlugin always supplies the physical component.

Persist simulation movement residual and host fixed-step residual separately. A jump restored halfway between fixed ticks must resume with the same next tick and collision behavior. Preserve current/previous physical samples or normalize them through a documented equivalent boundary used by both saved and control runs; reset render interpolation independently so old-world transforms cannot sweep across the city.

Also preserve ordinary **unaccepted wall-time debt accumulated before capture**, separately from those accepted residuals. At adoption publish the exact saved accepted instant; subsequent normal budgeted progression services that saved debt. Add no debt for file preparation, application closure or the old world's later elapsed time. A fixture with 400 ms debt and 15 ms fixed residual must restore both distinctly, rather than silently treating them as zero or as already completed movement.

## 3. Pending work has one restore policy per state

| Captured state | Persist | Restore behavior |
|---|---|---|
| Unsubmitted semantic intent | Actor/incarnation, duty/obligation, priority and input references | Restore the intent; submit only through the ordinary scheduler after adoption |
| Submitted but unfinished cognition | Semantic obligation ID, actor/incarnation, lane, drained inbox, presented/pending history, prompt or resolved input receipt, scheduling position | Discard the old external execution. Restore exactly one retry obligation and its original inputs; preserve later inputs separately |
| Completed but unapplied cognition | The same receipt plus the exact raw success/error completion and metadata required by normal application | Hydrate a held completion without another provider request. Keep drained inputs out of the inbox and apply once under normal current action validation when the floor permits |
| Committed effects awaiting presentation | Domain event, speech/utterance identity, readable receipt and presentation progress policy | Preserve the effects. Rebuild readable presentation; never execute the speech/action again |
| Uncommitted player microphone/STT | Available text, original input purpose, draft/proposition version, learned selections and interruption status | Restore as unsent text/status. A new intentional submission is required; old callbacks cannot submit it or fall back to public chat. Preserve any earlier committed speech effects separately |

M2c keeps exact drained/presented inputs on a separate load-retry obligation;
it never restores them into the actor's inbox or merges newer arrivals into the
accepted prompt. This avoids any prefix/coalescing ambiguity and preserves empty
idle prompts. Held input likewise stays drained until ordinary application.
Loading and provider Busy do not generate failure percepts or change the failure
counter/backoff. A real subsequent provider failure is separately reported while
its exact retry remains owned.

An unfinished idle turn is still an obligation. The present `requeue_unspent_turn` intentionally ignores `TurnLane::Idle`, so it is not sufficient for load recovery. Persist a load-specific pending-turn selection state, including fairness position, so an empty-inbox/off-stage idle turn is neither lost nor resubmitted twice. A newer player priority can defer that restored idle work through the ordinary policy.

The implemented queue is bounded at64. At saturation, new ordinary prompt
creation is backpressured until an existing exact retry retires; protected
intents remain queued. Composing suppression stalls that attempt as well. This
explicit capacity exception keeps every newly reachable active/deferred state
representable on the next complete save/load. The original seated Knowledge keys
and offered occasion belong to each deferred/resumed obligation, independently
of a newer same-actor context. Application temporarily installs the old context
and then reconciles it with the preserved newer context and application effects.

For Night Office, introduce a durable duty identity with subject, owed game day, actor incarnation when applicable, and queued/submitted/completed/dropped status. Current `last_reflected` is stamped at queue time and cannot substitute for completion status. Hydrate the saved queue directly rather than calling `enqueue`, which can suppress previously spent work. Unfinished night work retries only inside its restored validity window, still yielding to the player; already completed or deliberately dropped work stays spent. Ambient rerolls use a separate once-per-day guard.

Night V2 distinguishes an inactive saved flight from an accepted replacement
execution; inactive flights cannot harvest another lane's reused numeric request
ID and still participate in lazy gate computation with an empty ordinary queue.
New person duties record their queue-time incarnation in an explicit ordered V2
extension. Historical V1 supplied only an admission-time epoch: migration uses
that epoch or the saved current person lifetime and cannot invent earlier
authority. Missing-person migration checks its dropped-counter delta before any
mutation. Queue-time stamps and the ambient guard remain exact.

Speech V2 retains nonempty interrupted-input groups, original accepted provenance
and the owed terminal interruption receipts after draining the ledger update
queue. Those notifications survive complete re-save for later M3 publication.
Original committed effects and receipts remain unchanged. The current purpose
is public player speech; absent proposition state is not fabricated. Floor clears
microphone liveness, preserves foreground/background reading deadlines, and
reconciles old voiced waits to surviving readable progress capped by the original
failsafe. Missing readable progress releases the old wait at the saved instant;
preparation does not grant a fresh post-audio beat.

Strict future state/event equality applies when exact held completions or
recorded future execution/timing are supplied. M2c retries the exact accepted
prompt/options; it does not re-render them. A live provider can still return
different prose, so prove conservation of obligations, current validation and
exactly-once effects without claiming an uncontrolled live LLM reproduces the
unsaved future. The public prepared wrapper remains quarantined and can be
captured immediately, including an older deferred retry beside newer active
work. Only insufficient legacy V1 component export APIs refuse V2 state.

## 4. Host state that is more than presentation

The saved body includes custody `strain` and `struggling_reported` together with the command watermark. They can create struggle/escape commands; rebuilding only the tether resets progress or repeats an event. Moving these into the sim is a reasonable implementation choice, but leaving them unowned is not.

Vermin currently emits authoritative `WorldSound` events from a host subsystem. Preserve its deterministic seed/config, announced boil night and percept-repeat cursors, or move that semantic scheduling into the sim. Rebuilding rats visually must not announce a second swarm to nearby citizens or spend fresh cognition merely because a world loaded.

Give `ScheduledSounds`, `CueCooldowns`, `WellSoundState`, `ClockSoundState` and `CivicBellState` explicit presentation policies. Restore owed already-committed cues where supported, discard old buffers/handles, and initialize edge detectors at the saved calendar position. Never emit a new civic event or replay an already-presented curfew/flour/well cue solely because a new mirror's first clock snapshot arrived. Cosmetic rat scattering can reset under an explicit policy; percept deadlines cannot.

## 5. Prepare, then adopt at one host boundary

Read, decode, migrate and validate bounded DTOs while the current world keeps running. Resolve the exact manifest and prepare a `Send` checkpoint/projection value on a worker where possible. Hydrate the non-`Send` engine without `Engine::new`, ordinary seed functions, `poll` or external submissions. Stage required entities/resources under an inactive generation.

At one explicit host boundary after the prior fixed step and before accepting next-generation input, check the load request is still current and swap the whole owned bundle: engine, player controller and physical samples, fixed residual/origin, custody progress, mirror and movement baselines, command/result generations, and presentation/projection state. Apply deferred ECS changes and the necessary propagation barrier before consumers can observe mixed generations.

Bind saved logical time to **host time at final adoption**. Preparation may take seconds while the old world runs; binding at file read would silently advance the loaded city by those seconds. Repeat this rule after a retry or rollback.

Publish restored state through a dedicated complete-publication API, then enable new-generation services. Do not call `poll` to force the initial snapshot. Old input queues and every old callback type are discarded or rejected by generation: cognition, Night Office, STT, TTS, PCM, microphone status, backend status and `SpeechPresented` alike.

Prefer an infallible final owned-state swap. If host staging cannot make it infallible, retain the entire old bundle and test rollback of clocks, controller, custody and presentation as well as Engine. A partial fallback to the old engine with new-world entities is not recovery.

Retirement counts toward the frame budget. `BackendRuntime::drop` can wait 500 ms, and destroying a large city can also block. Use a shared runtime with generation-owned jobs or a designed asynchronous/budgeted retirement path; do not send a non-`Send` Engine to a worker by assumption. Measure destruction and subsequent copy-on-write frames, not only the pointer swap.

## 6. Publishing slots

Use save-operation IDs independent of whichever world is currently loaded. Capture metadata records the saved timeline/instant, not completion time. Bound queued snapshots by both count and bytes; a retained snapshot can trigger later `Arc::make_mut` copies even when capture itself was cheap.

Serialize same-slot publication in request order or explicitly supersede older pending publications. A slower older save must never overwrite a newer authorized save. Loading another world while a write finishes does not reassign that write to the new world.

Write and validate a temporary file in the destination directory, flush it, atomically replace the slot using supported platform semantics, and perform required directory durability after rename. Preserve the prior valid generation until success is durable. Failures identify the failed phase without marking the in-memory capture as a successful disk save.

Use immutable generation payloads plus a small slot reference as the initial publication design. Replacing the only payload file cannot by itself preserve a previous durable generation when a later directory flush fails. The sequence is:

1. Write a uniquely identified generation temporary file beside the slot, validate its bounded envelope/checksum and flush it. Publish its immutable generation name and make that directory entry durable.
2. Preserve the prior acknowledged slot reference as a durable recovery reference before replacing the active reference. Its old payload remains present. Do not depend on an unlinked file handle surviving a process crash.
3. Write/flush the new small reference containing the generation identity, checksum and captured metadata; atomically replace the active reference and perform directory durability. Only then report success.
4. Retain the previous acknowledged generation for recovery. Collect older unreferenced generations only after the new reference is durable and no active load/save depends on them. Keep at most two acknowledged generations plus one admitted in-progress candidate per slot; user-created separate slots remain deliberate storage choices.

Fault recovery validates references and payloads without hydrating the world. If publication failed after replacement but before durability was confirmed, report the phase and offer the preserved acknowledged generation; do not claim the newly visible file was durably saved. Recovery must not guess that an arbitrary unreferenced temporary payload is the latest successful save. Test process death after each publication step as well as returned IO errors. Platform-specific atomic replacement/durability behavior must be verified by M3 before promising crash safety on that platform.

[M3a](evidence/m3a/owner-design.md) implements this boundary with a durable pending
journal before candidate creation and retains that journal through known-old
cleanup. Its initial platform is Linux ext-family, tested on ext4. The caller
must durably create the selected store root and its ancestor entries; M3a opens
an existing directory and syncs its own entries. Eight operations include unread
terminals. Save metadata binds at capture attachment, and attached operations
survive world replacement. An uncaptured intent still requires the host's
source-world policy. Load bytes retain their shared lease through delivery;
inspection memory pressure reports Admission/WouldBlock, while full installed
M2 validation remains mandatory before adoption. Process-death evidence does
not establish physical power-loss behavior or prove a former UI observed its
last acknowledgement.

## 7. Required adversarial continuation evidence

In addition to ordinary populated-world round trips, M2/M3 must cover:

1. Repeated saves with zero elapsed simulation time create no extra turns or world events.
2. A jumping body resumes at nonzero host fixed overstep and sim movement residual under identical subsequent inputs.
3. A queued custody threshold command produces one escape/percept and retains prior resistance progress.
4. A rate change immediately after bedtime, on a skipped Round tick, processes each office/night/settlement exactly once.
5. An unfinished idle prompt with an empty inbox survives restoration off stage; a newer protected player reaction still gets priority.
6. A held completion captured after newer speech preserves both the old input graduation and the newer owed response.
7. Night duties survive queued/submitted/completed boundaries, midnight and actor departure/re-entry.
8. A rat swarm percept restores on either side of its next repeat deadline without duplication or suppression.
9. Every callback type from the old generation is inert even when its numeric request ID is reused.
10. Delayed load preparation still resumes exactly at the saved instant upon adoption.
11. Reverse-order completion of two same-slot writes leaves the latest authorized publication in the slot.
12. Failure injected at each write/adoption phase leaves a valid save and one coherent running world.
13. Virgin `Never` values work; invalid floats, counter overflow, missing adapters, changed ordered catalogs and incompatible geometry fail before adoption.
14. Nonzero ordinary wall debt survives capture/adoption separately from accepted physical residuals; delayed preparation and offline time add none.

M0 establishes numeric capture/adoption/frame-time and retained-memory budgets on available reference hardware, with default cast, 2,000 citizens and separately labeled 20,000 stress measurements. “No visible hitch” is an additional human check, not the only performance gate.

## M2a15 implemented host component boundary — 2026-09-09

`HostCaptureSet` is ordered after the complete ordinary `DrainBridge` and before
`ReconcileMirror`/`CollectInput`. `HostObservation` borrows actual owners at this
point. It requires the completed accepted physical boundary, matching single
controller/body owner, ready current-generation runtime and empty BridgeInbox.
It never polls, drains input, waits for a worker/provider or mutates the live
world to make capture eligible. Post-H nonconsequential worker arrivals may stay
queued. The independent host command allocator is captured from successful
enqueue authority; it need not equal H or the sim ledger high-water, because
valid refused-history gaps exist.

Current-generation unread PlayerIntent at its actual forwarding cursor is saved
as intentional but unsubmitted work, including its frozen choice/pose. Ordinary
forwarding supplies a fresh monotonic spatial identity before enqueue; it does
not restamp accepted commands. This handles real PreUpdate chat submissions
whose earlier spatial identity is passed by the pump's final physical sample.
Recording intentions carry explicit interrupted metadata, never raw microphone
bytes or an instruction to restart recording.

Unread committed PresentSpeech and semantic CivicBell are captured at their
distinct actual consumer cursors. Existing subtitle, independent ECS bubble and
timed HUD owners retain exact original words/attribution and readable progress.
The body reflex reader is separate and cosmetic. No capture acknowledgement,
audio stop, provider retry or old sound replay occurs. Exact M2c interruption and
M3 new-generation consumer publication remain separate implementation gates.

The pure `checkpoint::host` API admits a closed V1 scalar/typed-row component.
Decode validates against borrowed actual Engine authority or already-admitted
backbone/law/knowledge/marks/climate/animals/ledger candidates sharing the same
boundary/player. It needs no live replacement Engine/World. Actual collision,
CutMargin, gate kind/geometry, vermin definitions/configuration and installed mark
catalog/algorithm identities bind the continuation; nullable owner presence
cannot be removed under unchanged definitions. Historical item/actor choices
can outlive present world entities and are never silently retargeted.

[Owner coverage](evidence/m2a15/OWNER_COVERAGE.md) distinguishes exact saved owners,
rebuilds, interrupted device/service work and private prepared-owner evidence.
This component adds no production hydration/adoption path and does not establish
complete root/category agreement, full-cohort lifetime or frame-budget acceptance.

Supported host V1 fixture bytes were added on 2026-09-14 under
`crates/cathedral-sim/tests/fixtures/checkpoint_host`: initial actual host and
active readable/pending work. Their ordinary test decodes against the compatible
actual boundary and checks exact canonical roundtrip/candidate bytes there,
before later input forwarding changes issued. Only a detached fresh-export
generation fence is aligned for comparison; stored/candidate/live identities
remain untouched. Independent fresh writers agree exactly, with originals and
hashes in [fixture evidence](evidence/m2a15/owner_fixtures/fixture-equality-1.json).

## M2a16 complete read-only envelope — accepted 2026-09-15

The mandatory V1 outer fields are version, profile, world_identity, boundary,
manifest and ledger/operations/backbone/round/climate/knowledge/law/marks/animals/
social/continuity/scheduler/night/speech/cognition_inputs/host. Categories preserve
their existing V1 canonical byte contracts. Missing, duplicate, unknown, null
and unsupported outer values refuse; profile accepts only its string spelling.
Original input whitespace remains part of the admitted raw envelope.

Compatibility binds actual installed parsed definition roles separately from
saved mutable authority, ordered accepted seed/prompt inputs, exact compiled
source/toolchain/target/features and the running host image. The host hashes
`/proc/self/exe` once at startup using bounded streaming scratch; failure disables
capture only. The pure simulation does no IO. Git HEAD is supplemental evidence.
Exact-image compatibility intentionally permits debug/release/separately linked
executables to reject each other's envelopes. Runtime generation and a fresh
host's initial lineage do not participate in installed-definition equality.

Complete validation checks the semantic-root union, initial configuration and
saved publication relationships, full ordinary boundary, and next-consumer time
horizons. A100 ms next frame may advance at most one game day; seeded Round must
agree with navigation presence and retain its office cursor, and all three Round
clock consumers must be within three days of the next position. Existing Never,
negative calendar/history, checked-refusal and wrapping-counter semantics remain;
unchecked World counters receive complete-poll headroom.

The admitted candidate owns immutable raw bytes, category offsets and its exact
proof context. Later hydration must rebind that manifest. No production Engine
hydration, retry, interruption transformation, external adoption, file replacement
or new-generation publication occurs in this leg. Runtime-read fixtures are made
after final image freeze; an incompatible image must explicitly refuse them.

## M2b quarantined complete hydration — accepted 2026-09-15

Hydration consumes an admitted complete LoadCandidate through
prepare_hydration(asset_upper_bytes) and hydrate(factory, host_definitions,
RuntimeGeneration). The factory is invoked only after its disjoint subordinate
LoadCandidate lease is reserved. Actual original parsed WorldSeed, PromptEnv,
config and separately identified World asset roles must reproduce the exact
manifest; validation scratch precedes fresh resolver allocation. Shared asset
owners that outlive their caller remain covered, and caller-retained navigation
Arcs require coordinated admission for any later interior cache growth.

One fully validated private typed graph moves into exhaustive World and Engine
literals. There is no seeded temporary Engine, replay, ordinary tick, request,
provider poll, drain or publication. Saved semantic roots, Scheduler/Night held
and unfinished work, Floor and speech-action identities remain conserved. Typed
speech interruption/accepted-recording state, complete exact cognition inputs
and Host authority remain explicit continuation owners; empty inert transport
services do not discard them. M2c must prepare those obligations jointly before
execution; M3 must adopt the complete application bundle.

Saved lineage and all logical/calendar coordinates remain unchanged. The new
Engine generation is nonzero and differs from the saved Host fence; saved Host
generation remains historical authority. No host origin is bound during this
step, and preparation/offline elapsed time creates no wall debt. Omniscient
session transcript is empty by policy; committed readable Host lines remain
owned. New service availability is deferred until explicit runtime rebinding.

The result retains actual new owners, without raw checkpoint bytes. Public
category_digest streams those owners through a same-budget SavePayload lease,
and refuses an independent budget before staging. No public World borrow is
provided because immutable World access would expose NavData's interior mutable
cache. The256 KiB structural hydration allowance joins the unchanged128 MiB
complete expansion cap; factory and retained assets use their separate admitted
lease under the unchanged1 GiB aggregate. Charges remain attached through raw
disposal, errors, observer unwind and final owner destruction.

HydratedEngine inherits Engine's non-Send service trait objects. M3 still owes
Send decoded-bundle offload plus bounded host construction/binding, and measured
frame scheduling. Persisted M2b fixtures are generated after exact image freeze
and read in fresh same-image processes; other images must explicitly refuse.

## M2d complete future comparison — implemented 2026-09-15

The test-only full-envelope harness continues the actual admitted prepared Engine
under ordinary bounded polls with controlled external service responses. Its
external CPU Host starts from decoded Host scalars/records and advances exact
accepted time, physical samples and sampled publications. The original prepared
Host/boundary stays fixed. This is behavioral verification of simulation
continuation, not production host adoption, aggregate heap or frame-cost proof.

Each observation compares deterministic re-encoded bytes for every owner and
complete future EngineMessage vectors. The ordinary execution exclusion is
exactly `/host/scalars/boundary/generation`. During one unfinished Scheduler
obligation, separate full admitted observations enter the common prepared
representation; only `/scheduler/continuation/load_retries/0/flight/request_id`
differs. Original flight fields, input text, root and incarnation are pinned
independently. The exact one replacement LLM Thinking status is asserted, then
all other messages compare. After its recorded result settles, direct complete
equality resumes without representation edits.

Unfinished Night has an explicit retry pacing policy: a replacement submission
starts its pacing at retry time. The test asserts the original and replacement
values at `/night/night/night/next_attempt_at/at` before adjusting the control
expectation to that declared delta. While the replacement is active, its two
external identity paths are `/night/night/night/in_flight/request_id` and
`/cognition_inputs/night/request_id`; all other input/duty/root authority remains
compared. This is unfinished-work conservation, not a promise about an unsaved
live provider future. Queued/recorded Night and held recorded success/error use
direct full state/event equality. No whole owner or durable identity is ignored.
The [owner design](evidence/m2d/owner-design.md) specifies fixture witnesses and
the known ordinary escort endpoint-policy interaction. M3 adoption and the M0
renderer/full-stress gaps remain separate acceptance gates.
