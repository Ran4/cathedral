Status: Owner focused verification and normal production build passed, 2026-09-22. Partial M3 prerequisite only; complete allocation/adoption remains unaccepted.

# Immutable installed actor sources

When smart actors are enabled, startup captures the actual seed, occupations,
character files, areas, sound catalog and prompt sources once. LocalEngine
consumes that immutable snapshot. Disabled actors can still start without those
files. The reservation covers Rust-owned discovery/source storage and copies,
not native directory buffers, parsers/template compilation, generated crowds,
backend configuration or mutable world/host state. Complete admission still
refuses.

[DESIGN.md](DESIGN.md) records limits, construction/retention inventory and
remaining gates. [HANDOFF.md](HANDOFF.md) records integration and review scope.

## Executed focused checks

| Final applicable evidence | Passed | Source map |
| --- | ---: | --- |
| [Backend capture](owner/focused-01/backend_capture-result.json) | 9 | `ac6189e1263adf1239da21d509564bec00edbb1a92ac7792b7e2764645d46149` |
| [Existing sim lore](owner/focused-01/sim_lore-result.json) | 5 | Same first map |
| [Installed startup](owner/focused-02/installed_startup-result.json) | 6 | `bd5654bf035ff7bd52ad74f2228872959206ce9498daf9d1da9e8774bc1c0c3b` |
| [LocalEngine startup](owner/focused-02/local_engine_startup-result.json) | 2 | Same final map |

All commands exited zero and retained unchanged input maps. These are 22 distinct
focused tests, including 12 new witnesses. First-iteration host checks also
passed (five plus two); their records remain intact, and are not counted again.
No full workspace run is claimed for this patch.

Review found one compatibility regression before acceptance: disabled actors
must not require actor files, because their plugin skips engine creation. The
second iteration makes the snapshot optional, explicitly refuses an installed
engine build without it, and tests nonexistent roots plus unchanged backend
resolution. The [source delta](owner/source-delta-01-02.json) names the three
changed host files and proves all 393 backend/sim paths and every other component
input stayed identical. Their earlier pure capture/lore results are retained.
The simulator build script hashes host production source too: generated complete
build identities and binaries differ between iterations despite those unchanged
implementations. No binary-identity equivalence is asserted.

The maximal witness captures 4,089 character files and exactly 33,554,432 payload
bytes, with 1,019-byte relative paths. Its final run requested **57,386,946 Rust
allocation bytes**, within **59,779,814 admitted bytes**, and retained
**38,146,851 bytes**. The first run requested 57,395,145 bytes under the same
reservation; both raw outputs are preserved. The counter measures cumulative
requested Rust allocations, including temporary allocations. It does not observe
libc ReadDir storage or prove any parser/world allowance.

Other witnesses cover pressure before source IO, exact per-file/aggregate caps
and sentinel refusal, partial-failure cleanup, sorted discovery, symlinks and
empty casts, file/directory/depth/path/ignored-entry limits, lossy invalid-byte
names, last-owner retention and composition equivalence. The production routing
witness captures a distinct player name, deletes its fixture tree and successfully
uses the actual installed engine builder afterward.

## Reproduction and evidence

[run_tests.py](owner/run_tests.py) invokes the four focused modules sequentially,
using the default Cargo home/compiler/target, nice 15, one build job,
`--locked --offline`, headless fake mode and one test thread. The second run
selects only the two affected game modules. Actual START/PID records, toolchain,
shared component_input_sources maps, raw logs, deterministic gzip archives,
exit codes and SHA-256 identities are stored beside each result. Source map
checks include the final post-run map; reused run names are refused.

[run_build.py](owner/run_build.py) records one non-test production build with the
same environment and final source map, including executable SHA-256 without
running the executable. The [production result](owner/production-01/result.json)
exited zero with the unchanged final input map. Its executable SHA-256 is
`cc09931fc918ee31130445c30f19511c41c9b11625b9fa77266d634fb7ad0cd5`.
The normal target reports ten unused/dead-code warnings; no automatic
warning cleanup or additional source changes were made.

[The passing evidence audit](owner/audit-result.json), reproducible with
[audit.py](owner/audit.py), verifies command timestamps, log/archive
hashes, map continuity, helper identities and the actual executable hash, and
compares the final map against the current tree without invoking Cargo.

The preceding [workspace record](../integration_2026_09_22/REVIEW.md) passed 2,360
tests before this slice. That result does not validate the later patch. Neither
record establishes whole-App load/adoption, real-renderer frame limits, provider
behavior or device acceptance.

The later [native-directory allowance](../m3_native_directory/README.md) adds scoped Linux/glibc DIR-object admission. This source-capture record retains its original Rust-only scope, measurements and source identities.
