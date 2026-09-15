Status: implemented and frozen-source owner verification complete; coordinator release acceptance pending (2026-09-15).

# M2c joint pending-work preparation

The public transition is `Admitted<HydratedEngine>::prepare_continuation` (with
an observer variant). It consumes M2b's quarantined Engine, exact cognition
sidecars, speech candidate and retained Host candidate. The result is admitted
`PreparedContinuation`: no Engine/World borrow, extraction, mutation or poll is
public. `capture`, category digests, scalar reports, interrupted input and root
observations expose reviewable state while keeping execution disabled. Both
capture and observation require the original budget. Host boundary, lineage,
accepted time, saved debt and readable records remain the original authority.
M3 still owns final host-time binding, inactive ECS/controller staging, complete
publication, runtime job ownership, supersession, swap and retirement.

## Scheduler authority

`scheduler::continuation::LoadRetry` owns one complete original `InFlight` plus
`PromptContext`. It preserves the semantic operation, actor and presence epoch,
lane, selected round-robin position, exact prompt, accepted output budget,
drained inbox and presented history. The private queue is separate from both
ordinary actor-keyed `retry_work` and intent queues. Preparation moves an
unfinished flight into it. Held success/error/oversized completion stays on the
actual flight and takes the ordinary application path exactly once.

Retries remain in semantic allocation order; an accepted replacement updates
only the external request ID. Original method is `request_with_budget` and the
budget includes an explicitly accepted `None`. There is no prompt rendering,
inbox prefix restoration, coalescing, new semantic ID, fairness advancement or
novelty submission stamp. Later arrivals stay in their current buffers. Saved
presented history graduates on successful completion, in completion order, as
it does for an ordinary successful turn. The exact saved text is still the
provider input even if newer work completed first.

Restored protected reactions take priority. Otherwise ordinary newer protected
intents can defer a saved idle/handoff retry, including offstage empty-inbox
work. The retry ignores stage and novelty eligibility but honors composing
suppression. Busy preserves every field and root; it only schedules another
attempt after at least one second, without failure percept or failure counter.
A real later provider failure is distinct: it emits the ordinary real failure
percept/backoff while retaining its exact obligation in this queue, rather than
overwriting a newer actor-keyed retry. Incarnation/control/departure invalidation
releases an obsolete deferred root without submission.

There are at most64 deferred obligations. At full capacity, ordinary creation
is backpressured: spend the oldest existing retry before another ordinary prompt
can make a65th obligation; if composing suppresses it, retain all intents and
stall. This explicit saturation exception preserves the next save/load's
representability. It does not drop a protected intent or synthesize a failure.

## Prompt knowledge receipt

`Knowledge` formerly had one global seated-key receipt and one offered occasion
per actor. `render_prompt_and_drain` fills those, and scheduler application reads
and consumes them. The one-flight assumption no longer holds across a deferred
retry and newer active work. Preparation detaches the old seated/offered context
onto its obligation. Applying a resumed result temporarily installs only that
context, then restores the newer global receipt/actor occasion. Receipt-batch
deferral parks the still-owned old context again. Success preserves ordinary
application effects: an occasion created by application can supersede the saved
newer occasion only if its timestamp is at least as new. An unconsumed old
offered context on stale/replay retirement is discarded. Historical seated keys
need not still be live, but must be below saved Knowledge's allocation high-water.

## Night Office

The exact saved submitted flight remains owned, with `load_retry_pending`
explicitly distinguishing a retired external execution. It is excluded from
completion matching until `request_night` accepts a new request; shared numeric
IDs belonging to the scheduler therefore cannot be harvested by an inactive
saved Night flight. Pending retries participate in lazy gate computation even
with an empty ordinary queue. They respect existing pacing and NightGate,
including player priority, and are retried only on their owed game day for the
same present LLM incarnation. Busy keeps exact prompt/budget/root; obsolete work
increments dropped once and releases the root. Held completions use normal
Night application without a provider request. Queue-time `last_reflected`, run
totals and the independent ambient day guard are not re-enqueued or reset.

Ordinary new person duties now capture a queue-time incarnation. V1 never
represented this before semantic admission. Complete Night V2 explicitly owns
`queued_incarnations` in exact queue order: Some for a Person, None for a Ward,
same count as the queue, agreement with any existing admitted incarnation.
Busy/pop/admission preserve this field on Due. V1 preparation uses its saved
admission epoch if one exists, otherwise the saved person's current World epoch;
it cannot claim an earlier lifetime absent from V1. Missing historical persons
are dropped by migration. Preflight checks the exact drop delta against the
unchanged post-preparation counter headroom so immediate capture remains valid.

## Speech and Floor

The actual SpeechRouter starts inactive after hydration. Preparation retains
all available text, captures, accepted/parked recordings, original purpose and
interruption status in ordered `InterruptedSpeech` groups. The current source
has public-player-speech purpose only; no proposition state is invented. Host
draft/selection/readable state remains in the retained Host owner. A fresh
intentional submission is required. No transcription, microphone, TTS or World
speech action runs during preparation.

Each unfinished accepted recording advances its original command to
`recording_interrupted_by_load`, with original ordinal/affected references and
the saved boundary time. Already terminal commands retain their exact receipts
and effects. Speech-action roots are removed and released; the ledger's new
updates are drained into the durable interruption group. Thus interruption
notifications are owed, retained authority for later M3 publication and for
complete re-save; they are not dropped to satisfy the save boundary. Historical
terminal entries agree exactly with any retained ledger entry. If evicted, the
operation must be at/below that producer's accepted high-water and compacted
floor, and its receipt ordinal at/below ledger next_ordinal. Original accepted
provenance and terminal receipt agree on ID, ordinal and affected refs; time and
exact code/message are checked, and duplicate semantic IDs across groups/active
recordings refuse. There are at most64 nonempty archived inputs/groups.
Distinct command IDs also have distinct ordinals within and across histories.
Constant-space live/saved ledger scans reject reuse of any currently retained
ordinal, including active speech and other command families; historical accepted
provenance and its own terminal update deliberately share their original ordinal.

Floor clears microphone liveness and drains old voiced waits. Existing
foreground/background reading and post-utterance deadlines survive. Each old
voice wait becomes the maximum matching saved subtitle/bubble/unread reading
deadline, capped by its original failsafe, in its original foreground/background
scope. A wait with no readable owner releases at the saved instant. It does not
grant a new post-audio beat. Re-preparation does not re-arm pacing.

## Closed complete schemas and services

The full envelope remains version1 with exact manifest/image binding. Each
affected component dispatches strictly between its historical V1 and a closed
V2 wrapper: Scheduler `{version,base,continuation}`; Night
`{version,base,load_retry_pending,queued_incarnations}`; Speech
`{version,base,interrupted}`. Every field is required (including explicit nullable
fields), unknown fields refuse, and shared roots/Knowledge/ledger still validate
jointly. V1 fixture bytes and decoders stay unchanged. Only insufficient legacy
component export APIs refuse newly represented state. Complete capture supports
prepared, deferred, resumed and held work, including a newer active prompt beside
older deferred work and another load at that exact instant.

`bind_services(upper_bytes, factory)` reserves a separate same-budget lease
before invoking a generation-keyed factory. The returned four service handles,
capabilities and runtime path install without any trait call. Generation mismatch,
repeat binding, admission refusal, factory error and panic dispose services before
their lease. The supplied allowance is a trusted host construction/storage bound;
it does not infer device/thread storage or permit a non-Send Engine on a worker.
M1c's existing runtime-generation fences remain authoritative for cognition,
Night, STT/TTS, PCM, microphone/backend status and SpeechPresented.

## Evidence seam

Root owns independent public host and sim-behavior assertions. The only shared
adoption helper is cfg(test)-only `engine::continuation::adopt_for_test`: it
roundtrips actual pending-owner codecs, binds their checked sidecars, invokes the
same private preparation and preserves a recording service. This is explicitly
not production adoption. Full public host tests exercise complete capture,
hydrate, preparation, re-save and re-load without extracting an Engine.

`host_checkpoint::tests_continuation_owner::m2c_continuation_probe` uses actual
authored520 and +2000 successfully placed fixtures, ordinary pending cognition,
accepted speech and retained readable work. Preparation, inert-service binding
and prepared-owner disposal have separate timing columns. First-sample complete
V2 re-save/read/hydrate/prepare/re-save must preserve exact bytes. Optional
`ALIBI_CONTINUATION_FIXTURE` uses create_new; setting
`ALIBI_CONTINUATION_READ_FIXTURE=1` reads it in a fresh same-image process under
admission. Original host disposal, backend/device construction, M3 adoption and
frame-budget acceptance are excluded from these measurements.
