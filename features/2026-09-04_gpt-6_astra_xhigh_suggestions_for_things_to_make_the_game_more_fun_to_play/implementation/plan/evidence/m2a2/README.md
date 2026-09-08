# M2a2 private character, inventory and reference backbone

Status: Implemented and reviewed component cut, with workspace verification and repeated release measurements complete (2026-09-08). Complete M2a/M2b–M3 remain pending.

This cut extends accepted M2a1 (`72ed9be9115ffd2c899fe3da38a58252ec856725`) with exact private character and inventory authority and the World references needed to compose them. It deliberately exposes `WorldBackboneDtoV1`, not a complete World/Engine envelope. The [owner ledger](OWNER_COVERAGE.md) records covered state and all remaining owners, including exact later Round projection/planner/reference duties.

## Actual interfaces

- `Character::export_checkpoint` / `CharacterDtoV1::decode` preserve the whole private sheet and state separately. Seed holdings/pose/economic fields are historical, not a second live owner. No `from_sheet` is used to restore live state.
- `World::export_inventory_checkpoint` / `InventoryDtoV1::decode` preserve live items, offers, restock shares, ordered transform plans/reservations and held completion lineage. Candidate actor/catalog/event-sequence context must be supplied and validated by the eventual complete composition.
- `World::checkpoint_backbone_cost` meters the real borrowed structure without extracting it. `World::export_backbone_checkpoint` composes the covered records after admission and rejects unflushed DomainEvents or ledger dispatch/notification work.
- `WorldBackboneDtoV1::decode` builds the covered BTree/PlaceRegistry indexes and validates them against the supplied compatible catalog and ledger. `Admitted<WorldBackboneDtoV1>::into_candidate` revalidates and transfers those existing maps into an opaque covered candidate. It cannot replace World or construct an Engine through a production API.
- `Admitted<…>::encode` transfers the cohort charge to exact-capacity encoded bytes. Tokens remain attached until disposal; no plain returned DTO/payload releases its charge prematurely.

The new public DTOs implement serialization but not `Deserialize`; untrusted decoding goes through private remote wire definitions after aggregate admission. Every record denies unknown fields and requires all nullable fields to appear explicitly. Maps reject duplicate keys, sets reject duplicate identities, and ordered histories/reservations remain ordered without deduplication.

The pure sim performs no filesystem IO, clock reads, worker creation, save-only poll or service submission. The diagnostic example owns its own IO/timers outside the sim crate. A borrowed input buffer still needs a live admitted owner for its actual retention; the probe retains admitted save bytes while decoding. The load peak also conservatively includes input bytes. M3 remains responsible for bounded file acquisition, complete cohort ownership, cancellation/offload and host retirement.

## Behavioral correction and reference rules

`start_transform_job` now rejects a job ID already active under a different producer, before mutation. Completion receipts are globally keyed by job ID; admitting the second job previously created contradictory reservations that could become stuck at completion.

Live inputs/pockets/offers must resolve to one current holder, and their summed quantities cannot overflow or exceed the stack. Duplicate input and same-stuff output lines remain legal; output lines can merge into repeated produced IDs. Completed receipt IDs survive later transfer, merge or consumption. Legacy restock shares retain exact sorted provenance and current original-vendor binding.

Character identity uses 128 Unicode scalars, not the ledger's narrower 64-byte principal reference retention. Inbox and pending-history caps are both 64; recent history is 32. Matching prose in two buffers represents separate legal occurrences. Generated sheet/lore/appearance and private perspective are preserved exactly.

Place restoration retains entry order, exact home IDs and home bindings. Normal duplicate names keep their first entry; homes do not enter name lookup and can share names. No home ID is rehashed. Stale Needle claims and overdue/attempted legacy travel/edit state are preserved where the ordinary next movement/Round pass owns resolution.

## Verification

The final focused checkpoint group passes **26 tests, zero failures, two intentional fixture-generator ignores**. The coordinator's public Character target separately passed all five tests and is included in the full workspace run. New tests cover the three exact supported component fixtures, malformed/missing/duplicate/numeric inputs, aggregate/depth refusal before parsing, UTF-8 and escaped text limits, committed inventory arithmetic, duplicate active job admission, replay after historical item removal, body state, same-name home/index semantics, actual polyline/gait continuation, and ordinary Engine gut formation producing the same single item after restoring the component.

`backbone.json` exercises private live state and reservations; `backbone_completed.json` preserves historical completion lineage after live stock is removed; `backbone_empty.json` covers virgin empty state. These are component fixtures, not saved cities. All M2a1 component JSON fixtures and historical M0/M1/M2a1 evidence remain unchanged.

The full workspace run exited zero: **1896 passed, zero failed, 10 ignored across 34 targets**. [Commands/exits](verification.json), [workspace log](workspace-final.log), [focused log](focused-final.log) and [original/archive hashes](log_archives.json) preserve the exact final checks. [Development archives](development_archives.json) preserve intermediate compile/test diagnostics and the initial rejected allocation calibration without rewriting them. [The source/fixture/runner manifest](source_hashes.json) identifies the tested source; scoped rustfmt avoids baseline formatting changes outside this cut.

## Cost and remaining gates

See [ADMISSION.md](ADMISSION.md) for the concrete BTree/Vec/String/Option/enum and serde Content/error-scratch allocation argument. The actual minimal Character record charges 19,396 bytes, covering a sparse eleven-slot character BTree node bound of 18,872 bytes on this build/target. The real +2,000 generated diagnostic contains 2,520 actors, 2,638 items and 1,992 places; 9,118,806 encoded bytes require a conservative 406,687,186-byte retained cohort, or 813,374,372 shared bytes excluding Running. Per-owner actor/item ceilings are additional limits, not guarantees that the aggregate gate admits every combination.

[The final release record](performance/README.md) contains six runs and 3,600 raw phase samples, independently checked in the [coordinator review](coordinator/review.md). Authored/populated export p99 is 6.321/30.372 ms; decode p99 is 8.238/37.434 ms. At +2,000 every measured phase exceeds the 2 ms host-frame p99 target, including validation and disposal. Debug calibration timings are separate from these release results. Candidate timing validates/transfers indexes already built during decode; it is not a second index reconstruction phase.

Complete-envelope plus Running/retiring coexistence remains unproved. M2a1 maximum ledger export/encode p99 already measured 7.186/11.884 ms, making synchronous host-frame placement unacceptable. M3 needs offload/incremental coordination; finite numeric clock format bounds remain **unaccepted effective-rate CPU limits**.

Proposed next coherent cut: **M2a3 Round/residents/production/household/road-party authority**, binding its saved planner/service/reservation state to these character/item/place records and completing the projection contracts in the owner ledger. Do not begin until M2a2 is reviewed and committed. Complete knowledge/law/weather/animals, conversation/floor, scheduler/Night/speech, Engine/host and manifest composition follow before M2a can be accepted as a complete checkpoint system.
