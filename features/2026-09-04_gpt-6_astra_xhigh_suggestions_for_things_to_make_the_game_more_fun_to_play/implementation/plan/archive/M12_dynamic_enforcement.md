Status: Planned (2026-09-05).

# M12 — Guards execute their orders

Make an arrest happen because informed guards find and reach someone. This is ordinary off-stage simulation, not a quest scene callback.

## Entry

M11 orders, M8 duty arbitration, M4 navigation, M5 perception, M6 access and M3 persistence are accepted. Keep all test subjects and sites independent of An Alibi in Stone.

Follow [LAW_PROTOCOL](LAW_PROTOCOL.md) for the complete coordination contract, including issuance before any executor is informed, dispatch fairness, post coverage, station reservations and actual release egress.

## Task lifecycle

| Phase | Actual work | Exit or recovery |
|---|---|---|
| Receive | Officer attends a briefing or receives an authorised handoff | Record mandate/known cutoff; reject expiry or a version covered by an already-received recall |
| Assign | Reserve an eligible available officer and required support | Wait/reassign if shift, duty or capacity prevents acceptance |
| Select lead | Choose a known sighting, address, workplace or supported report | No lead means an unresolved wanted order, not a live-position lookup |
| Travel/search | Follow a valid route and examine the relevant local area | Empty/blocked/stale lead leads to bounded search or another known lead |
| Identify/approach | Perceive and recognise the subject under the order's identity standard | Lost sight freezes the last known position; wrong identity does not seize |
| Announce/apprehend | State authority and validate local seizure | Received recall, known grant expiry, blocked reach or another failed prerequisite prevents mutation |
| Escort | Both bodies traverse a valid route under custody/duty ownership | Wait, reroute, request help or record interruption |
| Handover | Reach an authorised keeper/place with capacity | Commit custody/receipt once; retain escort or take a defined alternative if unavailable |
| Complete | Record execution and release officer resources | The order is not silently reissued after later lawful release |

Run this service on a bounded ordinary simulation cadence independent of stage membership and cognition availability. LLM speech and choices may add character within permitted policies; they cannot be the only cause of progress.

## Assignment and information

Use explicit officer eligibility, jurisdiction/scope, shift, station capacity and duty priority. Enforce aggregate roster coverage as well as a bounded officer/pair assignment; multiple simultaneous orders cannot empty every post. Reservation and reassignment are idempotent.

Persist stable urgency/age queue order, dispatch cursor, next-attempt time and exhausted leads. A blocked oldest order cannot monopolise the service; a flow of newer ordinary orders cannot starve an older feasible one. Park exhausted work, release pre-seizure claims and wake it only on a relevant changed lead/availability or bounded retry. Travel-step replacement cannot reset the operation's overall progress/recovery budget.

Every lead has a source, time and supported identity. The engine may store Corin's exact location, but the task API must not expose it to an officer without an observation. A suspect leaving a shop before the guard arrives remains a real possibility.

Recognition and pursuit use M5. A nearby person behind a wall is not visible. Losing sight stores a floor-qualified last-seen position; reacquisition requires another valid perception. A guard can ask/check known places, wait at a plausible exit or receive new reports through the same ordinary mechanisms.

Execute scheduled registry checks and permitted local record/inquiry responses deterministically through M8/M11. The mandatory fixture starts with no informed executor; dispatch must not rely on the test secretly preloading an order into the guard.

## Shared seizure service

Refactor existing `seize`/`take_into_charge` so natural-language actions and deterministic enforcement invoke one validation/commit path. Check the officer's actual acquired finite mandate, capability/scope, received recall/version state, fixed cutoff and prior execution, plus identity, reach/occlusion, existing custody, capacity, destination and feasible handover route. The mandate's validity follows M11's causal service/cutoff policy; an unknown central recall cannot become a local refusal oracle.

The current helper mutates custody before a fallible route-budget check. Fix that ordering. A failed arrest command must not leave a prisoner record behind.

Preserve the rule that seizure is not silent. Use a deterministic local announcement through ordinary speech/event presentation when no model response is available. Announcing a seizure and committing it are linked domain effects; TTS delivery is not a prerequisite for the physical action.

Reserve a feasible station intake slot before capture, then revalidate it at arrival. Convert reservation to occupancy once; release it on cancellation/expiry. Count authored inmates and actual keeper availability. A second applicable order attaches a ground to existing custody instead of making a second escort. Autonomous seizure and dispatch converge through the same identity/receipt path.

Replace booking's silent item confiscation with a scoped M7 operation naming the item, present authorised receiver/container, reservation-conflict policy and custody receipt. Failed property work is explicitly pending/refused and independent of the prisoner hold; no broad inventory strip is required.

## Physical escort

Replace `custody::follow_escorts`' direct offset position assignment for NPC prisoners. The officer leads a route; the prisoner follows reachable recent route points at a bounded speed. Both use support surfaces, door permissions and passage reservations. The officer waits when separation exceeds the allowed escort distance.

Test initial capture separation, L-shaped corridors, stairs and closed doors. No initial snap, wall crossing or catch-up teleport is allowed. Keep the existing player tether/grip presentation coherent with the same custody authority while respecting player-controller ownership.

A physically resistant suspect can invoke the existing struggle mechanics where supported. If a new route is needed after escape, it belongs to ordinary movement and perception; there is no scripted despawn. The full delivery need not become a combat system.

## Interruption and recall

Recheck the actual finite grant before seizure and the relevant hold authority at later consequential steps. Receiving recall while searching ends attributable work; receiving it while escorting evaluates remaining grounds and initiates the appropriate release/handover action. Central recall alone stops new issue/renewal at the registry and needs a real delivery/check to change the distant officer's work. A guard already holding someone under a separate valid ground does not discard that matter.

Known mandate/hold cutoffs advance locally with the ordinary calendar. Include the original hold deadline/default in every custody handover; there is no fresh lease merely because a keeper changed. A courier-delayed mandate cannot gain a new lifetime at first receipt. At a near-bell watch arrest, use the predeclared next-bell fallback if intake cannot actually be reached in time.

Keeper absence, full capacity, officer departure, severe need, locked premises and loss of route all have bounded, visible outcomes. A suspect can remain wanted without being magically captured. Dynamic behaviour means causal behaviour, not a guaranteed quick success.

After seizure, interruption retains an actual custody-care/relief responsibility and processes the legal deadline; generic claim cleanup cannot abandon the prisoner. At discharge, deliver the instruction, remove restraint and provide real egress through M6. An obstructed exit is reported without pretending the actor has already left. Retain independent property/return claims.

## Tests

- With the player far away in stage mode, no cognition completions and initially no informed executor, a newly issued order reaches a guard through an ordinary check; the guard finds an arbitrary fixture suspect, seizes and hands them over through actual motion.
- An uninformed officer, stale lead, wall, different floor, wrong identity, received recall or expired mandate prevents inappropriate capture.
- Two guards assigned the same order produce one custody and one execution receipt.
- No-route, no station capacity and blocked threshold failures leave pre-action state intact.
- Escort displacement is speed bounded through corners/stairs/doors; no direct-offset teleport survives.
- Hunger, conversation, curfew and ordinary work cannot silently steal a custody route.
- Recall issuance, physical transit, recipient receipt and local cutoff during travel/approach/escort/handover have distinct idempotent results. Paired distant-officer worlds differ only after a locally available cause, without a hidden remote-authority check changing behavior.
- Save/load at every phase matches the uninterrupted control and rejects old callbacks.
- Reuse the test with different actors/districts through data changes only.
- Three simultaneous orders include an impossible oldest lead and a feasible later one; dispatch fairness and parking survive load.
- Competing escorts reserve the last intake slot correctly; overnight keeper coverage and relief preserve occupied posts.
- Two opposing escorts, a temporary blocker and a permanent blocker exercise operation-wide recovery budgets without replan/reset loops.
- Release before property return, an offered evidence packet and keeper replacement preserve separate scoped custody/ownership claims.

## Completion gate

An arrest order can cause a real off-stage arrest without case-specific code, omniscient tracking or LLM scheduling luck. Known order progress is legible through M10; hidden searches are not shown as a magic map marker. Existing law behaviour has been reconciled with the new disposition model.
