# M2a3 release measurements — 2026-09-08

Status: Accepted component measurements and ordinary CPU comparison. Complete-envelope and host acceptance remain pending.

The [release build](../coordinator/release_build.json) compiled `alibi_round_cost` and the unchanged `alibi_baseline` example together in 62.235 seconds. Source manifest SHA-256 is `3ade416b2fd037ad3aa6cedecbbf2f8086446e8d34d308c20fbae32b23a6c60b`. The Round executable is `08f5d2c0a37620cd779bc6b433afe58f40c01390f77e71dd24f14ac9a7a72e95`; the current ordinary executable is `fa1ab7d893bd303e07e735c5f927b3a5a1f61874141c9c5a25e86ffee7f6555f`.

## Round component

[IDENTITY](round/IDENTITY.json), [RESULTS](round/RESULTS.json) and [SUMMARY](round/SUMMARY.json) preserve six sequential release runs, alternating authored/populated order across three repetitions, with 100 samples per phase per run. There are **3,600 raw phase samples** in the lossless `.json.gz` files. [The independent audit](../coordinator/round_performance_audit.json) recomputes every percentile and median, checks original/archive hashes and metadata, and verifies current source, executable and runner hashes. Smoke checks used two samples per mode and are recorded separately; smoke results are not included in these statistics.

| Workload | Authored | +2,000 residents |
|---|---:|---:|
| World characters | 520 | 2,520 |
| Round people / residents | 514 / 0 | 2,514 / 2,000 |
| Occupied / destination reservations | 0 / 0 | 2,000 / 0 |
| Encoded bytes | 1,315,978 | 3,510,496 |
| Conservative expanded bytes | 24,203,156 | 51,657,412 |
| Definition scratch bytes | 4,194,304 | 4,194,304 |
| Retained peak per cohort | 104,958,958 | 221,359,536 |
| Shared Save+Load peak, excluding Running | 209,917,916 | 442,719,072 |

Both workloads retain nine water sources, 13 stalls, three production plans, six stock plans and two road parties. All requested residents are actually placed, and metadata repeats exactly in every run. Expanded/retained figures are conservative admission charges, not measured allocator usage. Maximum process RSS is 29,672/68,424 KiB; it includes generation and the running Engine.

Median per-run p99 costs, in milliseconds:

| Phase | Authored | +2,000 residents |
|---|---:|---:|
| Preflight | 5.534 | 11.903 |
| Export, including validation | 13.764 | 25.836 |
| Encode and DTO disposal | 6.476 | 15.438 |
| Decode and validation | 15.897 | 29.780 |
| Candidate validation/transfer | 7.955 | 11.925 |
| Payload and candidate disposal | 0.414 | 1.177 |

Populated maximum individual export/decode samples are 33.168/30.490 ms. Every phase except disposal exceeds the 2 ms coordinator p99 target in both workloads. This is a component API benchmark: it measures no full Engine capture/hydration, disk, host adoption or rendered frame. M2b/M3 must provide suitable staged/background execution and include the actual host-only work in later measurements.

The accepted M2a2 standalone Save+Load charge plus the populated Round charge is **1,256,093,444 bytes**, already above the unchanged 1 GiB shared ceiling and excluding Running/retiring authority. Separate component success does not establish complete-save admission. Composition requires an explicit allocation/ownership lifetime strategy; no limits are raised here.

## Ordinary simulation

The existing comparison runner retains its historical `m0`/`m1` labels. In this directory, **`m1` means the current M2a3 executable**, while `m0` is the preserved reference binary. [IDENTITY](ordinary/IDENTITY.json), [RESULTS](ordinary/RESULTS.json), [PAIRS](ordinary/PAIRS.json) and [SUMMARY](ordinary/SUMMARY.json) preserve **26 runs / 13 matched pairs / 30,800 raw samples**. The original harness, preserved M0 binary and all 540 authored content inputs remain unchanged. The [independent audit](../coordinator/ordinary_performance_audit.json) verifies raw percentiles/sums, pair deltas, all summary medians, process metadata, 802 current source/content hashes and both executable identities.

| Workload | M0 p95 / p99 ms | M2a3 p95 / p99 ms | Median paired p95 / p99 change |
|---|---:|---:|---:|
| Default idle | 1.509 / 2.438 | 1.515 / 2.389 | +0.45% / −2.03% |
| Default market | 1.662 / 2.600 | 1.590 / 2.524 | −3.95% / −3.52% |
| +1,000 market | 4.710 / 5.710 | 5.052 / 6.119 | +6.55% / +7.16% |
| +2,000 market | 7.894 / 9.134 | 8.258 / 9.464 | +4.56% / +4.03% |
| Capacity-limited stress, one pair | 43.895 / 48.486 | 41.124 / 44.479 | −6.31% / −8.26% |

Per-binary values are medians of each run's percentile; paired percentages are medians of within-pair changes, so they are deliberately not computed from the two displayed medians. Default/+2,000 p95 and p99 remain within the unchanged 10% comparison threshold. Repeated counters match exactly within each executable. Between M0 and current, only aggregate message counts differ, by the same values already recorded in M1d; the [historical comparison](../coordinator/semantic_comparison.json) verifies that no new counter difference appears here.

The +20,000 request again places only **9,072 residents**, giving **9,592 actors** and 10,928 unplaced requests. The one stress pair is not a repeated full-population or frame-rate acceptance result. Renderer/full 20,000-person stress availability remains as documented at M0; no availability probe is repeated. These ordinary-poll results do not measure complete save/load or the later active operation mix.
