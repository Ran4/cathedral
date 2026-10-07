Status: Approved by the developer on 2026-10-07 (D08): the revised R1–R11 order, the default answer to every question Q1–Q13, and proposed changes 1–8. R1 (M1) and R2 (M2) were implemented 2026-10-04. All eight changes were applied on 2026-10-07. Change 6: the capped worker no longer hashes `component_input_sources.py` and accepts any `<dirs>/owner/run_capped.py` runner; it was restarted and verified.

# Plan review — what to keep of Astra's plan

Seven independent reviewers, each with an adversarial verifier that re-checked their code claims, read the plan
against `DECISIONS.md` (D01–D07, binding) and the current code. Then a synthesis and a decision critic ran. The
conclusion:

**This is a trim, not a restructure.** The plan's direction stands:
- reusable systems, each proven on a non-case fixture;
- then the full quest;
- persistence early.

What changes is size and order. About half the plan is machinery no decision asks for, and the quest becomes
playable far earlier in the order. The 20 milestones become 11, and the first attempt's ~140k lines become roughly
12–18k.

## What is wrong with the plan

1. **Invented limits treated as requirements.** `RUNTIME_BUDGETS.md` sets about 25 numbers, and no decision asks
   for any of them. They include:
   - ≤2 ms p99 per frame for save/load;
   - a 1 GiB ceiling;
   - a 4,096-entry replay window;
   - 64/128 MiB payloads;
   - archive sizes of 16,384/65,536;
   - frame percentiles at 1080p;
   - a 20,000-resident stress gate.

   The only cost statement the developer made is D05's "disk work outside the simulation pump". These numbers are
   what produced the memory-proof spiral (LESSONS).
2. **Non-problems.**
   - D05 ("never pause") is already true. Nothing in `src/` gates `Time<Virtual>` or the pump: `pump_local_engine`
     runs unconditionally (`src/smart_actors/mod.rs:729`), and overlays only zero player input
     (`src/controller.rs:663`).
   - M1a's "accepted time / wall-time debt" would replace the 100 ms hitch clamp, which is a deliberate guard.
   - M1b's exactly-once command ledger guards an in-process crossbeam channel that never redelivers.
   - M1d's operation/duty kernel had zero production callers the first time.
3. **Systems far ahead of use.** M4–M13 build generic layers before any case exists: a strategy DSL, an
   entitlement economy, observation archives, a capability registry, dispatch fairness queues, intake reservations,
   rosters, mandates and tombstones. The quest needs a fraction of each.
4. **Order.** The first time a human can play anything is M16, the 16th milestone. The district, the quest's
   largest risk, comes 15th. The real-app save/load swap, the risk LESSONS names, comes 3rd but stays untested in
   the real game.
5. **Process overhead.** It includes:
   - 78 scenarios, of which 43 are machinery invariants and 6 are measurement process (only 29 test what a player
     would notice);
   - 40 generated requirements, of which about 16 restate a decision;
   - per-subcut handoff gates;
   - plan catalog regeneration and a plan validator that breaks on every nav rebake;
   - committed raw evidence;
   - an M13 "reusable-system gate" that repeats fixtures the systems milestones already ship.

## What is right and kept

- D01–D07, the case itself (`CASE_CONTRACT.md`, `PLAYER_EXPERIENCE.md`) and the GDD routes: Averil's
  sponsorship, E13 surveillance, Warin's cart favour and the not-supplied admission detail are all in the
  manuscript and stay, in reduced form.
- M0's real outputs:
  - the knowledge interface table, the law role map and the Tallage options (`BASELINE_RECONCILIATION.md`);
  - the gap tables in `SOURCE_AUDIT.md` §C–§F;
  - `PERSISTENCE_INVENTORY.md`, as a checklist rather than a gate.
- The checkpoint analysis (LESSONS): derive serde on the real types, exhaustive construction, the in-flight LLM
  rules and lockstep tests.

## Verdict per milestone

| Old | Verdict | Why |
|---|---|---|
| M0 Baseline | **close** | Its open gate (a 1080p reference-renderer trace and the 20k stress reconciliation) comes from no decision. "No GPU" was Astra's sandbox, since this machine has an RTX 4070. Placing 9,072 of 20,000 is documented crowd-knob behaviour. |
| M1 Live time / committed actions | **shrink → R1** | D05 already holds. What remains are three real bugs, about 90 lines: (1) T-key office cursors re-read through the new clock rate (`round.rs:6650`, `night.rs:437`); (2) `take_into_charge` seizes before the fallible `route_budget` (`actions.rs:3655` vs `:3669`); (3) unbounded LLM response bodies (`llm.rs:511,522`). Cut the debt policy, ledger, replay window, due-expiry boundary and kernel. The generation fence moves to save/load. Receipt states move to R4, where D04's UI needs them. |
| M2 Simulation checkpoints | **keep, ~10× smaller → R2** | D06 needs it early. Use derives on the real types, `#[serde(skip)]` + rebind, `Engine::restore` that never seeds, one host `time_offset`, and a lockstep harness. No DTO mirrors, manifests, admission budgets or physical-sample protocol. Add a thin real-app proof: a quicksave key, an off-thread slot write and boot-from-slot. |
| M3 Save-anywhere | **split**: the file and boot slice → R2; load-during-play → R3 | LESSONS: test the ECS swap in the real app early. In-process load has an open question (Q5). Cut the ≤2 ms/frame budget, retention cohorts and the slot publication protocol; use tmp+fsync+rename plus a `.bak`. |
| M4 Layered navigation | **shrink, merge → R5 Rooms** | The quest needs a few authored upper rooms that people walk. The gaps are real: the player controller has no step-up, nav is 2D at `WALK_Y` (about 60 sites), and the escort ignores y (`custody.rs:608`). Cut certification, the shared spatial crate and crossing yielding. |
| M5 Perception | **merge → R5 / R10** | One space/portal filter must ship with rooms, or street NPCs witness the loft through floors. Observation receipts already exist as minted facts. Cut the archive tier and swept continuity. The host line-of-sight ray goes to R10 (E13). |
| M6 Access | **shrink → R5 / R6 / R7** | No door or lock state exists. Doors, locks and pattern keys go to R5; grants with expiry go to R6; authorised opening of locked premises (D07) goes to R7. Cut loans, copies and the capability registry. |
| M7 Objects | **shrink → R6** | Items exist only in hands today. Add containers, take/put/look-in, documents with a read panel and a data examination table. Cut lineage, presentation descriptors and signed receipts. |
| M8 Activities / appointments | **shrink, spread** | Rounds, the rung ladder and `TravelIntent` already exist. Add a dated review trigger (R4), a dated errand also used for demonstrations (R6), and agreements for hush, restitution and the favour (R7). Cut the activity kernel, strategy DSL and entitlement economy. |
| M9 Inquiries / evidence | **reorder → R4, first playable** | Provenance already exists (`Held{hops,from}`). Missing: a deterministic way to ask a witness, evidence the player cannot lose (6 holdings / 64 receipts today), and a bundle evaluator. Use data rules, not a typed binding engine. |
| M10 Conversation / controls | **shrink, spread** | The request-status list (D04's states), lodge panel, Case tab, drafts and refusal reasons go to R4; the read panel to R6; the agreement panel to R7. Cut dictation purpose binding and the record delta protocol. |
| M11 Orders | **shrink → R7** | Extend `WardNotice`/`CustodyRecord`; don't add nine record types. Real gaps: holds capped at 240/360 real seconds; hearsay reaches a warrant through `summon` → `issue_warrants`; the single `is_law` gate lets the victim and a witness release prisoners. |
| M12 Enforcement | **shrink → R7** | Pursuit is the one genuinely missing D07 core: nobody searches for a warrant subject today. Build a small service on `take_into_charge`. Cut fairness queues, intake reservations and rosters. |
| M13 Authoring gate | **cut** | Its scenarios repeat earlier fixtures. Keep only a case-pack loader that refuses bad cross-references (R4), plus a grep check that no case IDs appear in engine code. |
| M14 Tallage district | **reorder → R8**, straight after the systems | It is the largest risk and the first real consumer of rooms, doors and containers. Replace the certification maths with measured routes, including player jumps and the Cut, plus sightline and acoustic checks. |
| M15 Incident and cast | **merge → R9** | Content has nothing to observe until it can be played. Warin is held at the Tallage station with grounds, not via the Stone House "prisoner" circumstance. |
| M16 Core investigation | **keep → R9** | The D01 review point, with a developer playtest. It absorbs M15 and M18's core (release, the guard-executed arrest, packet custody). |
| M17 Alternatives | **shrink → R10** | Every GDD route stays. Cut disclosure-history diagnostics, relay windows and the adversarial matrix. |
| M18 Consequences | **merge → R9 / R10** | Almost entirely reuse of custody, knowledge and the Night Office. |
| M19 Acceptance | **shrink → R11** | Keep developer playtests, one headless drive of the main path, a multi-day fake run, perf_suite at default and +2,000 residents, and lockstep scenarios at the decisive boundaries. Drop frozen budgets, the 20k gate, the accessibility matrix and the release record. |

## Revised milestones

Every milestone ships a non-case dev fixture for its new behaviour (D01) and at least one save→restore lockstep
scenario for its new state (D06).

| # | Milestone | From | Observable result | Rough size |
|---|---|---|---|---|
| R1 | **Baseline fixes** | M1 (real bugs only) | Pressing T across Kindling gives exactly one restock and settlement. A seize with no route leaves nobody in custody. A huge provider reply fails cleanly. | ~90 + tests |
| R2 | **Simulation checkpoints + boot from a save** | M2 + M3's file slice | A save→restore engine continues in lockstep with an uninterrupted one. Quicksave in the game, then start a new process from the slot, and the city continues. | 1.2–1.6k + ~1k tests |
| R3 | **Load during play** | M3 rest | Quickload in the running game swaps the world. Old callbacks are inert. HUD, puppets and player are reset. | 300–600 |
| R4 | **Accounts, inquiries and findings** (first playable dev mini-case) | M9 + M10 core + M8 review trigger + M13 loader | Question two witnesses in a dev case, lodge their accounts, and a dated review (attended or missed) gives a partial or full finding. | 1.6–2.2k |
| R5 | **Rooms: floors, doors, locks, senses** | M4 + M5 + M6 physical | The player and an NPC climb a stair. An NPC reroutes around or unlocks a locked door. A street NPC no longer overhears upstairs. | 1.8–2.6k |
| R6 | **Things in places** | M7 + grants + M8 errands | Find an item on a shelf, read a document, get consent to search, and watch a keeper move an item at Lamplight unless warned. | 1.5–2k |
| R7 | **Orders, enforcement, agreements** | M11 + M12 + M8 agreements | A finding issues an order. A guard learns of it, searches, seizes, escorts and commits the subject, and the hold lasts until review. An agreement can be kept or breached. | 1.2–1.7k |
| R8 | **The Tallage district** | M14 | Walk the district: the counter loses sight of the yard, and the public detour is measurably longer than the blind interval. | geometry + bake |
| R9 | **The core case** (D01 review point) | M15 + M16 + M18 core | The quest plays from a fresh world to Warin's release and Corin's guard-executed arrest. | 0.6–1.2k + content |
| R10 | **Alternatives and aftermath** | M17 + M18 rest + LOS ray | Every GDD route is reachable, and consequences persist on return visits. | 1.2–2k |
| R11 | **Playtest and release** | M19 (shrunk) | Developer playtests, a main-path drive run, a multi-day fake run and perf at default and +2,000 residents. | tests |

Notes the decision critic added (to carry into those milestone files when they are rewritten):

- **R4**
  - Evidence records extend `knowledge.player_learned` with a pin and a `FactId`. The knowledge README says quests
    own no receipt store of their own.
  - Missing-link text names only what the lodged bundle lacks, never unlodged evidence or a suspect (D04).
  - One NPC verb (e.g. `recount {fact}`, checked against `holds()`) makes a spoken answer mint the same receipt as
    the explicit "ask about" control (D04).
  - The request list shows an NPC's "I will…" differently from a committed action.
  - Open panels show change notices, and missed events land in the Case tab (D05).
  - Wire `fact_packs` into the game host (`local_engine.rs:440` passes an empty list today).
- **R7** has an explicit order-state enum for D07's ten states and acceptance checks for its eight named situations.
- **R8** re-pins places, homes, shelters and `resident_places` after the rebake, and re-assigns household doors.
- **R2/R9** never run fact/case seeding on restore (D02).

## Doc changes made now (only so the first two steps aren't distorted)

- `M1_live_time_and_transactions.md` is rewritten as **M1 — Baseline fixes** (R1).
- `M2_simulation_checkpoints.md` is rewritten as **M2 — Simulation checkpoints and boot-from-save** (R2).
- `M3_save_load_application.md` has a banner saying what moved into M2 and what remains.
- `RUNTIME_BUDGETS.md` is marked superseded.
- `CHECKPOINT_PROTOCOL.md`, `PERSISTENCE_INVENTORY.md`, `ARCHITECTURE.md` (§2 debt, §4 ledger), `OWNERSHIP_AND_GATES.md`
  and `VERIFICATION.md` have one-line banners saying the M1/M2 files govern.
- `ARCHITECTURE.md` §12 now states D06's default: loading resumes the saved simulation time.
- `M0_baseline_and_prerequisites.md` and the README status line: M0 closed.
- `evidence/component_input_sources.py` has a one-line stub. `scripts/capped_verification/worker.py` hashes it on
  every submit and claim, and its deletion in the reset made every submission fail.

## Proposed changes for your approval (not made)

1. Replace the README's 20-row table with R1–R11, and rewrite or rename the M3–M19 files to match.
2. Archive `RUNTIME_BUDGETS.md`, `REQUIREMENTS.md`, `REVIEW_LOG.md` and `BASELINE_REVIEW.md`.
   - Requirements become D01–D07 plus a one-page case-outcome checklist from `CASE_CONTRACT.md`.
   - Archive `RUNTIME_BUDGETS.md` once R2 has logged real save/load timings.
3. Keep `VERIFICATION.md` only as a source of edge cases. Each milestone file instead gets 3–8 player-visible
   acceptance checks. Keep the case playthroughs V64–V74.
4. Trim the remaining reference docs:
   - `ARCHITECTURE.md` to §1, §3, §6, §7, §8, §9 and §12;
   - `RISKS.md` to about 10 rows, with F01 ("the foundation becomes a universal engine before anything is
     playable") on top;
   - `OWNERSHIP_AND_GATES.md` to the keys-proposal boundary and the no-case-IDs rule;
   - `SPATIAL_PROOF_PROTOCOL.md` replaced by a design rule plus one route-measurement script;
   - `LAW_PROTOCOL.md` replaced by R7's scope.
5. Retire `evidence/plan_catalog.py`, `validate_plan.py`, `inventory_fields.py` and `run_baseline.py`, and
   `crates/cathedral-backends/examples/alibi_baseline.rs`, which duplicates `cathedral-headless --extra-ambient`.
   Stop committing raw evidence.
6. Fix the capped worker properly: drop `component_input_sources.py` from `identities()` and decouple
   `RUNNER_PATTERN` from this plan's path, so the folder can be archived. This needs a worker restart, which is
   your call.
7. `CASE_CONTRACT.md` edits:
   - the late wallet-then-desk route becomes core;
   - the hoist interval gets an independent root (a visible hoist cycle plus the tenders' testimony);
   - Warin is held at the Tallage station with grounds and a game-time expiry;
   - sponsorship becomes a release effect;
   - Warin's favour becomes a one-use agreement.
8. Code fixes to schedule in the named milestones:
   - `GutEntry` keeps the `ItemId` of non-stackable items (R5);
   - the "heard by N" badge counts only listeners who can see the speaker (R4);
   - LLM `raise_notice` without first-hand knowledge stays hearsay, and hearsay cannot climb to an executable
     warrant (R7);
   - release and settle of grounded holds are limited to a finding or the issuer (R7);
   - headless late-start fixtures use poll steps of 0.4 s or less, because the default 18 s watch-clock step drops
     walks.

## Questions for you

The default in the revised plan is given first in each case. None of them blocks R1 or R2.

**Decision conflicts**

- **Q1 (D01) — just-in-time systems.** Must every generic system be finished on dev fixtures before any case
  authoring (the default)? Or may a system be built together with the first quest step that uses it, still
  generic, data-driven and free of case IDs, and with its own dev-fixture test? The second saves fixture work and
  gets the case playable sooner, but it bends "systems first".
- **Q2 (D01) — Rooms fixture.** May R5's multi-storey fixture be a geometry-only Tallage greybox (no cast, evidence
  or history)? That retires the biggest site risk about four milestones earlier and avoids a throwaway building.
  Default: a separate dev building, with the Tallage at R8.
- **Q3 (D01) — early real-case slice.** Do you want a talk-only Warin-clearance slice with the real cast right after
  R4, to test whether the mystery is fun before rooms exist? Default: no; dev mini-cases until R9.
- **Q4 (D03 / preamble "private upper route") — must the route be upstairs?** Is "upper" essential, or is any short
  private connection between the two properties acceptable if the logical relationship holds? A ground-level
  connection would shrink R5 a lot, since walkable upper floors for NPCs mean y-aware nav, a player step-up and
  stair escorts. Default: keep it upper.
- **Q5 (D06) — how Load works during play.** Must Load swap worlds inside the running game (R3), or is relaunching
  the game straight into the slot acceptable? R2 builds boot-from-slot either way. Note that in-process load with
  local speech restarts the speech worker processes, unless the speech services are carried across.
- **Q6 (D05) — save hitch.** Saving clones the sim on the main thread at a poll boundary, and encoding and disk
  writing happen on a worker. If the clone still produces a visible hitch, is a one-off sub-quarter-second hitch on
  an explicit save acceptable? Under the kept 100 ms clamp that frame loses up to that much city time. Default:
  keep the clamp, measure and report.
- **Q7 (D07) — orders vs warrants.** Should the new order *become* the warrant rung (every warrant is pursued, and
  hearsay can no longer reach a warrant)? Or should orders sit beside warrants, with pursuit for orders only? The
  second leans toward the "separate quest police service" that D07 forbids. Default: the first.
- **Q8 (D07) — hold length.** Ordinary custody frees people after 240 s at a station and 360 s at the Stone House,
  with a cap of 4 arrests city-wide. Should only order- or finding-grounded holds run on game-day expiry (default),
  or should the ceiling change for everyone?
- **Q9 (D07) — recalls.** Does a recalled or corrected order stop every officer at once, or travel to officers the
  way the order did, so a distant guard can still act on stale information? Default: it travels like the order
  (about 30 lines).
- **Q10 (D02/D05) — the hoist interval.** Must the player be able to watch and time the hoist interval in the world
  (a visible cycle plus a stand-in demonstration)? Or is the tenders' independent testimony enough? Default: both.
- **Q11 (D03) — route proof.** Are measured routes (nav, jumps, roofs, the Cut) plus your own attempts to beat
  them enough? Or do you want an exhaustive proof that no collision-legal route is faster? Default: measured.
- **Q12 (D01) — the four GDD side routes.** Confirm that sponsorship, E13 surveillance, the cart favour and the
  not-supplied admission detail stay in reduced form. Default: yes.
- **Q13 (D05) — talking vs obligations.** Which obligations break a player conversation after one
  excuse-yourself turn: scheduled round legs, appointments and duties (default), or only appointments and duties?
  Dated `TravelIntent`s already escape the hold.

**Design-rule questions (not decision conflicts)**

- `knowledge/source.rs` forbids `Serialize` on `FactSource`, to keep provenance out of prompts, projections and
  logs. R2 serializes it through a private save-only `#[serde(with)]` module, so the type stays sealed. A save file
  necessarily contains the whole hidden world anyway: who holds the packet and where everyone is. Default: no
  spoiler obfuscation of save files.
