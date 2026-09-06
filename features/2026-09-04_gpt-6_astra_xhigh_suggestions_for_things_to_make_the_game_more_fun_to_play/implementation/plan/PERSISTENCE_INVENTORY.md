Status: Initial source-backed inventory (2026-09-05); M0 must reconcile it after knowledge M5 ships.

# Whole-world continuation inventory

M2/M3 must preserve the state that changes future behaviour, not only what is visible. This inventory assigns an initial policy to the actual current engine and to every planned subsystem. The owning module remains responsible for its DTO, validation and continuation tests. [CHECKPOINT_PROTOCOL](CHECKPOINT_PROTOCOL.md) fixes the capture, pending-work, clock and adoption defaults behind these rows.

**S** means save authoritative state. **R** means rebuild from saved state and an exact compatible content manifest. **T** means discard a transient handle/cache while following an explicit recovery rule. Several rows contain both S and R/T components.

## Existing authoritative simulation

| Owner/source | State | Policy and restore obligation |
|---|---|---|
| `world.rs` — `World` | Actor map, roster/insertion order, public/event/spatial counters | S. Preserve stable identities and ordering; begin a new host spatial/runtime generation without accepting old updates |
| `character.rs` — `CharacterSheet` | Authored/generated identity, control, appearance, lore/static context | S or exact manifest-backed reconstruction. Generated sheets and any runtime-mutated sheet values must be reproducible; a catalog lookup by name is insufficient |
| `CharacterState` | Position, yaw, presence and `presence_epoch`, economic class | S. Preserve who is actually present/departed and distinguish later visits by the same stable ID |
| `CharacterState` | Held-item order, pockets, gut entries, urgency/developer urgency | S. Validate quantities/reservations and unique-object lineage; no duplicate digestion on load |
| `CharacterState` | Goal, memories, known people/places | S. These are private and behaviourally significant; never route them through `PublicSnapshot` |
| `CharacterState` | Inbox, pending history, recent history | S, coordinated with scheduler in-flight inputs so unread events are not lost or applied twice |
| `CharacterState` | Movement path, speed, gait, patrol, choke wait | S with route/surface/content validation. Rebuilding a route may not reset an already completed journey or move a body through new geometry |
| `CharacterState` | Needs, travel intent, round edit, statuses, gesture deadline, departure flag | S. Rebase elapsed deadlines and preserve pending edits; cosmetic gesture presentation can be rebuilt from its remaining semantic state |
| `CharacterState` | Resolved daily-round descriptions and vendor list | R only where derived from saved effective rounds/vendor bindings; never regenerate from the original schedule after a saved edit |
| `World.household_doors`, `area_adjacency` | Household door registry and ward adjacency | S dynamic household bindings; R adjacency only from exact saved-compatible area geometry. Generated citizens retain the same door and derived home description |
| `world.rs`, `item.rs` | Items, quantity, kind and metadata | S. Validate catalog/version, uniqueness, positive quantities and ownership |
| `World.offers` | Giver, target, item, quantity and creation/order state | S. Preserve an actual pending offer and its commitments; revalidate on later acceptance normally |
| `inventory.rs` | Restock shares, active transform jobs and reservations, completed job IDs/results | S. A transform completion or historical stock sweep cannot repeat after load |
| `World.needle_claim` | Narrow-crossing owner and expiry/progress | S. Validate owner and remaining duration; do not silently grant two owners by rebuilding the queue |
| `notices.rs` | Live notices, sequence, summons, warrants and delivered/confrontation state | S. M11 will migrate semantics; current expiry/settlement bugs must not be encoded as desired new behaviour |
| `custody.rs` | Records, officer/holders, confinement, station, grip, struggle attempts, release/review anchors | S. Preserve actual custody and controlling authority; host tether is a projection |
| `marks.rs` | Marks, identities, strength, authors/subjects, sweeps and per-day work | S. Rebind catalog via manifest; recreate rendered chalk without minting new marks |
| `dogs.rs` / `World.dogs` | Animals' positions, movement/behaviour state and deterministic counters | S where it changes later motion. Rebuild bodies/animation handles |
| `World.ward_moods` | Settled nightly ward mood | S. Do not ask the model to invent a replacement on load |
| `knowledge/` | Fact identities/sequence, live facts, seeded/held knowledge and views | S. Preserve identity and origin; rebuild indexes only from saved values |
| M2 `Knowledge` / private `Holding` | `next_key`, `next_sequence`, actor holding key/hops/heat-at-learn/learned-on/from/view, air `Drift` heat/hops/via/stir, `last_sweep_game_days` | S. Preserve the actual saved fact-key mapping or explicitly remap every reference. Do not reroll stir, recompute acquisition heat or reset the sweep clock |
| M2 `Fact` social context | `quiet_among`, `craft_ear`, seeded origins and claimed-source data | S as recorded on the fact. Recomputing mint-time context from today's household/trade may change later spread |
| M2 engine/round polling | `Engine.next_player_pollen_game_days`, `Round.next_pollen` and `last_game_days` | S on the calendar basis, including valid Never sentinels. Do not reseed per-person polls or reinterpret old cursors through a new calendar rate |
| M2 derived spatial/salience support | `World.area_adjacency`, salience table, `pollen::WardGrid` process cache | R from the exact accepted content/geometry/behavior manifest. If later packs can change those inputs in-process, key/invalidate caches by that identity rather than keeping the first world's `OnceLock` result. Persist explicit measurement behavior overrides when supported, not a default guessed from current config |
| Knowledge M2 and remaining M3–M5 | Air heat, hop/garble/provenance state, polling anchors, player learning, occasions, systemic consequences | S. M2's status advanced during the review: include `Drift` heat/hops/via/stir and private holdings, then expand this row against the entire accepted feature in M0. M1's small `Knowledge` struct is not the final inventory |
| `World.events` | Undrained domain events | Require and assert a complete event-flush capture boundary. Persist resulting committed receipts not yet consumed by the host; never silently drop an event buffer that unexpectedly remains nonempty |
| `World.spoke_this_turn` | Same-turn seizure guard | T only at a guaranteed completed-turn/transaction boundary; capture must never cut a multi-action application in half |
| World catalogs/maps | Items, facts, marks, sounds, areas, shelters, places and nav | R through an exact content manifest, except genuinely mutable place/catalog state which needs its own S record |
| World switches/config | Sound, view cone, knowledge/marks settings affecting simulation | S for world behaviour; global user device/provider preferences remain separately configured |

## `Round` — state outside `World`

`crates/cathedral-sim/src/round.rs` owns much of the actual city. Serialising the character map while reseeding this service would recreate queues, stock, housing settlements and duties incorrectly.

| Group | Actual fields/examples | Restore obligation |
|---|---|---|
| People and daily work | `people: BTreeMap<ActorId, Townsperson>`, current phase/leg, home/work bindings, next decision, leash, lag, `epoch`, `evening_seed`, `excused` and active errands | S, including private per-person progress. Keep the saved effective schedule, incarnation and own ladder timing |
| Water | Sources, queues, current service, serving/sound deadlines and per-citizen state | S for occupancy/progress; R for immutable source geometry. Do not refill thirst, reorder a queue or duplicate its service sound |
| Markets | Stalls, counters, bindings, market errands, closed visits and stock plans | S for live sessions/reservations/bindings/visit IDs; R for immutable definitions from the exact manifest |
| Production | Plans, start counters, `production_last_game_days`, eligibility samples, stall-watch state | S. Preserve integrated progress and once-per-office/day starts; validate active jobs against inventory reservations |
| Household economy | Reserves, Watch/settlement day markers, poverty streaks and accounting totals | S. Reloading before/after a day boundary must not pay or redistribute twice |
| Road parties | Members, phases, cargo, routes, departure/re-entry state, departure grace and cash totals | S. Stable membership/presence epochs and cargo survive; departed actors do not respawn from the original seed |
| Weather reactions | Shelter intents, lightning reflex deadlines | S with correct elapsed/calendar basis; R static shelter definitions |
| Service refusal | `chalk_refused_until` and similar backoff state | S. Otherwise reload lets actors immediately repeat a refused operation |
| Lamps | Lit state, targets, night day, Belwyn selections and revision | S. R host lights; do not reroll tonight's dark lamp or relight every post |
| Calendar anchors | Processed calendar cursors replacing unsafe `last_office_now` interpretation, `last_game_days`, `seeded` and other once-per-crossing marks | S in their explicit time domain. A scale change cannot reinterpret the past through a new slope; hydration is not a second `Round::seed` |
| Worksites and resolved data | Worksites, resolved trade/production specs, counter groups | R only if exact definitions and stable bindings reproduce them; dynamic changes are S |
| Traces/scratch | `food_log`, `observed_cart_loads`, `departed_this_tick`, `ladder_scratch` | T or explicit flush boundary. An undelivered departure is a domain effect and cannot be dropped with scratch data |

## Engine time, scheduling and external work

| Source | State | Policy |
|---|---|---|
| `clock.rs` — `WorldClock` | Seconds/day, epoch mapping, scale, brightness | S as a logical clock descriptor. Rebind to a new host origin without changing saved calendar time |
| `weather.rs` — `WeatherTimeline` | Timeline seed/config, forced overrides, residue, strike sequence and processed anchors | S for future-affecting values. R derived sample/visual state from the restored timeline |
| `Engine` | `last_clock_now`, `movement_now`, movement accumulator residual, `next_round_tick_at`, weather anchors, `last_player_sound_at` | S/rebase consistently; no duplicate calendar edges or missing fixed-step progress. Convert semantic `NEG_INFINITY` into DTO `Never`, as for knowledge/marks sweep cursors |
| `Engine` | Owed bell strokes and sequence | S for owed semantic/audio events, or explicitly mark presentation already committed; do not ring a new civic event twice |
| `scheduler.rs` | Order, round-robin index, priority handoffs, protected player reactions, delay/backoff/running state | S. Retain obligations and fairness; do not reset the cast to the first actor after every load |
| Scheduler in-flight request | Semantic obligation ID, actor/presence epoch, lane/fairness position, drained inbox, presented/pending history, prompt/resolved input receipt | S semantic input/obligation; T external job. Restore one load-specific retry, including an empty-inbox idle turn; generic failure/idle-requeue helpers are insufficient |
| Scheduler held completion | Finished but unapplied exact success/error completion and its input receipt | S. Hydrate a held result without a provider request; normal current action validation still applies. Do not restore drained inputs to the inbox and then also apply the completion |
| `floor.rs` | Awaited presentation IDs/deadlines, foreground/background pacing, microphone hold | Preserve owed readable presentation/semantic speech state; T old audio acknowledgements and transient recording holds. Rebase legitimate pacing without blocking the city |
| `speech_router.rs` | Streams, parked recording, transcript jobs, utterance timing, input purpose/version, captured audience coverage and backend mapping | S committed text/utterance identity and supported observation history; T live capture/socket/job handles. Interrupted recording becomes available unsent draft/status, never an automatic new `say` or public-chat fallback. M5 extends the format for temporal hearing receipts |
| `night.rs` | Duty ID, owed day, subject/incarnation, queued/submitted/completed/dropped status, input, bedtimes, retry/yield anchors, totals and ambient day guard | S. Current `last_reflected` is stamped at queue time, not completion; restore queues directly without ordinary `enqueue`. Retry only within the saved duty's restored valid window |
| `Engine` social scheduling | Warm exchanges, last player partner, novelty/context counters | S. Preserve novelty's opaque `Memory.visit` bit salt unchanged and rebase `touched_at` separately. Supersede the old “do not save novelty” comment with this behavior-backed policy |
| `Engine` presentation caches | Last snapshot/law/chalk/lamp revisions, dogs-published, ready flag | T/R. Force complete initial publication to the new host without replaying world creation |
| `Engine.transcript` | Session transcript | Preserve the player's durable readable history through an explicit transcript/journal policy; developer omniscient logs may remain session artefacts. Do not conflate the two |
| Provider traits/env | Cognition/STT/TTS/Sight handles, prompt environment, capabilities | R using current services and validated content. No sockets, threads, secrets or old job handles are serialized |

## Host continuation

| Source | State | Policy |
|---|---|---|
| `PlayerController` | Physical position, velocity, yaw/pitch, grounded/coyote/jump state, flight | S with a matching accepted physical sample and command watermark. New input sampling starts from that physical state |
| Host fixed/virtual clock | Fixed-step overstep/residual and origin, virtual-step policy | S residual independently from sim movement residual; bind origin at final adoption. Loading a mid-jump body must not change when its next physics tick occurs |
| M1 accepted-time coordinator | Ordinary unaccepted wall debt already accumulated at capture | S separately from accepted fixed/movement residuals. Initial publication stays at saved accepted time; later budgeted steps service debt. Offline/preparation/old-world elapsed time adds none |
| `PhysicalPosition` / render transforms | Authoritative current/previous physical sample versus interpolation | S physical value/baseline; R interpolated transforms. No interpolation from the previous world's location; do not capture throttled/interpolated `GlobalTransform` as authority |
| `LocalEngine` | Command/completion channels, fake staging, dead/start flags | R with a new runtime generation. Retire old queues; do not reuse old callbacks based only on actor IDs |
| `WorldMirror`, `MovementInbox`, actor entities | Snapshot/motion caches and render bindings | R from complete new publication. Clear old-world entities/queues consistently |
| Custody host state | Tether/grip/reflex/projection plus `strain` and `struggling_reported` | R constraints from law; S strain/latch with the command watermark or move both to sim authority. A queued threshold command must not double escape or reset progress |
| Doors, marks, lamps, weather, dogs/carts | Entities, animation progress and graphics resources | R visuals from saved semantic state; S physical door/operation progress belongs in the sim |
| `src/city/vermin.rs` | Seed/config, `announced_boil_night`, `last_percept_minutes` and any pending semantic swarm announcement | S or move semantic scheduling into sim DTOs. These generate `WorldSound`/NPC cognition; rebuilding visuals cannot reset them. R cosmetic bodies/scattering under a stated policy |
| `src/soundscape.rs` | `ScheduledSounds`, `CueCooldowns`, `WellSoundState`, `ClockSoundState`, `CivicBellState` | Preserve owed committed presentation or explicitly discard already-presented/transient cues; rebase/init clock edge detectors at the saved instant. The first new snapshot must not duplicate civic/well/curfew events |
| Chat/notebook/document UI | Draft text, input purpose, proposition version, selected learned records, interruption state, reading position, unsent evidence bundle | S as player-owned UI state where valid. Restore as drafts; never auto-submit after load, confer new knowledge or erase already committed speech |
| Speech/audio/microphone | Playing clips, buffers, device streams and callback IDs | T. Clear old presentation; restore readable committed speech and start new capture only by player intent |
| User preferences | Audio/input/display/provider settings | Keep existing preference persistence separate. Loading a world must not require its old provider key or overwrite current device preferences |
| Save service | Write queue, temporary files, selected slot and last completion | T IO handles; S slot metadata only after durable publication. A previous valid save remains until the replacement succeeds |

## Planned systems: mandatory additions at their own milestone

| Milestone | State that must join the checkpoint |
|---|---|
| M1 | Logical time origin, command identity/high-water state, semantic receipts and operation IDs |
| M4 | Surface-qualified positions/routes, dynamic topology revision, passage reservations and content/geometry version |
| M5 | Relevant observation receipts, last-seen position/time/source and retained continuity chains |
| M6 | Door/lock/bar/opening state, grants, scope/revocation, key patterns, loans and pending access operations |
| M7 | Placed/contained object location, unique identity/lineage, document version, examinations/copies/custody receipts |
| M8 | Undertakings, step receipts, actor/resource ownership, invitations, attendance, deadlines and interruptions |
| M9 | Confirmed accounts, submissions, admissibility references, findings, amendments and retained evidence links |
| M10 | Player drafts/read state, input purpose/version, interruption state and knowledge-limited presentation continuation |
| M11 | Matters, allegations, authority decisions, registry orders, issued finite mandates, acquired versions/recall tombstones, shared execution identities, independent hold deadlines/defaults, deliveries and dispositions |
| M12 | Assignments, known leads, search/escort/handover phase and ordinary duty ownership |
| M13 | Installed pack/version set, per-world seed markers and authoring compatibility manifest |
| M15–M18 | Fixed incident installation, cast strategy states, actual evidence locations, hearings, disclosure, payments, favours and fulfilment/amendment history |

## In-flight request policy to implement once

At capture, classify each outstanding request as unsubmitted intent, submitted/unfinished work, completed-but-unapplied result, or committed effect awaiting presentation. Save its semantic category and references.

At restore, preserve committed effects, restore exact held completions, and restore one retry obligation for submitted/unfinished work. Regenerate presentation without emitting world speech twice and fence all old external completions. Inboxes drained for a pending prompt are restored only if that prompt's result will not be applied. This policy belongs in shared scheduling/persistence, not in individual quests. The detailed protocol also distinguishes Night Office duties and presentation-only clocks.

## Validation obligations

Reject unsupported versions, invalid counts/coordinates, duplicate ownership, cycles, dangling IDs, impossible reservations, multiple exclusive duty owners and contradictory custody/order references before adoption. Validate content hashes and stable references before placing any restored body into geometry.

Keep a machine-readable field-policy inventory or equivalent review aid as implementation evolves. Every added mutable field must be assigned a policy. Continue real behaviour across capture/restore boundaries to verify those policies; a serialization round trip alone cannot catch a schedule that silently reseeded.
