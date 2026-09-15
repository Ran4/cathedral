Status: implementation and frozen adversarial/workspace verification complete; Cargo/source ceded for coordinator acceptance (2026-09-15).

# M3a backend durable storage

The accepted predecessor is M2d `b0cc0f27c11f596839a53bd69a64ac9269757667`.
The owner reconciled actual complete capture/input/budget interfaces, both M2c
and M2d handoffs, repository instructions and the current M3/protocol/budget/
ownership/authority documents before implementation. Coordinator approved this
bounded design before production edits, and separately approved the subordinate
Running overhead lease. Root owns independent tests and acceptance/commit.

## Public ownership

`CheckpointStorage` is independent of `BackendsHandle` and RuntimeGeneration.
An operation ID combines OS-random service identity and a checked increasing
sequence. Eight retained intents and terminal results share one preallocated
queue. Save intent retains only the explicit slot. Attachment binds a validated
title, captured wall instant and player-known location to the actual capture;
a delayed intent cannot publish request-time metadata. Refusal returns both the
payload owner and its exact metadata. Actual lineage and accepted logical instant come from
the complete candidate. The metadata type validates construction and decoding;
slot identities accept at most 48 ASCII alphanumeric/underscore/hyphen bytes.

Only the oldest outstanding operation can accept a save payload. The entire
`Admitted<CompleteCheckpointCandidate>` moves to one serial worker after an
exact budget/cohort observation. Refusals return that same owner. No extraction,
clone, Engine movement, Send override or second world-sized service copy exists.
Manual requests are not coalesced away. Strict request serialization makes
reverse worker publication impossible; later capture attachment refuses.

Load length is bounded by the reference and complete 128 MiB ceiling. A
LoadCandidate lease precedes Vec allocation and exact-length reading. Streaming
or admitted-buffer SHA-256 checks precede the existing pure closed-envelope
inspection. That inspection proves framing/profile/metadata/category-object
shape only; installed manifest and all owner validation still require M2.
The original input and its actual lease move together into the terminal result.
Unread results consume queue capacity and the one LoadCandidate cohort; another
load returns a terminal admission refusal, never another candidate allocation.
Raw-read or later envelope-scratch capacity refusal is `Phase::Admission` with
`WouldBlock`; it never labels a valid save malformed. Releasing pressure allows
the same unchanged file to load and pass M2 validation.

The service reserves a disjoint 3 MiB Running overhead lease before its control
owners and worker allocate: 2 MiB explicit thread stack and 1 MiB bounded control,
path, reference/diagnostic and streaming scratch allowance. This includes the
eight entries, at most three bounded reference records, 16 KiB record buffers,
64 KiB hash/read buffers and fixed internal path strings. Existing Running must
already be charged. The subordinate lease retains its cohort after the parent's
drop; last control/worker ownership keeps it until actual disposal. This is a
new service bound, not an assertion about total existing Engine/ECS/OS heap.

The pure envelope inspector reserves raw capacity plus 4 MiB before parsing,
then restores its original reservation on success/refusal after parsed owners
and intermediate diagnostics disappear. Larger existing resolver scratch stays
charged. Existing wire limits are 4 KiB keys, 64 KiB individual metadata tokens
and three decoded manifest strings totaling 4,480 bytes. A conservative 32x
64 KiB diagnostic plus escape buffer/manifest/fixed owners is below 4 MiB.

## Durable ordering and recovery

The worker pins an existing trusted private directory FD and holds exclusive
advisory flock. Every file action uses openat/linkat/renameat/unlinkat relative
to that FD. Files are regular, opened O_NOFOLLOW and O_NONBLOCK; slot identity
cannot traverse. Parent path selection and absence of malicious external edits
are caller contracts. Cooperating processes cannot publish concurrently.
The root and its ancestor directory entries must already be durable: this
service creates no directory and owns fsync only for entries inside the pinned
root. M3b startup must create its chosen root and sync the necessary parent
entries before enabling publication. The initial runtime refuses filesystem
types other than Linux ext-family magic 0xef53; ext4 is the measured instance.

Each small record is a closed typed body plus a canonical-body SHA-256. Payload
identities are derived from operation IDs, never from unchecked file text.
Each slot has active, previous, pending and bounded fixed-name record temps.
Generation payload names are immutable; linkat refuses replacement.

1. Validate current active reference and payload. Refuse existing interruption
   or damaged/missing active with a retained predecessor until explicit recovery.
2. Durably write/flush/replace/sync a pending journal **before creating any
   candidate payload**. It retains the original active acknowledgement, its
   older reference and the admitted candidate identity.
3. Write the unique payload temp, reread exact length and SHA-256 against the
   admitted validated capture, flush, link its immutable name and sync directory.
4. Write/flush/replace/sync the original acknowledged active as previous.
5. Write/flush/replace/sync the new active small reference.
6. After new active publication is durable, collect only known now-unreferenced
   older ancestry and candidate/reference temps; sync cleanup. Keep the journal
   until cleanup is durable, so failure cannot admit a fourth known generation.
7. Remove pending and sync directory. Only then return Saved.

"After successful publication" for cleanup means after step 5's durable active
reference, before the caller acknowledgement. Two acknowledged payloads plus
one candidate bound all known files through every phase. Unknown/orphan files
are quarantined and never promoted, scanned as latest, or bulk-deleted.

Any failed publication retains an in-memory copy of the original journal and
blocks new saves globally until explicit recovery. On restart, a surviving
pending journal blocks that slot. Previous selection uses its original active
reference even if the previous-reference rotation had not happened yet. This
matters on the third save: B is older, A was acknowledged, C is in progress.
Failure must offer A, not accidentally select B or newly visible C.

Recovery validates the journal's original acknowledgement, durably preserves
the original journal again, publishes that predecessor in active and previous,
collects only known unreferenced interrupted ancestry, then durably clears
pending. Retrying interrupted recovery never snapshots newly visible C as
acknowledged. If active or journal is damaged, explicit recovery can instead
select the valid previous reference. Unknown damaged ancestry stays quarantined.
An interrupted first save recovers to an explicitly empty slot.

An in-process failed final pending-removal/directory-sync is still a failed
save and offers its cached original acknowledgement. After process death, no
filesystem protocol can prove whether an external caller actually saw an
acknowledgement around the final durable instruction. Reopened active means a
verified published reference, never proof of prior UI acknowledgement. Pending
means interrupted and requires previous selection; previous remains available
even if pending removal became visible before its final sync completed.

Linux file fsync does not imply directory-entry durability; directory fsync is
separate ([fsync(2)](https://man7.org/linux/man-pages/man2/fsync.2.html)). Same-directory
rename replaces the reference atomically. NFS can return a failed rename after
the server already performed it ([rename(2)](https://man7.org/linux/man-pages/man2/rename.2.html));
network filesystems and unsupported platform replacement fallbacks are outside
the supported contract. Tests use isolated `/tmp` ext4 locations. Fresh-process
death tests establish interruption handling, not a physical power-loss test or
proof a disk controller honors flushes.

## Worker lifetime

Cancel before the worker starts yields Cancelled and disposes attached payload
on the worker. Once it owns an operation, cancel returns TooLate and the actual
success/failure remains promised. Slow IO cannot be safely force-killed; it pins
the one service worker, directory lock and leases. There is no replacement
worker per cancellation and no joining destructor.

Shutdown signals queued cancellation and returns a handle retaining terminal
access. Unread terminal results keep the worker alive. Drain them before the
explicit off-frame join; dropping the handle abandons the receiver, and the
existing worker disposes remaining owners before exit. `discard_result` likewise
requests worker disposal while retaining capacity/charge until actual release.
Normal host-side delivery transfers a load input; its later staging/disposal
belongs to M3b rather than an implicit destructor promise in this service.

## Verification scope

Owner covers actual complete small fixtures made by the executing image, every
publication IO phase, third-save failure, process death, recovery interruption,
cleanup, malformed files, serial queue/admission and lifetime. Root separately
owns pure ownership/envelope boundaries plus backend damage/pinning/queue tests.
One retained-reader witness keeps loaded A and its original LoadCandidate lease
through C/D publication and removal of A's immutable filename, then fully
M2-validates A before releasing its cohort and loading D. Another holds actual
PayloadWrite behind a gate: dropping the only service handle returns on a
separate host thread while the gate remains closed; the worker retains payload,
service charge and directory lock until released and actually disposed.
Exact command-start maps, original raw logs and mtime-zero gzip archives include
failed development runs. Frozen focused/format/full workspace verification and
source cession are recorded in [OWNER_HANDOFF](OWNER_HANDOFF.md) and
[verification.json](verification.json).

M3b application adoption, M3c controls, M3d real host capture/adoption/retirement
frames, M0 renderer/full-stress, human and uncontrolled provider acceptance stay
pending. Existing synchronous capture/hydration/disposal budget overruns are
unchanged; no host save-anywhere performance claim follows from this backend.
