# M5 bounded observation foundation

Status: owner implementation; independent coordinator acceptance pending.

This slice adds a pure sampled visual-presence receipt API and improves actual
player targeting around closed gates. It does not complete the full M5 plan.

## Source reconciliation

`Sight` remains an unused permissive trait stub. Existing simulation sight,
hearing, prompt, witness, and knowledge consumers keep their current semantics.
Movement and facing do not reliably advance `World::world_revision`; a revision
alone therefore cannot certify that a sampled view remains current. Domain events
have no capture-time coverage interval. Associating a receipt with the latest
still-owed event does not certify witnessing that event's act.

`ObservationSnapshot::capture_latest` accepts at most 32 sorted, unique explicit
observer IDs. Its fixed inline receipts retain the original event sequence,
runtime generation, logical sample time, world/spatial/event revisions, immutable
navigation fingerprint, geometry revision and fingerprint, view cone, exact
observer/subject poses and presence epochs, and sampled recognition. IDs and
labels have finite inline byte limits. It does not scan the whole cast, allocate
heap storage, mutate the world, create knowledge, or consume an event.

Fresh-use validation requires the same supplied generation and exact logical
sample time plus those world and actor dependencies. A changed sample refuses;
the old owned receipt and its description remain unchanged. Only a supported
visible sample exposes a description, and an unknown actor remains a stranger.
This is sampled presence, not act detail, event-time evidence, continuous
coverage, a serialized archive, or a persisted protected root.

## Geometry and overload policy

The pure provider is explicitly named `complete_fixture`: its caller asserts
complete opaque-box geometry within a finite coverage box, with at most 128
occluders. Absent coverage is `Unavailable`, never clear sight. It currently
supports the street walking height only. Navigation is an identity dependency,
not a visual occlusion substitute. Production structural/portal coverage remains
a prerequisite for gameplay receipt adoption.

Actual host targeting retains the existing 20 m actor and 4 m item focus bands,
adds active controller `DynamicBarrier` occlusion, and replaces two temporary
candidate vectors with fixed arrays. Admission limits are 20,000 queried actors,
64 nearby candidates, 16,384 static solids, 65,536 static prism planes, and 32
dynamic barriers. Count checks precede bounded scans and actor-ID clones. Invalid
geometry, duplicates, or exceeded limits clear focus and the derived clue rather
than retaining stale interaction authority. The existing collision traversal is
still a bounded scan, not a measured full-frame performance claim.

The player clue, “Closed gate blocks the view”, depends only on the nearest
physical gate obstruction, never the existence, identity, or inventory of an
actor behind it. A nearer static obstruction suppresses the clue. Ordinary
conversation attention and item controls consume the resulting focus. Only
focus-hint rendering moves from early mirror reconciliation to immediately
after `UpdateFocus`, so the displayed hint uses this frame's focus. Inventory and
offer reconciliation remain in their original phase.

## Verification scope

Pure tests cover visible/anonymous support, one/two walls, absent coverage,
turning away, another floor, actor motion without a revision bump, navigation and
geometry changes, generation/time/presence changes, capacity refusal, event
drain refusal, and immutable old receipts. Minimal no-window host tests exercise
actual focus plus HUD systems, gate opening/closure, no hidden-actor clue leak,
nearer static walls, overload clearing, and existing targeting/interaction
behavior. No renderer, provider, audio device, or GPU proof is claimed.

Remaining M5 gates include structural geometry and room/portal acoustics,
migration of every perception consumer, delayed-transcription event-time
audiences, temporal observation coverage, shared durable observation identities,
archive/root admission and persistence, and full spatial/frame acceptance.
