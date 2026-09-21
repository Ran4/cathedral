# M3b2b4 owner handoff — installed startup slice

Coordinator narrowed the original complete-allocation dispatch to the explicit
InstalledRecipe/accounting seam, focused tests and the proven startup path.
See DESIGN.md for the exact admission scope and still-open broader b4 gates.
No branch, commit, push, visible app, audio, device or provider execution.

## Source/API

- `src/installed_recipe.rs`: `InstalledRecipe::new/budget/load_config`,
  bounded `retain_sources`, `RecipeSources::source/charged_bytes`, and
  borrowing-only `InstalledCost::navigation/total`. Startup settings retain a
  separate lifetime lease through `InstalledConfig`. These are crate-local.
- `src/main.rs`: creates shared startup owner before session logging; keeps
  config owner through App disposal. Title moves into Window via `mem::take`,
  preserving its previous text. No new per-world reservation.
- `src/session_log.rs`: `init(&CheckpointBudget)` starts both production sinks
  admitted. `bounded.rs` limits the unadmitted constructor to tests. No queue,
  evidence deadline, prompt archive or overload policy changes.
- `src/config.rs`: fixed 64 KiB plus sentinel file read; existing fallback
  behavior on refusal. The now-unused unadmitted `load_config()` convenience
  wrapper was removed; injectable `load_config_from_paths` remains.
- `src/installed_recipe/tests.rs` and one new test in
  `src/session_log/tests_bounded.rs`: six owner tests. Existing public diagnostic
  tests were not edited. A coordinator test module can be added to the new
  installed_recipe module after source cession.

## Verification / evidence

`run.py` preserves exact source-at-start maps, environment, helper identity,
raw combined stdout/stderr, deterministic mtime-zero gzip and result hashes.
Existing b3 runner was reused with the b4 raw prefix. All Cargo runs are offline,
one job, headless/fake, using the established compiler/cache environment.

`focused-01`: exit 101, 5 passed / 1 failed. This development run started before
the crate-private CheckpointError constructor usage was corrected and before
scoped future-seam warning documentation was added, so `sources_unchanged` is
false. It compiled successfully. Its actual test failure assumed a deep NavData
clone retained identical Vec capacities; Rust compacts those copies. Corrected
test bills the clone's actual graph capacity, preserving the shared-cache check.
Original raw log and archive remain intact.

Final stable focused result: pending command completion when this draft was
written; update below before final handoff.

Actual city inventory: 10,026 nodes; 6,366,037 graph/index/Arc bytes;
402,644,224 maximum cache bytes; 409,010,261 combined bytes. Inventory leaves
all rows cold. The historical `~60 MiB` source comment is not a valid current
bound. No full Running, two-world, frame or RSS acceptance is claimed.

## Preserved work

The pre-existing dirty `.claude/rules/LOGS_FOLDER.md` was read and preserved,
not changed by this slice. Unrelated untracked docs, feature drafts, gauntlet
and reference paths were preserved. No historical payload fixture was changed.
