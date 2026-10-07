Status: M0 closed; M1 and M2 implemented (2026-10-04); PLAN_REVIEW accepted 2026-10-07 (D08) and its changes 1–8 applied; next: M3, load during play. A first attempt at M1–M6 (2026-09-07 → 10-02) was removed from history on 2026-10-03; see [LESSONS_FROM_THE_FIRST_ATTEMPT.md](LESSONS_FROM_THE_FIRST_ATTEMPT.md).

# An Alibi in Stone — systems first, then the full quest

Build the machinery for investigations that happen inside a living city, then use it to deliver the complete quest. People keep their own time. Physical evidence has a location and a history. A finding may justify an order; available guards must then carry it out. The player can save at any moment and resume a coherent world.

The [authoritative decisions](DECISIONS.md) override the GDD where they differ. In particular, there is **no quest mode and no pause for reading, talking or selecting evidence**. A larger district redesign is authorised. Full world save/load and reusable systems are required parts of delivery. D08 adopts the [plan review](PLAN_REVIEW.md): the order below, its doc changes, and the default answer to each of its questions.

For a design review, read the [case contract](CASE_CONTRACT.md), its one-page [checklist](CASE_CHECKLIST.md) and [player experience](PLAYER_EXPERIENCE.md). For implementation, read the next milestone file, the decisions and LESSONS.

## Delivery order

Each milestone has an observable result a player could notice. M1–M7 build the reusable systems on dev fixtures (D01, Q1); M8–M10 author the district and the case on them; M11 is playtesting and release. The R numbers are the [plan review's](PLAN_REVIEW.md); the files keep the M numbers.

| Milestone | From (old plan) | Observable result | Rough size |
|---|---|---|---|
| [M0 — Baseline](M0_baseline_and_prerequisites.md) (closed) | M0 | Knowledge interfaces, law role map, Tallage options, persistence inventory | — |
| [M1 — Baseline fixes](M1_live_time_and_transactions.md) (R1, done) | M1, real bugs only | Pressing T across Kindling gives exactly one restock and settlement. A seize with no route leaves nobody in custody. A huge provider reply fails cleanly. | ~90 + tests |
| [M2 — Simulation checkpoints + boot from a save](M2_simulation_checkpoints.md) (R2, done) | M2 + M3's file slice | A save→restore engine continues in lockstep with an uninterrupted one. Quicksave in the game, then start a new process from the slot, and the city continues. | 1.2–1.6k + ~1k tests |
| [M3 — Load during play](M3_load_during_play.md) (R3) | the rest of M3 | Quickload in the running game swaps the world. Old callbacks are inert. HUD, puppets and player are reset. | 300–600 |
| [M4 — Accounts, inquiries and findings](M4_accounts_inquiries_findings.md) (R4, first playable) | M9 + M10 core + M8 review trigger + M13 loader | Question two witnesses in a dev case, lodge their accounts, and a dated review (attended or missed) gives a partial or full finding. | 1.6–2.2k |
| [M5 — Rooms: floors, doors, locks, senses](M5_rooms.md) (R5) | M4 + M5 + M6 physical | The player and an NPC climb a stair. An NPC reroutes around or unlocks a locked door. A street NPC no longer overhears upstairs. | 1.8–2.6k |
| [M6 — Things in places](M6_things_in_places.md) (R6) | M7 + grants + M8 errands | Find an item on a shelf, read a document, get consent to search, and watch a keeper move an item at Lamplight unless warned. | 1.5–2k |
| [M7 — Orders, enforcement, agreements](M7_orders_enforcement_agreements.md) (R7) | M11 + M12 + M8 agreements | A finding issues an order. A guard learns of it, searches, seizes, escorts and commits the subject, and the hold lasts until review. An agreement can be kept or breached. | 1.2–1.7k |
| [M8 — The Tallage district](M8_tallage_district.md) (R8) | M14 | Walk the district: the counter loses sight of the yard, and the public detour is measurably longer than the blind interval. | geometry + bake |
| [M9 — The core case](M9_core_case.md) (R9, D01 review point) | M15 + M16 + M18 core | The quest plays from a fresh world to Warin's release and Corin's guard-executed arrest. | 0.6–1.2k + content |
| [M10 — Alternatives and aftermath](M10_alternatives_and_aftermath.md) (R10) | M17 + M18 rest + LOS ray | Every GDD route is reachable, and consequences persist on return visits. | 1.2–2k |
| [M11 — Playtest and release](M11_playtest_and_release.md) (R11) | M19, shrunk | Developer playtests, a main-path drive run, a multi-day fake run and perf at default and +2,000 residents. | tests |

The superseded milestone files (old M3–M19) are in [archive/](archive/), as edge-case sources only.

## Rules for every milestone

- One milestone agent at a time with a fresh context and a bounded handoff (ESPFEIT, `features/AGENTS.md`). Re-read the shipped interfaces at each start; don't build against speculative outputs.
- Ship a non-case dev fixture for the new behaviour (D01) and at least one save→restore lockstep scenario for the new state (D06).
- Each milestone file lists 3–8 player-visible acceptance checks; those are its gate.
- No case IDs in engine code. Check with the grep in [OWNERSHIP_AND_GATES](OWNERSHIP_AND_GATES.md).
- Headless late-start fixtures use poll steps of 0.4 s or less: the default 18 s watch-clock step drops walks.
- Don't commit raw evidence (logs, traces, screenshots, test binaries). A milestone's status line states what was verified.
- Builds, tests and game runs go through the capped worker (`CLAUDE.md`), and game runs use `CATHEDRAL_HEADLESS=1`.

## Existing work and ownership

Knowledge and rumour is a separate shared feature, [implemented and accepted on 2026-09-06](../../../implemented/knowledge_and_rumor/README.md). [BASELINE_RECONCILIATION](BASELINE_RECONCILIATION.md) records its actual fact, holder, receipt and journal interfaces. This plan extends that service; it does not build a second fact store.

Law, movement, items, speech, weather and ordinary NPC rounds already exist. Their implementation is the starting point, including fixes and extensions required by this plan. Historical files under `features/implemented/` are records and must not be rewritten to pretend the new work has shipped.

## Non-negotiable acceptance conditions

1. Reusable foundations pass independent development scenarios before real-case authoring starts.
2. All ordinary simulation continues while the player reads, types, speaks, waits for a reply or uses an in-game menu.
3. Language never creates physical proof or commits an action outside the simulation's current permissions and preconditions.
4. Possibility, identification, theft, assault, recovery and release are separate findings with explicit supporting evidence.
5. An order is not an arrest. Guards must learn, travel, encounter and act; scene presence is not required for their work to progress.
6. Save/load covers the whole authoritative city and every new system. Old asynchronous results cannot mutate a newly loaded world.
7. Navigation, collision, access, perception and visual geometry describe the same spaces. Legal alternative routes remain legal.
8. The player can miss events, decline disclosure, clear only Warin or return later without corrupting the mystery.
9. No important consequence relies on an LLM getting an idle turn near the player.
10. Completion requires implementation behaviour and developer play, not only a passing authored walkthrough.

## Documents

| Document | Purpose |
|---|---|
| [Decisions](DECISIONS.md) | The user's authoritative choices (D01–D08) |
| [Plan review](PLAN_REVIEW.md) | Why the plan is shaped this way: per-milestone verdicts, the questions and their defaults |
| [Lessons](LESSONS_FROM_THE_FIRST_ATTEMPT.md) | What went wrong in the removed first attempt |
| [Case contract](CASE_CONTRACT.md) and [checklist](CASE_CHECKLIST.md) | The complete quest and its outcomes |
| [Player experience](PLAYER_EXPERIENCE.md) | Concrete play moments and playtest revision criteria |
| [Case playthroughs](VERIFICATION.md) | V64–V74, the real-case scenarios for M9/M10 |
| [Architecture](ARCHITECTURE.md) | Authority, receipts, statements, findings, activities, orders and compatibility |
| [Ownership](OWNERSHIP_AND_GATES.md) | The no-case-IDs rule and the boundary with the keys proposal |
| [Risks](RISKS.md) | The failures that can sink the delivery |
| [Baseline reconciliation](BASELINE_RECONCILIATION.md), [source audit](SOURCE_AUDIT.md) | M0's interface tables and gap analysis |
| [Persistence inventory](PERSISTENCE_INVENTORY.md), [checkpoint protocol](CHECKPOINT_PROTOCOL.md) | Field checklist and pending-work rules for save/load (M2 governs) |
| [archive/](archive/) | Superseded milestone files, budgets, requirements, protocols, review log and the full 78-scenario list |

## The Tallage illustration

The [existing Tallage illustration](evidence/tallage_existing.png) is generated from current cadastral/navigation data, with [source hashes and coordinates](evidence/site_audit.json). It is not a screenshot or a proposal for new interiors. Regenerate it after its source geometry changes:

```sh
uv run --cache-dir /tmp/cathedral-gdd-uv evidence/render_site_audit.py
```
