# M2a7 release measurements — 2026-09-08

The [runner](../../run_m2_marks_probes.py) uses one frozen release executable for three 100-sample runs per mode, alternating mode order. All **3,600 raw phase samples** are retained in lossless JSON gzip archives. [Results](RESULTS.json), [summary](SUMMARY.json) and [identity](IDENTITY.json) pin commands, source, binary, runners, timings and semantic counters. The [independent audit](../coordinator/marks_performance_audit.json) recomputes all percentiles and summaries and verifies current source/binary/runner hashes. A preceding [smoke audit](../coordinator/marks_smoke_audit.json) covers 24 separate phase samples.

Values below are milliseconds, each the median of three per-run percentiles, not a pooled percentile. There are 300 samples per phase per mode. The authored world has 520 characters; populated has 2,520, with exactly 2,000 production placements and none unplaced.

| Phase | Authored p50 | Authored p99 | Populated p50 | Populated p99 |
|---|---:|---:|---:|---:|
| Preflight | 0.270846 | 0.289593 | 1.060083 | 1.148515 |
| Export | 1.067986 | 1.109896 | 4.229604 | 4.411079 |
| Encode | 0.008235 | 0.011882 | 0.010642 | 0.015769 |
| Decode + validate | 0.273450 | 0.290227 | 1.065616 | 1.124874 |
| Candidate validation | 0.264930 | 0.277493 | 1.054310 | 1.113161 |
| Drop | 0.000192 | 0.000391 | 0.000386 | 0.000998 |

The largest individual export is 4.802286 ms; populated export exceeds the 2 ms host budget. This measures component API work only. Complete Engine capture/hydration, disk, renderer and maximum-owner stress remain unmeasured, and host capture requires bounded offload or incremental coordination.

All repetitions preserve three live marks (one cross, tally and ward sign), two historical authors, both household/place anchors, a faint weathered cross, spent sweep/day clocks and an existing chalk cache. Ordinary scrub followed by a same-day beat remains suppressed; weathering changes strength; ordinary Engine publication occurs once and is deduped on the next poll. Authored/populated caches contain 1/3 anchors and 1/3 kinds. Probe tally setup writes the public counter directly; separate ordinary Round continuation tests cover real increment and saturation. Orphans and maximum occupancy are covered by tests, not this timing workload.

| Admission bytes | Authored | Populated |
|---|---:|---:|
| Encoded J | 1,850 | 2,021 |
| Expanded upper E | 29,932 | 33,020 |
| Validation working | 4,194,304 | 4,194,304 |
| One cohort peak | 4,323,678 | 4,336,543 |
| Simultaneous Save + Load, excluding Running | 8,647,356 | 8,673,086 |

Charges are conservative admission bounds, not measured allocations. The existing backbone+Round naive Save+Load reservation of 1,256,093,444 bytes already exceeds the 1 GiB shared cap before Running or these owners. Full integration must establish actual phase lifetimes without increasing caps.

The [release build](../coordinator/release_build.json) preserves full build stdout/stderr and time output, with raw and archived hashes. Executable SHA-256: `8dec27e533f8415cf5578d809a20051566be8931be1cd1fee648ab9252460fb4`. Source remained frozen throughout build and measurements.

The shared auditor also passes historical Round, climate, knowledge and law datasets. [Three negative checks](../coordinator/auditor_regressions.json) prove it rejects changed percentiles, a reduced validation allowance and a lost scrub-window witness even when raw metadata/hash records are updated consistently.
