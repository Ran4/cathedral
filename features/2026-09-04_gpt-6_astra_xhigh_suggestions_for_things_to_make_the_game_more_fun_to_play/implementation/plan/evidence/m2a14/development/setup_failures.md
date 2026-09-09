# Retained M2a14 development failures — 2026-09-09

- `generate_fixture`: coordinator public harness imported private `world::checkpoint` instead of public `world::WorldBackboneDtoV1`. Compilation stopped before generating a fixture. Root corrected that import and also corrected its raw-input reservation harness; production validation was unchanged.
- `owner_fixture_compile`: three historical test-only Night Flight literals needed an explicit new field. They now use MissingLegacy because no actual provider submission created those fixture flights.
- `owner_initial`: six new Engine/sidecar tests passed, four owner lifecycle checks used stale 1,200/700 expectations from prose. Actual source/provider inputs were 2,400/1,400. Only those expectations changed; all ten private additions pass in the final focused suite.

All failed originals, command-start input maps and deterministic gzip archives remain present. No failed command was relabeled successful or normalized. A generated fixture is new; all historical fixture bytes remain unchanged. GPU unavailability is inherited and was not reprobed.
