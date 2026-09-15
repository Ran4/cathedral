# M2b dedicated hydration design

Status: implementation and final owner verification complete,2026-09-15;
coordinator release/fixture review and acceptance remain. This is the approved
bounded design, not final acceptance. M2a16 predecessor is
7b50ff0eb69de204f7c3f273a9191449cc69098f.

The public path consumes an admitted complete LoadCandidate:
prepare_hydration(asset upper bytes), then hydrate(asset factory, installed host
definitions, new RuntimeGeneration). Preparation reserves an additional disjoint
lease in that same LoadCandidate slot before the factory can allocate. There is
no public partial component installation, raw unchecked hydration constructor or
Engine/World extraction. The resulting HydratedEngine is quarantined until M2c
prepares external obligations and M3 prepares the complete application bundle.

The actual original WorldSeed is hashed in HydrationAssets::new; immutable
EngineConfig, freshly parsed/compiled PromptEnv and separately named World asset
roles are moved into the assets bundle. InstalledCheckpointDefinitions::from_assets
provides a fresh-process resolver independent of Engine construction. Its manifest
is built by the same implementation as from_engine. Both bind exact actual
prompts, ordered seed, item catalog, world/config navigation and shelters, areas,
sounds, marks, facts, salience, adjacency, configuration and host image. No
mismatched role is substituted by another similar one or a display name.

The complete typed decoder retains its existing owner and cross-owner validation.
It now returns a private ValidatedOwners graph. Ordinary validation disposes that
graph exactly as before; hydration moves the single graph into actual World and
Engine literals. It never makes a second typed graph or uses the raw candidate's
prior proof to authorize new allocations. No World::new/default, Engine::new,
Round::seed, ordinary creation, provider poll/submission, tick, drain or publication
is called. Narrow immutable observations and independently streamed category
hashes expose the newly constructed authority. The hydrated value keeps no raw
checkpoint bytes.

## Construction and field disposition

| Owner | Policy |
| --- | --- |
| Ledger and operations | Move the actual validated CommandLedger and OperationKernel, including their reconstructed indexes. Preserve IDs, allocation counters, replay state and protected roots. |
| Backbone | Move characters and all private fields, roster order, inventory/offer/restock/job maps, PlaceRegistry and its already metered indexes, household doors, round/travel semantic indexes, revisions, time/context copies, Needle claim and spoke-this-turn. Wrap moved household doors in one admitted Arc. |
| World events | Empty only because complete validation requires the existing ordinary event flush. No flush or drain is performed by hydration. |
| World speech_actions | Reconstruct the bounded shared-root index from every retained accepted recording's semantic ID. These roots are neither released nor terminalized. |
| Round | Move the actual validated Round, including residents, markets, production, households, road parties, queues, claims, seeded definition bindings and all processed cursors. |
| Climate | Move actual clock, private weather timeline, sampled weather, processed weather/office cursors, bell queue and sequence. Backbone retains matching World time/sound copies. |
| Knowledge | Move actual Knowledge, saved validated adjacency (one admitted Arc), switches, pollen cadence, door timers, journal/ward heat caches and sent receipts/time. No catalog seeding or global cache warming. |
| Law, marks, animals | Move Notices/Custody, Marks/switches, Dog vector, law/chalk publication caches, dog publication flag and exact shared movement anchor. |
| Social | Move Conversation, warm exchanges and Novelty, including opaque meeting salt bits and timing. Original exact config comes from admitted installed assets. |
| Continuity | Move Floor without interruption normalization, sound anchor, Round cadence, snapshot/lamp publication revisions, startup diagnostics, Ready flag and runtime TTS selection. |
| Scheduler/Night | Move actual decoded private owners, including queues, pending selections, held completions, original request IDs and duty/incarnation/once-per-day state. No enqueue, retry or provider-failure path. |
| Accepted cognition inputs | Retain the complete separately typed M2a14 owner with exact scheduler/Night prompts, semantic identity, lanes and output caps. M2c must join it to the moved scheduler/Night owners before execution. |
| Speech | Retain the complete typed M2a13 interruption/draft/accepted-recording owner. The Engine's transport router is inert/empty; it cannot be polled. Saved floor and speech roots remain pending joint M2c handling. |
| Host | Retain all scalar/record authority, including physical/controller state, original execution boundary, accepted wall debt and both residuals, unread committed presentation and drafts. No host scalar is retagged at hydration. |
| Immutable assets | Move owned config, PromptEnv and distinct World asset roles; share only exact explicitly retained Arcs. Caller-retained Arcs require coordinated admission over their whole lifetime, including later caller-driven cache warming. |
| Services/capabilities | Install inert zero-sized service sentinels without any availability probe, warm, request or device action. Actual service capability binding belongs to M2c/M3. |
| Transcript | Start an empty omniscient session transcript as the declared session-only presentation policy. Existing committed readable lines remain in the retained Host owner and are not spoken to the World again. |
| Identity | Preserve saved WorldIdentity and seed identity. Replace only ephemeral EngineConfig.runtime_generation with a nonzero supplied fence different from the saved host fence. |

All logical and calendar float bits remain in their saved coordinate system.
There is no host clock argument, no preparation/offline elapsed charge, and no
host-origin binding during hydration. M3 must bind the saved accepted instant to
the actual host origin only at final adoption. Saved host generation remains
historical authority; the new Engine generation is separately observable.

## Allocation and disposal contract

The unchanged four-cohort shared ceiling is 1 GiB. Encoded ceilings remain
64/128 MiB and complete cumulative typed expansion remains 128 MiB. Hydration
charges its own complete decode, plus256 KiB structural storage within that
same expansion ceiling. Structural storage covers Engine/World/wrapper roots,
Arc control blocks and the bounded reconstructed speech BTree. The populated
M2a16 bound left866,545 bytes; the additional262,144 bytes leaves604,401 before
any source/layout changes, so the actual final measurement remains a gate.

A separate asset lease reserves the trusted host-supplied bound before the
factory. It includes factory parsing/compilation/generation scratch, retained
config/catalog/nav/area/prompt capacities and any shared allocation surviving its
old owner. Factory scope measurements and source-backed bounds are required;
there is no claim that an arbitrary asset bundle fits a named constant. The owner
probe initially selects64 MiB for its scoped immutable assets and factory. The
same supported nav can eventually allocate a402,644,224-byte full distance cache;
this is not covered by that64 MiB. Public hydrated access cannot warm it, and
caller-owned shared Arcs need their own coordinated admission. M3 owes the active
cache/renderer/service lifetime model before adoption.

Complete validation scratch is explicitly reserved before fresh resolver
metadata. Typed owners are metered before allocation. Raw input stays in the
field-ordered preparation owner until after construction; success disposes raw
bytes before shrinking the primary reservation. Assets precede their subordinate
lease in each error/panic owner and in the final hydrated value. The outer
Admitted owner disposes Engine and retained continuation before releasing the
primary typed charge. Cancellation releases only when those owners are destroyed.

category_digest requires a same-budget SavePayload reservation before staging.
It uses existing actual owner serializers and a streaming hash sink; speech,
Host and accepted cognition inputs serialize from their retained typed owners.
No raw pass-through or copied expected hash supplies the result. An independent
budget is refused before allocation. Keeping separate Save/Load cohorts preserves
existing component serializer admission and includes temporary export copies.

## Verification seams and ownership

The owner owns production/source changes and Cargo until explicit cession. Root
owns tests_hydration_public.rs, independent review, release measurements and
acceptance/commit. tests_hydration_owner.rs exposes only an admitted setup helper
and supplies rich authored/+2,000 all16-category roundtrips and serial timing.
The original Engine/App can be dropped inside the already admitted asset factory
before the fresh resolver runs. Root tests independently inspect exact digests,
lineage/fence, rejected assets/generations, factory noninvocation under pressure,
shared-budget identity and Weak-nav disposal/lease lifetime.

No M2c retry/interruption execution, M2d full continued-input suite, or M3
filesystem/ECS/service adoption is claimed here. The wrapper's quarantine is a
mandatory ownership state, not a silently reduced complete-save scope.

## Factory accounting and navigation audit

The test host factory parses the original WorldSeed and compiles fresh PromptEnv
inside its admitted scope. Populated input repeats only the exact deterministic
ambient-definition generator before hashing the original seed. Every non-nav
definition is deeply cloned within that scope: areas, sounds, items, marks, facts,
salience, adjacency, config and both separately owned config/world shelters.
Inspection of their nested Clone fields finds owned Vec/BTreeMap/BTreeSet/String
or scalar data, with no further shared heap owners. Embedded JSON is static
read-only program data. Navigation is the only inherited catalog/config heap
allocation. MiniJinja additionally retains process/TLS caches described below.

A host-test-only System allocator wrapper counts cumulative requested bytes for
alloc, alloc_zeroed and realloc on the factory thread, without subtracting frees.
Factory work is synchronous and spawns no construction thread; unrelated backend
workers remain part of Running and never construct these assets. For fixture
reader/writer paths the scope also includes source-App destruction. The wrapper
changes no production allocator. The report adds the counted cumulative bytes to
each distinct retained world/config NavData Arc inventory once, plus64 bytes per
outer Arc, then adds a fixed1 MiB shared Prompt/runtime allowance. Independently
parsed graphs count independently; a shared Arc is one
allocation. Even distinct NavData values sharing an internal distance cache are
conservatively overcounted. The sum must fit the already reserved64 MiB lease.

The coordinator's source-hashed [MiniJinja derivation](coordinator/prompt-shared-bound.json)
bounds builtin BTree/function Arcs151,552B, two capped compiler TLS pools plus
roots532,480B, integer cache16,384B, delimiter/callback roots4,096B and remaining
empty TLS/env roots8,192B: total712,704B under the explicit1,048,576B allowance.
These allocations may predate the factory and may survive every PromptEnv in
process-global/TLS owners. Their full inherited bound is included in each
hydrated lease while retained; the caller's continuously retained Running/process
scope covers the persistent cache lifetime after candidate disposal. This is
conservative overlap accounting, not a claim those shared caches are reclaimed.
M3 must preserve that host-thread/process ownership contract.

Navigation inventories are observed before and after capture, validation,
hydration and all16 category digests, with exact no-growth assertions. The source
audit finds no cached_distance/route call in those paths: fingerprints are direct
source-hash reads; PlaceResolver uses node/site/place lookup; supply_pitch and
lamp_ring call the immutable is_walkable bitset; resident ProjectionAnchor::status
calls destination and reads resident_places().shelter_spots directly. Knowledge
centroids and area adjacency follow the existing metered deterministic geometry
derivation/validation, without World creation or navigation cache warming.

HydratedEngine inherits the existing Engine service trait objects' non-Send
property. M3 must produce a Send decoded bundle plus bounded host-thread
construction/service binding, or an equivalently admitted staged design. No
unsafe Send implementation or synchronous frame-budget acceptance is supplied by
this milestone. The current synchronous hydration times remain explicit offload
and scheduling obligations.

## Same-image persisted fixture seam

Ignored host tests m2b_hydration_fixture_write/read use explicit
ALIBI_HYDRATION_FIXTURE=/tmp/... paths. Writers require startup
ALIBI_COMPLETE_WORLD_ID and create_new exact raw output; readers reserve the
fixture's bounded exact file length before allocation/read, then require EOF.
Default workload is the rich active authored ordinary boundary; presence of
ALIBI_HYDRATION_INITIAL selects the initial workload. Both destroy the source App
inside the admitted factory, bind original parsed assets independently, and
compare all16 actual hydrated category digests against fixture slices. Optional
ALIBI_HYDRATION_REPORT writes the detailed record. A reader with
ALIBI_HYDRATION_EXPECT_INCOMPATIBLE requires exact running host image refusal
before any hydration. No manifest rewriting or include_bytes fixture embedding
is used; M2a16 historical executable-bound fixtures remain untouched.
