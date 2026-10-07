Status: Planned (2026-10-07).

# M4 — Accounts, inquiries and findings (the first playable dev case)

From: old M9 ([archive](archive/M9_inquiries_and_evidence.md)), the core of old M10
([archive](archive/M10_conversation_and_interface.md)), M8's review trigger and M13's pack loader. R4 in
[PLAN_REVIEW](PLAN_REVIEW.md). Entry: M3.

The first thing a human can play: question two witnesses in a small dev case, lodge what they said, and get a
finding at a dated review whether or not the player attends. Provenance already exists (`Held{hops,from}`); what is
missing is a deterministic way to ask, evidence the player cannot lose, and a bundle evaluator.

## Scope

- **Asking.** An explicit "ask about" control on a learned topic, and one NPC verb (e.g. `recount {fact}`, checked
  against `knowledge::holds()`), so a spoken answer mints the same receipt as the control (D04). A model cannot
  recount what the speaker does not hold.
- **Evidence the player keeps.** Evidence records extend `knowledge.player_learned` with a pin and a `FactId`; the
  knowledge README says quests own no receipt store of their own. A pinned account survives the 6-holdings /
  64-receipts caps.
- **Inquiries and findings.** Lodge a bundle with a reviewer. The evaluator applies data rules (independent origins,
  dated observation, identified object, confirmed account, contradiction) bound to one actor/object/incident/interval,
  and returns a partial or full finding. Missing-link text names only what the lodged bundle lacks, never unlodged
  evidence or a suspect (D04). Two retellings of one witness are one origin.
- **The dated review.** A review at a calendar office (a dated trigger on the existing rounds). It happens with what
  was lodged whether or not the player is there; the result reaches the Case tab.
- **Interface (D04/D05).** A request-status list showing a spoken intention, a pending request, an accepted
  undertaking, work in progress and a completed or interrupted action; an NPC's "I will…" looks different from a
  committed action. A lodge panel, a Case tab, preserved drafts and specific refusal reasons. Open panels show change
  notices; events missed while reading land in the Case tab. Nothing pauses.
- **Case packs.** A loader that refuses bad cross-references with a readable error. Wire `fact_packs` into the game
  host (`local_engine.rs` passes `fact_packs: Vec::new()` today). Packs seed only on a fresh world, never on restore
  (D02).
- **Conversation obligations (Q13).** Scheduled round legs, appointments and duties break a player conversation after
  one excuse-yourself turn.
- **Fix: "heard by N".** The badge (`hud.rs`) counts only listeners who can see the speaker.

Q3 (D08): no early real-case slice; dev mini-cases until M9.

## Not in scope

A typed binding engine or strategy DSL, an observation archive tier, dictation purpose binding, the record delta
protocol, generic authoring tooling, accessibility matrices.

## Dev fixture (D01) and lockstep (D06)

Two dev disputes with different shapes: one cleared by two independent witnesses with no offender, and one with a
correlated-rumour trap (two people repeating one witness) and a corrected statement. No Alibi cast or IDs. Lockstep:
save between lodging and the review; the restored run reaches the same finding once.

## Acceptance (player-visible)

1. Ask two witnesses about the dev incident by typing, voice or the control; each account appears in the Case tab
   with its speaker and time.
2. Lodge both: the review gives the full finding. Lodge one: a partial finding that names only the missing kind of
   support.
3. Lodge two retellings of the same witness: they count as one source.
4. Spend the review's office reading the journal elsewhere: the review still happens, and the Case tab reports its
   real outcome afterwards.
5. An NPC saying "I will fetch the reeve" shows as an intention, not a completed action.
6. Hear many other rumours: the pinned accounts are still there.
7. Save between lodging and review, load: the review happens once with the same finding.
8. A pack with a dangling reference is refused at load with a readable reason.

Edge cases worth reusing: archive V38–V42 (remove each support; cross-case mixing; hidden-cause noninterference;
autonomous NPC lodging; amended findings) and V43–V46 (input parity; stale draft; unknown off-stage result).
