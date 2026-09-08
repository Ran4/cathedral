Status: M0 ownership handoffs reconciled (2026-09-07); runtime gate remains partial. Downstream contracts remain required implementation, not accepted milestones.

# Ownership and acceptance handoffs

Use this guide to assign the next bounded milestone after its producers have passed acceptance. [DECISIONS](DECISIONS.md) governs scope; [ARCHITECTURE](ARCHITECTURE.md) governs responsibilities. The detailed capture/adoption and enforcement rules remain in [CHECKPOINT_PROTOCOL](CHECKPOINT_PROTOCOL.md) and [LAW_PROTOCOL](LAW_PROTOCOL.md). This document assigns their work and gates rather than duplicating those protocols.

Current source references below describe the audited working tree, including concurrent knowledge work. Names explicitly marked **proposed** are design vocabulary, not callable APIs or files promised to exist. Milestone acceptance still requires the implementation and evidence described below.

## The prerequisite belongs to knowledge and rumour

The [knowledge feature](../../../implemented/knowledge_and_rumor/README.md) completed its end-to-end release and independent M5 acceptance on 2026-09-06 under its own [plan](../../../implemented/knowledge_and_rumor/plan/README.md). [The M0 reconciliation](BASELINE_RECONCILIATION.md) records actual accepted interfaces, limits, retained provider evidence and source revision. Its prerequisite is satisfied; this investigation does not build a second fact/journal service. M0 itself remains partial because V03 renderer/full-stress evidence is unavailable.

The current source is already [knowledge/mod.rs](../../../../crates/cathedral-sim/src/knowledge/mod.rs), with separate catalog, source, mint and pollen modules; older references to `knowledge.rs` have drifted. The final prerequisite handoff is now recorded in BASELINE_RECONCILIATION. At each dependent milestone, re-read the accepted revision **and** relevant working-tree changes, then resolve these interfaces from actual source:

- Stable `FactId`/dense-key lifetime, `knowledge::holds`, creation/invalidation, direct versus claimed sources, garbled views and origin chains.
- Player learning and journal receipts, disclosure/utterance identity, event recipient ordering and prompt relevance/ignorance behavior.
- Durable references after gossip cooling or carrier eviction: M5 observations and M9 lodged accounts need retained support without expanding unsolicited prompt budgets.
- Hearsay intake, household refusals and other knowledge M5 consequences: M6/M11 must preserve their causal input while applying scoped physical/legal rules.
- Exportable knowledge state, deterministic indexes, retention and version requirements for M2. Reconcile the proposed `quest_phase` source stub without installing a quest-stage controller as the knowledge API.

Missing reusable contracts become explicitly assigned extensions to the accepted shared service. M0 closes only after the prerequisite evidence and reconciliation are complete. Read-only planning and the preliminary site survey can progress now; they are not permission to start dependent production work.

## Existing seams and future owners

Current [Engine](../../../../crates/cathedral-sim/src/engine.rs) owns the poll and command/message types; [Round](../../../../crates/cathedral-sim/src/round.rs) owns ordinary duties and movement decisions. Neither is yet the shared activity kernel proposed here. [PublicSnapshot](../../../../crates/cathedral-sim/src/snapshot.rs) deliberately omits private state. [LocalEngine](../../../../src/smart_actors/local_engine.rs) owns the host bridge, while [PlayerController](../../../../src/controller.rs) owns physical controller state. These existing boundaries anchor the work.

| Shared subsystem | First accountable producer | Handoff and downstream consumers |
|---|---|---|
| Facts, holdings, provenance, player learning and journal | Separate knowledge feature through its M5 | Accepted knowledge APIs to this M0; M2 persists them; M5/M9 reference them; M6/M8/M11 consume learned events; M10 extends their presentation. |
| Time, command identity, receipts, runtime generations | M1, extending Engine and host channels | M2 saves semantic/replay state; M3 fences replacement; every later action uses current validation and truthful results. |
| Single-operation resources and actor-duty ownership | M1, integrated with Round | Implemented in [M1d](evidence/m1d/README.md): `World.operations`, typed logical anchors, sibling-safe instance/step IDs, bounded claims/progress and a versioned NPC fixture adapter. M2 validates/persists continuations; M6/M7 register mechanical adapters; M8 composes them; M12 retains custody responsibility through failure. |
| Complete private checkpoint and compatibility manifest | M2, with each existing module exporting its state | M3 consumes a validated value. Every later state owner extends DTOs, validation and continuation evidence in its own milestone. |
| Files, host adoption and projection restoration | M3, backends plus host coordinator | Atomic slots and coherent restored physics/services/UI. Later owners provide projection rebuild/reset policies as well as sim DTOs. |
| Spaces, surfaces, routes, physical portals and revisions | M4, extending [nav](../../../../crates/cathedral-sim/src/nav/mod.rs), collision and bake | **Proposed** spatial service owns physical openings. M5 queries them; M6 supplies validated writers; M8/M12 traverse them; M14 authors the production district. |
| Perception, last-seen state and observation receipts | M5, replacing radius-only semantic checks | Knowledge receives supported observations; M6/M7 mint access/object observations; M8 demonstrations, M9 evidence and M12 pursuit consume them. |
| Ownership/capability registry, consent and mechanical access | M6 | **Proposed** registry/access services validate owners, delegates and scopes. M7 adds examiner capabilities, M9 reviewer capabilities, M11 legal capabilities. |
| Identified-object locations, containers and examination/custody receipts | M7, extending [items](../../../../crates/cathedral-sim/src/item.rs) and ordinary inventory services | M8 composes handling; M9 evaluates submitted receipts; M11/M12 perform scoped seizure/booking; M18 fulfils returns. M6's exact-key guarantee already works before this generalization. |
| Composite activities, appointments, strategies, agreements and post coverage | M8, extending the M1 arbiter | M9 supplies account/disclosure adapters; M11 supplies legal adapters and registry duties; M12 dispatches with real coverage; M15–M18 author behavior. |
| Accounts, submissions, evidence rules, findings and disclosure | M9 | M10 projects only learned/authorized records; M11 decides authority from submitted grounds; M8 agreements consume established disclosure events; content composes bounded rules. |
| Shared interaction controls and readable progress | M10, extending knowledge journal and existing host UI | Natural-language proposals and explicit controls reach the same domain service. M11/M12 add legal progress views; M14–M19 exercise production presentation. |
| Matters, legal authority, delivery and disposition | M11, migrating [notices](../../../../crates/cathedral-sim/src/notices.rs) and [custody](../../../../crates/cathedral-sim/src/custody.rs) | M6 receives real scoped mandates; M8 gains procedural adapters; M12 executes them; M18 performs independent money/property outcomes. |
| Dispatch, informed search, seizure, escort, handover and release work | M12, extending ordinary law and Round | Consumes M4–M8/M11; publishes execution and interruption receipts to those owners and M10. M13 proves reuse; real case content never substitutes a teleport. |
| Cross-module authoring validation and tools | M13; declarations/loaders begin with each earlier owner | M2 starts manifest identity; M4–M12 ship their own typed sections. M13 closes composition and documentation before M14–M18 use them. |

Every row includes migration of its touched base-game behavior, authored fixture data, deterministic negative tests, persistence and player-readable failure results. An owner cannot hand over a type declaration while leaving its only writer or restore path to an unnamed future milestone.

## Avoid dependency cycles

```mermaid
flowchart LR
  K[Knowledge end-to-end] --> M0
  M0 --> M1["M1 time + operation kernel"] --> M2 --> M3
  M3 --> M4["M4 space + physical portals"] --> M5["M5 perception"]
  M4 --> M6["M6 access + capability registry"]
  M5 --> M6 --> M7 --> M8["M8 activity composition"]
  M8 --> M9 --> M10 --> M11 --> M12 --> M13["M13 foundation acceptance"]
  M13 --> M14 --> M15 --> M16 --> M17 --> M18 --> M19
```

This is delivery order; the owner table records additional consumers. M5 tests open/closed portals using M4 fixture writers, so it need not wait for M6 permissions. M6/M7 perform real single operations through M1, so they need not wait for M8 meetings. M8 initially composes only shipped travel/wait/fixture/examine/transfer adapters. M9 and M11 later register their own typed adapters and integration tests. This extends the accepted composition service; it does not make M8 depend retroactively on those milestones. Missing adapters fail validation instead of completing placeholder steps.

## Boundary with the separate keys proposal

[Keys and locked places](../../../keys_and_locked_places.md) still says “SPEC ONLY — unimplemented.” Its milestone numbers are local to that feature. The following is proposed absorption, not a declaration that its M0–M5 have shipped.

| Required behavior | Owner in this roadmap | Acceptance boundary |
|---|---|---|
| Stable useful thresholds and real blocked/open routes | M4 physical source; M6 mechanical writers | Collision, navigation, sight and sound consume one portal revision. |
| Non-stackable exact keys, matching patterns, consent, opening, loan and exact return | M6 | Copied patterns can work with distinct IDs; wrong/pocketed/reserved keys fail. Permission remains independent of possession. A loan expires without teleporting its key. |
| Key/evidence identity through bodily handling and placed/container custody | M6 implements its immediate key guarantee; M7 generalizes location/lineage | Moving, swallowing or returning a key cannot manufacture another identity or satisfy a loan with a copy. |
| Keeper opening, appointments, accepted services and absence recovery | M6 simple operations; M8 composite duties/agreements | Ordinary schedules compete through one arbiter. An absent keeper produces actual cover, rescheduling or a usable alternative. |
| Witnessed access and scoped official search/entry | M5 observations, M6 consent/forced-entry mechanics, M11 authority, M12 execution | Owner-authorized forced entry proves M6 mechanics first; actual order-backed entry is accepted only with M11/M12 delivery and execution. |
| Durable access/grants/loans and clear controls | M6–M8 extend M2/M3; M10 completes interface parity | Save during a door operation or overdue loan; retain the exact current state and understandable next action. |

The broader keys feature remains pending: impression-making and locksmith commission economy, copying authorization/refusal content, owner-driven loss discovery and re-keying, four distinct acquisition routes across its own slice, a persistent 12–20-place access network, and a dedicated key-ring/learned-lock presentation pass. Pattern compatibility alone does not implement manufacturing a copy. Bounded lawful forced entry here does not implement general lockpicking, burglary or destructible buildings. Later keys work must consume these shipped services and reconcile its spec once, rather than introducing another door/loan store.

## Concrete acceptance cuts for M1–M3

Each subcut is reviewed before the next dependent cut. These are portions of one milestone owner's work, not permission for simultaneous agents.

| Cut | Concrete work product | Evidence handed to the next cut |
|---|---|---|
| M1a | Logical/host/calendar boundaries and ordinary input/physics watermark ordering | Never-pause traces; deadline and rate-change tests; a matching accepted physical sample without a save-only poll. |
| M1b | Shared IDs/receipts and validate-before-mutate paths | Duplicate delivery gives one effect; failed transfer/custody prerequisites leave domain state intact. |
| M1c | Whole-runtime generation fencing across every asynchronous channel | Old cognition, speech, status and presentation acknowledgements cannot affect the current world. |
| M1d | Minimal durable operation/resource/duty kernel | A non-quest operation reserves, progresses, interrupts/completes and cleans up; competing movement writers obey one arbiter. |
| M2a | Per-owner DTO inventory, explicit time/sentinel policies and manifest | Every authoritative field has a disposition; invalid values/references fail before hydration. |
| M2a1 | First review cut: strict replay/kernel DTOs, clock/time/manifest components and attached admission | Owner-local and active-reference validation, real array wire records, component fixtures and component CPU/allocation measurements. Complete owner/envelope acceptance remains M2a2+. |
| M2b | Ordinary-boundary capture and separate hydration path | No creation seeding, extra poll or half-transaction; canonical populated-state round trip. |
| M2c | Pending cognition/speech/night obligation restoration | Held results apply once; unfinished requests retain one retry obligation; newer inputs remain separate. |
| M2d | Fresh-engine continuation suite and capture profile | Control/restored runs agree under recorded inputs; intentional live retries prove obligation conservation instead of guessed prose equality. |
| M3a | Backend slot writer with bounded queues and durable publication | Fault-injected writes retain the previous valid slot; reverse completion cannot overwrite a newer save. |
| M3b | Candidate validation, host staging and complete adoption | Fresh-process resume restores controller, fixed residual, custody progress and projections together; preparation delay adds no game time. |
| M3c | Normal save/load controls and honest readable continuation | Save while moving, reading, recording or waiting; drafts remain unsent and old audio stays fenced. |
| M3d | Application integration and latency/memory acceptance | Hidden-window traces, failed-adoption recovery and measured capture/adoption/retirement costs meet M0 budgets. |

M1 hands M2 actual time/replay/operation contracts. M2 hands M3 a validated private value and host-continuation requirements. M3 hands every later owner working whole-city persistence. Follow the [field inventory](PERSISTENCE_INVENTORY.md) and checkpoint protocol for exhaustive coverage; these cuts are not a reduced save scope.

## Following gates and content handoff

| Milestones | Coherent subcuts and exit artifact |
|---|---|
| M4–M5 | Shared spatial source → supported traversal/bake agreement → all semantic perception consumers. Deliver a playable stacked-room fixture and supported observation traces. |
| M6–M7 | Capability/access declarations → validated door/key operations → generalized object locations → examinations/custody. Deliver consent-only and key-loan routes plus independently examinable objects. |
| M8–M10 | Activity/roster arbitration → strategies/agreements → confirmed accounts and bounded evidence rules → equivalent readable controls. Deliver a missed appointment and amended submission through ordinary UI. |
| M11–M12 | Authority/migration → real delivery → fair dispatch/search → physical seizure/escort/handover/release. Deliver an order issued with **no informed executor**, completed off stage without cognition; exercise recall, multiple grounds and obstruction. |
| M13 | Compose schemas/tools → independent scenario review → full foundation acceptance. Deliver real commands, author guide, fixture packs and continuation/performance evidence. |
| M14–M15 | Finish district and spatial measurements → bind cast/history/documents. Install basic scheduled packet retrieval in M15 so M16 already runs in a changing world. |
| M16–M18 | Core/public investigation → private/hush/late/reopening alternatives → lasting ordinary-city fulfilment. Separate clearance, assault, property, custody and payment outcomes. |
| M19 | Full deterministic, spatial, provider, durability and human acceptance; reconcile final documents and actual feature status. |

**M13 is a hard gate before M14.** Its independent workshop, collection, pursuit and agreement exercises must use production services with different evidence structures, including no offender and changed discovery order. They must remain playable with the player absent, cognition unavailable and saves across important transitions. Real-case IDs cannot be hidden prerequisites. M0 may survey Tallage risk; finished district authoring and incident installation wait for this demonstrated foundation.

## Sequential execution and truthful bookkeeping

Follow [features/AGENTS.md](../../../AGENTS.md): ESPFEIT, one fresh-context milestone agent at a time. Work directly in the current tree; do not create branches or worktrees. Before dispatch, inspect status and implemented records, preserve unrelated/concurrent changes, and read the accepted predecessor's actual interfaces.

Give the agent its milestone, applicable instructions, decisions, relevant protocol sections, allowed file scope and predecessor evidence. Require source-backed changes, focused tests, appropriate repository checks and a bounded handoff containing changed interfaces, schema/content versions, acceptance commands/results, captures and unresolved limitations. Run Bevy evidence only with `CATHEDRAL_HEADLESS=1`. The coordinator reviews that handoff before starting the next agent; a discovered predecessor defect returns to its owner before dependent work proceeds.

Update the milestone and plan README status using absolute dates and actual evidence. Partial subcuts remain explicitly partial; missing human/provider/visual acceptance stays pending. A planning document, new API name or passing unrelated suite is never “implemented.” Move the whole feature into `features/implemented/` and update `features/order.json` only when its full scope is delivered. Record absorbed keys behavior as a tested subset when it actually ships; keep the broader proposal pending. Preserve historical implemented records and their original coordinates.


## M2a2 owner handoff — 2026-09-08

The character/item/transform/reference-backbone owner supplies strict private
components, mandatory fields, aggregate admission and behavior witnesses under
[evidence/m2a2](evidence/m2a2/README.md). The [coverage ledger](evidence/m2a2/OWNER_COVERAGE.md)
keeps complete World/Engine/host and all remaining private owners explicitly
pending. Proposed M2a3 is Round/residents/production/household/road-party authority,
with saved planner bindings and exact later projection contracts; do not start
it until the coordinator accepts and commits M2a2. Coordinator owns final release
measurements/review and commit; no complete-save or host-frame acceptance follows
from the component handoff. Historical M2a1 reference evidence remains unchanged.


## M2a3 owner handoff — 2026-09-08

M2a3 covers complete private Round/resident/market/production/household/road-party state against the accepted backbone. See [coverage](evidence/m2a3/OWNER_COVERAGE.md) and [admission](evidence/m2a3/ADMISSION.md). The milestone owner supplies source, fixtures, raw test/layout logs and source hashes; coordinator owns independent public-boundary tests, source review, release component and matched ordinary-poll measurements, staging and commit. No M2a4 begins before M2a3 acceptance/commit.

Whole-envelope/Running/retiring coexistence, remaining World/Engine/private owners and CPU-safe effective clock/accounting horizons are explicit gates before M2b. Component leases and successful standalone measurements do not establish shared full-save admission. Existing M2a1/M2a2 host-frame failures and preserved reference executables remain binding evidence; no renderer/full-stress reprobe accompanies this cut.

## M2a4 climate owner handoff — 2026-09-08

M2a4 supplies private weather, sampled World climate/context, live/initial clocks and owed Engine bell authority. [Coverage](evidence/m2a4/OWNER_COVERAGE.md) identifies exact fields and still-pending temporal owners; [admission](evidence/m2a4/ADMISSION.md) separates fixed sampling scratch from lexical/cohort charges. The milestone owner supplies implementation, fixtures, raw test/layout/diagnostic logs and final source hashes. Coordinator owns the independent public boundary tests, source audits, release component measurements, staging and commit. Source/cargo cession is explicit after final verification; no M2a5 starts before acceptance/commit.

No ordinary hot-path implementation changes are required by this component. The forced-storm release probe cannot bound scheduled-sample p99 or the whole save. Historical M2a1/M2a2 host-frame failures, the naive M2a2+Round cohort overage, full effective-rate/calendar/accounting bounds, other owners and adoption remain open gates. Initial config-clock provenance is preserved separately from the mutable clock; no other temporal owner can be omitted merely because this cut validates the climate boundary.
