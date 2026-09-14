# Running scope review (working evidence)

This is a source classification, not an accepted heap measurement. Final owner
capacity measurements must accompany it. The 512 MiB minimum is a trusted caller
contract for retained checkpoint authority; it does not automatically account
for every allocation in an arbitrary application process.

The actual authored/+2,000 probe uses the existing CityPlugin fixture in
src/host_checkpoint/tests.rs. It creates MinimalPlugins, AssetPlugin and concrete
mesh/material/image/audio asset stores, controller/soundscape/actor/city systems,
and a fake local Engine. It does not create a native window or rendering/audio
device. EngineGuard still owns BackendsHandle and its two-worker Tokio runtime.
Fake mode therefore must not be described as having no backend allocation.

| Owner | M2a16 accounting and future obligation |
| --- | --- |
| World and Engine semantic owners | Same-budget Running admission remains live throughout capture and validation. The sixteen decoded category bounds describe the candidate representation; they do not by themselves prove live spare capacities. Final supported-workload inventory must substantiate the trusted allowance. |
| Parsed definitions | Count actual retained World/Engine catalogs, prompt/config data, map and shelters. Distinct roles can contain distinct objects. Deduplicate an Arc only with observed pointer equality. |
| Navigation | World and Engine ordinarily share one Arc. Vermin retains a separate NavData. Account graph capacities and derived distance cache storage separately; current cache occupancy is not its eventual upper bound. |
| Host semantic state | CollisionWorld includes boxes, convex prism headers and nested vertex/plane buffers. Vermin includes colony, rat and leg capacities as well as its navigation. Host publications, readable messages, pending commands and clocks are authoritative at the ordinary HostCaptureSet boundary. |
| Bridge transport | LocalEngine owns command/event endpoints and a publication charge counter. Source limits are 128 commands of at most 24 KiB each, 8,192 event records and 128 MiB publication payload admission. Actual boundary occupancy is different from these maxima. These existing transport counters are not automatically leases in CheckpointBudget. |
| Backend delivery | Backend mailbox limits are 256 callback records, 64 terminals, 64 MiB terminal bytes and 4 MiB stream bytes. The fake host still owns this mailbox. Later arrivals belong to the live generation; they are not silently drained into the saved boundary. M2c/M3 retain responsibility for service adoption and retirement. |
| Session and service artifacts | Engine transcript is an unbounded session presentation artifact. LocalEngine also retains PromptLog and shared fake completion storage; EngineGuard retains runtime/config/session ownership. Live provider clients, tasks, worker stacks, audio files and service handles require their own application admission scope. Candidate compatibility and semantic accepted-work records do not account these allocations. |
| ECS/assets/rendering | The probe contains real ECS entities and asset stores. Mesh buffers, materials, textures, archetype/table spare capacity and renderer/device state are outside this read-only candidate's allocation theorem. M3 must account actual application and retiring ownership before publication. |

Source anchors reviewed: src/smart_actors/local_engine.rs (LocalEngine,
EngineGuard, spawn and pump), src/smart_actors/bridge.rs (transport admission),
crates/cathedral-backends/src/{lib,runtime,mailbox}.rs,
crates/cathedral-sim/src/engine.rs (Engine/transcript), src/controller.rs
(CollisionWorld), and src/city/vermin.rs (Vermin/Colony/Rat/Leg).

Process peak RSS can corroborate a concrete run, but cannot replace this owner
classification or prove a shared-budget bound. The probe's source identity,
counts, actual capacities, logical cohort peaks and RSS must remain separate
reported facts.
