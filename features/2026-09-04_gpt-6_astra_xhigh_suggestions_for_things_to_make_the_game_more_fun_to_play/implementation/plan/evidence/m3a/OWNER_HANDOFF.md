Status: implementation and frozen verification complete; Cargo/source ceded to coordinator for acceptance (2026-09-15).

# M3a owner handoff

M3a implements backend durable slot storage on shared `develop`, following
accepted M2d `b0cc0f27c11f596839a53bd69a64ac9269757667`. The owner made no
branch, worktree, commit or push. The coordinator owns independent review,
acceptance and the coherent-leg commit. Unrelated untracked work is untouched.
All owner Cargo, test/probe and diagnostic processes have exited. The owner
explicitly cedes Cargo and source ownership after successful workspace and seal;
no further owner source edit or verification command is planned.

## Implemented boundary

`cathedral_backends::checkpoint_storage` is a Linux-only service, separate from
world service generations. It accepts eight total retained operations/results,
serializes disk work, owns one same-budget admitted save payload, and returns
one admitted load input. Metadata attaches with the captured payload; a refusal
returns both without losing the capture's date or player-known location. IDs
combine a random service identity and checked sequence, independent of world
lineage or runtime generation. Preview types and deserialization enforce bounds
and safe slot components. No Engine extraction or unsafe Send override exists.

Durable pending journal publication precedes candidate payload creation.
Immutable payload publication, previous reference retention and active reference
replacement each receive the required file and directory flushes. Known-old
cleanup follows durable active publication while the original journal still
identifies cleanup debt; only successful cleanup, pending removal and final
directory sync permit Saved. Two acknowledged generations plus one candidate
bound known ancestry. Recovery retains the original acknowledgement across
interrupted recovery itself; missing/corrupt active files have an explicit repair
path through a verified previous reference. Unknown artifacts are quarantined.

Load checks reference slot identity/checksum, bounded exact payload length/hash,
closed framing, profile and metadata before returning the original admitted
buffer. Full installed-definition/component M2 validation remains mandatory.
Capacity pressure is an Admission/WouldBlock outcome, so valid files remain
retryable. Loaded bytes remain usable after their old immutable filename is
collected: the focused test holds actual A through C/D publication, then M2
validates A and releases its original LoadCandidate before loading D.

The new service charge is a disjoint 3 MiB child of existing Running admission:
2 MiB explicitly configured worker stack plus 1 MiB bounded control/IO scratch.
It is retained by the last control/worker owner, including detached shutdown.
Cancellation before work starts disposes on the worker and promises Cancelled;
active work truthfully completes or fails. Unread terminals remain in the
eight-slot bound. Shutdown preserves terminal access; explicit off-frame join
requires draining, while dropping the handle signals abandonment without joining.
The existing worker disposes owners. A blocked-write test proves prompt handle
drop while payload/service charges and the pinned directory lock remain alive.

Pure sim additions are narrow: `owns_admitted` observes exact budget/cohort
identity; `reserve_running_overhead` reuses shared subordinate accounting; and
`CompleteCheckpointInput::inspect_envelope` reuses the existing closed parser.
Inspection admits 4 MiB temporary scratch before parse and restores at least
the original reservation after parser allocations/diagnostics are gone, including
error paths and preexisting larger definition-resolution scratch.

## Frozen verification

Final component-inputs-v2 map: 993 inputs, SHA-256
`97936f28124b1c9876b8f1de1c91a7e5e271f00eef982f38ee6126c3a5c52f23`.
Six existing inputs change and six Rust files are added; the remaining 981 are
identical to accepted M2d. All ten scoped Rust files pass `format-final-03`.
`focused-final-04` passes 16 backend tests, with two intentional ignored helper
entry points. Four independent pure sim tests pass in `sim-focused-final-01`;
all 318 inputs of that crate are identical to final source, and final workspace
includes those tests again.

`workspace-final-01` passed 2,249 tests, zero failures and 46 intentional ignores
across 46 groups on that unchanged map. Backend library: 174/0/2. Simulation
library: 842/0/28. Application: 602/0/11. The exact command is
`cargo test --workspace --offline -j1 -- --test-threads=1`; its
[start](owner/workspace-final-01-start.json) records absolute argv, isolated
environment, removed inherited flags and helper/source identities. The
[result](owner/workspace-final-01-result.json) records 1,570.946 s elapsed and
`sources_unchanged=true`. Raw log SHA-256:
`78418ee6ed38de0a3f2fed25c1c42f9b9d4dedc8d33b55e3df3f639b17b0998e`.
Exact mtime-zero gzip SHA-256:
`1834b401c5a116603d0e48c5df673f3325c3eee32580917c8d4c90b594f29edd`.

[owner/seal.py](owner/seal.py) succeeds for all 25 recorded commands, the final
993-input map, raw/gzip identities, fault/death matrix and retained release
image/fixture/report identities. It publishes [verification.json](verification.json)
and [source_hashes.json](source_hashes.json). The bounded `build-progress-01`
observation during the long build is retained separately from functional checks.
Coordinator independent provenance/release audit and final acceptance remain
the coordinator's responsibility.

The backend focused raw evidence retains 42 distinct returned-fault cases,
21 publication deaths, 16 recovery deaths and 37 fresh same-image M2 readers.
It includes injected ENOSPC after an actual partial 37-byte payload write,
third-save failure,
interrupted first save, repeated recovery/cleanup failures, final marker/sync
ambiguity, corruption/admission boundaries, delayed metadata, bounded queue and
worker ownership. Four independently owned backend tests cover preview decoding,
damaged active repair, directory pinning/slot substitution and unread-load
shutdown; four independently owned sim tests cover admission/envelope behavior.

[README](README.md) explains every development failure and mixed-source result.
Each command retains exact original `/tmp` stdout/stderr, mtime-zero gzip,
start environment and source map. Subprocess records preserve checked UTF-8
losslessly in JSON and cannot inflate parent test counts. The failed initial
fixture was fixed through ordinary startup polling; capture remains read-only.
The earlier successful release probe is preserved separately as mixed-source
history, not final acceptance or timing.

The final release writer performed 32 actual saves and full M2 read validation;
a fresh process invoked `/tmp/alibi-m3a-final-release-image` directly and validated
the same complete save. Image SHA-256:
`52f7f4f408a5286194b509d477bf324d9ec44571c8bfdd2bf9dbeaa193bf2246`.
The retained 25,298-byte payload SHA-256 is
`70eb84dc01962adc1db56d659a5c18291921be1f01b92a48d1ccb75a532eb083`.
Both reference files and immutable generations remain in
[owner/fixture-release](owner/fixture-release), with original paths and hashes
in [release-retention.json](owner/release-retention.json). Creating-image
identities were never rewritten. The preserved earlier mixed-source image and
fixture remain separately named under `pre-fix` records.

For this small demo fixture, p99 submit/attach/terminal-take times were
4.525/3.938/8.553 microseconds. Attach-to-durable p50/p99 was
10.127/13.617 milliseconds, separate from capture, M2 validation and polling
delay. There are only 32 observations, so p99 equals the observed maximum.
Startup/shutdown-signal times were 39.257/1.676 microseconds. Shared admitted
peak was 626,302,647 bytes, returning to the original 536,870,912-byte Running
charge after shutdown; these are budget counters, not process RSS. Exact arrays
are retained in the [writer](owner/release-writer-final-report.json) and
[reader](owner/release-reader-final-report.json) reports.

## M3b integration responsibilities and limits

The initial platform accepts Linux ext-family filesystems and was tested on
`/tmp` ext4, `rw,nosuid,nodev,relatime,errors=remount-ro`, kernel 6.8.0-138-generic,
rustc 1.96.0. It pins a trusted private directory FD, uses relative non-symlink
regular-file operations and an advisory exclusive directory flock. M3b startup
must durably create the root and required ancestor entries before enabling the
service. It must retain the same shared budget and the service's Running child
across world transitions. Network filesystems and malicious external edits are
unsupported; physical power-loss/controller flush behavior was not tested.

M3b must attach a capture and its metadata from one coherent actual boundary,
with an explicit policy for still-uncaptured intents if a world load occurs.
Attached operations and their terminal results survive unrelated world loads.
On load, perform full M2 validation, admitted hydration and inert pending-work
preparation before any host adoption. Delivered load input ownership and later
validation/staging/disposal belong to that coordinator. Preserve truthful
capacity/recovery failures and never infer orphan files as latest saves.

M3b retains actual Bevy/controller/host-time adoption, initial publication without
polling, service activation, projection staging, old-generation disposal and
retirement. M3c controls and M3d real host-frame/renderer acceptance remain later.
The small release fixture measures only new storage API work, excluding capture
and M2 validation from its call timings. It is not the authored/populated city.
Historical M2a16 capture p99 remains 76.099/225.320 ms authored/populated;
hydration and populated disposal require their later offload/staging work.
M0 renderer/full-stress and human/uncontrolled-provider gaps remain unchanged.

No visible app, live provider, audio/device, GPU or 20,000-resident probe ran.
`docs/codex_gdd/`, `gauntlet/`, `reference/` and the ambient feature work remain
outside this leg and untouched.
