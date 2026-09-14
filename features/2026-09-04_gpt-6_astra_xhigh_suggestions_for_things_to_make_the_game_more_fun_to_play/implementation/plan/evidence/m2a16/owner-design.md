# M2a16 complete-envelope owner design (reviewed evidence)

The final source, measurements and coordinator acceptance are recorded in
[the reviewed handoff](README.md). M2b hydration, M2c backend adoption/interruption and M3 publication,
retirement and complete application admission remain outstanding.

## Retained authority and wire

The closed outer V1 object requires version, profile, lineage identity, manifest,
boundary and sixteen categories: ledger, operations, backbone, Round, climate,
knowledge, law, marks, animals, social, continuity, scheduler, Night, speech,
accepted cognition inputs and host. The inner component V1 representations are
unchanged. Host rows retain the existing stable family/key order, using an
8 MiB bounded borrowed RecordRef index and up to8 MiB stable-sort scratch; repeated records preserve
their consumer order. No entire World is cloned for capture. Ledger and kernel
still use admitted small owned DTO staging, in private subordinate Save leases.

The candidate owns one immutable raw Vec, private category offsets, the exact
manifest and boundary proof. Its public views borrow from that owner. There is
no Clone, Deserialize or detached extraction API. Every component, ledger root
union, publication relationship and definition role is validated against this
same raw buffer before admission. Load contexts borrow the saved backbone,
ledger, clock and other saved owners, never a seeded or later mutable World.
Typed validation owners coexist in this implementation; all are destroyed
before the candidate charge shrinks. M2b must rebind the exact retained manifest
before hydrating; the historical proof alone never authorizes a changed resolver.

Input owns bytes before its Reservation field, including all failure paths.
from_owned checks actual Vec capacity after constructing this field-ordered
owner. The caller must reserve before IO/allocation. copy_from charges both the
borrowed slice length and new allocation before copying; it cannot infer excess
backing capacity behind an arbitrary slice, which remains caller-owned.

## Allocation model

The global limits remain authored64 MiB/populated128 MiB raw,128 MiB complete
expanded representation, and1 GiB across the four exclusive cohorts. Private
subleases add disjoint charges to the existing cohort; dropping the parent
cannot release a surviving child's slot. Expansion accumulates across every
category and index; it is never reset after validation/disposal.

Raw validation reserves actual owned capacity plus up to twice that capacity
for serde JSON escape scratch, independently of typed expansion. It additionally
reserves64 MiB validation/reference/definition scratch. The manifest is fixed
shape: bounded4096/256/128-byte texts, fixed digests/scalars. It receives a16 KiB
complete expansion allowance; malformed metadata has a64 KiB raw field limit.
Installed definition construction streams borrowed parsed objects into hashes;
it allocates only bounded manifest/config text and scalars. The host load wrapper
calls prepare_definition_resolution before constructing that metadata. Public
resolver constructors require the same caller-owned definition allowance.

The private Metered deserializer forces dynamic sequence/map size_hint=Some(0).
It debits at DeserializeSeed::deserialize, before the visitor receives the first
actual value; a terminal None does not reserve a fictitious element. Structs and
tuples remain inline. The parent's actual Seed::Value size includes the largest
inactive enum and Option storage, not the JSON spelling's apparent size.

Sequence allowance: first16*T+128, subsequent6*T+128. It covers std Vec's initial
4/8 element allocation, BoundedVec's explicit initial reserve_exact(8), geometric
old/new buffer overlap, and direct BTreeSet insertion through SeqAccess. A
Vec-to-BTree conversion (such as Engine knowledge door timers) retains input
while inserting output: ordinary large-T Vec first4 plus BTree first11 fits16;
subsequent input/reallocation and sparse tree occupancy fit6. Bounded ledger
Vecs have a separate explicit reconstructed-index charge before candidate().
The operation kernel likewise receives its existing2 MiB allocated kernel bound
before reconstruction of its three indexes while the decoded DTO remains alive.

Map allowance: first11*(K+V)+128, subsequent8*(K+V)+128. Sparse BTree root/split
payload is11 pairs plus node header/12 child pointers; non-root occupancy is at
least5. The extra factor covers audited post-decode map conversions. PlaceRegistry
builds four HashMaps with cloned identity/name strings. Its entry sequence's
16/6 stride covers the by_id/by_name tables; its homes input map's8 stride covers
retained input plus owner_by_home/home_by_owner growth. A valid unique home must
refer to an already-accounted entry; malformed references fail before inserting
an unaccounted home. First-entry sequence slack also covers the small-table
minimum allocations. Text allowances include four times borrowed text length
plus64 bytes before owned conversion; they cover clones and tagged serde Content
buffers. Arc constructors, Vec-to-AreaKey conversion, unique-set/map adapters and
remote/newtype records keep the wrapper through nested decoding. No closed
owner adapter opens an independent serde_json decoder.

Tagged host record and routine adapters can buffer serde Content. Their dynamic
Content maps/sequences are metered before construction; owned string copies and
final inline record storage are covered separately. Representative layout and
corruption witnesses passed in the [final command audit](coordinator/owner-command-audit.json).

RawEnvelope first requires an object and borrows all21 values before parsing
metadata. The linear no-allocation lexical scan caps depth64 and object keys4096
raw bytes. The typed meter bounds identifiers and wrong-type scalar strings before
serde can render hostile text into an error. Each metadata field is independently
bounded before its small typed parse. Diagnostics must remain bounded on failure,
including retained error text after the input owner is disposed.
Known enum variants with malformed unit/tuple/struct payloads can fail inside
serde's underlying VariantAccess before the wrapped visitor runs. Before any
parse, a no-allocation scan finds the largest encoded string token L, including
an unterminated final token, and admits32*L+4096 additional diagnostic scratch.
The encoded token bounds its decoded text; Debug escaping uses at most6 bytes
per input byte. The32 multiplier covers formatter old/new allocation overlap,
retained serde error, and the final512-byte bounded diagnostic, concurrently with
the independent2*raw escape scratch. This allowance is monotonic across categories
and separately reported; it does not count as retained complete expansion. A
huge malformed token can be refused by the shared budget before serde allocates.

## Complete field ownership reconciliation

World's ledger and operations retain their own categories. Backbone owns
characters/roster, inventory/offers/restock/transform jobs and completion, legacy
round/travel semantic receipts, revision/event/spatial sequences, current
time, public sound/view settings, places and household doors, needle_claim and
spoke_this_turn. World.speech_actions is exactly reconstructed from speech's
retained semantic IDs (its existing World-context validator checks equality).
Climate owns current_weather. Knowledge owns knowledge, its enable/no-salience
switches and pollen state. Law owns notices/custody; marks owns marks, enable/kinds
and ward moods; animals owns dogs. Actual area_map/item_catalog/sound_catalog,
nav/shelters, mark_catalog, fact_catalog/salience and area_adjacency are installed
manifest roles. World.events is explicitly required empty at this boundary.

Engine's checkpoint_seed_identity is the retained ordered-definition binding;
world is reconciled above. Scheduler owns scheduler; social owns
conversation/npc_exchanges/novelty, speech owns speech_router, and cognition_inputs
owns exact accepted submissions and held completions. Climate owns live
clock/weather, last_weather_days/sample, last_clock_days, bell_strokes/sequence
and movement_now. Round and Night retain their complete categories. Continuity
owns floor, next_round_tick_at, last_snapshot_revision, last_player_sound_at,
lamp_revision_sent, startup_diagnostics, ready_emitted and selected TTS state.
Knowledge owns next_stage_hop_at, next_player_pollen_game_days, door_shut_until,
last_journal/receipts/at and last_ward_heat. Law owns last_law_standing; marks owns
last_chalk_standing; animals owns dogs_published. The PromptEnv has a
construction-time definition binding.
EngineConfig is role-hashed except lineage (saved outer authority), host image
(separate mandatory role), runtime generation (replacement host authority) and
runtime_dir (host-local external path). Current service capabilities and live cognition,
transcription, TTS and sight handles are external capabilities; semantic accepted
work is retained in the dedicated checkpoint owners, while backend adoption,
joint speech interruption and the session transcript presentation-artifact policy
remain M2c/M3. Capabilities are used by initial Ready/startup availability and
selected-TTS changes; they are not a substitute for saved scheduler/speech work.

Host V1 retains accepted time/debt, H/physical state and exact pending commands,
publications/readable originals/unread cursors, lamp/climate/scene continuation,
vermin state and cooldowns at HostCaptureSet. Asset/ECS materialization and audio
handles are not silently reconstructed or installed by this read-only candidate.

## Compatibility

The build script hashes actual production source bytes, manifests/lock and embedded
inputs (including navigation.bin and ombreval_buildings.json), rustc -vV, target,
profile and effective sim compilation flags. Git HEAD is supplemental evidence
and does not participate in compatibility. A host startup OnceLock separately
hashes the actual /proc/self/exe image with a64 KiB streaming buffer. This binds
the final linked dependency feature set and host implementation. Failure leaves
normal startup available and complete capture unavailable. No simulation IO was
introduced. Exact-image policy may reject debug/release or separately linked
executables even when production source is identical.

Actual installed role hashes retain ordered original WorldSeed and accepted
prompt template/string inputs at construction. World item/mark/fact/salience,
area/nav/shelter/sound/area-adjacency roles and Engine nav/shelters/configuration
are separately bound. Host geometry/barrier/vermin/catalog roles use the existing
actual parsed HostObservation definitions. Independent World/Engine copies are
not silently substituted. RuntimeGeneration and newly allocated lineage are
excluded from definition equality. The saved lineage remains candidate authority.

Fixtures are runtime-read files created only after final executable freeze.
Their bytes are excluded from production hashing and never include_bytes! into
the image. Writers require an explicit initial lineage input before Engine
construction. A later incompatible image must refuse; tests never replace the
saved manifest or identity with today's to manufacture compatibility.

## Boundary and next consumers

World.events must be empty at borrowed export. Engine's final ordinary flush
drains the preexisting queue; speech dispatch changes conversation/floor/resident
interruption, world-event handlers can mint knowledge, and sound handlers adjust
publication/scheduler state, without re-emitting World events. Final publications
update caches. The ordinary rich speech workload witnesses capture after this
path. A nonempty buffer is a refusal, never a save-only drain or extra poll.

Full validation joins all semantic root categories and the exact protected ledger
union, completed scheduler-submission/Novelty handoff, Ready/snapshot/lamp caches,
saved host publications and physical/H/time identity, player/configuration roles,
initial clock/weather configuration, live climate cursors and next accepted frame.
Floor holds and interrupted speech are independent retained owners; no invented
composing==hold invariant is imposed. M2c must reconcile interruption jointly
with retry/adoption, preserving each owner's saved authority.

A maximum accepted frame is100 ms. The live clock may advance at most one game
day across that frame. Seeded Round must match Engine navigation presence and
retain Some(last_office_days); all three retained Round consumer cursors must be
within3 days of the next position. This bounds offices_crossed_days allocation and
production_overlap_minutes loops while retaining negative/history anchors and
ordinary cadence lag. Engine climate's existing exact processed-cursor checks
remain. The World unchecked revision/event counters receive additional full-poll
headroom; checked-refusal or wrapping ledger/conversation exhaustion remains legal.

Specifically, World revision/event counters require2^40 remaining increments.
The ordinary pump admits at most128 backend completions plus128 host commands;
one command/provider application has at most256 receipt steps. Round actor,
plan and catalog domains are each bounded at25,000 and plans at256 legs; the
next accepted100 ms interval has at most two50 ms movement slices and the
retained Round horizon covers at most four calendar days/seven offices per day.
A deliberately loose ceiling of64 increments per actor/step/plan-leg visit,
including a broadcast to every actor for every input step, gives
`64*(256*256*25000 + 2*25000*256 + 4*7*25000 + 25000*256)`
=106,131,200,000, below2^37 and well below the2^40 reservation. Actual mutation
counts are much smaller: production_overlap_minutes' inner office/leg loops
inspect only, plan start/finish perform the touches; restock batches its row
changes; household donor_index advances monotonically and settlement batches a
touch. Nav/area/recipient searches do not increment for each compared pair.
This is full-pump headroom, separate from the component-local notice/mark/Night
bounds and from allocators whose ordinary checked_add/wrapping/refusal remains
safe at exhaustion. Spatial publications retain two extra host sequence slots.

## Scoped Running decomposition

512 MiB is a minimum trusted Running reservation, not a theorem about arbitrary
Engine or ECS heaps. The caller must charge its actual retained authority and
spare capacities, increasing the reservation if necessary. Supported authored/
+2000 workload owner/capacity inventory is a construction/history proof plus
actual read-only capacity observations, not inference from process RSS. M3 must
admit actual ECS/assets/backend and retiring
ownership before publication; process RSS and wire expansion alone do not prove
that scope. Complete capture is synchronous here and does not claim M3 frame
budget acceptance.

The selected fixture is the actual CityPlugin host with520 or2520 characters,
all requested citizens placed, no arbitrary injected capacity, the ordinary
startup boundary or one fixed fake warmup and four committed player lines. It
has at most24 warmup pump updates. The pending seed is released after Hello.
This bounded construction/history is part of the evidence scope; an arbitrarily
long played or programmatically modified Engine must supply its actual larger
Running charge. The following conservative allowances total500 MiB, leaving
12 MiB below the named512 MiB minimum:

| Retained scope | Allowance | Basis |
| --- | ---: | --- |
| World/Engine semantic owners and ordinary unused capacity |176 MiB|Complete closed layout up to128 MiB plus48 MiB construction/history slack, detailed below|
| Actual World/Engine and Vermin navigation |128 MiB|Actual graph/index capacities plus the ordinary reachable destination-cache ceiling, below|
| Other parsed definitions and PromptEnv |32 MiB|Fixed source-bound small catalogs/configuration and compiled prompt programs, below|
| Fake cognition retained history and staged replies |64 MiB|32 prompts at1 MiB and64 replies at400,000 bytes, plus actual Vec/error/record capacity|
| Backend completion mailbox |68 MiB|Existing64 MiB terminal and4 MiB stream admission maxima; fixed channel metadata is covered by host allowance|
| Selected host semantic geometry/readables/transport and transcript |32 MiB|Actual collision/prism/CutMargin/vermin capacities, bounded channels plus observed payload/count capacities; current fixture has no undrained publications|

The128 MiB semantic-layout term includes full CharacterSheet and CharacterState,
all Round/World/Engine saved owners and reconstructed indexes; it is not the raw
wire size. The decoder's4x text,16/6 sequence and11/8 map coefficients already
dominate ordinary newly constructed String/Vec/BTree occupancy and growth.
For this fixture, CharacterState starts gut/inbox/history/daily-round/vendor
vectors empty (`character.rs::from_sheet`); sheet/state clones are both represented.
Seeded world/round collection sizes persist during the short warmup; BTree
removal frees nodes, and dropping a movement/intent Option frees its nested
route rather than retaining an empty route buffer. String replacement drops
old storage. No arbitrary reserve operation is part of this constructor/history.

The extra48 MiB covers storage that a current-length wire cannot observe:
16 KiB per actor (39.375 MiB at2520) for emptied/replaced prose/vector capacities,
plus8.625 MiB for global scratch and short-lived historical high-water storage.
The three prose windows are bounded at64/32/64 entries and begin empty; their
String slots use at most128/64/128 capacities even with growth. The sparse
pocket/gut/vendor/held-item vectors either retain their represented seed rows or
first-allocation capacity; those, inline fields and small per-actor containers
fit the16 KiB unused-capacity margin independently of retained string text.
The whole-cast Round ladder scratch is separately observed by capacity and is
drained between ticks. Round departed/cart tracing buffers cover only the two
fixed road parties in this history. The journal/publication/scheduler and exact
accepted-input allocations remain in their typed owner terms. The session
transcript is separately counted in host inventory, not silently saved as
semantic authority. This construction argument is deliberately narrower than
all combinations of legal component maxima or arbitrary historical capacities.

Navigation requires a separate consumer bound. The current admitted bake has
10,026 nodes, so a theoretical fully warm cache is402,082,704 bytes of f32 rows
per independent graph, before headers. The old source comment of approximately
60 MiB is historical and is not used here. The diagnostic reports that full
theoretical ceiling honestly; two such maxima exceed the named Running minimum.
The sole production `cached_distance_m` caller is `prompt/places.rs` and its
goal is the nearest node of a registered World place. The fixed supported
registries contain491/1992 entries (ordinary startup logs). Even if every entry
has a distinct destination, at most1992*(10026*4+32)=79,950,912 row bytes can be
retained by this workload. Movement changes the origin, not that destination.
The actual World/Engine Arc is deduplicated only on ptr_eq. Vermin owns a separate
graph but calls only grid/is_walkable and never requests cached distances, so
its empty OnceLock array and graph capacities are included without imaginary
warm rows. Complete capture/validation hashes definitions and validates retained
inputs; it never renders a prompt to populate these caches. The report retains
actual warm-row counts, each graph/index capacity and cache-sharing evidence.
The128 MiB allowance leaves over50 MiB for both graph/index stores and headers
above the76.25 MiB destination-row ceiling; final reported capacities check this.

Other definition storage is small compared with its32 MiB allowance. The actual
turn/night/string prompt inputs total40,111 bytes. PromptEnv retains compiled
MiniJinja instructions/owned strings and parsed PromptStrings, not an unbounded
history; even one retained instruction per source byte with128 bytes for its
instruction/line/span metadata, doubling vector storage and several source-text
copies is below16 MiB. [Exact locked-library excerpts](owner-prompt-library.json)
record the instruction/Value representation and initial vector capacities. World
item/mark/fact/salience/area/sound and shared shelter/configuration definitions
come from the fixed source-bound catalogs (roughly80 KiB combined for this
fixture), with no extra fact packs or dynamic template overrides. A further
16 MiB covers their parsed row/lookup allocation and independent World/Engine
role copies. Resolved rounds, homes, resident plans and production tables are
already included in the Round/backbone semantic terms; construction-only seed,
JSON parser and generation scratch is released before the observed boundary.

Host transport inventory reports commands/event queue lengths, actual published
payload charge, independent completion usage, fake prompt/staged capacities,
transcript bytes and Round scratch without draining. Collision includes nested
Box-slice planes/footprints; Vermin includes colony/rat/leg Vec capacities and
its navigation is separately counted. The32 MiB host term covers current
semantic/readable storage, the fixed8192-record event and128-record command
channel allocations, up to3 MiB command payload, and small metadata. It is not
the128 MiB possible publication-payload maximum: the ordinary captured fixture
has completed publication drain, and its fixed fake work cannot populate that
maximum during capture. Mailbox snapshots are independent observations, not an
atomic whole-backend occupancy proof. Live backend implementations, worker/task
stacks, runtime arenas, files/audio buffers, renderer assets, Bevy archetypes,
rebuildable NPC mirrors and retiring generations are explicitly M3 scope; fake
mode still starts a real backend handle and two-worker Tokio runtime. They are
not claimed to consume zero or to be admitted by the table above.

The host observer records definitions, borrowed extraction/count preflight,
encoding, outer parsing, typed decode/index validation, typed disposal, final
source recheck and candidate retention. Input copy and final candidate disposal
are separately timed. Custom adapters build some indexes as part of typed decode;
that coupled phase is reported honestly, not replaced with old component timing.
All cold/tail samples and shared logical byte peaks remain in raw probe output.

## Legacy fixture refresh at the final source

The first full workspace run (`workspace-final-10`) correctly refused the old
host component fixtures: the existing M2a15 `installed_catalogs` role hashes all
bytes of `src/city/mod.rs`, including the newly appended test-only capacity
observer. Its other source inputs were unchanged. The original run, source map,
fixture bytes and exact copied executable remain preserved; no old saved
identity was normalized or accepted against changed definitions.

The existing `m2a15_write_component_fixtures` writer ran in three fresh processes
against that copied executable, creating initial and active files in fresh `/tmp`
directories. All three writers produced identical bytes, and a full JSON value
diff found only `/scalars/definitions/installed_catalogs` changed in each file.
The exact generated originals now replace the current supported host component
fixtures. [Refresh evidence](host-component-fixture-refresh/record.json) records
both old/current hashes, every changed JSON value, helper identity and the two
fixture-only source-map changes. No Rust source changed after the preserved
format/library checks. Complete release-image fixtures remain a separate final
release-binary workflow, with explicit other-image refusal.
