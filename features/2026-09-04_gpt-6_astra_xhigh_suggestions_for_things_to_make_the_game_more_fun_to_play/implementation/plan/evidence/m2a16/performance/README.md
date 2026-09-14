# M2a16 complete release measurements — 2026-09-15

Six fresh serial processes, 100 repeated complete save/load samples per process,
measure the actual CityPlugin authored 520 and populated 2520 host boundaries.
All 2,000 requested additions were placed. The separate two-sample smoke pair
also passed. These are renderer-free real host fixtures with fake cognition,
not a replacement Engine or application save/load adoption test.

The [independent audit](../coordinator/performance-audit.json) preserves all
2,400 end-to-end phase samples and 7,800 nested stage samples, including cold
first samples and every tail. Pooled percentiles use nearest rank. There are
1,203 end-to-end samples above 2 ms and1,200 above 30 ms. Stage intervals
overlap the end-to-end totals; they must not be added as independent frame work.

| Phase | Authored p99 / max ms | Populated p99 / max ms |
| --- | ---: | ---: |
| candidate_disposal | 0.703852 / 0.785867 | 0.018724 / 0.023607 |
| complete_capture | 76.099359 / 77.472183 | 225.319801 / 239.667836 |
| complete_load | 34.256788 / 35.859895 | 104.522227 / 111.318788 |
| input_copy | 0.907791 / 0.922651 | 0.783317 / 3.811383 |
| load_stages/CandidateRetention | 0.001738 / 0.003722 | 0.002872 / 0.005065 |
| load_stages/Definitions | 1.187564 / 1.203026 | 1.712467 / 2.249282 |
| load_stages/OuterParse | 6.466494 / 6.651203 | 24.027450 / 24.609399 |
| load_stages/TypedDisposal | 1.270331 / 1.352276 | 4.934315 / 7.396363 |
| load_stages/TypedValidation | 25.752680 / 27.259916 | 75.835396 / 81.305771 |
| save_stages/CandidateRetention | 0.000728 / 0.000817 | 0.000951 / 0.001257 |
| save_stages/Definitions | 2.725867 / 3.923369 | 4.442488 / 4.957384 |
| save_stages/Encode | 13.763023 / 14.147708 | 39.473605 / 40.616412 |
| save_stages/FinalRecheck | 14.748628 / 14.774007 | 43.285264 / 56.409246 |
| save_stages/OuterParse | 6.507151 / 6.846734 | 25.020044 / 28.601360 |
| save_stages/Preflight | 12.311295 / 12.411127 | 37.043276 / 38.473597 |
| save_stages/TypedDisposal | 1.318199 / 1.325881 | 5.056524 / 5.985223 |
| save_stages/TypedValidation | 26.030601 / 27.240335 | 75.348590 / 79.210671 |

| Quantity | Authored | Populated |
| --- | ---: | ---: |
| characters | 520 | 2,520 |
| encoded_bytes | 3,309,064 | 12,779,988 |
| expanded_upper_bytes | 38,046,363 | 133,351,183 |
| validation_peak_bytes | 115,717,075 | 239,434,667 |
| retained_candidate_bytes | 3,310,684 | 12,781,608 |
| Shared peak including Running | 669,365,203 | 793,082,795 |
| Process RSS peak, KiB | 380,736 | 476,628 |

All byte limits remain unchanged. Expanded populated headroom is only
**866,545 bytes** below 128 MiB. Shared peak includes512 MiB Running and
actual simultaneous Save/Load plus subordinate capture leases; validation_peak
alone does not include all of that coexistence. No arbitrary mixture of legal
component maxima is promised to fit. RSS corroborates process behavior but
does not establish the logical allocation proof or renderer admission.

The actual capacity inventory records two distinct 6,365,973-byte navigation
graphs. World and Engine share an Arc and 20 cached distance rows; Vermin has
no populated distance rows. The theoretical two-full-cache ceiling is 818,020,394
bytes. The ordinary registered-destination consumer bound instead gives at most
32,920,098/93,164,234 navigation bytes for these authored/populated workloads,
within the 128 MiB navigation allowance. See the [source/capacity argument](../owner-design.md#scoped-running-decomposition)
and each raw report's running_inventory. Empty observed completion/publication
queues do not imply zero backend runtime or service allocation.

The copied optimized test executable is 151,811,648 bytes, SHA-256
`feee70f0632732184c6a7b3aa3f2ebde02d0dc4b27a6a9d0e4225955988beabc`. Initial executable hashing took
76.180–76.679 ms across these processes, once per process outside capture timing.
This is a startup cost, not hidden inside a claim of free compatibility checking.

All measurements bind the unchanged961-input map SHA-256 `fe370c6f8a82815eb0d806f2975cee1a5db6814cfcbfafb0d56657c1c72ce53b`.
The exact command environments, helper identities, original reports/logs/time
files and mtime0 gzip archives are in IDENTITY.json, RESULTS.json and the
per-command records here. The release build succeeded in 324.609 seconds.
The first smoke runner supplied a 17-byte lineage; production correctly refused
it. The corrected 16-byte input passed with the same executable. Both attempts
and the helper correction remain in [release-history-audit](../coordinator/release-history-audit.json).

**Frame acceptance remains open.** Every complete capture and load sample
exceeds 30 ms. M3 must move or split definition scanning, extraction, encoding,
validation and disposal while preserving a coherent ordinary boundary. This
record accepts complete read-only composition, not synchronous frame placement,
typed Engine hydration, external adoption or generation retirement.
