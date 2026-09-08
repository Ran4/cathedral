# M2a1 private checkpoint components — 2026-09-08

Status: Accepted component cut (2026-09-08). Implementation, workspace verification, coordinator review and component measurements are complete. Complete M2a owner/envelope coverage and M2b–M3 integration remain pending.

This is a coherent first M2a cut after accepted M1d commit
`02fb30980b78cf15a96763a410b0f62275cfb0a3`. It implements real private replay and
operation payloads, exact clock/manifest/time components and bounded admission.
There is deliberately no complete-save type, capture boundary, Engine hydration,
external retry, host adoption or slot writer. [Owner coverage](OWNER_COVERAGE.md)
keeps every remaining M2a owner and complete-envelope gate explicit.

## Implemented interfaces

`CommandLedger::checkpoint_v1` and `OperationKernel::checkpoint_v1` export strict
v1 array-record DTOs. The owner `decode_json`/`encode_json` methods validate and
consume an admission reservation; `Admitted<T>` retains that charge with the
value and transfers it through encoding. No clone or unwrapped extraction API
releases the charge while leaving the admitted value alive. Send payloads carry
the charge to workers. External code remains responsible for using one actual
coordinator and charging all running/retiring owners; M3 integration is pending.

Ledger validation preserves issued/high-water independence, receipt lookup
before compacted-floor refusal, protected roots without receipts, and valid
recent/protected rows below the floor. Duplicate records/ordinals/roots,
unsupported versions, oversized text/counts/entries, invalid times/counters and
incomplete dispatch/notification flushes reject. Historical/attempted receipt
principals can refer to a consumed item or rejected absent target. Current active
bindings still require exact actor/incarnation/resource/receipt/root agreement.
The future complete envelope must call `validate_owner_roots` with all real
semantic owners; local validity does not imply that complete agreement.

The kernel reconstructs actor/resource claim indexes from authoritative records,
compares fixture declarations and adapter versions, and preserves all counters,
work/retry/recovery state and exclusive deadlines. Shared `validate_continuation`
now rejects materially impossible credited work. Its arithmetic tolerance is
1 ns per elapsed work second plus 64 epsilon units of the absolute logical
anchor; at the v1 limits this is below 101 microseconds. Zero-credit operations
retain their initial progress anchor. Load cannot grant a material elapsed span.

`WorldClockDtoV1` saves the exact original rate segment, with bit-equal explicit
calendar-position validation. `HostTimeV1` preserves nanosecond elapsed/debt/wall,
host fixed residual and separate simulation residual. Only declared negative
infinity sentinels become `Never`; negative finite calendar history is valid.
`serde_json/float_roundtrip` preserves accepted/calendar/deadline bits. V1 supports
logical seconds 0..1e9, calendar days ±1e6, seconds/day 1..1e9 and scale
1e-6..1e6. These are numeric format limits, **not a CPU-safe effective calendar
rate**: complete-envelope cadence/rate and continuation CPU validation remain
pending, especially at extreme legacy day-crossing rates.

The compatibility component requires exact content, geometry, behavior,
generator and hash implementation identities, including toolchain/target/build
when std hashing depends on them. This cut supplies comparison primitives; the
real canonical manifest builder/resolver and complete envelope remain M2a2+.
The five supported v1 component fixtures are not M3-loadable city saves.

## Resource bounds and measurement scope

One running, one save, one load and one retiring cohort share a 1 GiB byte
ceiling. The 64/128 MiB complete-payload ceilings remain format integration
inputs. Ledger components cap encoded bytes at 5 MiB, each entry at 1 KiB, recent
records at 4096, retained/protected at 256 and producers at 32. Reconstructed
recent-tree heap is conservatively checked against 4 MiB; decode/extraction/index
working allowance is 16 MiB. Kernel components cap encoded bytes at 1 MiB and
working allowance at 8 MiB, retaining the existing 2 MiB kernel heap ceiling.

Decode charges its working allowance plus 3× input bytes, covering input and
up to 2× serde escaped-string scratch before bounded visitors run. Strings and
arrays validate before retained growth; arrays grow geometrically capped at
count limits. Encoding counts first and allocates exactly that output capacity.
Actual encoded bytes, conservative heap and peak reservation are different
figures; none measures full Engine/allocator RSS or host frame work.

`cathedral-backends/examples/alibi_checkpoint_cost.rs` supports
`--mode authored|maximum --samples 1..1000 --output <json>`. It uses the real
nav-less authored Engine and unavailable services. Maximum mode admits 256 real
fixture operations and 4096 recent/256 retained protected ledger rows. JSON
records separate raw export/encode/decode-validation/index-validation/drop
microseconds, counts, bytes, conservative heap and reservation peak. Included
validation/counting/destruction work is labeled in `phase_notes`. The ledger
index phase really constructs/disposes its recent/retained maps; kernel
validation constructs/disposes claims. This cannot accept complete save CPU,
real-host frame latency, renderer or full-population gates.

## Verification and reviewed handoff

The final full workspace run exited zero: **1881 passed, zero failed, 9 ignored across 33 targets**. The focused checkpoint suite passes 16 tests and ignores one explicit component-fixture generator. All preexisting golden fixtures remain unchanged.

Focused checkpoint tests cover real owner export/continuation,
saturated replay holes/floors/reserved roots, malformed records/numerics,
multibyte/escaped-string limits, attached charge disposal, exact float/calendar
bits, 400 ms debt + 15 ms fixed residual, operation obstruction/progress and
supported component fixture wire. Raw [verification commands/exits](verification.json), [full workspace log](workspace-final.log), [focused log](focused-final.log), [source/fixture hashes](source_hashes.json) and [original/archive log hashes](log_archives.json) preserve the final tested source. The only earlier environment preflight failure stopped before compiling because the sanitized PATH omitted rustc; explicit RUSTC/RUSTDOC corrected it, and its raw log is retained. Historical M0/M1
archives, original baseline harness, unrelated untracked directories and all
existing golden fixtures remain unchanged.

Both authored and maximum diagnostic probe smokes exited zero. The coordinator then ran six sequential release probes with 6,000 raw phase samples; [the measured record](performance/README.md) preserves all runs, percentiles and byte charges. Maximum mode has 520 authored actor records, 256 active fixture owners, 4096 recent entries and 256 retained/protected roots. Ledger JSON is 3,111,542 bytes with a 4,121,504-byte conservative recent-tree bound; kernel JSON is 169,034 bytes with a 1,410,048-byte conservative heap bound. Counts and byte charges repeat across runs. Saturated ledger export/encode p99 costs of 7.186/11.884 ms require moved or split work in the future host integration; this cut does not meet the host coordinator frame gate.

The [release example build](release-build.json) exited zero after the workspace run; its [raw build log](release-build.log) and binary hash are archived. The [coordinator review](coordinator-review.json) independently verifies the 23-file source manifest, final test totals/logs, archive normalization, all raw performance percentiles and repeated metadata, and scoped formatting of 14 Rust files. The source and measured executable remain unchanged after verification. M2a2 follows as the next coherent owner cut.
