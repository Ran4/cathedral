# Staged startup / committed recipe seam — 2026-09-21

This is the smallest startup publication part of M3b2c. **Whole-App checkpoint
restoration and live adoption are not implemented or accepted by this cut.**
`CommittedStartup::require_complete_admission` explicitly returns
`UnprovedWholeAppAccounting`; caller estimates cannot override it. The complete
allocation and restoration gates below remain mandatory.

## Construction and refusal

Main loads bounded AppConfig under its accepted lease and applies existing
scalar environment overrides. `StagedStartup::prepare` then consumes it and the
shared InstalledRecipe into an immutable committed Arc owner, before creating
the App. It parses the embedded NavData once, resolves BackendsConfig once,
and starts the existing admitted runtime (16 MiB), prompt archive (20 MiB) and
preparation service (4 MiB) on that same budget. Existing diagnostic sinks keep
their separate two 8 MiB leases. No provider request, speech device or window is
started by this staging constructor. BackendsConfig still performs its normal
environment/file capability resolution; its unbounded input/copy accounting is
explicitly a remaining gate.

The navigation node array is counted without building a JSON Value. Its exact
maximum cache rows are reserved before parsing. A separate 64 MiB construction
allowance applies only to the current immutable embedded navigation artifact,
not to a mutable world or an arbitrary asset factory. The owner allocator
witness counts all synchronous requested allocation bytes, including immediate
frees/reallocations, for that actual parser input: **19,689,148 bytes** in
focused-02, with **8 bytes** in count-only preflight. After parsing and all parser
temporaries have died, the charge shrinks to actual graph/index/Arc capacity
plus the unchanged maximum cache. No cache rows are warmed for admission.
The actual 10,026-node cache maximum is 402,644,224 bytes; its old ~60 MiB comment
is not an admission bound. Cache-fill scratch remains outside this scoped
retained inventory and must join complete authority accounting later.

Any preparation error drops staged owners without accessing an App. Startup
returns an error before game construction rather than publishing partial shared
services. This also makes a broken embedded Nav artifact a startup refusal,
rather than the old separate engine/overlay fallback. These compiled artifacts
are verified in focused tests. `install` checks for an existing recipe or
already-built actor/navigation plugins before publishing resources. Refusal
returns the complete staged owner and leaves all existing resources untouched.
There is no live-world swap, command drain, extra poll or partial restore.

## Actual consumers and lifetime

Main keeps a committed handle across App::run and installs a resource before
plugins. The ordinary plugin uses that same immutable configuration.
NavDebugPlugin holds the same Nav Arc plus a recipe handle. Map teleportation
uses that resource's graph; CityPlugin had no separate production Nav parse
(its NavData parse occurrences are tests). LocalEngine uses the same Nav Arc
and resolved backend config/runtime; its ordinary Hello moves this Arc into
both EngineConfig and World. Legacy isolated tests/embedders retain an explicit
uninstalled construction path.

BackendsHandle now stores Arc<BackendsConfig>; successor generations clone the
Arc and preserve distinct immutable endpoints on the shared runtime. Cognition
and speech construction retain their existing per-service behavior. Installed
startup constructs one PromptLog template with the admitted shared archive
worker. LocalEngine forks that session, preserving its same-second filename
counter, directory, model and completion progress across generations; sharing
only the worker would not preserve those contracts. The old default archive
writer is bypassed on this production path. A focused real-file regression
records two same-second/same-actor exchanges through separate committed forks
and requires both ordered archives to survive.

Both LocalEngine and EngineGuard retain the recipe. RetiredLocalEngine carries
it after the domain, services, backend guard and callback owners. The committed
owner drops parsed data and shared services before their navigation/config
leases. AppConfig's lease follows the bounded startup copies through shutdown.
The preparation worker is deliberately outside the Arc recipe: a retiring
payload can retain that recipe on the disposal worker, and must not thereby own
a join guard for the same worker. Its accepted service keeps its own native
lifetime admission through actual exit.

## Remaining whole-App gates

- Installed non-navigation definitions and a HydrationAssets factory must be
  committed without borrowing a live World or re-running ordinary creation.
- Backend environment/keys/worker settings, HTTP transport/idle policy and
  detached configuration/task captures need complete bounded accounting.
- Actual mutable live/candidate/retired authority and ECS/projection storage,
  cache-fill scratch and remaining startup metadata/path scratch need disjoint
  source-backed allowances. Neither 64 MiB parser scratch nor 512 MiB aggregate
  Running is a per-world estimate.
- The bootstrap control root still is not a movable mutable-world authority
  root. Complete allocation promotion must separate persistent recipe ownership
  from the measured/admitted generation roots.
- Complete inactive ECS staging, every saved host row, physical/fixed clocks,
  initial no-poll publication, final exclusive adoption and bounded old-world
  retirement must be implemented together after admission is proved.

The existing 512 MiB aggregate Running minimum, 128 MiB typed ceiling and 1 GiB
shared cap are unchanged. A staged recipe is not a permission token for a
complete load. No complete host/frame/RSS acceptance is claimed.
