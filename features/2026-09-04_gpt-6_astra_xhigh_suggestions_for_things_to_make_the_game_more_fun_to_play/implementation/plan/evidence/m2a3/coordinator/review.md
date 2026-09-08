# M2a3 coordinator review — 2026-09-08

Status: Accepted component cut. Complete M2a/M2b–M3 and host acceptance remain pending.

Base: `3e9c93f` (M2a2). This cut preserves Round, resident reservations and the associated production, household and road-party authority. Its opaque candidate resolves against either the running actor/item/place backbone or an unadopted backbone candidate. It cannot replace a World or Engine, and validation does not invoke their seed paths.

## Source review

The explicit records cover the current Round and resident fields, including queue service deadlines, production eligibility/watchdog samples, household sampled/completed settlement markers, road trip state and pending notifications, resident fairness/support/weather state and the last published resident projection inputs. Navigation compatibility hashes the original JSON and bitset at the existing loader; cache warmth does not change that binding. Shelter rows, the item catalog and embedded planner definitions have separate exact bindings.

Resident projection validation uses the ordinary writer's pure projection logic. Speech interruption can change a controller after publication, so the stored sampling inputs remain authoritative for the displayed values. The next ordinary resident pass performs the next publication. Occupied and destination claims may legitimately name the same actor's spot, and an origin claim may remain after the smaller arrival tolerance has cleared the controller's spot.

The review corrected an invented unique-buyer restriction on stock plans, checked recipe quantities at their actual quantities, required orphan vendor/resident projection consistency and preserved the exact authored preferred vendor even when that actor is absent. Manual inventory transformations have no origin tag: arbitrary valid job identifiers or recipes cannot be rejected solely because their spec name matches a saved planner. Installed planner recipes, work and sites are checked exactly; absent plans remain absent and stranded jobs retain their ordinary watchdog obligations.

Pollen deadlines now use signed numeric ordering and an explicit initially-due sentinel. The former nonpositive-key clamp could repeatedly pop and reinsert the same key on a negative calendar. Tests cover the corrected initial/rearm behavior, stale entries and positive calendar cadence. Cumulative accounting rejects values without room for one maximum-population pass; complete calendar/rate/horizon validation remains a later composition gate.

## Admission review

The fixed definition scratch allowance is 4,194,304 bytes. The installed definitions meter to 1,730,786 expanded bytes and 179,470 encoded bytes; two expanded copies, three encoded copies and 128 KiB for resolver/comparison work total 4,131,054 bytes. Site count/text and resident/shelter geometry caps bound external context inputs. Geometry-derived validation streams expected rows instead of allocating an index from uncharged external cardinality.

Raw-input admission adds the definition allowance before typed parsing. It never replaces the original lexical charge with a smaller canonical serialization charge. The coordinator's public regression appends 1 MiB of whitespace to a valid record and checks that the additional input charge survives decoding and candidate ownership.

The component admission model remains a conservative bound for these closed record shapes, not measured allocator use. Complete envelope plus Running/retiring coexistence and host-frame placement remain unproved; no budget is raised by this cut.

## Verification

- **Workspace: 1,912 passed, zero failed, 11 ignored across 35 targets**, process exit zero. Focused checkpoint tests: 38 passed and three intentional fixture-writer ignores. Public tests: nine passed, including four independent Round boundary cases.
- [Log audit](log_audit.json) checks all 16 archives and independently recounts the final results. Workspace raw SHA-256 is `0aea16bb0bfd732836679909d7f71ac8d6ea3321c4924077b4da83b0581396ec`; normalized archive is `57bf8f924306a37864416de8ebafc5ac26c7b2f0a463a8de1ca5934e08eda15f`. Normalization removes only redundant terminal LF bytes. Focused/public originals were recovered from the agent's recorded reversible newline delta and match its pre-normalization hashes; workspace raw bytes were copied before normalization.
- [Source audit](source_audit.json) checks all 77 manifest entries and covers all 22 implementation/fixture paths. Manifest SHA-256 is `3ade416b2fd037ad3aa6cedecbbf2f8086446e8d34d308c20fbae32b23a6c60b`. All 20 changed/new Rust files pass [scoped formatting](format_audit.json), with unchanged hashes at the final freeze.
- [Release build](release_build.json) passes in 62.235 seconds. Component smoke checks and six full runs pass; [the audit](round_performance_audit.json) independently verifies all 3,600 phase samples, hashes and repeated metadata.
- Ordinary smoke checks and all 26 runs / 13 pairs pass. [The audit](ordinary_performance_audit.json) recomputes 30,800 poll samples, all paired statistics and medians, and checks 802 source/content files plus both executables. Default/+2,000 p95/p99 remain within the unchanged 10% comparison threshold. All cross-binary counter differences match [the historical M1d comparison](semantic_comparison.json).
- [Auditor regression checks](auditor_regressions.json) accept the preserved historical measurements and reject deliberately altered p99 values. Historical evidence is unchanged. [Plan validation](plan_validation.json) passes 822 roadmap links/anchors; the evidence audit passes another 32 links across five Markdown files. Working-tree and staged whitespace checks pass.

## Limits and handoff

[Release measurements](../performance/README.md) confirm 1,315,978/3,510,496 encoded bytes and 209,917,916/442,719,072 shared standalone Save+Load bytes, excluding Running. Populated export/decode median per-run p99 is 25.836/29.780 ms. All measured phases except disposal exceed the 2 ms coordinator p99 target. Combined populated M2a2/M2a3 standalone charges already exceed 1 GiB; full composition must stage ownership/allocation lifetimes within existing limits.

The full Engine envelope, remaining owners, complete reference/time/root validation, effective calendar-rate CPU bounds and Running/retiring coexistence remain required. Renderer and full-population stress are still unavailable; the stress comparison honestly records the 9,072 actually placed residents. No repeated availability probe was made.

After this cut is committed, the next coherent owner cut is private clock/weather and sampled World climate/context authority. No M2a4 work began before acceptance, and no production World/Engine adoption API was added here.
