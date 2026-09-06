Status: Implementation plan delivered (2026-09-05). M0–M19 describe required implementation; no game milestone in this plan is accepted yet.

# An Alibi in Stone — systems first, then the full quest

Build the machinery for investigations that happen inside a living city, then use it to deliver the complete quest. People keep their own time. Physical evidence has a location and a history. A finding may justify an order; available guards must then carry it out. The player can save at any moment and resume a coherent world.

The [authoritative decisions](DECISIONS.md) override the GDD where they differ. In particular, there is **no quest mode and no pause for reading, talking or selecting evidence**. A larger district redesign is authorised. Full world save/load and reusable systems are required parts of delivery.

For a design review, read the [case contract](CASE_CONTRACT.md) and [player experience](PLAYER_EXPERIENCE.md) next. For implementation, read [ownership and gates](OWNERSHIP_AND_GATES.md), then the next accepted milestone's successor and its relevant protocols. The [40 requirements](REQUIREMENTS.md) and [78 acceptance scenarios](VERIFICATION.md) make scope and verification traceable without requiring every implementer to reread the entire plan.

## Delivery order

The foundation gate is deliberately substantial. M0–M13 establish and prove the reusable systems using development fixtures. M14–M18 author and integrate the real district and complete investigation. M19 is the final acceptance gate. Tests, persistence and player feedback accompany each milestone rather than waiting for M19.

| Milestone | Deliverable | Observable result |
|---|---|---|
| [M0 — Baseline and prerequisite gate](M0_baseline_and_prerequisites.md) | Reconcile the completed knowledge feature, current code and production risks | A verified starting point and explicit contracts; no competing fact store |
| [M1 — Live time and committed actions](M1_live_time_and_transactions.md) | Advancing time, receipts, generations and a minimal operation/duty kernel | Reading never pauses the city; work and resource ownership are explicit |
| [M2 — Complete simulation checkpoints](M2_simulation_checkpoints.md) | Export/restore all authoritative state and rebase host time | A deterministic run continues coherently across an engine replacement |
| [M3 — Save-anywhere in the application](M3_save_load_application.md) | Atomic files, version checks, asynchronous writes and complete host restoration | Save or load during normal play without lost evidence or duplicated actions |
| [M4 — Places on more than one floor](M4_layered_navigation.md) | Shared spatial data, traversable interiors, stairs and dynamic route revisions | Player and NPC can take the same real upper route |
| [M5 — Seeing, hearing and remembering observations](M5_perception_and_observations.md) | Local perception with occlusion and durable observation provenance | A witness learns what they could perceive; following survives walls and lost sight |
| [M6 — Doors, authority and permission](M6_access_and_permissions.md) | Physical access, scoped consent, keys and entry operations | A real door and route respond to permission, possession and current state |
| [M7 — Objects that can be investigated](M7_objects_and_examinations.md) | Placed objects, documents, containers, comparison and custody receipts | A paper or fragment can be found, moved, compared and accounted for |
| [M8 — Undertakings and appointments](M8_activities_and_appointments.md) | Composite activities, strategies, agreements, favours and scheduled meetings | Citizens actually attempt promised work and report fulfilment or interruption |
| [M9 — Statements, evidence and findings](M9_inquiries_and_evidence.md) | Recorded accounts, provenance, submissions and bounded proof rules | Independent evidence supports a specific finding without reading hidden guilt |
| [M10 — Clear conversation and controls](M10_conversation_and_interface.md) | Shared action semantics, explicit controls, receipts and a live notebook | The player understands what was said, requested, done and still pending |
| [M11 — Orders and lawful dispositions](M11_orders_and_dispositions.md) | Evidence-backed orders, separate allegations and explicit discharge | Returning property cannot silently erase an unrelated assault allegation |
| [M12 — Guards execute their orders](M12_dynamic_enforcement.md) | Informed assignment, search, encounter, seizure, escort and custody | An off-stage guard can find and arrest a suspect through ordinary simulation |
| [M13 — Authoring tools and reusable-system gate](M13_authoring_and_foundation_acceptance.md) | Validated content packs, development controls and independent scenarios | Another investigation can be authored without adding case-specific engine branches |
| [M14 — Build the investigative district](M14_tallage_district.md) | Finished connected places, props and verified routes/sightlines | The shortcut is an enjoyable spatial discovery in a functioning district |
| [M15 — Install the fixed history](M15_incident_and_cast.md) | Seven cast roles, original incident, object history and honest witness knowledge | The mystery already exists whether or not the player engages with it |
| [M16 — The complete core investigation](M16_investigation_and_public_review.md) | Discovery, demonstrations, independent clearance and public findings | The player can clear Warin and substantiate the case through their own reconstruction |
| [M17 — Alternatives and moving evidence](M17_alternatives_and_recovery.md) | Private resolution, hush bargain, observation, early/late recovery and reopening | Different approaches and missed events produce coherent continuations |
| [M18 — Life after the finding](M18_lasting_consequences.md) | Integrated law, work, relationships, records and repeat visits | The district changes because citizens act on what happened |
| [M19 — Full acceptance and release](M19_acceptance_and_release.md) | Long-running, save/load, accessibility, provider and human play acceptance | A complete, legible investigation that remains sound when players depart from the walkthrough |

Every milestone is an implementation/review unit under `features/AGENTS.md`: use ESPFEIT, one milestone agent at a time with a fresh context and a bounded handoff. Do not implement dependent milestones against speculative outputs. Read the shipped interfaces again at each gate.

## Existing work and ownership

Knowledge and rumour is a separate shared feature. At the final 2026-09-05 status check, its README reports M0 measured and M1/M2 implemented, with M3–M5 pending; M2 advanced through concurrent work during this review. Its existing end-to-end-first decision remains in force. This roadmap consumes that feature after its acceptance gate; it does not duplicate or silently replace its remaining milestones.

Law, movement, items, speech, weather and ordinary NPC rounds already exist. Their implementation is the starting point, including fixes and extensions required by this plan. Historical files under `features/implemented/` are records and must not be rewritten to pretend the new work has shipped.

Source names and proposed contracts do not certify that a new API or command already exists. Status changes need implementation evidence. The plan explicitly reconciles the separate keys proposal as a required subset; it does not mark that broader feature complete.

The review repaired material gaps in the earlier GDD: the complete private trip omitted a departure delay from its conservative timing check; the physical route did not satisfy the old theft recipe; late wallet/desk search needed noncircular grounds; and the original review time could not wait for a slow reader. The case contract also fixes calendar anchoring, independent hold expiry, private versus hush terms and the unpaid pledge's actual property outcome. These are stated changes, not hidden assumptions that the old validator already proves them.

## Non-negotiable acceptance conditions

1. Reusable foundations pass independent development scenarios before real-case authoring starts.
2. All ordinary simulation continues while the player reads, types, speaks, waits for a reply or uses an in-game menu.
3. Language never creates physical proof or commits an action outside the simulation's current permissions and preconditions.
4. Possibility, identification, theft, assault, recovery and release are separate findings with explicit supporting evidence.
5. An order is not an arrest. Guards must learn, travel, encounter and act; scene presence is not required for their work to progress.
6. Save/load covers the whole authoritative city and every new system. Old asynchronous results cannot mutate a newly loaded world.
7. Navigation, collision, access, perception and visual geometry describe the same spaces. Legal alternative routes remain legal.
8. The player can miss events, decline disclosure, clear only Warin or return later without corrupting the mystery.
9. No important consequence relies on an LLM getting an idle turn near the player.
10. Completion requires measured implementation behaviour and human play evidence, not only a passing authored walkthrough.

## Review documents

| Document | Purpose |
|---|---|
| [Decisions](DECISIONS.md) | The user's authoritative choices and the GDD policies they replace |
| [Architecture](ARCHITECTURE.md) | Shared responsibilities, time, operations, knowledge, evidence and enforcement contracts |
| [Source audit](SOURCE_AUDIT.md) | Verified current implementation, concrete gaps and audit limits |
| [Ownership and gates](OWNERSHIP_AND_GATES.md) | Producers/consumers, dependency order, keys overlap, subcuts and sequential handoffs |
| [Persistence inventory](PERSISTENCE_INVENTORY.md) | Existing/planned state owners and restore policies |
| [Checkpoint protocol](CHECKPOINT_PROTOCOL.md) | Exact capture/adoption, pending cognition, time, callbacks and durable slot policies |
| [Spatial proof protocol](SPATIAL_PROOF_PROTOCOL.md) | Traversal coverage, complete timing, perception continuity and historical applicability |
| [Law protocol](LAW_PROTOCOL.md) | Briefings, fair dispatch, post coverage, station capacity, custody grounds and actual release |
| [Runtime budgets](RUNTIME_BUDGETS.md) | Accepted time, resource admission, archival knowledge, replay retention and measurement gates |
| [Case contract](CASE_CONTRACT.md) | Complete quest requirements and explicit repairs to the GDD's evidence/timing gaps |
| [Player experience](PLAYER_EXPERIENCE.md) | Concrete play moments, meaningful choices and formative iteration criteria |
| [Requirements](REQUIREMENTS.md) | D01–D07 mapped to 40 testable requirements |
| [Verification](VERIFICATION.md) | 78 planned behavioral scenarios, grouped by owning milestone |
| [Risks](RISKS.md) | Failures that block delivery and the evidence needed to retire them |
| [Review log](REVIEW_LOG.md) | Actual planning checks, source concurrency, failed visual survey and remaining evidence |

## Inspecting and maintaining the plan

The [existing Tallage illustration](evidence/tallage_existing.png) is generated from current cadastral/navigation data, with [source hashes and coordinates](evidence/site_audit.json). It is not a screenshot or a validated proposal for new interiors. The attempted headless visual survey produced no screenshot; that limit remains explicit in the review log.

From this plan directory:

```sh
uv run --cache-dir /tmp/cathedral-gdd-uv evidence/plan_catalog.py
uv run --cache-dir /tmp/cathedral-gdd-uv evidence/probe_design.py
uv run --cache-dir /tmp/cathedral-gdd-uv evidence/validate_plan.py --write
```

The first checks generated requirement/scenario tables; use `--write` after editing their catalog source. The second reproduces finite arithmetic and original-model recipe observations. The third checks local links/anchors, milestone status, generated coverage, current site-source hashes and the probe's original-model hash. [design_probe.json](evidence/design_probe.json) and [structural_validation.json](evidence/structural_validation.json) record their scope. None is a claim that the planned gameplay acceptance scenarios have run.

Regenerate the site illustration with `uv run --cache-dir /tmp/cathedral-gdd-uv evidence/render_site_audit.py` after its source geometry changes. Keep the original GDD generation separate until its manuscript/model are deliberately reconciled with the implementation authority.
