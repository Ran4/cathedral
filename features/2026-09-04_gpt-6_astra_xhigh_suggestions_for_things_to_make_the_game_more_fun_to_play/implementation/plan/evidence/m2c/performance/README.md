Status: release evidence independently audited and accepted (2026-09-15).

# M2c release preparation measurements

Three fresh processes per profile each perform 100 samples: 600 total, with
1,800 complete-phase and 3,000 stage observations. Every cold first sample and
every sample above the 2 ms and 30 ms frame thresholds is retained without
normalization in the [independent audit](../coordinator/release-performance-audit.json)
and [threshold rows](../coordinator/release-performance-frame-threshold-samples.json).
The [raw dataset](RESULTS.json) binds exact originals, gzip archives, source,
environment, helpers and executable identities.

All times below are microseconds; quantiles use nearest rank over all 300
samples per profile.

| City | Phase | p50 | p95 | p99 | Maximum |
| --- | --- | ---: | ---: | ---: | ---: |
| authored | preparation | 9.864 | 10.512 | 11.561 | 13.964 |
| authored | service binding | 1.945 | 2.809 | 2.985 | 3.611 |
| authored | prepared owner disposal | 853.095 | 1078.475 | 1402.100 | 1821.504 |
| populated | preparation | 10.153 | 10.643 | 13.114 | 13.964 |
| populated | service binding | 2.782 | 3.210 | 3.649 | 5.390 |
| populated | prepared owner disposal | 3839.359 | 4252.663 | 4524.353 | 4961.803 |

All 600 source boundaries contain one submitted unfinished scheduler request and
one accepted recording. Preparation retains one exact retry and one interruption
notification without polling or submitting services. Eight unaffected owner
categories match per sample. The first sample in every process additionally
re-saves, loads and prepares the full result with exact repeated bytes.
Held and Night outcomes have separate deterministic unit coverage; they are not
the measured actual-city source shape.

Typed preparation is 38,447,138 B for 520 characters and 133,751,958 B for 2,520
characters (2,000 requested and placed), leaving 465,770 B beneath the unchanged
128 MiB cap in the populated case. Asset/service leases are 64 MiB/64 KiB.
Maximum aggregate peaks including Running and re-save are
758,093,673/977,116,076 B, below 1 GiB. Maximum process RSS is
383,636/495,908 KiB. RSS is an observation of this executable, not the admission
proof; [ADMISSION](../ADMISSION.md) records that proof and its scope.

Preparation, inert service binding and prepared-owner disposal are separate
timed regions. Input capture, validation, hydration, hashes and re-save are
excluded from those timers. The original host retains shared navigation;
this is neither whole-host retirement nor last-navigation-owner disposal.
All 300 populated disposal samples exceed 2 ms; no measured phase exceeds 30 ms.
M3 must account for capture/hydration, projection, runtime/device ownership and
retirement beyond the adoption frame.

The preserved creating release executable is `/tmp/alibi-m2c-host-reference-binary-1`,
155,895,728 B, SHA-256
`36512740197e3a687f86dff2efdc86a8e06c7189690b3634b8215ba4c6701e52`. The frozen 980-input source-map SHA is
`b277379757dd70a1476adb44e583aeb808533ef58178fa88f70f5e20600d35a1`. Publication subsequently adds only the
two verified runtime fixture outputs and their README; all prior inputs remain
byte-identical.
