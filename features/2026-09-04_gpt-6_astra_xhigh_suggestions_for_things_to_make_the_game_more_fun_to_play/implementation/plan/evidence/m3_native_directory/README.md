Status: Implemented and owner verified 2026-09-22; root review/commit pending. Partial M3 prerequisite only.

# Actor-source native directory accounting

`CapturedActorSources::capture_admitted` now includes **1,052,736 bytes** for the
single native directory object in its pre-IO reservation. The existing source
owner keeps that reservation through discovery and reading; it shrinks only
after all temporary `ReadDir`/`DirEntry` owners have closed. Retained snapshot
cost and last-owner behavior are unchanged. No public API, directory traversal,
source limit, disabled-actor behavior or runtime platform restriction changes.

This completes the previously omitted DIR-object allowance on the audited
platform below. It does not establish complete source admission on every libc
or allocator configuration. Complete application admission still refuses.

## Platform and derivation

Audited target: x86_64 Linux/GNU, Ubuntu glibc 2.35-0ubuntu3.15, ordinary ptmalloc
mappings with `glibc.malloc.hugetlb=0`, and 4,096-byte system pages.
[Pinned identities](audit/identity.json) record the installed libc and rust-src
hashes, upstream source URLs/hashes and local disassembly hashes.

The installed Rust std `sys/fs/unix.rs` opens with `libc::opendir` (2080–2088),
reads through `readdir64` (852–940), and drops `DirStream` through `closedir`
(1000–1036). `DirEntry` shares the stream's Arc, but capture never retains an
entry beyond its loop body. Pending directories contain paths only, so native
streams cannot overlap. All error paths unwind the stream before the source
owner and its lease. This path does not use `fdopendir`.

[glibc opendir](https://raw.githubusercontent.com/bminor/glibc/glibc-2.35/sysdeps/unix/sysv/linux/opendir.c)
clamps the directory block hint to 32 KiB–1 MiB and makes one malloc request.
The installed [opendir disassembly](audit/opendir.disassembly.txt) verifies both
clamps and the 48-byte DIR header: maximum request **1,048,624 bytes**. This is
a source/binary bound; the tests do not claim to create a filesystem with the
maximum block hint. Failed allocation closes the already opened descriptor.

[readdir64](https://raw.githubusercontent.com/bminor/glibc/glibc-2.35/sysdeps/unix/sysv/linux/readdir64.c)
reuses that buffer. On this libc it aliases `readdir`; the archived
[symbol table excerpt](audit/directory-symbols.txt) and
[readdir disassembly](audit/readdir.disassembly.txt) identify the actual code.
`getdents64` refills it without an additional allocation. The
[closedir disassembly](audit/closedir.disassembly.txt) frees the DIR block before
closing its descriptor, including a close-error return.

[glibc malloc's allocation-size description](https://raw.githubusercontent.com/bminor/glibc/glibc-2.35/malloc/malloc.c)
at 104–109 bounds normal chunk overhead by 32 bytes and direct ordinary mmap
overhead by two size words plus a page remainder. The reservation uses
`1,048,576 + 48 + 16 + 4,096 = 1,052,736`, conservatively covering either form.
Its huge-page branch at 2343–2347 can use another mapping alignment; that
configuration is outside this mapping-envelope proof. The DIR malloc-request
bound does not depend on page size. Tests clear inherited allocator overrides
without recording their values and set the audited huge-page setting explicitly.

The allowance is charged on all targets to preserve ordinary startup behavior,
but proof remains specific to the pinned implementation and allocator mode.
Other libc/platform versions, replacement allocators, huge-page tuning, shared
arena/tcache allocations, allocator retention and process RSS remain unproved.
No exact resident-process bound is inferred from one object or its usable size.

## Diagnostic witness

The ignored Linux/GNU backend test uses the separately compiled
`owner/native_probe.c`, never linked into production. Its shim is preloaded only
into the probe command (`nice`, then the test executable), never Cargo or the
production binary; counters remain unarmed in `nice`.

Fixed TLS counters are armed only around capture. Each directory symbol calls
its own resolved `RTLD_NEXT` address once; malloc/free delegate directly to
glibc, avoiding resolver recursion. The three-directory fixture asserts exactly
three native opens to detect alias double counting. The probe records malloc
requests and usable sizes, peak simultaneous objects, frees and the source
budget visible before native work and after each `closedir` returns.

The witness covers admission refusal with only the old Rust allowance free
(zero native opens), successful traversal, a real opendir malloc failure with
descriptor-count recovery, injected iterator EIO, and application path-limit
unwinding. Every opened stream must be freed before the reservation shrinks;
the maximum live count is one and native reads/closes must allocate nothing.
The injected read error exercises Rust's error cleanup, not an actual kernel
getdents error. The real maximum request is established by the audit above.

## Verification records

All commands below finished with exit 0 on the same unchanged component-input
map `6c583d0df739ab5e41556b672fe36ded9abcaeace09b55161c338924b3957ad7`.

| Check | Result | Record |
| --- | --- | --- |
| Existing backend capture module | 9 passed, native probe ignored | [focused](owner/focused-01/summary.json) |
| Installed startup module, including maximum-shaped source fixture | 6 passed | [startup](owner/focused-01/installed_startup-result.json) |
| Explicit isolated native probe | 1 passed | [native](owner/native-01/summary.json) |
| Non-test cathedralbevy binary build | exit 0; binary not run | [build](owner/production-01/result.json) |

The native fixture observed a 32,816-byte request and 32,824 usable bytes per DIR,
three opens/allocations/frees on success and a maximum of one live DIR. Every
sample during native work retained the full construction reservation. These
are ordinary fixture observations, not measurements of the 1 MiB branch.
The source peak now reserves **60,832,550 bytes** (the unchanged 59,779,814-byte
Rust inventory plus 1,052,736 native bytes). The maximum-shaped fixture requested
**57,395,145 Rust allocation bytes** and still retains **38,146,851 bytes**.
Its Rust allocation counter does not measure native
malloc, allocator arena/RSS, or any parsers; those measurements are kept separate.

The normal executable SHA-256 is `36a186cc5a5ea28d574c5d7e31c6d334a0cb9457d32629fddd8e3874a425c168`. It was built,
never launched. No full-workspace rerun is claimed.

Runners preserve actual START/PID/END records, complete component source/input
maps, command/environment identity, raw/gzip log hashes and compiler/library/probe
identities. Probe C is evidence tooling outside component-input source scope
and has its own recorded hash. [audit.py](owner/audit.py) verifies the archived
records against the current source map without launching Cargo.

Reproduce after obtaining source/build ownership: use `uv run --no-project
--offline python owner/run_tests.py --after-freeze --run-name <new-focused>
--suite backend_capture --suite installed_startup`, then the native runner with
`--run-name <new-native> --focused-run <new-focused> --after-freeze`, then the
build runner with a new run name and `--after-freeze`. The native helper fails
preflight on a changed libc/page configuration; production startup has no such
restriction. New runner output goes into `owner/evidence/` and cannot overwrite
these archived records.

## Remaining M3 gates

Parsing and template compilation, generated crowds, backend environment/config,
transport ownership, production HydrationAssets construction and owner transfer,
mutable worlds/ECS/renderer, whole-App adoption, functional save/load controls
and restored-frame acceptance remain open. Prior workspace and source-capture
records retain their earlier source identities; they are not rerun claims for
this patch.
