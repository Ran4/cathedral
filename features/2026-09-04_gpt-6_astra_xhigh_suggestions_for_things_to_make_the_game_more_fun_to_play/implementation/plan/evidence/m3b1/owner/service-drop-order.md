# Failed service binding: owner lifetime correction

`final-workspace-02` preserved the corrected storage suite, then aborted in the
existing host `service_factory_refusal_mismatch_unwind_and_repeat_binding_release_all_owners`
test. Its real InertService Drop checks that the candidate and service storage
remain charged. The mismatch path kept returned services in
PreparedContinuation.failed_services, but that field followed both asset and
service reservations. Rust's declaration-order field destruction released those
leases too soon, causing the assertion and subsequent destructor double panic.
This was a new M3b1 regression, not an accepted predecessor failure.

The fix places failed_services before both leases, alongside the other actual
owners. The same invariant covers the retained API: a failed returned graph
keeps its real services until disposal, while its charge remains alive. A new
retained mismatch test checks zero early drops, unchanged charge, adoption
refusal, repeat binding refusal without another factory call, and all four
actual service drops before returning to the Running baseline.

`service-order-after` was an invalid Cargo target invocation (`--lib` for the
binary-only cathedralbevy package), exited101 before compilation, and is
preserved as command feedback. `service-order-after-02` uses the actual
cathedralbevy binary target and runs the entire public continuation test module,
including the exact pre-existing failure and the new retained lifetime witness.
It passed all eight tests with zero failures/ignores on unchanged source
ea4086cf10fdf5e6d0781f3191e2f73cc9dcd7460889a4d98646a1f993783d80.
The subsequent `final-workspace-03` passed the complete concurrent workspace on
that same source, including both service-lifetime witnesses.
