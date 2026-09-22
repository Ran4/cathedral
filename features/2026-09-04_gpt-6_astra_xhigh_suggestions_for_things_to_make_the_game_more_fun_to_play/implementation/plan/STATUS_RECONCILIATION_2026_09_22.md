# Implementation status reconciliation — 2026-09-22

Earlier progress messages and commit titles overstated complete milestone
delivery. The committed code contains useful partial cuts; the original
completion gates still apply. In particular, `CommittedStartup::require_complete_admission`
always refuses, and F6/F9 do not perform a save or restore. No successful
whole-App save/load or restored-frame acceptance is established by these cuts.

| Recorded cut | Implemented scope | Still required |
| --- | --- | --- |
| M3b2b3 | Finite diagnostic storage, native worker retention, evidence fences | Independent complete application accounting/acceptance |
| M3b2b4 | Startup budget, config bounds, navigation inventory | Complete immutable assets/config/transports and mutable world allocation |
| M3b2c | Shared startup config/navigation/runtime/archive owner | Complete candidate staging, controller/projection restore and atomic adoption |
| M3 actor sources, owner verified | Frozen enabled-actor inputs, admitted Rust-owned discovery/read/copy storage and audited Linux/glibc DIR object; disabled startup preserved | Other allocator/platform modes, parser/compiler/configuration admission, production hydration factory and complete application accounting |
| M3 embedded shelters, owner verified | Pre-admitted parse/validation and shared immutable rows reused by installed EngineConfig/World; disabled bypass preserved | Other definitions/configuration/transports, production hydration and complete application accounting |
| M3c | Routing lifecycle and bounded refusal controls | Working save/load, slots, confirmation and continuation UI |
| M3d | Screenshot admission and truthful write receipts | Complete restored application, latency/memory and frame evidence |
| M4 | Street queries, temporary closure checks and map barrier refusal | Layered interiors, portals and full traversal/persistence |
| M5 | Sampled observation fixtures and gate-aware focus | Semantic perception migration, durable observations and full coverage |
| M6 current-authority cut | Current travel/release policy and optional bounded receipts | Registry, keys/grants/loans, portal operations and persistence |

Historical focused checks remain evidence only for their recorded source and
scope. The later [integration record](evidence/integration_2026_09_22/REVIEW.md)
passes 2,360 workspace tests before the actor-source patch; it does not validate
that later patch. [Source capture verification](evidence/m3_asset_sources/README.md)
records its separate focused checks. The [native-directory follow-up](evidence/m3_native_directory/README.md) adds 16 passing focused witnesses and a normal production build on its own exact source map, without accepting complete M3. Review found an M6
compatibility regression: receipt capacity was incorrectly imposed on existing
custody release. The correction preserves direct release for valid custody
states above 32 holders and refuses only optional receipt capture.

The corrected M6 current-authority cut is committed as a partial foundation.
Continue unfinished M3 accounting/adoption prerequisites after the scoped
actor-source repair; source retention does not close parsed-asset or world admission.
Do not advance to M7 while calling M3–M6 complete. The separate M0 renderer,
live provider/device and human play acceptance gaps remain explicit.

The [embedded shelter follow-up](evidence/m3_shelter_admission/README.md) records 16 passing focused tests and a normal build on its own exact source map. This admits one immutable definition role, not complete M3 or generic JSON parsing.
