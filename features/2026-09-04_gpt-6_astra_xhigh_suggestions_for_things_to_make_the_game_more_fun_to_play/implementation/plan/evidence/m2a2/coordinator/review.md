# M2a2 coordinator review — 2026-09-08

Status: Accepted component cut. Complete M2a/M2b–M3 and host acceptance remain pending.

Reviewed against base `72ed9be9115ffd2c899fe3da38a58252ec856725`. The cut preserves private character and inventory authority, exact registry bindings and the covered World references, with allocation admitted before extraction or decoding. It fixes the production global transform-ID collision. It does not expose a complete World/Engine save or a production World replacement API.

## Behavioral and source review

The review checked every current CharacterSheet/CharacterState field against the explicit remote records, inventory ownership and summed reservations against production rules, and PlaceRegistry indexes against insertion/home behavior. Review corrections preserve the 64-entry unread window, repeated prose occurrences, historical seed holdings and completed item lineage, the city's 128-Unicode-scalar IDs, mandatory nullable fields and inclusive receipt step bounds. Normal duplicate place names keep their earliest normal entry; homes preserve saved IDs and remain excluded from name lookup. Existing next-tick cleanup semantics permit overdue travel and stale or zero-direction Needle claims.

Root added five public Character API tests independently of the implementation agent. Private tests additionally continue transform completion/replay through later item removal, real navigation movement/gait and ordinary Engine digestion producing the same single result. Three exact fixtures preserve empty, active/private and completed historical component states. The [owner ledger](../OWNER_COVERAGE.md) keeps complete Round projection/planner, geometry, temporal, semantic-root and other-owner validation explicit.

The allocation review covers the concrete closed record shapes, geometric Vec capacity, nullable inline state, BTree spare slots, registry indexes, internally tagged serde buffering, escaped-string scratch and errors. The measured minimal Character record charges 19,396 bytes, exceeding its 18,872-byte sparse BTree-node bound. Malicious-shape tests assert preflight-specific failures, so ordinary unknown-field rejection cannot conceal missing allocation admission. These are conservative bounds on supported shapes, not a generic guarantee for arbitrary deserializers or measured allocator usage.

## Verification and provenance

- **Workspace: 1,896 passed, zero failed, 10 ignored across 34 targets**, exit zero, 510.503 seconds. Focused checkpoint group: 26 passed and two intentional fixture-generator ignores. The five public Character tests pass and are included in the workspace total.
- [Log audit](log_audit.json) independently recounts test summaries, verifies both final logs and all 12 development archives, and matches original bytes. Only redundant terminal blank lines were removed from the two plain final logs; gzip development logs preserve exact bytes.
- [Source audit](source_audit.json) verifies all 45 manifest entries and coverage of all 21 changed implementation/fixture paths. Manifest SHA-256: `c79340bf6c16bbe12f0142376b8355d82fa7a8b068c3e81d0e078b909fe3d4f2`.
- [Formatting audit](format_audit.json): 17 changed/new Rust files pass scoped rustfmt. `item.rs` has pre-existing formatting drift; its sole change is the checkpoint module declaration. Formatting HEAD and current source independently yields identical results after removing that declaration. No unrelated formatting was applied.
- [Release build](release_build.json) passes in 51.785 seconds. The measurement runner's help and smoke checks pass, followed by all six complete runs.
- [Performance audit](performance_audit.json) independently recomputes every percentile/median from **3,600 raw phase samples**, verifies archive/original hashes and repeated metadata, and matches all **789 source/content files**, the release binary and runner dependencies. Source and executable remained unchanged throughout measurement.
- [Plan validation](plan_validation.json) passes all 814 local links/anchors, generated catalog checks and recorded source/model hashes. Both working-tree and staged whitespace checks pass; the staged paths are confined to this cut.

Original workspace log SHA-256: `ded0d3576293662274c311113b7191c36136059d61c7d376db9d2824217ed789`; normalized archive: `ca3a33bb43d974e1ce67301e4a1fa5a4d8dc72ed1b1b3d0d9774a3af8a18b81d`.

The original baseline harness and preserved M0 binary still match `80973b4fa79f4cbb1a4f0673d6bc4f52baffe12ff5f6f75bf470222f6fda33da` and `f7ba972a440a2d6d2690c4dd2afd230363e5778ebf2c3fe7d62d4becca3e9057`. Historical M0/M1/M2a1 evidence remains unchanged. The earlier renderer/full-stress limitations are retained without another availability probe.

## Measured limits and next owner

[The release record](../performance/README.md) reports 1,919,795/9,118,806 encoded bytes for 520/2,520 actors. Simultaneous conservative save/load charges are 166,326,706/813,374,372 bytes, **excluding Running**. Populated export/decode median per-run p99 is 30.372/37.434 ms, and every populated phase exceeds the 2 ms coordinator p99 target, including validation and disposal. Existing numerical limits are unchanged.

M2b/M3 must provide complete cohort admission and suitable background/incremental execution, including destruction and later copy-on-write costs. Full-envelope plus Running/retiring coexistence, effective calendar-rate CPU bounds, complete Engine continuation, host frames and renderer acceptance remain unproved.

After this cut is committed, M2a3 may implement Round/residents/production/household/road-party authority against these actual actor/item/place interfaces. It must preserve saved projection cadence and planner/reservation identity, and must not seed a replacement city. Other owners and the complete envelope still follow before M2a acceptance.
