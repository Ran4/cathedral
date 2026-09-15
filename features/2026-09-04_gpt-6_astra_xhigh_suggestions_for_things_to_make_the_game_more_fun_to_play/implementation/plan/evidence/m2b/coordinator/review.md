Status: M2b dedicated quarantined hydration accepted (2026-09-15). M2c pending-work preparation, M2d continuation and M3 application adoption remain.

# Dedicated hydration independent review

Predecessor M2a16 is accepted and committed as
`7b50ff0eb69de204f7c3f273a9191449cc69098f`. Its full workspace passed 2,178 tests;
release capture/load evidence and strict fixture compatibility are preserved
under m2a16. Its source remains historical evidence, not a new M2b test result.

The sequential fresh-context owner was m2b_hydration. It completed implementation
and final verification, then explicitly ceded Cargo and source ownership. Root
owns independent tests, final review, release measurements, status and commits.
The user has authorized continued implementation.

The agreed direction moves a single privately validated typed graph into actual
World/Engine literals plus explicitly retained speech, host and cognition-input
continuation owners. A protected wrapper offers immutable observations and
canonical serialization from those actual new owners; it cannot expose a mutable
Engine or poll before M2c preparation. Inert service sentinels make no external
calls; the session transcript starts empty under its declared presentation policy.
Raw-envelope pass-through alone cannot establish hydration correctness.

A fresh-process resolver must accept actual installed parsed assets/configuration
in their independent roles without creating a throwaway Engine. The capture
Engine must be droppable before hydration. Rebuilt immutable indexes/adjacency
need explicit deterministic derivations and pre-allocation admission. Changed
installed input cannot inherit authorization from an older candidate proof.

The raw candidate, typed graph, new indexes/asset storage and surviving pending
owners must retain their charges until actual disposal, including failures and
cancellation. The same four-cohort 1 GiB limit and 128 MiB cumulative expansion
ceiling apply. The previous populated fixture has only 866,545 bytes expanded
headroom; a duplicate typed graph cannot be hidden by resetting expansion after
validation. New larger retained authority needs actual allowance, not a claim
that the predecessor's scoped 512 MiB Running minimum measures any future heap.
Assets must be reserved before a factory, clone or parse allocates, with shared
Arc identity distinguished from independent copies and failure before factory
invocation when capacity is unavailable.

M2c owns external retry, held-result and joint speech/floor adoption; M2d owns the
full continued-input suite; M3 owns file/UI/service/ECS publication, real retiring
and renderer/runtime lifetimes, and frame-budget acceptance. M2b needs a real
protected hydration result that preserves those obligations without silently
exposing a partially usable Engine. Concrete public signatures and tests follow
the asset-ownership reconciliation.

## Source review during implementation

The construction path consumes the private `ValidatedOwners` graph in exhaustive
World and Engine struct literals. Domain containers move directly; World events
start empty only under the existing complete flush-boundary invariant. The new
speech semantic-root index and Arc control blocks join a separate 256 KiB charge
inside cumulative expansion. Runtime service objects are inert zero-sized values.
The original accepted cognition inputs and speech interruptions remain explicit
typed owners beside the quarantined Engine, and no public World borrow exposes
navigation's interior-mutable cache.

Two concrete review findings were repaired before acceptance tests: resolver
metadata initially preceded its validation-scratch reservation, and category
serialization initially accepted an unrelated budget. Scratch admission now
precedes resolver construction; category observation checks shared budget
identity against the attached asset lease before staging. The public refusal
tests cover the second boundary and aggregate pressure directly.

`Admitted::try_map_mut` retains its primary reservation while closure-local
owners unwind. Preparation stores raw authority before its asset lease; the
temporary factory owner stores assets before preparation; the final hydrated
value stores the Engine and every continuation owner before its asset lease.
Success discards raw bytes before shrinking the primary charge. Source review
is paired with six public tests, including actual Weak-nav lifetime observations
after original App destruction and intentional panics at all six observer stages.

Independent tests live in `src/host_checkpoint/tests_hydration_public.rs`.
They compare all sixteen original category SHA-256 values against serialization
from actual new owners, preserve world lineage and saved host generation while
using a fresh Engine fence, reject changed config/navigation/host definitions,
reject zero and reused generations, prove asset-factory noninvocation under
pressure, cover preparation cancellation and factory failure, and reject category
observation through a foreign budget. The only shared implementation-owner test
code is the admitted installed-asset factory; assertions are independent.

Focused tests verify the scoped factory allocation plus retained shared-definition
bound, nonempty pending work, owner equality and failure/cancellation behavior.
The final workspace result below binds those tests to unchanged source. Release
measurements and persisted-fixture execution remain acceptance gates.

Root also traced the navigation transitive calls used by checkpoint validation.
`round/checkpoint/definitions.rs` builds a temporary `PlaceResolver` using named
sites, places and node positions; its `supply_pitch` and `lamp_ring` helpers use
walkability bitsets. Resident projection `status` and `destination` read saved
patch/spot identifiers. `NavData::checkpoint_fingerprint` returns the stored
definition digest and `resident_places` borrows existing data. These paths do not
invoke the distance-cache warmer. Final before/after inventory measurements will
check the actual fixture as well. Shared caller-driven cache growth remains the
caller's separately admitted responsibility.

The existing Engine service traits lack Send bounds, so an opaque hydrated
Engine cannot simply be moved from a decoding worker to the host thread. M3 must
separate worker-safe decoded ownership from bounded host-thread construction and
service rebinding, or supply another measured coordination design. This cut does
not add an unsafe Send assertion or establish synchronous frame-budget acceptance.

The initial host development run `tests-03` passed ten tests, including all six
independent public boundaries, authored/populated category equality, a held
scheduler completion with interrupted recording, and submitted Night work.
Its exact result records source unchanged during that command; subsequent fixes
make it development evidence rather than the final source result. Populated
decoded expansion was 133,613,327 bytes, leaving 604,401 below 128 MiB.

Further review identified two gaps in the evidence and closed their designs.
First, all-category hashes alone do not observe the separately reconstructed
World speech-root index. Narrow count/membership observations now permit an
ordinary accepted-recording fixture to verify that nonempty index independently
of the retained speech DTO. The corrected scenario passes as recorded below.

Second, a fresh PromptEnv shares MiniJinja builtin maps and can reuse compiler
TLS buffers. Counting factory allocations plus navigation alone omitted those
owners. The independent `prompt-shared-bound.md`/JSON/auditor traces the actual
2.21.0 source and derives 712,704 bytes under a fixed 1 MiB allowance. That amount
joins counted factory allocations and shared navigation within the same 64 MiB
asset lease. Persistent static/TLS owners remain covered by coordinated Running
after candidate disposal; their physical reclamation is not claimed. This
correction requires refrozen checks and corrected release measurements.

The first corrected run, `tests-07`, preserved a real fixture-precondition
failure: its directly queued recording used the previously accepted simulation
spatial sequence, which could already trail ordinary queued host samples. It
failed before hydration; the other ten tests passed. The fixture now uses the
current physical position and the ordinary host `position_for_action` producer.
`tests-08` passes all eleven hydration tests, including one accepted recording
and exactly one matching reconstructed World speech action. No production
hydration fix was needed for this fixture correction. Its exact source map is
`be5312aa3547f660396f61d39014354446f765211ca0ea2c10314bc5b54eb57b`
(967 inputs), shared with final format-09 and workspace-10.

## Final source verification

The unchanged-source workspace-10 passes **2,191 tests, zero failures, 43
intentional ignores across 46 groups**. Final formatting covers all 24 changed
or new Rust files; the diff check passes. Root's independent
`owner-command-audit.json` reconciles all twelve owner commands with their exact
original logs, archived bytes, helper identities and command-start source maps.
The failed fixture precondition and superseded development runs remain intact.
Final workspace raw SHA-256 is
`144efcc146e8e20684033cc0b469e7bea7b1bfbee365e451f2b116de36f2d565`.

The final debug image is preserved at `/tmp/alibi-m2b-debug-reference-binary-1`,
2,291,439,072 bytes, SHA-256
`7ca7c8e3aed207a323a575358f409076e7cb836ffef8afcf89f223d115f15f1d`.
`debug-reference.json` binds it to the same source map.

## Release and fixture acceptance

The preserved release image is 150,439,688 bytes, SHA-256
`60831ac78fc0e6b18314f9ec8d85332d10dbaa28bd4826561b7cb80fda41b281`.
The independent build audit verifies its original compiler output and both
preserved executable identities. Both release smokes pass, followed by three
100-sample processes per city size. All 600 hydrations match all sixteen owner
hashes; all 2,000 additional residents are placed in the populated case.

[Measurements](../performance/README.md) retain 1,200 end-to-end and 3,600 stage
values. Hydration p99 is 39.981 / 119.324 ms and new-owner disposal p99 is
1.418 / 4.346 ms. Maximum shared admission including Running is
726,174,363 / 843,676,620 bytes. Full scoped asset bounds are at most
13,022,860 / 22,998,242 bytes within the existing 64 MiB lease. Typed expansion
remains 38,308,507 / 133,613,327 bytes under the unchanged 128 MiB limit.
These timings include the test allocator hook and do not establish M3 frame
placement, original-App retirement or last-shared-navigation-owner disposal.

All ten fresh fixture commands pass. Three writers agree on exact bytes for
each initial/active state. Same-image readers dispose their source App before
fresh resolver construction and verify all restored owners. Different-image
readers refuse with the exact image-mismatch error before construction. Root's
independent fixture audit hashes original UTF-8 category slices without JSON
reserialization, checking 128 category comparisons across eight compatible runs.

Verified outputs were then copied unchanged to
`src/host_checkpoint/fixtures/hydration-v1/`. The post-publication map has 970
inputs, SHA-256 `4519679b5866452b4fcd6c2096ad9522479d9f4a81a91690843916e26745d3f9`.
`fixture-publication.json` proves that only two already-tested JSON outputs and
their README were added; every prior source input remains byte-identical. Final
build/test records retain their original 967-input map. No historical fixture
or saved manifest was rewritten, and no Rust changed after final verification.

M2b is accepted as the dedicated protected reconstruction path. Exactly-once
external retries, held application and speech interruption belong to M2c;
continued-control equality belongs to M2d. Complete host/ECS/service publication,
worker-safe ownership, file durability and measured active/retiring lifetimes
remain M3 obligations. M2 and the full investigation are not complete.
