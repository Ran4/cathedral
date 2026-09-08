# M2a12 continuity release measurements — 2026-09-09

Status: Component measurements accepted. Complete capture, hydration, speech interruption, files and host-frame acceptance remain pending.

Three interleaved 100-sample trials for each population retain all 3,600 phase timings. Each sample measures preflight, export, encode, decode/validation, candidate validation and drop. Engine construction, ordinary bounded setup, diagnostic witnesses and filesystem work are outside these component timings. Source, executable and runner identities are pinned at start and checked at end.

| Population | Characters | Awaited speech | Payload J (B) | Expanded E (B) | Component peak (B) | Save + Load peak (B) |
|---|---:|---:|---:|---:|---:|---:|
| authored | 520 | 1 | 719 | 12,264 | 55,309 | 110,618 |
| populated | 2,520 | 1 | 719 | 12,264 | 55,309 | 110,618 |

Ordinary typed speech and scripted provider/TTS values establish acknowledgements, a reply held through the conversation beat, accepted voiced speech, reading fallback after TTS refusal, a player sound and cooldown refusal, an active microphone hold, and Cloud-to-Local selection. The exact initial configuration remains distinct from current voice selection. No private Engine or World mutations manufacture the measured boundary; no audio service runs. Primary witnesses retain submitted input, TTS requests, all committed Speech publications and the exact boundary record. The full-publication FNV digest is diagnostic only.

| Population | Phase | Median per-run p50 (ms) | Median per-run p95 (ms) | Median per-run p99 (ms) |
|---|---|---:|---:|---:|
| authored | preflight | 0.002017 | 0.002198 | 0.002611 |
| authored | export | 0.002008 | 0.002175 | 0.002439 |
| authored | encode | 0.002498 | 0.002805 | 0.003348 |
| authored | decode_validate | 0.002191 | 0.002660 | 0.003799 |
| authored | candidate_validate | 0.000088 | 0.000133 | 0.000154 |
| authored | drop | 0.000069 | 0.000088 | 0.000114 |
| populated | preflight | 0.002010 | 0.002294 | 0.002574 |
| populated | export | 0.001982 | 0.002117 | 0.002409 |
| populated | encode | 0.002494 | 0.002798 | 0.003432 |
| populated | decode_validate | 0.002198 | 0.002649 | 0.003576 |
| populated | candidate_validate | 0.000068 | 0.000109 | 0.000136 |
| populated | drop | 0.000071 | 0.000091 | 0.000168 |

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

Validation uses borrowed player lookup and pairwise comparison of at most 32 awaited IDs, with no additional allocated index. The `4,096 + 4E + 3J` reservation is a conservative admission bound, not measured allocation. Variable parser/error strings remain charged to E/J. Process RSS includes the running Engine and setup. Existing 128 MiB E/J, depth 64 and shared 1 GiB ceilings are unchanged. Earlier naive backbone+Round Save+Load already exceeds 1 GiB before Running; complete integration must solve actual phase/lifetimes.

[Identity](IDENTITY.json), [results](RESULTS.json), [summary](SUMMARY.json), [independent audit](../coordinator/continuity_performance_audit.json) and [release provenance](../coordinator/release_archive_audit.json).
