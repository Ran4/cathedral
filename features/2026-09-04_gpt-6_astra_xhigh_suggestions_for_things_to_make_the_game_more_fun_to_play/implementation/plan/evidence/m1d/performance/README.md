# M1 CPU measurements — 2026-09-08

The ordinary Engine workload passes the relative and absolute CPU limits fixed in [RUNTIME_BUDGETS](../../../RUNTIME_BUDGETS.md). The separate timed-fixture probe fits the provisional active-work CPU ceiling. Renderer/frame/focus acceptance and the full M13 adapter mix remain unmeasured.

All runs use release binaries on the recorded Intel i5-13600KF/Linux/rustc 1.96.0 environment. Builds and measurements ran sequentially. Source/content and executable hashes were unchanged throughout both measurement sets. Per-poll samples are elapsed durations of synchronous Engine calls; process CPU and peak RSS are separately recorded by `/usr/bin/time -v`. Startup is outside the per-poll samples. Every raw measured run is retained.

## Ordinary city: preserved M0 versus M1

[The comparison runner](../../run_m1_comparison.py) uses the preserved M0 executable with SHA-256 `f7ba972a440a2d6d2690c4dd2afd230363e5778ebf2c3fe7d62d4becca3e9057` and the current build of the **unchanged** original `alibi_baseline.rs`. All original asset/lore hashes match. The fixture retains Stage, fake cognition, Dayspring/day 2, 3,600 seconds/day and 50 ms supplied polls. Market input is the original periodic typed question.

Each ordinary workload has three balanced sequential pairs, alternating M0/M1 order. Runs warm for 100 polls and measure 1,200 polls (5 + 60 simulated seconds). Stress has one pair, with 1,000 measured polls. There are **26 runs and 30,800 raw samples**. The table reports median per-run percentiles; paired changes are the median of paired ratios, not a ratio of these table medians.

| Workload | M1 p95 ms | M1 p99 ms | Paired p95 change | Paired p99 change |
|---|---:|---:|---:|---:|
| Authored, idle | 1.535 | 2.430 | −1.57% | −1.82% |
| Authored, market | 1.576 | 2.542 | −1.27% | −1.11% |
| +1,000 residents, market | 4.858 | 5.902 | +2.95% | +2.65% |
| +2,000 residents, market | 8.530 | 9.982 | −0.02% | +0.96% |
| +20,000 requested, **9,072 placed** | 42.094 | 47.424 | −1.83% | +3.15% |

Ordinary workloads satisfy both the ≤10% relative limit and their preset absolute p95/p99 envelopes. Variation within this range is not evidence of a speed improvement. Stress remains capacity limited and too slow for ordinary frame-rate acceptance; one pair does not establish repeatability for each stress executable.

All repeated runs of each executable have identical recorded semantic counters. Across M0/M1, actor counts, actual placement, displacement, speech, snapshot counts/bytes, knowledge bytes and cognition/prompt counters match. The only recorded difference is message count: M1 adds 50/52/66 messages for the authored/+1,000/+2,000 market workloads and 47 in stress; idle matches. This is consistent with the added action-receipt publication, without claiming the harness classifies every extra message.

[Identity](ordinary/IDENTITY.json), [all run summaries](ordinary/RESULTS.json), [paired deltas](ordinary/PAIRS.json) and [aggregate statistics](ordinary/SUMMARY.json) accompany the named `.json.gz`, `.time` and `.stderr` files.

## Active operations at 60 Hz

[The probe runner](../../run_m1_probes.py) compares the same city/configuration with zero or 16 active stationary fixtures and unavailable cognition. Both declare the same 64 candidate resources. Admission chooses the first 16 valid candidates; one out-of-range candidate is explicitly refused. The added [poll-duration option](../post_workspace_probe_change.json) allows **1/60-second polls**, so shared need refresh is measured between the Round's 20 Hz ticks. Each run warms for 300 polls and measures 1,200: 5 + 20 simulated seconds. There are three alternating pairs at authored count and +2,000 residents.

| Population | Active-16 p95 / p99 ms | Median paired added p95 / p99 ms | Median paired added p50 ms |
|---|---:|---:|---:|
| Authored | 1.041 / 2.539 | 0.029 / 0.049 | 0.084 |
| +2,000 placed | 8.225 / 10.081 | 0.462 / 0.431 | 0.725 |

Every measured active sample retains exactly 16 operations; no fixture silently finishes or interrupts during measurement. Recorded counters repeat across all three pairs. Fixture ownership changes actual activity: 473 actors move more than 0.01 m versus 488 in the empty control. The conservative retained kernel bound rises from 92,204 to 154,312 bytes, including identical declared fixtures in both. These are whole-Engine measurements with the intended duty/needs behavior change, not isolated instruction costs. The 25-second segment ends before generated residents' minimum dwell expires and does not cover a full day.

The +2,000 fixture probe stays below the provisional added p95/p99 ceiling of 2/3 ms and total 12/16 ms. **This does not accept M13's eight-travel/four-work/four-contention, appointment, order and observation mix.** A separate 20-poll short-work trace records 16 Accepted, 16 InProgress and 16 Completed transitions and 16 actual counter increments; it is functional evidence, not a steady-state performance result.

## Publication allocation accounting

The publication example includes the production `publication_bytes.rs` implementation directly. It takes a real snapshot after five simulated seconds, warms 1,000 traversals, then measures 5,000 traversals per run, with three runs per population.

| Extra residents | Snapshot actor records | JSON bytes | Conservative charged bytes | Hot accounting p95 / p99 µs |
|---|---:|---:|---:|---:|
| 0 | 518 | 231,570 | 1,198,767 | 2.230 / 2.447 |
| 2,000 | 2,518 | 1,732,413 | 5,020,594 | 10.171 / 14.713 |

This measures **hot repeated allocation-accounting traversal only**. Snapshot construction/encoding, queue atomics, downstream consumption and rendering are excluded. JSON payload bytes and conservative retained-allocation charges are different quantities. These five-second snapshots are also distinct from the original smaller snapshot canary and the M0 final 65-second snapshots.

[Probe identity](kernel-and-publication/IDENTITY.json), [run summaries](kernel-and-publication/RESULTS.json) and [aggregate statistics](kernel-and-publication/SUMMARY.json) accompany all raw files. This set has **19 runs / 44,420 samples**: 30,000 publication traversals, 14,400 steady operation polls and the 20-poll functional trace.

## Reproduction and review

From the repository root, build the three examples with `cargo build --release -p cathedral-backends --example alibi_baseline --example alibi_operation_cost --example alibi_publication_cost`, then run the two linked Python scripts with `uv run --no-project ... --output-dir <empty-directory>`, sequentially and without competing compilation. Both runners offer `--smoke`; both smoke checks passed before these full runs. Reproducing the exact comparison requires the preserved M0 executable identified above.

[The release build log](release-build.log), [tested source manifest](../source_hashes.json), [sole later example change](../post_workspace_probe_change.json) and [coordinator checks](../coordinator-review.json) preserve provenance. The coordinator independently rehashed source, verified all archived test-log bytes and per-target totals, recomputed every raw run's sample count/percentiles and checked the fixed CPU limits. No production source changed after the 1,865-test workspace pass.
