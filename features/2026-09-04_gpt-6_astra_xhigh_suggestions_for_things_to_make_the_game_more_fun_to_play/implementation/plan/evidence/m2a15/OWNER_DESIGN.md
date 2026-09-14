# M2a15 owner reconciliation and implemented closed boundary

Coordinator-reviewed design and implementation, 2026-09-09. This is a read-only
component, not complete save/load, hydration, M2c retry, or M3 publication.

## Concrete owners and policies

| Actual owner | Exact continuation or explicit disposition |
|---|---|
| `controller::PlayerController` | S flying, velocity xyz, yaw, pitch, grounded, coyote_remaining, jump_buffer_remaining. Finite f32 bits, including signed zero, are retained. Jump buffer is an already accepted intent, unlike a held keyboard key. |
| `PhysicalPosition` | S previous/current xyz. Both belong to fixed physics. Transform/GlobalTransform/camera matrices rebuild; interpolation reset is M3, never extraction. |
| `ControllerInput`, ButtonInput, accumulated mouse input, cursor | Release/re-sample movement/running/fly_vertical and held device state. No replay of mouse delta, fly toggle, jump edge, submission or click. Tests reapply the same NEW control sample after immediate canonical equality. |
| `CollisionWorld` | Bind actual ordered boxes and convex prism min/max/planes/bounds, not just nav/source identity; streaming fingerprint, no clone or lazy geometry build. Actual CutMarginProfile presence/ordered rectangles, feather flags, ramps and stairs also bind. Exact actual gate barrier kind, geometry and translation bind; observed active must equal the pure saved runtime blocking value. |
| `LiveTime`, Time Virtual/Generic/Fixed | S AcceptedTime elapsed/debt/wall, exact accepted virtual elapsed/delta and last_frame; fixed elapsed/timestep/overstep/delta. Validate actual elapsed agreement and elapsed+overstep relation. Distinct sim movement residual comes from actual engine movement_now. Real clock/process timestamps are T. |
| `LocalEngine` | S completed AcceptedHostBoundary generation/input_watermark/physical_sequence/position/yaw/elapsed, plus actual input_watermark. No extra poll. Execution generation is a fence, not a durable event identity. Engine/services/channels/handles/fake completion staging are R/T; M2c retains exact sim obligations through its own components. |
| `BridgeHandle` | S independent `issued` HOST_PRODUCER sequence; it advances only after successful enqueue, including explicit retries. Do not derive from input_watermark. Retired/poisoned/generation-mismatched endpoints reject capture. |
| `PlayerSpatialState` | S sequence, last_position, last_yaw, last_background_send. The last field has a Never sentinel. Binding at a completed physical boundary is exact; next background timing cannot be invented. |
| `drain_bridge_messages` Local message_seq | Make the actual consumer sequence an explicit resource, preserving existing increment/coalescing behavior; S sequence and runtime fence. Distinct from speech accepted sequence. |
| `PlayerCustodyState` | S strain and struggling_reported, sampled custody (all fields) and notices (ordered). Bind sampled value to Engine.last_law_standing at completed drain, including f64-to-f32 conversion. Latch is set BEFORE try_send; failed enqueue retains it. BrokeFree resets strain BEFORE send; preserve failure behavior. |
| `GateRuntime` (newly reconciled) | S GateSchedule.previous calendar/position, initialized, target position, and all three Motion value/start/target/elapsed/duration records. They currently own actual player collision; not all gate state is sim authority. DynamicBarrier.active derives without advancing; do not call animate_gate_mechanisms for publication. Leaf/bar render transforms rebuild from saved motions. |
| `Vermin` | S installed seed/swarm_percepts, announced_boil_night, last_percept_minutes. Bind actual ordered colonies (name/anchor/radius/all_offices/ordinary+boil rat definitions and count), actual NavData fingerprint, captured installed config. Density can differ from edited config; retain at construction. Rat motion/scatter/mesh are cosmetic R. Empty colonies must stay empty. Stamp retained on failed WorldSound enqueue. |
| `WorldClockState` | S sampled present/day/fraction/office/weekday/brightness/scale/seconds_per_day, bound to engine's current clock publication. Soundscape/vermin/gates consume previous published clock in Update; their edge detectors may legitimately trail the newly drained clock. Never falsely equate these edge samples. |
| `CueCooldowns` | S exact free_at map and last_pruned_at; includes shared bell rope occupancy and is not all cosmetic. No rehash-dependent order: canonical key order after admission. |
| `CivicBellState` | S observed_office and curfew_day. Curfew claims the day/rope before send; no retry after refusal. |
| `ClockSoundState`, `WellSoundState`, `WorkSoundState` | S flour_day, all well end/pause/last_draw/crossed_bucket_day gates, and work map position/active. They govern one-time incidental cues and well mechanism animation. Preserve to avoid first-publication replays. |
| `SoundscapeCue` unread at actual ingest cursor | S CivicBell accepted meaning/order (including years). Other cue kinds are cosmetic event obligations: explicitly discarded, not replayed after load. Do not infer semantic bell from scheduled strokes. |
| `ScheduledSounds`, soundscape asset handles, weather assets, Playing* entities | T: accepted/queued old audio is interrupted, never replayed. |
| `FootstepTracker`, `NpcSoundState`/NpcTimer, `UrbanNatureState`, `CartSoundState`, WeatherAudioState, OccupancyCache | Cosmetic cadence/history/virtualizer state R at saved calendar with no semantic bridge commands. Cart cue events are cosmetic; only bell_bridge_command crosses into semantic sim. |
| `AreaBedGeometry`, descriptors/static emitters/bed geometry | Definition binding/rebuild; no mutable semantic authority. |
| `ChatInputState` | S open, exact Unicode buffer, character cursor. ctrl_down/blink are release/re-sample/reset. Closed ordinary chat has cleared text; no auto-submit on restore. |
| `PlayerJournal`, JournalUiState, JournalEntriesRoot ScrollPosition | S resolved attribution/word rows, ordered standing, open and exact xy scroll if open. Bind rows to Engine.last_journal using real row_from_entry policy; empty initial journal is supported. Closed panel already clears scroll ordinarily. UI child/layout/cache rebuild. |
| `InteractionState` | S selected_item/index, coin_offer_count, dismissed_broadcasts, pending request_id/kind/sent_revision/succeeded, wrapping next_request. Active offer card is a mirror-derived readable projection; preserve its exact resolved current value until ordinary reconcile. Pending entries are command suppression/correlation, not provider requests to replay. |
| `InventoryUiState`/ContextMenu | S open and exact item/source/screen_pos/frozen spit target ID+label. Do not re-resolve the aimed actor. References preserve historical IDs even after departure/consumption; UI entities rebuild. |
| `ChalkHold`, ChalkChoice, ChalkStanding | S selected choice and exact standing projection. Half-finished hold progress/intent is captured; final publication interrupts the hold until fresh deliberate input (no completion from held C). Old mark/chalk focus is recomputed. |
| `SpeechPresentationState` | S generation fence, actual reader position, last_event_seq, ordered pending subtitle receipts and exact visible_since/minimum_seconds/audio-interruption flags. No reconstruction from transcript or TTS maps. Existing 512 pending bound stays unchanged. |
| `SpeechBubble`/SpeechBubbleStack/ECS Text | S actual live event_id, expiry, speaker ID, last-known world anchor and exact plain Text. Current subtitle stores only formatted label; add a bounded structured receipt on ordinary accepted NPC speech to retain original event seq/ID/speaker/label/target/plain words/position/recipient count, rather than split text. Capture bubble lifetime independently because audio can prolong it. |
| unread `PresentSpeech` | S exact current-generation words, identity, attribution, anchor, expected audio and ordering at the speech reader's actual cursor. Body::track_reflex_signals has an independent cursor and cosmetic reflexes; it is not readable consumption. |
| HUD subtitle, player_transcript, offer_outcome, transient TimedMessage | S exact existing readable text and remaining Duration; HUD subtitle and speech front are separate exact owners because capture precedes Present and the HUD may still show its prior sample. Player speech receipt attribution/identity must be retained alongside its ordinary HUD acceptance. Generic toast is overwrite-prone by ordinary policy, but surviving text is preserved. |
| HUD inventory/offer/focus/law/journal strings | Exact sampled readable data or validated derived projections from saved host/mirror owners; no access to omniscient engine transcript. |
| SmartActorRuntime, WorldMirror/MovementInbox, body/hand/dog/cart/mark/lamp/weather render owners | R from accepted complete publication; exact mutable interaction and sampled readable exceptions above. Runtime connection/status/TTS preference state is service execution, not world settings. Microphone voice backend preference stays user-owned. |
| MapState, config menu, AreaDebugState, InspectedActor | Map open is a player overlay choice; preserve. Config menu/display/provider/device preferences and debug inspector are outside world state, close/rebuild without generating input. Inventory/journal/chat are above. |
| ActorFocus/MarkFocus/ChalkPrompt, inventory UI Local caches, journal Local cache, fonts/assets | R projections; clear input edges. No learned proposition/document owner exists today. |
| Drive, perf, session logging, nav overlay | T/debug; do not save scripts, callbacks, screenshots, rolling timers, logs or diagnostics as world commands. |

## Supported closed API

Pure `cathedral_sim::checkpoint::host` owns a strict V1 scalar record and a
closed tagged `RecordV1<T>` row vocabulary. No public unchecked Admitted
constructor, generic Deserialize admission, or host dependencies. Fixed scalar
families are controller/physical/time/boundary/spatial/clock/gates, UI toggles,
custody latch, soundscape gates and consumer sequence fences. Variable records
are typed custody/notice, journal, pending interaction/selection, chalk,
cooldown/work/well, speech/subtitle/bubble/unread receipt and HUD text rows.

`HostCheckpointSource` exposes a Copy scalar observation and an allocation-free
iterator of borrowed closed `RecordRef` values. Each string is borrowed;
each row has an explicit family and identity/order. Nested variable fields
become separately typed rows. This permits streaming measurement BEFORE host
row collection, string clone, sorting, or validation index construction.
`checkpoint_host_cost(&source)`, `export_host_checkpoint(&source,
context, Reservation)` -> `Admitted<HostDtoV1>`; consuming encode and
`HostDtoV1::decode(bytes, context, Reservation)` -> admitted DTO; consuming
`into_candidate(context)` -> `Admitted<HostCandidate>`, read-only typed access.
No adoption/install API. Candidate fields have strict explicit-null decoding,
unknown/duplicate rejection and semantic references/numerics validation.

`HostCheckpointContext` borrows actual Engine (export) or already-admitted
backbone/law/knowledge/continuity/clock components (decode), plus a closed
installed-host-definitions identity derived by adapters from actual owners.
Definition streams cover collision/gate geometry and vermin installed nav,
colony/rat configuration; reading/hashing cannot seed or warm owner caches.
Context construction is borrowing-only; validation follows charge reservation.

The binary's `src/host_checkpoint.rs` exposes observation/export of actual ECS
owners and LocalEngine without mutation. Owner modules expose narrow borrowed
projection helpers for private fields. A `src/host_checkpoint/tests_public.rs`
seam is reserved for coordinator tests. Private test restore helpers only write
covered owner fields into deliberately scrambled prepared owners.

## Reviewed ordinary boundary

Capture after the full ordinary DrainBridge and before later CollectInput.
Require coherent ready/live/generation owners, exact physical sample equality,
and empty raw BridgeInbox. New nonconsequential worker commands after the frozen
input cohort belong to the later old timeline and DO NOT prevent capture.
Record the main-thread consequential issued allocator at the ordinary pump and
compare at the barrier; it is not equal to input_watermark or necessarily ledger
high-water (valid refused-history gaps survive). Preserve unread current
PlayerIntent at forward_player_intents' actual cursor as unsubmitted intentional
work. Recording intent is explicit interrupted metadata, never old audio replay.
Never consume, drop, wait, poll or freeze across frames to manufacture eligibility.

Committed readable obligations emitted by the completed drain remain supported
even before Present: save unread PresentSpeech at the actual reader cursor,
plus existing accepted subtitle/bubble/HUD work. Unread semantic CivicBell cues
are likewise retained at the ingest reader cursor. Explicit readable consumer
tracking must include rejected/deduped rows, not infer unread solely from the
last accepted speech event_seq (overflow currently acks BEFORE sequence update).
Raw bridge publications before drain are an incomplete host consumer boundary
and reject; later ECS consequential submissions make that physical cut stale.
Pending UI references may name consumed items or departed actors; validate them
as historical references rather than requiring every ID in current backbone.
Full category/receipt cross-root linkage remains a later assembly gate. All
new ordinary structured speech receipt storage is bounded before its clones,
and checkpoint row charges include the largest inline enum stride even for
short variants. Singular record families and order are explicitly validated.

## Verification and probes

1. Virgin and meaningful active component round trips; canonical equality of
   privately installed covered owners before any new step; wrong prepared
   collision/nav/config/gate geometry/context is refused.
2. Actual fixed movement control/restored comparison for jump/coyote/buffer,
   nonzero fixed residual and ordinary debt. Real LiveTime/MinimalPlugins next
   accepted frames; equal new input samples after the release boundary.
3. Actual custody meter/reflex/tether: threshold below/above, successful/full
   queue, preserved latch/strain and ordered consequential command identities.
4. Actual gate schedule/motion/barrier: midway closing/opening, threshold next
   collision, no schedule observe/advance invoked merely to capture/publish.
5. Actual vermin announcement before/at repeat deadline, selected empty colony,
   installed changed seed/density and full-queue stamps. Cosmetic rebuild emits
   no extra WorldSound.
6. Actual soundscape ingest/curfew: pending CivicBell, rope overlap, office edge,
   failed enqueue stamp; exact next commands and no old scheduled audio replay.
7. Actual Unicode draft editor and journal open scroll; selection/pending
   interaction/context-menu correlation and frozen target. Fresh input only.
8. Multiple unread+accepted speech lines, ECS bubbles and timed HUD text,
   partially elapsed current minimum and queued full minimum. Private restored
   audio is absent; remaining readable minimum is max(0, minimum-elapsed),
   queued rows receive full minimum. No say, acknowledgement or audio service
   is invoked by codec; M2c owns exact old interruption/retry adoption.
9. Strict malformed/numeric/limit/reference/generation/sequence tests,
   exact source-backed row limits and precharge-before-clone witnesses.
   Historical fixture bytes untouched; add supported host fixtures only.
10. Owner renderer-free timed probe uses ordinary meaningful host paths and
    emits primary physical/boundary/command/readable witnesses; separately
    measures preflight/export/encode/decode/validate/drop. Root owns release
    repetitions. Source/helper/environment digests recorded at command start;
    raw stdout+stderr retained and deterministic gzip archives stored.

Unchanged 128 MiB encoded/expanded, depth 64 and shared 1 GiB caps apply.
Full-cohort lifetimes already fail the naive 1,256,093,444-byte sum before
Running; this component cannot establish full assembly or a host frame budget.

## Implementation refinements after the reviewed inventory

The concrete scalar is `ScalarsV1`. `HostDtoV1` serializes publicly but only a
private closed wire type deserializes, after reservation. `HostCheckpointContext`
borrows either Engine or admitted backbone/law/knowledge/marks/climate/animals
and command-ledger candidates. Candidate component boundaries, player and ledger
logical horizon are checked again at decode and candidate validation. No live
World/Engine is constructed to validate a future saved component.

Definition identity includes actual ordered physical data, installed mark
catalog values and source algorithm bytes. Mandatory installed gate presence and
vermin presence/seed/density bits/swarm flag prevent null continuation removal or
independent configuration override under unchanged hashes. Physical controller
and body must belong to one single ECS owner. Pure gate blocking derivation is
shared with ordinary animation without advancing a motion or emitting a cue.

Ordinary forwarding gives each still-unsubmitted pose-bearing UI intent a fresh
monotonic spatial identity while preserving the action's original frozen pose.
This fixes the confirmed PreUpdate chat -> ordinary final physical sample ->
CollectInput ordering regression. Already accepted commands are never restamped.

The host receipt allocator is separate from checkpoint admission. Original
speech strings reserve their bounded extra storage before cloning; shared Arc
leases survive independent subtitle, bubble and player-caption lifetimes. A
trailing lease releases only after message fields drop. Ordinary provisional
caption replacement, expiry, disconnect and generation changes clear its prior
committed receipt. Current limits and saturation policy are in ADMISSION.md.

Controller validation uses the same unchanged setter/solver constants as the
actual controller; definition identity also streams those six installed f32
values now that their definitions live outside controller.rs. Horizontal bounds, pitch, coyote and jump-buffer bounds follow
those setters; gravity velocity has a conservative accepted-lifetime bound under
the existing supported logical horizon. Quickbar next-index and reader/message
sequence arithmetic cannot overflow, and accepted speech sequence never exceeds
the bridge message sequence, even with no retained speech rows.

Receipt-storage saturation preserves ordinary speech display/audio and its
normal acknowledgement timing. The live player-caption unavailable flag
distinguishes a committed caption with no retained original from provisional
text; NPC subtitle/bubble owners use an explicit missing original. Observation
refuses while missing readable authority survives. Replacement/expiry/disconnect/
generation reset clear the player flag. Capture neither waits nor drains to
remove this component eligibility limitation; full M3 save-anywhere remains a
later gate.
