# M2a1 component measurements — 2026-09-08

Six sequential release runs, three per mode, with 100 samples of each of five
phases for each owner: 6,000 raw phase samples. Modes alternate between repeats.
The real authored Engine contains 520 actor records, with navigation and external
services unavailable to isolate component work. Ordinary mode has empty replay
and operation owners; maximum mode admits 256 fixture operations through Engine
commands and fills the production ledger to 4,096 recent records, 256 retained
records and 256 protected roots. Full-length legal receipt text/reference fields
exercise retained memory. Counts and byte charges repeat exactly.

`IDENTITY.json` records compiler, CPU, source/content and executable hashes.
`RESULTS.json` preserves every run and process timing; `SUMMARY.json` gives median
per-run percentiles. Compressed JSON files preserve all raw samples. Source and
binary hashes matched before and after the run. The release binary SHA-256 is
`ec3b2960db4f406c1c13a147b8b001ccd0bc5f5f6e39d5be256eafc10c4bbdd8`.

## Maximum component costs

Times below are milliseconds, using the median of three per-run percentiles.
The last column is the largest individual sample across all three runs.

| Owner / phase | p50 | p95 | p99 | Largest sample |
|---|---:|---:|---:|---:|
| Ledger export | 6.256 | 6.546 | 7.186 | 8.048 |
| Ledger encode | 10.513 | 10.984 | 11.884 | 13.115 |
| Ledger decode / validate | 9.752 | 10.254 | 10.775 | 11.837 |
| Ledger index build / validate | 6.992 | 7.243 | 8.136 | 10.072 |
| Ledger disposal | 0.404 | 0.430 | 0.470 | 1.198 |
| Kernel export | 0.707 | 0.723 | 0.788 | 1.547 |
| Kernel encode | 0.749 | 0.764 | 0.774 | 1.344 |
| Kernel decode / validate | 0.675 | 0.691 | 0.709 | 1.103 |
| Kernel index build / validate | 0.448 | 0.463 | 0.486 | 0.733 |
| Kernel disposal | 0.028 | 0.030 | 0.031 | 0.040 |

Export includes owner validation, extraction and DTO validation. Encoding includes
validation, counting, exact-capacity writing and destruction of the consumed DTO.
Decode includes validation; kernel validation also constructs/disposes candidate
indexes. The separate index phase really constructs/disposes the restored maps,
with validation. These overlapping checks are actual API costs, not disjoint
pieces of an optimized final save pipeline. Disposal drops the decoded DTO and
encoded bytes, releasing both reservations.

Empty-owner median p99 costs are 3.717/7.039/7.505/2.859/0.365 microseconds for
ledger export/encode/decode/index/disposal, and
0.439/0.813/0.913/0.184/0.145 microseconds for the kernel. These are repeated small
component calls, not ordinary Engine poll measurements.

## Bytes and integration consequences

| Component | Encoded empty | Encoded maximum | Conservative retained heap maximum |
|---|---:|---:|---:|
| Ledger | 1,621 B | 3,111,542 B | 4,121,504 B for the recent tree |
| Kernel | 39 B | 169,034 B | 1,410,048 B for all kernel owners/indexes |

Ledger encoded data stays below 5 MiB and its recent-tree allocation bound below
4 MiB by 72,800 B. Retained receipt/root allocations are covered separately by the
16 MiB working allowance. Kernel encoded data stays below 1 MiB, with retained
heap below 2 MiB and an 8 MiB working allowance. Each probe deliberately reserves
40 MiB for save and 40 MiB for load, retaining 80 MiB of admission while both
component values exist; this excludes the running Engine. Every sample confirms
that both reservations release after disposal. Maximum measured process RSS is
28,280 KiB, including fixture construction and Engine state; it is not a measured
component allocation total and does not replace conservative admission.

**The saturated ledger APIs exceed the future host coordinator's 2 ms p99 / 5 ms
maximum frame budget.** M2b/M3 must move eligible work off the host frame or split
the remaining work, measuring extraction, staging, adoption, destruction and
later COW effects on the actual host. Synchronous placement of these calls on a
host frame would fail that gate. No runtime save path is installed by M2a1.

Complete-world extraction/encoding/hydration, population scaling, global retained
generations, disk IO, ordinary-pump changes and renderer/frame acceptance remain
pending. These component measurements do not accept those gates or the extreme
effective calendar rates still called out in the M2a1 handoff.

Reproduce after release compilation and all source edits stop:

```sh
PATH=/usr/bin:/bin:/home/ran/.local/bin uv run --no-project --cache-dir /tmp/alibi-uv python features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/run_m2_checkpoint_probes.py --output-dir /tmp/alibi-m2a1-repeat
```
