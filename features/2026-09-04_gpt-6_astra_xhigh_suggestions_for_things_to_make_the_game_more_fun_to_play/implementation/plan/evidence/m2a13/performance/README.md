# M2a13 speech release measurements — 2026-09-09

Status: Component measurements accepted. Complete capture, hydration, interruption adoption, files and host-frame acceptance remain pending.

Three interleaved 100-sample trials for each population retain all 3,600 phase timings. Each sample measures preflight, export, encode, decode/validation, candidate validation and drop. Engine construction, ordinary bounded setup, diagnostic witnesses and filesystem work are outside these component timings. Source, executable and runner identities are pinned at start and checked at end.

| Population | Characters | Accepted recordings | Available drafts | Payload J (B) | Expanded E (B) | Component peak (B) | Save + Load peak (B) |
|---|---:|---:|---:|---:|---:|---:|---:|
| authored | 520 | 3 | 1 | 1,824 | 31,756 | 4,330,896 | 8,661,792 |
| populated | 2,520 | 3 | 1 | 1,815 | 31,756 | 4,330,869 | 8,661,738 |

Ordinary typed speech and scripted provider/TTS/STT values establish two committed Speech publications, microphone onset, a completed unsubmitted 85-byte draft, an active stream, two batch jobs and one parked recording. Sibling command steps share a root and filename while remaining distinct obligations. All 17 setup polls are at most 0.04 seconds, with no discarded physical time. Real content/navigation and successful +2,000 placement are used. No private Engine or World mutations manufacture the timed boundary; no audio device or external provider runs. Primary witnesses retain submitted input, prompts and output budgets, TTS/STT requests, all committed Speech and exact boundary records. The full-publication FNV digest is diagnostic only.

| Population | Phase | Median per-run p50 (ms) | Median per-run p95 (ms) | Median per-run p99 (ms) |
|---|---|---:|---:|---:|
| authored | preflight | 0.006161 | 0.006611 | 0.008410 |
| authored | export | 0.006601 | 0.007113 | 0.007553 |
| authored | encode | 0.010452 | 0.011495 | 0.014870 |
| authored | decode_validate | 0.007040 | 0.008196 | 0.010076 |
| authored | candidate_validate | 0.000543 | 0.000601 | 0.000720 |
| authored | drop | 0.000183 | 0.000253 | 0.000336 |
| populated | preflight | 0.006032 | 0.006380 | 0.007978 |
| populated | export | 0.006638 | 0.006851 | 0.008320 |
| populated | encode | 0.008890 | 0.010712 | 0.013726 |
| populated | decode_validate | 0.006371 | 0.007168 | 0.009071 |
| populated | candidate_validate | 0.000517 | 0.000573 | 0.000607 |
| populated | drop | 0.000176 | 0.000263 | 0.000388 |

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

Validation reserves 4 MiB for bounded saved-ledger sets; subsequent at-most-eight-row scans borrow records and allocate no index. The `4,096 + 4E + 3J + 4,194,304` reservation is a conservative admission bound, not measured allocation. Variable parser/error strings remain charged to E/J. Process RSS includes the running Engine and setup. Existing 128 MiB E/J, depth 64 and shared 1 GiB ceilings are unchanged. Earlier naive backbone+Round Save+Load already exceeds 1 GiB before Running; complete integration must solve actual phase/lifetimes.

[Identity](IDENTITY.json), [results](RESULTS.json), [summary](SUMMARY.json), [independent audit](../coordinator/speech_performance_audit.json) and [release provenance](../coordinator/release_archive_audit.json).
