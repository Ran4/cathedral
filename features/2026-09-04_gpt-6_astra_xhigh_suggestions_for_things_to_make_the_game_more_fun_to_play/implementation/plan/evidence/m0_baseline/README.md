# M0 current-source baseline — 2026-09-07

V01/V02 source reconciliation is in [BASELINE_RECONCILIATION](../../BASELINE_RECONCILIATION.md). V03 is **partial**: the CPU fixture runs; a current renderer/frame trace and full 20,000-person stress population do not. No M1 gameplay implementation or performance improvement is asserted.

## Reproduce the measured workload

From the repository root, sequentially:

```sh
cargo build --release -p cathedral-backends --example alibi_baseline
uv run --cache-dir /tmp/alibi-m0-uv features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/run_baseline.py --output-dir /tmp/alibi-m0-new-run
```

The output directory must not already contain results. `--smoke` uses a fresh `/tmp` directory by default, so it cannot overwrite acceptance evidence. The runner never calls providers or edits gameplay configuration. Build first; do not compete with another build/app/benchmark while measuring. It checks unchanged source and binary identity at the end and rejects paired semantic counter differences.

The Rust example loads the shipped cast, nav, shelters, prompt/sound assets and ordinary Engine/Round services. It uses production `generate_ambient` with authored occupied positions, first index 0 and no worker overrides. It enables Stage, novelty, curiosity, knowledge, weather and Night Office; TTS/STT/Sight are null. Clock is Dayspring, day 2, 3,600 s/day. The far-corner idle fixture has no conversation. The market fixture holds the player at `[-21, 0.91, 252]` and submits the same typed group well question every 10 seconds; fake completions return through the normal command path.

Each nominal poll increment is 0.05 s. Ordinary runs warm for 100 polls/5 s and measure 1,200 polls/60 s; stress warms for 100 and measures 1,000/50 s. There are three consecutive A/B pairs per ordinary workload and one stress pair. This longer segment lets generated residents begin local movement after the 45-second minimum dwell. The player does not walk a collision-controlled route in this fixture. Physical NPC movement, ordinary duties and speech actually run, but no single segment crosses an office/night boundary or exercises every need, queue or weather state.

## Measured result

The table uses the **median of per-run nearest-rank percentiles**, in milliseconds. Max is the worst individual measured poll across that workload; RSS is the largest process peak reported by `/usr/bin/time -v`, including startup. Per-poll values are elapsed CPU execution time in one pure synchronous engine, not GPU/frame time. Process user/system CPU is recorded separately, not mislabelled as per-poll CPU counters.

| Workload | Runs | p50 ms | p95 ms | p99 ms | Max ms | Peak RSS MiB |
|---|---:|---:|---:|---:|---:|---:|
| Authored cast, idle | 6 | 0.657 | 1.449 | 2.354 | 3.462 | 28.3 |
| Authored cast, market conversation | 6 | 0.677 | 1.498 | 2.452 | 3.449 | 30.3 |
| +1,000 requested/placed, current config count | 6 | 3.622 | 4.573 | 5.568 | 6.601 | 45.5 |
| +2,000 requested/placed | 6 | 7.061 | 8.160 | 9.242 | 12.463 | 64.3 |
| +20,000 requested, **9,072 placed** | 2 | 38.313 | 41.203 | 43.530 | 53.188 | 197.5 |

All 13 pairs have equal actor/placement/displacement/message/speech/publication/prompt/byte counters. Same-binary A/B differences reached 8.00% at p95 and 6.56% at p99 (max-poll variability 18.94%); these are measurement variation, not improvements. Ordinary runs move 491/604/729 actors by >0.1 m at 0/1,000/2,000 extras and deliver six speech events. Stress moves 505 by the shorter end instant and delivers five; occupancy caps admit 6,968 housed + 2,104 hardship residents and leave 10,928 unplaced. No unsupported workers are synthesized.

The production stress request therefore measures **9,592 total actor records**, not 20,520. Its p95 pump already exceeds a 16.67 ms whole-frame target, and its maximum exceeds one 50 ms supplied step. The direct harness finishes bounded work without wall-time debt, so this does not measure production overload recovery. Increasing a count in the command does not certify a full 20,000-resident workload.

Final public snapshot byte sizes are 234,523 idle / 234,517 market at authored count; 986,080 with 1,000 extra; 1,737,971 with 2,000; 7,048,132 stress. Maximum sampled private knowledge+auxiliary bytes are 13,266 / 49,184 / 78,184 / 275,187 across those counts; these are the module's size estimate, **not whole-engine heap/checkpoint bytes**. Longest rendered prompt ranges 21,662–22,525 bytes in market workloads. Idle submits zero prompts.

The established 160 KiB snapshot canary is a different, smaller authored fixture. This nav/round-populated world's existing snapshot already exceeds it. Keep the old canary and treat the expanded-world byte figures separately; no cap has been silently enlarged. The knowledge feature's 32 MiB saturated-store test measures another workload, not the tiny hot store in these ordinary runs.

## Evidence identity and scope

- [IDENTITY.json](IDENTITY.json): `46e24abf429b3113d3c5745324e5746d82345140`, tracked/untracked source state, individual source/content hashes, measured binary and runner hashes, release build/environment, rustc 1.96.0, Linux x86_64, Intel i5-13600KF, 20 logical CPUs, 32,667,680 KiB host RAM. Exposed CPU affinity is 0–19; cgroup CPU/memory maximum files were unavailable, so host RAM is not a verified container limit.
- [RESULTS.json](RESULTS.json), [PAIRS.json](PAIRS.json): commands, all run counters/percentiles, process CPU/RSS and paired differences. Each named `.json.gz` contains raw microsecond samples; each `.time` contains the corresponding unedited process timing. Serialization and sampled knowledge-size probes occur outside measured poll spans. Startup time is separately recorded; overall process timing includes it.
- [sim_tests.log](sim_tests.log): the current deterministic baseline command's RTK summary, 1,063 passed/one ignored, 21 suites. It is a summary, not a full per-test transcript. [build.log](build.log) records the successful release example build.
- [post_measurement_changes.json](post_measurement_changes.json), [host_focused.log](host_focused.log): the subsequent baseline repair adds the skin asset only to the shared host test helper; 29 focused tests pass. This intentional test-only source delta postdates the CPU identity record and does not change the measured backend binary. Root’s complete workspace review is recorded separately.
- [review_checks.json](review_checks.json): independent raw/sample/percentile/counter/source checks by the coordinator, before that test-only repair. All 30,800 raw samples and 13 pairs reproduce their summaries.
- [workspace_gate.json](workspace_gate.json): final complete workspace rerun after the test-fixture repair, 1,780 passed, zero failed, eight ignored; compressed initial/focused-failure/final transcripts are linked in that record. The [coordinator review](../../BASELINE_REVIEW.md) records acceptance boundaries.
- [BUDGETS.json](BUDGETS.json): machine-readable numerical targets and explicit unmeasured branches; these do not report absent services as measured.
- [owner_fields.json](owner_fields.json): 26 selected private owners / 323 field declarations, checked by `../inventory_fields.py`; the policy table lives in PERSISTENCE_INVENTORY.
- [renderer_availability.json](renderer_availability.json): no display variables or GPU devices; NVIDIA query exits 9. The approved outside-sandbox check found the same state. No new Bevy run or screenshot was attempted; no current frame/VSync/focus evidence exists.

An earlier short measurement used the compatibility crowd helper without authored spawn exclusions and ended before resident dwell expired. It was rejected in review and moved outside the feature evidence; none of its numbers above survives. The final direct production generator run uses a newly built binary and longer declared segments.

Missing evidence remains material: real-renderer/ordinary-host frames and accepted-time recovery; actual full 20,000 extra residents; full-day physical-time behavior; active future operations; checkpoint/replay/archive DTO allocation and concurrent generation residency. Fake dialogue and archived provider probes are not human comprehension/play evidence. [RUNTIME_BUDGETS](../../RUNTIME_BUDGETS.md) distinguishes numerical limits from these unmeasured achievements.
