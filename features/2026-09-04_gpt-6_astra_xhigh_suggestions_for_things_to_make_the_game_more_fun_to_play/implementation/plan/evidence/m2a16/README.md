Status: M2a16 complete read-only envelope implemented and independently accepted (2026-09-15). M2b–M2d and M3 application integration remain.

# M2a16 — Complete checkpoint envelope

The actual completed host boundary now exports one strict V1 envelope containing
all sixteen authoritative components. Validation uses the saved components and
exact installed definitions together. It refuses inconsistent component mixtures,
missing fields, incompatible definitions, unsafe next-consumer horizons and
nonempty World events before returning an admitted immutable candidate.

The candidate owns the original bytes, bounded manifest and private category
offsets. Views borrow that owner; raw bytes and admission cannot be detached.
Stable world lineage is independent of runtime generation. Compatibility includes
the actual host executable, so separately linked images can reject each other's
saves even when their source is identical.

The shared four-cohort 1 GiB limit, 64/128 MiB encoded limits and 128 MiB total
expanded limit remain unchanged. Typed expansion, reconstructed indexes, JSON and
diagnostic scratch, extraction staging and surviving subleases are charged before
allocation and retained until disposal. The 512 MiB Running allowance is a
trusted scoped minimum, with a construction/capacity proof for these selected
authored and populated workloads. It does not measure every live process heap.

## Verification

The unchanged final workspace passed **2,178 tests, zero failures and 40 ignored**
across 46 result groups, including twelve independent public complete-checkpoint
cases. All fourteen owner command attempts, including failures and the exact host
fixture refresh, are retained and independently audited. All 48 changed/new Rust
files passed scoped formatting and remain unchanged after the fixture refresh.

- [Coordinator review](coordinator/review.md) records findings and acceptance gates.
- [Owner handoff](owner-handoff.md) names production interfaces and remaining work.
- [Allocation and boundary proof](owner-design.md) covers typed layout, scratch,
  numeric consumers and the explicitly scoped Running inventory.
- [Running scope review](coordinator/running-scope.md) distinguishes semantic
  authority, actual fake transport and excluded runtime/renderer allocations.
- [Original command audit](coordinator/owner-command-audit.json) checks exact
  originals, archives, source identities and final test totals.
- [Host fixture refresh audit](coordinator/host-fixture-refresh-audit.json) proves
  three identical writers and the sole installed-catalog fingerprint change.

Both release smokes, all six timing runs and exact complete fixture checks passed.
The [measurement record](performance/README.md) and [fixture audit](coordinator/fixture-audit.json)
preserve the actual costs and executable compatibility policy. Synchronous
capture is not accepted for the application frame budget. No Engine hydration,
external retry/interruption adoption, file publication or renderer restoration is
implemented by this cut.
