Status: M2c implementation and evidence accepted (2026-09-15). M2d and M3 adoption remain.

# Pending work independent review

The accepted predecessor is M2b commit
`019f451c12bf7c94c9823678403f50244c88d421`. Its post-fixture source map has
970 inputs and SHA-256
`4519679b5866452b4fcd6c2096ad9522479d9f4a81a91690843916e26745d3f9`.
The tested pre-publication map had 967 inputs; only two verified fixture outputs
and their README were added afterward. All prior input bytes remain identical.
M2b passed 2,191 workspace tests, 600 release hydration samples and ten fresh
fixture commands. Its exact evidence and creating images remain historical.

The sequential fresh-context owner is `m2c_pending_work`, with sole implementation
and Cargo ownership until explicit cession. Root owns this review, milestone
status, release evidence, independent tests and commits. No other milestone agent
is implementing concurrently. The user authorized continued delivery and a commit
after each coherent leg.

## Agreed design and review gates

Consume the admitted HydratedEngine into a protected PreparedContinuation.
The Engine stays private and non-Send; preparation and admitted generation-keyed
service binding cannot poll, submit or expose World. M3 owns complete host
adoption, initial publication and host-origin binding. Saved time and lineage
remain exact throughout preparation.

Keep load-specific exact retries separate from ordinary scheduler queues and
generic provider-failure retry_work. The initial closed bound is 64 obligations,
still subject to unchanged cumulative expansion and shared admission limits.
Retain semantic/incarnation/lane/fairness identity, original method/prompt/budget,
drained events and presented history. Later arrivals stay separate. Busy retains
the exact obligation without a false failure percept, re-render or lost idle
turn. New protected work may defer an old idle/handoff retry. Capture must also
represent both obligations if another save occurs at that point.

Root traced a related ownership dependency: Knowledge stores one global seated
receipt and per-actor offered occasions. Rendering a newer prompt can overwrite
the old seated receipt; a newer same-actor occasion can also be displaced.
Deferred/resumed obligations therefore retain their own prompt knowledge
context. Completion uses its original context while conserving newer context
and actual application-created effects. No stale captured cognition sidecar may
replace the current active/deferred ownership split during observation or save.

Held success/error/oversized completions need no new request and apply exactly
once through ordinary validation, floor and receipt-capacity behavior. Their
drained input stays out of inbox. Night queue/flight and saved owed day/incarnation
remain explicit; retry only in the valid window, yield to player work, and keep
settled/dropped duties and the ambient daily guard spent. Queue-time last_reflected
is not a completion record.

Uncommitted recording becomes durable unsent text/purpose/status, preserving
actual host choices and existing readable committed effects. The current speech
format has only public_player_speech purpose; no future proposition feature is
invented. Terminalize accepted recording roots before releasing speech_actions.
CommandLedger::advance creates update receipts, so preparation must retain those
updates in an explicit owed-publication owner instead of losing them or leaving
the immediate re-save boundary invalid. Clear microphone liveness, preserve
existing reading deadlines, and explicitly reconcile old audio awaits with
surviving readable progress. No speech action or fallback chat is replayed.

New complete state requires closed versioned scheduler/Night/speech representations
with required fields and full root/reference validation. Legacy V1 component
APIs may explicitly refuse state they cannot encode. The complete path must
support immediate re-save of the new state under the existing limits. Historical
fixture bytes and manifests remain unchanged; old schemas do not acquire hidden
defaults for new authority.

Preparation and service factories reserve before allocation. Move retained
buffers and context instead of cloning them; charge new container capacity,
terminalization/publication receipts and validation scratch. Drop Engine,
services and continuation owners before their leases, including failure and
unwind. Limits remain 1 GiB shared, 64/128 MiB encoded and 128 MiB typed expansion;
M2b's populated case has only 604,401 bytes expansion headroom. The 512 MiB Running
minimum is a trusted scoped requirement, not an arbitrary process-heap proof.

## Independent test ownership

Root owns `src/host_checkpoint/tests_continuation_public.rs` for public phase,
admission and protected observations, plus
`crates/cathedral-sim/src/engine/continuation_review_tests.rs` for behavior through
one cfg(test)-only adoption stand-in. Shared fixture/setup helpers may be supplied
by the owner; assertions remain independent. Module/helper signatures must be
agreed before root writes the test files. Private behavioral test access does
not imply production whole-host adoption or M3 acceptance.

Final source, meaningful owner/public tests, unchanged-source workspace results,
allocation proof, actual release scenarios and exact failed/final command evidence
remain acceptance gates. M2d future continuation and M3 application/file/runtime
publication remain separate mandatory legs.

## Source review findings during implementation

Night harvests before scheduler, and scheduler drains unmatched completions.
An inactive saved Night flight must therefore be excluded from request-ID
matching until its replacement submission is accepted: a fresh scheduler request
can reuse the saved numeric Night ID while Night is gated. Pending Night work must
also participate in `could_submit` when its ordinary queue is empty, so the lazy
stage check is still evaluated. The owner is adding both guards and a cross-lane
regression case; final verification remains pending.

The existing Knowledge validator permits historical seated keys after their facts
have expired, but requires every key to be below the saved allocation high-water
mark. Moving seated authority into per-obligation PromptContext does not remove
that requirement. Complete V2 validation must check deferred and resumed contexts
against saved Knowledge, without incorrectly demanding current live facts.

The service trait for Cognition is submission-only. Completion delivery enters
through EngineCommand and the existing generation envelope; there is no Cognition
poll method to fence. Host microphone, speech presentation, fake-worker staging
and bridge callbacks already have separate M1c generation tests. M2c still needs
behavioral evidence that these envelopes remain effective on the prepared owner;
M3 must retain those routes when replacing the actual host services.

New interruption receipts must agree with their saved accepted provenance:
`advance` preserves ordinal and affected references, moves time forward, and
sets the specific interruption outcome. A retained ledger entry must match the
terminal receipt exactly; historical eviction requires the appropriate producer
and compaction bounds. Receipt order is inclusive: `next_ordinal` stores the last
issued ordinal, and `begin` assigns that exact value after incrementing it.
Review caught an incorrect strict comparison that would reject the newest
recording after preparation. The owner is correcting it before verification.

Night queued Due rows previously had no person incarnation until admission. The
approved V2 extension preserves a queue-aligned incarnation for newly queued
person work; Ward rows remain explicitly null. V1 migration can anchor only to
the saved person's current incarnation (or an already admitted epoch). It cannot
claim knowledge of an earlier queue lifetime. This is a documented correction to
ordinary new queue behavior; historical V1 bytes and readers remain required.

The 64-obligation retry limit also bounds new ordinary admission: admitting a
65th active request would create a state that cannot be prepared after another
save. Saturation therefore spends already owed exact work before accepting more
ordinary requests. Protected intents remain queued. Composing suppression still
holds background execution and cannot fall through into an unrepresentable
65th request. This is an explicit capacity exception to ordinary priority.

## Independent tests written; final results pending

Seven host public tests cover unchanged unrelated owners and exact saved host/
time/lineage, refusal under shared memory pressure, unwind at each preparation
stage, inert admitted service binding, service mismatch/refusal/panic/rebinding,
lease lifetime during service disposal, same-budget capture/observation, an
ordinary accepted recording interrupted once across a second complete save/load,
and closed required V2 fields with contradictory/duplicate receipt rejection.
The first development public run passed six and failed one while a newer Night
test wrapper and earlier compiled library overlapped; it is not final evidence.

Six sim behavior tests use the agreed cfg(test) adoption stand-in. They cover
empty/off-stage exact idle work with newer protected work across a second load;
Busy/input/cursor conservation; original prompt Occasion versus newer same-actor
and global seated context; held success/error/oversized results applied once;
inactive Night versus a reused scheduler request ID; saved Night day/incarnation
expiry; and old/unwrapped Engine callback families with a current-generation
positive control. The public path exercises full hydration and capture; the
private stand-in does not claim whole-host adoption or admission lifetime proof.

The second continuation development run reached behavior: 31 passed, two root
fixture preconditions failed, and one historical fixture writer was ignored.
All four other root cases and owner Night/Floor/saturation/provider-failure cases
passed. The failed fixtures were corrected without relaxing continuation gates:
pending history is checked as the original flight's presented receipt, while
rendered recent history checks prompt identity; claimed facts now belong to the
actual reader and subject-name questions trigger ordinary relevance selection.
The old presented receipt must graduate exactly once, and both original and newer
prompt contexts must contain their intended seated keys. Final reruns are pending.

The structural layout witness reports 112,128 B under the 131,072 B allowance:
44,544 B retry container/sort growth, 14,336 B interruption-group growth, 49,152 B
receipt working storage and 4,096 B wrappers. Source review confirms missing-
person Night migration preflights its exact drop delta against the unchanged
post-preparation counter headroom. These are preparation bounds, not a bound on
future polling or arbitrary service/backend construction.

Complete-envelope mixed deferred/new-active coverage is being added by the
owner after review identified that the existing mixed test roundtripped only
pending-owner codecs. The new case must exercise full root/Knowledge agreement
through capture/validate/hydrate/prepare, without adding production extraction.

Coordinator release tooling is prepared but has not run. It preserves command-
start identities, every original/log/archive and all cold/slow samples. Fresh V2
writers use a fixed lineage and create-new files; fresh same-image readers check
their input hash against the actual prepared output and second re-save hash.
The probe reports its real pending shape, so held and unfinished work cannot be
silently described as the same measured scenario.

The next development host run (`public-02`) passes all seven independent public
tests. The full sim-library development run (`sim-lib-01`) passes all six root
behavior tests: 822 library tests passed overall, one failed and 28 were ignored.
Its sole failure is the new owner's synthetic Host fixture lacking required
singular HUD records before its first capture; correction and final verification
remain pending. That is not evidence of completed full-envelope mixed coverage.

Newly rung person queues now correctly refuse historical Night V1 export because
their queue-time incarnation needs V2. Existing integration tests are being
reconciled without exposing a mutable candidate or cloning/extracting NightOffice:
ordinary queue observations/refusal remain, and explicit variants of immutable
historical V1 fixtures preserve legacy queue/Busy/root/prompt/clock/pace checks.
Current complete V2 coverage remains responsible for new runtime serialization.
Historical fixture files have no source diff.

The first authored debug probe stopped in readable-workload setup before any
continuation measurement: no real subtitle was produced within the helper's
24 updates. Review identified a probe-only configuration difference: M2c forced
idle cognition to all actors before startup, whereas M2b's measured fixture used
the default configuration. The override was removed and the original subtitle
precondition retained. Preserved M2b performance metrics for both profiles report
an unfinished scheduler flight already under the default setup. The corrected
M2c probe must confirm its own source shape; no M2b observation substitutes for it.

Further schema review requires global receipt ordinal identity after eviction:
two distinct archived recording commands cannot claim the same ordinal, nor can
an evicted archived command claim an ordinal held by another retained command.
The owner is adding bounded constant-space history and ledger scans plus valid-
eviction negative cases. The same command's accepted and terminal receipts
deliberately share their original ordinal. No count or byte allowance increases.

The corrected authored debug probe (`probe-authored-02`) passes with 520 actual
characters, one submitted unfinished scheduler request, no held/Night request,
and one accepted recording. Preparation produces one exact retry and one
interruption receipt; eight unaffected category hashes and complete second-load
re-save bytes agree. The debug sample reports 38,447,138 B typed admission and
758,093,679 B shared peak. Its 13.217 microsecond preparation, 2.963 microsecond
inert binding and 1,612.364 microsecond disposal are development smoke timings,
not release distributions or whole-host disposal evidence. Populated verification
and the final source freeze remain pending.

The populated debug probe also passes with 2,520 total characters and all 2,000
requested residents placed. It records one actual unfinished scheduler input and
one accepted recording, one restored retry/terminal notification, all eight
unaffected categories equal and exact complete second-save bytes. Its cumulative
typed charge is 133,751,958 B (465,770 B below 128 MiB); shared peak is
977,116,076 B. The debug preparation/binding/disposal sample is
13.671/3.890/4,299.372 microseconds. Both successful probes and the 39-file
format check used unchanged 980-input map
7cc3c5f0b71713502bd5f8001768196e6ca138f55e4e9356960a97c1629fcf4c.
These diagnostics do not replace frozen-source final or release verification.

The first legacy integration run identified two stale 2,400-token assertions
after its component-composition fixture changed from a person to a V1-compatible
ward. The correct 1,400-token ward budget was already pinned by the separate
unchanged exact-input matrix. The two assertions were corrected, preserving
that matrix and production behavior. Final format/workspace records must include
this two-line test delta after the successful debug probe source map.

Final owner workspace verification passes 2,215 tests, zero failures and 44
intentional ignores across 46 groups. Root independently counted the exact raw
log. Both the complete-envelope mixed-obligation test and ordinary-eviction
ordinal test pass, as do all six independent sim and seven public host tests.
The run took 1,156.634 seconds including compilation and retained unchanged
980-input map b277379757dd70a1476adb44e583aeb808533ef58178fa88f70f5e20600d35a1.
Raw log SHA-256 is dc88e0c8d1498345a5b0a54313fe1a1468b9ec36bf479c0769ef279641502216;
its exact gzip archive is e3fa83441b2e6bf2f8177f47c5c66048c64e3a3cb04e8b5a755487a68eb443be.
Release evidence and final acceptance remain pending.

Final coordinator acceptance follows the unchanged-source full workspace and
format checks, all 23 owner-command audits, a preserved release build, four
release smoke samples, 600 release measurements and eight fresh-process fixture
commands. No source or test assertion changed during this release verification.

The release executable is `/tmp/alibi-m2c-host-reference-binary-1` (155,895,728 B),
SHA-256 `36512740197e3a687f86dff2efdc86a8e06c7189690b3634b8215ba4c6701e52`.
The [performance audit](release-performance-audit.json) verifies 1,800
complete-phase plus 3,000 stage observations, retaining all cold/slow samples.
Preparation p99 is 11.561/13.114 microseconds, inert binding 2.985/3.649,
and prepared-owner disposal 1,402.100/4,524.353. Every populated disposal exceeds
2 ms; none of the measured phases exceeds 30 ms. The measured work is explicitly
quarantined preparation and inert binding, with the source host retaining nav;
it does not establish whole-host retirement or M3 frame acceptance.

The [fixture audit](fixture-audit.json) verifies three identical fresh writer
outputs for each profile and fresh same-image readers whose prepared and
re-saved bytes equal their original input exactly. The source pending shape is
truthfully one unfinished scheduler request plus one accepted recording.
The published authored/populated fixtures are 3,290,705/12,761,629 B, SHA-256
`a613b24cb0010b9069dbe57b73cf45b8b4adf25d4e4677afcc74df4d5ad430e2` and
`5ec758a795a7f3264e40a50b0c9a2257f58837fe67b739587dc48176d0ae30eb`.

[Publication](fixture-publication.json) adds exactly those two files and their
README after all frozen commands. The final 983-input map is
`d5c73f9a292414d1e1c2a874ae18974c3d734f240507d61b3ca1f64917a24d2a`; every prior input equals the tested
980-input map. Accepted historical fixture/evidence bytes remain unchanged.
There are no open M2c source findings. M2d must now test broader future
continuation; M3 must bind/adopt/publish real host and service state.
