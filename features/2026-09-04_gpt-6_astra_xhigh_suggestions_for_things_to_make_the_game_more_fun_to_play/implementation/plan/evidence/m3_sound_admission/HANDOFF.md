Status: Owner implemented and verified 2026-09-22; root review/commit pending. No commit made.

## Implemented scope

The enabled installed startup now calls SoundCatalog::from_toml_str_admitted on
its actual captured runtime-editable TOML. It stages that value, then LocalEngine's
EngineSeed and World share the same private Arc-backed immutable rows. Empty
Default stays allocation-free; legacy new/from_toml_str/getters/order/Debug/equality
keep their behavior. Legacy new's intentionally limited ambient validation is
unchanged. Disabled actors need no sound source/catalog.

The constructor first reserves the byte-sized token allocation, then runs the
pinned full grammar with the original whitespace/recursion wrappers and a stack
counting receiver/error flag. It drops tokens before resizing to the derived full
parser/wire/conversion/diagnostic/Arc charge. Final actual Vec/String capacities
replace construction scratch. Static error enums cannot outlive an error lease.
Rows precede the reservation in storage, and the last by-value clone releases it.

## Proof boundary

This is the closed SoundCatalog wire schema on audited x86_64 Linux/GNU,
Rust 1.96.0 and pinned TOML/parser/serde implementations. Direct parser/spanned
dependencies name already-locked types; the recorded feature query confirms the
same TOML default/display/parse/serde/std feature set, no preserve_order/unbounded.
The exact full grammar is preserved, including accepted struct sequences,
multiline strings, Unicode and long numbers. The caller owns/admit source bytes;
installed capture already does. A 4 MiB source admission is not a guarantee that the
parsed role fits under real shared running pressure.

Root review caught malformed short hex escape expansion: decoder output can
exceed encoded source length, so the charge permits 2S output and 11S combined
decoder/key-clone/typed text. Float parsing is the heap-free Rust core dec2flt
path, whose DecimalSeq uses a fixed 768-byte digit array, not serde_json's lexical
bigint path. Dotted-path Vecs are charged before the separate 80-key rejection.
The whole source/event/container/error inventory and 57 pinned file hashes are
documented in README and audit/identity.json.

Structural growth terms prove simultaneous storage. Successful host measurements
count cumulative requested Rust bytes and are separate observations. Error tests
prove refusal/cleanup, without a direct error-branch allocation measurement. The
original 11,052-byte catalog has 14 sounds and four ambients; ordered all-field digest
9e159ccd522dab6d8b6d00694ac348708c5270d960396f1f245485bf10feaf55 matches legacy.
No assets or source identities are substituted with synthetic production inputs.

## Verification and ownership

Eight admission, seven legacy, ten startup and two routing tests passed, as did
one normal production build. Archived records and owner/audit-result.json bind
actual completion, metrics and source identity. The installed catalog requested
586,181 bytes under a 1,458,128-byte peak and retains 9,229 bytes; clones request zero.
The initial host compile failed with E0609 from incorrect EngineConfig field
references in new routing tests. Only that test file changed to EngineSeed.catalog
and World; the 15 passing sim tests retain map A evidence, and the 12 affected
host/routing tests plus build use map B. Production/parser sources are identical
across the exact recorded delta; compiled build identity may still change.
The raw logs remain byte-for-byte unchanged, with matching gzip archives; do not
normalize Cargo's trailing blank-line whitespace. No full workspace rerun is
claimed. The production executable is never launched.

Commands are serial nice 15, default Cargo/compiler/target, -j1 locked/offline,
headless/fake and one test thread. No source changes occurred during owned runs.
No app/window/audio/device/provider/network or secret-file activity, commits,
pushes or subagents. Unrelated dirty paths are preserved. At final handoff the
owner explicitly cedes source/Cargo/executable ownership to root and has no live
owned commands.

## Remaining M3 work

Existing by-value hydration fields can carry the same shared catalog lease
through HydrationAssets, DecodedHydration and the Engine inside HydratedEngine;
no unused fence or incomplete production factory was added. Areas/public mutable
fields, seed/lore composition, prompt compilation, broader EngineConfig::default
and other default catalogs, generated crowds, backend configuration/transports,
production hydration and complete application accounting/adoption remain open.
Caller operations such as emittable_sound_ids also own their result allocations.
The complete admission gate still refuses; no milestone completion is claimed.

Root's next-cut observations, not implemented here: Engine::new builds area
adjacency, replaces salience in pollen-flat mode, and clones/extends FactCatalog
before knowledge seeding. World::default parses marks/facts/salience each time;
ItemCatalog::embedded uses a process-global LazyLock<Arc>. A future grouped
definition factory must route actual World/Engine constructors through provided
admitted assets rather than first allocating defaults and replacing them. It must
not attach one budget's lease to that global LazyLock.
