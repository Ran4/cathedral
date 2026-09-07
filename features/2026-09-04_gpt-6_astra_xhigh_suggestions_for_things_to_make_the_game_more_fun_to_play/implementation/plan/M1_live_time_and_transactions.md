Status: Partial; paused after M1b at the developer's request (2026-09-07). M1a–M1b implemented and coordinator reviewed; combined M1b verification passes 1,810 tests (eight ignored). M1c–M1d remain. Renderer/focus performance remains unavailable.

# M1 — Live time and committed actions

Make advancing time and clear, idempotent action results foundational. Every later UI and activity relies on this contract.

## M1a implementation handoff — 2026-09-07

`timeline::AcceptedTime` and host `LiveTimePlugin` own the production accepted stream: 100 ms maximum per frame, retained ordinary debt, no overlay/focus exception. Controller fixed steps, engine elapsed/calendar deadlines and physical gates consume that stream. The normal local pump freezes a finite command cohort, appends the newest authoritative `PhysicalPosition.current`/yaw sample, flushes events and verifies the matching sim sample/watermark. Engine, Round and Night Office crossing cursors now store calendar days; Night has an explicit ambient day guard. Exclusive expiry precedes same-instant provider actions.

[The M1a record](evidence/m1a/README.md) gives interfaces, field dispositions, test commands and limitations. Direct legacy coarse `Engine::poll` calls remain an explicit debug/test API and now diagnose discarded physical spans; both headless clock watching and ordinary turn stepping use at most 50 ms. Production gameplay admission retains debt instead of entering that legacy path. M1b supplies command replay below; save/load remains M2/M3 work.

## M1b implementation handoff — 2026-09-07

The production host assigns ordered command identities, and the sim checks replay/conflict before carried position or domain effects. All 48 existing command variants have an exhaustive policy; 36 consequential variants share receipts, including voice recording submission and fire-and-forget sounds/settings/debug effects. Provider turns and Night reflections reserve complete bounded reply batches using semantic identities independent of provider execution IDs. Travel and schedule edits publish acceptance, then the existing movement/round consumers publish their actual final results. Voice speaks once through its retained recording receipt.

[The M1b record](evidence/m1b/README.md) tracks current verification, policy coverage and M2 owner obligations. Combined verification passes 1,810 tests with eight ignored, with final source hashes and per-target replacement details recorded. Coordinator review accepts this implementation cut. The user requested a pause after M1b; generation fences and the generic operation/duty kernel remain M1c/M1d.

## Entry

The M0 baseline is delivered and the shared knowledge feature has shipped. The developer [authorized continuing despite M0's recorded renderer/full-stress gaps](EXECUTION_AUTHORITY.md). Existing fake/headless tests pass on the implementation baseline; unavailable measurements remain pending.

## Existing seams

- `Engine::poll` in `crates/cathedral-sim/src/engine.rs` already drives offices, movement, custody and rounds independently of ordinary conversation-floor gating.
- `WorldClock` in `clock.rs` maps process-style `now` to calendar time. `Round` and numerous engine services retain elapsed-time anchors.
- `NpcScheduler`, `ConversationFloor`, `SpeechRouter` and `NightOffice` separately retain pending work and deadlines.
- `src/smart_actors/local_engine.rs` receives backend completions and pumps the engine from Bevy time.
- `src/controller.rs` owns physical player movement; UI input capture is different from simulation pausing.

## Implementation

### M1a — Explicit clock boundaries

Introduce a documented logical simulation-time origin that survives engine replacement. Keep host monotonic polling separate from calendar appointment times and physical elapsed durations. Existing floats may remain behind typed adapters while the inventory is migrated; do not spread ambiguous new `f64` deadlines.

Implement the accepted-time/debt policy in [RUNTIME_BUDGETS](RUNTIME_BUDGETS.md). Controller motion and domain deadlines consume one coherent accepted progression under a finite per-frame recovery budget. Record wall duration, accepted duration and debt explicitly. Removing the host's 100 ms virtual cap without repairing movement catch-up would merely trade one silent time loss for another. Ordinary overlays/focus changes are not exceptional-suspension cases; sustained overload fails the supported-workload gate.

Audit all deadline comparisons and time-scale changes. A restored anchor must neither ring offices twice nor defer them by the age of the old process. Store processed calendar positions for each crossing consumer; do not evaluate an old `last_office_now` through a new clock slope. Include Night Office after command application and Round polls skipped by their cadence. Define ordering for commands arriving exactly on a deadline: retain a single documented poll/transaction ordering and test it. Do not let UI and sim disagree about whether an appointment has already begun.

Provide a due-expiry boundary before same-instant consequential effects. Later domain adapters declare their interval semantics through it; M11 uses exclusive authority cutoffs, so an extension received exactly at expiry cannot retroactively continue the old hold. Test this seam with a generic finite resource grant here, leaving legal procedures to their owner.

Establish the ordinary host/poll boundary described in [CHECKPOINT_PROTOCOL](CHECKPOINT_PROTOCOL.md): authoritative physical sampling, a finite input watermark and matching accepted player sample, followed by complete event flush. This applies with and without saving. Save requests must not add a poll or submit extra cognition.

Test advancement while chat, inventory, evidence reading, settings and speech delivery are active. Review unfocused-window behaviour too: losing application focus must not become a hidden investigation pause while the app is running.

### M1b — Shared action receipts

Add stable command/operation identities and a typed result envelope. Reuse existing `EngineCommand`/`EngineMessage` infrastructure; add a shared service boundary rather than duplicating a command interpreter in the UI.

Results distinguish rejection, accepted work, progress, completion, interruption and supersession. Retain the affected domain references, actual time and bounded failure reason. A receipt can say the packet moved while the menu was open without revealing its unknown new location.

For immediate commands, validate before mutation. For long-running commands, commit an undertaking and let later steps publish their own results. Resolve idempotency before effects, and persist replay/high-water state in M2. Fix existing touched paths that mutate before a fallible route or authority check.

Use a bounded ordered consequential-command ledger with a persisted compacted floor and payload-conflict checks. Position samples use their own latest-sequence policy. Old IDs below the floor remain rejected after their detailed receipts are collected, including previously unaccepted commands; active/referenced operation identities remain protected. Freeze the initial 4,096-envelope budget after M0 measurement and test window exhaustion, not only immediate duplicates.

### M1c — Runtime generations

Add a runtime generation boundary for commands and all asynchronous callbacks. Prepare load-time replacement without accepting a completion addressed to the previous engine. The generation must cover STT, TTS, cognition, Night Office, PCM, microphone/backend status, speech-presented acknowledgements, pending UI commands and fake-backend staging. Durable operation IDs are separate from ephemeral execution IDs.

Do not mistake an actor's existing `presence_epoch` for a whole-world generation. Both matter: a character can leave/re-enter within one world, and an entire world can be replaced while stable actor IDs remain identical.

### M1d — Minimal operation and duty kernel

Implement the shared single-operation kernel now, before doors and examinations need it: start/progress/interruption/completion, stable step IDs, exclusive resource claims, actor-duty ownership, bounded cleanup and deterministic mechanical completion. It knows nothing about evidence, appointments or legal orders.

Integrate duty ownership with the existing round so two services cannot independently overwrite an actor's movement. Early fixtures can reserve an actor/work resource, perform a bounded elapsed-time operation and release it. M6 adds door operation adapters, M7 examinations, and M8 composes these accepted primitives into activities and appointments.

Track operation-wide last progress, retry count, obstruction revision and recovery budget. Replanning a step or restoring a save does not renew that budget. Define terminal interruption/parking separately from post-seizure custody care, which cannot simply release a real prisoner duty as generic cleanup.

Define versioned declarations and validation alongside the kernel. Later owners register typed domain adapters; an unavailable adapter is a load/validation error, never a successful placeholder operation. M2 owns the complete checkpoint manifest and saves this state.

## Behaviour contracts

| Situation | Required result |
|---|---|
| The player opens a document for five minutes | Schedules, needs, weather, custody and appointments continue |
| A bounded fixture operation is accepted | Its resource/duty ownership is visible; completion has not happened yet |
| The recipient walks away before handover commits | No transfer; a specific failed/pending result, with the draft retained |
| An input is delivered twice | One effect and a repeatable receipt or explicit old-command rejection |
| A provider answers from an old runtime generation | No domain mutation in the current world |
| A time-scale change occurs on a bell | One calendar crossing, with no rewritten past measurement |

## Tests and evidence

- Engine tests with active conversation-floor holds while movement, custody expiry and calendar crossings occur.
- Host tests that open each input-capturing overlay while the engine continues to publish advancing time and actor motion.
- Duplicate-command and no-mutation-on-failure tests for representative item and custody operations.
- Same-frame deadline/command ordering and calendar-rate-change tests.
- Generation rejection tests across all completion types, including a stale fake completion and a late `SpeechPresented` acknowledgement.
- A headless trace showing accepted → running → interrupted/completed transitions for a small fixture action.

## Completion gate

Time ownership is explicit, actions have truthful receipts, the minimal operation/duty kernel works, and every external result can be fenced by runtime generation. The existing base game still runs and existing prompt changes have reviewed golden fixtures. No investigation UI has a pause capability. Examination-specific work is verified in M7, not smuggled into this milestone's generic fixture.

Hand M2 the complete time/replay-state contract and M8/M10 the shared command/receipt interface.
