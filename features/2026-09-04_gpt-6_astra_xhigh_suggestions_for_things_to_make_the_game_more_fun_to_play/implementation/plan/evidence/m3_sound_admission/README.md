Status: Implemented and owner verified 2026-09-22; root review/commit pending. Partial M3 prerequisite only.

# Captured sound catalog admission

Enabled installed startup parses its captured `sounds/catalog.toml` under an
admitted construction lease, then retains the actual immutable catalog capacities.
LocalEngine's EngineSeed and World share the same private backing. The
source remains runtime-editable captured TOML, not an embedded substitute.
Disabled actors bypass this role. The complete M3 gate continues to refuse.

## Supported scope

The proof is pinned to x86_64 Linux/GNU, Rust 1.96.0, serde_core 1.0.228,
toml 0.9.12+spec-1.1.0, toml_parser 1.1.2+spec-1.1.0 and serde_spanned 1.1.1.
The resolved TOML features are default/display/parse/serde/std, with neither
preserve_order nor unbounded. The direct parser/spanned dependencies name existing
locked types; they do not change the TOML feature set. Source hashes and the
actual feature command are archived alongside the verification records.

The production capture envelope remains 4 MiB per file, 32 MiB total and 4,096
retained sources (seven fixed plus 4,089 characters); discovery separately allows
16,384 examined entries and 256 directories. That source cap does not guarantee parsed admission: the
derived charge can conservatively refuse under the real shared 1 GiB budget and
current running pressure. No new TOML grammar, field, row, crowd or runtime
platform restriction is added. `from_toml_str_admitted` accepts only this closed
catalog wire schema; it is not a generic Deserialize admission API. Borrowed
source storage remains the caller's responsibility; production capture pays it.
This is a Rust-owned allocation/capacity bound, not shared allocator arenas,
retained heap pages, stack memory or process RSS.

## Counted preflight

Before `Source::lex().into_vec()`, reserve `(3N+4)*sizeof(Token)+64`, where N is
the source byte length. The lexer initially reserves N slots, not its token
count; the EOF token can cause doubling. The formula includes old/new buffers,
the minimum four-element capacity and two block headers. The lexer iterator and
parser itself are allocation-free with a counting receiver and error flag.

The receiver is the parser's public `FnMut(Event)` implementation, wrapped in the
same `ValidateWhitespace` and `RecursionGuard(80)` as the TOML crate. Syntax errors
refuse before AST construction, just as the original constructor refuses them.
This uses the full pinned grammar, including recovery behavior, rather than an
ad-hoc syntax recognizer. Checked stack counters record:

- T: tokens including EOF; E: all emitted events.
- K: simple-key events; V: scalar events.
- A: array or array-table opens; C: A plus inline-table opens.
- S: sum of encoded spans for simple-key and scalar events.

The temporary token Vec dies before resizing the lease for the unchanged
`toml::from_str::<CatalogFile>` parser. Thus recovery event expansion is counted
exactly rather than guessed. Explicit dotted paths are checked against 80 only
after their Vec is constructed (`de/parser/key.rs`); their temporary storage is
charged from K. The array guard alone is not claimed to bound all table depth.

## Full construction inventory

Let Q=`sizeof(Spanned<Cow<str>>)` and W=`sizeof(Spanned<DeValue>)`. All addition
and multiplication used in the charge is checked.

| Owner or temporary | Pre-admitted term |
| --- | --- |
| Token buffers | `(3N+4)*sizeof(Token)+64` |
| Event buffers, including initial T even when E<T | `(T+3E+4)*sizeof(Event)+64` |
| BTree AST | `K*(16+11*(Q+W)+12*sizeof(usize)+32)` |
| AST array buffers | `(3*(V+C)+4A)*W+64A` |
| Active dotted-path buffers | `7K*Q+64K` |
| Decoder text, Cow key clones and typed string copies | `11S+8*(K+V)` |
| String allocation headers | `64*(K+V)+64K+32V` |
| Both wire Vecs and both result Vecs | `(3C+4)*(sizeof(SoundRow)+sizeof(AmbientRow)+sizeof(Sound)+sizeof(AmbientSound))+256` |
| Source-bearing and static/dynamic diagnostics | `N+32S+8*1024` |
| Shared catalog root | `sizeof(SoundStorage)+2*sizeof(usize)+32` |

The pinned BTree node has an aligned 16-byte prefix, eleven key/value slots and
up to twelve child edges. One full internal node per key overcounts sparse nodes.
An AST array push needs a scalar/container event; nonempty paths consume keys,
so their aggregate three-count growth plus four-slot minimum is at most 7K.
Every successfully typed row requires a table/sequence container, including
serde's struct-as-sequence representation; blank-line events cannot create rows.
Both wire/result allocations are counted even if collect reuses storage. The
three-count Vec terms prove simultaneous old/new storage, not cumulative requests.

Malformed short `\x`/`\u` escapes can decode two bytes into the three-byte UTF-8
replacement character. Decoder output is therefore bounded by 2S even on error,
with 6S growth overlap. Key descent/header logic contributes at most another 2S
retained plus 2S transient clone. Valid borrowed strings copied during typed
deserialization add S: together 11S. Per-string minimum capacity and headers are
separate. Parser errors prevent malformed decoded strings reaching typed rows.

The first TOML error can own one `Arc<str>` copy of N source bytes. Errors never
cross admission as owned strings and their Display implementation is not called.
For fixed decoder diagnostics, inspected descriptions are below 48 bytes, the
largest expected-escape list has ten one-character entries, other expected lists
have at most four short descriptions, and rendered literals add bounded escaping.
A decoded fixed message is below 1 KiB. The eight fixed 1 KiB allowances cover
its old/new buffers (three), current fixed-type ParseError formatting (one), the
closed schema's under-256-byte known-field text through growth/copy (one), context
Vec control (one), source/error/string block headers (one), and fixed datetime/
conversion formatting (one). Dynamic serde payload Debug escaping is at most 6S;
formatter growth and `Error::custom`'s overlapping `to_string` copies, context
keys and catalog validation IDs fit separately inside 32S. Validation scans for
duplicate IDs without an auxiliary collection.

Arbitrary supported float tokens use Rust core `num/imp/dec2flt`, including its
slow path. `DecimalSeq` holds `[u8;768]`, and the slow path's other buffers are
fixed arrays/scalars. It allocates no heap and needs no embedded numeric shortcut.
The numeric/parser/container files are hashed in the audit identity record.

## Retention and compatibility

After parser, wire conversion and validation scratch disappear, actual capacities
of the two result Vecs and every retained String, their headers and one Arc block
replace the peak charge. Retained<=peak is checked before shrinking and before
the Arc allocation. Static `Admission`/`InvalidDefinition` errors replace temporary
owned errors while the lease is still live.

SoundStorage declares rows before its optional reservation. Clones share rows
without allocation; the final shared owner releases the charge after row disposal.
Legacy `new`, `from_toml_str`, `empty`, getters and order remain compatible, with
no lease for legacy storage. Debug and PartialEq present only rows. In particular,
legacy `new` still only checks duplicate ambient IDs; it does not gain unrelated
validation of public AmbientSound fields. Empty Default remains allocation-free.

The installed 11,052-byte catalog contains 14 sounds and four ambients. The
ordered serialization of every public row field has SHA-256
`9e159ccd522dab6d8b6d00694ac348708c5270d960396f1f245485bf10feaf55`, equal
between legacy and admitted constructors. No sound asset bytes changed.

The existing by-value EngineSeed/World/HydrationWorldAssets fields naturally
carry the lease through future HydrationAssets, DecodedHydration and HydratedEngine.
No unused opaque fence or partial production hydration factory is added. Later
caller operations such as `emittable_sound_ids` allocate their own result and
remain the caller's admission responsibility. Broader EngineConfig::default,
areas, seed/lore, prompts, default catalogs, crowds, backend configuration and
transports, whole-App accounting/adoption and complete save/load remain open.

## Verification

All 27 focused tests passed: eight new admission witnesses, seven legacy sound
tests, ten installed startup tests and two LocalEngine routing tests. The normal
production binary build passed; the executable was never run. No whole-workspace
rerun is claimed. Commands were serial, nice 15, default Cargo/compiler/target,
locked/offline, headless/fake and one test thread.

The actual installed catalog requested **586,181 cumulative Rust allocation
bytes**, under its **1,458,128-byte construction reservation**, and retains
**9,229 bytes**. Its clone requested zero bytes.

| Synthetic 4 MiB source | Cumulative requested | Construction reservation | Retained |
| --- | ---: | ---: | ---: |
| comment | 201,326,728 | 306,194,800 | 184 |
| string | 205,523,997 | 486,560,074 | 4,194,717 |

These successful measured shapes are observations; the source/event-derived
formula supplies the broader structural proof, including error paths. Error
witnesses verify refusal and lease cleanup, without a direct error-allocation
measurement.

The 15 sim tests passed on map `2093b4ad924337ddc1c7c7d4c33d1297fc3b9f64ceb46f7e98c75aaa3759a8c0`. The first host compile
then failed with E0609 because new tests incorrectly referenced an EngineConfig
field. Only `src/smart_actors/local_engine/startup_tests.rs` changed to use the
actual EngineSeed/World path; all production and sim/parser sources were unchanged.
The generated build identity can still change. The 12 affected host/routing tests
and normal build passed on map `1062db045e4d196ae9098df4c5b4c11e0fe38bdbf0a9b675511d2e870eb62361`. No claim is made
that all 27 tests ran on one final map. [Exact delta](owner/source-delta.json) binds
both source maps, and the original failed compile remains archived. The binary SHA-256 is
`8f5415b9880d864f30fa78ebfaaa594f629d1e74ab1af34e63595379a635e3f2`. [Initial records](owner/focused-01/summary.json), [host rerun](owner/host-02/summary.json),
[normal build](owner/production-01/result.json), [feature query](owner/features-01/start.json)
and [source audit](audit/identity.json) retain actual commands, START/PID/END,
identities, raw logs and byte-identical gzip archives. [audit.py](owner/audit.py)
checks these against current sources; its observed result is saved separately.
The runners reproduce checks with a fresh run name after source ownership is ceded.
No application/window/audio/device/provider/network or secret-file access occurred.

The host counter records cumulative Rust allocation requests, separate from the
structural live-storage proof. Tests cover full TOML encodings, malformed escapes,
deep paths, long numeric tokens, pressure at both stages, 4 MiB comment/string
inputs, immutable source/copy lifetimes, last-owner retention, disabled bypass and
the actual captured runtime-edited catalog after original source files disappear.
