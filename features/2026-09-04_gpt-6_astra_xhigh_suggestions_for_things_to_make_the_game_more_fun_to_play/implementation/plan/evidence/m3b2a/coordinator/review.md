Status: M3b2a independently accepted (2026-09-15). Whole-App allocation/runtime accounting and adoption remain pending.

# M3b2a coordinator review

Accepted predecessor: M3b1 `72bc316e592a260344033689ae33b1ca7aa4c361`,
source map `ea4086cf10fdf5e6d0781f3191e2f73cc9dcd7460889a4d98646a1f993783d80`.

The full M3b2 design has three necessary boundaries: stable allocation identity
and promotion, actual allocation/log/runtime accounting, then complete
application staging/adoption. This cut implements the first. The 512 MiB
Running minimum, 1 GiB shared cap and complete checkpoint limits remain
unchanged. Renaming cohorts alone cannot establish two-world residency or
save-again capacity during delayed retirement.

## Source contracts

Reservations now identify one of six fixed allocation groups by a monotonic ID.
Every resize, subordinate admission, release and role transition resolves that
identity under the same Usage mutex. Persistent Running overhead has a separate
group from the replaceable world. Old and candidate children follow their
original allocation group when its role changes.

Promotion preflight borrows the live owner's Option<Reservation> and the
candidate lease. It retains a retirement pin and prepares only a transition
marker. Dropping the permit removes that marker while leaving the live root
in its original owner. Commit moves that root into the retirement pin and
changes the two roles under the accounting mutex, with no allocation or
fallible external action. Complete host preflight, fencing and actual graph
transfer remain the future application's responsibility.

The opaque Admitted<T> wrapper can prepare promotion while borrowing its
original reservation, without exposing or extracting its payload.

## Review corrections

- Consuming the sole live root during promotion preparation would permit a
  dropped ticket to release the running graph's charge. The borrowed permit
  instead leaves the root in its live owner until commit.
- Observing only Weak<RetirementOwner> would report release while a migrated
  child still kept retired storage alive. Release now also checks the stable
  original/attached group identities through weak usage access. The observer
  retains metadata, not the corresponding byte charges.
- An already-used retirement group cannot accept another world through a newly
  wrapped surviving child. A group-level used marker preserves that refusal
  after the original retirement handles disappear.

Reviewed lock order is Usage before retirement-root/group metadata locks.
RetirementLease::bytes takes only the root lock; it does not acquire Usage.
The final release observer uses the same Usage-to-group order as commit.
Accounting critical sections perform no IO, graph destruction or user callback.

## Independent witnesses

The root owns `crates/cathedral-sim/src/checkpoint/promotion_review_tests.rs`:

1. Ordinary ticket Drop preserves the exact live root, all original roles and
   charges, including repeated prepare/cancel cycles.
2. Old, candidate and persistent children follow the correct roles. A delayed
   endpoint and then a migrated child both prevent final release. A later
   same-sized retirement cannot revive an old weak observer.
3. Foreign budgets, persistent roots and occupied retirement refuse without
   consuming the live owner or mutating charges.
4. Thirty-two barrier-coordinated worker resize/drop races leave the newly
   installed Running group untouched.
5. An actual Vec payload in the opaque Admitted wrapper remains charged through
   its destructor after promotion.

These use small charges to test accounting identity; they are not an
application allocation census or a proof of the Running floor.

The fifth test was added while the first focused compile was starting. That
command must retain its actual source-integrity result as development feedback;
final acceptance requires the unchanged final source map.

## Final acceptance and handoff

Frozen owner tests passed 3/0; the complete checkpoint namespace passed
210/0/27 ignored, including all eight new tests. Scoped formatting passed.
The final source map is
`04046178497e975d6942f76232ef3696fd5a4ad7674494ffc817e79d5b978032`:
1,003 inputs, one new review-test file, two changed checkpoint files and
1,000 unchanged M3b1 inputs.

The final unchanged workspace passed **2,280 tests, zero failures, 47 ignored**
across 46 printed result groups. The actual saved-file fresh-process preparation
test has an explicit passing row; this invocation suppressed passing child
stdout, so no unprinted child group is added. Compilation took 22m46s and the
recorded runner wall time was 1,497.473528s. Final format and diff checks passed.

All nine command records passed the independent provenance audit, with no
pending or orphan command maps. The audit verifies each original raw hash,
deterministic gzip hash and lossless contents, sanitized Cargo environment,
runner/enumerator identities and recorded source changes. Development results
remain distinct from frozen acceptance. The final raw workspace SHA-256 is
`6d9128f662662d617c237ec456bf98b8cc24cf991ff69cee20f463dfc6348010`;
its gzip SHA-256 is
`c35df04bdc82c7b5ae9f4b3c453a918fe1c23f5ec8a710dce8b0f2ea592258f6`.

The boundary audit requires all eight new witnesses and twelve predecessor
transport/service lifetime tests to have passing rows. The source delta and
final image audits also passed. Read-only GDB measured Usage=240, Group=32,
Reservation=24, PromotionPermit=64, RetirementOwner=64 and
RetirementRelease=24 bytes. The prior largest-delivery-root bound, current Core
and new fixed metadata sum to 60,712 bytes, including 1,024 bytes for fixed
allocation rounding, within the unchanged 65,536-byte control allowance.
Both final test ELFs were independently hashed in place and their exact queries
checked against the raw output. No separate executable copies were preserved.
These are fixed layouts, not world/host/native heap measurements.

The owner explicitly ceded source and executable ownership after the final
diff record; no command remained running. Root accepts this bounded primitive
for its reviewed commit. M3b2b must establish disjoint live/candidate/
retired/persistent bounds and log/runtime backpressure before M3b2c can adopt
the complete application. Root's reserved host adoption test file has not been
implemented in this prerequisite.
