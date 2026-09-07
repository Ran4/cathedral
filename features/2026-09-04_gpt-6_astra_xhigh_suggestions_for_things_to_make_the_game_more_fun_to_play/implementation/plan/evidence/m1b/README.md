# M1b — Stable identity and action receipts

Status: Implemented, verified and coordinator accepted (2026-09-07). M1c/M1d are not started. Work pauses after this M1b leg at the developer's request.

## Production boundaries

`BridgeHandle::try_send` assigns ordered HOST operation sequences exactly once on successful enqueue, preserves explicit retries and advances its allocator when an explicit valid HOST sequence was accepted. UI request strings remain correlation only and are omitted from payload digests. The M1a finite cohort and final physical sequence recurse through envelopes.

`EngineCommand::policy` exhaustively classifies all 48 preexisting variants: 36 consequential commands, latest-position `SpatialUpdate`, attention `PlayerAttention`, six speech lifecycle inputs (`PlayerUtteranceStarted`, `SpeechPresented`, audio begin/chunk/end/abort), and four provider callback variants (LLM, transcription, TTS, backend status). The lifecycle/callback transport remains on its existing utterance/request sequencing until M1c. In particular, raw microphone audio does not allocate ledger entries; the consequential `PlayerRecording` handoff does.

Raw legacy sim calls receive a one-shot LEGACY producer sequence. Intentional transport replay uses `Identified`; repeated UI correlation text alone is not an identity. All consequential dispatch results are typed from actual service outcomes, including sound cooldown/disabled/unknown failures. Duplicate/conflict checks precede action-carried pose updates. `take_into_charge` validates its route before `custody.seize`.

## Ledger and payload format

`receipts::CommandLedger` retains 4,096 recent envelopes ordered by an independent acceptance ordinal; per-producer issued/high-water/floor metadata supports 32 producers. Up to 256 semantic roots and 256 displaced referenced records remain protected. Dispatch tickets reserve their slots before effects; a provider batch reserves its root and every action step atomically before history, transcript, priority, knowledge or domain writes. The bounded `Deferred` result is transport backpressure, not a committed domain rejection: retry after capacity clears may be admitted. Forgotten identities below a compacted floor remain explicitly expired, including holes never admitted. Malformed envelopes/raw payloads are explicitly outside domain admission: `retryable=false` requires a corrected intentional submission, not a retry of the invalid packet. A never-admitted invalid ID has no retained payload claim and can be admitted if corrected before compaction; no authoritative Rejected receipt was promised. Still-protected roots retain exact duplicates and may admit valid new steps despite an advanced floor.

Payload v1 is recursively sorted JSON object keys with serde_json scalar/array encoding, prefixed by `cathedral-command-payload-v1\0`, hashed with SHA-256 (`sha2 0.10.9`, no default features). The version is explicit; this is not an assertion of RFC 8785 compatibility. Raw command borrowed size/finite-number validation precedes JSON projection. Borrowed JSON depth/node/byte preflight and a bounded hashing writer reject over-16-KiB canonical payloads without an unbounded clone or encoded buffer.

Whole provider replies preserve the existing 100,000 Unicode-scalar limit. A 400,000-byte check precedes scalar counting; then a SHA-256 descriptor hashes the exact borrowed UTF-8 bytes with domain `cathedral-provider-reply-v1\0` and explicit reply version 1. The small root payload includes that descriptor and its subject/incarnation/day. Comments and parse errors remain part of identity, while each action still has its separate 16-KiB envelope. An already committed semantic root rejects conflicting replies, errors and oversized duplicates before any second retry/backoff/history/duty bookkeeping.

Records encode to at most 1 KiB. Human messages are bounded to 192 UTF-8 bytes; codes remain exact within 48 bytes and valid principal refs within kind 16/id 64 bytes, with controls rejected. At most two principal refs are included; they are not exhaustive witnesses or recipients. Optional oversized refs are omitted, never rewritten. The receipt identity still names the exact action. Actual encoded size also accounts for escaping. Terminal results are immutable through `advance`; duplicate progress observes the retained result. Progress cannot predate the receipt’s latest accepted time or regress InProgress to Accepted.

## Existing undertaking adapters

Provider go_to receipts are Accepted until movement starts, then InProgress, with explicit arrival Completed and expiry/stop/pressure Interrupted. Custody, confinement, departure, road recall and replacement reconcile to truthful interrupted/superseded reasons rather than inferring arrival from a missing intent. Root protection lasts while any bound travel/edit step remains.

`set_round` is Accepted while `RoundEdit` is pending. Its Round consumer records Completed only after place, enrollment, leg and trade checks; failed consumption records a rejection. Ward teaching happens only with actual accepted schedule mutation. Replaced or obsolete edits cannot strand protected roots.

NPC and Night requests retain a semantic obligation plus captured presence incarnation independently from RequestId. An entire successful reply is replay guarded, including parse/action refusal lines, history absorption, transcript, priority and Night mood bookkeeping. Failed external attempts consume their execution once, restore/requeue/back off as before, and may retry the same unfinished obligation. Exact capacity-held completions remain intact. Night busy retries preserve incarnation, and every terminal drop releases allocated work.

A voice recording's receipt is protected through its STT/speech result. Immediate held transcripts wait until the outer receipt is committed, then speak in the same command/poll and update Accepted to Completed or Rejected. Final speech checks terminal receipt state before clearing floor or mutating conversation/history. Backend callbacks/chunks themselves remain transport lifecycle inputs.

## Verification

[The combined per-target record](verification.json) reports **1,810 passed, zero failed, eight ignored across 33 targets**. [Final source hashes](source_hashes.json) cover 29 changed/new Rust and dependency files. This is combined verification, not a claim that a single final full-workspace invocation passed:

- [First full workspace run](workspace-first.log) completed all targets. Its 12 failures were the provider's previously valid exact-100,000-scalar reply control and 11 host tests expecting raw command bodies.
- [Full sim and host rerun](sim-host-final.log) passed all selected targets, including 528 host tests (three ignored), all sim targets and 20 supply-chain host tests. Fixtures assert exact HOST step-zero sequences before preserving their original body assertions.
- [Final semantic-root rerun](committed-root-final.log), after the last production repair, passed 589 sim library tests, 69 Engine tests, 27 scheduler integration tests and 47 speech tests. It covers valid reply replay, altered comments, failed executions, scalar-overlimit and byte-overlimit duplicates without repeating world/history/knowledge/transcript/queue/retry/backoff/Night duty effects. That last repair changes only Scheduler/Night committed-root handling and its shared reply-size predicate; host command/projection code was unchanged after the full host pass.
- [Allocation/library witness](allocation-and-library.log): the 4,096-entry ledger with all lifecycle-update IDs queued allocates 4,024,136 heap bytes plus 896 inline. Two complete replacement cycles with 32 interleaved producers peak at 3,823,760 heap bytes. Final recent encoded metadata is 3,134,449 bytes. The later provider guards add no ledger fields.
- [Focused custody/Night run](focused-custody-night.log) includes the full-world no-route refusal witness and busy Night incarnation cleanup. [Plan validation](plan-validation.json) checks roadmap structure and links only; it is not gameplay acceptance.

The ledger regressions also cover pending outer tickets under a full window, holes below compacted floors, protected future steps, protected saturation followed by capacity recovery/redelivery, canonical ordering, oversized/deep JSON, worst-case escaped references/max counters and immutable monotonic progress. The existing eight ignored tests remain explicit limitations. No visual/focus/reference-renderer performance result is claimed.

Coordinator review checked the admission, whole-reply and pending-work paths, the custody refusal witness, host identity assertions and measured ledger allocation. An independent check matched all 29 final source hashes, compared recorded test counts with the raw logs and recomputed the 33-target aggregate. Archived logs omit redundant terminal blank lines; their test output is unchanged. This accepts M1b's implementation scope; it does not accept the unfinished M1c/M1d contracts or close external performance evidence.

Exact commands (all from the repository root):

```text
CARGO_HOME=/tmp/alibi-m1b-cargo CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 /home/ran/.cargo/bin/cargo test --workspace --no-fail-fast
CARGO_HOME=/tmp/alibi-m1b-cargo CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 /home/ran/.cargo/bin/cargo test -p cathedral-sim -p cathedralbevy --no-fail-fast
CARGO_HOME=/tmp/alibi-m1b-cargo CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 /home/ran/.cargo/bin/cargo test -p cathedral-sim --lib --test scheduler_tests --test engine_tests --test speech_router_tests --no-fail-fast
CARGO_HOME=/tmp/alibi-m1b-cargo /home/ran/.cargo/bin/cargo test -p cathedral-sim --lib --test receipt_allocation_tests --no-fail-fast -- --nocapture
```

Raw temporary paths were `/tmp/alibi-m1b-workspace-first.log`, `/tmp/alibi-m1b-sim-host-final.log`, `/tmp/alibi-m1b-committed-root-final.log`, and `/tmp/alibi-m1b-lib-allocation3.log`, respectively. The isolated Cargo home links existing registry cache/source entries read-only and stores six newly needed hash crates in /tmp; it avoids writes to the shared registry. Python uses `UV_CACHE_DIR=/tmp/alibi-uv uv run --no-project`. No renderer evidence is claimed. No unrelated files or initial untracked directories were removed.

The [selected owner field capture](owner_fields.json) records current declarations; the persistence delta covers additional receipt/intent/host owners outside that index. M2 save/restore remains unimplemented. See [the M1b persistence delta](../../PERSISTENCE_INVENTORY.md#m1b-receipts-and-semantic-work-owner-delta--2026-09-07). M1c owns world/runtime generations, including raw microphone and provider callback channels; M1d owns the generic exclusive operation/duty kernel.
