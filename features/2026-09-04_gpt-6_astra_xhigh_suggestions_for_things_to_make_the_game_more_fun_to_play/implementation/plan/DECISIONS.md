Status: Authoritative user decisions recorded (2026-09-05); incorporated into the delivered implementation plan.

# Decisions that govern the implementation

These decisions come from the user's answers on 2026-09-05. They override conflicting recommendations in the 2026-09-04 GDD. This plan concerns **An Alibi in Stone**, the investigation centred on witness movements, a private upper route, stolen papers and an assault. The culprit's canonical name is **Corin Copp** (`fc9rn`).

## D01 — Deliver reusable systems first, then the full playable quest

The end product is the complete playable investigation. Before authoring it into the city, implement the general systems needed to make further quests of this kind. A thin script that makes this one case appear to work does not meet the request.

Prove foundations with small, separately authored development fixtures. Their purpose is to exercise reusable behaviour without introducing An Alibi in Stone's character IDs, evidence recipes or special outcomes into engine code. Ship the completed shared knowledge and rumour feature before quest work, preserving the existing schedule decision in its README.

The early playable milestone is a review point, not the final scope. Public and private submissions, the hush bargain, early and late evidence recovery, movement of evidence, reopened inquiries, lasting consequences and full persistence remain in the delivery.

## D02 — An established incident in ordinary city history

The crime happened before the investigation opens. Its fixed history is installed when the relevant world/campaign is created, not invented when the player accepts a quest or first asks about it. The investigation takes place in normal gameplay.

A developer shortcut may create a reproducible fresh world at a useful starting position and time. It must use the same production systems and content. It must not patch fabricated evidence into an existing save or bypass discovery rules in ordinary play.

Simulating the original crime live, and allowing the player to prevent it, is not required for this case. Reconstructing it through ordinary movement and interaction is required.

## D03 — District redesign is authorised

A larger redesign of the Tallage district is allowed if it produces a better investigation. The plan must assess streets, buildings, interiors, sightlines, acoustic connections and other citizens' journeys together.

The GDD's 576 m public circuit, 48 m private circuit and 32–40 second interval are design proposals, not protected numbers or surveyed geometry. Keep the logical relationship and make the places convincing. Re-measure every legal route after changes. Geometry, navigation, place records, doors, housing assignments and collision must agree.

This is authorisation to design the district well, not to install invisible blockers, secretly alter movement speed or reject an unanticipated route that actually works.

## D04 — Conversation and explicit controls, with clear feedback

Support natural conversation and explicit evidence/action controls. Make it very clear to the player what is happening.

Interpretation of language proposes an action; the authoritative simulation checks its current preconditions and commits the actual result. The player must distinguish a spoken intention, a pending request, an accepted undertaking, an action in progress and a completed or interrupted action. A character saying “I will arrest him” is not an arrest.

Provide typed and voice input, readable records, specific reasons for unavailable actions, and equivalent explicit controls for consequential operations. The interface must not reveal unknown clues, decide the culprit for the player or turn hearsay into a witnessed fact.

## D05 — The city never pauses for investigation

The user's instruction is explicit: **“Keep the city running during conversations and reading — never pause! This plays out during normal gameplay. No special ‘quest mode’!”**

This overrides the GDD's pause policy. Opening a document, composing speech, waiting for recognition or cognition, selecting evidence, previewing a submission, using the notebook or opening an in-game menu must not stop the simulation clock, schedules, movement, appointments, needs or enforcement.

The UI must tolerate changing state. Commit-time validation handles expired consent, an absent recipient, moved property or an amended order. Preserve drafts and reading position. Show meaningful changes without making players race the interface or concealing a missed event. Missing one appointment must leave a coherent record and opportunities to continue.

Normal social behaviour may cause a particular citizen to stop and talk, provided obligations can interrupt them. This never freezes the rest of the city. Timed demonstrations use the same advancing simulation as everyone else.

A coherent save capture at a simulation boundary is an implementation operation, not permission to introduce a gameplay pause. Disk work must occur outside the simulation pump.

## D06 — Proper whole-world save/load, available anywhere

Full persistence is part of the delivery, despite its size. Saving only the visible snapshot or a quest-stage integer is insufficient.

Restore the world, people, inventories, knowledge, schedules, appointments, custody, evidence, permissions and long-running operations coherently. Capture a durable point in simulation history without waiting for a provider request, an escort or a conversation to finish. Loading must not duplicate committed effects or accept completions from the world that was replaced.

As a planning default, loading resumes the saved simulation time; time spent with the application closed does not secretly advance the city. This is distinct from D05, which applies while normal gameplay is running. Document this default in the save UI and architecture contract.

## D07 — Consequences are actions by the ordinary city

Findings can produce lasting changes through law, schedules and knowledge. Enforcement should be as dynamic as possible: an arrest order names Corin, officers acquire that order, available guards pursue it, and an actual encounter can lead to seizure, escort and custody.

An order's issue, receipt, assignment, search, encounter, seizure, escort, commitment, suspension and discharge are different states. They must not collapse into a quest callback that moves Corin to prison or declares him arrested while no guard has reached him.

The system must handle absence, stale information, interrupted journeys, locked premises, competing duties, an unavailable officer, a corrected finding and a suspect already held for another matter. Nobody gains the suspect's live position or private evidence merely because an order exists.

Use and, where necessary, rework the shipped law and movement systems. Do not build a separate quest police service. A quest emits a supported request or authorised order; the city performs its consequences.

## Changes from the GDD

| GDD recommendation | Governing implementation decision |
|---|---|
| Pause during blocking investigation interactions | D05: always-running ordinary gameplay |
| First build concentrates on one bounded case | D01: build and prove reusable foundations before the full case |
| Preserve existing anchors and make local changes | D03: larger district redesign permitted, with a full consistency audit |
| Public finding may directly stage an escort | D07: an order enters ordinary dynamic enforcement; arrest is a subsequent physical event |
| Save/load can be the final broad milestone | D06: establish persistence early and require every new subsystem to participate |

The original DOCX/PDF remain the design proposal. This decision record and the milestone plan are the implementation authority where they differ.
