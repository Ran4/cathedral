All 7,200 measured phase samples are retained. The table pools the three 100-sample runs per owner and population.

| Owner | Population | Phase | Pooled p99 (ms) | Observed maximum (ms) |
|---|---|---|---:|---:|
| scheduler | authored | preflight | 0.041374 | 0.043603 |
| scheduler | authored | export | 0.041565 | 0.043208 |
| scheduler | authored | encode | 0.048632 | 0.052599 |
| scheduler | authored | decode_validate | 0.043421 | 0.051987 |
| scheduler | authored | candidate_validate | 0.003135 | 0.003497 |
| scheduler | authored | drop | 0.000163 | 0.000223 |
| scheduler | populated | preflight | 0.058050 | 0.077100 |
| scheduler | populated | export | 0.054141 | 0.061537 |
| scheduler | populated | encode | 0.049252 | 0.057808 |
| scheduler | populated | decode_validate | 0.057098 | 0.073894 |
| scheduler | populated | candidate_validate | 0.012323 | 0.020560 |
| scheduler | populated | drop | 0.000248 | 0.000469 |
| night | authored | preflight | 0.021630 | 0.023958 |
| night | authored | export | 0.023191 | 0.025267 |
| night | authored | encode | 0.021632 | 0.022556 |
| night | authored | decode_validate | 0.021913 | 0.024541 |
| night | authored | candidate_validate | 0.004652 | 0.005910 |
| night | authored | drop | 0.000133 | 0.000268 |
| night | populated | preflight | 0.032059 | 0.043014 |
| night | populated | export | 0.023130 | 0.024389 |
| night | populated | encode | 0.016006 | 0.039821 |
| night | populated | decode_validate | 0.029686 | 0.052997 |
| night | populated | candidate_validate | 0.011948 | 0.013584 |
| night | populated | drop | 0.000166 | 0.000298 |

0 phase samples exceeded 2 ms. Their owner, run, phase, sample index and duration are retained in [the tail audit](tail_latency_audit.json). These are component costs; complete host capture, worker coordination and frame budgets remain separate acceptance gates. Timing records alone do not establish the cause of individual long samples.
