Status: Native backend retention implemented and independently accepted (2026-09-15).

# M3b2b2 — Retained native backend work

Backend native work now has explicit finite ownership through real termination:
shared runtime/DNS admission, generation-bound recording disposal, bounded local
child/reaper/logger generations, and retained bounded realtime close tasks.
Final off-frame destruction joins native workers. Cancelled async waiters cannot
release running DNS capacity; cancelled close futures dispose their transport
before returning generation retention and admission.

The shared runtime uses two async and two additional blocking workers, with
explicit 2 MiB stacks. Its admitted constructor reserves a scoped 16 MiB
persistent Running allowance from the caller's existing CheckpointBudget.
A configured speech generation has at most eleven native workers, including
retained local cleanup, for 22 MiB of requested stack extent. These are separate
source-backed native scopes, not a measured whole-process heap or proof that
existing world reservation sizes cover complete native/transport allocations.

The final source map is
`320640c264e4646c374ccbc3c8681ea14a05dddede6b273810fff46e8feaf0ef`
with 1,007 inputs: two new files, eleven changed predecessor inputs and 994
unchanged inputs. Accepted predecessor is M3b2b1 commit
`b37d8fcff58efcd219766a98fd0e1a83b731df7c`.

The corrected frozen focused backend run passed 202 tests, zero failed, three
ignored. Its eight new witnesses cover DNS retained capacity and limits, native
joins/charge lifetime, recording disposal saturation and exact input refusal,
repeated child cleanup saturation, bounded close timeout, and close cancellation
before first poll and at await. The independent review supplied three of these
witnesses. Final formatting passes on the same source. The frozen full workspace
passed 2,300 tests, zero failed and 47 ignored across 46 summaries in 1,172.2024
seconds. All 46 read-only final ELF layout queries passed; preparation fixed
control remains 60,952 bytes under its 65,536-byte allowance. The final command
records report unchanged source. Exact retained logs, source maps and executable
identities are linked from the owner handoff.

The first native focused failure is retained as a fixture setup error: the
independent fixtures initially omitted the mandatory Running authority before
persistent admission. The first workspace run was deliberately interrupted at
source review after cancellation disposal ordering was found insufficient.
That partial raw log, deterministic gzip, original source map and explicit exit
130 result remain under owner/final-workspace; final acceptance uses the distinct
owner/final-workspace-02 record. No partial run is claimed as acceptance.

See [implementation and allocation scope](owner/implementation.md),
[owner handoff](owner/HANDOFF.md), and [independent review](coordinator/review.md).

Diagnostic sinks, immutable installed startup recipe, complete disjoint App
allocation accounting and whole-App staging/adoption remain later cuts.
Default startup still needs actual shared-budget wiring. The original 512 MiB
aggregate Running minimum, 1 GiB shared ceiling and 128 MiB typed limit are
unchanged. M0 renderer/full-population and M3d frame acceptance limitations remain.
No live provider/device, visible window or GPU probe was used. Unrelated files
and accepted predecessor evidence remain untouched.
