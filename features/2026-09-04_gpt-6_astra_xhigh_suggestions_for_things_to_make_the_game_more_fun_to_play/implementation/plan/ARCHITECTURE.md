Status: Proposed implementation contracts (2026-09-05). These describe required work, not shipped interfaces.

# Architecture contracts

The deliverable is a living city that can host investigations. Case content supplies people, places, records, activities and evidence rules. Shared systems decide whether actions are possible and perform them. These contracts apply to both the development fixtures and the final quest.

Names introduced below are proposed interfaces, not claims about existing code. Implementation may improve their names while preserving the responsibilities and tests. Existing shipped APIs take precedence over speculative signatures.

## 1. Keep authority in the simulation

`cathedral-sim` remains pure Rust without filesystem, network, threads or Bevy. It owns mutable world state, commands, permissions, observations, activities, evidence, findings, legal orders and their transitions. `cathedral-backends` owns IO and provider services. Bevy projects state and gathers player input.

Shared spatial data must become sufficient for headless navigation and visibility tests. Bevy and the sim consume the same authored surfaces, portals and occluders; a render-only raycast is not the sole authority for an off-stage guard. Rendering may use richer art, but an opaque structural wall or usable stair cannot exist in only one half.

The private checkpoint is separate from `PublicSnapshot`. It contains the secrets required to restore the world and must never be installed as a UI mirror. UI read models expose only the player's learned records, visible state and authorised institutional information.

## 2. Three kinds of time, with one advancing world

Distinguish:

| Time | Purpose | Save/load treatment |
|---|---|---|
| Host monotonic time | Polling external services and driving a running application | Establish a new origin after load; never compare saved process timestamps to a new process clock |
| Simulation elapsed time | Movement, interaction durations, local observation intervals and operation progress | Preserve the saved elapsed position and continue advancing from it |
| World calendar time | Offices, days, ordinary schedules, reviews, release dates and news age | Preserve the clock mapping and rate; calendar appointments remain calendar appointments |

Do not make every timer a calendar timer, or every calendar appointment a fixed real-time delay. The hoist's physical cycle is an elapsed-time operation. A review at High Wick is a calendar appointment. Changing the developer day rate cannot rewrite a past observation's timing or make the same physical route suddenly traversable in fewer seconds.

No interaction owns a pause flag. Conversation-floor pacing can regulate whose voice plays next; it cannot regulate movement, enforcement, needs or the city clock. An individual may stop to converse under the ordinary behaviour policy, but an urgent duty can interrupt the exchange.

Loading restores a previous world, rather than simulating the time the application was closed. Saving captures a consistent boundary while play continues. Neither requires a quest mode.

The accepted-time coordinator advances physical motion and domain/calendar time together through bounded substeps. Ordinary unaccepted wall-time debt is distinct from already accepted fixed/motion residuals; both survive a checkpoint, and only later budgeted progression spends the debt. Offline time and load preparation add none. Current virtual-time and movement caps cannot silently discard a span or let the calendar outrun physical work. [RUNTIME_BUDGETS](RUNTIME_BUDGETS.md) and [CHECKPOINT_PROTOCOL](CHECKPOINT_PROTOCOL.md) specify the boundary, admission and adoption contract.

## 3. A command is a request; its receipt reports reality

Route natural-language proposals and explicit controls through the same command services. A proposed envelope contains a world/runtime generation, command identity, actor, target references, requested operation and relevant expected revisions. Do not force callers to supply the entire global `world_revision`: unrelated crowd movement must not make every action stale.

Validation is narrow and current: the exact item is still at the relevant location, the recipient is present and in range, consent still covers the act, the applicable acquired mandate or hold is exercisable under its declared service/expiry policy, the object revision matches where comparison requires it, and the required resource is available.

| Receipt | Meaning in the world | Example player wording |
|---|---|---|
| Rejected | No requested domain effect committed | “Lise has left the counter. The packet has not been handed over.” |
| Accepted | A durable undertaking exists; its physical work is still pending | “Mott has accepted the papers for examination.” |
| In progress | A named activity has begun and owns the necessary resources | “The comparison is underway.” |
| Completed | The specified result committed | “Odo matched the wrapper to the recorded counterfoil.” |
| Interrupted | Work stopped without pretending its final result occurred | “The demonstration stopped when the doorway was blocked.” |
| Superseded | A newer effective authoritative decision replaced the request | “The guard received the recall before reaching him.” |

Each receipt references a stable action/operation ID, an actual time and a bounded reason code with player-facing text. The UI may summarise these, but cannot optimistically mark a pending arrest or comparison complete.

For short operations, validate all fallible prerequisites before applying mutations. Commit the inventory transfer, custody change and corresponding event together. A no-route error after changing custody is a failed transaction design, not a legitimate partial result.

For long operations, each completed step is a real durable result. A later interruption does not erase earlier travel, disclosed information or handed-over property. The operation explains the current state and recovery options.

## 4. Exactly-once effects within a restored timeline

Use stable command, operation and step identities with persisted acceptance/completion state. Repeated input, repeated provider delivery and repeated polling must not give an item twice, issue two orders or complete an examination twice.

An in-memory receipt cache alone is insufficient. Persist the required high-water marks and active/recent operation receipts; reject old command identities outside the replay window rather than silently accepting them again. Define bounded retention without dropping references still used by an active activity, case or save migration. [RUNTIME_BUDGETS](RUNTIME_BUDGETS.md) defines a provisional 4,096-envelope replay window, separate position sequencing, replay-floor rejection and retained semantic roots; M0 freezes the actual limits.

A newly loaded world gets a fresh runtime generation. Every host command, provider result, STT/TTS callback and presentation acknowledgement carries that generation or passes through an equivalent generation-owned adapter. Late callbacks from the replaced world are discarded even if actor IDs and request counters happen to match.

This is not a promise that two live LLM runs produce identical wording. It is a promise that committed domain effects are not duplicated or retroactively invented. Deterministic continuation tests use the fake backend or recorded completion inputs.

## 5. Facts, observations and evidence have different jobs

The completed knowledge system owns facts, who holds them, provenance chains and player learning. It remains the only authoritative knowledge store.

An observation receipt records a specific event and its supported perception: observer, subject/object identity where recognised, time/interval, place, sensory channel and the spatial/event revision. A historical authored observation is explicitly authored history; it is not presented as something the player witnessed live.

Evidence references facts, observations, objects and examination results. It adds the inquiry's requirements for admissibility and independence without copying all of knowledge into a second graph. A rumour repeated by two people retains its common origin. A document copy retains the identity and custody of its original and the act that produced the copy.

Durable evidence cannot vanish when ordinary gossip cools or an actor's six carried-fact slots turn over. Reconcile retention with the shipped knowledge APIs: preserve referenced fact identity and the learned/evidential receipt, while allowing its news heat and unsolicited prompt presence to decay. Do not solve this by increasing every citizen's prompt or by automatically granting them all case facts.

M5 extends that same owner with archived identity and bounded retention; M9 registers inquiry/account roots. Retain transitive provenance as needed, admit finite essential content, and handle saturation without silently deleting accepted evidence. Cold history is separate from hot-news caps, prompt payloads and the player's paginated learned read model. This is an extension of knowledge, not a competing fact graph.

## 6. Statements may be false without changing history

Keep a statement's proposition, speaker, recipients, context and confirmation separate from the underlying event. Corin can deny leaving the counter; that denial cannot rewrite his historical movements.

Free conversation may be evasive, mistaken or unrecorded. A consequential recorded account must have an explicit supported proposition and a legible confirmation step. The shared interaction service can produce the stable core sentence and let the model provide surrounding voice and reaction. The system must not infer a legally meaningful confession from an ambiguous flourish such as “all this is my fault.”

Ordinary speech may already have been heard before a formal action is confirmed. Cancelling or correcting that action preserves the speech's history. Sealed record delivery grants access without speaking its proposition; explicit unsent drafting creates no disclosure. Spoken presentation follows captured event-time hearing, and an undetected listener cannot become a warning, anonymous audience count or refusal oracle. M5/M9/M10 and [SPATIAL_PROOF_PROTOCOL](SPATIAL_PROOF_PROTOCOL.md) own the detailed temporal and knowledge boundaries.

NPCs receive only their own knowledge, current perceptions and allowed strategy. Hidden author truth never enters the player's notebook or an uninformed witness's prompt. A model cannot mint a physical fragment or silently grant permission by describing it.

Ordinary `raise_word` claims remain compatible with this: correlate the spoken event and formal account rather than counting the same utterance as two independent sources.

## 7. Findings evaluate a submitted bundle

The evaluator takes the requested finding, an admissible submitted bundle, supported public examination results and a versioned rule definition. Its interface does not expose the hidden culprit flag, the full world or unlearned case facts.

Use a bounded set of typed predicates: identified object, authenticated comparison, dated observation, independent source origins, established access, supported interval, voluntary confirmed statement and contradictory recorded propositions. Content composes these into alternatives. There is no arbitrary script that can inspect private state to announce the right answer.

The evaluator uses established source relationships, not omniscient causal history. Two identical admissible bundles yield the same finding and diagnostic even if an unlearned leak or planting differs behind them. Newly established contamination can support an amendment later. Bind actor/object/incident/interval variables throughout a rule so valid fragments from unrelated disputes cannot combine into a supported charge.

The result explains the supported conclusion and any missing class of support, within the player's knowledge. “This establishes access, but not who used it” is legitimate. “Find the brass catch under the desk” is not legitimate if the player has never learned that a catch exists.

Keep allegations and requested decisions separate. Recovering property is not proving assault. Clearing Warin is not necessarily identifying Corin. An authorised finding can support a search or arrest order without staging a criminal trial or declaring a final conviction.

## 8. Activities compete with ordinary duties

M1 supplies a minimal operation/resource/duty kernel integrated with the existing round's behaviour ladder. M6/M7 add door/examination adapters; M8 composes accepted operations into activities, appointments, bounded strategies, agreements and service entitlements. M9/M11 later register statement/submission and legal adapters when their services exist. Examples include examining a document, attending an appointment, demonstrating a route, moving stored property, briefing an officer and escorting a prisoner.

An activity owns bounded resources: an actor's movement duty, an object reservation, a work position, a fixture or an access scope. Define priority and interruption rules in one place. Ordinary needs, curfew, custody, urgent danger, player conversation and LLM `go_to` cannot independently overwrite the same actor's route.

An accepted invitation is not a teleport. Participants travel, arrive, wait for a bounded interval, decline or miss the meeting. A review uses recorded statements where procedurally allowed and reschedules a physical demonstration when its necessary participant is absent. Never require the player to escort seven people manually just to begin a scene.

Activities continue off stage under deterministic simulation. The model may choose an allowed strategy or phrase a refusal, but a provider outage cannot indefinitely pin an actor or block an already authorised release.

## 9. Orders create work, not instant outcomes

Separate inquiry findings, institutional authority, orders, assignments and custody. An order specifies a subject or supported description, its grounds, scope, issuer, validity and disposition rules. Officers must acquire it through an explicit authorised channel.

Keep order authority/version, finite issued mandates, learned deliveries, assignment attempts, execution receipts and independent hold grounds orthogonal. Registry recall stops new issue/renewal; field authority follows actual recall receipt or the preissued grant's fixed cutoff. Known hold deadlines/defaults travel with custody, while unscheduled early discharge needs real service. Recall and property return cannot erase unrelated grounds. [LAW_PROTOCOL](LAW_PROTOCOL.md) specifies these rules, registry-check cadence, fair dispatch, recovery budgets, post/keeper coverage, station reservations, written cause and release egress. A single combined order-status enum cannot represent these concurrent facts safely.

An enforcement task uses known leads: an actual last sighting, a known residence/workplace or a report. It never queries a suspect's live transform merely because the simulation stores one. Arrival at an empty address means a failed lead and another decision.

Seizure is an ordinary validated local action. Escort uses traversed routes and speed-bounded movement for both people. A closed door, unavailable keeper, recalled order or second charge has defined consequences. The player can observe the same process that occurs when they are elsewhere.

Property return, gossip cooling, order discharge, review and physical release are distinct transitions. Existing short-term custody safety mechanisms must not repeatedly release and rearrest a person under the same already executed order.

## 10. Authoring data and proving reuse

Use versioned content packs with stable namespaces and explicit dependencies. Manifest identity begins with persistence; each runtime owner ships its typed declarations, loader and validation incrementally. M13 closes cross-module authoring and tooling, rather than first making the systems authorable. A pack binds existing cast roles and places, defines authored incident history, objects/documents, statements, activities, rule alternatives and public outcomes. Shared code owns every verb and transition.

Use bounded, typed declarations with validation. Do not add an unrestricted scripting VM, arbitrary filesystem access or a universal procedural-crime generator as a prerequisite. The required generality is demonstrated by another independently authored investigation using the same capabilities.

The foundation gate requires at least two development scenarios with different people, sites and evidence structure, including one without a culprit and one with changed order of discovery. Merely changing Corin's ID in a copy of the same script does not demonstrate enough reuse.

The real quest must not be loaded to make a guard, door, examination, save or appointment work. Conversely, disabling the real quest must leave a coherent ordinary city with those capabilities intact.

## 11. Bounded cost and readable diagnostics

Long-lived operations run on bounded simulation cadences and indexes. Do not scan every actor against every order, evidence object or visible surface each frame. Use active duties, spatial neighbourhoods, relevant order queues and stable retention rules.

Measure normal cast, 2,000 extra citizens and a 20,000-citizen stress run separately. Existing high-count sim cost is already substantial; do not claim the quest caused all of it or hide a new multiplicative cost inside that baseline.

Diagnostics expose developer truth with explicit scope: command receipt, operation transition, order assignment, lead provenance, navigation revision, save capture revision and restore generation. Player displays are separate and never inherit omniscient debug fields.

## 12. Compatibility is a deliberate contract

Keep current city content working while migrating foundations. New layered navigation must preserve ground-only routes; new law orders must define how existing notices and authored inmates behave; save manifests must identify the actual content and geometry they restore against.

Reject an incompatible save before replacing the running world. A district redesign cannot silently load an old actor inside new masonry. Either provide an explicit tested migration with safe locations and retained references, or report the incompatibility and leave the current world intact.

Document any intentional behaviour change with its replacement and validation. Byte-stable prompt fixtures remain useful for unaffected worlds, but preserving a proven bug is not an acceptance requirement.
