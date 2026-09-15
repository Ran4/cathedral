# M3b1 owner final handoff

M3b1 implementation and verification are complete for root acceptance. The cut
provides real M3a-file worker preparation and actual LocalEngine retirement
transport. Complete App adoption is the explicitly assigned fresh-owner M3b2
cut. No checkpoint wire/schema version, raw/typed cap, cohort limit or 1 GiB
aggregate limit changed. No Engine Send or unsafe Send implementation was added.

## Frozen verification

Final source map:
`ea4086cf10fdf5e6d0781f3191e2f73cc9dcd7460889a4d98646a1f993783d80`.
The component-inputs-v2 map contains1002 inputs:9 additions,16 changed M3a
inputs,977 unchanged. `final-workspace-03` records the exact concurrent command
`cargo test --workspace --offline -j1 -- --nocapture` with explicit Rust tools,
offline Cargo home and headless/fake environment. It exited0 on unchanged
source in771.977 seconds:2272 outer tests passed, zero failed,47 intentional
ignores across46 outer groups. The separately printed fresh-process child adds
one passing test and one summary group; it is not counted twice as an outer test.
Exact totals and all group rows are in final-results.json.

`final-format-03` passed all25 changed Rust files; `final-diff-check-03` passed.
`final-control-layout` passed read-only GDB queries with no target execution and
archived its output. final-images.json records the final backend test ELF's
381031000 bytes and SHA256
`c6a73440866614dbd11a5d5bede6b85f2d094627b6b71b55b5aa02ab9de7b311`.
The ELF was hashed before and after the query; no separate large binary copy is
claimed. The preserved M3a release image remains26730496 bytes with SHA256
`52f7f4f408a5286194b509d477bf324d9ec44571c8bfdd2bf9dbeaa193bf2246`.
control-accounting.md derives the fixed service allowance from these layouts
and states the remaining native TLS/thread/allocator assumption explicitly.

All completed commands retain start/result/source maps, raw logs and lossless
gzip archives. Two failed workspace runs remain preserved. The first exposed a
real predecessor Store lock lifetime defect, established separately by a
failing duplicate-descriptor regression and fixed by worker-owned explicit
unlock guarding all post-lock startup operations. The second exposed an M3b1
failed-service field-order regression. Both fixes pass the final concurrent
workspace; their focused evidence and the rejected binary-package `--lib`
invocation remain documented in storage-lock-lifetime.md/service-drop-order.md.
setup-provenance.md accounts for the aborted pre-Cargo setup and historical
runner variant. Mixed-source development runs are feedback, not acceptance.

## Stable production seams and witnesses

- `CompleteCheckpointInput::prepare_hydration` consumes the real admitted file
  input. Opaque Send `DecodedHydration` validates exact definitions and all16
  typed owner categories before fixed-move host construction. A child process
  loads the actual file and proves all16 original owner digests without seeding
  or polling a replacement Engine.
- `CheckpointPreparation` admits captured recipes before construction, owns one
  preparation slot and one retirement slot, and keeps a delivered candidate's
  original return entitlement through construction/M2c binding, errors,
  cancellation, shutdown and worker disposal. Retained M2c errors return the
  actual disposal-only graph. Factory diagnostics and panic payloads are bounded
  or destroyed while charged. Tests cover large-capacity errors, delayed work,
  exact generation/definitions and every delivered stage behind an occupied
  Retiring slot and retained SavePayload.
- `ForwardingServices`, opaque `CandidateDisposal`/`RetiredEngineState` and
  `LocalEngine::retire_to_worker` move actual Send service/domain/queue owners.
  An already reserved retirement permit makes submission after fencing
  infallible. Fence-only mailbox and bridge seams leave real payload destruction
  to worker draining. RetirementLease pins actual sender/receiver/job and
  publication lifetimes; transported-owner disposal and last-owner release are
  distinct. Real PromptLog flush/destruction and SessionDir cleanup are exercised
  with an actual delayed destructor witness and external owners still retained.
- Root's eight independent tests and the legacy plus retained service-lifetime
  witnesses pass in the final workspace. The final log retains eight authored
  debug preparation phase arrays and actual LocalEngine queue inventory. Maximum
  observed construction/continuation/return were21.566/13.690/24.106 microseconds;
  maximum candidate worker disposal was125.144 microseconds. One actual old
  LocalEngine detach was15.835 microseconds; worker disposal was4.063247 ms and
  included the injected real PromptLog destructor gate. These are transport
  observations, not populated-city, renderer, frame/p99 or heap acceptance.

## Required next cut and cession

M3b2 must supply the immutable production startup recipe and real allocation
accounting for shared assets/caches, live ECS/services, global PromptLog producer
queues and external runtimes; reconcile Running/candidate/Retiring promotion with
persistent3 MiB storage plus4 MiB preparation charges; extend the retained
delivery through actual ECS staging and atomic whole-host adoption; restore all
saved Host owners, controller samples and time/debt/residual while excluding
preparation time; explicitly flush deferred work and propagate transforms before
consumers; rebase Bevy transport cursors with bounded work while preserving
semantic IDs/order; preserve old attached saves and fence uncaptured old-world
intents. Root's fresh M3b2 handoff details those source-backed barriers.

The512 MiB Running minimum and trusted160 MiB retirement fixture allowance are
not heap measurements. Global queued exchanges may outlive PromptLog, and
shutdown_background/detached provider work does not prove final runtime
allocation release. All such closure and complete App/frame acceptance remain
open. implementation.md contains the detailed boundary.

All owner Cargo/GDB/tool jobs have exited. Source and Cargo are explicitly ceded
to root after this handoff; no further owner mutation is planned. No branch,
worktree, commit, push, visible window, audio or live-provider probe was made.
Unrelated untracked work remains untouched. Root owns acceptance, final status
documents, scoped commit and dispatch of the fresh M3b2 owner.
