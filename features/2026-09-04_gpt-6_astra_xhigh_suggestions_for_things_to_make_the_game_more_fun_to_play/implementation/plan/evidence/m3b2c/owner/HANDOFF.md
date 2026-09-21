# Owner handoff: staged startup / committed recipe

This cut implements startup owner publication, not whole-App checkpoint
restoration. See DESIGN.md for the explicit remaining gates and the hard
`UnprovedWholeAppAccounting` refusal. No world-size estimate, schema/cap increase,
live restore, extra simulation poll, visible app, audio/device or provider call.

## Changes

- Added `installed_recipe/startup.rs`: immutable `CommittedStartup`, fallible
  `StagedStartup::prepare`, and startup-only `install` with owned refusal before
  resource mutation. One shared budget retains Nav graph/full cache, config,
  admitted runtime/archive/preparation and the existing diagnostics.
- Main commits post-environment settings and all staged owners before plugin
  construction. NavDebugPlugin and the map teleport consumer use the installed
  graph. LocalEngine reuses it and resolved backend settings/runtime.
- BackendsHandle stores Arc<BackendsConfig>; new `with_shared_runtime` and
  existing `next_generation` share immutable settings with fresh endpoints.
- LocalEngine and EngineGuard retain the installed recipe; retirement carries
  it after actual domain/service disposal. The preparation worker is separate
  from the recipe to avoid giving its retirement payload a self-join owner.
- One committed PromptLog template supplies forks; generation construction
  shares filename/progress identity and the admitted writer.
- Test-only reuse of the existing synchronous allocation counter proves the
  embedded Nav parse's scope without installing a second global allocator.

## Evidence

Runner copied from accepted b4 with the c prefix; it preserves exact start
source maps/environment/helper SHA, combined raw bytes, deterministic mtime-zero
gzip and hashes. No historical fixture or public test file changed.

`focused-01`: exit101, compilation failed before tests; five errors were the
map's old Navigation tuple accessor and four legacy build calls needing an
explicit uninstalled wrapper. Source was unchanged during this command. Raw
and archive are retained. Afterward, source review additionally placed the
config lease after shared services and retained the recipe in EngineGuard.

`focused-02`: 4 passed / 0 failed / 0 ignored, source unchanged. Witnesses cover
Nav count/parser allocation, budget-pressure refusal before backend resolution,
shared consumer identities/duplicate-install rollback, and actual LocalEngine
Hello preserving the Nav Arc into EngineConfig and World. Parser cumulative
requested bytes19,689,148; count-only preflight8. The subsequent PromptLog
session correction is deliberately outside this run's acceptance.

Final `focused-03` result will be recorded after completion. Its filters include
all installed_recipe, LocalEngine startup, session_log and config tests, adding
the real same-second archive collision regression. No full workspace or release
performance claim is made by this owner; coordinator review/checks follow cession.

The pre-existing dirty b4 HANDOFF and LOGS_FOLDER documentation and all unrelated
untracked docs/feature drafts/gauntlet/reference paths are preserved.
