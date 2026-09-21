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

`focused-02`: exit 0, **29 passed / 0 failed / 0 ignored**, 602 filtered,
`sources_unchanged=true`. One Cargo invocation selected the installed recipe,
session_log and config test families, using `--bin cathedralbevy`; this includes
the six new owner tests and existing diagnostic/public/config regressions.
Only the historical perf::Probe dead-code warning remains in the test build.
Elapsed command time 195.579 seconds; test execution 0.29 seconds. Scoped
rustfmt and git diff --check also passed before this final command.

- Exact final source map: `focused-02-sources.json`, SHA-256
  `0c4ea86662fe07ae06f1fcc10b7dbffec43c7a8f618921df135a63b23f75ff21`.
- Raw: `/tmp/alibi-m3b2b4-focused-02.log`, SHA-256
  `e8cb43e7fb91727fce65a46d574d84911d92cf29c02e7152e27833d333757eed`.
- Archive: `focused-02.log.gz`, SHA-256
  `c0327927cbc8de923525917395e7eddf771ca2cbd4dc6284624f9d1e111336a5`.
- Source/Cargo/executable ownership explicitly ceded to coordinator after
  session3272 exited successfully. No owner commands remain active. No source
  edits after that result; only this documentation was finalized.

No full workspace, release performance or whole-App acceptance was run by this
owner; those remain coordinator work after independent review.

Actual city inventory: 10,026 nodes; 6,366,037 graph/index/Arc bytes;
402,644,224 maximum cache bytes; 409,010,261 combined bytes. Inventory leaves
all rows cold. The historical `~60 MiB` source comment is not a valid current
bound. No full Running, two-world, frame or RSS acceptance is claimed.

## Preserved work

The pre-existing dirty `.claude/rules/LOGS_FOLDER.md` was read and preserved,
not changed by this slice. Unrelated untracked docs, feature drafts, gauntlet
and reference paths were preserved. No historical payload fixture was changed.
