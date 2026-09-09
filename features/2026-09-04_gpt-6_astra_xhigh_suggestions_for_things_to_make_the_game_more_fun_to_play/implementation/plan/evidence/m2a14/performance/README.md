# M2a14 exact-input release measurements — 2026-09-09

Status: Component measurements accepted. Full capture, hydration, retry/adoption and host-frame acceptance remain pending.

Each owner uses three interleaved 100-sample trials per population. All 7,200 phase samples and 48 smoke phase samples are retained. Engine construction, ordinary bounded setup, diagnostic witnesses and filesystem work are outside these component timings. Full source, binary and runner identities are checked before and after the commands.

| Owner | Population | Actors | Payload J (B) | Expanded E (B) | Component peak (B) | Save + Load peak (B) |
|---|---|---:|---:|---:|---:|---:|
| scheduler | authored | 520 | 20,874 | 44,938 | 4,440,774 | 8,881,548 |
| scheduler | populated | 2,520 | 20,034 | 43,258 | 4,431,534 | 8,863,068 |
| night | authored | 520 | 5,395 | 14,512 | 4,272,633 | 8,545,266 |
| night | populated | 2,520 | 5,395 | 14,512 | 4,272,633 | 8,545,266 |

Scheduler setup retains three actual successful calls and one exact saved flight, with held success, protected player reaction, handoff and retry authority. Night retains two actual calls, a saved Wick ward flight, held success and a previously committed ward mood. The submitted request method, full prompt and budget are independently matched to the saved row and a pinned primary witness hash. Real content/navigation places all 2,000 extra actors. Ordinary setup uses at most 40 ms scheduler steps and nominal 50 ms Night steps; neither discards physical time. Defaults preserve the old component metadata exactly. No visible application, audio device or external provider runs.

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

0 phase samples exceeded 2 ms. Their owner, run, phase, sample index and duration are retained in [the tail audit](../coordinator/tail_latency_audit.json). These are component costs; complete host capture, worker coordination and frame budgets remain separate acceptance gates. Timing records alone do not establish the cause of individual long samples.

Admission keeps 4 MiB of sequential borrowed old-owner validation scratch. The reservation is a conservative bound, not a measured allocator peak. Existing 128 MiB encoded/expanded, depth 64 and shared 1 GiB caps remain unchanged. The earlier naive backbone+Round Save+Load peak of 1,256,093,444 bytes exceeds 1 GiB before Running; full integration must solve actual phases and lifetimes.

[Scheduler identity](scheduler/IDENTITY.json), [scheduler results](scheduler/RESULTS.json), [scheduler summary](scheduler/SUMMARY.json), [Night identity](night/IDENTITY.json), [Night results](night/RESULTS.json), [Night summary](night/SUMMARY.json), [release provenance](../coordinator/release_archive_audit.json).
