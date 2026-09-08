from pathlib import Path
p=Path('features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/PERSISTENCE_INVENTORY.md');s=p.read_text()
s=s.replace('| `SpeechRouter::TranscriptionTask.semantic`, `resolved` | S pending recording operation and captured attention/pose/task binding. `resolved` stages only synchronous completion until the outer receipt finishes and must be empty at the completed command/poll boundary. Submitted/uncommitted microphone/audio/text restores as the protocol\'s unsent draft; committed speech restores receipt/presentation without speaking again. STT RequestId/job IDs remain ephemeral execution IDs for M1c fencing. |','| `SpeechRouter::TranscriptionTask.semantic`, `resolved` | M2a13 S exact optional CommandId/receipt, request correlation, basename, accepted pose/backend and public-player-speech purpose; T unsent CapturedAttention. Nonempty `resolved` refuses this completed-boundary format rather than being dropped or applied. Accepted jobs/parked input becomes an unsent draft/status requiring new intentional submission; parked deadline is exact provenance, never a resumed timer. Full M2c still owes atomic ledger/root/Floor interruption and host draft publication. Earlier committed speech keeps receipt/readable presentation without another say; STT jobs remain E. |')
s=s.replace('| `SpeechRouter.next_job` | E checked transcription execution allocation; refusal on exhaustion. Its M1b recording semantic roots, exact held results, deadlines and unfinished work retain their existing S policy. M3 binds retry execution to a fresh generation. |','| `SpeechRouter.next_job` | E checked transcription execution allocator, omitted by M2a13 interruption projection; refusal on exhaustion remains ordinary behavior. M2a13 retains every accepted recording obligation/receipt and unconsumed stream text, drops unsent attention, and refuses synchronous `resolved` staging. Parked deadline is provenance only. M2c/M3 must release old execution/holds and publish unsent drafts with a new-generation fence; loading does not retry STT or auto-submit speech. |')
s+='''

## M2a13 SpeechRouter interruption component — 2026-09-09

Strict read-only SpeechRouter/EngineSpeech components now preserve the sim-owned
input to later complete load interruption: independent ordered onset basenames,
stream basenames and exact available text, and every accepted batch/parked recording
occurrence. Accepted rows retain optional exact CommandId/Receipt, request string,
basename, accepted pose/backend and raw parked-deadline provenance. Reused basenames
or requests do not merge tasks; sibling command steps can share a protected operation.
Actual router grace bits remain independent from Engine's original stored config.
Public purpose is `public_player_speech`, status is `interrupted_unsent`; current source
contains no proposition version or learned selection to invent. Task lacks original
spatial_seq/pre-accepted pose, so full envelope still owes original payload/category
binding rather than recomputing a purported digest from this projection.

[Every-field coverage](evidence/m2a13/OWNER_COVERAGE.md) classifies old stream protocol,
CapturedAttention, timing, jobs, TTS maps and execution allocator as transient/rebound.
Nonempty synchronous resolved staging refuses the completed-boundary format. Live
World.speech_actions must exactly match Some accepted IDs; unadopted backbone plus
ledger validates their reconstructed R index and exact retained/protected receipts,
including reachable terminal results and affected order. No candidate World/ledger,
service query, IO, poll, constructor, normal abort/resolve/say or partial Engine
installation occurs. Prior committed attribution stays in earlier saved owners.

[Admission](evidence/m2a13/ADMISSION.md) retains raw input charges and adds a proved4MiB
saved-ledger validation allowance under unchanged128MiB encoded/expanded, depth64 and
shared1GiB caps. Eight captures/eight streams/eight accepted rows are independent;
old stream_jobs are not falsely bounded by active streams and duplicate TTS map IDs
are not made invalid. Available text admits400000UTF8B without speech validation;
new intentional submission is still required. Max/sparse cases remain private tests;
actual authored520/all+2000placed2520 measurement uses ordinary <=50ms polls and pure
controlled services with primary submitted/committed input-output witnesses.

Full M2c still owes atomic interruption outcomes, root release, Floor hold cleanup,
old execution retirement and draft/status publication with no repeated say. Owed
already-committed readable presentation must bind the actual event/message/host text
owner, not TTS backend maps. M3 owns new-generation callbacks and dedicated initial
publication without poll. Complete manifest/build/target/toolchain/DefaultHasher,
all-consumer horizons, original cognition output-token receipt, full capture/hydration
and actual Running/Save/Load/retiring lifetimes remain pending. Earlier naive
backbone+Round Save+Load already exceeds1GiB before Running; this is no complete-save
or synchronous-host acceptance claim.
'''
p.write_text(s)
