# M3b2b1 owner handoff

Status: implemented and verified; explicit source/Cargo/executable cession is
recorded in the accompanying final handoff message. The coordinator still owns
independent acceptance, staging and commit.

The agreed coherent cut is production prompt archive admission/retention.
M3b2b overall and complete App adoption remain pending. The coordinator owns
independent review/tests, acceptance documents, staging and commit. No branch,
worktree, push or commit was created by this implementation owner.

## Source and preservation

Accepted base: `30f8188301b8f1585df478e2e6df5f16bc1434cc` on shared `develop`.
Final component map:
`8f246541e779e999382526d0a730eb95accb60965b1ff37df4b175f54c2ba0ca`,
1,005 inputs. The coordinator's delta audit records 29 changed predecessor
inputs, two new Rust files, 974 unchanged inputs and no removals. The two new
files are the sim opaque archive owner and the coordinator-owned backend
`prompt_archive_review_tests.rs`. Final source starts are recorded separately
from development maps.

Unrelated untracked `docs/codex_gdd/`, `gauntlet/`, `reference/` and
`features/2026_09_14_more_ambient_stuff.md` were preserved. No retained image,
fixture, cache or binary was deleted to free space. `/home/ran/w` was not read.
No live provider/device, visible-window or GPU probe was used.

## Implemented boundary

See [implementation.md](implementation.md) for actual owners and the scoped
allocation formula. Both scheduler lanes reserve before provider acceptance;
provider or archive refusal keeps their existing semantic retry/input policy.
Held completions retain the original permit. Actual immutable event Arcs carry
strings and admission together; clones share them. Saved checkpoint authority
and equality remain semantic. Complete held-result service binding reserves
both lanes transactionally before installing either, without using saved
RequestIds as archive keys.

A finite writer governs eight outstanding records and 16 MiB accepted payload,
including queued, active, held and externally cloned exchanges. It streams the
existing JSON and Markdown formats, preserves per-session ordering through
forks, and waits for an accepted prefix during off-frame flush/normal teardown.
Queue pressure does not drop accepted archives. Existing IO-error and process-
exit five-second best-effort behavior remains; no new durability guarantee is
claimed for disk errors or hard aborts.

Standalone owned refusal returns the same input allocations before filename
assignment. Shared ingress requires original admission and returns the same
Arc on unsupported/foreign/duplicate/size refusal. The legacy borrowed-event
convenience reserves before copying into its own separate admitted record.
Disabled default logs create no global worker.

The writer's separate off-frame join owner retains Core through actual native
thread termination; receipts retain Core without retaining sender/join owners.
`start_admitted` keeps its actual 20 MiB shared Running reservation until both
thread termination and every surviving permit. It is a tested integration seam,
not production complete-App startup admission.

## Evidence and development history

Every recorded command uses the exact environment in its start JSON, offline
Cargo with one job, a component map, retained `/tmp/alibi-m3b2b1-*.log` raw bytes,
lossless deterministic gzip archive and result/source comparison. The runner
and enumerator hashes are recorded at command start.

`development-check-01` found the stale `_world` parameter name.
`development-backend-tests-01` found constructor privacy, the old model test
field and borrow-guard return errors; `development-backend-tests-02` found the
optional writer borrow mismatch after making disabled startup lazy. Those two
backend development commands overlapped continuing edits and correctly record
`sources_unchanged: false`. `development-sim-check-01` found one remaining old
event pattern. All four are preserved compiler feedback, not passing tests.
`development-workspace-check-01` subsequently checked all workspace test targets.

`focused-sim-01` passed all six new sim tests. `focused-backend-01` passed all
six coordinator tests and the existing archive format/model tests (20 total).
`focused-continuation-01` passed 37 tests with one existing ignored test, including
the six owner tests with explicit provider-attempt refusal assertions. These
are pre-final evidence: the final source adds exact-capacity construction for
Night's synthetic ward ActorId. The unchanged assertions run again on the final
workspace source; no assertion was weakened to accept the final refinement.
`final-format-02` checks all 31 touched/new Rust files on the final map.

The normal application target reports a newly unused `PromptExchange` import,
now used only by retirement tests. The coordinator explicitly retained the
freeze and deferred moving this import into that test module until the next
substantive host edit. This is distinct from existing performance/future-seam
warnings. There are no ignored archive Result-return warnings in the focused
backend run; the existing tests now unwrap acceptance so refusal cannot silently
weaken their assertions.

## Completed final verification

| Record | Result | Cargo compile | Runner wall |
| --- | --- | --- | --- |
| `focused-sim-01` | 6 passed / 0 failed / 0 ignored; 1 printed groups | 4m 07s | 247.759818s |
| `focused-backend-01` | 20 passed / 0 failed / 0 ignored; 1 printed groups | 1m 40s | 100.124275s |
| `focused-continuation-01` | 37 passed / 0 failed / 1 ignored; 1 printed groups | 3m 12s | 199.653744s |
| `final-format-02` | exit 0 | n/a | 0.163762s |
| `final-workspace` | 2292 passed / 0 failed / 47 ignored; 46 printed groups | 22m 53s | 1481.282558s |
| `final-layout` | exit 0 | n/a | 5.319631s |

The exact final command was `/home/ran/.cargo/bin/cargo test --workspace
--offline -j1`, started at 2026-09-15 09:00:55 UTC. `sources_unchanged` is true.
Raw-log SHA-256:
`3079fe2c46bc8c3e52564c82a43da7fc6dac17264626d47b3a4a95a3e61c376c`.
Archive SHA-256:
`1a916af612e7a494730d85a5d4b19d2b876b239faea177c143defd59ae5f48ea`.

The 46 printed outer results total 2,292 passed, zero failed and 47 ignored.
The actual saved-file/fresh-process sixteen-owner preparation witness also has
an explicit passing outer row. Passing child stdout is suppressed by this
Cargo invocation; no unprinted child result was added to these totals. Existing
ignored tests remain ignored. All 12 new archive witnesses have explicit final
passing rows:

1. `prompt_archive_review_tests::archive_review_actual_admitted_worker_and_permit_keep_the_persistent_charge`
2. `prompt_archive_review_tests::archive_review_shared_actor_id_spare_capacity_cannot_bypass_admission`
3. `prompt_archive_review_tests::archive_review_byte_refusal_returns_the_same_input_without_spending_a_filename`
4. `prompt_archive_review_tests::archive_review_event_clones_keep_capacity_after_the_writer_finishes`
5. `prompt_archive_review_tests::archive_review_forks_preserve_order_and_foreign_sessions_return_the_same_arc`
6. `prompt_archive_review_tests::archive_review_unadmitted_shared_input_refuses_and_convenience_owns_a_separate_copy`
7. `engine::continuation::complete_tests::archive_complete_held_service_binding_reserves_both_or_preserves_the_candidate`
8. `night::continuation::tests::archive_night_held_result_owns_its_permit_until_the_actual_exchange_dies`
9. `night::continuation::tests::archive_night_refusal_preserves_queued_and_resumed_semantics`
10. `scheduler::continuation::tests::archive_scheduler_backpressure_retains_held_and_same_poll_exchange_until_last_clone`
11. `scheduler::continuation::tests::archive_scheduler_refused_resumed_request_preserves_exact_obligation_and_inputs`
12. `scheduler::continuation::tests::archive_scheduler_stale_actor_and_failed_results_keep_their_admitted_archive`

## Final layout evidence

[final-images.json](final-images.json) records both final workspace ELF paths,
in-place SHA-256 values, all 28 exact read-only GDB queries and measured layouts.
Neither target was executed or copied for this inspection. Source and ELF
identities stayed unchanged during the inspection.

The preparation fixed-control upper bound is **60,952 bytes**
against the unchanged 65,536-byte allowance. Its largest-root calculation is
`4 * max(PrepState=11552, PreparedDelivery=11648,
DeliveryDisposal=11432, RetiredPayload=24)` = 46,592 bytes.
The retained formula also counts Core, permit/job, usage, retirement/promotion
metadata and 1,024 bytes for fixed allocation rounding. This remeasures the
actual enlarged preparation owners after scheduler/Night permit storage.

Archive fixed roots include Core=80, Receipt=32, Session=56, Progress=64,
WriteJob=72, WriterOwner=32, ArchiveWriter=32 and PromptLog=56 bytes; the immutable
exchange body is 144 bytes and its permit is 16 bytes. These measured fixed
layouts support the scoped design; they do not measure variable heap/native
allocation or replace the explicitly trusted native allowance.

Final whitespace verification is recorded by `final-diff-check-*`.


## Mandatory next work

Read the accepted evidence plus `/tmp/alibi-m3b2b-accounting-review.md`,
`/tmp/alibi-m3b2b2-handoff-draft.md`, `/tmp/alibi-m3b-host-review.md` and
`/tmp/alibi-m3b2-handoff.md` before the next fresh sequential cut. The archive
startup budget/session recipe, the actual diagnostic JSONL/stderr owners,
backend generation/native runtime/recording-discard/child-reaper closure and
the disjoint complete-App numerical capacity proof remain pending. Native
closure includes transitive DNS/spawn-blocking tasks and native threads beyond
the two async workers. Do not infer whole-process or external model/GPU bounds
from this accepted-job governor or a DTO inventory.

Preserve the 512 MiB aggregate Running, 1 GiB shared and 128 MiB typed limits.
Prove save-again while delayed retired actual owners survive, or refuse before
mutation while retaining every original owner. Whole-App staging/adoption is
M3b2c; RequestId rebasing/fences and retaining the existing archive session via
fork belong to that activation path. M3c controls and M3d whole-path frame
acceptance still follow.
