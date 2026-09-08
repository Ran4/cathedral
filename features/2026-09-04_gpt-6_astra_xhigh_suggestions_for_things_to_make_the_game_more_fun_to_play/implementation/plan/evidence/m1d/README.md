# M1d operation/resource/duty kernel — 2026-09-08

Status: Implemented, workspace verified and coordinator accepted (2026-09-08). Ordinary CPU limits and preliminary timed-fixture ceiling pass; renderer/full M13 workload evidence remains pending.

This cut implements the minimal operation owner on the real Engine path. M1a, M1b and M1c are accepted historical baselines (commits `2697a0a`, `33c0329`, `a735227`); their archived evidence is unchanged. Save DTOs/adoption remain M2/M3, and the later investigation adapters/UI remain their own milestones.

## Production contract

`EngineConfig.operations` declares bounded fixture resources and their versioned adapters. `EngineCommand::Operation(Request)` uses the existing consequential admission/digest/receipt service. Start, Replan, SetObstructed and Cancel all have their own stable command receipts; the initiating start receipt retains the actual work outcome. `InstanceId(CommandId)` includes the initiating action step, so two actions under one provider root cannot alias. V1 has one logical step, index zero; a replan increments plan revision while keeping the same step identity.

The shipped `timed_fixture` v1 adapter performs stationary NPC work within one metre of a declared resource. It increments that resource's durable `completed_units` once on actual completion. It explicitly refuses player actors, unknown resources, unavailable adapters, version mismatch and out-of-range starts. This is a foundation fixture, with no quest, legal order, examination, player movement lock, prompt verb or player UI implied. Later adapters enter this same owner through their versioned services.

Admission validates the stored representable budget, actor/incarnation, resource, atomic service, mandatory duty, instance count and protected-root capacity before taking claims or replacing a route. Accepted `go_to` is explicitly Superseded and its binding released. Ordinary movement cannot overwrite an active fixture claim. Starting an atomic queued/drawing well service or queued/eating food service must finish first; cancellable approaches/returns use the existing queue/reservation release paths.

All new runtime anchors use `LogicalTime`; recovery uses `ExclusiveDeadline<LogicalTime>`. Work amounts are explicit seconds. At each Engine boundary, active work refreshes the existing shared Round need clock, then reconciles duties/incarnations before physical motion. Existing custody and ordinary Round processing run before fixture progress/terminal commit. Work credit then precedes FIFO commands: completion wins over a newly delivered same-time Cancel/Replan; an already due recovery cutoff or mandatory duty interrupts before completion. An obstruction set/released by those commands changes the next elapsed span. Blocked or out-of-range intervals add no work, while observation/recovery time continues. Replan never resets accepted time, accumulated work, last progress, retry spending, obstruction revision or recovery deadline.

The single movement priority policy is, highest first: custody, urgent danger, scheduled road return, curfew, critical needs, committed work, conversation, ordinary needs, LLM travel, routine movement. Existing custody retains its separate real prisoner/escort responsibility. A generic terminal result releases only its own actor/resource claim. Routine Round, resident local/weather routes, conversation interruption, stock-plan holds, production eligibility and the shared route/movement writers obey this owner. Road return and duty reconciliation explicitly preempt it. A held production batch keeps its reservations and resumes without crediting the blocked interval.

Terminal cleanup removes the active record and both claims immediately. The original result remains in the M1b replay ledger. Shared-root cleanup checks pending tickets, travel, round edits, accepted recordings and all kernel siblings; a control runs cleanup again after its own ticket finishes. Recording/fixture siblings retain each other's root in either completion order.

## Bounds and validation

| Owner | Bound |
|---|---|
| Active fixture instances | 256, independent of the existing 256 protected roots; several instances may share one root. |
| Resource declarations | 256; exact IDs 1–64 bytes with no control characters; finite positions; registered adapter/version. Retained config and admitted identifiers are compacted. |
| Step/recovery | One v1 logical step; existing command steps 0–256; at most 32 replans; checked obstruction revision; work/recovery at most 86,400 accepted seconds. Recovery must be representably longer than required work. |
| Retained kernel | Conservative 2 MiB heap limit, counting all owned string allocations and all BTree indexes. The bound allows one maximum-sized BTree node per live entry, which exceeds actual shared-node allocation. At 256 active instances with maximum 64-byte IDs: 139,411 active-record encoded bytes and 1,440,256 conservative retained heap bytes, below 2,097,152 bytes. |
| Recording root index | At most eight accepted SpeechRouter jobs/parked/resolved tasks, using existing admission. |
| Receipt storage | Existing 4,096 recent entries, 256 referenced entries/roots, 32 producers, ≤1 KiB encoded entry and ≤4 MiB recent-ledger bound; no new unbounded terminal history. |

`validate_continuation` checks a decoded candidate against its declarations, current actor incarnations, exclusive indexes, bounded allocations, exact step identity, representable/monotonic budgets and matching live receipt/protected root. Checked logical-time scalar decoding rejects negative/nonfinite values. This is validation of candidate values, not full checkpoint encoding or restored-run evidence; M2 must supply DTOs, bounded decoding and adoption without polling or rerunning effects.

## Verification and handoff

The final full-workspace run exited zero: **1,865 passed, zero failed, eight preexisting ignored tests across 33 targets**. The 25 added tests exercise the real Engine and Round consumers: accepted/running/completed receipts, replay and exact command/deadline ordering, actor/resource contention, sibling identities, capacity refusal before mutation, retry exhaustion, incarnation loss, mandatory duties, atomic refusal, superseded travel, resident routes/reservations, road return/lightning, production pause/resume, custody care, shared recording roots and need decay between Round ticks. Candidate-validation, retained-allocation and checked-time decoding witnesses complete the coverage.

- [Verification manifest](verification.json): exact commands/environment, process exits, per-target totals, raw temporary paths and original/archive SHA-256 hashes.
- [Full workspace log](workspace-final.log): `cargo test --workspace --no-fail-fast` on the final recorded source.
- [Focused kernel/Round log](unit-focused.log): 13 passing tests, including the printed full-window allocation witness on final source.
- [Focused Engine log](engine-focused.log): 11 passing operation tests; supporting run before the final allocation-validator adjustment and formatting, with exact final source subsequently covered by the full workspace run.
- [Workspace source hashes](source_hashes.json): 23 changed/new Rust files plus unchanged Cargo.lock; includes the coordinator's publication probe. These preserve the workspace-tested example versions before any later CPU-harness timing changes.
- [Selected owner fields](owner_fields.json): 26 indexed owners and 334 fields; new kernel/config owners are additionally specified in the persistence delta.
- [Plan validation](plan-validation.json): cross-reference and milestone checks after this handoff update.

[The completed CPU measurements](performance/README.md) preserve all 45 raw runs / 75,220 samples. The original M0 harness is unchanged; its matched ordinary comparison passes the preset relative and absolute limits. The operation probe uses cognition unavailable and 60 Hz boundaries: +2,000 residents with 16 continuously active fixtures add median paired p95/p99 of 0.462/0.431 ms. A separate short-work trace completes all 16 counters exactly once. Production publication allocation accounting is measured separately. [The sole post-workspace example change](post_workspace_probe_change.json) adds the bounded poll-duration option; production code and tests are unchanged. [Coordinator checks](coordinator-review.json) verify source/log hashes, per-target totals, every raw run’s percentiles and numerical gates. This timed-fixture result does not replace M13’s full activity mix or renderer/focus evidence.

Exact build/test environment: `CARGO_HOME=/tmp/alibi-m1b-cargo CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1`, cargo `/home/ran/.cargo/bin/cargo`. Python uses `UV_CACHE_DIR=/tmp/alibi-uv uv run --no-project`. Cargo uses the existing isolated temporary registry/cache, with no shared registry writes. Formatting is scoped to changed/new Rust with `skip_children=true`. Historical archives and unrelated untracked directories remain untouched.

The [persistence delta](../../PERSISTENCE_INVENTORY.md#m1d-operationresourceduty-owner-delta--2026-09-08) names each owner and M2 obligation. Extra active-duty need refresh reuses resident/hearth support formulas but samples them more often while operations exist; the no-double-decay witness checks the shared anchor and unchanged actors. CPU overhead is measured separately. No claim is made for save/load, later adapters, a player controller lock or unavailable renderer performance.
