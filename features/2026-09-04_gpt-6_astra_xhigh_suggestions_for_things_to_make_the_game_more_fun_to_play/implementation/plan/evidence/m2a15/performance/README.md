# M2a15 release host component measurements

Accepted component evidence, 2026-09-14. Six independent processes, three per mode, each retain 100 samples for all six phases: **3,600 raw phase samples**, with cold samples and all tails preserved. The two separate smoke processes retain 24 additional samples. These measurements do not establish complete-save or host-frame acceptance.

[Independent timing and provenance audit](../coordinator/performance-audit.json) binds each original JSON, stdout/stderr and process timing to deterministic gzip archives, command-start source/environment/helper hashes and the immutable executable. [Smoke audit](../coordinator/smoke-audit.json) and [build audit](../coordinator/release-build-audit-final.json) passed.

Final source map: `5251ea6af940cce508c66111573bcdf1d53343207e7dea1ee2e5dbc048f62b6a` (950 inputs). Binary: `/tmp/alibi-m2a15-host-reference-binary-2`, SHA-256 `e6be43620573a23046479444fc80ca7c3f735e90fd991d491d24ee049506fd3a`, 144,706,536 bytes. Build 1 is pre-fixture evidence only; [build notes](../coordinator/release-build-notes.md) preserve the environment difference and initial lock wait.

Actual renderer-free CityPlugin workloads contain 520/2,520 characters and 18,496/73,984 ECS entities; all 2,000 requested additions are placed. Both retain 751 collision boxes, 1,153 convex prisms, two barriers, installed CutMargin and eight vermin colonies/150 rats. Their 22 readable rows include four unread speech messages, one unread intent, a subtitle, bubble, original player caption, pending input and semantic sound gates. World revision stays 1103/5103; event sequence stays 7 and input watermark stays 10.

Both modes encode **6,275 bytes**, with **107,462 bytes** conservative expansion, **5,081,249 bytes** per component peak admission and **10,162,498 bytes** simultaneous Save plus Load, excluding Running. The latter is not the live World footprint or the total host allocation. Diagnostic copies, ECS/asset setup and report writing are outside phase timing and these component charges.

## Pooled timings

Milliseconds; nearest-rank quantiles over all 300 samples per mode and phase. Rounded display only; originals retain fractional microseconds.

| Mode | Phase | p50 | p95 | p99 | Maximum | >2 ms | >30 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| authored | candidate_validate | 0.975437 | 0.986550 | 0.995289 | 1.020663 | 0 | 0 |
| authored | decode_validate | 1.038733 | 1.063147 | 1.083816 | 1.160727 | 0 | 0 |
| authored | drop | 0.001003 | 0.001533 | 0.001858 | 0.008881 | 0 | 0 |
| authored | encode | 0.042342 | 0.057513 | 0.060614 | 0.065262 | 0 | 0 |
| authored | export | 5.549440 | 5.602324 | 5.634495 | 7.036447 | 300 | 0 |
| authored | preflight | 2.251566 | 2.286900 | 2.365707 | 3.288829 | 300 | 0 |
| populated | candidate_validate | 1.479550 | 1.506236 | 1.519295 | 2.199595 | 2 | 0 |
| populated | decode_validate | 1.551127 | 1.584279 | 1.622757 | 2.374960 | 2 | 0 |
| populated | drop | 0.001125 | 0.001663 | 0.002005 | 0.005146 | 0 | 0 |
| populated | encode | 0.048776 | 0.060852 | 0.064027 | 0.081899 | 0 | 0 |
| populated | export | 9.865319 | 10.014673 | 10.245549 | 10.546378 | 300 | 0 |
| populated | preflight | 4.087666 | 4.163386 | 4.367553 | 6.401564 | 300 | 0 |

**Frame integration remains pending.** Every preflight and export sample exceeds 2 ms. Export p99 is 5.634495/10.245549 ms; maxima are 7.036447/10.546378 ms. Four populated decode/candidate samples also exceed 2 ms. All 1,204 such samples remain individually enumerated in the audit; none exceed 30 ms. M3 must arrange bounded offload or incremental host work, including source observation and definition hashing, without freezing ordinary accepted input across frames. Encoding alone being fast is insufficient.

## Process and machine context

Maximum process RSS is **379,372 KiB authored / 409,272 KiB populated**. It includes actual ECS geometry/assets, fixture setup, allocator retention and diagnostics, and is supporting process evidence rather than a cohort-allocation proof. The full envelope still owes actual Running/Save/Load/retiring lifetimes under the existing 1 GiB ceiling.

A [machine snapshot](../coordinator/machine-20260914.archive.json) was captured immediately after the probes, not at command start: x86_64 Linux 6.8.0-138-generic, Intel Core i5-13600KF. Exact CPU/memory/OS text is archived. The queried cgroup limit files were unavailable; no container allowance is inferred. This leg performed no renderer/device availability check and makes no GPU, visible-window, audio or provider claim.
