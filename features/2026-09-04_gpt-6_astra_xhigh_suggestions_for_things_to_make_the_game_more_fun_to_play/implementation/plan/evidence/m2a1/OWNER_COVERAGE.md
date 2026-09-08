# M2a owner coverage — 2026-09-08

Status: In progress. M2a1 component implementation; complete M2a remains pending.

## Covered in M2a1

| Owner | All relevant fields and policy | Gate |
|---|---|---|
| `CommandLedger` | Save all 32 `producers.{issued,high_water,compacted_floor}`, `next_ordinal`, `recent`, `retained`, `protected`. Reject nonempty `pending`/`updates` at export. Every entry saves payload version/digest, command producer/sequence/step, acceptance ordinal, logical receipt time, typed outcome/code/message and two principal references. | Count, encoded/heap bounds, unique IDs/ordinals/roots, valid counters/floors, finite time, known reference kinds. Complete cross-owner root agreement remains an explicit subsequent gate. |
| `OperationKernel` | Save active step/actor/incarnation/resource/adapter, accepted/observed/progress times, exclusive recovery deadline, required/completed work, retry limits/spending, obstruction/plan revisions, obstructed/running; save fixture declaration and completed_units. Derive actor/resource claims in stable record order, checking live indexes on export and duplicate claims before candidate use. | Exact declarations/adapters, present NPC/incarnation, legal ID/step, one actor/resource owner, protected nonterminal receipt, monotonic/recovery/progress constraints, conservative 2 MiB retained kernel limit. |
| `WorldClock` | Save all five current private fields: seconds_per_day, epoch_days, elapsed_origin, scale, night_brightness. | Exact calendar position/bit identity, accepted-origin and supported numeric/rate bounds. |
| Host accepted-time component | Save AcceptedTime elapsed/debt/wall as exact seconds+nanos, fixed step and residual independently, simulation movement residual as exact round-trip f64. | wall = elapsed + ordinary debt; residuals inside their own cadence and accepted interval. No process host origin and no offline/preparation time. |
| Shared manifest component | Schema, content/geometry/behavior digests, generator and procedural hash names/versions/implementation digests, command/reply digest versions. | Exact equality to caller's installed manifest. Implementation digests must cover toolchain/target/build where algorithms rely on them. Runtime manifest generation/resolution is not yet implemented. |
| Wire/admission primitives | Count-capped array/string visitors, active recursion limit, bounded byte input, 3× input allowance for input plus escaped-string scratch, exact-capacity encoded output; attached cohort reservation transfers from DTO to bytes. | One of each running/save/load/retiring cohort, shared 1 GiB; owner working peak reserved before extraction/decode/index validation. |

## Required M2a2+ owner payloads and composition (all pending)

| Owner/source | Remaining authority/disposition work |
|---|---|
| Complete envelope / `Engine` | World identity, exact real asset/geometry/behavior/generator manifest construction/resolution, complete domain payload and host continuation requirements; no permissive optional payload omissions. Complete owner-root, time, reference, count and byte validation. Small full-save fixtures. |
| `World`, `character`, `item`, `inventory`, `offer`, `places` | Full roster/maps/counters, private sheets/state/history/memory, items/quantity/ownership, restock, transform reservations/jobs/completions, offers, movement/travel and round-edit bindings, needle claim, household doors, switches, mutable registry authority. Full-flush events and same-turn guards. |
| `Round` and `round/residents` | Actual per-person phase/schedules/needs, water/food/market queues and service progress, production/stock, household settlement, road parties/presence/cargo, shelters/reflexes, lamp selection/revision, calendar cursors, generated resident dwell/weather/reservations/fairness/support/departure state. Explicit deterministic rederivation only for known geometry/static indexes and scratch publication. |
| `weather`, `dogs` | Timeline/config/seed/forced/residue/strike and processed anchors; animal state and deterministic counters. Rebind immutable shelter/catalog data. |
| `knowledge` including source/mint/pollen | Full fact-key identities/counters, holdings/views/acquisition/provenance, air and stir, learned receipts/occasions/consequences, all sweep/poll cursors. Exact salience/area-manifest derivations. No duplicate graph or omitted later knowledge milestones. |
| `notices`, `custody`, `marks` | All law/notices/delivery/confrontation state, custody holder/station/strain/expiry anchors, mark identity/strength/sweeps/per-day work. Complete historical/current reference rules. |
| `conversation`, `attention`, `floor` | Warm exchanges/attention captures/utterance and witness identity, novelty context plus opaque time-bit seed and expiry separately, floor/presentation obligations and transient microphone disposition. |
| `scheduler`, `night`, `speech_router` | Full pending semantic obligations with lane/fairness/input receipt/history/incarnation; exact held success/error; Night duty/day/status and once-per-day guard; interrupted recording text/purpose/task/attention as unsent drafts; derived World.speech_actions ownership. Distinguish execution IDs/generation from semantic identity. |
| Remaining `Engine` state | Scheduler/config/clock/weather/round, last-clock/movement/cadence/pollen/sound/bell anchors and owed strokes, transcript/committed readable receipts, asset/services disposition. |
| Host body/controller/custody | Accepted physical sample/watermark/sequence/yaw, previous/current fixed body, velocity/view/coyote/jump state, accepted virtual time context, custody strain/latch and separate HOST issued allocator. Actual export/adoption still later. |
| Host mechanisms/vermin/sound/UI | Physical gate progress; deterministic vermin announcements/percept cursors; scheduled cue/cooldown/well/clock/civic edge policies; unsent drafts/read state and learned selections; readable committed presentation. |
| External IO/execution | Fresh generations/endpoints/services; discard old backend/socket/STT/TTS/PCM/microphone handles under the already specified retry/readable/unsent continuation rules. |

The full source-backed `PERSISTENCE_INVENTORY.md` remains authoritative for details.
This table does not replace its hundreds of field policies or M2d behavior tests.
No M2b capture, Engine hydration, M2c load retry or M3 adoption/storage is in M2a1.

## Proposed next coherent cut (not started)

M2a2 should own the private `World` identity/reference backbone and
`CharacterSheet`/`CharacterState`, items, pockets/gut lineage, offers, stock shares,
transform jobs/reservations/completions and travel/round-edit receipt bindings.
It should add actual typed reference registries and count/byte admission while
retaining explicit pending slots for the remaining owner composition, rather
than presenting this partial world as a complete envelope. This supplies stable
actor/item/reservation/time references needed by Round/residents, knowledge/law
and scheduler/speech DTO validators. Those owners then complete M2a before the
M2b capture/hydration gate. The coordinator reviews and commits each coherent
subcut before another begins; no M2a2 source is implemented in this handoff.

M2a2's character/history/item arrays need an aggregate allocation proof and
admission before decoding their first row. Independent per-field limits are
insufficient if their combined worst case exceeds the shared 1 GiB ceiling.
Exercise short encoded payloads with many empty records as well as maximum
strings; account for all parallel DTO/index/scratch copies within each actual
retained cohort. The fixed 16/8 MiB working allowances in M2a1 are specific to
its already bounded ledger/kernel owners, not universal checkpoint estimates.
