# M2a9 release measurements — 2026-09-08

The [runner](../../run_m2_night_probes.py) executes three 100-sample runs per mode with alternating order on one frozen release executable. All **3,600 phase samples** remain in lossless JSON gzip archives. [Results](RESULTS.json), [summary](SUMMARY.json) and [identity](IDENTITY.json) retain exact commands, counters, source/binary/runner hashes and process timings. The [independent audit](../coordinator/night_performance_audit.json) recomputes every percentile and summary, validates witnesses and byte charges, and checks current source/binary/runner hashes. The preceding [smoke audit](../coordinator/night_smoke_audit.json) covers 24 separate phase samples.

Values below are milliseconds, each the median of three per-run percentiles, not a pooled percentile. Each mode has 300 samples per phase. The authored cast contains 520 characters; the populated world contains 2,520 after all requested 2,000 production placements succeed.

| Phase | Authored p50 | Authored p99 | Populated p50 | Populated p99 |
|---|---:|---:|---:|---:|
| Preflight | 0.016747 | 0.025360 | 0.016579 | 0.018426 |
| Export | 0.033961 | 0.048061 | 0.033546 | 0.036078 |
| Encode | 0.021339 | 0.030381 | 0.021237 | 0.023548 |
| Decode + validate | 0.019511 | 0.033683 | 0.019228 | 0.026538 |
| Candidate validation | 0.000369 | 0.000774 | 0.000370 | 0.000447 |
| Drop | 0.000586 | 0.000983 | 0.000434 | 0.000611 |

The largest individual export is 0.138482 ms. This measures existing Night component calls, excluding complete Engine capture/hydration, disk, renderer, maximum queue/history occupancy and the complete host frame. Earlier components still require bounded offload or incremental host coordination.

Ordinary Engine polls select the same boundary in both modes at 20.200000000000003 seconds: six unadmitted queued ward reflections, one submitted reflection with its exact held successful reply, eight queue-time stamps, thirty resolved bedtimes and one previously settled ward mood. The first provider attempt was Busy; three attempts produced two submissions, and one reflection completed before the second was deferred by a full protected receipt window. The current held prompt has 4,987 bytes, its reply has 60, and the largest of the two submitted prompts has 5,301. The ledger has 4,096 recent entries, 256 retained entries and nineteen protected roots. Ambient work was observed; the separate private continuation test proves same-day suppression and next-day eligibility. No private Night mutation manufactures the measured boundary. Rare terminal errors, stale incarnations, mark/round effects, extreme clocks and maximum collection shapes are separate deterministic tests.

Both modes have identical charges:

| Admission quantity | Bytes |
|---|---:|
| Encoded J | 7,480 |
| Expanded upper E | 52,832 |
| Validation working | 4,194,304 |
| One cohort peak | 4,432,168 |
| Simultaneous Save + Load, excluding Running | 8,864,336 |

Charges are conservative admission bounds, not allocator measurements. The existing backbone+Round naive Save+Load total of 1,256,093,444 bytes already exceeds the unchanged 1 GiB shared cap before Running and the other owners. Complete integration must establish actual phase lifetimes.

The [release build](../coordinator/release_build.json) retains complete stdout/stderr and time logs with original/archive hashes. Executable SHA-256: `7923e9eca4e2d0f1e5a8e563a6d557035d59e834b7716e5a54ef6634d2b18ff3`. The final 852-file compiled source/assets manifest remained unchanged through the release build and post-measurement check. The measurement identity retains its inherited 876-path scope. A [scope reconciliation](../coordinator/release_archive_audit.json) verifies all 850 shared hashes, separately checks the two frozen-only lore/places JSON inputs and confirms the 26 additional text-fixture/document paths match both committed HEAD and the earlier M2a8 identity.

The shared auditor also passes the historical Round, climate, knowledge, law, marks and animals datasets. [Three negative checks](../coordinator/auditor_regressions.json) reject altered percentiles, a reduced validation allowance and loss of the held-result witness, even after raw hashes and metadata are updated consistently.
