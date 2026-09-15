Status: M2c admission reviewed against debug and release evidence (2026-09-15).

# M2c preparation admission

All numeric budgets remain unchanged:1GiB across Running/SavePayload/
LoadCandidate/RetiringGeneration,64/128MiB encoded profiles,128MiB cumulative
typed expansion. M2b's separate64MiB asset lease and512MiB trusted Running scope
remain. Prepared continuation adds128KiB to the actual cumulative hydration
meter before any transformation; the result must still be <=128MiB and its
reservation must fit shared headroom. It is not an unmetered side allocation.

Existing prompt/drained/presented/occasion/receipt buffers move. No old text is
restored into an inbox, and no prompt is rendered. The extra structural bound
covers the following additions alongside already charged decoded old capacities:

* Scheduler deque growth: a bounded64-row queue can allocate at most128 new
  LoadRetry slots on one push. Stable sort gets another64-slot scratch allowance.
  Original decoded allocations remain charged. The layout test multiplies the
  actual `size_of::<LoadRetry>()` by192.
* Interrupted-speech group vector:64 nonempty inputs bound the group count;
  one push can allocate at most128 new InterruptedSpeech slots. Existing records
  move; the layout test uses128 times the actual group size.
* At most8 active accepted recordings require receipt progress. Existing ledger
  entries are <=1024 encoded bytes (`receipts::MAX_ENTRY_BYTES`), and the speech
  projection additionally limits affected refs to2 (kind<=16, ID<=64), outcome
  code<=48 and message<=192. `advance` replaces outcome/time, serializes/trims
  the bounded entry, returns a temporary clone and marks an update. `drain_updates`
  clones up to8 bounded receipts into the archive and releases retired retained
  entries. Allow three2048-byte buffers per row (49,152B) for serialization/
  growth, temporary/retained clones and update-tree/vector working storage.
*4KiB covers wrapper/control growth. Floor drains existing waits and scans
  borrowed readable rows; Night migration mutates existing Due slots and may
  remove rows without allocating. Their old decoded capacities are already
  charged. Night V2's temporary queued-incarnation vector is metered on decode,
  and its serialization is borrowed.

`continuation_structural_admission_bounds_growth_and_receipt_working_storage`
prints the actual closed layout calculation and refuses an accidental future
size increase beyond128KiB. The debug layout measured in `continuation-02` is
44,544B retry growth/sort +14,336B archive growth +49,152B receipt work +4,096B
wrappers =112,128B, leaving18,944B inside the131,072B allowance. This is a
preparation-only bound. Later polling,
provider requests, application actions and copy-on-write runtime frames belong
to the Running generation's existing admission policy and later M2d/M3 evidence.

Preflight also refuses an unrepresentable65th retry, more than64 interrupted
inputs, unresolved accepted cognition budgets, or Night migration that would
consume the unchanged dropped-counter headroom. At runtime a full64-row exact
retry queue backpressures ordinary prompt creation so another load can represent
every accepted obligation. Composing suppression stalls both rather than adding
a65th prompt. All input buffers and protected intents remain owed.

Service binding reserves an explicit disjoint lease before the factory. Its
trusted byte bound includes owned/shared service handles, capability/runtime path
and synchronous factory working storage. The measured inert-service fixture uses
64KiB; this is not a bound for arbitrary real backend/device/thread construction.
The factory must return the requested runtime generation; no service availability,
poll, submit, frame, audio or backend warmup call occurs during binding.

Drop order is structural: Admitted owns the value before its main reservation;
PreparedContinuation owns Engine/Host/report before asset/service leases; the
temporary service factory owner holds returned handles before the prepared value.
Normal disposal, mismatch, factory refusal and unwinding therefore destroy
semantic/service storage while the corresponding main/subordinate charges are
still held. Root tests instrument all four service destructors and each public
observer boundary independently. No detached owner or alternate budget can pay
for capture/observation. No unsafe Send implementation is introduced.

Prepared capture streams actual owners plus retained Host into the unchanged
complete validation pipeline. Save scratch and raw output reserve before
allocation, all typed verification remains under the one cumulative meter, and
the returned candidate retains only its admitted bytes/offsets/manifest. A second
load releases the old prepared owner before admitting a new asset generation in
the probe. The complete re-save path includes deferred and active retry contexts,
interruption history and notifications, and queue-time Night incarnations.

Release reports separately record actual source pending shapes, cumulative typed
preparation charge, asset/service lease, aggregate shared peak, exact unchanged
category checks, and immediate V2 second-preparation equality. No synchronous
whole-host adoption, source-host retirement or last-shared-nav-owner disposal
claim follows from these numbers.

The successful debug authored/populated probes (`probe-authored-02`,
`probe-populated-01`) prepare38,447,138/133,751,958 typed bytes. The populated
case leaves465,770B below128MiB. Shared peaks, including prepared full re-save
and Running, are758,093,679/977,116,076B, below1GiB. Both actual source shapes
have one unfinished scheduler flight and one accepted recording; neither has a
held completion. These one-sample diagnostics establish reachability/admission
and exact re-save, not release timing distributions or host-frame acceptance.

The audited 600-sample release dataset confirms the same typed charges:
38,447,138/133,751,958 B. Maximum shared peaks including Running and re-save
are 758,093,673/977,116,076 B; maximum process RSS is 383,636/495,908 KiB.
Preparation p99 is 11.561/13.114 microseconds and inert binding p99 is
2.985/3.649 microseconds. Prepared-owner disposal p99 is 1,402.100/4,524.353
microseconds, with every populated disposal above 2 ms. These measurements
retain all cold/slow observations and exclude whole-host/last-nav-owner
retirement. No existing cap or M3 frame requirement is relaxed.
