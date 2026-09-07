Status: Partial (2026-09-07). Knowledge/source/ownership reconciliation and CPU baseline delivered; V03 reference-renderer and full stress acceptance remain unavailable. M1 must not advance yet.

# M0 — Baseline and prerequisite gate

Build from what has actually shipped. This milestone closes the knowledge prerequisite, fixes the implementation contracts and identifies risks that could invalidate later content. It does not author the real quest prematurely.

## Execution record — 2026-09-07

- **V01 reconciled:** knowledge M0–M5 was already accepted on 2026-09-06. [BASELINE_RECONCILIATION](BASELINE_RECONCILIATION.md) records the actual interfaces, source privacy, receipt/provenance caps, completed provider evidence and explicitly owned archive extension. Historical records were preserved.
- **V02 reconciled:** source `46e24abf429b3113d3c5745324e5746d82345140`; [persistence inventory](PERSISTENCE_INVENTORY.md) now includes complete shared knowledge, conversation onset/utterance/witness state and generated resident reservations/cursors. The reconciliation maps actual law citizens, migration regressions and three district designs to their owners. Requirements/scenario IDs stay unchanged.
- **V03 partial:** 1,063 current sim tests passed, one ignored. [Runtime evidence](evidence/m0_baseline/README.md) contains 26 sequential release runs / 13 pairs, 0.05-second polls, identical paired semantic counters, raw timing/RSS/source/hardware records, numerical budgets and explicit scope. Default authored, configured 1,000 and target 2,000 additional residents are measured. Production generation admits only 9,072 of the requested 20,000 stress residents; this is capacity-limited stress, not 20,000-body acceptance.
- **Workspace baseline repaired and verified:** the shared host test fixture now registers the skinning asset required by the current dog renderer. All 1,780 workspace tests pass, with eight ignored; [the coordinator review](BASELINE_REVIEW.md) and [archived gate](evidence/m0_baseline/workspace_gate.json) record the initial failure and final rerun.
- **Gate remains unaccepted:** no display or GPU is accessible, including the checked outside-sandbox environment. There is no current host frame/focus/overlay trace. Save/replay/archive DTO byte costs and active foundation workload have finite declared admission targets, but do not exist to measure yet. No dependent gameplay milestone is implemented or marked accepted.

To close M0, obtain the reference renderer and capture ordinary host/frame/input behavior with the same source/content identity, reconcile the stress admission limit honestly, and review V03 against [RUNTIME_BUDGETS](RUNTIME_BUDGETS.md). Later owners must measure their own DTO/active-work bytes and cost before claiming those separate gates. An offline CPU pair or historical software UI screenshot cannot substitute for the missing frame evidence.

## Entry and existing sources

Read [DECISIONS.md](DECISIONS.md), [ARCHITECTURE.md](ARCHITECTURE.md), `features/AGENTS.md`, `crates/cathedral-sim/AGENTS.md`, and `features/implemented/knowledge_and_rumor/README.md`.

At the opening 2026-09-05 planning audit, knowledge M1 was present in the working tree; concurrent work then committed it and continued M2. That dated audit's final status was M1/M2 implemented and M3–M5 pending. The 2026-09-07 reconciliation above supersedes that prerequisite status using the feature's actual later acceptance evidence; its implementation is not attributed to this plan.

## Work

1. Finish and accept knowledge and rumour through its own M5 under its existing plan. Preserve ownership of facts, provenance, player learning, journal, reports and retention. Re-read the final interfaces afterwards; do not implement this roadmap against the unshipped `quest_phase` stub or a guessed M4 journal API.
2. Record a source/status inventory for the sim, backends, host, navigation bake, collision, items, law and UI. Separate implemented behaviour, an existing proposal and a new requirement. Record the commit and relevant working-tree state because concurrent knowledge work is not described by a stale HEAD alone.
3. List every mutable subsystem that a whole-world checkpoint must restore, including state outside `World`. Assign its export/restore owner, time basis and validation obligations. No subsystem may be dismissed because it is absent from `PublicSnapshot`.
4. Reconcile legal authority roles. Current `is_law` covers broad occupations and is not an adequate capability model for evidential orders. Identify the existing citizens who can receive evidence, issue/recall an order, enforce it and keep a prisoner.
5. Survey the Tallage and nearby district as a production risk. Identify possible floors, stairs, entrances, public alternatives, witness positions and acoustic openings. A design sketch is enough for this gate; a claimed timing certificate is not. The full authored redesign belongs to M14 after the reusable foundation gate.
6. Freeze milestone contracts and acceptance scenario IDs. Identify old behaviours that must migrate: global notice knowledge, LLM-only arrest choice, direct escort placement, notice-linked automatic release and radius-only following.
7. Inventory current tests and running-game tools. Capture a baseline for default cast and ordinary play before replacing broad systems.

Freeze an executable workload/budget record under [RUNTIME_BUDGETS](RUNTIME_BUDGETS.md), including numerical CPU/frame/byte limits, source/hardware identity and paired-run statistics. Existing provisional targets and canaries are explicit inputs; they are not already measured acceptance. Reconcile shared knowledge's archived-identity/quota extension before promising that referenced evidence survives its live-fact cap.

## Timing risk that must be carried forward

The GDD's illustrative private-trip check divides 48 m by 2.1 m/s and adds eight action seconds, giving about 30.86 s. The fixed narrative also includes a three-second delay before departure, so its complete reference journey returns at about 33.86 s. That fits nominal 36 s, but does not satisfy a stronger claim that this complete itinerary fits every interval down to 32 s.

Resolve the intended proof precisely before authoring a final interval. Include departure delay, opening/closing, stairs, interaction duration, waypoint behaviour, crowd effects and measurement uncertainty. The desired robust contract is a private execution that fits the conservative permitted interval while every relevant public alternative exceeds it. Re-author numbers and event timing where necessary; do not preserve a misleading “passed” label from the illustrative validator.

Street-graph A* returns a route through that graph. It does not prove the shortest route a freely moving player can take through the collision world. M4/M14 must account for free walking, corners, jumps and other permitted movement when making a public-route impossibility claim.

## Outputs

- A dated source audit and complete persistence inventory.
- Final knowledge integration contracts, including retained evidence references and player receipts.
- A capability/role map for the law and its ordinary communication channels.
- A list of viable district designs and the measurements that will select among them.
- Requirement-to-milestone and acceptance mappings without unresolved ownership.
- A baseline validation record with commands, results and material limitations.

## Advance only when

- Knowledge M0–M5 has actual acceptance evidence, and this plan references its shipped APIs.
- Every new requirement has an owner; every existing subsystem that can affect continuation has a checkpoint policy.
- The team can explain which current law/perception behaviours would invalidate this mystery and where they are replaced.
- The route arithmetic is framed as a measurement task with explicit uncertainty, not a promise that the proposed site already exists.
- No game/lore/backlog status has been marked implemented merely because this planning file exists.

## Verification and handoff

Use `cargo test -p cathedral-sim --tests` for the deterministic baseline. Run the completed knowledge feature's documented headless and live-provider acceptance; its own tests and cadence targets remain authoritative.

For movement-faithful M0 measurements, use an existing `Engine::poll` test/harness path with accepted increments at most 0.05 seconds, or a valid ordinary host run. The current coarse `--watch-clock` does not spend every physical span and cannot certify this baseline. Its production/developer-runner repair belongs to M1/M4; M0 need not depend on that future code to establish controlled inputs. Record hardware, poll pattern and source state with each timing result.

Hand M1 the final clock/input contract and saved-state inventory. Hand M4 the spatial risk brief. Hand M9 the real knowledge APIs. Hand M11/M12 the audited law gaps. A later agent must be able to start from these artefacts without reconstructing this conversation.
