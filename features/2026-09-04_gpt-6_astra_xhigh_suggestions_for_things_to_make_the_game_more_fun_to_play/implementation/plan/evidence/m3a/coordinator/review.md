Status: M3a backend storage independently accepted (2026-09-15). M3b–M3d application acceptance remains pending.

# M3a coordinator review

Accepted predecessor: M2d commit
`b0cc0f27c11f596839a53bd69a64ac9269757667`, 2,229 workspace tests passing.
One fresh-context M3a owner performed implementation and Cargo verification. The coordinator owns
independent review/tests, final verification audit and the commit. This leg covers
backend slot storage; application adoption, controls and host-frame acceptance
remain M3b–M3d.

## Design decision

The reviewed design serializes operations on one storage worker. A save keeps its
original admitted immutable capture and service operation identity across world
replacement. Eight operation reservations include queued, active and unread
terminal work. Only the oldest eligible save intent may acquire the single
SavePayload; a returned load retains the shared LoadCandidate lease until its
actual owner is disposed or transitioned through M2.

The worker pins the selected store directory and uses directory-relative file
operations under an exclusive store lock. The initial implementation requires
Linux ext-family filesystems and is tested on ext4; other types are refused.
The store must report the
actual filesystem used for acceptance. Atomic reference replacement and durable
publication are separate operations: Linux documents a separate directory
flush for directory-entry durability, and its rename contract specifically
warns about uncertain failure results on NFS.
Sources checked 2026-09-15:
[fsync(2)](https://man7.org/linux/man-pages/man2/fsync.2.html) and
[rename(2)](https://man7.org/linux/man-pages/man2/rename.2.html).

The caller must durably create the selected store directory and its ancestor
entries before starting this service. M3a opens an existing directory and syncs
its own entries; application startup owns initial directory creation.

A bounded pending journal becomes durable before creating a candidate payload.
It identifies the candidate and preserved reference ancestry. Incomplete
publication blocks another automatic save until explicit recovery. Recovery
never promotes an arbitrary orphan or temporary file. The prior acknowledged
reference/payload survives replacement and durability failures. A reopened
published reference cannot prove whether the former caller observed the final
acknowledgement; this ambiguity must remain explicit.

Approved pure extensions are identity/cohort observation without extracting
admitted storage, bounded complete-envelope inspection without hydration, and
a disjoint Running overhead lease. Inspection preserves any prior larger
reservation and disposes scratch before shrinking. Service metadata, bounded IO
scratch and its explicitly configured worker stack remain charged through the
last service/worker owner. These extensions do not expose a private Engine.

## Independent acceptance questions

| Boundary | Required witness |
|---|---|
| Shared admission | Foreign-budget/wrong-cohort owners cannot enter the worker; refusal returns ownership. Service overhead fails admission before allocations and outlives its parent Running lease when needed. |
| Unread results | Eight admitted operations consume capacity even after completion; an unread load still owns LoadCandidate. Cancellation cannot manufacture free capacity while retained data survives. |
| Ordering and identity | Delayed older same-slot work cannot publish after newer work; a world change cannot relabel a pending save. IDs fail safely at exhaustion and remain scoped to their service. |
| File read | Bounds precede allocation; short/growing/corrupt payloads and incompatible envelopes fail without a candidate leak. Framing/checksum success does not claim full M2 definition/component validation. |
| Publication | Returned errors and process death cover every write/flush/link/reference/directory phase. Begin with two valid generations before interrupting a third save. |
| Recovery | Interrupt recovery itself, repeat it, and fail cleanup. Preserve the selected valid reference and bound known payloads across restart; never infer an unknown file is acknowledged. |
| Lifetime | Slow disk and nonblocking handle destruction leave one bounded worker/charge, prevent concurrent writers, and eventually release resources. No Engine is sent to a worker. |
| Scope | Fresh processes use complete saves made by the same final image; renderer, physical power loss and M3b adoption are not inferred from CPU/file tests. |

## Review findings and independent tests

Early source/design review required three recovery/lifetime corrections before
acceptance: retain durable cleanup ancestry until old-file cleanup finishes;
repair a missing or corrupt active reference/payload from the valid predecessor
even without an interrupted-write journal; and keep the existing worker alive
for unread terminal disposal after shutdown. Metadata construction now checks
borrowed limits before copying, and deserialization also enforces those limits.
These source corrections pass executable verification.

The coordinator independently authored four pure tests in
`crates/cathedral-sim/src/checkpoint/storage_review_tests.rs`:

- An admitted owner retains its exact budget/cohort identity when moved to a
  worker; equal-sized foreign charges grant no authority. The real complete
  candidate/input types must satisfy Send without an unsafe override.
- A Running service sublease cannot bypass saturation or integer bounds and
  cannot free its cohort while the child still owns storage.
- Envelope inspection refuses before consuming unavailable scratch, retains its
  input, and preserves a prior larger resolver reservation after malformed input.
- A 32 KiB hostile typed metadata value returns only a bounded diagnostic and
  releases its parse scratch without retaining the input through the error.

Four backend tests in
`crates/cathedral-backends/src/checkpoint_storage/review_tests.rs` use the public
service API and the owner's real complete-capture fixture:

- Preview and slot deserialization cannot bypass the bounded closed types or
  introduce private NPC preview fields.
- Missing/truncated active references and a truncated active payload each allow
  previous-load validation, explicit recovery, a new successful save and another
  valid predecessor load. Damage occurs offline between service lifetimes.
- After the opened directory is renamed and its original pathname replaced,
  subsequent publication remains in the pinned directory. An offline copied
  checksummed reference cannot authorize a different slot; the original still
  passes complete M2 validation. This is a bounded isolation witness, not a claim
  to support arbitrary concurrent external edits.
- A completed unread load prevents a second LoadCandidate allocation. Shutdown
  retains its disposal worker and exact charges until abandonment is disposed.

Review also moved UTC/location/title metadata from request enqueue to capture
attachment. A queue entry fixes the slot intent; attachment binds the admitted
payload and its capture metadata, and every refusal returns both unchanged.
This prevents a delayed save or attachment retry from acquiring metadata from
another live instant. Actual lineage and logical time come from the candidate.

The owner added a separate blocked-disk destructor witness after review: abandon
the only service handle while the payload-write gate remains closed, require its
drop to return, keep charge and lock pinned, then release the gate and validate
the resulting save after actual worker disposal. Worker-operation unwind also
has an explicit failed terminal and recoverable journal witness.

The independent tests pass alongside final workspace and release fresh-process
verification.

Final review found one further error-reporting defect: envelope inspection can
refuse its additional scratch reservation before parsing a valid file. Storage
originally mapped that refusal to a malformed-envelope error. The corrected
path preserves `Phase::Admission` and `ErrorKind::WouldBlock`; other capacity
refusals use the same classification. A backend test reserves enough Running
capacity to permit the exact raw read but refuse inspection scratch, checks
unchanged payload bytes and no leaked LoadCandidate, then releases pressure and
fully M2-validates the same file. The corrected 993-input freeze is
`9de70e2bcdd31fc92ba98e6415beec217352d1f1fe2706fa269710812181d2bc`.
Earlier passing runs and the first release writer remain pre-fix or mixed-source
development evidence, according to their recorded source maps.

The final owner coverage check also added a retained-reader witness to the
existing two-generation test: materialize loaded A, retain its LoadCandidate
while C and D publish and collect A's immutable filename, then fully validate A
from its still-owned bytes. Only after actual candidate disposal can another
load be admitted. The coordinator reviewed this test before the final release
and workspace freeze; it changes no production behavior.

After that test extension, the final 993-input freeze is
`97936f28124b1c9876b8f1de1c91a7e5e271f00eef982f38ee6126c3a5c52f23`.
`format-final-03` checks all ten changed/new Rust files. `focused-final-04`
passes 16 tests with no failures and two intentionally ignored manual helpers;
its 42 returned-fault cases, 21 publication deaths, 16 recovery deaths and
37 successful fresh same-image M2 readers are retained in the original output.
The final workspace passes on the same frozen source map.

## Independent release audit

`release-writer-final-02` and `release-reader-final-01` both pass on that final
source map. [release-audit.json](release-audit.json), produced by
[audit_release.py](audit_release.py), independently verifies the retained
executable, both checksummed reference bodies, both payload files, all raw
timing arrays and the fresh-process reader's identity/results. The exact release
test executable is `/tmp/alibi-m3a-final-release-image`, 26,730,496 bytes,
SHA-256 `52f7f4f408a5286194b509d477bf324d9ec44571c8bfdd2bf9dbeaa193bf2246`.
Its complete demo payload is 25,298 bytes, SHA-256
`70eb84dc01962adc1db56d659a5c18291921be1f01b92a48d1ccb75a532eb083`.
The previous generation remains separately referenced. The fresh process loads
the actual published slot and passes full M2 validation without identity edits.

The 32 serial samples measure only this small complete fixture's storage API.
Submit, attach and successful terminal-take maxima are 4.525, 3.938 and 8.553
microseconds. Worker durability from attachment has median 10.127 ms and maximum
13.617 ms. Startup and shutdown signaling are measured separately at 39.257 and
1.676 microseconds. The admitted peak is 626,302,647 bytes, including the trusted
512 MiB Running fixture reservation and the 3 MiB service allowance; this is not
a whole-process heap/RSS measurement. The audit retains all samples; no host
capture, populated-city, frame-time or physical-power-loss acceptance follows.

## Final acceptance

`cargo test --workspace --offline -j1 -- --test-threads=1` passes **2,249 tests,
zero failures and 46 intentional ignores across 46 test groups**. It completed
in 1,570.946 seconds on unchanged final source. The original raw log is
`/tmp/alibi-m3a-workspace-final-01.log`, SHA-256
`78418ee6ed38de0a3f2fed25c1c42f9b9d4dedc8d33b55e3df3f639b17b0998e`;
its exact mtime-zero gzip has SHA-256
`1834b401c5a116603d0e48c5df673f3325c3eee32580917c8d4c90b594f29edd`.

[command-audit.json](command-audit.json) independently checks all 25 owner
commands, their raw/archive bytes, source maps, runner/helper identities and
removed build flags. There are no incomplete command records. The sole failed
development suite and every mixed-source/pre-final result remain explicitly
recorded; they are not counted as final acceptance. The final focused output
proves both complete fault matrices without inflating totals from escaped child
test output. The read-only build-resource observation is platform/progress
evidence only; its namespace exposed no compiler process rows.

[source-delta.json](source-delta.json) confirms six added Rust files, six changed
existing inputs, no removals and 981 unchanged predecessor inputs. The four
independent pure tests also pass in the final workspace; their prior focused
run used byte-identical simulation crate inputs.

M3a is accepted for backend slot publication/read/recovery and bounded service
ownership on its stated platform. Commit this coherent leg before dispatching
the next fresh owner. M3b must preserve storage identity and service admission
while preparing and adopting a whole host generation. Normal controls, actual
whole-city capture/adoption/retirement frame costs, renderer/full-stress and
human/provider acceptance remain with their named later owners. No cap changed,
no visible game window was launched, and unrelated work remains outside this leg.
