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
