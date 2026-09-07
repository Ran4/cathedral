# M1c — Runtime generations and bounded delivery

Status: Implemented, verified and coordinator accepted (2026-09-08). M1d and saved-world adoption remain unimplemented.

## Production ownership

`RuntimeGeneration`/`RuntimeEnvelope<T>` are pure values. The production host allocates a checked, nonzero process-monotonic generation and fixes it into `LocalEngine`, `EngineConfig`, both bridge endpoints, every backend sender/receiver, fake cognition staging and microphone service. `EngineCommand::InGeneration` is stripped only for the current engine. Wrong/nested transport generations are rejected before command identity, carried position, input watermark or final physical sample sequence. Actor `presence_epoch`, semantic operation IDs and ephemeral numeric RequestId/TranscriptionJobId/SpeechEventId remain separate identities.

Generation zero supports the existing isolated direct sim/headless fixtures and single-world headless runner. It is not a replacement-runtime identity. New host runtimes use checked allocation; numeric cognition and transcription job exhaustion refuses before wrap. Caller-owned factories (`BackendsHandle::start_for_generation`/`next_generation`) require the host's unique generation. `next_generation` rejects a generation no newer than its source handle and creates fresh channels/producers; it cannot retag existing endpoints. M3 must use a single host adoption coordinator, never allocate competing replacements with the same generation.

`BridgeHandle::try_send` binds intentional commands at successful producer enqueue. Its `BridgeCommandSender` replaces raw microphone access and permanently retains the original generation and retirement flag. `PlayerIntentWriter` tags input at creation; late Bevy input messages cannot allocate new HOST IDs or resolve a replacement UI request. The finite command cohort validates transport shape before sequence accounting, including a nested old-generation envelope carrying `i64::MAX`.

Backend completion conversion preserves the receiver's immutable generation for cognition/Night, batch/realtime STT, buffered/streamed TTS and status. Fake staging carries its own immutable generation. All asynchronous callback variants pass through the actual Engine guard. Ready/start/disconnect publications and the accepted host boundary also carry generation. An empty inbox still receives one nonblocking disconnect probe; that synthesized lifecycle event belongs to the inbox's original generation, and explicit matching disconnects are reported once.

Speech presentation messages (`PresentSpeech`, WAV, failure, PCM, end, stop and clear) retain generation. The consumer synchronizes its owner before delivery, rejects stale events before touching a matching current speech ID, and acknowledges only through the same generation's handle. A retired microphone acknowledgement cannot release a new suspension. The microphone poll checks service generation before all lifecycle events or its disconnect path. Presentation cleanup never resumes a different generation's microphone.

## Terminal conservation and admission

A successful external submission reserves one terminal record and worst-case result bytes before the job is queued/spawned, including synchronous fake and misconfigured cognition. Capacity is not released when the worker finishes: its queued completion owns the reservation through consumption. Duplicate terminal publication is ignored. The final unexpected producer lease drop publishes a normalized explicit cancellation/failure; a success-shaped fallback cannot manufacture success. Deliberate clear/submission refusal disarms the reservation, preserving realtime acknowledgement tombstones. Deliberate runtime retirement fences and drains old results, whose semantic obligations belong to the later restore/retry owner.

PCM admission and terminal publication share a per-job mutex through enqueue, so concurrent cloned producers cannot publish a chunk after the terminal. Chunk identity must match its reserved job. Byte/count overflow spends that job's reserved terminal failure once; late chunks, end or panic cannot publish another result. Status is bounded best-effort health/progress; dropping redundant status does not drop accepted terminal work. Backend refusal drives the existing speech text/floor fallback or cognition/STT busy policy while deterministic engine polls continue.

`LocalEngine` publishes nonblockingly. The host outbox reserves one record and byte headroom for a fault. Exceptional count/byte overflow retires this runtime, emits one disconnect and stops that entire flush. It never restores `AcceptedHostBoundary` after an incomplete publication. This exception is not the supported ordinary-burst policy: the combined maximum-input/reply witness remains live under actual voice admission refusal.

## Frozen capacities and measured retention

These are pipeline-owner bounds, not a claim about whole-process RSS. Existing world data, Bevy assets, HTTP/TLS/socket implementation buffers and bounded-input parser temporaries are separate allocations. Queue charges end at dequeue; the finite consumer cohort can coexist with a refilling producer queue. Audio consumers have their own caps.

| Owner | Bound and accounting |
|---|---|
| Host input | 128 records; 24 KiB each including conservative allowance for both identity/generation wrappers. Borrowed String capacities and PCM bytes are checked before identity allocation. Production UI producers are fixed systems; transport admission is nonblocking. |
| Backend callbacks | 256 records total, including reserved future terminals; at most 64 terminals. Finite receive cohort at most 128. |
| Terminal result reservations | 64 MiB aggregate. LLM 401,024 bytes; STT/realtime 16 KiB; TTS 16 MiB + 1,024. Thus at most three worst-case TTS jobs can be outstanding, even though the worker command queue has 32 records. |
| Backend stream/status retention | 4 MiB aggregate; PCM at most 256 KiB/chunk, fixed valid sample-rate range, status metadata at most 4 KiB. Actual String capacities and shared sample allocation headers are counted. Fifteen maximum chunks plus metadata fit; the next chunk triggers one reserved failure. |
| Channel/lease metadata | Full-window test prints actual Rust record/lease sizes and charged payload totals. A separate conservative bound allows 64 bytes per crossbeam slot, 32 bytes per lease allocation, 4 KiB per retained failure descriptor and 4 KiB channel/shared base; total below 384 KiB. This is additional to payload reservations. Stable fallback IDs are preflighted to 256 bytes before cloning. |
| Host publications | 8,192 records, 128 MiB charged queue-envelope retention; normal traffic leaves one record and 1 KiB for fault reporting. No deep-cloning publication API shares a single accounting token. `publication_bytes` covers nested snapshots, IDs, maps, Vec capacities and audio; BTree metadata has a conservative 1,024-byte allowance per entry plus root. |
| Presentation | 512 pending subtitles; two PCM streams, each fixed 1,048,576 `f32` samples: 8 MiB allocated sample storage total, plus one bounded conversion temporary. Three ready WAV clips / 48 MiB; one actively playing clip can add 16 MiB. Overflow releases its pending audio expectation and retains readable text. |
| Fake cognition | 64 staged outcomes; 32 retained prompts, each at most 1 MiB allocation. Staged successful replies at most 400,000 UTF-8 bytes / 100,000 scalars each; oversize becomes a small explicit failure. |
| HTTP cognition/STT | Cognition prompt allocation at most 1 MiB. Success/error response body at most 2 MiB before JSON/text parse, with exact reserve growth; enough for the supported 100,000 Unicode scalars including JSON escapes. Oversize is a bounded explicit failure. Cloud STT file read stops at 16 MiB + one overflow byte. |
| Speech workers | STT queue 4; discard queue 64 (path allocation at most 4 KiB). TTS queue 32 but admitted jobs additionally need terminal reservation; text at most 500 scalars / 2,000 allocated bytes, voice at most 64 scalars / 256 bytes, event ID at most 256 bytes. Worker protocol line at most 22,373,720 bytes; stderr line at most 64 KiB. |
| Realtime STT | 32 admitted utterances, keys at most 256 bytes; action queue 512, audio append at most 2,400 mono samples / 100 ms at 24 kHz (6,400 base64 bytes plus bounded metadata). Websocket frame/message cap 64 KiB; incoming provider IDs at most 256 bytes. Test-only FakeSocket's unbounded transport is not a production lane. |
| Microphone | Lifecycle queue 16, commands 8, shutdown/ack 1; lifecycle text at most 4 KiB, recording IDs validated within 128 bytes. Native capture queue 64 × 65,536 `f32`: measured 16 MiB allocated, plus one callback temporary. Oversized capture or queue overrun reports explicit failure. Native error queue has one bounded message. |

The full-window mailbox witness also proves that queued terminal/PCM allocations are freed after the final endpoints drop: accounting does not own its own sender, avoiding a channel/Arc ownership cycle. The controlled race witness pauses a chunk before enqueue, verifies the per-job guard is held, starts a terminal publisher, and then releases the chunk; delivery is always chunk then end.

## Verification

[The final full-workspace run](workspace-final.log) exits zero: **1,840 passed, zero failed, eight ignored across 33 targets**. [Per-target totals and command/environment records](verification.json) describe that single complete run. [Final source hashes](source_hashes.json) cover all 32 changed/new Rust files plus Cargo.lock. The [selected owner capture](owner_fields.json) records 26 owners/332 fields; the persistence delta covers additional host/backend owners outside that review index.

Two unchanged-source evidence runs also exit zero: [the backend library](backend-final.log) passes all 158 tests, and [the host smart-actor subset](host-final.log) passes 286 tests with one ignored. These retain the raw capacity witnesses: 256 mailbox records/64 terminals (25,665,536 terminal bytes and 21,202 status bytes), actual queued record 128 bytes, lease 168 bytes, conservative additional channel/lease metadata 328,320 bytes; PCM 8,388,608 allocated bytes, ready WAV allowance 50,331,648 bytes, microphone capture 16,777,216 allocated bytes. The values are per-owner retention bounds, with consumer/transient exclusions stated above.

[Plan validation](plan-validation.json) and whitespace checks pass; these are structural checks, not gameplay/renderer evidence. Early full-workspace compile attempts found the explicit headless EngineConfig and body-test PresentSpeech initializers missing generation fields; both were repaired before the successful final run. No golden fixture changes were needed.

Coordinator review checked generation rejection before identity/pose effects, delivery conservation, controlled chunk/terminal ordering, allocation ownership, publication-failure boundaries and the supported burst. An independent check matched all 33 source/lock hashes and all 32 changed Rust files, and recomputed the 33-target full-workspace aggregate from its raw log. This accepts M1c's implementation scope; M1d, save/load adoption and external performance evidence remain outstanding.

Production witnesses establish:

- Stale commands, current fake staging, cognition, Night, status, batch/realtime STT, all TTS outcome forms and SpeechPresented reuse live numeric/event identities and remain inert; current positive controls reach their owners.
- Every microphone lifecycle variant plus empty-channel disconnect, every queued presentation variant, pending UI input and microphone suspend acknowledgement is fenced against a matching replacement owner.
- Byte overflow yields exactly one disconnect, no later smaller publication, and no accepted capture boundary. Nested/stale carried `i64::MAX` samples leave current watermark/final sample usable.
- The ordinary combined burst produces 128 HOST receipts, 257 provider root/step receipts, 383 speech events and 254 actual voice queue-full fallbacks. The host remains live and a non-player resident moves while 63 terminal reservations remain held. No supported input/reply publication overflow occurs.
- Synchronous fake speech, synchronous misconfigured cognition, terminal saturation, retirement/reused IDs, oversized spare-capacity input/error metadata, unexpected producer loss and controlled PCM/terminal ordering are exercised on production submit/channel seams.

Exact command environment (repository root):

```text
CARGO_HOME=/tmp/alibi-m1b-cargo CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 /home/ran/.cargo/bin/cargo test --workspace --no-fail-fast
CARGO_HOME=/tmp/alibi-m1b-cargo CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 /home/ran/.cargo/bin/cargo test --bin cathedralbevy smart_actors:: -- --nocapture
CARGO_HOME=/tmp/alibi-m1b-cargo CATHEDRAL_HEADLESS=1 CATHEDRAL_FAKE_BACKEND=1 /home/ran/.cargo/bin/cargo test -p cathedral-backends --lib -- --nocapture
```

Raw temporary logs are `/tmp/alibi-m1c-workspace-final3.log`, `/tmp/alibi-m1c-backend-final.log` and `/tmp/alibi-m1c-host-final.log`; archived logs omit redundant terminal blank lines to pass the repository whitespace check; all test output bytes are unchanged. The verification manifest records both original and archived hashes. The temporary Cargo home is the M1b isolated cache: existing registry source/cache entries are linked read-only, local cache additions stay in `/tmp`; no shared registry writes are needed. Python uses `UV_CACHE_DIR=/tmp/alibi-uv uv run --no-project`. Only changed Rust files were formatted with `skip_children=true`; existing unrelated formatting in body.rs/item.rs was preserved outside the edited functions. No renderer was run and no focus/GPU performance claim is made. No unrelated untracked directories or historical evidence was changed.

## M2/M3 handoff and remaining limits

[The persistence delta](../../PERSISTENCE_INVENTORY.md#m1c-runtime-and-delivery-owner-delta--2026-09-08) distinguishes durable semantic work from ephemeral runtime execution. M2 must preserve exact unfinished semantic obligations, held results and accepted host watermarks; it must not interpret an old execution cancellation as a new domain refusal. M3 creates fresh channels, provider objects, fake staging, microphone ownership and presentation/UI owner state before adoption, with a unique new generation. Old results are never relabeled.

`LocalEngine::retire_runtime` and `BackendsHandle::retire` are immediate fences that retain the old bundle. Panic handling uses the same fence and retains potentially half-mutated engine/backend owners; it never polls them again. There is no unbounded reaper and no saved-world adoption yet.

Destruction is not universally nonblocking: native `Worker::close` can synchronously terminate/reap for about one second, and `SessionDir` recursively deletes audio files. `MicrophoneService` Drop requests shutdown and detaches without its former 300 ms wait, but its existing `discard_recording` fallback can still synchronously delete a confined file on the main thread. Microphone worker lifecycle sends are bounded **blocking worker** sends: if M3 retains an old full receiver without polling it, shutdown acknowledgement may wait until that receiver is drained/dropped. M3 must not wait for stopped acknowledgement while retaining a full undrained event receiver.

STT discard overflow defers file deletion to SessionDir retirement; the 64-entry memory queue does **not** bound retained disk files during a long active session. M3 owns budgeted filesystem/worker destruction and old-bundle draining. BackendRuntime Drop now uses background shutdown rather than a 500 ms grace wait. These ownership limits do not weaken the generation fence, but they prevent claiming all cleanup is nonblocking. M1d still owns the generic operation/duty kernel; this cut does not implement it.
