Status: Partial M3 source-input repair implemented and owner verified 2026-09-22; root review/commit pending. Complete M3 remains unaccepted.

## Scope

When smart actors are enabled, freeze the actual actor source bytes used by installed startup, admit their
Rust-owned discovery/read/copy buffers before IO, and retain one immutable owner.
The source factory never borrows a running World. Production LocalEngine consumes
that snapshot; legacy loaders keep their existing public APIs.

This does not admit parser/compiled-template/generated-crowd storage, backend
environment/configuration, native directory buffers, transport allocations or
mutable World/ECS/renderer ownership. Complete application admission must still
refuse. No HydrationAssets factory or whole-App adoption is added.

## Implementation

1. Add CapturedActorSources in cathedral-backends::world_data with fixed input,
   discovery and path caps, a pre-admitted reusable sentinel read buffer, sorted
   character sources and an Arc owner whose reservation drops last.
2. Add borrowed pure composition and a borrowed LoreCast constructor sharing the
   old owned-input implementation, avoiding another full source text copy.
3. Capture in StagedStartup, retain under CommittedStartup, and route every
   installed LocalEngine disk-source read through the snapshot. Preserve the
   existing service and navigation owners and keep the complete gate closed.
4. Verify refusal, bounds, content equivalence, last-owner lifetime, immutable
   production routing and a maximum-shaped Rust allocation witness.

## Supported capture input

At most 4,096 total sources (seven fixed plus at most 4,089 character files),
4 MiB per source and 32 MiB aggregate. Discovery examines at most 16,384 entries,
256 directories including the character root, and eight levels below the root.
Raw relative paths are at most 1,024 bytes; joined absolute paths at most 4,096.
Paths retain the existing lossy UTF-8 conversion, file contents require UTF-8,
and discovered symlinks are ignored. A cast with no regular JSON files refuses.

The captured bytes form a stable source set for this startup, not an atomic
filesystem transaction. Source changes during capture can produce mixed file
versions; changes after capture cannot affect its consumers. Hostile concurrent
filesystem replacement and blocking special-file behavior are not repaired by
this slice. Missing/oversized source errors occur earlier during staging for enabled actors.
Disabled actors preserve the existing plugin early-return behavior: no actor
source snapshot is captured and missing lore/prompts do not prevent staging.
Attempted installed engine construction without a snapshot refuses explicitly;
it never falls back to fresh filesystem reads.

## Accounting

The peak formula in capture.rs includes preallocated discovery/output vectors,
their record layouts, raw paths, lossy retained paths, 32 MiB source retention,
two 4 MiB+1 transient buffers and enumerated filesystem/path scratch. Fixed-size
errors allocate no retained diagnostics. PathBuf allocations reserve their
eventual size before push; lossy names are copied to exact retained strings.

The scope is reviewed against x86_64 Linux/GNU Rust std. ReadDir retains one
Arc<InnerReadDir> with a DIR pointer and PathBuf; its root clone and Rust-owned
DirEntry CString/file_name copy are included. Linux dirent64 d_reclen has u16
width, so two 64 KiB filename envelopes cover names before application length
checks, including names longer than the misleading d_name[256] declaration.
Only one native directory stream is open at a time. Its libc DIR allocation is
explicitly unproved; other platforms need a separate filesystem-owner audit.

The read-only rust-src audit used:

`/home/ran/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/std/src/sys/fs/unix.rs`

SHA-256: `a25523228b0ba0dbf53aaa478b52e42021513d6563df033a34896ed31a20c3a5`.
Relevant regions are lines 259–275 (ReadDir), 418–424 (DirEntry), 892–940
(variable-length names), 1045–1051 (file_name) and 2080–2089 (root clone).

The retained formula uses actual vector/string capacities and boxed string
lengths. It must fit the admitted peak before resizing, after temporary discovery
and read buffers are disposed. Snapshot Arc clones retain the same reservation
through the last consumer. LocalEngine already retains CommittedStartup last.

## Review and verification gates

The patch was formatted and passed git apply --check before application.
Actual focused verification and non-test production-build records are archived
under owner/. README.md binds their successful results to the exact tested maps.
The maximum-shaped allocator witness passed in both recorded host iterations.
It observes Rust requested allocations only, explicitly excludes native ReadDir
and runs no parsers; it does not establish a complete installed-asset bound.

The focused backend capture, existing lore, installed startup and LocalEngine
startup modules passed, with only changed host modules repeated after review.
A normal production build also passed. The earlier workspace result belongs to
the pre-patch frozen tree and remains separate evidence.
Do not expand any whole-world budget, turn F6/F9 into alleged working save/load,
or mark M3/M4/M5/M6 complete based on this repair.
