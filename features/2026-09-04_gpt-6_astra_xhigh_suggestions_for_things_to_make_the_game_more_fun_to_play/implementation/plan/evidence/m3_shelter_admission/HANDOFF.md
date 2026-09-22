Status: Owner implemented/verified 2026-09-22; root review/commit pending. No commit made.

## Implemented role

ShelterMap::installed_admitted parses only the existing embedded shelter artifact,
pre-admits its concrete parser/container/validation/error/Arc inventory, then
retains actual capacities under shared private storage. Startup stages it only
for enabled actors; LocalEngine, EngineConfig and World reuse the same Arc.
Default remains allocation-free and legacy from_json_str keeps its API/validation.
Clone is now cheap immutable sharing; Debug/equality/fingerprints depend on rows.

The lease is stored after rows in shared storage and survives both outer Arc and
ShelterMap clones. One constructor-created outer Arc is counted; arbitrary caller
wrappers need caller admission. Existing HydrationWorldAssets/EngineConfig fields
can carry this same owner through HydrationAssets, DecodedHydration and Engine in
HydratedEngine; no unused opaque fence or partial fake factory was introduced.

LocalEngine still calls broader EngineConfig::default through struct update,
including temporary empty catalog/Arc owners. Those preexisting default/config/
world construction allocations remain open and are outside this shelter lease.

## Exact boundary

The proof is for the embedded 13,240-byte artifact and pinned Rust 1.96.0,
serde_core1.0.228/serde_json1.0.150 on x86_64 Linux/GNU. It includes checked
stack-only lexical counts, JSON scratch, String/Vec capacities and overlap,
borrowed-ID BTree validation, malformed/duplicate diagnostics and both Arc blocks.
The derived constructor reservation is 239,518 bytes; retained cost 10,945 bytes.

Root review caught float_roundtrip's possible bigint fallback. The archived
numeric audit proves the current 309 tokens use the nonallocating fast path:
maximum significand 25,727, at most two fractional digits, no exponent notation. Future source,
numeric/schema or parser edits require re-audit despite lexical counts recomputing.
There is no public admitted caller-source JSON API, generic parser proof, shared
allocator/RSS proof or new runtime platform/crowd/config restriction.

## Verification and source ownership

Six new shelter tests, one legacy shelter test, seven installed startup tests and
two LocalEngine routing tests pass on the exact archived source map. The host
success witness counts cumulative Rust allocations and zero-allocation clones;
it is separate from the peak proof. Error tests verify refusal/cleanup but do not
directly measure error allocations. Ordered rows match the legacy serialized
fingerprint ac1acb80bcd16db8d9684f0c86e1bced321576918063b0a8c93ec0328d7ce5b6.
The normal production build passed; executable never launched. No full workspace
rerun is claimed. README/owner/audit-result.json bind actual metrics and identities.

No source edits occurred during owned tests/builds. Commands were serial, nice 15,
default Cargo/compiler/target, -j1 locked/offline, headless/fake and one test thread.
No window/audio/device/provider/secret-file activity, commit/push or subagent.
Unrelated dirty paths are preserved. At final handoff source/Cargo/executable
ownership is explicitly ceded to root, with no owned live commands.

## Next prerequisite

SoundCatalog has private immutable sounds/ambients vectors and can use similar
shared ownership despite World passing it by value. Captured TOML parsing, AST,
conversion and diagnostics need a separate bounded audit. AreaMap's public mutable
fields need a different ownership decision. Seed/lore composition, prompt
compilation, default catalogs, generated crowds, backend configuration/transports,
the production hydration factory and complete App accounting/adoption remain open.
Complete admission still refuses; no milestone completion is claimed.

Root's next-cut TOML notes: lock pins toml 0.9.12+spec-1.1.0 and parser 1.1.2.
parse_document eagerly builds source.lex().into_vec(), an events Vec with token
capacity, then DeTable; RecursionGuard is 80 unless unbounded. Check unified Cargo
features for preserve_order/indexmap/BTree and unbounded before deriving bounds.
The DeTable::parse error sink is Option (first error); the recoverable Vec path is
unused there. These are investigation leads, not admission proof for this cut.
