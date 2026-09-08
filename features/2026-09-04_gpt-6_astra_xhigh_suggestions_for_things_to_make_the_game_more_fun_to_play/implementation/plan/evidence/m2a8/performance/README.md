# M2a8 release measurements — 2026-09-08

The [runner](../../run_m2_animals_probes.py) executes three 100-sample runs per mode with alternating order on one frozen release executable. All **3,600 phase samples** remain in lossless JSON gzip archives. [Results](RESULTS.json), [summary](SUMMARY.json) and [identity](IDENTITY.json) retain exact commands, counters, source/binary/runner hashes and process timings. The [independent audit](../coordinator/animals_performance_audit.json) recomputes every percentile and summary, validates witnesses and byte charges, and checks current source/binary/runner hashes. The preceding [smoke audit](../coordinator/animals_smoke_audit.json) covers 24 separate phase samples.

Values below are milliseconds, each the median of three per-run percentiles, not a pooled percentile. Each mode has 300 samples per phase. The authored cast contains 520 characters; the populated world contains 2,520 after all requested 2,000 production placements succeed.

| Phase | Authored p50 | Authored p99 | Populated p50 | Populated p99 |
|---|---:|---:|---:|---:|
| Preflight | 0.013598 | 0.014699 | 0.013745 | 0.015148 |
| Export | 0.027448 | 0.029642 | 0.027758 | 0.032386 |
| Encode | 0.018049 | 0.020626 | 0.018367 | 0.023351 |
| Decode + validate | 0.013978 | 0.018793 | 0.014125 | 0.023457 |
| Candidate validation | 0.000295 | 0.000416 | 0.000282 | 0.000375 |
| Drop | 0.000239 | 0.000393 | 0.000240 | 0.000418 |

The largest individual export is 0.042930 ms. These ten-dog component calls remain below 2 ms in this diagnostic. This does not measure complete Engine capture/hydration, disk, renderer, maximum pack/path occupancy or the complete host frame. Earlier components still require bounded offload or incremental host coordination.

Ordinary Engine polls select the same boundary in both modes: ten dogs, eight resting, one moving and one turning, with two active paths and sixteen waypoints. Two decision epochs are spent. The initial resting pack publishes once, the quiet sub-slice publishes nothing, and 27 movement publications witness acceleration and turning. The exact boundary is 6.061000000000001 seconds; the saved movement anchor is 6.0499999999999865, retaining a residual of 0.011000000000014332 seconds. No private animal path/rest/epoch is manufactured for this workload. Arrival stops, failed routes, wrapping counters and maximum collection/path shapes are separate deterministic tests.

Both modes have identical charges:

| Admission quantity | Bytes |
|---|---:|
| Encoded J | 4,188 |
| Expanded upper E | 70,590 |
| Validation working | 1,048,576 |
| One cohort peak | 1,347,596 |
| Simultaneous Save + Load, excluding Running | 2,695,192 |

Charges are conservative admission bounds, not allocator measurements. The existing backbone+Round naive Save+Load total of 1,256,093,444 bytes already exceeds the unchanged 1 GiB shared cap before Running and the other owners. Complete integration must establish actual phase lifetimes.

The [release build](../coordinator/release_build.json) retains complete stdout/stderr and time logs with original/archive hashes. Executable SHA-256: `32fd3e1c55ca9247ae327165662c524b94f266e360f7562c36fa53a7f057f5b4`. The final 841-file source remained unchanged throughout compilation and measurement.

The shared auditor also passes the historical Round, climate, knowledge, law and marks datasets. [Three negative checks](../coordinator/auditor_regressions.json) reject altered percentiles, a reduced validation allowance and an invented quiet-rest publication, even after raw hashes and metadata are updated consistently.
