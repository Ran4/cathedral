Status: Proposed ordinary-city contract (2026-09-05); owned by M6/M8/M11/M12/M18, not implemented.

# From a supported order to an actual consequence

This contract makes the dynamic-law requirement executable. It applies with the investigation pack disabled and with cognition unavailable. It supplements the milestone files with the coordination rules that cannot safely be invented independently by each owner.

## 1. Keep the records separate

| Record | Owns | Does not imply |
|---|---|---|
| Matter/allegation | The reported wrong, parties and procedural history | That the report is true or anyone may arrest |
| Authority decision | Issuer capability, submitted grounds and permitted scope | That an officer received it |
| Order authority | Stable order ID, version, active/stayed/recalled/expired/discharged validity | Assignment or physical custody |
| Execution mandate | A previously issued finite grant bound to order/version, recipient, scope and fixed cutoff, with received recall/version state | That its named recipient has acquired it or can use it after its cutoff |
| Delivery receipt | Recipient, acquired version, source/channel and actual receipt time | That every colleague also knows |
| Assignment attempt | Officers/resources, leads, phase, progress/retry budget and outcome | A new legal ground or successful execution |
| Execution receipt | The actual completed act, people, place/time and authority used | A command to repeat that act forever |
| Custody | One physical control/delivery state for a subject, with a set of active hold grounds | Ownership of everything the person carries |
| Property receipt/claim | Specific item custody, owner/claimant, scope and required return/review | That ending detention resolves property claims |

An order can have several delivery and failed-attempt records while its legal validity changes independently. Do not encode these as one linear status enum. A second valid order against an already-held subject attaches a validated ground to their existing custody; it does not replace their officer, restart an escort or reset earlier review deadlines.

Registry recall advances the authority version and stops new mandate issue/renewal there. Its field effect follows the ordinary delivery policy below. A recipient's received recall tombstone prevents a later stale issuance from reviving that mandate. A subject with another valid ground remains held under that ground, with a clear receipt. An autonomous lawful seizure and an existing dispatch task converge on the same physical custody record and execution coordination.

### Finite authority and causal recall

Choose one explicit procedural rule: a mandate **already issued** before registry recall remains usable by its named qualified recipient until they receive recall or its fixed cutoff passes. Actual receipt is required before use. The distinction includes a mandate already handed to a carrier but not yet delivered; an unknown remote recall cannot make first receipt fail psychically. A generic order notice carries information and cannot itself create a new execution grant.

Issue records the mandate's scope, permitted use, fixed civil cutoff and applicable hold/review limits. Check those authoritative local grant facts, capability, acquired version, recall tombstone and execution history at each consequential action. Receiving, forwarding, assigning, saving or loading cannot restart its lifetime. Reissue/renewal requires a new authorised registry decision and actual delivery. A consumed mandate cannot arrest the same released person again merely because its date has not passed.

Several officers' or renewed mandates for the same execution purpose share its durable execution identity. Issuing another piece of authority cannot reset an already completed arrest into unexecuted work. A genuinely new matter/decision uses its own supported transition. Validation does not require a held mandate's source version to equal an unreceived newer registry version.

Unscheduled recall uses an existing capable carrier, handoff or scheduled check to reach the executing officer and any affected keeper. Receipt cancels only attributable uncommitted work and applies the appropriate grounded release/correction instruction. A central decision does not silently stop a distant search or become that officer's knowledge. No complete wrongful-arrest/court simulation is needed for this first policy: validity of the preissued grant itself follows the declared service/cutoff rule. The registry may report “recall issued; executor receipt unconfirmed,” rather than claiming the field action has stopped.

M11 freezes finite mandate periods in procedure data against M0's measured journeys, review and checking cadence. Do not choose a new duration on every retry or extend a hold to accommodate a slow messenger. Arrest authority cutoff governs new execution; a committed hold has its own independently fixed grounds/deadline and pre-delivered fallback. Serving a recall may end it earlier. Both dates are preserved through handover.

## 2. Give orders an ordinary route out of the registry

M11 declares the issuing registry and physical briefing/check location. M8 provides an accepted duty for eligible officers to check at shift start and at least once per office while available. M12 schedules and processes those checks without a model turn, through actual travel/local receipt. If the institution is closed or nobody can reach it, the order remains undelivered with a real operational cause.

Checking a known record does not require the officer to know the order's ID first. It queries the accessible queue under their role/scope, records the versions actually received and lets the dispatcher consider them. A public rumour may prompt an earlier check but is not an authoritative copy.

An urgent physical relay, if authored, needs an existing eligible available carrier, a reserved delivery duty, a journey and receipt. The first complete implementation may rely on the bounded scheduled checks; it must not silently use instant global notification to make the fixture pass.

Known-place inquiry and asking a relevant available keeper need deterministic permitted response adapters. They return only records/observations the respondent holds and is willing/authorised to disclose. Optional prose adds character around the response; absent cognition cannot block the only supported way to get a next lead.

The acceptance fixture begins with **an issued order and no informed executor**, the player elsewhere, a valid available lead and feasible institutional resources. It must reach briefing, assignment, search and physical execution. A test starting with a pre-informed guard misses the first required link.

## 3. Dispatch fairly and stop wasting unavailable officers

Use a stable queue ordered by procedural urgency and age, with a deterministic tie-breaker. Immediate physical custody/safety duties outrank new investigation work. Within ordinary dispatch, age promotes a feasible waiting order after two offices so a stream of newer ordinary requests cannot starve it. Emergencies may still delay it; expose that cause rather than claiming unconditional latency.

Persist attempt identity, dispatch cursor, age, `next_attempt`, known-lead consumption and obstruction revision. Limit concurrent assignments per subject/order and apply aggregate post coverage, not only a pair limit per order.

An exhausted lead set parks the order and releases pre-seizure officer/resource claims. Wake it on a new received lead, a relevant changed availability/topology revision or a bounded scheduled retry. Polling the same empty address every frame is neither investigation nor progress. One impossible oldest order cannot monopolise every dispatch attempt.

M1/M8 own operation-wide progress accounting. Replanning a travel step does not reset last-progress time, attempt count or total recovery budget. The initial policy allows bounded alternative routes and one relief request before reporting a persistent obstruction; M0/M12 fix numerical durations from measured ordinary journeys. Record those values in data and their test fixtures rather than scattering magic timeouts through tasks.

Before seizure, failure releases the assignment cleanly. After seizure, an interrupted escort still owns a real prisoner and needs a custody-care/relief or lawful-release disposition; it cannot drop the physical duty merely to satisfy a generic cleanup rule. Preserve feeding/watering and process the hold's own deadline while recovery runs.

## 4. Posts and stations are resources

M8 declares post intervals, minimum required coverage, relief eligibility and explicit handover. M12 cannot assign a keeper to an arrest if doing so leaves a required occupied post uncovered. A person cannot count simultaneously as keeper, escort and their own relief. Select only existing qualified people; an unimplemented staffing economy cannot conjure replacements.

Current Stone House staff all return home at Snuffing. M11/M12 must author a valid overnight roster or explicit unavailable hours and their custody policy. A committed prisoner cannot silently turn off the need for care when their arresting officer resumes the ordinary round. M6 projects the actual keeper/access consequences.

Replace “capacity” as an unexplained predicate with station declarations: reachable intake, accepted hold types, physical occupancy limit, keeper/coverage requirements, confinement and egress locations. Count authored inmates in physical occupancy. Keep any global safeguard limiting newly generated arrests separate from station space; current code's four-new-arrest cap excluding eight authored inmates is not a station model.

Before capture, reserve a feasible intake slot against a destination and bounded arrival window. At arrival revalidate station authority, coverage, occupancy and access. Two escorts cannot claim the last slot. Convert reservation to occupancy exactly once at handover; release it once on cancellation, expiry or relocation. A reservation is not proof of guaranteed future availability.

If intake becomes unavailable, choose another already authorised feasible destination, request available relief or enter the defined temporary-care policy. Do not invent a cell, teleport the prisoner or renew the legal hold simply because the journey failed.

## 5. Hold deadlines and physical release

Every hold procedure names its grounds, review deadline, permitted extensions, default disposition and responsible qualified role. New extensions require their own authority transition and grounds. Deferring a hearing, waiting for a provider or failing to locate the preferred reviewer does not extend detention.

Deliver each known hold deadline and its fallback with the initial custody authority and every keeper handover. At its scheduled expiry the responsible local service already has the instruction: remove the relevant ground and start physical release if none remains. It does not need to await a new remote message. An unscheduled early discharge/recall instead reaches that service through the actual communication rule above. Remove restraint, perform necessary door/egress assistance and let the actor resume an ordinary route. These are distinct, observable steps. Do not report “left the station” merely because the custody map entry disappeared.

A receiving keeper who already knows the relevant recall cannot accept that cancelled ground from an uninformed escort. Preserve any independent valid ground and arrange appropriate care/release for the actual person present. Transferring physical responsibility is not a way to wash out received cancellation or establish fresh detention authority.

Authority validity ends **exclusively** at its fixed cutoff. Process due expiry before effects stamped at that same accepted instant. “Next office” means the next strictly later calendar boundary after the originating event. An extension needs its independent supported authority and actual receipt before the original cutoff; receipt, intake or handover exactly at expiry cannot retrospectively continue the old hold. An actually new lawful ground remains a separate current decision. These rules must agree under regular/jittered polling and save/load, with no tick-sized grace period.

M6/M12 must provide an authorised usable egress policy and relief for an unavailable keeper. If masonry, another body or a temporary obstruction physically blocks exit, retain a truthful “release authorised; exit obstructed” operation and solve that local obstruction through ordinary movement/access. The person is no longer lawfully restrained under the expired ground; the system must not use the obstruction to manufacture a renewed hold or teleport them out.

Preserve existing prisoner feeding and watering. A newly layered/locked station must not turn a procedural delay into death by an omitted need adapter.

Sponsorship has named accepted obligations through M8, a procedural decision and a keeper's release action. Pending sponsorship never prevents an existing hold from expiring. Missing a later reporting appointment is an observed failure to attend, with a contact/summons procedure; deliberate evasion requires further established grounds.

## 6. Booking and property

Replace current booking confiscation's silent one-item transfer with a scoped M7 operation. Name the authorised item/object scope, present qualified receiver or container, custody purpose and receipt. Resolve existing reservations explicitly: defer, cancel through the supported service or refuse with a cause. An offered packet is not silently taken twice. Unrelated pocket contents remain outside the authorised search.

A failed property operation does not retroactively undo a valid arrest, nor does it pretend evidence arrived at the station. It becomes pending/refused property work with a recorded current holder and known location. Physical possession by a keeper does not transfer ownership.

On release, preserve evidence-retention grounds and unclaimed-property/return claims independently. A valid tool pledge remains valid until genuine redemption or a supported release; a forged collection document failing does not itself discharge the debt. M18 delivers authorised returns through actual available people and receipts.

## 7. Migrate the ordinary city explicitly

| Existing flow | Required migration |
|---|---|
| Hearsay report and summons | Preserve claimed/garbled provenance and lower-confidence intake; no automatic upgrade to arrest from nonattendance alone |
| Witnessed immediate breach of peace | Require supported firsthand perception and the proper watch capability; record cause and a bounded temporary hold |
| Gate/night-watch intervention | Follow the existing canon's delivery of prisoner **and written cause** to the Stone House by the next office bell, with a defined deadline fallback; a nearest arch is not an indefinite replacement |
| Bench/court order | Issue scoped authority through the registry, actual delivery and the shared execution service |
| Authored historical inmates | Install explicit legacy grounds/review policies without inventing fresh evidence or releasing everyone with no old notice |
| Posted fees and ordinary debt | Preserve their actual payment/claim procedures; sharing an occupation family does not grant a debt collector arrest authority |
| Escape/resistance | Keep physical struggle and knowledge of the event separate from any new legal decision; one executed order is not an infinite arrest loop |
| Recovery/restitution | Resolve the specific property/money obligation while unrelated assault or hold grounds remain |

These fixtures run with An Alibi in Stone disabled. If any canon is intentionally changed, record the new procedure and its lore update together; do not accidentally change it by reusing a convenient station helper.

## 8. Coordination acceptance

Test three simultaneous orders with an unreachable first subject and a feasible later one; no starvation or exhausted-lead busy loop. Test opposite-direction escorts through a narrow passage, a temporary blocker that clears and a permanent blocker whose recovery budget survives load. Test competing claims on one intake slot, overnight keeper relief and multiple arrests without emptying required posts.

At each coordination race, save and resume: stale delivery after recall, one of two hold grounds recalled, autonomous seizure during dispatch, slot expiry during travel, release instruction before a keeper change, and an item reservation blocking booking. Verify conserved resources, one physical custody, correct independent property claims and no replayed authority.

Explicitly distinguish central recall from recipient receipt. Compare identical distant-officer states with and without an undelivered registry recall: behavior stays the same until actual receipt or the already-known cutoff; the registry's own issuance queue differs. Deliver recall before an older mandate and ensure the local version tombstone rejects resurrection. Deliver an already-issued mandate after central recall but before local receipt of that recall and exercise its original fixed cutoff. Check a keeper handover on both sides of known hold expiry: the deadline/default transfers intact and neither new custody ownership nor late instruction renews it.

Exercise gate/night-watch seizure just before an office bell, where the canon's next-bell delivery may be physically impossible. The explicit procedure must take its predeclared deadline fallback at that bell while retaining actual care/egress responsibilities; it cannot teleport to intake or silently reinterpret “next bell” as a new full office. A fresh hold extension requires independent grounds and authority, not travel-budget exhaustion.
