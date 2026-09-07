Status: M0 CPU workload measured and numerical admission targets recorded (2026-09-07). V03 remains partial: renderer/frame, full 20,000-resident stress and future DTO/active-work measurements are not accepted.

# Running continuously within measured limits

The user requires an ordinary living city and save-anywhere. These policies prevent slow frames, background IO and durable history from quietly defeating those promises. M0 freezes the numerical reference record before dependent implementation; M13 and M19 consume that record rather than selecting accommodating thresholds after the work.

## M0 record — 2026-09-07

[The executable baseline and raw evidence](evidence/m0_baseline/README.md) use current source `46e24abf429b3113d3c5745324e5746d82345140`, a release build on Intel i5-13600KF/Linux/rustc 1.96.0, 0.05-second physical polls, Stage/novelty/curiosity and fake cognition. The final 26 sequential runs form 13 identical-source A/B pairs. Default authored, configured 1,000-extra and target 2,000-extra market runs measure 60 seconds after five seconds warmup; the stress pair measures 50 seconds after five seconds warmup. Source/content hashes, placement counts, process CPU/RSS, raw samples and paired counters are archived. This measures a bounded ordinary segment, not a full day or host frame loop.

Median per-run market pump p95/p99 is **1.498/2.452 ms** at authored count, **4.573/5.568 ms** at +1,000, and **8.160/9.242 ms** at +2,000. Peak process RSS is **30.3/45.5/64.3 MiB** respectively. Idle authored p95/p99 is **1.449/2.354 ms**. Paired same-source p95/p99 variation reaches 8.00%/6.56%; later comparisons need the same inputs and paired-run statistic, not a selected fastest sample. These CPU figures establish a reference; no feature cost reduction is claimed.

### Numerical targets fixed for review; measurements still required where named

[BUDGETS.json](evidence/m0_baseline/BUDGETS.json) records the same numbers with explicit measured/unmeasured fields.

| Scope | Numerical limit | Evidence state / owner |
|---|---|---|
| Unchanged ordinary pump, authored/+1,000/+2,000 | Relative added p95 and p99 ≤10% against matched paired baseline. Absolute p95 ≤2/6/10 ms and p99 ≤3.5/8/12 ms respectively. Compare median paired ratios; preserve every raw run. | Current CPU reference lies inside these absolute envelopes. A later unchanged workload must satisfy relative and absolute checks. Host frame acceptance is separate. |
| Ordinary real-renderer host, authored/+2,000 | 1920×1080, release, full assets, VSync off for cost: frame p95 ≤16.67 ms, p99 ≤33.33 ms; also record VSync-on pacing, overlays and normal unfocused policy. | **Unmeasured.** No display/GPU devices in this environment, even outside the sandbox. These are reference goals, not an accepted hardware result. Closing this part of V03 blocks M0. |
| Save/load coordinator | ≤2 ms p99 and ≤5 ms maximum per frame, including capture, hydration/staging, adoption, destruction and later COW effects. | M2/M3 must instrument every phase on real host frames; no save DTO exists to benchmark at M0. |
| Active foundation fixture | +2,000 residents, 16 concurrent operations (eight travel, four work/examine, four resource-contention/wait), eight appointments, four pending legal orders, 16 observation tracks; half off stage; cognition unavailable. Incremental pump p95/p99 ≤2/3 ms over matched ordinary workload, total p95/p99 ≤12/16 ms; added resident allocations ≤128 MiB. | Counts/ceilings are fixed design inputs to M13, not an executable current fixture. M1/M4–M12 introduce the needed adapters; M13 executes the actual mix. |
| Private encoded save payload | ≤64 MiB authored, ≤128 MiB +2,000. One save payload, one load candidate, one retiring generation maximum in addition to running world; shared resident allocation ceiling 1 GiB for these four cohorts. | Finite admission targets reserving substantial headroom over measured 30.3/64.3 MiB process baselines; process RSS is **not** DTO size. M2/M3 measure the real export/encode/hydrate/COW sizes and reject before exceeding budgets. The host's reported 31.2 GiB RAM does not prove a container allowance. |
| Replay window | 4,096 consequential envelopes, ≤1 KiB encoded receipt/digest metadata each, ≤4 MiB recent ledger; ≤256 active operations and ≤32 producers, independent latest-position sequencing. | M1/M2 must measure real records and reject overlarge payload/producer admission; archive semantic roots separately. These are design allocations, not `size_of` results for nonexistent types. |
| Retained shared knowledge | 16,384 archived identities, 65,536 provenance links, 4,096 protected roots and ≤32 MiB archive allocations; existing 256 live/6 carried/3 rendered limits unchanged. | [Shared-owner contract](BASELINE_RECONCILIATION.md) assigns private DTO/retention/admission to M2/M5/M9. Archive bytes and transitive roots still need actual implementation measurement. |
| Existing canaries | 64 KiB prompt, 160 KiB **original authored snapshot fixture**, 32 MiB saturated knowledge+consequence estimate. | Keep current tests and fixture identities. M0's nav/round-rich snapshot is a different workload, recorded below. |
| Nav/round-rich publication | ≤256 KiB authored, ≤2 MiB +2,000 per full snapshot; no private evidence in snapshot; future learned-record pages ≤32 KiB at ≤2 Hz outside explicit page requests. | Current final snapshots 234,517 B authored and 1,737,971 B +2,000; future pages require M5/M9/M10 measurements. No claim this replaces the old 160 KiB fixture canary. |

These targets are visible before dependent production work. **M0 is still partial**, so a target for an unimplemented type or absent renderer is not a frozen measured acceptance claim. Review and reconcile the remaining V03 evidence before M1; later owners are responsible for measuring their own new DTO/workload costs rather than retroactively treating the limits as observations.

### Dated baseline exceptions and limitations

1. A request for 20,000 extra residents currently places only **9,072**, leaving 10,928 unplaced under production occupancy admission. That pair reports 9,592 total actor records, p95/p99 41.203/43.530 ms and worst poll 53.188 ms, peak RSS 197.5 MiB. It is capacity-limited stress and cannot satisfy a literal 20,000-extra simulated workload. Do not bypass occupancy or pretend requested equals placed. Reconcile supported stress scope/placement capacity before accepting that V03 branch.
2. Ordinary nav/round population produces **234,517 B** final snapshot even without extra residents, exceeding the original 160 KiB **fixture's** number. Keep separate fixture contracts instead of silently changing that canary. Crowded snapshot bytes are 986,080 at +1,000, 1,737,971 at +2,000 and 7,048,132 in admitted stress. Render/publication cost of those messages remains unmeasured here.
3. No display/GPU means frame, production focus scheduling, GPU memory and overload recovery are unavailable. [Availability evidence](evidence/m0_baseline/renderer_availability.json) names the exact checks. The earlier knowledge software UI runs are not current frame measurements. No M0 Bevy window was launched.
4. Checkpoints, replay DTOs, archived identity and the declared active mix are future types/services. M0 measures public serialized bytes and a knowledge-only heap estimate; it cannot report actual complete-save bytes. Those null/missing measurements remain explicit, with producer gates above.

## Accepted time and overload — M1

Use one accepted logical elapsed stream for physical motion and domain deadlines. Host wall time, accepted time, accumulated debt and any deliberately discarded exceptional-suspension time are separately measured. Calendar scaling changes the calendar mapping; it does not excuse losing movement spans or interpreting old crossing cursors through a new slope.

Normal gameplay accumulates elapsed work and spends it in bounded physical substeps with a finite per-frame recovery budget. Keep unspent debt rather than advancing appointments while dropping the actors' journeys. A save captures the accepted boundary and residuals, not an implied future in which the debt has already been simulated. Live input is stamped/queued against the documented accepted-time boundary.

Preserve **ordinary wall-time debt already accumulated at capture** as a separate host-continuation field. Example: logical time 10 seconds, 400 ms ordinary debt and 15 ms accepted fixed residual restore as those three values. Initial publication still shows logical time 10; later normal budgeted substeps service the saved debt. Time while the app is closed or the load is being prepared adds no debt. Exceptional suspension deliberately excluded under the documented policy was never ordinary debt and cannot reappear on load.

Coordinate the controller and engine, not only the engine clock. Current Bevy virtual time drops duration beyond a 100 ms frame cap; changing only Engine to real time would instead hit its 0.4-second movement cap. M1 must define how both consumers receive the same accepted progression and how debt survives ordinary jitter. M4/development fast-forward spends that progression in order as well.

Do not permit unbounded catch-up to monopolise the UI. Sustained inability to service debt under the supported default/2,000-citizen workload fails performance acceptance and requires a fix. Separately document exceptional OS suspension/debugger recovery; do not hide a normal overlay or unfocused window behind that exception. A 20,000-citizen stress overload is reported as such, not passed as ordinary responsive play.

Test production focus scheduling, not only the headless override. Normal unfocused mode presently schedules roughly 60 Hz rather than pausing; headless explicitly forces continuous updates and can mask differences. Opening reading/chat/settings/save views must not change the accepted-time policy.

## Numerical reference record — M0, M13, M19

Record CPU, GPU, RAM, OS, build profile, source/tree and relevant content hashes, resolution, renderer/VSync mode, clock rate and exact workload. Repeated paired baseline/change runs must use the same workload and report raw samples plus p50/p95/p99/max, not only an average FPS.

Use existing `src/perf.rs` instrumentation (`CATHEDRAL_PERF`) for frame records where a valid renderer is available. Controlled `Engine::poll` increments no larger than 0.05 seconds provide movement-faithful CPU baselines without depending on a future repaired clock watcher. Report startup separately. Do not compare a coarse clock-only watcher with a physical simulation run as though they do equal work.

Provisional targets to evaluate and freeze at M0:

| Workload | Proposed gate before measurement |
|---|---|
| Normal idle/ordinary city, default and 2,000 extra citizens | Added pump p95 and p99 no more than 10% over paired baseline, with absolute reference-hardware frame goals recorded too |
| Save/load coordinator work on a frame | At most 2 ms p99 and 5 ms maximum, including extraction/staging/publication/retirement and subsequent copy-on-write costs |
| Active foundation workload | A fixed declared mix of concurrent operations, observations, appointments and orders, compared against the same ordinary city; M0 sets its counts and absolute CPU/memory ceiling |
| Save payload and resident resources | Absolute byte ceilings derived from measured default/2,000-citizen worlds and reference RAM, including all simultaneously retained generations |
| 20,000 extra citizens | Separate bounded stress run reporting slowdown, peak memory and recovery; no unsupported claim of ordinary frame-rate parity |

These are proposed acceptance thresholds, **not measured achievements**. M0 must publish executable workload definitions and final absolute limits. If an existing baseline already breaches a proposed target, record a dated exception with evidence or make its repair a prerequisite. Do not quietly enlarge limits after a regression. Later remeasurement is justified by changed code, failed gates or unresolved risks, not repeated ritual runs.

## Resource admission — M2/M3

The first design admits at most **one retained save payload, one load candidate and one retiring generation**, in addition to the running world, under a shared byte budget. These are maxima, not an obligation to allocate all at once. Check admission before extraction/hydration/staging; a request queue stores small intent metadata rather than extra world copies. Supersede/cancel obsolete requests before admitting the next candidate.

Track payload extraction, encoding, decode/validation, non-`Send` hydration/index building, inactive entity staging, final publication, destruction and subsequent COW separately. Make expensive host-only phases incremental under the frame budget where they cannot move to a worker. A pointer swap with a 500 ms destructor afterwards fails the same gate as a blocking load.

Repeated cancelled loads and saves during delayed retirement must stay within both generation-count and byte limits. Old inactive entities cannot accumulate behind successful cancellation messages. Save-operation identity/order remains independent of the currently loaded world.

Bound callback admission and draining too. Current backend channels can be unbounded and the host drains until empty; generation rejection alone prevents wrong effects but does not bound stale allocation or frame work. Use generation-owned jobs, bounded producer/channel capacity and a finite terminal-result cohort/watermark per normal pump. Preserve terminal outcomes or an explicit cancellation/failure state; do not drop a completion that is the only record of an owed response. Audio chunks need their own bounded streaming/backpressure policy.

## Durable knowledge without unlimited live news — M0/M5/M9

The completed knowledge feature remains the sole owner of fact identity and holdings. [M0 reconciles its accepted behavior](BASELINE_RECONCILIATION.md): the shipped 256-live-fact cap can call invalidation that removes identity, holdings and air, while standing facts cannot be evicted. Neither this deletion nor pinning every fact in the live-news budget satisfies durable evidence.

Add the necessary **archival identity tier inside shared knowledge**, separate from bounded live-news/relevance indexes. M5 supplies retained observation roots and the generic retention API; M9 registers inquiry/account/provenance roots through it. Preserve stable historical proposition identity and required source links without making cold records volunteer in prompts. Never persist dense internal indexes as durable identity without a defined remap.

Define protected roots, transitive provenance dependencies and reference release. Closed inquiries compact redundant transition chatter while retaining the immutable submitted core, findings/amendments and their required evidence. Ordinary hot news may cool/leave working indexes; that is different from destroying a referenced historical proposition. Re-mint/reactivation after cooling must not create a second identity for the same retained event.

Preflight installed content against protected-history quotas and reserve capacity for its essential records. Publish explicit archive/live limits, admission and saturation behavior during M0/M5 reconciliation. If protected history cannot be admitted, fail pack validation before world creation or defer a new inquiry through its ordinary service; do not begin a promised case and silently discard its only evidence. Completing ordinary world actions must not depend on unlimited gossip allocation: their authoritative domain receipts remain real even when optional news cannot enter the hot index.

Test beyond 256 mints and six carried holdings, including source correction, transitive shared roots, closed/reopened inquiries and fresh-process save/load. Add and remove a content pack only through its declared compatibility policy. This is a bounded history/retention extension, not a second quest fact store or an unrestricted permanent event trace.

## Replay retention — M1/M2

Use a coordinator-owned ordered stream for consequential envelopes; high-frequency latest-position samples have separate sequence handling and do not consume the durable action ledger. Persist a replay floor, bounded recent acceptance/rejection window, payload digest and active operation/step identities.

Initial capacity is 4,096 recent consequential envelopes per world, plus separately bounded active/referenced operations; M0 measures the actual byte cost before freezing it. Same ID with a different payload is a conflict. IDs at or below the compacted floor are rejected even if their detailed receipt is gone, including old never-accepted commands. Retrying an expired request requires a new intentional request and current validation; it is never silently reissued.

Terminal-operation compaction retains sufficient execution/disposition references and replay high-water state to prevent repeat effects. Active operation IDs and records still referenced by an inquiry/order cannot be collected solely to meet the recent-window count. Bound producer count, out-of-order admission and unresolved holes so an absent command cannot pin an unlimited replay window. Tests exercise window exhaustion and restore, not merely two immediate duplicate clicks.

## Prompt and publication budgets — M0/M5/M9/M10

Preserve or explicitly review changes to existing canaries: **64 KiB prompts** and **160 KiB public snapshots for the authored-cast fixture**. The latter is not a 20,000-citizen snapshot promise. The current knowledge design also bounds rendered slots, carried holdings and provenance-chain depth; reconcile the accepted values at M0 rather than hiding increased costs in this feature.

Keep private evidence out of `PublicSnapshot`. Expose bounded pages/deltas of player-learned records through a request/revision contract. A page can become stale while the city runs; validate its narrow references at action time without retransmitting the whole archive on every crowd movement. Measure bytes and publication frequency during ordinary observation, submission and amendment work, with separate crowd-scaling figures.

## Acceptance artifact contract

Each Vxx result names the executable fixture/input, source/content/geometry/behavior versions, time policy, expected result, actual result and evidence path. Maintain small supported-version save fixtures from M2 and load them through M3's real file/adoption path in fresh processes; test explicitly incompatible variants the same way.

Mechanical runs use production commands/services with explicit controls, unavailable cognition or recorded completions. Asynchronous fault fixtures deliberately delay/reorder/fail cognition and speech. Live-language records archive inputs, model/configuration and observed semantic boundaries. Human sessions record route/input variation and concrete comprehension failures. The current minimal fake responses and short silent fake audio cannot substitute for these latter kinds of evidence.
