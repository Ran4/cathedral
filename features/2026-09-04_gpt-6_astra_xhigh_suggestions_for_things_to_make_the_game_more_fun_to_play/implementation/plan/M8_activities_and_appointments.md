Status: Planned (2026-09-05).

# M8 — Undertakings and appointments

Make promises produce attempts in the ordinary world. Activities must share people, places and time with meals, trade, conversation, travel and the law.

## Entry

M1–M7 are accepted. Activities use their command receipts, persistence, routes, access, observations and objects.

## Runtime model

Compose M1's accepted single-operation kernel into bounded multi-step activities with definition, participants/roles, resource claims, preconditions, current step, completed step receipts, deadlines, interruption reason and recovery policy. Initial definitions compose the now-shipped adapters: travel, wait, operate a fixture, examine, transfer and finish. M9 later registers confirmed-statement/submission adapters; M11 registers order/disposition adapters. An unavailable adapter is rejected at load, never faked as an accepted step.

Use domain services for effects. A definition cannot write arbitrary `World` fields, teleport participants or grant facts to everyone. Each step performs current validation and commits once. A step's failure leaves completed earlier steps real and explains what remains possible.

Appointments specify calendar time/window, invited roles, site, arrival policy and what to do when a participant is missing. Acceptance creates an undertaking, not instant availability. Record invitation delivery and acceptance; an uninvited person is not mysteriously expected to attend.

## Ordinary behaviour integration

Extend M1's movement/duty arbiter in the existing `Round` ladder. Publish a clear priority table covering confinement, physical escort, immediate danger, accepted institutional duty, urgent needs, explicit travel, conversation courtesy and the daily round. The exact order must be tested against the base game's needs and custody policies. This is one arbiter, not a replacement that makes M6/M7 operations use different ownership rules.

Resource ownership is explicit and temporary. One actor cannot escort a prisoner, operate a hoist and serve a market counter at once. One packet cannot be examined while another activity takes it. Claims have bounded recovery when a participant departs, becomes confined or loses access.

Add finite post/roster obligations: required coverage intervals, eligible existing relief and actual handover. Aggregate assignments must not empty a required keeper/gate post. Implement a deterministic record-check/inquiry adapter so an available officer can visit a registry and obtain permitted records without waiting for optional model dialogue. M11 supplies the legal record type; M12 uses the duty and coverage contracts in [LAW_PROTOCOL](LAW_PROTOCOL.md).

Conversation can interrupt an actor's work when their policy permits it. It cannot pause an activity's private clock while the rest of its participants keep moving. A timed demonstration either continues under its declared rules or records an interruption, preserving the reason and allowing a repeat.

Provider availability never determines whether an accepted mechanical step can finish. Use authored neutral announcements when necessary, delivered through the ordinary speech/event channel. LLM choices can select among permitted strategies, but an unanswered optional choice needs a documented deterministic fallback.

## Bounded strategies and autonomous duties

Add data-authored policies triggered by calendar edges, actual received facts/events and operation results. A policy can choose among permitted activities, decline, retry within a bound or change its current strategy. It cannot query arbitrary hidden world state, detect the player's private notebook or branch on a case's secret culprit field.

Use stable transition identities and persisted trigger-consumption state so polling/load cannot start the same undertaking twice. Deliver deterministic duties even off stage. M9 adds the adapter by which an informed NPC can lodge its own confirmed account; M11 adds procedural review and order duties. These later adapters use the same policy service.

## Agreements and service entitlements

Implement bounded agreements with named parties, explicit accepted terms, actual reserved resources, fulfilment/default/breach receipts and one-time redemption. Support payment and a concrete service undertaking through the existing item/offer/reservation machinery. A promise is not a completed payment or delivered service.

Reconcile money reservations with every ordinary spending/transfer path, including market purchases and fees. Reserving eight sparks cannot leave the same eight spendable elsewhere or fund a second agreement. An accepted but unfunded obligation has an explicit outstanding state; a later funding/payment step revalidates current resources and commits once. Do not create a separate quest purse or freeze all economic activity to protect one payment.

Terms name each responsible party, required/prohibited act, object/evidence scope, allowed recipients, deadline/completion condition and supported default consequence. A promise to remain silent does not become fulfilled merely because nothing was said on its first poll. Independent third-party disclosure cannot be attributed to the contracting player. Reporting/sponsorship duties need actual notice delivery and attendance windows.

A service entitlement names the provider, eligible task, scope/destination limits, availability and remaining uses. Redeeming it creates an ordinary activity; lack of a cart, permission or available worker is a real pending/refused condition. It does not conjure resources.

Confidential undertakings can refer to an agreed disclosure scope. M9 supplies the actual disclosure receipts and contamination checks; no agreement may judge a hidden/private player intention as a breach. Restitution, hush payment and cargo favours later author different terms on this one service.

Define cover narrowly: choose an already eligible available alternative, reschedule, remain closed or report blockage. Do not invent a replacement citizen or assume an unimplemented staffing economy exists.

Ship versioned activity, policy, agreement and entitlement declarations with their loaders/validators in this milestone.

## Route demonstrations

A demonstration binds participants, a specific claim, endpoints, an allowed traversal profile, physical actions, access conditions, mechanism cycle and observers. A stand-in can do the movement so the player need not win a sprint test.

Follow [SPATIAL_PROOF_PROTOCOL](SPATIAL_PROOF_PROTOCOL.md): bind each claimed hidden/intermediate act to an actual observer, verifiable physical result or supported participant account. Mott seeing departure/return does not certify a hidden rack stop. Record narrow observed return timing separately when intermediate work is not supported.

Record actual motion and elapsed duration, not `distance / speed` alone. Include opening time, turning/waypoint behaviour, setup inside the measured interval and task actions. Preserve geometry/access revisions and identify whether the attempt matched the historical conditions.

Use M7's actual stroke/rate/load/interruption state for a timing mechanism. Last counter visibility, stroke start/end and renewed sighting are separate events; include Lise's travel/recognition delays. Compare relevant historical/present stock occluders, doors, witness poses and fixture configuration as irrelevant, conservative, material or unknown. Repeated present-day cycles cannot by themselves establish historical equivalence.

A successful attempt proves that attempt possible. A failed attempt in a crowd does not prove all routes impossible. A lower-bound certificate, where used, is a separate spatial analysis with explicit coverage and uncertainty.

Detect simulation hitches or dropped movement spans that invalidate a timed comparison. Never award proof from a wall-clock duration while actors lost part of their movement budget. Retry with an honest explanation; do not freeze the city or secretly slow a mechanism to compensate.

## Meetings without a quest mode

Participants travel through the city and may be delayed. The meeting uses only the people necessary for the requested business. Recorded accounts can stand in for absent speakers where the procedure permits; physical demonstrations wait for their actual requirements.

The meeting can occur without the player, using only material already lodged. Publish its real result through records/participants. Late arrivals can inspect that result and request another supported step. Do not retroactively pretend their unsubmitted deductions were presented.

## Tests

- Two activities contend for the same actor/object/fixture; exactly one owns each exclusive resource.
- Hunger, curfew, conversation, custody and departure interrupt/resume according to the declared table.
- An invited actor cannot reach the appointment; the meeting reports absence rather than teleporting them.
- A late player observes what actually happened and retains a continuation path.
- Repeated ticks and retries do not duplicate transfers, statements or completion effects.
- A stand-in demonstration uses real routes, timing and observer availability.
- Occlusion/crowds/door closure/hitches interrupt evidence continuity or timing where appropriate.
- A stand-in skips hidden work and returns on time; only the narrow covered return claim succeeds, not the complete itinerary.
- Save/load at every activity step and during interruption matches an uninterrupted control.
- The same activities run off stage with fake or unavailable cognition.
- Payment/service agreements reserve real resources, fulfil once and leave explicit default/pending state when funds or providers are unavailable. Two agreements and an ordinary purchase competing for the same coins cannot spend them more than once, including across save/load.
- A learned warning changes an allowed activity strategy; an unreceived warning and the player's private notes do not.

## Completion gate

Independent citizens can keep or fail undertakings visibly in normal gameplay. No activity owns a world pause or bypasses shared command services. M9/M10 can use examinations and meetings, and M12 can add institutional enforcement duties through the same arbiter.
