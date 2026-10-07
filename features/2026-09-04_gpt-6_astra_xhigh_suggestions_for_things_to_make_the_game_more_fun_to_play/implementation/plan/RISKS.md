Status: Open risks, trimmed 2026-10-07 to the ones that can sink the delivery (PLAN_REVIEW item 4). The full 2026-09-05 table is in git history.

# Risks that can invalidate the delivery

A risk closes when its owning milestone's player-visible checks pass, not when a document describes a mitigation.

| ID | Failure and early signal | Owner and response |
|---|---|---|
| F01 | **The foundation becomes a universal engine before anything is playable.** Generic layers (DSLs, registries, economies, archives) grow ahead of any case that uses them. | Every milestone. Build only what its observable result needs; [M4](M4_accounts_inquiries_findings.md) is the first playable dev case. No case-ID branches, and no expansion beyond demonstrated capabilities. |
| S01 | A load restores visible people but reseeds rounds, work, knowledge or the incident, or loses a new subsystem's state. | [M2](M2_simulation_checkpoints.md) (restore never seeds). Every later milestone adds its state to the checkpoint and ships a save→restore lockstep scenario. |
| S02 | After an in-game load, a late reply or voice from the replaced world speaks or acts in the new one; or the load leaves one world's HUD, puppets or player beside another world's city. | [M3](M3_load_during_play.md): a fresh cognition channel per adopted engine, speech services carried across with stale results ignored, and a full host reset tested in the real app. |
| G01 | Upstairs works only in flight, or nav, collision and the escort disagree about floors. | [M5](M5_rooms.md) proves a separate dev building: player step-up, y-aware nav, stair escorts. |
| G02 | The claimed public detour misses a jump, a roof, the Cut or door/departure time, so the case's timing is false. | [M8](M8_tallage_district.md) measures routes (nav, jumps, roofs, the Cut) with one script, and the developer tries to beat them (Q11). |
| P01 | A street NPC hears or sees through a floor; an endpoint sample hides an exchange that continuous observation should have broken. | M5 ships one space/portal filter with rooms; [M10](M10_alternatives_and_aftermath.md) adds the host line-of-sight ray for E13. |
| E01 | Valid clues from different incidents combine into a false finding, or missing-link text reveals unlodged evidence or a suspect. | M4's bundle evaluator binds actor/object/incident/interval, and names only what the lodged bundle lacks (D04). |
| K01 | A referenced clue falls out of the player's holdings (6 holdings / 64 receipts today) and the case becomes unwinnable. | M4 pins evidence records in `knowledge.player_learned` with a `FactId`; quests own no receipt store of their own. |
| L01 | An order never reaches a guard, or "arrest" is a teleport: nobody actually searches, seizes and escorts. | [M7](M7_orders_enforcement_agreements.md): pursuit on `take_into_charge`, using only known leads, with D07's states explicit. |
| C01 | The late route secretly needs the evidence it is supposed to unlock, or a slow reader who misses the review loses every continuation. | [M9](M9_core_case.md) ships the late wallet-then-desk route as core; M4/M9 record missed reviews and keep later recovery open. |
| U01 | Players cannot tell a spoken intention from a pending request or a completed action. | M4's request-status list (D04's states); NPC "I will…" shown differently from a committed action. |
| F02 | Correct mechanics feel like collecting paperwork; the upper route feels contrived. | M8 site review with the quest disabled; [M11](M11_playtest_and_release.md) developer playtests drive revision. |

## Order and failures

Retire continuation risks first (M2, M3), then the first playable case (M4), then space (M5, M8) and law (M7) before the case consumes them. When a check fails, return the defect to its shared owner. Never work around it with a case-specific teleport, clue grant, pause or private-world query.
