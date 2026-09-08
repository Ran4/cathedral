Status: Accepted component measurements (2026-09-08). Synchronous host-frame and complete-save acceptance remain pending.

# M2a5 release knowledge measurements

Six sequential release runs alternate authored and populated order, with 100 samples per phase per run: 3,600 raw phase samples. Both use production placement, with 520 and 2,520 characters and six carried holdings per character. Each retains six live facts, 48 air rows, seven journal receipts, an invalidated historical receipt and seated key, an offered occasion, and published journal/eight-ward caches. This is the explicit `carried_news_with_historical_receipts_and_caches` diagnostic; it does not measure every legal knowledge state.

The [independent audit](../coordinator/knowledge_performance_audit.json) recomputes every percentile and summary from lossless raw archives, verifies placement/history/charge gates and all 830 source hashes, runner hashes and the release binary. [Summary](SUMMARY.json), [per-run results](RESULTS.json) and [identity](IDENTITY.json) retain the complete record. The earlier [smoke audit](../coordinator/knowledge_smoke_audit.json) verifies the two-run, 24-sample harness check. [Negative auditor cases](../coordinator/auditor_regressions.json) reject a changed percentile, undercharged working allowance and missing historical receipt even when corresponding archive hashes and metadata are updated. The final auditor also passes historical [climate](../coordinator/historical_climate_audit.json) and [Round](../coordinator/historical_round_audit.json) archives.

Median of the three per-run p99 values, in milliseconds:

| Phase | Authored | +2,000 residents |
|---|---:|---:|
| Borrowed preflight | 1.918 | 7.630 |
| Export and validate | 5.201 | 18.358 |
| Encode and dispose DTO | 1.895 | 8.757 |
| Decode and validate | 2.537 | 9.058 |
| Candidate validation | 0.947 | 1.433 |
| Drop saved bytes/candidate | 0.052 | 0.224 |

The largest individual sample is 19.524 ms, populated export. Authored export/decode and populated preflight/export/encode/decode exceed the 2 ms per-frame added-work budget. This confirms the existing requirement for bounded offload or incremental coordination before application integration. No complete capture/hydration, filesystem, renderer or ordinary-poll performance claim follows. Ordinary Engine/knowledge source is unchanged apart from checkpoint module wiring and comments, so no redundant ordinary-poll comparison was run.

| Charge, bytes | Authored | +2,000 residents |
|---|---:|---:|
| Encoded payload | 501,528 | 2,381,526 |
| Conservative expanded bound | 8,066,338 | 38,050,338 |
| Attached working allowance | 4,456,448 | 4,456,448 |
| Per-cohort peak | 38,230,480 | 163,806,474 |
| Simultaneous Save+Load, excluding Running | 76,460,960 | 327,612,948 |

Payload and standalone component charges fit their existing limits. They are conservative retained reservations, not measured allocator usage. Maximum process RSS is 67,912 KiB and includes world construction and the running Engine; it cannot replace the complete-envelope admission proof. The already recorded M2a2+Round Save+Load sum is 1,256,093,444 bytes before this component or Running, above the global 1 GiB limit. Complete phase lifetimes and Running/retiring coexistence must still be designed and verified without raising the limit.

The [release build](../coordinator/release_build.json) binds final owner manifest SHA-256 `6f1b57b911cad90e2cbb99b18eccf38b3e978ed1035b9c96c45f27e6d8f51262`. Binary SHA-256 is `776ee0ff74fc92c0c4f40c055b85eb1a73feb29fbe193fb8be6cf0cd351172d4`, preserved at `/tmp/alibi-m2a5-knowledge-reference-binary`. Measurements used the existing Intel Core i5-13600KF environment with headless/fake backends and no concurrent build or other coordinator benchmark.
