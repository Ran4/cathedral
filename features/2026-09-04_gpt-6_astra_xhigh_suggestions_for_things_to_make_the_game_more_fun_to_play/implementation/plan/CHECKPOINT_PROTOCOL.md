Status: Proposed implementation contract (2026-09-05); owned by M1–M3, not implemented.

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

The implementation must make the same ordering apply with and without a save request. The frame schedule currently pumps before its interpolated/throttled position synchronization, so M1/M3 must change that seam deliberately rather than assuming “same frame” means coherent.

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

Restore drained event/history prefixes chronologically before newer arrivals, applying the existing bounded-buffer/coalescing policy without losing protected reactions. Do not both restore the drained inbox and apply its saved completion. Do not call the generic provider-failure path: loading is not a failed provider request and must not generate a false error percept or change backoff.

An unfinished idle turn is still an obligation. The present `requeue_unspent_turn` intentionally ignores `TurnLane::Idle`, so it is not sufficient for load recovery. Persist a load-specific pending-turn selection state, including fairness position, so an empty-inbox/off-stage idle turn is neither lost nor resubmitted twice. A newer player priority can defer that restored idle work through the ordinary policy.

For Night Office, introduce a durable duty identity with subject, owed game day, actor incarnation when applicable, and queued/submitted/completed/dropped status. Current `last_reflected` is stamped at queue time and cannot substitute for completion status. Hydrate the saved queue directly rather than calling `enqueue`, which can suppress previously spent work. Unfinished night work retries only inside its restored validity window, still yielding to the player; already completed or deliberately dropped work stays spent. Ambient rerolls use a separate once-per-day guard.

Strict future state/event equality applies when exact held completions or recorded future execution/timing are supplied. Re-rendering unfinished prompts can legitimately produce different prose and choices. For that path, prove conservation of obligations, current validation and exactly-once effects; do not claim an uncontrolled live LLM reproduces the unsaved future.

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
