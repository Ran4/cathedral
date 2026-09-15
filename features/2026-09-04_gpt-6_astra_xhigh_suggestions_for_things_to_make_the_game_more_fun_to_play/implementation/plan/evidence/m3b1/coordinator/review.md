Status: M3b1 independently accepted after corrected frozen workspace and source/command/phase/image audits (2026-09-15).

# M3b1 coordinator review

Accepted predecessor: M3a `5559d26654faba677971ceeef66f0869495e6489`.
One fresh sequential owner implements this leg and exclusively runs Cargo. The
coordinator independently reviews source, supplies eight boundary tests, audits
the command evidence and owns acceptance/commit. M3b1 supplies actual worker
preparation and disposal transport. Whole-App replacement remains M3b2.

## Reviewed ownership contracts

Full installed-definition and all-sixteen-owner validation remains in the
existing complete decoder. The worker now constructs an opaque Send
`DecodedHydration`; the host constructs the non-Send Engine by exhaustive moves.
There is no creation seeding, extra poll, unsafe Send override or exposed private
World. Actual M3a loaded bytes retain their LoadCandidate identity throughout.
The recipe builder allocates owned captures only after recipe admission. The
worker admits asset/definition scratch before factory invocation and disposes
raw/resolver storage before delivery.

The service has one preparation slot and one independent retirement slot on
one worker. A delivered candidate keeps its original preparation return slot
through construction, continuation, binding, cancellation and service close or
abandonment. Failed retained continuation/binding is disposal-only because it
may already have transformed some owners. Returning it never requires an empty
RetiringGeneration slot. Actual Send service owners are detached before the
host drops their empty forwarding adapters. The original candidate and service
leases accompany worker disposal.

The old LocalEngine transport performs all fallible startup/generation/pin
checks before fencing. Refusal returns the entire original Engine/guard/handle/
inbox/permit bundle. Successful preflight pins original command, publication
and mailbox lifetimes, fences producers, detaches the Send services and moves
the exhaustive inert domain into the already reserved worker slot. Queued
callbacks, commands and publications are explicitly drained on that worker.
PromptLog flush and session/service cleanup run there too.

Disconnecting a crossbeam channel does not prove its queued allocations have
been destroyed while endpoint clones survive. The new acyclic shared pins
therefore retain the retirement charge through surviving senders, receivers,
jobs and externally held publication allocations. `owners_disposed` and final
`released` are distinct observations. Neither proves that every global log or
detached runtime allocation has been accounted for; M3b2 owns that closure.

The explicit 2 MiB worker stack and separate fixed control allowance are a
persistent 4 MiB Running child. World/assets/recipe/services/retirement payloads
have separate charges. This is a declared admission contract, not an arbitrary
whole-process heap measurement or permission to hold two 512 MiB worlds under
a 1 GiB cap. The caps and saved wire versions remain unchanged.

## Corrections required during review

- Preserve the existing consuming M2c API's panic propagation while the new
  retained path catches service-factory failure and returns the actual graph.
- Bound retained factory diagnostics by allocating a fresh UTF-8-safe prefix
  of at most 4,096 bytes; truncating length alone would retain excess capacity.
- Catch asset-factory, builder and cancelled-capture/service destructor panics
  while the corresponding actual owner leases still exist. The retirement
  worker holds its lease outside the destructor catch, including panic-payload
  destruction. These guards do not bound arbitrary provider execution time.
- Fence old callback production without the existing inline receiver drain;
  transfer actual receivers for worker draining and pin all surviving endpoints.
- Retain a candidate's return entitlement after cancellation/service shutdown;
  prohibit further preparation/binding through an obsolete delivery.
- Gate the preparation module and host retirement entry on Linux, matching the
  existing storage platform boundary.

Mechanical extraction and fixture type errors were corrected during development.
Failed and mixed-source commands remain in the evidence; only an unchanged
final source map can establish final acceptance.

## Independent tests

The coordinator owns three pure tests in
`crates/cathedral-sim/src/checkpoint/retirement_review_tests.rs`:

- Foreign same-sized budgets and non-retirement reservations cannot identify
  the current retired cohort.
- Actual cloned retirement owners keep one charge; weak release observers
  cannot release it early or confuse a later cohort with the old one.
- Recipe subleases require the original load cohort, respect shared saturation
  and overflow, and keep the cohort alive after the parent input is dropped.

Three tests in
`crates/cathedral-backends/src/mailbox_retirement_review_tests.rs` exercise real
PCM/WAV ownership through fencing and worker drain, delayed jobs and receiver
clones, refusal without mutating the original mailbox, and six stale terminal
families alongside live reused IDs in the fresh generation.

Two tests in
`crates/cathedral-backends/src/checkpoint_preparation_review_tests.rs` exercise:

- A real M3a loaded candidate at Decoded/Hydrated/Prepared/Bound stages, retained
  save payload and independently blocked retirement worker. Closing/dropping
  the service and delivery preserves exact charges until worker cleanup. The
  bound stage witnesses actual cognition owner destruction on the worker. Its
  two-second guard is a deadlock timeout, not a frame-performance budget.
- Shared-budget exhaustion before continuation, returning the actual admitted
  graph. Freeing the competing charge does not authorize retry of the failed
  candidate; its original preparation slot still disposes it correctly.

The blocked retirement payload in the first test proves independent-slot
contention and lifetime behavior. It is not an inventory of a retired World.
Separate owner tests transport the actual LocalEngine bundle, flush a real
prompt pair, drain actual queues, hold external lifetime owners and refuse a
wrong generation without consuming the live bundle. Their injected PromptLog
destructor delay is explicitly included in the reported worker sample.

## Validation and scope

The real-file preparation fixture hashes all sixteen source capture categories,
loads the actual durable slot in a fresh process launched from the same test
image, then compares every hydrated owner digest. It performs construction and
M2c preparation without polling or creation seeding. Definition mismatch and
reusing the old execution generation refuse that same file.

Eight raw authored fixture samples separate worker decoding/raw disposal,
host construction/continuation/return, and candidate worker disposal. They are
debug transport observations; they do not establish a populated-city p99 or
whole-App frame acceptance. Storage and candidate cleanup are measured outside
the host return operation.

The first frozen workspace command compiled successfully in 19m12s, then failed
one existing M3a recovery test: reopening a shut-down store produced
`Phase::Open / WouldBlock`. Its backend group passed 185 tests, failed one and
ignored three; the printed fresh-process child passed one separately. All
M3b1 backend tests passed in that run. This failed command is not acceptance.

Independent source review found that Store held its directory flock until File
close, with no explicit unlock. Linux documents that `fork`/`dup` descriptors
share an open file description and its flock; closing only the original
descriptor can leave that lock alive. Explicit `LOCK_UN` releases it even while
a duplicate exists. See [flock(2)](https://man7.org/linux/man-pages/man2/flock.2.html),
checked 2026-09-15. A concurrently starting child retaining the old descriptor
before exec is a mechanism consistent with the observed failure, not a captured
syscall trace of that particular run.

The deterministic duplicate-descriptor test fails against the unfixed Store
(`lock-regression-before`: zero passed, one failed, unchanged source). The
correction constructs the Store guard immediately after successful flock,
before fallible directory sync, and explicitly unlocks in its worker-owned Drop
while the descriptor remains live. Interrupted unlock calls are retried. A
second test injects sync failure after cloning the locked descriptor. Both
tests keep an actual duplicate alive and prove that a separate writer is
refused until the original store finishes.

`storage-lock-after` passes 18 storage tests, zero failures and two intentional
ignores concurrently, including both regressions and the previously failing
recovery test. Its source map is
`af88d1966adb11388c3b015819c3a9e7c500897352bf118e2c38336878f0d3b2`.
The accepted M3a serial run did not establish this concurrent-spawn property.
The second concurrent workspace command compiled in 12m28s and passed its
backend group (188 passed, zero failed, three ignored, plus one separately
printed fresh-process child). It then aborted in the host's pre-existing
`service_factory_refusal_mismatch_unwind_and_repeat_binding_release_all_owners`
test. Its actual InertService destructor detected that admitted storage had
already been released; another destructor panic during unwinding caused
SIGABRT. This second failed command is not acceptance.

Source review identified an M3b1 regression in PreparedContinuation: the new
`failed_services` field followed its asset and service reservations. Declaration
order therefore released those charges before destroying generation-mismatched
services. The correction moves the failed service owner before both leases.
The existing consuming API and the new retained API share that corrected
layout. An additional owner regression checks the retained graph's real four
services: no early drops, exact retained charge, adoption refusal, repeat
binding refusal without another factory invocation, and disposal before charge
release. This complements the existing consuming-API lifetime test.

`final-workspace-02` retains its complete raw log and deterministic gzip
archive. Its unchanged source map was `af88d1966adb11388c3b015819c3a9e7c500897352bf118e2c38336878f0d3b2`;
raw SHA-256 is `5c8d2e172240a227ed1c4b460aa9c676dae0c4e6621e99918a8832afa716ede8`
and archive SHA-256 is `952259e5e07a6ddf35db3524e28e8c80e7810c6d31467aa41b9c89a8a8885635`.
Elapsed command time was 894.8360894520301 seconds.

The focused public continuation module passes all eight tests, including both
service lifetime witnesses. Final formatting checks cover all 25 changed/new
Rust files and diff checks pass. These commands use unchanged source map
`ea4086cf10fdf5e6d0781f3191e2f73cc9dcd7460889a4d98646a1f993783d80`.
The subsequent concurrent workspace run passes, as recorded below. The invalid binary-only package
`--lib` invocation is preserved as command feedback, not a source or test failure.

## Final acceptance

`final-workspace-03` exits zero on the unchanged `ea4086...` source map:
2,272 outer tests passed, zero failed, 47 ignored, across 46 groups. The
separately printed fresh-process child passed one test. Compilation took
11m09s; the full command took 771.9773938511498 seconds. The host group passed
605 tests with eleven ignored, including both corrected service lifetime paths.

All 28 recorded commands pass the independent provenance/archive audit, with
their actual success/failure and unchanged/mixed-source statuses preserved.
The final raw log is 593,695 bytes, SHA-256
`8d3e8b9ca34a419953efb828e3f7e1441b1f8c6b3bf70f7fb3b461d2f72cc21e`;
the lossless deterministic gzip SHA-256 is
`f9efa0599ef866ca66d9cac9fa2ee2a6f790a4871b3932e2baa4f041fa8c674e`.
Current input enumeration equals the final 1,002-input map: nine new files,
sixteen changed predecessor files and 977 unchanged predecessor inputs.

The phase auditor verifies all fifteen selected ownership boundaries, including
the eight coordinator tests, both storage-lock regressions, the old recovery
test, both actual LocalEngine retirement tests and both service lifetime tests.
Eight authored debug samples have maxima of 21.566 microseconds host
construction, 13.690 microseconds continuation, 24.106 microseconds return,
19.840113 milliseconds worker preparation and 0.125144 milliseconds candidate
disposal. Actual old-host detachment is 15.835 microseconds; worker disposal is
4.063247 milliseconds including the injected PromptLog gate. These are small
fixture observations, not whole-App or populated-city frame/p99 acceptance.

Read-only GDB layout queries and independent image hashing agree on the final
381,031,000-byte backend test ELF, SHA-256
`c6a73440866614dbd11a5d5bede6b85f2d094627b6b71b55b5aa02ab9de7b311`.
The nine layouts support a conservative 59,016-byte fixed-root total under
64 KiB. The separate native thread/TLS/allocator allowance remains a stated
trusted assumption, not measured whole-process ownership. No final executable
copy or persistent M3b1 slot fixture is claimed. M3a's retained release image
matches its original size and hash.

M3b1 is accepted for its preparation/retirement transport scope. The next owner must consume the actual
accepted interfaces and close the complete host/controller/time/projection
adoption path, allocation transitions and remaining detached-owner accounting.
M3b2, M3c controls, M3d whole-path measurements and M4–M19 remain pending.
