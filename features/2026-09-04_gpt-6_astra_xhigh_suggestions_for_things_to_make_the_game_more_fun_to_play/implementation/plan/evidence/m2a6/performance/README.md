Status: Accepted M2a6 component measurements (2026-09-08); complete save/load and host acceptance remain pending.

# Law release measurements

Three alternating release runs per mode, 100 samples per phase per run, retain **3,600 raw phase samples**. [Identity](IDENTITY.json), [results](RESULTS.json) and [summary](SUMMARY.json) pin the exact binary, full source inventory, runners and machine. Every raw JSON is losslessly archived with its original hash. The [independent audit](../coordinator/law_performance_audit.json) recomputes all nearest-rank percentiles and medians, verifies semantic counts and costs, and checks current source/binary/runner hashes. The separate [24-sample smoke audit](../coordinator/law_smoke_audit.json) is not percentile evidence.

The scenario has 520/2,520 actual actors, four notices (three summoned, one warranted), six served pairs, eleven custody records (eight authored), one grip, one closing, and both an expired notice link and a departed committed officer. All runs retain identical semantic counts within each mode. The extra 2,000 residents were placed by the production population path.

Median of the three per-run p99 values, milliseconds:

| Phase | Authored | +2,000 residents |
|---|---:|---:|
| preflight | 0.291 | 1.156 |
| export | 1.063 | 4.490 |
| encode | 0.034 | 0.041 |
| decode validate | 0.294 | 1.154 |
| candidate validate | 0.257 | 1.119 |
| drop | 0.002 | 0.003 |

The largest export sample is 4.630 ms. Populated export exceeds the 2 ms host frame budget; component capture must run through bounded offload or incremental coordination during M2b/M3 integration. This bounded active-law workload is not maximum legal occupancy or a full-save latency guarantee. Ordinary law source is byte-identical after removing new module wiring, so no ordinary hot-path comparison was needed.

Payloads are 6,935/6,934 bytes; conservative expanded charge is 91,572 bytes and validation working charge is 4,194,304 bytes. Attached Save+Load reservations peak at 9,170,986/9,170,980 bytes, excluding Running. These are admission bounds, not allocator or complete-city memory measurements. Existing 1 GiB, 128 MiB and depth limits remain unchanged. The previously measured backbone+Round overlap already exceeds the global budget; complete integration must establish real phase lifetimes.

The [auditor regressions](../coordinator/auditor_regressions.json) reject an altered percentile, reduced working charge and removed historical-officer witness, with consistent tampered raw hashes/metadata for the latter cases. The updated auditor also accepts historical [Round](../coordinator/historical_round_audit.json), [climate](../coordinator/historical_climate_audit.json) and [knowledge](../coordinator/historical_knowledge_audit.json) archives without rerunning those measurements.

Release binary SHA-256: `0dcaebe4dd75acf8c00e09e8619ecbab370517021ba8f295617105e996c7a995`. The [release build](../coordinator/release_build.json) pins the final 136-file coordinator manifest and preserves a reference executable in `/tmp/alibi-m2a6-law-reference-binary`. No renderer, disk, host adoption or complete checkpoint behavior is measured here.
