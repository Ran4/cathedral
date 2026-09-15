Status: M3a backend implementation and frozen verification independently accepted (2026-09-15). M3b–M3d application acceptance remains pending.

# M3a durable backend slot storage evidence

The [coordinator review](coordinator/review.md) accepts this leg after source,
fault/lifetime, release-image and full-workspace audits. Owner seal status below
records the earlier handoff; it is preserved as produced.

This leg follows accepted M2d `b0cc0f27c11f596839a53bd69a64ac9269757667`.
It implements backend publication, read, explicit recovery and bounded service
ownership. M3b application adoption, M3c controls and M3d host-frame acceptance
remain later work. The [design](owner-design.md) defines the public boundary,
durable ordering, memory allowances, platform contract and cancellation semantics.

The final 993-input map is
`97936f28124b1c9876b8f1de1c91a7e5e271f00eef982f38ee6126c3a5c52f23`.
It adds six Rust files and changes six existing inputs relative to accepted M2d;
the other 981 inputs remain identical. Ten changed/new Rust files pass scoped
formatting. Full component-inputs-v2 enumeration includes Rust, Cargo and content
inputs; documentation and evidence are outside that enumerator's scope.

## Verification and provenance

Every `owner/NAME-start.json` records exact argv, command-start source identity,
toolchain/hidden-fake environment, removed inherited flags and helper identities.
Its corresponding result binds the original `/tmp/alibi-m3a-NAME.log` and exact
mtime-zero `owner/NAME.log.gz` by SHA-256. No failed or mixed-source run is erased.
The runner is [owner/run.py](owner/run.py); final evidence is sealed by
[owner/seal.py](owner/seal.py), then independently audited by the coordinator.

All Cargo runs are offline, `-j1`, with one owner and
`CARGO_HOME=/tmp/alibi-m1b-cargo`. No target/cache cleanup, visible app, live
provider, audio/device, GPU, renderer or 20,000-resident probe ran.

| Run | Role and result |
| --- | --- |
| `focused-dev-01` | Failed development: 1 passed, 7 failed, 1 ignored; fixture captured before normal Ready boundary; source changed while in flight |
| `fixture-dev-02` | Corrected fixture startup through ordinary initial poll; 1 passed; unchanged command-start map |
| `focused-dev-03` | 11 passed, 1 ignored; later tests added while in flight; historical mixed-source result |
| `format-dev-01`–`format-dev-04` | Successful scoped mutating rustfmt runs; original changed-source flags retained |
| `focused-final-01` | 15 passed, 2 ignored; blocked-disk disposal witness added while running, so historical mixed-source result |
| `environment-01` | Historical platform observation; original helper retained as `environment-v1.py` |
| `environment-final-01` | Platform evidence with expected helper SHA bound in argv and stdout, at pre-format map `3089a6…`; not final-map functional verification |
| `format-final-01`, `focused-final-02` | Previous frozen map `b4b7bc…`; ten files formatted and 15 passed, 2 ignored |
| `sim-focused-final-01` | Four independent pure tests pass at previous map `b4b7bc…`; all 318 cathedral-sim inputs are byte-identical to the final map; final workspace also runs them |
| `release-writer-final-01` | Historical passing probe built while the admission-reporting fix landed; mixed-source image/report/fixture retained separately, excluded from final timing |
| `retain-release-pre-fix-01` | Preserves that exact historical release image and fixture without rewriting creating-image identity |
| `format-final-02`, `focused-final-03` | Previous frozen map `9de70e…`; ten files formatted and 16 passed, 2 ignored |
| `format-dev-05` | Final retained-reader test formatting; succeeded without changing bytes |
| `format-final-03` | All ten scoped Rust files pass on final `97936f…` map |
| `focused-final-04` | 16 passed, 0 failed, 2 intentional ignored helpers on final map |
| `release-writer-final-02` | Final release image; 32 actual saves, full M2 read validation; ignored scoped probe passes |
| `retain-release-final-01` | Preserves the exact final image, both references and two immutable payload files |
| `release-reader-final-01` | Direct preserved-image fresh process; same slot/image/payload passes full M2 validation |
| `build-progress-01` | Bounded read-only process/memory/disk observation while workspace ran; separate diagnostic, not functional verification |
| `workspace-final-01` | Final map: 2,249 passed, 0 failed, 46 intentional ignores across 46 groups |

[verification.json](verification.json) seals all 25 command records and the
current [source_hashes.json](source_hashes.json). Six final-map commands cover
formatting, focused storage tests, release writer, image retention, direct fresh
reader and full workspace. The older pure sim focused test has identical final
crate inputs; platform/resource observations remain separate diagnostic evidence.

The full command was `cargo test --workspace --offline -j1 -- --test-threads=1`,
with exact absolute argv/environment in its
[start record](owner/workspace-final-01-start.json). It passed after 1,570.946 s;
source remained unchanged. Backend library: 174 passed, 2 ignored. Simulation
library: 842 passed, 28 ignored. Application: 602 passed, 11 ignored.
The original `/tmp/alibi-m3a-workspace-final-01.log` SHA-256 is
`78418ee6ed38de0a3f2fed25c1c42f9b9d4dedc8d33b55e3df3f639b17b0998e`;
the exact [mtime-zero gzip](owner/workspace-final-01.log.gz) SHA-256 is
`1834b401c5a116603d0e48c5df673f3325c3eee32580917c8d4c90b594f29edd`.

The single failed development suite came from setup, not durable publication:
`Fixture::new` now obtains its ordinary initial Ready boundary by polling the
new engine once during startup. `Fixture::capture` remains read-only. No private
Ready override, capture-time poll or post-capture owner repair is used.

The late load-pressure correction maps pure inspection admission refusal to
`Phase::Admission`/`WouldBlock`, preserving valid-file retry. It is covered by
the final focused suite; the earlier release image remains explicitly historical.
The last source change retains loaded A while C/D publish and collect A's file,
then M2-validates its original admitted bytes before releasing LoadCandidate.

## Fault and lifetime evidence

The focused raw log exposes 42 unique returned-fault cases, before and after
each of 21 publication phases. It also exposes 21 publication process deaths,
16 recovery process deaths and 37 fresh same-image readers that perform full M2
validation of the preserved predecessor. Each subprocess is one structured JSON
record with exit status and exact checked-UTF-8 stdout/stderr. Escaped child
`test result:` strings are excluded from parent test-group totals.

The phase matrix includes journal write/flush/replace/directory sync, payload
write/reread validation/flush/immutable publication/directory sync, previous and
active reference write/flush/replace/directory sync, cleanup/directory sync and
pending removal/final directory sync. Additional tests cover injected ENOSPC
after an actual partial 37-byte payload write, interrupted first save, third
save with two acknowledged generations,
interrupted recovery, repeated cleanup failures across restart, damaged/missing
active recovery, orphan quarantine and incompatible framing/manifest boundaries.

Ownership tests cover eight intents plus unread terminal capacity, same-budget
and cohort identity across thread movement, capture ordering and exact returned
metadata, retryable pressure, explicit ID exhaustion, slow IO cancellation,
directory pinning/slot substitution, retained-reader cleanup, unread-load
shutdown and nonjoining destruction during blocked payload IO. Actual worker
disposal retains and finally releases the service, payload/input and lock owners.
Four independent root tests reside in each of sim and backend modules.

## Platform and measurement limits

The recorded system is Linux x86_64, kernel `6.8.0-138-generic`, rustc 1.96.0
`ac68faa20`, LLVM 22.1.2. `/tmp` is ext4 with
`rw,nosuid,nodev,relatime,errors=remount-ro`. The API currently refuses other
filesystem types. The caller must select a trusted private root whose directory
and ancestor entries are already durable. M3b startup owns that creation and
parent sync work; the backend pins and syncs entries inside the selected root.

Fresh-process death verifies interruption handling, not physical power loss or
storage-controller flush behavior. A reopened active reference proves validated
publication, not that a former UI observed acknowledgement. Network filesystems
and hostile external mutation are outside the supported contract.

The release probe uses 32 saves of the actual small complete demo fixture,
approximately 25 KiB. It measures storage API calls separately from durable disk
latency, excluding synchronous capture and full M2 validation from those timings.
This is not the authored/full-populated fixture, host-frame acceptance, or a
claim that current capture/hydration/retirement fit their eventual frame budget.

## Retained final release fixture

[owner/release-retention.json](owner/release-retention.json) binds the original
slot at `/tmp/alibi-m3a-release-slot-final-02`, all four exact copied files in
[owner/fixture-release](owner/fixture-release), the original test image and its
preserved copy `/tmp/alibi-m3a-final-release-image`. The image SHA-256 is
`52f7f4f408a5286194b509d477bf324d9ec44571c8bfdd2bf9dbeaa193bf2246`
(26,730,496 bytes). The complete payload is 25,298 bytes, SHA-256
`70eb84dc01962adc1db56d659a5c18291921be1f01b92a48d1ccb75a532eb083`.
No creating-image manifest or fixture byte was rewritten. The fresh reader
invokes that preserved image directly; rebuilding creates a different identity.

The exact phase arrays and ownership counters remain in the
[writer report](owner/release-writer-final-report.json) and
[fresh reader report](owner/release-reader-final-report.json). The caller-facing
times exclude capture and M2 validation; attach includes constructing that call's
bounded capture metadata. Disk latency ends at the actual final directory sync,
before the host's polling delay. The figures below use nearest-rank percentiles;
with 32 samples p99 equals the observed maximum.

| New boundary, microseconds | p50 | p95 | p99 / max |
| --- | ---: | ---: | ---: |
| Save intent submit | 1.088 | 2.124 | 4.525 |
| Capture attach | 2.678 | 3.706 | 3.938 |
| Terminal take | 3.259 | 7.431 | 8.553 |
| Attach to durable publication | 10,127.171 | 13,425.632 | 13,616.802 |

Off-frame startup was 39.257 microseconds; shutdown signal was 1.676 microseconds.
The explicit service allowance is 3,145,728 bytes, including 2,097,152 bytes of
worker stack. Shared admitted peak was 626,302,647 bytes (597.289 MiB), including
the existing 536,870,912-byte scoped Running allowance and actual capture/M2
scratch. After shutdown only that original Running allowance remained. These
admission counters are not process RSS or total Engine/ECS/OS heap introspection.
