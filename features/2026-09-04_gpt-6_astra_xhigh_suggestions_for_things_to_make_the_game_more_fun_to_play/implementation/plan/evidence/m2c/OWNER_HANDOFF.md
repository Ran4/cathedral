Status: implementation and frozen-source owner verification complete; Cargo ceded to coordinator for release acceptance (2026-09-15).

# M2c owner handoff

The M2c implementation is complete on shared `develop`, after accepted M2b
`019f451c12bf7c94c9823678403f50244c88d421`. The owner has made no commit or push.
Every owner Cargo process has exited. Explicit Cargo cession was sent to the
coordinator after `workspace-01` exited successfully and its source map was
copied to [source_hashes.json](source_hashes.json). Subsequent owner writes are
feature-excluded evidence and detailed milestone/protocol prose only.

## Implemented boundary

`Admitted<HydratedEngine>::prepare_continuation` consumes the actual quarantined
owners and returns admitted `PreparedContinuation`. Exact unfinished scheduler
inputs become bounded, separate load retries, including empty/offstage idle
work. Newer protected work can coexist with an older retry; each owns its original
seated Knowledge and offered Occasion context. Busy, later provider failure,
held success/error/oversized application and repeated capture are distinct
states. At the closed 64-obligation bound, new prompt creation backpressures
without dropping protected intent or accepting an unrepresentable 65th flight.

Night keeps exact request method/options, saved day, semantic root and actor
lifetime. Inactive saved flights participate in lazy gate computation but cannot
harvest reused numeric execution IDs. Ordinary new person queues now retain
queue-time incarnation authority; historical V1 migration anchors only to its
available saved authority. Queue-time stamps, settled/dropped duties and ambient
daily guards remain distinct. Missing-person migration preflights its exact
dropped-counter delta before mutation.

Uncommitted speech becomes durable unsent interrupted-input history. Accepted
recordings retain original provenance and exact terminal interruption receipts,
including owed notifications for M3 publication. Validation checks retained or
evicted ledger authority, accepted producer watermarks, receipt ordinal/affected
identity, timestamps and uniqueness across histories and command families.
Committed effects do not replay. Floor preserves existing foreground/background
reading deadlines, clears microphone liveness and reconciles old voiced waits
against surviving readable progress with the original failsafe.

Strict required-field component V2 wrappers represent deferred/resumed scheduler
contexts, Night queue incarnations/pending replacement and speech history. The
complete envelope remains V1 and supports every prepared state immediately.
Insufficient legacy V1 component exports explicitly refuse new state; historical
fixtures and readers remain intact. Full-envelope validation checks context
allocation against Knowledge and exact protected semantic roots.

Prepared capture and category observations use the same admission budget.
Service binding reserves its own subordinate lease before constructing inert,
generation-scoped handles. It invokes no service trait. Destruction, refusal,
mismatch and unwinding keep charges until the corresponding objects are gone.
The public wrapper exposes no Engine/World borrow, mutation, poll or extraction;
M3 still owns application adoption. The simulation behavior seam is cfg(test)
only and uses the common private preparation path.

## Frozen verification

[verification.json](verification.json) is produced by the evidence-only
[owner/seal.py](owner/seal.py) audit. It checks every original command record,
helper/source identity, raw log hash, exact mtime-zero gzip contents, current
source equality and final debug binary identity.

| Evidence | Result |
| --- | --- |
| `workspace-01` | 2,215 passed, 0 failed, 44 intentional ignores; 46 groups |
| Final sim library | 824 passed, 0 failed, 28 ignored |
| Final host binary | 602 passed, 0 failed, 11 ignored |
| `format-check-02` | All 39 changed/new Rust files pass at the final map |
| `legacy-integration-03` | Both component boundary suites pass, 12 cases total |
| Independent M2c cases | All 7 public host and 6 simulation behavior cases pass in the full workspace |

The final 980-input map SHA is
`b277379757dd70a1476adb44e583aeb808533ef58178fa88f70f5e20600d35a1`.
The workspace command is `cargo test --workspace --offline -j1 -- --test-threads=1`;
its [start record](owner/workspace-01-start.json) records exact absolute argv and
the hidden/fake, offline build environment, including removed inherited flags.
The original raw log is `/tmp/alibi-m2c-workspace-01.log`, SHA
`dc88e0c8d1498345a5b0a54313fe1a1468b9ec36bf479c0769ef279641502216`.
Its [gzip archive](owner/workspace-01.log.gz) SHA is
`e3fa83441b2e6bf2f8177f47c5c66048c64e3a3cb04e8b5a755487a68eb443be`.

Final debug executables:

| Executable | Bytes | SHA-256 |
| --- | ---: | --- |
| `target/debug/deps/cathedralbevy-eafe86af140beaa1` | 2,298,531,808 | `b5ed7a238bcf1bff423cbacfc69fd82925d46029cad0d5017315257e0adf3c61` |
| `target/debug/deps/cathedral_sim-29e329e3d5b1c8e4` | 323,261,552 | `d077b552e99be196653b57984d79fce41969257eedd1b1d79ff7e838120583f2` |

The full-envelope regression exercises old deferred and newer active same-actor
prompts through admitted full capture/validate/hydrate/prepare/capture, retaining
both distinct seated facts. A second load produces exact complete bytes; corrupt
allocation and missing protected-root mutations refuse at their actual owner
boundary. Its synthetic Host fixture and diagnostic JSON copies are behavior
and joint-validation evidence, not an allocation measurement. The evicted
interruption regression first produces two valid ordinarily evicted identities,
then rejects ordinal collisions within/across groups and with retained commands
from another family. Both regressions pass in the final full workspace.

All 23 owner attempts remain archived, including eight failed development
attempts. Earlier mixed-source or mutating attempts retain their original
`sources_unchanged=false`; they are not presented as frozen acceptance. The
[README](README.md) describes the mechanical compile corrections, independent
fixture preconditions, initial synthetic Host omissions, probe startup pacing
and legacy fixture corrections. No failing assertion was accepted as a measured
continuation outcome. [Coverage](coverage.md) documents where historical V1
integration checks remain and where new V2 lifetime checks moved.

## Actual-city debug probes

The ignored probe is
`host_checkpoint::tests_continuation_owner::m2c_continuation_probe`.
It accepts `ALIBI_CONTINUATION_MODE=authored|populated`,
`ALIBI_CONTINUATION_SAMPLES=1..1000` and `ALIBI_CONTINUATION_REPORT`.
Optional `ALIBI_CONTINUATION_FIXTURE` writes a new `/tmp` file;
`ALIBI_CONTINUATION_READ_FIXTURE` selects a fresh reader of that file. The report
includes original input hashes, emitted fixture and re-save hashes, image
identity, actual pending shapes, admission counts, exact category equality and
separate preparation/service-binding/prepared-owner-disposal timings. Each owner
invocation records probe variables directly in its argv. Coordinator runners
own release distributions and fresh-process writer/reader evidence.

| One debug sample | Authored | Populated |
| --- | ---: | ---: |
| Actors | 520 | 2,520 (2,000 requested and placed) |
| Cumulative prepared typed bytes | 38,447,138 | 133,751,958 |
| Shared peak bytes, including Running | 758,093,679 | 977,116,076 |
| Preparation / binding / disposal, µs | 13.217 / 2.963 / 1,612.364 | 13.671 / 3.890 / 4,299.372 |
| Immediate prepared full bytes | 3,290,715 | 12,761,634 |

Both actual source shapes have one unfinished scheduler request and one accepted
recording, no held result and no active Night request. Each preparation retains
one exact retry, one interrupted input and one terminal receipt. All eight
unaffected categories match and the same-process second full preparation/re-save
has exact bytes. The [authored report](owner/probe-authored-02-report.json) SHA is
`80d770dff2470acc6d8123cdc4667b0c1770f4a2f8f4e0a413a9b7366cdcc73a`;
the [populated report](owner/probe-populated-01-report.json) SHA is
`1e52d21667cfe3976c797de167fcc0b23de6ef252d0e8e391c1863794e505f04`.
Both name the final debug host image. Their source maps differ from the final
map only in the two later labeled legacy integration fixture corrections.

The populated preparation leaves 465,770 B under the unchanged 128 MiB typed
cap. The source-derived structural preparation allowance is 131,072 B; the
closed layout/receipt working-storage calculation is 112,128 B. The
[admission proof](ADMISSION.md) covers queue/vector growth, serialization,
receipt advance/drain, preflight and lease/drop order. These debug samples prove
reachability and admission, not release timing distributions or host-frame
acceptance. Input capture/hydration/file IO, original host retirement, device
construction, final shared navigation destruction and M3 adoption are excluded.

## Remaining milestone work

The coordinator owns release measurements, fresh same-image fixture/read audits,
final review, publication evidence and the M2c commit. M2d adds the broader
deterministic future-continuation matrix under fake/recorded timing; unfinished
work retries its exact accepted input but an uncontrolled live provider may
produce a different future. M3 owns host-time binding, inactive ECS/controller
staging, owed receipt publication, runtime jobs, supersession, authoritative
swap and retirement. No production Engine extraction, unsafe Send, device or
renderer probe, or 20,000-resident stress run was introduced in this leg.

Accepted M2a/M2b evidence and historical fixture bytes remain untouched. The
unrelated `docs/codex_gdd/`, `gauntlet/`, `reference/` and ambient feature work
remain outside this owner scope.
