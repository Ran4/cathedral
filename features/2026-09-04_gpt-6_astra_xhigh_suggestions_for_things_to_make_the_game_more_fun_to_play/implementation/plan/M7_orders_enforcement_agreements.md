Status: Planned (2026-10-07).

# M7 — Orders, enforcement, agreements

From: old M11 orders ([archive](archive/M11_orders_and_dispositions.md)), M12 enforcement
([archive](archive/M12_dynamic_enforcement.md)) and M8's agreements; replaces
[archive/LAW_PROTOCOL.md](archive/LAW_PROTOCOL.md). R7 in [PLAN_REVIEW](PLAN_REVIEW.md). Entry: M6.

D07: a finding produces an order, and the ordinary city carries it out. Pursuit is the one genuinely missing piece:
nobody searches for a warrant subject today.

## Scope

- **Orders on the shipped law.** Extend `WardNotice`/`CustodyRecord`; don't add nine record types. The order becomes
  the warrant rung (Q7): every warrant is pursued.
- **Explicit states.** An order-state enum for D07's ten states: issued, received, assigned, searching, encountered,
  seized, escorting, committed, suspended, discharged.
- **Pursuit.** A small service on `take_into_charge`. An officer who has received the order searches known leads
  (last sighting, home, workplace, a report), never the subject's live position; an empty address is a failed lead.
  Then seize, escort and commit through the existing custody code.
- **Holds.** Holds grounded in an order or finding run on game-day expiry (Q8). Ordinary custody keeps its
  240 s/360 s release and the city-wide cap of 4.
- **Recalls** travel to officers the way the order did (Q9), so a distant guard can still act on stale orders.
- **Locked premises.** An officer holding a delivered order may open a locked door (M5) to execute it (D07).
- **Agreements.** One small record (payer, payee, purpose, terms, deadline, state) for hush, restitution and a
  one-use favour. Funds are reserved on acceptance and paid once; kept or breached is decided at the deadline.
- **Fix: hearsay stays hearsay.** An LLM `raise_notice` without first-hand knowledge stays hearsay, and hearsay cannot
  climb to an executable warrant (today it can, through `summon` → `issue_warrants`).
- **Fix: who may release.** Release and settlement of grounded holds are limited to a finding or the issuer (today the
  single `is_law` gate lets the victim or a witness free a prisoner).

## Not in scope

Fairness queues, intake reservations, rosters, mandates and tombstones, an order registry, a separate quest police
service.

## Dev fixture (D01) and lockstep (D06)

A dev finding against a dev subject who is elsewhere in the city, plus a dev agreement. Lockstep: save with a guard
mid-escort and an agreement pending; the restored run commits once and pays once.

## Acceptance (player-visible)

1. A finding issues an order while the subject is across the city; a guard learns it, searches, finds, seizes,
   escorts and commits them, and the hold lasts until the review. The status list shows each state as it happens.
2. D07's eight situations each end in a defined, visible outcome: the subject is absent; the guard's information is
   stale; the journey is interrupted; the premises are locked; the guard has a competing duty; no officer is
   available; the finding is corrected (recall); the subject is already held for another matter.
3. A rumour can bring a summons but never an executable warrant.
4. The victim or a witness cannot release a grounded hold; the issuer or a finding can.
5. An agreement kept pays exactly once; breached, its stated consequence follows; a reload pays nothing twice.
6. Save with a guard mid-escort, load: the escort continues and commits once.

Edge cases worth reusing: archive V47–V55 (recall in transit, a second ground, intake races, release while a door is
blocked) and V36 (two agreements competing for the same funds).
