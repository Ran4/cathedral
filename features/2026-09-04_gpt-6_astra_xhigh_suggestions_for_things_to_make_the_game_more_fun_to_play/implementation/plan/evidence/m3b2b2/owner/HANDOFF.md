# M3b2b2 owner handoff

Status: implementation, focused and frozen workspace verification, final layout
inspection and whitespace verification passed. Source/Cargo/executable ownership
is ceded to root (2026-09-15 10:31 UTC); no owner command remains running.

Accepted predecessor: `b37d8fcff58efcd219766a98fd0e1a83b731df7c`.
Current frozen source map:
`320640c264e4646c374ccbc3c8681ea14a05dddede6b273810fff46e8feaf0ef`
(1,007 inputs; two new, eleven changed, 994 unchanged).

## Interfaces and ownership

- BackendRuntime now joins actual native workers on final off-frame Drop.
  `start_admitted(&CheckpointBudget)` reserves a separate persistent 16 MiB
  Running allowance before construction; default startup wiring remains later.
  Production native closures retain BackendExecutor (Handle then shared core),
  not an Arc that can destroy Runtime on its own worker. No core owns Runtime.
- Runtime has two async workers and two additional blocking workers, each with
  explicit 2 MiB stack. Its shared two-slot NativeResolver replaces production
  reqwest/Tokio implicit DNS paths. DNS admission includes queued/running calls
  and retained answer iterators. Immutable generation endpoints remain pinned
  through non-abortable work and delayed result disposal. Ordered DnsWork and
  Answer aggregates retain admission after their actual payloads on cancellation.
- STT/TTS retain native JoinHandles and their outer endpoint/runtime until joins.
  STT disposal retains one active plus 64 queued unlinks; batch remains four
  queued jobs. Resolved path capacity is bounded at 4,096 bytes. Owned disposal
  refusal returns the exact original PathBuf; ordinary full-queue deferral still
  belongs to the private SessionDir policy.
- Worker allows two child generations total across active processes, reapers and
  stderr readers. A child exit alone cannot free a slot. Repeated poison refuses
  before another spawn while both remain; completed native handles are joined
  before reuse. NativeChild retains its immutable endpoint outside the closures.
  Failed reaper/logger spawn cannot abandon Child. close is terminal and keeps
  the existing off-frame SIGTERM / one-second grace / SIGKILL / wait behavior.
- Realtime detached cleanup has one slot and a one-second best-effort timeout.
  RetainedClose declares its future/transport before endpoint before slot, with
  Drop preventing disjoint async capture. Actual cancellation tests cover before
  first poll and at await while transport Drop is blocked. Further close requests
  while occupied dispose their transport without adding a detached task.

Read implementation.md for actual startup/failure/retirement field ordering,
scoped native counts and dependency source evidence. No Engine/World extraction
API, independent budget, provider/device or visible-window behavior was added.

## Verification and provenance

All commands use owner/run.py, the shared component_input_sources.py enumerator,
CARGO_HOME=/tmp/alibi-m1b-cargo, offline -j1 Cargo, explicit toolchain paths,
CATHEDRAL_HEADLESS=1, CATHEDRAL_FAKE_BACKEND=1 and bytecode disabled. The runner
removes inherited linker/Rust wrapper flags, writes start/source/result JSON,
retains exact /tmp/alibi-m3b2b2-* raw logs and deterministic lossless gzip.

- `focused-backend-02`: `cargo test -p cathedral-backends --lib --offline -j1`:
  202 passed, zero failed, three ignored. All eight new witnesses pass.
- `final-format-02`: scoped Rust 2024 rustfmt --check passes.
- `final-workspace-02`: `cargo test --workspace --offline -j1`: 2,300 passed,
  zero failed, 47 ignored across 46 summaries; exit zero in 1,172.2024 seconds.
- `final-layout`: read-only GDB inspection of the exact workspace test ELFs:
  46 queries pass; retained preparation fixed control remains 60,952 bytes under
  its 65,536-byte allowance. DnsWork is 96 bytes; RetainedClose is 64 bytes.
  No target execution or binary copy occurred during layout inspection.
- `final-diff-check`: `/usr/bin/git diff --check` passes with an empty raw log.

All five final records share the source map above and report sources_unchanged.
Exact commands, complete source enumeration, environment, raw-log and gzip hashes
are in their named start/source/result JSON files. Final whitespace verification
uses `owner/run.py final-diff-check /usr/bin/git diff --check`.

Workspace raw log: `/tmp/alibi-m3b2b2-final-workspace-02.log`.
Its SHA-256 is
`6d231160b1feaa49ad5b1e9da7f550ecd3ffc8628783b6256b3c3aff1f40e682`;
the retained deterministic `final-workspace-02.log.gz` SHA-256 is
`940bf29332fed3e949a6093abfb26e576d4dc2b040a7923994ca33f3fe79bdae`.

Final inspected images are retained in place and recorded in final-images.json:

| Test ELF under target/debug/deps | Bytes | SHA-256 |
| --- | ---: | --- |
| cathedral_backends-22310518b4c01644 | 387,831,896 | 87709de68c2168a4b1518678450556e5234e2124e972a8ab1fb13a795a794024 |
| cathedral_sim-29e329e3d5b1c8e4 | 333,258,360 | 6d18307944eb721d9fc1083068b6d676bcad1af29566f74d773e683594868888 |

The earlier `focused-native-01` failure is a preserved fixture setup error:
mandatory Running authority was absent before persistent admission. The corrected
fixtures retain an independent Running root throughout native cleanup and show
that its charge remains after shared/retired charges are released.

The first `final-workspace` run was interrupted deliberately during compilation
when source review identified cancellation field-order dependence. It is not
acceptance: its original source 75b29c55 remains unchanged, tool exit is 130,
and the partial raw/gzip log plus manually finalized interruption result records
what happened. RetainedClose and DnsWork corrections precede the distinct final
freeze and final-workspace-02. Earlier successful/development maps remain separate.

## Limits and next owner

A configured speech bundle's source-backed native maximum is eleven threads,
each requesting 2 MiB stacks. Its two local drivers admit four directly managed
child generations total; arbitrary descendants/model/GPU allocations are not a
process census. Shared runtime native allowance, per-generation stack inventory,
measured fixed Rust layouts, and external model memory are distinct. Mailbox/world
lifetime pins do not prove arbitrary earlier world sizes cover these allocations.

Actual complete startup admission, diagnostic sinks, immutable installed recipe
and disjoint complete-App accounting remain later M3b2b work; whole-App adoption
is M3b2c. Root recorded further HTTP task/pool accounting questions separately.
512 MiB aggregate Running, 1 GiB shared and 128 MiB typed limits are unchanged.
M0 renderer/full-population and M3d frame acceptance remain unmeasured/pending.

No branches, worktrees, commits or pushes were made. Unrelated docs/codex_gdd/,
gauntlet/, reference/ and features/2026_09_14_more_ambient_stuff.md are preserved.
Accepted predecessor evidence, retained executables and caches were not deleted.
The one task-generated Python bytecode file from interrupt finalization was
removed explicitly; it is not retained evidence.

## Cession

At 2026-09-15 10:31 UTC, the owner explicitly cedes all implementation source,
Cargo, test executable and layout-inspection ownership to root. All owner command
sessions have completed. Root may independently audit this evidence and commit
the coherent cut. No successor work was begun, and no further edits or executable
commands will run without renewed coordination.
