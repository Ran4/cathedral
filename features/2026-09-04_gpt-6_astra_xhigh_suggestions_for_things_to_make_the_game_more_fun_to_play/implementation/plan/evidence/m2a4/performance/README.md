Status: Accepted component measurement (2026-09-08). Full checkpoint and scheduled-weather latency acceptance remain pending.

# M2a4 release climate measurements

Six sequential release runs alternate authored and +2,000 populations, three runs per mode and 100 raw samples for each of six phases: **3,600 samples**. Each capture contains a forced thunderstorm and three future bell strokes computed before a live clock-rate change. Actual populations are 520 and 2,520; the extra population placed exactly 2,000 of 2,000 requested residents. All semantic counts and charges repeat within each mode.

The table reports the median of each mode's three per-run p99 values, using nearest-rank percentiles, in **microseconds**. It does not combine them into a whole-save latency.

| Phase | Authored p99 (µs) | +2,000 p99 (µs) |
|---|---:|---:|
| Preflight | 78.172 | 77.506 |
| Export | 147.116 | 147.210 |
| Encode | 10.616 | 10.830 |
| Decode and validate | 79.802 | 79.391 |
| Candidate validation | 69.220 | 69.695 |
| Dispose payload and candidate | 0.137 | 0.136 |

Every individual sample is below 0.153 ms; the largest is authored export at 152.234 µs. Each measured phase is below the unchanged 2 ms coordinator p99 budget for this workload. The forced timeline skips scheduled semantic-boundary/drying sampling, so this is not a universal climate bound. Other owners already exceed synchronous frame budgets, and no complete capture, hydration, host adoption, disk or renderer work is measured here.

Both modes encode **2,316 bytes**, with **31,314 expanded upper bytes**, a separate **65,536-byte sampling allowance**, and **201,836 bytes per retained cohort** under `4096 + 4*expanded + 3*encoded + 65536`. Standalone Save+Load reserves **403,672 bytes excluding Running**. Maximum process RSS is 28,908/67,216 KiB for authored/populated; this includes world preparation and the running Engine and is not an allocator proof. [Admission](../ADMISSION.md) gives the closed-layout and scratch argument. The earlier M2a2+Round standalone sum remains 1,256,093,444 bytes before this component or Running, so whole-envelope lifetime/phase coordination remains required.

[Raw results](climate/RESULTS.json), [summary](climate/SUMMARY.json) and [identity](climate/IDENTITY.json) retain all raw phase arrays in lossless gzip, original/archive hashes, exact commands, process metadata and machine/toolchain details. All **814 source/content hashes**, the executable and runner dependencies stayed unchanged throughout. The [independent audit](../coordinator/climate_performance_audit.json) recalculates every percentile/summary and checks counts, charges and hashes; [negative checks](../coordinator/auditor_regressions.json) reject altered p99, scratch charge and scenario state after the relevant metadata/hash updates. A separate [smoke audit](../coordinator/climate_smoke_audit.json) covers 24 samples; these are excluded from the final 3,600.

[Release build](../coordinator/release_build.json) succeeded in 58.624 seconds against the final 95-file component manifest. Binary SHA-256: `03e6ff384adc22e8959b00e118342dabc01929c3a47d6ffa56afd7a7089d2374`. An identical reference executable is preserved outside the repository at `/tmp/alibi-m2a4-climate-reference-binary`. Original build log/time bytes were copied before any terminal-LF normalization and independently audited. No ordinary Engine/weather logic changed, as the [source audit](../coordinator/source_audit.json) verifies, so the M2a3 ordinary-poll comparison remains the applicable historical measurement.

Reproduce after stopping builds and source changes with [run_m2_climate_probes.py](../../run_m2_climate_probes.py), using `--output-dir` pointing to an empty directory. Build `alibi_climate_cost` in release mode first; the archived build report records the exact offline cargo command/environment. Validate with [audit_m2_component_probes.py](../../audit_m2_component_probes.py) and `--check-current target/release/examples/alibi_climate_cost`.
