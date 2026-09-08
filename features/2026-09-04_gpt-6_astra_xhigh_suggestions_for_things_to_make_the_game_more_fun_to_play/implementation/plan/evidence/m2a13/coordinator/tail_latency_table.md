All 3,600 measured phase samples are retained. The table below pools the three 100-sample runs per population, rather than taking a median of per-run percentiles.

| Population | Phase | Pooled p99 (ms) | Observed maximum (ms) |
|---|---|---:|---:|
| authored | preflight | 0.009749 | 0.016112 |
| authored | export | 0.009243 | 0.010273 |
| authored | encode | 0.015101 | 0.016336 |
| authored | decode_validate | 0.013111 | 0.018397 |
| authored | candidate_validate | 0.000816 | 0.000989 |
| authored | drop | 0.000401 | 0.001008 |
| populated | preflight | 0.007984 | 0.018357 |
| populated | export | 0.008516 | 0.009428 |
| populated | encode | 0.013726 | 0.015517 |
| populated | decode_validate | 0.011356 | 0.019817 |
| populated | candidate_validate | 0.000644 | 0.000975 |
| populated | drop | 0.000389 | 0.000758 |

0 phase samples exceeded 2 ms. Their exact run, phase, sample index and duration are retained in [the tail audit](../coordinator/tail_latency_audit.json). These measurements establish component costs only; the complete host capture, worker coordination and frame budget remain separate acceptance gates. Timing records alone do not establish the cause of individual long samples.
