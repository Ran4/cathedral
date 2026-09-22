Status: Implemented and owner verified 2026-09-22; root review/commit pending. Partial M3 prerequisite only.

# Installed shelter definition admission

Enabled installed startup now parses the same embedded `assets/world/shelters.json`
under a source-specific construction reservation and retains the parsed rows'
charge through shared ownership. LocalEngine uses that exact staged Arc in
EngineConfig and World. Disabled actors skip shelter staging, and legacy JSON
loading keeps its existing validation, defaults and authored order.

This admits one real parsed-definition role. Areas, sounds, seed/lore composition,
prompt compilation, default catalogs, generated crowds, configuration/transports
and complete application allocation/adoption remain open. The configured crowd
range and generation path are unchanged; no whole-world allowance is introduced.

## Supported artifact and constructor

`ShelterMap::installed_admitted(&CheckpointBudget)` accepts no caller-provided
JSON. Its only production input is the same build-identified embedded source
previously parsed by LocalEngine. The current artifact is 13,240 bytes, with
28 shelters, 112 polygon vertices and 30 explicit office values. Its SHA-256 is
`721956d1c93a758a3ae0914c7c24b96892c54eacf68bd7d5fe4e16139383cc4e`.
The private source seam exists solely for refusal/error witnesses.

The storage audit is pinned to x86_64 Linux/GNU, Rust 1.96.0,
serde_core 1.0.228 and serde_json 1.0.150 with `float_roundtrip`.
[audit/identity.json](audit/identity.json) hashes the artifact and all audited
parser/container source files. This is a Rust-owned allocation/capacity inventory,
not a bound on shared allocator arenas, retained heap pages or process RSS.

The current JSON numbers also require a specific audit: all 309 numeric tokens
have at most two fractional digits, no exponent notation and significands no
larger than 25,727. `de.rs:625` calls `parse_concise_float`; `algorithm.rs:18`
takes its fast path because the significand is below 2^53 and the exponent is
within f64's -22..22 range. No lexical bigint allocation occurs for this artifact.
The lexical storage counts recompute from source bytes, but **future numeric,
artifact/schema or serde changes still require re-audit**. This is not an
admission claim for arbitrary long-number JSON or an exported generic decoder.

## Construction inventory

A stack-only lexical scan counts bytes N, quoted tokens Q, longest encoded
quoted token L, object openings O and array openings A. Quoted delimiters and
escaped quotes do not count as containers; unfinished quoted tails contribute
to L. Let R=O+A, V=A and F=Q. These deliberately overcount all shelter rows,
polygon vertices and office values, including serde's struct-as-sequence form.
Every multiplication/addition used for admission is checked.

| Allocation | Pre-admitted term |
| --- | --- |
| JSON escape/ignore scratch and growth overlap | 3N |
| Retained id/label text and allocation headers | N + 64R |
| Shelter Vec, including old/new growth overlap | (3R+4) × sizeof(Shelter) |
| Polygon Vecs | (3V+4R) × sizeof([f64;2]) |
| Office Vecs | (3F+8R) × sizeof(Office) |
| Old/new vector allocation headers | 32 × (4R+2) |
| Validation BTreeSet of borrowed IDs | R × (16 + 11×sizeof(&str) + 12×sizeof(usize) + 32) |
| Parser/validation error formatting and overlap | 32L + 4,096 |
| Document/control roots | sizeof(ShelterDocument) + 256 |
| Shared backing and constructor's outer Arc | sizeof(each payload) + two reference-count words + 32, for each Arc |

serde's Vec visitor begins with JSON's zero size hint, then pushes; the pinned
Rust Vec grows geometrically. Three element counts plus minimum capacity cover
simultaneous old/new allocation storage. This is a live-storage bound, not a
theorem about cumulative allocation requests. Every dynamic collection in the
wire shape is a Vec; fixed vertex arrays, enums and numeric defaults allocate
nothing. Missing offices default to an empty Vec.

Validation preserves the original schema, geometry, finite-number, capacity and
duplicate checks. Its BTreeSet borrows strings; the node term permits one full
internal node per row, including the sparse first node and control overhead.
The pinned node contains an aligned 16-byte prefix, eleven keys and up to twelve
child pointers. The 32L term includes escaped diagnostics, duplicate-ID formatting,
formatter growth and malformed quoted tails. No arbitrary serde Value tree or
blanket 64 MiB parser allowance is introduced.

The reservation precedes parsing, validation and both Arc allocations. Parser
and validation scratch disappear before shrinking to actual Vec/string capacities
and shared owner/control storage. Heap-owned parse errors are discarded while
the reservation is live; `ShelterAdmissionError` crossing the boundary is a small
allocation-free enum. No direct allocator measurement of error branches is claimed.

## Ownership and compatibility

ShelterMap's private optional shared backing owns rows before its optional
reservation. The empty Default stays allocation-free. Existing `from_json_str`
keeps its public API; its legacy maps have no admission lease. ShelterMap clones
now share immutable rows without allocation; the final shared owner releases
the charge. PartialEq and Debug use only rows, preserving value/debug behavior.
Checkpoint definition fingerprints serialize `shelters()` rows, so storage and
lease bookkeeping do not enter content identity.

The constructor pays for one outer Arc<ShelterMap>. Cloning that Arc or the map
itself adds no allocation; arbitrary additional caller-created Arc wrappers need
the caller's own admission. Config/World/HydrationWorldAssets already retain
Arc<ShelterMap>. A future real HydrationAssets factory can clone this role; its
lease then naturally remains through DecodedHydration's assets and the constructed
HydratedEngine's EngineConfig/World, including cancellation and retirement. No
unused opaque owner fence or production hydration factory is added here.

LocalEngine's EngineConfig struct update still invokes the broader
EngineConfig::default, including temporary empty catalog/Arc owners. Those
preexisting default/config/world construction allocations remain open; the
embedded shelter constructor's lease does not claim them.

## Verification

All checks finished with exit 0 on unchanged component-input map
`cce7e13ba76ac09ccc9dbf215348767fd17c340d062ad11193aa5132eb269023`.

| Check | Result | Record |
| --- | --- | --- |
| New shelter admission witnesses | 6 passed | [focused](owner/focused-01/summary.json) |
| Existing shelter validation | 1 passed | [legacy](owner/focused-01/sim_shelter_legacy-result.json) |
| Installed startup, including parser/allocation witness | 7 passed | [startup](owner/focused-01/installed_startup-result.json) |
| LocalEngine installed routing | 2 passed | [routing](owner/focused-01/local_engine_startup-result.json) |
| Non-test normal binary build | exit 0; executable not run | [build](owner/production-01/result.json) |

The production parser requested **11,905 cumulative Rust allocation bytes**
under its **239,518-byte construction reservation** and retains **10,945 bytes**.
Both Arc and ShelterMap clones requested zero allocation bytes. The measured
cumulative total is a sufficient observation for this artifact; the structural
formula separately bounds live storage. Error tests prove refusal/cleanup and
lexical handling, without a direct error-branch allocator measurement.

The serialized ordered row SHA-256 is
`ac1acb80bcd16db8d9684f0c86e1bced321576918063b0a8c93ec0328d7ce5b6`, equal
for legacy and admitted constructors. No assets were changed.
The normal binary SHA-256 is `1718a9d925f12adbba4366d7e19c7478fdc036792039ea322005783a5c4b2b80`; it was never run.
No whole-workspace rerun is claimed. Historical evidence retains its old maps.

[audit.py](owner/audit.py) checks real START/PID/END ordering, all command/log/gzip
hashes, source maps, executable identity and the pinned artifact/parser audit.
The executed runners use nice 15, default Cargo/compiler/target, locked/offline
mode, headless/fake inputs and one test thread. Reproduce after obtaining source
ownership with the test runner's four recorded suites and a new run name, then
the build runner. New records go under owner/evidence and cannot overwrite these
archives. No app, window, audio, device, network provider or secret file was run/read.

## Next work

SoundCatalog has private immutable vectors and can use similar shared ownership,
but captured TOML parsing, its intermediate representation, conversion and errors
need their own exact admission audit. AreaMap's public mutable fields need a
separate ownership design. Seed/prompt/default catalog/crowd/configuration and
whole-App accounting gates remain open; complete admission still refuses.
