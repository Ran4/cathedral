# M2a2 release component measurements — 2026-09-08

Status: Measured and independently checked. Component costs exceed synchronous host-frame limits; complete-world and host acceptance remain pending.

Six sequential release runs alternate authored and populated modes (three per mode), with 100 samples of each of six phases per run: **3,600 raw phase samples**. Actual production generation places all 2,000 requested additional residents. The Engine has navigation, Round and the item catalog, with cognition unavailable; it is sampled after its ordinary initial poll. These workloads contain no active offers or transforms. Rich active/completed state is covered by separate component continuation fixtures, not by this timing population.

The runner checks identical metadata across repeats and unchanged source, executable and runner dependencies before/after. The coordinator independently recomputed every nearest-rank percentile and median from the compressed raw JSON, verified its original-byte hashes and archive hashes, and matched all 789 source/content files. [Independent audit](../coordinator/performance_audit.json), [build command/log identity](../coordinator/release_build.json), [run identity](IDENTITY.json), [individual results](RESULTS.json) and [summary](SUMMARY.json) retain the evidence.

Release executable SHA-256: `08c39b6c673e4e2bbfb407b461fdf4ce961312a5419fd90c76ae9e2b9876b571`. The release build passed in 51.785 seconds. A two-sample-per-phase smoke run passed first; smoke timings are excluded from these figures.

## Payload, admission and process memory

| Mode | Actors / items / places | Encoded bytes | Expanded charge | Per-cohort peak charge | Shared save/load peak, excluding Running | Maximum process RSS (KiB) |
|---|---|---:|---:|---:|---:|---:|
| Authored | 520 / 638 / 491 | 1,919,795 | 19,349,968 | 83,163,353 | 166,326,706 | 30,640 |
| +2,000 residents | 2,520 / 2,638 / 1,992 | 9,118,806 | 94,831,668 | 406,687,186 | 813,374,372 | 78,216 |

Encoded component payloads fit the 64/128 MiB population ceilings. The expanded and peak figures are the conservative admission formula, **not measured allocator bytes**. The cohort keeps its peak reservation after encoding and through candidate disposal. Process RSS includes generation, catalogs, the running Engine, allocator retention and component work. These quantities cannot substitute for one another.

The shared 1 GiB gate remains unchanged. The populated probe reserves 813,374,372 bytes for save/load alone; complete-envelope plus Running and retiring-generation coexistence remains unproved. Adding remaining owners must preserve the global gate and account for actual retention, including any later safe phase shrinking.

## Phase CPU time

Values are **milliseconds**. p50/p95/p99 are medians of the three individual-run nearest-rank percentiles. Maximum is the largest sample across all three runs, not a median of maxima. No run or cold sample is discarded.

### Authored (520 actors)

| Phase | p50 | p95 | p99 | Maximum |
|---|---:|---:|---:|---:|
| Borrowed serialization preflight | 4.510 | 4.572 | 4.604 | 4.657 |
| Export (preflight + clone + validation) | 6.045 | 6.267 | 6.321 | 6.509 |
| Encode (preflight + write + DTO disposal) | 5.854 | 6.009 | 6.085 | 6.353 |
| Decode, build indexes and validate | 8.069 | 8.170 | 8.238 | 8.438 |
| Validate and transfer candidate maps | 0.832 | 0.861 | 0.893 | 0.926 |
| Dispose candidate and encoded bytes | 0.513 | 0.529 | 0.555 | 0.583 |

### Populated (2,520 actors)

| Phase | p50 | p95 | p99 | Maximum |
|---|---:|---:|---:|---:|
| Borrowed serialization preflight | 21.753 | 21.939 | 22.028 | 27.774 |
| Export (preflight + clone + validation) | 29.237 | 29.713 | 30.372 | 36.656 |
| Encode (preflight + write + DTO disposal) | 29.501 | 30.350 | 31.672 | 32.992 |
| Decode, build indexes and validate | 36.528 | 37.119 | 37.434 | 40.482 |
| Validate and transfer candidate maps | 3.664 | 3.911 | 4.027 | 4.383 |
| Dispose candidate and encoded bytes | 2.384 | 2.777 | 2.920 | 3.123 |

Preflight serializes the borrowed structure into a nonallocating meter. Export repeats that preflight, clones the covered authority and validates it. Encoding includes a counting pass, exact-capacity output and destruction of the consumed DTO. Decode includes typed parsing, construction of BTree/PlaceRegistry indexes and validation. Candidate timing revalidates and transfers those already-built maps; it does not reconstruct them a second time. Disposal includes both retained values and their cohort releases. Reservations are zero after every sample.

## Execution placement

The existing coordinator target is **2 ms p99 / 5 ms maximum per host frame**. Authored export/encoding/decode already exceed it; every measured populated phase exceeds the p99 target, including candidate validation and disposal. No synchronous save path is installed by this cut. M2b/M3 must move eligible work to workers, make remaining host-only work incremental and measure the real complete capture/staging/adoption/retirement path. A final pointer swap cannot conceal deferred destructor or copy-on-write cost.

These results do not accept complete Engine capture/hydration, effective calendar-rate CPU bounds, disk, host frames, rendering, the 20,000-resident stress workload or M13's mixed active-work fixture. Historical numerical targets and M0/M1/M2a1 reference evidence are unchanged.
