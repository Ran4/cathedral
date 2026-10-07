Status: Reference contracts (2026-09-05), trimmed 2026-10-07 to §1, §3, §6–§9 and §12 (PLAN_REVIEW item 4). The milestone files govern where they differ; earlier sections are in git history.

# Architecture contracts

The deliverable is a living city that can host investigations. Case content supplies people, places, records, activities and evidence rules. Shared systems decide whether actions are possible and perform them. These contracts apply to both the development fixtures and the final quest.

Names below are proposed, not claims about existing code. Existing shipped APIs take precedence over speculative signatures.

## 1. Keep authority in the simulation

`cathedral-sim` remains pure Rust without filesystem, network, threads or Bevy. It owns mutable world state, commands, permissions, observations, activities, evidence, findings, legal orders and their transitions. `cathedral-backends` owns IO and provider services. Bevy projects state and gathers player input.

Shared spatial data must become sufficient for headless navigation and visibility tests. Bevy and the sim consume the same authored surfaces, portals and occluders; a render-only raycast is not the sole authority for an off-stage guard. Rendering may use richer art, but an opaque structural wall or usable stair cannot exist in only one half.

The private checkpoint is separate from `PublicSnapshot`. It contains the secrets required to restore the world and must never be installed as a UI mirror. UI read models expose only the player's learned records, visible state and authorised institutional information.

## 3. A command is a request; its receipt reports reality

Route natural-language proposals and explicit controls through the same command services. A proposed envelope contains a command identity, actor, target references, requested operation and relevant expected revisions. Do not force callers to supply the entire global `world_revision`: unrelated crowd movement must not make every action stale.

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

## 6. Statements may be false without changing history

Keep a statement's proposition, speaker, recipients, context and confirmation separate from the underlying event. Corin can deny leaving the counter; that denial cannot rewrite his historical movements.

Free conversation may be evasive, mistaken or unrecorded. A consequential recorded account must have an explicit supported proposition and a legible confirmation step. The shared interaction service can produce the stable core sentence and let the model provide surrounding voice and reaction. The system must not infer a legally meaningful confession from an ambiguous flourish such as “all this is my fault.”

Ordinary speech may already have been heard before a formal action is confirmed. Cancelling or correcting that action preserves the speech's history. Sealed record delivery grants access without speaking its proposition; explicit unsent drafting creates no disclosure. Spoken presentation follows captured event-time hearing, and an undetected listener cannot become a warning, anonymous audience count or refusal oracle. [M4](M4_accounts_inquiries_findings.md) and [M5](M5_rooms.md) own the detailed knowledge and perception boundaries.

NPCs receive only their own knowledge, current perceptions and allowed strategy. Hidden author truth never enters the player's notebook or an uninformed witness's prompt. A model cannot mint a physical fragment or silently grant permission by describing it.

Ordinary `raise_word` claims remain compatible with this: correlate the spoken event and formal account rather than counting the same utterance as two independent sources.

## 7. Findings evaluate a submitted bundle

The evaluator takes the requested finding, an admissible submitted bundle, supported public examination results and a versioned rule definition. Its interface does not expose the hidden culprit flag, the full world or unlearned case facts.

Use a bounded set of typed predicates: identified object, authenticated comparison, dated observation, independent source origins, established access, supported interval, voluntary confirmed statement and contradictory recorded propositions. Content composes these into alternatives. There is no arbitrary script that can inspect private state to announce the right answer.

The evaluator uses established source relationships, not omniscient causal history. Two identical admissible bundles yield the same finding and diagnostic even if an unlearned leak or planting differs behind them. Newly established contamination can support an amendment later. Bind actor/object/incident/interval variables throughout a rule so valid fragments from unrelated disputes cannot combine into a supported charge.

The result explains the supported conclusion and any missing class of support, within the player's knowledge. “This establishes access, but not who used it” is legitimate. “Find the brass catch under the desk” is not legitimate if the player has never learned that a catch exists.

Keep allegations and requested decisions separate. Recovering property is not proving assault. Clearing Warin is not necessarily identifying Corin. An authorised finding can support a search or arrest order without staging a criminal trial or declaring a final conviction.

## 8. Activities compete with ordinary duties

There is no generic activity kernel: rounds, the rung ladder and `TravelIntent` already exist. M4 adds a dated review trigger, M6 a dated errand (also used for demonstrations) and M7 agreements. Examples include examining a document, attending an appointment, demonstrating a route, moving stored property, briefing an officer and escorting a prisoner.

An activity owns bounded resources: an actor's movement duty, an object reservation, a work position, a fixture or an access scope. Define priority and interruption rules in one place. Ordinary needs, curfew, custody, urgent danger, player conversation and LLM `go_to` cannot independently overwrite the same actor's route.

An accepted invitation is not a teleport. Participants travel, arrive, wait for a bounded interval, decline or miss the meeting. A review uses recorded statements where procedurally allowed and reschedules a physical demonstration when its necessary participant is absent. Never require the player to escort seven people manually just to begin a scene.

Activities continue off stage under deterministic simulation. The model may choose an allowed strategy or phrase a refusal, but a provider outage cannot indefinitely pin an actor or block an already authorised release.

## 9. Orders create work, not instant outcomes

Separate inquiry findings, institutional authority, orders, assignments and custody. An order specifies a subject or supported description, its grounds, scope, issuer, validity and disposition rules. Officers must acquire it through an explicit authorised channel.

Keep the order, what each officer has actually received, assignment attempts and independent hold grounds apart. A recall travels to officers the way the order did (Q9), so a distant guard can still act on what they last heard. Known hold deadlines travel with custody, while unscheduled early discharge needs real service. Recall and property return cannot erase unrelated grounds. [M7](M7_orders_enforcement_agreements.md) owns the scope: an explicit order-state enum for D07's ten states, with recall and independent hold grounds kept beside it rather than folded into it.

An enforcement task uses known leads: an actual last sighting, a known residence/workplace or a report. It never queries a suspect's live transform merely because the simulation stores one. Arrival at an empty address means a failed lead and another decision.

Seizure is an ordinary validated local action. Escort uses traversed routes and speed-bounded movement for both people. A closed door, unavailable keeper, recalled order or second charge has defined consequences. The player can observe the same process that occurs when they are elsewhere.

Property return, gossip cooling, order discharge, review and physical release are distinct transitions. Existing short-term custody safety mechanisms must not repeatedly release and rearrest a person under the same already executed order.

## 12. Compatibility is a deliberate contract

Keep current city content working while migrating foundations. New layered navigation must preserve ground-only routes; new law orders must define how existing notices and authored inmates behave; a save identifies, by format version and content fingerprints, the map it restores against.

Loading resumes the saved simulation time; time spent with the game closed does not advance the city (D06). The save UI says so.

Reject an incompatible save before replacing the running world. A district redesign cannot silently load an old actor inside new masonry. Either provide an explicit tested migration with safe locations and retained references, or report the incompatibility and leave the current world intact.

Document any intentional behaviour change with its replacement and validation. Byte-stable prompt fixtures remain useful for unaffected worlds, but preserving a proven bug is not an acceptance requirement.
