# Complete hydration measurements — 2026-09-15

Six fresh release processes each run 100 captures/hydrations at an actual rich
ordinary CityPlugin boundary: three with 520 characters and three with all 2,000
requested additions placed, for 2,520 characters. All sixteen restored owner
hashes match the saved categories after every hydration. Capture changes no
ordinary boundary/event/input witness, and before/after navigation inventories
match. Two-sample release smokes for both modes passed separately.

The source map is `../source_hashes.json`, 967 inputs, SHA-256
`be5312aa3547f660396f61d39014354446f765211ca0ea2c10314bc5b54eb57b`.
The executable is `/tmp/alibi-m2b-host-reference-binary-1`, 150,439,688 bytes,
SHA-256 `60831ac78fc0e6b18314f9ec8d85332d10dbaa28bd4826561b7cb80fda41b281`.
The build, each command start, exact original logs/metrics/time output and lossless
archives are bound by `IDENTITY.json`, `RESULTS.json` and the coordinator audits.
No source or helper changes occurred during those commands.

## End-to-end durations

Each row has 300 samples. Values below are milliseconds; quantiles use nearest
rank. All first samples and tails are retained, including the maximums.

| Mode / phase | p50 | p95 | p99 | Maximum |
| --- | ---: | ---: | ---: | ---: |
| 520 / hydrate including asset factory | 38.734 | 39.241 | 39.981 | 44.903 |
| 520 / hydrated-owner disposal | 0.977 | 1.250 | 1.418 | 1.491 |
| 2,520 / hydrate including asset factory | 116.616 | 117.688 | 119.324 | 121.202 |
| 2,520 / hydrated-owner disposal | 3.782 | 4.129 | 4.346 | 4.620 |

The hydration clock starts after asset-lease preparation and prior complete-input
validation. It includes the synchronous admitted factory and the six checkpoints
below, then stops when the admitted hydrated owner returns. Category observations
are outside that clock. Disposal measures dropping the new quarantined Engine,
continuation owners and asset lease after those observations. The original App
still owns shared navigation during timed repetitions, so this is not a last-nav-
owner or whole active/retiring host/runtime destruction measurement. File IO,
renderer/device work and application adoption are outside this probe.

## Hydration checkpoints

Each row has 300 samples per mode; these p99 values are milliseconds.

| Checkpoint | 520 | 2,520 |
| --- | ---: | ---: |
| Assets | 7.216453 | 24.699182 |
| Definitions | 2.720114 | 9.826238 |
| TypedOwners | 30.265055 | 85.118156 |
| Construction | 0.004411 | 0.004962 |
| RawDisposal | 0.001596 | 0.001744 |
| Retention | 0.000091 | 0.000081 |

`../ADMISSION.md` defines the exact work and ownership transitions at each point.
The audit retains 1,200 end-to-end values and 3,600 checkpoint values. Across
these 4,800 values, 2,700 exceed 2 ms and 904 exceed 30 ms. The full individual
rows, with process/sample/phase identity, are in
`../coordinator/release-performance-frame-threshold-samples.json`; first samples
and all distributions are in `../coordinator/release-performance-audit.json`.

## Admission and retained assets

| Bound / observed high water, bytes | 520 | 2,520 |
| --- | ---: | ---: |
| Complete typed expansion including new structure | 38,308,507 | 133,613,327 |
| New structural allowance within expansion | 262,144 | 262,144 |
| Separate asset lease | 67,108,864 | 67,108,864 |
| Retained hydrated owner plus asset lease | 105,417,371 | 200,722,191 |
| Maximum full measured/scoped asset bound | 13,022,860 | 22,998,242 |
| Maximum shared admission including 512 MiB Running | 726,174,363 | 843,676,620 |

Every sample stays under the unchanged 128 MiB expansion and 1 GiB shared limits.
The populated expansion retains only 604,401 bytes of headroom. The factory bound
adds cumulative synchronous allocation requests, 7,409,445 bytes for the distinct
retained navigation graph/cache/Arc, and 1,048,576 bytes for shared Prompt/runtime
storage. The independent source audit derives 712,704 bytes for the latter;
the larger admitted allowance remains within the existing 64 MiB asset lease.
Non-nav installed definitions are deep cloned into independently owned values.

World and Engine share one navigation Arc in these probes: 10,026 nodes,
6,365,973 graph/index bytes and 20 retained cache rows using 1,043,408 bytes.
The possible full cache is 402,644,224 bytes and is not included in this scoped
asset bound. Quarantined observations cannot warm it; caller-driven growth and
persistent static/TLS storage need continuous coordinated Running ownership.

Maximum process RSS is 382,512 / 479,556 KiB. RSS and cumulative allocation
requests are different measurements; neither replaces admission accounting.
The test-only allocator forwards to System and counts requests on the factory
thread, including reallocations without subtracting frees. Timings include its
diagnostic hook. The production executable allocator is unchanged.

The measured hydration and populated disposal exceed the host frame target.
M3 must provide bounded offload/incremental coordination, service binding,
publication and actual retirement accounting. These results do not establish
synchronous host-frame acceptance; no renderer or stress run was attempted.
