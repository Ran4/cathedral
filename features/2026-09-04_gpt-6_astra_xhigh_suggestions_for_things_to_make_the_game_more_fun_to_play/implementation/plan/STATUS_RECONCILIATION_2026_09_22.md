# Implementation status reconciliation — 2026-09-22

Earlier progress messages and commit titles overstated complete milestone
delivery. The committed code contains useful partial cuts; the original
completion gates still apply. In particular, `CommittedStartup::require_complete_admission`
always refuses, and F6/F9 do not perform a save or restore. No successful
whole-App save/load or restored-frame acceptance is established by these cuts.

| Committed cut | Implemented scope | Still required |
| --- | --- | --- |
| M3b2b3 | Finite diagnostic storage, native worker retention, evidence fences | Independent complete application accounting/acceptance |
| M3b2b4 | Startup budget, config bounds, navigation inventory | Complete immutable assets/config/transports and mutable world allocation |
| M3b2c | Shared startup config/navigation/runtime/archive owner | Complete candidate staging, controller/projection restore and atomic adoption |
| M3c | Routing lifecycle and bounded refusal controls | Working save/load, slots, confirmation and continuation UI |
| M3d | Screenshot admission and truthful write receipts | Complete restored application, latency/memory and frame evidence |
| M4 | Street queries, temporary closure checks and map barrier refusal | Layered interiors, portals and full traversal/persistence |
| M5 | Sampled observation fixtures and gate-aware focus | Semantic perception migration, durable observations and full coverage |
| M6 working tree | Current travel/release policy and optional bounded receipts | Registry, keys/grants/loans, portal operations and persistence |

Historical focused checks remain evidence only for their recorded source and
scope. No full-workspace pass after M3b2b2 is claimed here. Review found an M6
compatibility regression: receipt capacity was incorrectly imposed on existing
custody release. The correction preserves direct release for valid custody
states above 32 holders and refuses only optional receipt capture.

Finish review/verification of the existing M6 cut, commit it as a partial
foundation, then return to unfinished M3 accounting/adoption prerequisites.
Do not advance to M7 while calling M3–M6 complete. The separate M0 renderer,
live provider/device and human play acceptance gaps remain explicit.
