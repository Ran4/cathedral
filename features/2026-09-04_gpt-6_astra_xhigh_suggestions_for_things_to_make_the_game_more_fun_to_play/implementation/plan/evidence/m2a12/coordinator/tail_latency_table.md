All 3,600 measured phase samples are retained. The table below pools the three 100-sample runs per population, rather than taking a median of per-run percentiles.

| Population | Phase | Pooled p99 (ms) | Observed maximum (ms) |
|---|---|---:|---:|
| authored | preflight | 0.003701 | 0.007873 |
| authored | export | 0.002510 | 0.003357 |
| authored | encode | 0.005192 | 0.006105 |
| authored | decode_validate | 0.004528 | 0.009323 |
| authored | candidate_validate | 0.000160 | 0.000275 |
| authored | drop | 0.000140 | 0.000359 |
| populated | preflight | 0.002749 | 0.008608 |
| populated | export | 0.003217 | 0.003654 |
| populated | encode | 0.004026 | 0.006581 |
| populated | decode_validate | 0.004352 | 0.011820 |
| populated | candidate_validate | 0.000144 | 0.000402 |
| populated | drop | 0.000198 | 0.000318 |

0 phase samples exceeded 2 ms. Their exact run, phase, sample index and duration are retained in [the tail audit](../coordinator/tail_latency_audit.json). These measurements establish component costs only; the complete host capture, worker coordination and frame budget remain separate acceptance gates. Timing records alone do not establish the cause of individual long samples.
