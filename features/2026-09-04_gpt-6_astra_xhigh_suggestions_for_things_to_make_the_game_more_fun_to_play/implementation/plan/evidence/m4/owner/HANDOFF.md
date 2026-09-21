# M4 owner handoff — street-query foundation

Accepted predecessor: `7e7f0bb` (M3d). This bounded slice is ready for coordinator review; it is not completion of M4 or its M3 entry prerequisites. Source/Cargo/executable ownership was explicitly ceded after host session 14024 exited. No owner process remained live; only owner documentation changed after that cession.

## Implemented

- Pure `nav::query`: borrowed StreetQuery, explicit Surface, closed error enum, canonical bounded Closure slices and RouteTicket graph/revision/geometry identity. Interior requests remain explicitly unsupported. Node/adjacency/grid-span/work bounds refuse unsupported queries deterministically. Existing A* ordering, baked costs and distance caches are preserved.
- `actions::route_budget`: bounded graph pricing with the existing XZ nearest-node anchor semantics. No floor-qualified World migration is implied.
- Actual map-click system: fixed-array projection of active physical gate barriers; malformed, duplicate or unknown overlapping owners refuse. A closed destination emits no TeleportPlayer and leaves the map open; reopening permits the original destination. No new persistent door state or checkpoint schema.
- Six pure query regressions and two host regressions. The host fixture runs the actual map-click system with committed NavData and a controlled gate barrier, without renderer/audio/provider initialization. A test-only enabled AreaDebugState constructor supports the fixture.

Changed source: `crates/cathedral-sim/src/nav/query.rs`, `nav/query/tests.rs`, `nav/mod.rs`, `actions.rs`, `src/map.rs`, `src/smart_actors/area_debug.rs`. No historical fixture was changed. Preexisting dirty rule/prior handoffs and unrelated untracked paths were preserved.

## Verification and retained failure

| Command | Passed | Failed | Ignored | Result |
| --- | ---: | ---: | ---: | --- |
| pure-01 | 37 | 0 | 0 | Initial query plus existing nav/go_to/navigation cases; 36 emitted groups including empty filtered groups |
| pure-02 | 33 | 0 | 0 | Unit suite after adding graph/grid envelope guards |
| custody-01 | 0 | 1 | 0 | Real compatibility regression from imposing strict street height on a supported y=0 legacy custody anchor |
| compatibility-01 | 7 | 0 | 0 | Existing go_to and custody regressions after preserving XZ graph pricing |
| host-01 | 13 | 0 | 0 | All map tests, including actual closed/reopened gate click and malformed-owner refusals |

All five command source maps remained unchanged during their runs. The custody failure is preserved as a production regression, not relabeled a fixture problem: the action helper also serves seizure, so its old XZ anchor contract had to remain until World/mover adoption. No existing test was weakened or edited. Only the historical perf::Probe warning appeared in the host build. Scoped rustfmt and git diff --check passed. The coordinator owns wider workspace checks.

Final source-map SHA256 (compatibility-01 and host-01):
`618bd7ad8684f9ecf9843d8cac71dda05ce817dcba730b45ec1f5ed0c383a610`.

Final host raw log: `/tmp/alibi-m4-host-01.log`.
Raw SHA256: `e47be5470f2b4e44f9f566e7bf8d5ffbd01bcc7938ffbe488afe1061f60fd83f`.
Archive SHA256: `188c06ef371ea83d7f3b669300cee119bce6b1b9cdf04b8caf35273f6704ec6d`.

Each command retains `<label>-start.json`, `<label>-sources.json`, `<label>-result.json` and `<label>.log.gz`; raw originals remain in `/tmp/alibi-m4-<label>.log`. `run.py` records exact start source/helper/environment identity and preserves combined stdout/stderr in deterministic mtime-zero gzip. No GPU/device/provider probe, window or sound ran.

## Remaining gates

See DESIGN.md for the exact contract. This is ordinary street graph routing, not a permissive player-traversal certificate or fastest-route proof. Exact endpoint approach remains separate. Map validation checks the barrier snapshot at input production; queued TeleportPlayer revalidation, portal-aware NPC movement, floor-qualified World ownership, an authored two-floor building, stairs/support/crowd semantics, checkpoint migration and full frame/render acceptance remain open. Query caps are finite component bounds, not admission of a whole running world.
