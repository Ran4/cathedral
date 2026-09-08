# Nested component admission

Status: Implemented and reviewed; scoped M2a2 allocation argument and population measurements. Complete-world coexistence remains pending.

Untrusted input enters only the new public `decode` functions. Public DTOs deliberately do not implement `Deserialize`; their remote wire definitions and extraction constructors are private. The lexical preflight stores counters/flags only, performs no Unicode escape expansion and constructs no values, keys, token list or trees. It requires a LoadCandidate charge for the supplied input length plus 4,096 bytes before scanning. Save preflight similarly requires 4,096 bytes before serialization into the counting sink. Initial errors are bounded static text; no attacker-controlled text is interpolated by the lexer.

For these concrete v1 record shapes, the scan charges:

- 512 bytes for each array/object;
- 64 bytes for each key, text value or numeric/bool/null token;
- twice each encoded byte within a string;
- no more than 64 nested containers, 128 MiB encoded or 128 MiB expanded charge.

Before typed serde decoding or owner cloning, the attached reservation must grow to `4,096 + 4 × expanded + 3 × encoded`. Refusal leaves the live world untouched and releases the failed candidate's charge. The same non-Clone, Send-capable cohort token remains attached through DTO, encoded bytes and candidate retention. Its charge is intentionally not shrunk at phase boundaries in this cut.

## Concrete layout argument

This is not a generic bound for arbitrary Rust `DeserializeOwned` implementations. Only the closed private v1 Character, Inventory and WorldBackbone wire roots use it. Their constructors fail compilation when a new runtime field lacks a remote-record disposition.

The 64-byte scalar allowance covers a String/Vec header (24 bytes on the supported 64-bit target), Vec element geometric growth (at most two headers), and small enum/discriminant/numeric layouts. Owned decoded string capacity is bounded by twice encoded string bytes. Arrays of strings/IDs/points/records use visitors without trusting a hostile size hint. BTree node spare capacity and pointers are covered by key/value allowances and the associated record/container charge; singleton/root nodes also have the enclosing map charge. The largest map values (Character) and inline nullable records (LoreProfile, Movement, TravelIntent, ResidentStatus) are covered by the **sum of their enclosing explicit field/key/container charges**, not by claiming each null separately covers its Rust Option. No `Vec<Option<Movement>>` or arbitrary enum graph is an admitted wire shape. The layout test records the actual supported sizes and checks the minimal Character's charge against its full inline nullable layout, plus the concrete inventory/place array elements.

The recorded 64-bit sizes are Character 1,680, sheet 808, state 872,
LoreProfile 488, Option<Movement> 112, Option<TravelIntent> 128,
Option<ResidentStatus> 112, Option<GeneratedRoutine> 56, Item 80,
TransformJob 144, StockSpec/restock share 56 and PlaceEntry 104 bytes.
The minimal empty-text/null-heavy Character record charges **19,396 bytes**.
A sparse BTree root has eleven `(ActorId, Character)` slots:
`11 × (24 + 1,680) + 128 = 18,872` bytes including a conservative node/edge
header allowance. The test checks that inequality explicitly. Occupied
nonroot nodes have at least five entries, so their per-entry cost is much
smaller. Primitive ID sets and metadata maps use the enclosing 512-byte
container plus their key/value charges to cover singleton roots; string/ID
Vec minimum capacity is covered by the enclosing array, then the per-element
64-byte allowance covers geometric growth. This sparse-root argument is
specific to the declared supported shapes and build/target.

Four expanded allowances cover retained DTO/runtime-shaped records, a covered candidate/derived indexes, validation maps/sets and transient serde buffering/error construction. This includes serde's internally tagged `GeneratedRoutine` Content buffer, even when an unknown nested field is eventually rejected. Direct fields/maps use strict duplicate-key/set visitors. Typed errors occur only after this aggregate reservation. The separate three encoded allowances cover retained input plus serde_json's reusable escaped-string scratch, whose geometric capacity is at most twice the input. Counting encoding allocates exactly the counted output capacity. The running World and other host-retained allocations require their own Running/RetiringGeneration admission; they are not claimed free by this component charge.

Many short or empty records are charged per token/container before parsing, so a small encoded array of hundreds of thousands of empty records can exhaust expanded admission before a decoder allocates it. Deep nesting and long escaped payloads are also rejected before or within their reserved phase. Individual owned prose/plan/metadata values additionally cap at 65,536 UTF-8 bytes; IDs use their separate scalar validity policy.

## Population check and host placement

The first diagnostic deliberately used a provisional 2,048-byte container coefficient. Authored state passed, but actual +2,000 generated state exceeded the 128 MiB expanded cap. That result is retained as calibration evidence; it is not the final formula.

With the concrete 512-byte coefficient, a real Engine with navigation/Round and 2,000 generated citizens had 2,520 total actors, 2,638 item records and 1,992 places. Its component encoded 9,118,806 bytes, expanded charge 94,831,668 bytes, and one cohort conservatively retained 406,687,186 bytes. Simultaneous save/load charges total 813,374,372 bytes **excluding Running**. Complete-envelope plus Running/retiring coexistence has not been proved; the shared 1 GiB gate must refuse if all actual reservations do not fit. This component is not a full-world memory acceptance claim.

[Six reviewed release runs](performance/README.md) confirm these populated byte charges. The separate `alibi_backbone_cost` probe records preflight, full export, encode, decode plus index construction/validation, candidate validation/ownership transfer, and drop. It reports conservative charges, not RSS. Candidate transfer does not rebuild the maps a second time: typed decoding already constructs the BTree and PlaceRegistry indexes. Full export and encoding include a serialization preflight, so their timings must not be presented as clone-only or serializer-only time.

M2a1 already measured maximum ledger export/encode p99 at 7.186/11.884 ms. Synchronous host-frame placement is explicitly unacceptable. M2a2 retains memory-only APIs and measurements; M3 must provide offload or incremental coordination and cancellation/admission across the complete owner cohort before any host budget claim.
