# M2a3 — Round authority checkpoint evidence

Status: Implemented and reviewed; focused/public, full workspace and coordinator release audits pass. Complete envelope and host acceptance remain pending. Base: M2a2 `3e9c93f5354176b2f506c50ca4de5c63e1de7f17` (2026-09-08).

This cut adds the complete private Round component: residents and reservation indexes, water/food service, production and market planning, household accounting, lamps, road parties, temporal cursors and retained publications. It composes with the real M2a2 actors/items/places through a borrowed context, including an unadopted backbone candidate. It does not construct a replacement World/Engine or expose a partial full-save envelope.

## Ownership and interfaces

[OWNER_COVERAGE.md](OWNER_COVERAGE.md) lists every field family, historical-reference policy and remaining owner. [ADMISSION.md](ADMISSION.md) gives the concrete allocation argument and pending complete-envelope limits.

`Round::export_checkpoint` returns an admitted `RoundDtoV1`; encode consumes it into admitted bytes. `RoundDtoV1::decode` admits the raw input and definition scratch before typed parsing, then validates strict records and references. `into_candidate` validates the constructed indexes and moves them into an opaque `RoundCandidate`. The reservation remains attached through every representation and destruction. No public candidate adoption method is supplied. `RoundCheckpointContext::from_backbone` supplies the future candidate-to-candidate seam without seeding a World.

Exact context bindings cover the original navigation JSON and bitset, catalog, shelter definitions, and embedded Round/Food/Homes inputs. Lazy navigation cache warmth does not affect compatibility. Shared pure production helpers resolve trades, sites, recipes, resident shelters, daily prose and vendor listings, so checkpoint validation follows ordinary owner rules. Retained recipes and static geometry are checked against the installed declarations; missing production plans leave stranded Inventory obligations for the ordinary watchdog.

Two ordinary paths changed to support safe exact continuation:

- Pollen due keys now preserve signed calendar ordering and use an explicit initially-due sentinel. A negative calendar can rearm a future poll without continually popping/reinserting key zero. Positive calendar cadence and legitimate stale re-enrollment entries are retained.
- Residents retain the inputs of their last published status projection. Speech interruption and later movement can change the controller after publication; validation compares the saved cache with its sampled inputs. The ordinary publication predicate is preserved, and its occupied-handle String allocation is reused.

The coordinator’s matched ordinary CPU comparison passes the unchanged default/+2,000 p95/p99 threshold; its raw samples and limits are in [the release record](performance/README.md). Finite clock format bounds do not establish safe effective rates.

## Behavioral and corruption witnesses

The full workspace passes 1,912 tests across 35 targets, with 11 ignored and zero failures. The final focused command passes 38 tests, with three intentionally ignored fixture writers; the public targets pass nine tests. Raw output and exact command provenance are in [verification.json](verification.json) and [log_archives.json](log_archives.json). The focused log includes concrete target layouts and the empty component cost.

The private tests retain component admission while comparing continued covered Round authority and independently prepared World controls. They exercise:

- Active water service with household priority inserted before the serving actor; paused/resumed production and exactly-once completion; separate household Watch and successful-settlement markers.
- Manual jobs naming installed specs with arbitrary IDs/slots, and ordinary abandonment of jobs whose saved planner has vanished.
- Road departure notifications, presence epochs and the next stable trip number.
- Negative and positive pollen cadence, mixed signed ordering, and stale re-enrollment entries.
- Market travel and conversation holds, a seller temporarily absent from the same counter session, restored spending limits and the completed visit record.
- Resident interruption after publication, cursor fairness and support recovery; occupied/destination aliases; the legitimate gap between clearing a current spot and releasing its origin claim; actual weather transit followed by arrival and occupied shelter ownership.

Corruption cases cover duplicate owner keys/queue entries, immutable recipe/site tampering, invalid actual stock quantities, missing authoritative nullable fields, impossible resident patch/preference/target/projection bindings, orphan Character vendor/resident projections, accounting exhaustion, and invalid cursor/index bindings. Malicious nesting and many-empty-container tests assert aggregate preflight rejection reasons before serde can reject the unknown fields.

The coordinator's independent public tests cover exact raw navigation compatibility, cache warmth, wrong-context refusal and reservation release, the unadopted-backbone seam, and a valid component padded with 1 MiB whitespace whose larger raw charge survives candidate conversion. The earlier Character public tests are also rerun.

Supported exact fixtures are `crates/cathedral-sim/tests/fixtures/checkpoint_v1/round_empty.json` and `round_active.json`. The active fixture includes actual installed geometry, a partial authored cast, generated residents, water service, market/planner state and paused transform authority. Byte equality is checked by the focused suite; the explicit ignored writer changed only these new Round fixtures. Historical M2a1/M2a2 fixtures and golden prompts are unchanged.

## Cost evidence and its limits

`alibi_round_cost --mode authored|populated --samples N --output PATH` uses the same six raw phase arrays as the backbone probe. Populated means exactly 2,000 actually generated citizens added to the 520-character authored World. Counts also report Round people, residents, sources, stalls, plans, parties and reservations. Cost includes encoded, expanded, definition-working and retained peak bytes; shared peak explicitly excludes Running.

[authored-static-diagnostic.json](authored-static-diagnostic.json) and [populated-initial-diagnostic.json](populated-initial-diagnostic.json) are single-sample development diagnostics, not final release measurements. The populated diagnostic predates the additional 4 MiB definition allowance; its raw timing and metadata remain unchanged. The authored diagnostic includes that allowance but predates final helper/test changes. Their commands and development provenance are recorded in [log_archives.json](log_archives.json). The [final release record](performance/README.md) and [coordinator review](coordinator/review.md) preserve the completed build, six component runs, 26 ordinary comparison runs and independent audits.

The initial populated encoded/expanded shape is 3,510,496 / 51,657,412 bytes. The final release runs confirm that the separate definition allowance gives 221,359,536 bytes per cohort and 442,719,072 for standalone Save+Load, excluding Running. Adding the accepted M2a2 diagnostic's 813,374,372 bytes already exceeds the unchanged 1 GiB ceiling. These APIs therefore do not prove complete-envelope coexistence. Future composition needs explicit phased/offloaded work and charge lifetimes; historical M2a1/M2a2 latency failures already preclude synchronous host-frame placement.

## Remaining gates and handoff

Complete World/Engine ownership, scheduler/speech/backend obligations, capture/adoption, full-envelope cross-validation, Running/retiring coexistence and CPU-safe clock/accounting horizons remain pending. No M2b or host save/load path is introduced. The proposed next coherent M2a cut is private clock/weather plus sampled World climate/context, followed by knowledge/conversation/pollen and law owners before scheduler composition. It starts only after this cut is reviewed and committed.

[source_hashes.json](source_hashes.json) pins 77 source, fixture and context inputs after focused/public verification and at the workspace freeze. Coordinator-owned source/format/performance audits remain under `coordinator/`; historical probes, reference binaries and evidence are preserved.
