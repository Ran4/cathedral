# M2a4 climate component handoff — 2026-09-08

Status: Implemented and reviewed component cut (2026-09-08). Final workspace, source checks and coordinator release measurements pass. M2 as a whole remains in progress.

This cut preserves the private WeatherTimeline, the Engine's live and initial clock descriptors, weather/office sampling cursors and pending bell obligations, and sampled World climate against exact installed context. It reuses M2a1's clock format and M2a2's unadopted backbone interface. The opaque candidates expose no partial World/Engine replacement API. There is no complete save envelope, host capture command or production hydration path.

Base: `31ec2a36506c09bfd538708eb28f4ee6c7bab575`. Final source manifest: **95 files**, SHA-256 `8aec88e3c25b186ce96a6a237f3ac61861f9bac5c807e8980ab9baef237ffd27`. Coordinator independently checks all 16 changed implementation/fixture paths, all 10 scoped Rust files and all 11 unchanged historical fixtures. Engine/weather ordinary logic is byte-identical to the base after removing the new module declarations; no ordinary CPU benchmark is repeated for this cut.

## Reviewable component contracts

[OWNER_COVERAGE.md](OWNER_COVERAGE.md) maps every covered private field, exact references and preserved nullable/stale states. [ADMISSION.md](ADMISSION.md) gives the closed-layout, queue capacity, sampling scratch and context traversal argument. [layout.json](layout.json) ties measured type sizes to the final focused raw output.

`WeatherTimeline::export_checkpoint`, `World::export_climate_checkpoint` and `Engine::export_climate_checkpoint` export strict v1 records only under attached admission. Public DTOs expose validated bounded decode, encode and opaque candidate conversion. `ClimateCheckpointContext` accepts either a running World or an unadopted backbone plus installed navigation, shelter, area and sound definitions. World time and the sound switch are consistency copies of existing backbone authority. Context fingerprints do not replace the still-pending complete World manifest or geography/resolver validators.

The exact timeline preserves forced wetness inheritance, drying residue, reset/disabled states and wrapping override revisions. Queued office strokes keep their original logical deadlines and identities through a live scale change; duplicate ordered deadlines remain distinct obligations. The initial clock descriptor is validated at its own origin, so its unused old slope need not remain in the present calendar format range. Live clock position and sampled copies must agree exactly at the saved boundary.

The explicit v1 WeatherClimate restriction accepts finite nonnegative knobs at most 1,000,000 with ordered duration bounds. It is narrower than arbitrary public runtime `with_climate` values. Existing clock numeric ranges remain format policy, not CPU-safe effective-rate proof. Next-pass bell sequence headroom does not establish a complete lifetime accounting horizon.

## Verification and fixtures

[verification.json](verification.json), [commands.json](commands.json) and [log_archives.json](log_archives.json) record exact commands/environment, process results, original paths and original/archive hashes. Final focused checkpoint tests: **50 passed, 0 failed, 5 ignored**. Final workspace: **1,928 passed, 0 failed, 13 ignored across 36 targets**. The unchanged public Character/Round tests and four new coordinator climate tests contribute 13 passing public cases, all included in the final workspace.

The new private witnesses cover scheduled climate and unique lightning crossings; forced weather with inherited wetness; override clearing and residue; disabled/zero-frequency explicit overrides; nullable creation anchors and revision wrap; queued office bells across a scale change; and initial/live clock provenance. Continuation controls are independently driven through the same non-climate history. The resumed Engine's covered fields are deliberately scrambled, then restored through the test-only seam and immediately compared byte-for-byte before any poll can mask an omitted cursor. This is component continuation evidence, not production whole-Engine hydration.

Corruption tests reject missing nullable fields, duplicate/unknown fields, invalid numbers/anchors/counters, mismatched sampled copies, wrong installed context, wrong backbone time, exhausted bell sequence and over-limit queues. Maximum 65,536 duplicate deadlines, raw input with 1 MiB extra whitespace, excessive nesting and many empty containers test admission before typed decoding and attached charge release. Root's public cases independently cover changed sound percept context, disabled overrides/residue and candidate-to-candidate time binding.

Six new exact fixtures under `crates/cathedral-sim/tests/fixtures/checkpoint_v1/` are pinned by normal tests: `climate_initial.json`, `climate_active.json`, `world_climate_virgin.json`, `weather_initial.json`, `weather_forced.json` and `weather_residue.json`. The two explicit ignored writers generate only these component fixtures. They are not M3 save slots; all historical fixtures/goldens are unchanged.

The first private run had four assertion failures because the test expected a first bell deadline of 15.5, whereas the clock's exact value is 15.499999999999998. All four failed before fixture comparisons. The expected queue now uses production `elapsed_at_day` and `stroke_times`; no runtime rounding or behavior changed. A successful first full workspace is retained as intermediate evidence against `coordinator/pre_assertion_source_hashes.json`. Only the immediate restoration assertion changed afterward; final `focused-frozen` and `workspace-frozen` verify the final manifest.

All original raw logs remain untouched at their `/tmp/alibi-m2a4-*.log` paths. Development archives are lossless gzip with `mtime=0`. Final text archives remove only redundant terminal LF bytes using `raw.rstrip(b"\n") + b"\n"`, with separate raw/archive hashes and sizes; no other bytes change. The final raw workspace SHA-256 is `076dfc90ce97a04af8c87f4d0213e204865b8fa7101cc239303808329825d3b6`.

## Measurement handoff and remaining work

`alibi_climate_cost --mode authored|populated --samples N --output PATH` follows the existing six-phase schema. The scenario contains forced storm weather and three pending office strokes computed before a scale change. Metadata includes actual population/placement, bell/forced/residue counts and a separate **65,536-byte validation allowance**. [diagnostics.json](diagnostics.json) preserves one optimized-debug development sample per mode: both are 2,316 encoded / 31,314 expanded / 201,836 retained bytes per cohort, with 403,672 bytes for standalone Save+Load **excluding Running**. Populated places exactly 2,000 extra citizens. These single samples are not release percentile evidence.

The coordinator completed the [release build and repeated measurements](performance/README.md) after source/cargo cession: 3,600 phase samples, all source/binary/runner hashes stable, with [independent acceptance review](coordinator/review.md). The forced-sky probe skips scheduled semantic-boundary/drying sampling, so it cannot establish universal climate p99. Scheduled continuation and scratch tests address that distinct path. Historical performance archives and reference binaries remain untouched.

The accepted M2a2 plus M2a3 standalone Save+Load sum already reaches **1,256,093,444 bytes**, above 1 GiB before Running or this component. No existing budget rises. Complete composition needs real phase/lifetime and offload coordination; previous component latency failures still rule out synchronous host-frame placement. Full capture/adoption, Running/retiring coexistence, all-consumer calendar horizons and remaining World/Engine owners stay pending.

The proposed next coherent owner cut is knowledge/facts/pollen and its geography/area-adjacency contract. Conversation/attention, law and scheduler/speech composition follow their actual dependencies. No M2a5 begins before M2a4 review and commit.
