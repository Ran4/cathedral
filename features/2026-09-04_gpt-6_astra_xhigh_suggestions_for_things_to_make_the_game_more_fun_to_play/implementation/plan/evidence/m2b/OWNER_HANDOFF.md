# M2b implementation-owner handoff

Status: implementation and final owner verification complete; source frozen and
Cargo ceded to the coordinator on2026-09-15. Acceptance, release measurements,
fixture execution/publication and commit belong to the coordinator.
No implementation-owner commit, push, worktree, sub-agent or renderer run occurred.

Production hydration is implemented in checkpoint/complete/hydration.rs,
engine/hydration.rs and the existing complete owner modules. The complete decoder
returns one private ValidatedOwners graph after all unchanged owner/cross-owner
checks. Ordinary validation still disposes it. Hydration moves that graph into
exhaustive World and Engine literals, plus explicit typed speech, accepted
cognition input and Host continuation owners. It retains no raw checkpoint.

Public entry/observation seams:

- Admitted<CompleteCheckpointCandidate>::prepare_hydration(assets_upper_bytes).
- Admitted<HydrationPreparation>::hydrate(factory, host_definitions,
  RuntimeGeneration), or hydrate_observed with six host timing checkpoints.
- HydrationAssets::new(original_parsed_seed, config, fresh_prompt_env,
  HydrationWorldAssets), with independent World/config roles and the fresh
  InstalledCheckpointDefinitions::from_assets resolver.
- HydratedEngine boundary, lineage, runtime generation, costs, character/item
  counts, protected-speech-action count/membership and typed continuation views.
- category_digest(category, same-budget SavePayload reservation), serializing
  the actual new domain owner or its explicitly retained continuation owner.

There is no public Engine/World borrow, mutation, poll or extraction. Construction
calls no normal World/Engine/Round creation, provider probe/request, tick or drain.
Exact saved logical/calendar scalars and lineage remain; only Engine's ephemeral
runtime generation changes to a nonzero fence different from saved Host. Empty
inert transports/capabilities and a session-only empty omniscient transcript are
declared policies, not loss of protected speech or committed Host authority.

[Owner design](owner-design.md), [admission](ADMISSION.md) and
[coverage](coverage.md) record the full field/drop/asset/observation contracts.
The unchanged128 MiB expansion ceiling includes262,144B of new structural storage.
Actual authored/populated expansion is38,308,507/133,613,327B; populated headroom
is604,401B. The separate64 MiB factory lease is proven by cumulative requested
allocations plus distinct retained nav capacity plus1 MiB shared Prompt/runtime
allowance. All2,000 requested additional residents are actually placed.

The final focused run observes one nonempty accepted recording and one actual
new protected speech action, including exact ID membership. Separate ordinary
fixtures retain a held scheduler result with speech capture/stream and a submitted
Night prompt/duty. Held Night success/error and recognized draft text are not
claimed by those complete hydration scenarios.

## Verification history and evidence ownership

Every owner command uses owner/run.py and preserves original raw stdout/stderr,
lossless gzip, effective/removed environment, exact argv, start source map and
helper identity. Original helpers run-v1.py and run-v2.py remain archived. Early
v1 gzip timestamp headers remain untouched; v2 archives use mtime0 and record
enumerator and inherited target environment identities.

| Command | Result and interpretation |
| --- | --- |
| check-01 | cathedral-sim cargo check passes; early development source. |
| tests-02 |8 complete tests pass; sources changed during command, development only. |
| tests-03 |10 host hydration tests pass,1 ignored; source unchanged, superseded by later accounting/fixture work. |
| format-04 |24 Rust files pass; source unchanged, superseded. |
| workspace-05 |2,190 passes,0 failures,43 ignored in46 groups; source changed after newly identified shared-cache/root-index review points, superseded. |
| format-06 |24 Rust files pass on the first root-index fixture version. |
| tests-07 |10 passes,1 failed precondition,3 ignored; exact frozen source preserved. The new recording fixture used an old Engine sample instead of the ordinary current physical input producer, so it captured no accepted recording. |
| tests-08 |11 passes,0 failures,3 ignored after ordinary PhysicalPosition/PlayerSpatialState staging correction; source unchanged. Accepted recording/root count1 now asserted independently. |
| format-09 |Final24 changed/new Rust files pass; source unchanged. |
| workspace-10 |Final full workspace passes2,191 tests,0 failures,43 ignored in46 groups;451.43s; source unchanged. |
| diff-11 |Final git diff whitespace check passes; source unchanged. |
| audit-12 |All original raw/gzip/hash/helper/start records verify, including failed/superseded attempts; current source equals final map. |

Final frozen source scope is967 inputs, map SHA
be5312aa3547f660396f61d39014354446f765211ca0ea2c10314bc5b54eb57b.
tests-08, format-09 and workspace-10 start maps agree. The exact serialized map is
[source_hashes.json](source_hashes.json); the consolidated record is
[owner/verification.json](owner/verification.json). Final workspace raw log SHA
is144efcc146e8e20684033cc0b469e7bea7b1bfbee365e451f2b116de36f2d565.
The final debug host test binary is
/home/ran/src/rust/cathedralbevy/target/debug/deps/cathedralbevy-eafe86af140beaa1,
2,291,439,072 bytes, SHA
7ca7c8e3aed207a323a575358f409076e7cb836ffef8afcf89f223d115f15f1d.
Root owns its preservation before subsequent image work. All owner processes
have exited; no further owner Cargo invocation is authorized without explicit
reassignment. Available disk at cession is47 GiB; nothing was cleaned.

The exact old M2a16 fixtures
and all their historical evidence remain unchanged. New fixture files are not
embedded in source or rewritten into compatible manifests.

Root owns tests_hydration_public.rs and all independent assertions there. Owner
tests expose only admitted installed-asset setup to those tests. Root also owns
release sampling, preserved-image copying, persisted fixture execution and final
acceptance/commit. Ignored writer/read tests compile but have not been executed by
this owner; root will execute them after Cargo cession.

## Remaining milestone obligations

M2c must jointly prepare speech/Floor/ledger interruption, exact cognition retry
and held-completion once-only application with new execution IDs. M2d must run
the full continued-input control suite. M3 must own actual files, atomic whole-ECS
and service adoption, initial publication, active/retiring/cache/renderer leases
and host-origin binding. HydratedEngine is non-Send because of existing Engine
service trait objects; a Send decoded bundle with bounded host-thread construction
and rebinding remains required. Neither unsafe Send nor synchronous frame-budget
acceptance is introduced.

Caller-retained shared navigation Arcs and persistent process/TLS Prompt caches
must remain under coordinated Running ownership when outside the hydrated lease.
The64 MiB scoped factory allowance does not include eventual full navigation
cache warming; current public observations cannot trigger that growth.

Unrelated docs/codex_gdd/, gauntlet/, reference/ and
features/2026_09_14_more_ambient_stuff.md remain untouched. The denied /home/ran/w
tree was not accessed. All host runs use headless/fake flags, all Cargo commands
are offline -j1, and no Cargo process overlaps another.
