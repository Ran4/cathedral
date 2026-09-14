# M2a15 host component admission

This is one read-only component. The unchanged limits are 128 MiB encoded,
128 MiB expanded, depth 64 and a shared 1 GiB across admitted cohorts. No
Running, retiring generation, complete save or frame-budget acceptance follows
from a successful host-component capture.

## Before allocation

`HostObservation` borrows the actual ECS World and LocalEngine. It does not
create QueryState, collect entities, clone collision geometry, warm caches,
consume Messages or run a system/poll. Its entity scans and actual collision,
gate, CutMargin and vermin definition hashing are included in the timed source
passes. Context construction borrows Engine or saved component candidates; it
does not instantiate a replacement Engine/World or allocate a player ID.

`checkpoint_host_cost` serializes the closed borrowed view into counting sinks.
The existing aggregate lexical model charges encoded bytes, conservative string
and structure expansion, simultaneous codec copies and container overhead. Host
adds 4,096 bytes per raw JSON object/array opener and a fixed 4 MiB validation
allowance. Charging every container is conservative and covers the largest
inline `RecordV1<String>` stride even when every row is a short variant. The
wire unit test checks that stride plus row overhead fits this allowance.

Export requires a SavePayload reservation before measurement and grows it to
the measured peak before any row/string collection, sorting or index creation.
The encoded writer has exact premeasured capacity and refuses growth. Because
the public borrowed-source trait can use interior mutation, export rechecks the
actual raw lexical and row costs against the reservation before typed decode.
The coordinator's adversary separately expands bytes and substitutes 10,000
small rows for a larger-byte draft; neither escapes admission.

Decode first requires LoadCandidate ownership of the entire original input plus
4 KiB and rejects the encoded cap before scanning. Aggregate lexical/depth and
host container costs are reserved before the private wire Deserialize. Raw
whitespace remains charged. Candidate validation retains the same lease;
consuming encode or conversion never exposes an unchecked admission constructor.
Errors and final drops return their reservation after owned data drops.

## Validation scratch and borrowed authority

Validation allocates bounded canonical-row indexes only after admission. Context
publication comparison serializes the already charged candidate row and streams
expected context bytes against it; a tiny hostile candidate cannot cause an
unbounded expected-row serialization. Resolved journal formatting checks total
borrowed source text before clones/joins. The fixed 4 MiB allowance covers its
sequential formatter scratch and existing command-ledger validation indexes.
Saved context retains and checks each supplied component's boundary/player and
the supplied ledger's logical horizon on decode and candidate conversion.

All objects reject unknown/missing/duplicate fields. Explicit null is mandatory;
it is not an optional missing field. Unit spellings, mixed unit variants,
Duration and `Never` have closed host wire adapters, without changing historical
component wire. Scalar validation binds exact clocks/body, issued allocator,
physical sequence, message/read cursors and installed definitions/presence.
Historical pending item/actor references are not required to still exist.

## Extra ordinary readable receipt storage

Original speech receipts add at most 8 MiB per SpeechPresentationState, separate
from checkpoint leases. Each receipt's combined original String bytes are at
most 16 KiB. An atomic reservation for String lengths, wrapper and Arc header is
made before cloning. Subtitle, ECS bubble and player caption share the receipt;
the last owner releases it. Rust field order ensures the message storage drops
before its trailing ReceiptLease returns capacity. Queue length alone does not
bound these separate lifetimes. The existing 512 pending-presentation bound is
unchanged. Receipt retention refusal preserves ordinary display, audio expectation and
acknowledgement timing. Missing NPC originals remain explicit None; a separate
unavailable-committed-player-caption flag distinguishes a missing original from
a provisional caption. Capture refuses while such readable authority survives,
without waiting, draining, acknowledging or reconstructing it. Replacement,
ordinary expiry, disconnect and generation sync clear the player flag. The
existing queue-overflow disposition is unchanged. This temporary component
eligibility limitation is not full M3 save-anywhere acceptance or a proof of
aggregate Running/retiring resource admission.

Committed caption receipts clear on ordinary provisional replacement, expiry,
disconnect and generation sync. Captured original speech, bubble text/anchor/
expiry and subtitle progress are separate typed owners. Shared originals compare
position bits as well as identity, words and attribution. The exact ordinary
player delivery caption and reading minimum are checked against each original.

## Measurement scope

The owner probe reports six fractional-microsecond phase arrays, encoded and
expanded costs, the peak simultaneous SavePayload plus LoadCandidate reservation,
actual city geometry/entity/actor counts and concrete readable row counts.
Repeated samples must produce identical bytes and unchanged revision/event/H.
Raw command logs and JSON remain in `/tmp`; deterministic gzip and SHA256
sidecars are retained here. Command-start source scope remains v2, including
`default_config.ron`; no new runtime input outside that scope was introduced.

The authored development smoke used actual CityPlugin geometry: 751 boxes,
1,153 convex prisms, two gate barriers, installed CutMargin, eight vermin colonies
and 150 rats. Its 520-character workload retained a subtitle, bubble, player
receipt, four unread speech rows and an unread intentional chat submission.
Release repetitions and RSS are independently owned by the coordinator.

Earlier naive backbone plus Round Save+Load is already 1,256,093,444 bytes before
Running. This leg neither raises the cap nor resolves whole-envelope lifetimes,
hydration, M2c interruption/retry adoption, inactive publication or retirement.

## Persisted fixture test storage — 2026-09-14

The test-only ExpectedFixture reads the file length, reserves a Running cohort
for its raw bytes plus initial allowance, then allocates and reads exactly that
length. A trailing reservation outlives the byte vector. This charge is fixture
storage only, not admission of its live CityPlugin World. The same budget then
owns LoadCandidate decode and an explicit SavePayload reservation while the
borrowed decoded DTO is serialized; output drops before that Save lease. After
candidate validation, an admitted fresh Save DTO covers one reused output buffer
for the detached-generation fresh projection and unchanged candidate projection.
Every comparison uses exact canonical bytes, not float PartialEq. The checked-in
payloads and historical fixtures are never rewritten by ordinary tests.
