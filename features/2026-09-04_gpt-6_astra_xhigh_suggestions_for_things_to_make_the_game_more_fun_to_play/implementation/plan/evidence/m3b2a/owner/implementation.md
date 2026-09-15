# M3b2a — Stable admission groups and atomic promotion

This is a prerequisite subcut of M3b2, agreed with the coordinator on
2026-09-15 after reading the actual M3b1 interfaces. Accepted predecessor is
`72bc316e592a260344033689ae33b1ca7aa4c361`. It does not implement complete App
adoption, an allocation census, immutable startup assets, archive/runtime
backpressure, player controls or frame acceptance.

## Ownership contract

`CheckpointBudget` now stores at most six allocation groups in a fixed array.
Each group gets a monotonically increasing identity; exhausted identities are
refused, never wrapped. A `Reservation` retains that identity, its own charged
bytes and the shared usage mutex. Its cohort is resolved under that mutex.
Resize, child admission, destruction and promotion all use the same lock, so
a worker cannot debit the new Running world using a stale cohort label.

Ordinary subordinate leases belong to their parent's group. Candidate typed,
asset and service children therefore follow their candidate through promotion.
`reserve_running_overhead` instead joins a separate persistent Running group.
The existing storage and preparation service calls automatically use that
disjoint group and cannot follow a replaced domain into retirement.

The six maximum groups are: active Running authority, persistent Running,
SavePayload, LoadCandidate, retirement transport, and the former Running
authority attached to that retirement. Child leases share an existing group;
they add no entries. A fresh ordinary reservation remains refused while any
group of the requested cohort survives. There is still at most one save, one
candidate and one retired generation.

## Two-phase promotion

`prepare_promotion(&mut Option<Reservation>, &Reservation, &RetirementLease)`
checks the shared budget, replaceable Running group, candidate group, an unused
retirement transport and the absence of another promotion. It changes no role,
byte charge or producer state. Its ticket borrows the live `Option` and the
candidate reservation; it leaves the live root in its original bundle. Simply
dropping the ticket clears only its exclusive transition marker. No explicit
cancel operation is needed to keep the old world charged.

`Admitted<T>::prepare_promotion` provides the same operation for an opaque
candidate without exposing its reservation or extracting its value.

After the host has independently preflighted its entire old and new bundles,
`PromotionPermit::commit()` performs only infallible moves under the admission
lock. It changes the old authority group's role to RetiringGeneration, changes
the candidate group to Running, and transfers the original live root into the
existing shared `RetirementLease`. Total admission is unchanged. Persistent
Running and SavePayload are untouched. The ticket provides no Engine extraction
or permission to use it as a substitute for the complete host barrier.

Both the retirement transport and the attached old authority group become
permanently marked as used for retirement. A surviving child cannot wrap itself
in another `RetirementLease` and masquerade as fresh transport for a second
world. A later load requires a genuinely fresh retirement group after the
previous generation has released every child.

## Actual release versus worker disposal

The retirement owner holds the original running root in addition to its
transport reservation. Existing endpoint/job/publication clones of this lease
pin that original charge without cloning its capacity.

An observer created before promotion learns the two stable group identities
through one fixed shared pair. `RetirementRelease::released()` requires both
the end of the lease's strong ownership and the disappearance of every child
in either group. The observer holds only a weak usage reference and fixed
identity metadata; it does not retain a charged reservation. Reusing the same
cohort or numeric byte amount cannot revive an old observation. This preserves
the preparation worker's distinction between `owners_disposed` and `released`.

Locks involving retirement metadata use usage first, then the retirement root
or group-pair mutex. The byte accessor takes only the root mutex. No destructor,
IO, arbitrary callback or payload destruction runs under the usage lock.

## Numerical limitation retained

The shared ceiling is still 1 GiB; complete raw/typed caps and the 512 MiB
Running minimum are unchanged. Migration does not manufacture capacity. Moving
a 512 MiB old root into retirement and growing a second Running root to 512 MiB
cannot fit with persistent services. Promoting a smaller candidate also does
not by itself satisfy the existing Running minimum for another capture.

The next owner must establish an actual disjoint allocation and frozen-old-world
bound, persistent installed/shared allocation ownership, bounded global archive
work and detached backend-job lifetime. It must prove save-again during delayed
retirement, or refuse before any migration/fence with all original owners intact.
The small-charge promotion tests establish ownership arithmetic and lifetime,
not those unimplemented application or heap claims.

## Verification provenance

`owner/run.py` invokes the shared `component_input_sources.py` enumerator and
records the exact command, sanitized Cargo environment, start source map,
result and raw-log hashes. Raw files remain under `/tmp/alibi-m3b2a-*`; each
completed command also retains deterministic lossless gzip. Development or
source-changing commands remain separate from final frozen verification.

`focused-promotion-01` began before the coordinator's fifth independent test
was added; it is development feedback even if it passes. Its recorded source
comparison identifies the change. Final verification must use the complete
frozen owner and coordinator test files.
