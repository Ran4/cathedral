Status: implemented and accepted after independent review and frozen verification (2026-09-15).

# M2d deterministic future continuation design

M2d exercises the complete public capture/validate/hydrate/prepare/bind pipeline
and subsequent ordinary Engine polls. It adds no production adoption or Engine
extraction API. The owner test seam is compiled only under cfg(test), inside the
Engine module. The actual Admitted<PreparedContinuation> owns the newly hydrated
Engine throughout every poll, assertion and destruction. try_map moves the
wrapper together with its typed lease; its service and asset leases stay inside
that wrapper. A scoped Running allowance remains until the prepared owner dies.
These small diagnostic fixtures and JSON observations are behavioral evidence,
not a new aggregate heap or frame-time measurement.

The external CPU Host fixture is explicit. It initializes its scalar state and
owned records from the actual decoded Host owner, then changes only execution generation. The
original prepared Host and saved boundary are never mutated. Each later normal
poll advances the fixture host's accepted elapsed/wall time, fixed residual,
current/previous physical sample, spatial sequence, issued high-water and clock
projection. It retains the actual sampled journal/law/chalk publication records,
using `checkpoint::host::resolved_journal_row` for journal wording. Subsequent
polls update those projections from the Engine's actual sampled publications;
other Host records, counters and debt persist. Steps are at most
100 ms; nonzero debt requires a full 100 ms step, for which AcceptedTime::admit
with wall delta 100 ms preserves that debt. There is no wall-clock read, Bevy
controller integration, unread presentation emulation or M3 adoption claim.
Initial physical samples enter through an ordinary SpatialUpdate command.
Poll instants use exact Duration seconds from integer milliseconds, avoiding
floating multiplication/Duration roundtrip disagreements in movement residuals.
The legacy initial control Engine uses generation 0 and this explicit CPU Host
uses fence 1; hydration always chooses saved Host fence + 1. The default tiny
real navigation permits ordinary movement time to advance. Installed-content
fixtures instead use the committed full navigation JSON/binary.

## Comparison contract

Every ordinary checkpoint observation captures and validates the complete
21-field envelope, including all sixteen owner categories. Each owner is then
re-encoded deterministically and compared as bytes, preserving signed-zero
numeric distinctions. No recursive key removal or category exclusion is used.
The sole ordinary state exclusion is the exact JSON pointer
`/host/scalars/boundary/generation`, the replacement execution fence. Stable
world/actor/item/fact/operation identities, every allocation counter, accepted
time, sampled state, calendar cursors, private queues and ledger ownership
remain compared. Full future message vectors are compared in deterministic,
held-result and queued-Night scenarios.

An unfinished scheduler flight changes representation during preparation:
its accepted flight and CognitionInputs move to a load retry with the detached
original Knowledge context. Its actual control Engine remains uninterrupted.
While that obligation is unfinished, separate full admitted observations of
each timeline are prepared into the common production representation; every
resulting owner is compared. Each observation is disposed while retaining its
own leases. The fixture requires exactly one retry on both sides and normalizes
only `/scheduler/continuation/load_retries/0/flight/request_id`, which belongs
to the discarded or replacement external execution. It independently asserts
original method/prompt/token budget, semantic root/epoch preservation through
complete ownership, one accepted replacement request, and separation of newer
inbox arrivals. The newer arrival is installed before capture, after the original
prompt drained its inbox. After the accepted retry, both external services return
Busy; no post-capture owner mutation or scheduler close repairs the timelines.
The one replacement Thinking status is checked against its exact LLM lane and actor;
all other messages agree. After the recorded result settles, direct complete
state and message equality resumes with no scheduler/Knowledge transformation.
No live-provider future prose equality is claimed.

Unfinished Night is a separate conservation case. Preparation retains the one
owed subject/day/incarnation/root and exact accepted input. The replacement
request uses its original method/prompt/token budget. Unlike an uninterrupted
flight, a retry resets `next_attempt_at` from its actual retry submission time
(`night/continuation.rs::submit_load_retry`). At the chosen fixture instants the
original deadline is `7.5 + 60/400` seconds and the retry deadline is
`7.65 + 60/400`. This declared policy delta is asserted explicitly at the exact
pointer `/night/night/night/next_attempt_at/at`; the control expectation is then
adjusted to that proven value and every owner is compared. It is not presented
as equality of the unsaved provider future. The queued/recorded Night case does
require direct full equality, with no pacing adjustment. No version or context
is erased after settlement; ordinary writers choose their own V1 representation.
While the replacement Night request is active, the full state comparison also
permits the proven old/new external IDs at exactly
`/night/night/night/in_flight/request_id` and
`/cognition_inputs/night/request_id`. After settlement those exemptions vanish.

## Concrete fixtures

| Boundary | Capture precondition | Ordinary future witness |
| --- | --- | --- |
| Food transform/residents | Installed e7mil mill owns an active reserved grain job in its eligible Waning office; three generated residents include an actual MovingLocally route and destination | Bounded Round polls complete the grain-to-three-flour transform and displace a generated resident |
| Market queue | Real provisions stall has hungry buyers hgry1/hgry2 in FIFO order and is serving its first buyer | Both purchases commit, preserve exact buyer receipts, and produce the seller's coalesced two-sale percept |
| Road | Brede party is DeparturePending at its installed gate | Next bounded Round poll departs once and increments presence epoch once |
| Warm exchange | Typed player speech has established Sven as current partner | Partner survives the boundary and expires on the original warm deadline |
| Pending offer | Root-owned case captures the original herring offer | Identified acceptance transfers once; another complete restore and duplicate identity replay keep the receipt spent |
| Custody escort | NPC prisoner is InCharge beside a real escort, with an active station intent and path | Bounded ordinary navigation displaces both officer and prisoner, preserving custody/holder/intent identity during escort |
| Weather/bells | Thunderstorm override and future old-slope bell strokes remain after an ordinary scale change | Later override transition and queued strokes occur on both timelines identically |
| Knowledge | A live dynamic claim is held by Sven and not yet held by the player | Ordinary pollen carries the same fact and receipt identity |
| Held cognition | Exact recorded success/error is held behind reading pace; original inbox is drained | Ordinary application occurs once without another request, retaining newer arrivals |
| Unfinished cognition | One exact accepted scheduler prompt owns its semantic root and drained input | One replacement execution reuses exact method/prompt/budget and settles once |
| Queued Night | Snuffing crossing stamped owed duties while cognition returns Busy | Fresh recorded services later submit the same Night request and settle one reflection with full state/event equality |
| Unfinished Night | One accepted Sven reflection owns its exact input and duty; no other queued duty | One replacement request preserves the duty, uses the declared retry pacing, and applies the recorded memory once |
| Accepted Host time | Root-owned case uses real AcceptedTime admission with nonzero debt and fixed residual | 100 ms polls, spatial samples and repeated complete loads preserve distinct residual/debt/command authority |

The resident fixture puts seeded dwell at its deadline before the initial ordinary
poll; production's default 45–150 second dwell otherwise outlasts the observation.
The capture assertion requires an actual MovingLocally destination and walking
body, and the future assertion independently requires physical displacement.
No future wall jump discards the movement work.

The escort witness covers continued movement, not commitment. Existing ordinary
policy uses a 6 m place-intent arrival radius (`crates/cathedral-sim/src/lib.rs`,
`PLACE_ARRIVE_RADIUS_M`), a 4 m station threshold and 1.5 m trailing distance
(`crates/cathedral-sim/src/custody.rs`, `STATION_ARRIVE_RADIUS_M` and
`CUSTODY_ESCORT_CONTACT_M`). In the fixed fixture both timelines later stop with
officer distance 5.9181795344551205 m, prisoner distance 7.418179534455124 m,
custody InCharge and officer intent None. This existing ordinary endpoint-policy
interaction is recorded for later enforcement work; M2d makes no production
custody change and does not label the observed stop as station commitment.

## Verification boundary

The initial component-inputs-v2 map contains 983 inputs and exactly matches the
accepted M2c post-publication map. Owner commands use the existing full enumerator,
explicit offline -j1 Cargo, removed inherited build flags and hidden/fake flags.
Every development/failing command keeps its original /tmp log, command-start
source/environment/helper identities, SHA-256 and exact mtime-zero gzip archive.
Focused owner/root cases, all nine Rust formatting checks and frozen full-workspace
verification pass at one 987-input map. The workspace passes 2,229 tests, zero
failures and 44 intentional ignores across 46 groups; [verification](verification.json)
seals all thirteen commands, including seven failed development attempts.
Coordinator acceptance remains separate. No renderer, device, real-provider, full 20,000-resident stress,
profile-cost rebaseline or cap increase is part of M2d. Historical M0 renderer
and full-stress gaps and M3 offload/adoption/retirement work remain explicit.
