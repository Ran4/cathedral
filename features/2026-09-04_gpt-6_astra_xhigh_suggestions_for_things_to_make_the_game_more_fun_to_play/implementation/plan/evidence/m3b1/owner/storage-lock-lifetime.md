# Storage lock lifetime correction found during final verification

The initial frozen `final-workspace` command compiled all targets but exited 101
on the backend test group: 185 passed, 1 failed, 3 intentionally ignored. Every
M3b1 test passed. The existing M3a recovery review test failed when reopening a
store after joined shutdown with Phase::Open / WouldBlock. Its source was
unchanged. This concurrent run is retained as failed evidence, not acceptance.

Store previously owned a File locked using LOCK_EX | LOCK_NB, with O_CLOEXEC.
Closing the worker's File does not necessarily end that lock: duplicated/forked
descriptors share its open-file-description. A concurrent child retaining such
a descriptor before exec is consistent with the suite failure, but this run did
not trace which process held the descriptor. The lock-sharing semantics are
documented in Linux [flock(2)](https://man7.org/linux/man-pages/man2/flock.2.html).

A deterministic regression duplicates the actual Store.directory using
File::try_clone, verifies a distinct Store cannot acquire the live writer's
lock, drops the original Store, then requires reopening while the duplicate is
still alive. `lock-regression-before` fails this exact last assertion, with
0 passed / 1 failed. This establishes the production lifetime defect without
relying on a race or attributing the concurrent failure to a traced child.

Store now explicitly unlocks its live owned descriptor in Drop, retrying an
Interrupted syscall. The Store guard is constructed immediately after the
successful lock, before the fallible startup sync. Private open_with_sync keeps
the normal File::sync_all call and permits a test to fail that exact boundary.
The second regression clones the locked descriptor inside the injected sync,
verifies another writer is still refused, returns EIO, and verifies reopen while
the duplicate survives. Dropping a Store on the storage worker releases its
lock; it does not relax concurrent-writer rejection, change durability ordering,
add a host-thread destructor, or change wire/schema/budget limits.

`storage-lock-after` records the concurrent focused storage suite on corrected
source:18 passed, zero failed, two intentional ignores, including both new
regressions and the original failed recovery test. `final-workspace-02` then
passed the backend suite before a separate M3b1 host service-drop regression
aborted the host binary (see service-drop-order.md). `final-workspace-03` carries
both fixes. Source maps distinguish these runs from the preserved earlier
failure. No serial-only rerun substitutes for the failing execution mode.
