# Release build provenance

Build 1 completed on 2026-09-09 against the pre-fixture 947-file source map.
Its immutable binary and all originals remain preserved. Final release evidence
uses build 2, which starts from the fixture-inclusive 950-file map verified
by `workspace-final-3`. The production delta between these maps is empty; the
change is the owner fixture test and three fixture files, including their README.

The explicit environment overrides and Cargo command match between builds.
The inherited `LDFLAGS` changed from `-L/opt/homebrew/opt/openssl/lib` in build 1
to unset in build 2. Both start records preserve those actual values. The
compiler, sysroot and thirteen installed allocator/parser source files were
separately checked unchanged on 2026-09-14; see
`allocation-library-resume-check.json`.

Build 2 briefly waited for the shared release-directory lock. The previous owner
confirmed every test session had exited; the lock cleared without interrupting
any process, deleting a lock or changing build directories. Cargo then rebuilt
dependencies. The precise cache-invalidating cause was not instrumented. The
two builds are not treated as a performance comparison: smokes and timing runs
use only the final immutable binary and record its exact source and environment.
