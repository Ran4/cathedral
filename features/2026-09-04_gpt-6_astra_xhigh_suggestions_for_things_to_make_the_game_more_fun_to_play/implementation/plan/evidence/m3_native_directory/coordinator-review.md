# Coordinator review, 2026-09-22

Reviewed the native allowance derivation, Rust stream lifetime, isolated C
interposition and Rust callback/panic cleanup. Review explicitly identified
glibc huge-page tuning as outside the ordinary mapping-slack proof; the final
code/evidence records that boundary. Production retains safe std traversal and
adds no platform/version restriction.

Independently verified the focused, native and production source maps against
all 1,033 current inputs; all remained unchanged. Checked every raw/gzip log
hash and successful exit, the C/helper library hashes, the actual probe test
executable and installed libc identities, and the normal production binary.
All 16 focused tests pass and the normal build exits zero.

The native fixture observes a 32,816-byte request and 32,824-byte usable block,
one stream at a time, with no read/close allocations. Budget samples remain at
the full peak through native release. Pressure, allocation failure, injected
iterator error and application path-limit cleanup all pass. The 1 MiB maximum
buffer is established by pinned source and local binary audit, not this fixture.

This closes the single directory-object allowance for the documented platform
and allocator mode. It does not prove global allocator retention, other modes
or platforms, parsing/configuration, hydration or complete application adoption.
