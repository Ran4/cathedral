# M5 evidence — 2026-09-06

`../m5_measurements.md` is the reviewed interpretation. `RESULTS.json` contains the parsed final
band, per-ward counts, crowd timings/deltas and manual guard. `summarize_measurements.py` verifies
the raw records and reproduces that JSON without any provider or simulation call.

- `band`, `base`, `flat`, `crowd-{0,1000,20000}-{on,off}`: full logs and exact pollen streams in
  gzip, plus uncompressed `/usr/bin/time -v` records. Phase JSON files retain each exact command,
  environment change, binary hash, exit status and pollen hash. Gzip is storage compression only.
- `ignored-20000` and `same-trade`: unchanged release tests with output and timing captured.
- `knowledge-m5-gate.json` and matching compressed logs: final explicit-file format check,
  clippy output, workspace gate, canary, independently rerun allocation test and app build.
- `VISUALS.json`, `knell.log.gz`, `door.log.gz`: accepted drive commands, fixture configuration,
  limitations and 139 unmapped/unfocused X11 samples. Rejected fixtures are explained there and
  in the independent review; the accepted door diagnostic is the behavioral evidence.
- `knell_journal.png`, `ward_heat.png`, `shut_door.png`: visually inspected captures, stored via
  Git LFS. They use the documented software UI-asset fallback; no real-GPU claim is made.

`run_measurements.py` rebuilds nothing and calls no provider. Build once, then run its phases
sequentially with no source edits or competing app/build runs. `run_visuals.py` creates an
unmapped window, uses fake cognition and no AudioPlugin, and requires the real mint/door
diagnostics before accepting a screenshot. The door drive changes only a temporary configuration
copy. It does not edit the user's configuration.

The source gate preceded the final tune. No source changed afterward; the additional same-trade
test captures a diagnostic hidden by the otherwise green normal test runner. Raw commands retain
their original paths, before the feature folder was moved at landing.
