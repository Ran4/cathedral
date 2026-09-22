Status: Owner implementation/verification complete 2026-09-22; root review/commit pending. No commit made.

## Change

The existing installed actor-source owner now pre-admits one native DIR object
with a named 1,052,736-byte allowance. The constant combines glibc's 1 MiB maximum
buffer, its 48-byte DIR header and ordinary ptmalloc chunk/mapping slack. The
lease remains held until discovery closes every native stream, then shrinks to
the same retained source cost as before. Safe std traversal and public APIs are
unchanged; no startup platform checks are added.

Proof is pinned to the audited x86_64 Linux/GNU libc, 4 KiB pages and ordinary
ptmalloc mappings with glibc.malloc.hugetlb=0. Other libc/platform/allocator modes,
huge-page tuning and shared allocator arenas/tcache/RSS remain unproved. The
maximum native request is a source/local-binary bound; the live native fixture
observes the usual 32 KiB buffer, not an invented maximum-filesystem measurement.

## Evidence

All commands were sequential, nice 15, default Cargo/compiler/target, locked and
offline, with headless/fake inputs and one test thread. No app/window, device,
audio or provider was run. Actual records bind START/PID/END, command/environment,
raw/gzip logs, source maps and compiler/libc/C-probe/library identities.

The backend capture module passed nine existing tests; the installed startup
module passed six; the explicitly selected ignored native probe passed one.
Its armed TLS instrumentation detects alias double counting, checks one live
DIR, zero allocations during reads/close and full reservation retention through
closedir, and covers pressure plus native/application failures. Probe instrumentation
is isolated to the probe command; Cargo and production never preload it.
The normal production binary also built successfully and was never launched.
See README.md and owner/audit-result.json for actual final identities and metrics.

Production edits: crates/cathedral-backends/src/world_data/capture.rs only.
Test edits: capture/tests.rs and new capture/tests/native_probe.rs. Evidence C
is separately hashed tooling, not product code or a new directory parser.

## Remaining gates and ownership

No parser/template compiler, generated crowd, backend environment/config,
production HydrationAssets factory, whole-application allocation/adoption,
working save/load or restored-frame gate is claimed. Complete admission still
refuses. Prior workspace/source-capture results remain historical evidence.

Source/Cargo/executable ownership is explicitly ceded back to root at final
handoff. No owned command remains live. No commit/push was made and unrelated
dirty paths were preserved. Root may review and commit this partial prerequisite.
