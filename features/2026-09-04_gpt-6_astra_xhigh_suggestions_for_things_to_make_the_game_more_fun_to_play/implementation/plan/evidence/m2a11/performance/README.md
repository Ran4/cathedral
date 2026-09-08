# M2a11 scheduler release measurements — 2026-09-08

Status: Component measurements accepted. Complete Engine capture, hydration, load-specific retry, files and host-frame acceptance remain pending.

The frozen release binary ran three interleaved 100-sample trials for each population. Each sample records preflight, export, encode, decode/validation, candidate validation and drop. All 3,600 phase timings are retained with exact raw archives, process timing and source/binary/runner identity. World generation, the 136 ordinary setup polls, publication diagnostics and filesystem writes are outside these component timings.

| Population | Characters | Weighted slots | Payload J (B) | Expanded E (B) | Component peak (B) | Save + Load peak (B) |
|---|---:|---:|---:|---:|---:|---:|
| authored | 520 | 719 | 27,444 | 111,932 | 4,728,460 | 9,456,920 |
| populated | 2,520 | 2,719 | 44,596 | 266,236 | 5,397,132 | 10,794,264 |

Both measured states retain a protected flight with its exact 59-byte held success, a later protected follow-up for that actor, one ordinary handoff, one failed semantic obligation and provider failure count one. The weighted cursor is zero because setup uses event-driven turns; private tests separately verify weighted fairness. The probe records all three original provider prompts and output budgets, separates later input from the submitted prompt, and uses at most 40 ms ordinary steps with zero coarse-discard diagnostics. A full publication FNV digest is diagnostic; the independently checked SHA-256 binds the entire primary witness JSON.

| Population | Phase | Median per-run p50 (ms) | Median per-run p95 (ms) | Median per-run p99 (ms) |
|---|---|---:|---:|---:|
| authored | preflight | 0.057785 | 0.060037 | 0.062352 |
| authored | export | 0.071738 | 0.075003 | 0.076638 |
| authored | encode | 0.075832 | 0.078747 | 0.083731 |
| authored | decode_validate | 0.065444 | 0.070759 | 0.080721 |
| authored | candidate_validate | 0.002499 | 0.002602 | 0.002668 |
| authored | drop | 0.003021 | 0.003260 | 0.003962 |
| populated | preflight | 0.111286 | 0.115746 | 0.120989 |
| populated | export | 0.160375 | 0.164533 | 0.167253 |
| populated | encode | 0.147223 | 0.152400 | 0.159175 |
| populated | decode_validate | 0.144913 | 0.153655 | 0.158934 |
| populated | candidate_validate | 0.009800 | 0.009975 | 0.011498 |
| populated | drop | 0.022390 | 0.023478 | 0.024506 |

All 3,600 measured phase samples are retained. The table below pools the three 100-sample runs per population, rather than taking a median of per-run percentiles.

| Population | Phase | Pooled p99 (ms) | Observed maximum (ms) |
|---|---|---:|---:|
| authored | preflight | 0.068592 | 0.071478 |
| authored | export | 0.077532 | 0.093167 |
| authored | encode | 0.086154 | 0.100251 |
| authored | decode_validate | 0.080721 | 0.096108 |
| authored | candidate_validate | 0.003197 | 0.004859 |
| authored | drop | 0.004109 | 0.006759 |
| populated | preflight | 0.132861 | 0.137459 |
| populated | export | 0.186618 | 0.209647 |
| populated | encode | 0.175042 | 0.410428 |
| populated | decode_validate | 0.169326 | 0.173254 |
| populated | candidate_validate | 0.013225 | 0.018644 |
| populated | drop | 0.026205 | 0.027583 |

0 phase samples exceeded 2 ms. Their exact run, phase, sample index and duration are retained in [the tail audit](../coordinator/tail_latency_audit.json). These measurements establish component costs only; the complete host capture, worker coordination and frame budget remain separate acceptance gates. Timing records alone do not establish the cause of individual long samples.

The 4 MiB validation allowance and `4,096 + 4E + 3J` aggregate charge are conservative admission bounds, not measured allocations. Process RSS includes the running Engine and setup. The existing 128 MiB E/J and shared 1 GiB ceilings are unchanged. Naive backbone+Round Save+Load already exceeds 1 GiB before Running; full composition needs an actual phase/lifetime solution.

[Source and executable identity](IDENTITY.json), [results](RESULTS.json), [summary](SUMMARY.json), [independent audit](../coordinator/scheduler_performance_audit.json) and [release archive audit](../coordinator/release_archive_audit.json).
