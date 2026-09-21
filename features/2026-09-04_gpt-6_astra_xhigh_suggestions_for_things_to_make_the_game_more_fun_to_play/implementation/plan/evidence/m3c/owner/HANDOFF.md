# M3c owner handoff

Implemented the first explicit control-ownership/refusal seam after accepted
startup staging. This is **not completed M3c save/load UI or whole-App adoption**;
the concrete remaining gates are in [DESIGN.md](DESIGN.md).

## Changes

- `checkpoint_controls.rs`: fixed, admitted terminal control receipts, checked
  operation identities, actual owner/generation checks and F6/F9 HUD refusals.
  No checkpoint operation is submitted, awaited or marked successful.
- `smart_actors/bridge.rs`: shared Staged/Active/Retiring route replaces the
  shared active boolean. Both senders reject inactive input before identity
  allocation; retirement cannot be rolled back into active control.
- `smart_actors/local_engine.rs`: stages its endpoint before construction,
  activates only on startup success, and exposes a borrowed ownership check
  against the actual endpoint and committed recipe identities.
- `installed_recipe/startup.rs`: reserves the disjoint fixed 512-byte control
  owner before construction, publishes it with the startup resources and checks
  duplicate installation before mutation. Existing service owners are retained.
- Minimal key-system and real LocalEngine tests cover refusal conservation,
  stale/impostor endpoint rejection, surviving retired worker clones, zero
  allocation across 10,000 refused attempts and operation-ID exhaustion.

The runner is copied from accepted M3b2c with the M3c raw prefix. It records
source/helper/environment at command start, retains exact stdout+stderr and
archives deterministic mtime-zero gzip with hashes. No coordinator tests or
historical fixture bytes changed. Pre-existing dirty LOGS_FOLDER and b2c
HANDOFF documents plus unrelated untracked paths are preserved.

## Verification

`focused-01`: 15 passed / 3 failed / 0 ignored; exit 101, source unchanged.
The three isolated controls fixtures incorrectly used a bare CheckpointBudget;
the established persistent-overhead API requires a retained Running root.
Actual startup, sender/retirement and LocalEngine continuation witnesses passed.
The fixtures now retain the real InstalledRecipe root and compare against its
baseline, with pressure submitted through the existing overhead API. No
production change was needed. Exact first raw output, archive and start/result
metadata are retained, including the failure assertions.

`focused-02` repeats the same selected tests on settled source:
checkpoint_controls, existing/new bridge tests, LocalEngine control continuation
and installed startup tests. Result pending process exit.
No full-workspace, complete-frame, render or provider acceptance is claimed by
this owner; the coordinator owns independent review and broader checks.
