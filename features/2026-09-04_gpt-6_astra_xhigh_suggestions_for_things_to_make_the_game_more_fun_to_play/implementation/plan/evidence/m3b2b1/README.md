Status: M3b2b1 production prompt archive admission implemented and independently reviewed (2026-09-15).

# M3b2b1 — Prompt archive admission and retention

The scheduler and Night reserve archive capacity before accepting cognition
work. Capacity remains owned through held completions, immutable shared event
copies, queued/active writes and actual disposal. Archive refusal preserves
input allocations and semantic retry obligations. Restored held completions
obtain both lane permits transactionally before service activation.

The production writer has eight outstanding slots and a 16 MiB payload bound.
It streams the existing JSON/Markdown formats and preserves filename ordering
through session forks. The admitted constructor reserves a scoped 20 MiB
persistent Running allowance and retains it through worker join and surviving
permits. Ordinary reserve/record paths do not wait on disk. Final log teardown
remains off-frame. Existing IO-error and process-exit best-effort policies remain.

## Acceptance

- Final command: `cargo test --workspace --offline -j1`.
- **2,292 passed, zero failed, 47 ignored**, across 46 printed outer groups.
  All 12 new archive witnesses pass. The named fresh-process preparation test
  passed; an unprinted child summary is not counted again.
- Compilation: **22m53s**; total recorded time: **1,481.283s**.
- Final formatting and whitespace checks pass.
- All **19** command records pass independent raw/gzip/source/environment/helper
  audits; development failures and earlier source maps remain preserved.
- Independent boundary audit confirms the 12 new tests and 14 predecessor
  ownership/continuation tests.
- All **28** final ELF layout queries pass; preparation control storage is
  **60,952 / 65,536 bytes**. Native TLS/allocator overhead remains a trusted
  scoped allowance, not a measured process heap.

The accepted source map is
`8f246541e779e999382526d0a730eb95accb60965b1ff37df4b175f54c2ba0ca`:
1,005 inputs, comprising two new files, 29 changed predecessor inputs and 974
unchanged inputs. No input was removed. The accepted predecessor is M3b2a
commit `30f8188301b8f1585df478e2e6df5f16bc1434cc`.

The retained raw final log is
`/tmp/alibi-m3b2b1-final-workspace.log`, SHA-256
`3079fe2c46bc8c3e52564c82a43da7fc6dac17264626d47b3a4a95a3e61c376c`.
Its lossless deterministic gzip has SHA-256
`1a916af612e7a494730d85a5d4b19d2b876b239faea177c143defd59ae5f48ea`.
The final backend and sim ELFs were hashed in place; no separate copies were
preserved. Exact paths, sizes, hashes and queries are in
[the image audit](coordinator/image-audit.json).

See [the owner handoff](owner/HANDOFF.md),
[implementation and allocation scope](owner/implementation.md),
[independent review](coordinator/review.md),
[command audit](coordinator/command-audit.json),
[source delta](coordinator/source-delta.json) and
[boundary audit](coordinator/boundary-audit.json).

## Remaining scope

The default production writer is finite; its shared-budget constructor still
needs complete startup integration. Idle directory/model configuration, native
workers and transitive DNS tasks, diagnostic JSONL/stderr sinks, the immutable
installed recipe and disjoint complete-App allocation proof remain M3b2b work.
This cut does not claim whole-process, GPU or external-model memory accounting.

M3b2c whole-App staging/adoption, M3c controls, M3d frame acceptance and M4–M19
remain pending. The 512 MiB aggregate Running minimum, 1 GiB shared cap and
128 MiB typed limit are unchanged. No live provider/device, visible window or
GPU probe was used. The cosmetic test-only PromptExchange import cleanup is
deferred to the next substantive host edit; existing warnings are recorded
separately in the handoff. Unrelated work and prior retained artifacts remain
untouched.
