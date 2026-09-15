Status: implemented and accepted after independent review and frozen verification (2026-09-15).

# M2d deterministic future evidence

The [owner design](owner-design.md) fixes the complete capture/hydration/poll
seam, actual lease ownership, explicit CPU Host, phase-specific comparison paths
and active-to-future fixture matrix. Nine changed/new Rust files contain
cfg(test) registration, test helper visibility,
the admitted harness, owner fixtures and independent coordinator cases.

The [coordinator review](coordinator/review.md) accepts this leg after auditing
all thirteen command records and the unchanged-source workspace result:
2,229 passed, zero failed, 44 intentional ignores across 46 groups.

The initial 983-input map in [owner/initial-sources.json](owner/initial-sources.json)
exactly matches accepted M2c's post-publication map. The frozen final map contains
987 inputs, adding four Rust test files, and has SHA-256
`28a535a0b232d887c0e2653a308ee0c92eb62c9b04240b7aaa587886c2815188`.
The full existing `component-inputs-v2` enumerator is unchanged.

[owner/run.py](owner/run.py) records exact command-start source maps, helper and
enumerator hashes, offline/headless/fake environment and removed inherited build
flags. Each command retains its original `/tmp/alibi-m2d-*.log`, result hashes
and an exact mtime-zero gzip archive. No command name is reused, and unsuccessful
or source-changing attempts retain their original status. The final
[owner/seal.py](owner/seal.py) checks every command and publishes the final source
map and verification report after full-workspace success. Coordinator provenance
review is independent.

Seven unsuccessful development commands remain evidence:

| Attempt | Finding and resolution |
| --- | --- |
| `harness-01` | Mechanical compile errors in ledger high-water access, decoded Host scalar borrowing and the road phase variant; corrected against actual source |
| `harness-02` | Missing ordinary Host physical ingress; common fixture now submits real SpatialUpdate sequence 1 with the exact f32 body before initial capture |
| `future-01` | A local position shadowed the host setup function; renamed |
| `future-02` | Tiny navigation lacked installed food/market/road/custody definitions; those fixtures now bind committed full navigation. Generic fixtures now have real tiny nav so movement time advances; Host fence/time setup follows actual saved authority |
| `future-03` | Exact Duration rounding and actual sampled publication rows were required by Host validation. The mill requires Waning; market setup was replaced with a real serving FIFO. The escort revealed the existing 6 m/4 m/1.5 m endpoint interaction documented in the design, so its reviewed claim is continued escort movement with both body displacements |
| `future-04` | Mechanical missing Thinking-status import and extra string reference; corrected |
| `future-05` | Resident dwell outlasted the observation, and seller history coalesces consecutive identical sales. Pre-capture expired dwell now produces an asserted active resident route; the market asserts the exact two-sale percept |

`future-06` passed all twelve owner scenarios. `focused-final-01` passed all
sixteen selected cases: twelve owner, two independent coordinator, and two
existing tests matched by the `future_` filter. It additionally pins original
preparation inputs/roots and compares the exact active Night request identities
with its explicitly asserted retry pacing. `format-final-01` passed all nine
Rust files. Both final commands use the same frozen map as `workspace-final-01`.
The full workspace passed 2,229 tests with zero failures and 44 intentional
ignores across 46 groups. The [verification report](verification.json) and
[owner handoff](OWNER_HANDOFF.md) record the sealed results and remaining limits.

The two mutating formatting runs and four development test commands with
`sources_unchanged=false` are not frozen acceptance. No failure was converted
into a passing measurement by dropping an owner. The final comparison remains
byte-exact for every owner after its narrow declared execution/pacing policy;
failure diagnostics are bounded to owner, lengths, first differing byte and
local context.

This leg adds no release cost probe or supported on-disk image fixture. Existing
M2a/M2b/M2c admission limits and historical creating-image fixtures are unchanged.
The synthetic Host, diagnostic JSON copies and repeated admitted observations
are behavioral evidence, not aggregate multi-host heap or frame-time evidence.
M3 retains actual storage/Bevy/offload/publication/adoption/retirement work;
M0's renderer and full-stress gaps remain explicit. No visible app, provider,
device, audio, GPU or 20,000-resident probe ran.
