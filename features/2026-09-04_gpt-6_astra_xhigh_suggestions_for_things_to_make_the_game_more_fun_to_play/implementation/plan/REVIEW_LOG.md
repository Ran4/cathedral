Status: Planning review completed (2026-09-05). Game implementation milestones remain unaccepted.

# Evidence and limits of this planning pass

This log records work actually performed while building the implementation plan. Planned Vxx scenarios live in [VERIFICATION.md](VERIFICATION.md); their presence is not a test result.

The requested sustained review began at **2026-09-05 09:01:00 UTC** and its closing review was recorded at **2026-09-05 12:01:42 UTC**: **10842 seconds**, exceeding the three-hour minimum. This is elapsed review/work time from the recorded clock readings, not a sum of agent runtimes or a claim that game implementation occurred.

## Source baseline and concurrent work

The opening audit used HEAD `f56a2c306ec1523fc569c7c2de232b8c90d4d4ef` with uncommitted knowledge M1 and other unrelated work already present. During the pass, separate repository work committed knowledge goldens as `18bd26d` and M1 as `0504d29`, then continued pollen/M2 changes. Early reads reported M2–M5 pending; the final status read reports M2 implemented and M3–M5 pending. The headless pollen measurement mode also acquired a 0.4-second default and corrected its movement-budget comment during this concurrent work.

This pass did not implement or claim that concurrent work. No branch/worktree was created. Game code, lore, configuration and backlog ordering were not edited for the plan. Plan files and their source-data illustration are separate from the pre-existing DOCX/PDF/manuscript/model.

## Actual checks

| Check | Actual result | Limit |
|---|---|---|
| `cargo test -p cathedral-sim --tests` on the opening working tree | Exit 0; existing unit/integration suites passed | Baseline only, before later concurrent pollen changes. Does not exercise the proposed systems |
| Source audit of engine, scheduler, Night, clock, round, inventory, law, nav, collision, host and lore | Concrete gaps recorded in [SOURCE_AUDIT](SOURCE_AUDIT.md) and milestone contracts | Static source is not proof of new runtime behavior |
| Original GDD model/content review | Identified missing departure delay, incomplete physical theft recipe, alibi/contradiction conflation, missing private detail and late-search gaps | Original model/validator remain unchanged; their old pass is not acceptance of the repaired case |
| Hidden-window Bevy survey | Compilation succeeded; watchdog exit 124 at first screenshot request; screenshot directory empty | No successful visual survey or GPU performance claim |
| `uv run evidence/render_site_audit.py` | Generated current-footprint PNG/SVG and source/hash report; diagram viewed and bottom-label layout corrected | Source-data illustration only; no proposed room, path or timing certificate |
| `uv run evidence/plan_catalog.py --write` | Generated coverage for 40 requirements, 78 planned scenarios and 20 milestones | Structural traceability, not game tests |
| First `uv run evidence/validate_plan.py --write`, 11:16 UTC | Passed 20 milestone/status checks, 37 Markdown files, 696 local links/anchors and two site-source hashes | Structural scope only; the final report is regenerated after later edits |
| `uv run evidence/probe_design.py`, 11:16 UTC | 17 assertions reproduce original arithmetic, recipe and schedule observations | A finite design probe, not an implementation or repaired-case certificate |

## Hidden-window survey record

The attempted command, from the repository root, was:

```sh
CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 CATHEDRAL_DRIVE_TIMEOUT=120 \
CATHEDRAL_DRIVE='wait-online; weather clear; tp -215 95 160 0 -50; sleep 3; shot plan_tallage_overview; tp -190 1.8 54 0 -4; sleep 2; shot plan_pawnshop_front; tp -206 7 48 270 -10; sleep 2; shot plan_pawnshop_side; tp -213 2 42 180 8; sleep 2; shot plan_tally_bridge; tp -218 2.1 53 -30 -5; sleep 2; shot plan_weighbeam; tp -213 65 65 0 -89; sleep 2; shot plan_tallage_overhead; quit' \
cargo run > /tmp/alibi_plan_tallage_survey.log 2>&1
```

The renderer logged software-only support at `09:43:47 UTC`. Drive reached `shot plan_tallage_overview` at its 11.7-second mark and then hit the 120-second watchdog. The directory [session 777 screenshots](../../../../logs/session_777_2026-09-05_11_43_46/screenshots/) was empty when checked. The run also logged cursor-confinement and audio-loader errors. No claim is made about their cause beyond the recorded messages; they were not repaired by this planning pass.

The headless flag was used throughout. No synthetic desktop input was sent. This failed run was not retried as a slow software-rendering benchmark. The useful fallback was to illustrate the actual cadastral/navigation source, explicitly labeled as such.

## Sequential review and concrete changes

Fresh-context reviewers were used one at a time under `features/AGENTS.md`'s ESPFEIT instruction. Read-only findings were integrated by the coordinator; the ownership and player-experience deliverables each had one explicitly bounded file owner.

| Review | Findings incorporated into the plan |
|---|---|
| Current law/custody | Scoped capabilities, separate allegations, firsthand-vs-hearsay authority, validation before mutation, explicit order lookup, physical escorts and release semantics |
| Current geometry/perception | Layered surfaces, stable entrances, shared physical portal state, actual stairs/interiors, occlusion consumers and conservative route claims |
| Case logic | Continuous packet/shove account, same-wallet dating, separate alibi/handling propositions, private corroboration, independent Warin clearance and noncircular late searches |
| Milestone dependencies | M1 minimal operation kernel before M6/M7, M4 physical portals before M5/M6, incremental adapters/loaders, M15 basic retrieval before M16, explicit agreements and sponsorship owners |
| Persistence | Accepted physical watermark, fixed residuals, exact held completions, load-specific idle retry, Night duty identity, valid Never sentinels, novelty salt, host vermin/custody, atomic adoption and budgeted retirement |
| Evidence/interface | Cross-case variable binding, established-vs-hidden provenance, live privacy conditions, unknown appointment outcome and unpaid redemption state |
| Dynamic enforcement | Actual briefing cadence, fair queue/parking, post coverage, station capacity/reservations, multiple hold grounds, scoped booking, written cause and physical release egress |
| Spatial proof | Numerical margin/coverage protocol, Lise's complete sighting endpoints, observable item descriptors, intermediate observation gaps, witness coverage of hidden work and movement-faithful late starts |
| Cost/acceptance | Accepted-time/debt policy, bounded generations/callback cohorts, archival knowledge identity, replay-window exhaustion, existing prompt/snapshot canaries and versioned acceptance artifacts |
| Ownership handoff | Shared producer/consumer map, knowledge prerequisite, keys subset boundary, concrete M1–M3 cuts and M13-before-M14 gate |
| Player experience | Concrete spatial/social discoveries, legitimate Warin-only completion, understood disclosure/economic choices and proposed five-to-eight-session formative criteria |
| Final dependency/calendar consistency | M1 in-memory tests precede M2 continuation, M8 agreements precede M9 disclosure adapters, civil/case anchors and independent expiry, binding hush terms and actual Lise access consequences |
| Voice/privacy | Ordinary speech survives cancellation, formal actions require confirmation, explicit dictation remains unsent, temporal STT audience is captured, hidden-listener warnings/counts cannot leak information |
| Final case reachability | Lise's existing lodged account also identifies the particular upper-room desk before its search; all other reviewed late/refusal, Warin-only and money/property paths remained coherent |
| Recall and deadline causality | Registry decisions versus finite issued mandates, physical recall receipt, in-transit grants, local version tombstones, keeper-carried expiry defaults and exclusive boundary ordering |
| Closing durability/source reconciliation | Immutable save generations with recoverable acknowledged references; pollen M2 holdings, mint context, per-person cursors and content-keyed derived caches added to the continuation inventory |

These changes are implementation requirements, not implemented fixes. The original GDD remains a historical design proposal where [DECISIONS](DECISIONS.md) and [CASE_CONTRACT](CASE_CONTRACT.md) explicitly supersede it.

## Remaining implementation evidence

No new quest, world save/load, layered navigation, evidence evaluator or dynamic dispatch system was built by this planning pass. No live-provider investigation acceptance or human tester session occurred. No proposed district route has a measured fairness certificate. Numerical reference-hardware gates, complete knowledge acceptance and every Vxx production scenario remain work for their owners.

The [final structural validation record](evidence/structural_validation.json) covers local links/anchors, 20 milestone statuses, 40 requirements, 78 planned scenarios, the two site-source hashes and the original model hash. The finite design probe records 17 arithmetic/recipe/schedule observations. Source, case, dependency and protocol reviews resolved the concrete contradictions documented above. These checks do not certify the unimplemented gameplay scenarios or retire their production risks.
