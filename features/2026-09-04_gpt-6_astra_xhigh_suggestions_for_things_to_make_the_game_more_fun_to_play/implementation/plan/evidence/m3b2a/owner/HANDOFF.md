# M3b2a owner handoff

The stable admission-group promotion prerequisite is implemented and verified.
This is the coherent M3b2a cut agreed with the coordinator; complete whole-App
adoption remains outstanding. The coordinator owns acceptance documents,
independent audits, staging and commit. No branch, worktree, push or commit was
created by this implementation owner.

## Source identity and ownership

- Accepted base: `72bc316e592a260344033689ae33b1ca7aa4c361` on shared `develop`.
- Final component map: `04046178497e975d6942f76232ef3696fd5a4ad7674494ffc817e79d5b978032`,
  1,003 inputs, recorded in `final-workspace-sources.json`.
- Changed Rust files: `crates/cathedral-sim/src/checkpoint/budget.rs` and
  `crates/cathedral-sim/src/checkpoint/mod.rs`.
- New coordinator-owned Rust file:
  `crates/cathedral-sim/src/checkpoint/promotion_review_tests.rs`.
- The coordinator's independent source-delta audit reports two changed inputs,
  one added input and 1,000 unchanged inputs against accepted M3b1.
- No Rust source changed during the final workspace, layout or format checks.
  Unrelated untracked directories/files and retained images, fixtures and
  caches were preserved. `/home/ran/w` was not read.

## Implemented contract

See [implementation.md](implementation.md) for the source-backed design.
Reservations retain stable allocation-group identities. Cohort resolution,
resize, release and promotion share the usage mutex, so surviving children
follow their original world without debiting a later Running generation.
Persistent service reservations use a separate Running group.

`CheckpointBudget::prepare_promotion` borrows the live root option and the
candidate. Ordinary ticket Drop preserves the live root and all charges.
`Admitted<T>::prepare_promotion` supports opaque candidate ownership.
`PromotionPermit::commit` infallibly migrates old and candidate groups and
attaches the original root to the already reserved retirement owner.
Retirement observers wait for every migrated or transport child after all
strong lease owners disappear, without pinning those charges themselves.
Previously used retirement groups cannot be reused to merge another world.

The 1 GiB aggregate ceiling and 512 MiB aggregate Running requirement are
unchanged. This primitive proves ownership and role transfer; it does not
prove a complete application's actual retained allocation bounds.

## Actual verification

Every command uses the exact sanitized environment in its `*-start.json`.
Each completed command has its source map, result JSON, retained raw log under
`/tmp/alibi-m3b2a-*`, and deterministic lossless `*.log.gz`. The runner and
component enumerator hashes are recorded at command start.

| Record | Result | Cargo compile | Runner wall |
| --- | --- | --- | --- |
| `focused-owner-01` | 3 passed, 0 failed, 0 ignored, 881 filtered | 3m 02s | 182.034182s |
| `focused-checkpoint-01` | 210 passed, 0 failed, 27 ignored, 647 filtered | 0.08s | 0.813432s |
| `final-format` | Scoped rustfmt check, exit 0 | n/a | 0.018542s |
| `final-workspace` | 2,280 passed, 0 failed, 47 ignored; 46 printed groups | 22m 46s | 1497.473528s |
| `final-layout` | Read-only final ELF inspection, exit 0 | n/a | 5.846651s |

The final workspace command was exactly
`/home/ran/.cargo/bin/cargo test --workspace --offline -j1`, started at
2026-09-15 07:36:38 UTC. Its `sources_unchanged` result is true. Raw-log SHA-256:
`6d9128f662662d617c237ec456bf98b8cc24cf991ff69cee20f463dfc6348010`.
Archive SHA-256:
`c35df04bdc82c7b5ae9f4b3c453a918fe1c23f5ec8a710dce8b0f2ea592258f6`.

All eight new tests have explicit passing rows in that final log:

1. `checkpoint::budget::tests::opaque_admitted_owner_promotes_and_keeps_actual_payload_charged_through_drop`
2. `checkpoint::budget::tests::previously_used_retirement_group_cannot_be_rewrapped_for_another_world`
3. `checkpoint::budget::tests::promotion_retirement_observation_tracks_children_without_pinning_them`
4. `checkpoint::promotion_review_tests::promotion_review_children_follow_their_world_and_release_observers_wait_for_them`
5. `checkpoint::promotion_review_tests::promotion_review_dropped_ticket_keeps_live_root_and_all_original_roles`
6. `checkpoint::promotion_review_tests::promotion_review_foreign_and_persistent_roots_refuse_without_mutation`
7. `checkpoint::promotion_review_tests::promotion_review_opaque_admitted_payload_keeps_its_charge_through_disposal`
8. `checkpoint::promotion_review_tests::promotion_review_worker_resize_and_release_never_debit_the_new_running_group`

The actual saved-file fresh-process witness also has an explicit passing row:
`checkpoint_preparation::tests::actual_saved_file_prepares_all_sixteen_owners_in_a_fresh_process`.
This workspace invocation suppresses passing child stdout. The 46 groups and
2,280 passes count only printed outer results; no unprinted child result is
added. Existing ignored tests remain ignored. Existing unused future-seam
warnings remain; no failure was observed.

`format-01` and `format-02` are development formatting records and correctly
report source changes. `focused-promotion-01` is retained development feedback:
it passed 5 tests but overlapped the coordinator's late fifth test addition
and reports `sources_unchanged: false`. It is not final acceptance evidence.
The corrected focused and final runs above contain the frozen full test file.

## Final layout evidence

[final-images.json](final-images.json) records both final workspace ELF paths,
their in-place SHA-256 values, all exact GDB commands and measured Rust layouts.
The targets were not executed or copied for this inspection.

The fixed-control upper bound is 60,712 bytes against the unchanged 65,536-byte
allowance. The retained largest-root calculation is
`4 * max(PrepState=11504, PreparedDelivery=11600, DeliveryDisposal=11384, RetiredPayload=24)`
= 46,400 bytes. It also counts the Core, permit/job state, new usage and
retirement/promotion metadata and 1,024 bytes for fixed allocation rounding.
The coordinator independently checked the ELF hashes, queries and arithmetic.

Final whitespace verification is recorded by the subsequent
`final-diff-check-start.json`, `final-diff-check-result.json` and
`final-diff-check.log.gz` records.

## Mandatory next work

Read `/tmp/alibi-m3b2b-accounting-review.md`, `/tmp/alibi-m3b-host-review.md`
and `/tmp/alibi-m3b2-handoff.md` before designing the next cut. The next fresh
sequential owner must close actual disjoint live/candidate/retired/persistent
allocation accounting, bounded global PromptLog retention and scratch, and
the lifetime of detached backend work. Account for the actual scoped owners;
do not infer whole-process or external-model heap bounds from DTO sizes.

Numerically justify the old world's retained bound and the replacement's
Running allowance. Preserve the aggregate 512 MiB Running contract and prove
save-again during delayed retirement, or refuse before migration/fence while
keeping every original owner. Renaming cohorts alone cannot make two unchanged
512 MiB world roots plus persistent services fit the 1 GiB ceiling.

Whole-App adoption then still requires immutable startup inputs, saved host,
time/body and presentation restoration, staging isolated from every actual
reader, cancellation cleanup, persistent projection-cache correctness, and
an infallible final whole-App barrier. M3c controls and M3d whole-path frame
acceptance follow. No part of this cut claims those later requirements passed.
