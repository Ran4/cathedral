# M2a10 release measurements — 2026-09-08

The [runner](../../run_m2_social_probes.py) executes three 100-sample runs per mode in alternating order on one frozen release executable. All **3,600 phase samples** remain in lossless JSON gzip archives. [Results](RESULTS.json), [summary](SUMMARY.json) and [identity](IDENTITY.json) retain commands, exact counters and witnesses, source/binary/runner hashes and process timings. The [independent audit](../coordinator/social_performance_audit.json) recomputes every percentile and summary, validates all semantic and admission gates, and checks current source/binary/runner identity. The separate [smoke audit](../coordinator/social_smoke_audit.json) covers 24 phase samples.

Each value is milliseconds, the median of three per-run percentiles, not a pooled percentile. Each mode has 300 samples per phase.

| Phase | Authored p50 | Authored p99 | Populated p50 | Populated p99 |
|---|---:|---:|---:|---:|
| Preflight | 0.002321 | 0.006842 | 0.003137 | 0.006910 |
| Export | 0.002493 | 0.004127 | 0.003392 | 0.005404 |
| Encode | 0.003004 | 0.004265 | 0.004161 | 0.006646 |
| Decode + validate | 0.002734 | 0.005043 | 0.003638 | 0.006624 |
| Candidate validation | 0.000149 | 0.000208 | 0.000186 | 0.000251 |
| Drop | 0.000156 | 0.000308 | 0.000215 | 0.000340 |

The median table hides a slow third populated run. The [tail audit](../coordinator/tail_latency_audit.json) retains and identifies all nineteen phase samples above 2 ms, all in `populated-3`; four are exports. The recorded timings do not identify the cause. No run was discarded or repeated. Pooling the 300 populated samples per phase gives export p99 **3.577735 ms** and decode p99 **11.295041 ms**. This component also requires safe host scheduling; low median per-run figures do not establish a frame bound.

| Run | Export p99 ms | Export max ms | Decode p99 ms | Decode max ms |
|---|---:|---:|---:|---:|
| authored-1 | 0.002757 | 0.005418 | 0.004155 | 0.015361 |
| populated-1 | 0.005404 | 0.005656 | 0.006624 | 0.480466 |
| populated-2 | 0.005333 | 0.007901 | 0.005817 | 0.568780 |
| authored-2 | 0.004127 | 0.005051 | 0.007412 | 1.678592 |
| authored-3 | 0.005874 | 0.006306 | 0.005043 | 0.363732 |
| populated-3 | 5.081690 | 13.120826 | 46.903921 | 55.358842 |

The largest individual export is 13.120826 ms. These are social component API costs, excluding complete Engine capture/hydration, host adoption, disk, rendering and maximum supported collection occupancy. Complete host integration still needs offload or incremental coordination for these measured tails and the earlier components.

The authored world has 520 characters; all requested 2,000 production placements succeed in the 2,520-character populated world. The probe selects real settled bodies from each loaded population. Ordinary named player speech submits one prompt; a scripted successful reply addresses the player and a nearby peer, and a later gaze sample leaves partial focus at the 0.3-second boundary. Both modes retain reciprocal engagement, an independent invitation, focus, one warm NPC pair and one submitted Novelty context. Authored/populated state retains three/six prior witnesses and two/six Novelty memories. The populated peer is a generated resident. Exact utterances, all three emitted speech events and recipient lists, positions, selected actor and the 19,849/19,029-byte submitted prompt lengths repeat. Canonical witness digests in the runner/auditor pin the complete reviewed functional-smoke records.

| Admission quantity | Authored bytes | Populated bytes |
|---|---:|---:|
| Encoded J | 695 | 937 |
| Expanded upper E | 14,296 | 18,632 |
| Fixed validation working | 65,536 | 65,536 |
| One cohort peak | 128,901 | 146,971 |
| Save + Load, excluding Running | 257,802 | 293,942 |

Charges are conservative admission bounds, not allocator measurements. The [admission proof](../ADMISSION.md) covers private layouts and malformed aggregate-bounded shapes separately. The prior naive backbone+Round Save+Load sum of 1,256,093,444 B still exceeds the unchanged 1 GiB shared cap before Running and other owners; full integration must resolve phase lifetimes.

The [release build](../coordinator/release_build.json) retains exact original stdout/stderr and timing bytes in gzip archives. Executable SHA-256: `35dd91998a907fcfe4399ba1db0a9d1487f77a7d77063d842782c33a6dca1d4c`. The shared [input enumerator](../../component_input_sources.py) unifies prior owner and measurement scopes and includes `default_config.ron`; the same 895-path map is captured at command start, checked before/after release build and measurement, and independently verified in the [release archive audit](../coordinator/release_archive_audit.json). Historical records retain their original source scopes.

All seven historical component datasets pass the amended auditor. [Four corrupted-data checks](../coordinator/auditor_regressions.json) fail at the intended gates: altered percentile, reduced working charge, loss of the retained partner, and omission of runtime defaults from the source identity. Updating raw hashes and metadata cannot conceal the semantic or charge changes.
