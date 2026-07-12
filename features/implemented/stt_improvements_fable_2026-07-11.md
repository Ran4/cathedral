# Streaming player speech transcription

Status: implemented; live-provider validated 2026-07-11 (see Measured results).
In-game microphone validation (speak/firewall/Z-toggle/idle-reconnect matrix)
remains a manual pass.

## Problem

The time that matters for player speech is the time until the transcribed
utterance is applied to the world — `apply_action(world, player, "say", ...)`
in `server.py` — because that is the moment NPC inboxes receive it. Call this
interval, measured from the player's last spoken word, **T_hear**.

Today the pipeline is strictly batch. While the player speaks, audio only
accumulates in a local WAV (`microphone.rs`); the cloud is first contacted
after the utterance has completely ended:

1. End of speech is detected after 700 ms of trailing silence
   (`TRAILING_SILENCE`).
2. The finished WAV — 32-bit float at the device rate, roughly 190 KB per
   second at 48 kHz — is finalized, announced to Bevy, and referenced to the
   sidecar as `player_recording`.
3. The sidecar uploads the whole file to OpenAI's batch endpoint
   (`audio.transcriptions.create`, `gpt-4o-transcribe`) and waits for one
   complete response, typically 1–3 s for short clips plus upload time.
4. The say action is applied and NPC inboxes are updated.

T_hear is therefore ≈ 0.7 s + upload + batch inference ≈ **2–4.5 s**, and it
grows with utterance length because upload and inference both scale with
audio duration. None of the work overlaps the time the player is speaking.

Two adjacent losses compound this:

- Every component after endpointing is serialized, even though the audio was
  fully known hundreds of milliseconds to seconds earlier.
- After the say is applied, no NPC turn is prioritized. Only fake-mode
  `debug_player_say` with an explicit target calls `scheduler.prioritize`;
  real transcribed speech is a broadcast say, so the reacting NPC waits for
  its round-robin turn before it can respond.

## Existing foundation

- The microphone worker already produces mono `f32` buffers on a dedicated
  thread with voice-activity gating: ~80 ms start confirmation, 250 ms
  pre-roll, 700 ms trailing-silence endpointing, 15 s utterance cap
  (`PLAYER_SPEECH_MAX_SECONDS`).
- The bridge already streams base64 PCM over the JSON-lines protocol in the
  NPC→player direction (`tts_chunk`, mono 24 kHz signed 16-bit PCM). Player
  audio streaming is the symmetric inbound path.
- The bridge writer accepts commands from non-ECS threads (the audio-ack
  sender precedent), the command channel is bounded at 128, and protocol
  lines may be up to 1 MB.
- The sidecar polls every 20 ms (`run_stdio`), owns the OpenAI credentials,
  and already isolates STT work on a dedicated worker with bounded queues and
  credential-safe status reporting.
- `transcription_result`, WAV ownership/cleanup, and the frozen-position say
  application in `_poll_transcriptions` are proven; the streaming path should
  converge into them, not replace them.

## Provider facts

Per the current OpenAI realtime transcription documentation
(<https://developers.openai.com/api/docs/guides/realtime-transcription>):

- Transcription-only sessions connect to
  `wss://api.openai.com/v1/realtime?intent=transcription` and are configured
  with a `session.update` of `session.type: "transcription"`, an input format
  of `audio/pcm` at 24 kHz mono, and an `audio.input.transcription` block
  naming the model.
- **`gpt-realtime-whisper` is the natively streaming model designed for
  realtime sessions**; `gpt-4o-transcribe` is documented for higher-accuracy
  *non-streaming* workflows (community reports confirm it does not emit
  incremental deltas mid-speech). The streaming model exposes a `delay`
  setting (`minimal` … `xhigh`) trading transcript quality against lag.
- For `gpt-realtime-whisper` the docs recommend omitting `turn_detection`
  (set to `null`) and committing audio manually with
  `input_audio_buffer.commit`. This matches our design: the local VAD stays
  the single endpointing authority.
- Audio is sent as base64 chunks via `input_audio_buffer.append`; the commit
  acknowledgement carries the `item_id`; the final transcript arrives as
  `conversation.item.input_audio_transcription.completed`. Ordering of
  completion events across turns is **not guaranteed** — correlation must use
  `item_id`, never arrival order.

Because appended audio is processed while the player is still speaking, the
post-endpoint cost collapses to commit → completed (a few hundred
milliseconds) instead of upload + whole-clip inference.

## Goals

- Reduce warmed-path T_hear (last word → say applied) from today's 2–4.5 s to
  **≤ 1.2 s median, ≤ 2.0 s p95**, independent of utterance length.
- Overlap upload and transcription with the utterance itself: by the time the
  endpoint fires, the provider must already hold all but the final partial
  chunk of audio.
- Keep the local VAD as the only endpointing and gating authority. Audio
  leaves the machine only between voice-confirmed start (including pre-roll)
  and endpoint — never while idle, exactly as the batch path behaves today.
- Keep the existing batch pipeline byte-for-byte as the fallback, and keep
  the local (Canary-Qwen) backend and the `Z` selection semantics untouched.
- Prioritize the nearest LLM recipient's turn when a transcribed say is
  applied, so hearing is followed by the earliest possible reaction.
- Preserve all existing protocol strictness, resource bounds, credential
  hygiene, and offline testability.

## Non-goals

- Partial-transcript presentation (live captions). Deltas may be ignored;
  only the completed transcript drives the world. `transcription_result` and
  the status wire messages keep their current shapes.
- Replacing or retuning the VAD algorithm, echo cancellation, or the
  full-duplex microphone policy (`pause_microphone_during_npc_voice`).
  Spurious VAD triggers from NPC playback bleed behave exactly as today.
- New STT vendors, local streaming STT, or speech-to-speech cognition.
- Changing the batch upload format (the fallback WAV stays 32-bit float at
  device rate; shrinking it buys nothing on the streaming happy path).

## Design overview

```text
mic worker (Rust)                       sidecar (Python)                 OpenAI
─────────────────                       ────────────────                 ──────
VAD start (80 ms + 250 ms pre-roll)
  ├─ WAV writer (unchanged, fallback)
  └─ player_audio_begin ────────────────▶ ensure realtime session ──────▶ session.update
speech… resample 24 kHz s16, ~100 ms
  └─ player_audio_chunk (seq, b64) ─────▶ append by basename ───────────▶ input_audio_buffer.append
endpoint (trailing silence, 500 ms)
  ├─ finalize WAV
  ├─ player_audio_end ──────────────────▶ commit, remember item_id ─────▶ input_audio_buffer.commit
  └─ RecordingFinished → Bevy
Bevy: player_recording (unchanged) ─────▶ join transcript + position
                                          ◀──────────────────────────────  …transcription.completed
                                          say applied, prioritize hearer,
                                          transcription_result, cleanup
```

The correlation key for the whole feature is the existing `wav_basename`
(`player-recording-N.wav`): it is unique per utterance, already flows through
`player_recording` and `Discard`, and names the fallback artifact. No new
identifier is introduced and **`player_recording` keeps its exact current
schema** — the server treats it as "transcribe this utterance", preferring a
live streamed transcript for that basename and falling back to the file
otherwise.

## Rust capture changes

### Streaming tap in the microphone worker

The worker thread (not the CPAL callback) gains a streaming stage that runs
only while a recording is active and streaming is enabled for the current
utterance:

- **Resampling.** Convert mono `f32` at device rate to signed 16-bit little
  endian at 24 kHz with a phase-accumulator linear resampler (exact 2:1 for
  48 kHz, fractional for 44.1 kHz). No new crates; speech models are robust
  to linear resampling, and correctness is asserted by unit tests on sample
  counts and simple waveforms rather than audio fidelity.
- **Chunking.** Accumulate ~100 ms per chunk (2 400 samples, 4 800 bytes,
  6.4 KB base64). The pre-roll is flushed as the first chunk(s) so the
  provider hears the same audio the WAV contains. The final partial chunk is
  flushed before `player_audio_end`.
- **Transport.** The worker holds a cloned bounded `Sender<BridgeCommand>`
  (the audio-ack precedent). New commands `PlayerAudioBegin/Chunk/End/Abort`
  are enqueued with `try_send` and are **never allowed to block**: on a full
  queue the worker drops the stream for this utterance, marks it degraded,
  and stops sending chunks — the batch fallback covers it. At ~10 messages
  per second against a 128-slot queue this should never trigger in practice.
- **Ordering invariant.** All streaming commands for an utterance are
  enqueued before the worker emits `RecordingFinished`. Because Bevy sends
  `player_recording` through the same channel afterwards, the sidecar always
  observes begin → chunks → end → `player_recording` in order.

### Streaming gate

The worker does not know about `Z`/backend selection, so Bevy pushes the gate
down: a new `MicrophoneCommand::SetStreaming { enabled }` is sent whenever
the effective value of (cloud STT selected ∧ cloud STT available ∧ streaming
configured on) changes. Each utterance snapshots the gate at
`RecordingStarted`, mirroring how `RecordingContext.stt_backend` already
freezes the backend choice mid-flight. Local-backend utterances never stream.

### Utterance terminations

- **Normal end:** `player_audio_end { wav_basename, chunk_count, silent: false }`.
- **Silent discard** (voiced frames below minimum): the WAV is deleted
  locally as today and the worker sends `player_audio_end` with
  `silent: true`; the server clears the uncommitted buffer and forgets the
  utterance. No commit, no say.
- **Cancellation** (disable, shutdown, suspend, `Discard` of the active
  recording): the worker sends `player_audio_abort { wav_basename }`.
- **Bevy-side discards after finish** (stale context, resync barrier, missing
  player entity — the paths in `poll_microphone` that discard instead of
  sending `player_recording`): Bevy sends `player_audio_abort` for that
  basename so the server never holds a committed transcript forever.

### Endpointing constant

`TRAILING_SILENCE` drops from 700 ms to a configured default of **500 ms**
(the same figure OpenAI uses for its own server-VAD default), exposed as
`smart_actors.stt_trailing_silence_ms` in `config.ron` and validated to
300–1500 ms. This is a direct T_hear subtraction; the config knob exists
because more aggressive endpointing fragments deliberate mid-sentence pauses
into separate says, and that trade belongs to the player, not the code.
`START_CONFIRMATION`, pre-roll, and all VAD thresholds are unchanged.

## Bridge protocol additions

Protocol version stays 1. Four new client→server message types with strict
`_exact_payload` schemas:

```json
{"type": "player_audio_begin",
 "payload": {"wav_basename": "player-recording-7.wav",
             "sample_rate": 24000, "format": "pcm_s16le"}}

{"type": "player_audio_chunk",
 "payload": {"wav_basename": "player-recording-7.wav",
             "seq": 0, "pcm_s16le_base64": "…"}}

{"type": "player_audio_end",
 "payload": {"wav_basename": "player-recording-7.wav",
             "chunk_count": 34, "silent": false}}

{"type": "player_audio_abort",
 "payload": {"wav_basename": "player-recording-7.wav"}}
```

Validation rules, mirroring the outbound `tts_chunk` discipline:

- `sample_rate` must be exactly 24000 and `format` exactly `"pcm_s16le"`.
- `seq` must be contiguous from 0; a gap, duplicate, or chunk after `end`
  invalidates the stream (that utterance degrades to batch, never an error
  to the player).
- Per-chunk base64 is capped at 32 KB; chunks per utterance at 256 (15 s at
  100 ms plus pre-roll, with headroom); both caps also bound server memory.
- A `begin` for a basename that is already streaming replaces the old stream
  after clearing it. `abort` and `end` for unknown basenames are idempotent
  no-ops.
- These messages carry no `request_id` and produce no `command_result`
  (like `spatial_update`); failures surface only as `stt` status messages
  and, ultimately, through the unchanged `player_recording` request.

Server→client messages are unchanged. `player_recording` is unchanged.

## Sidecar design

### `RealtimeTranscriptionSession` (speech_client.py)

A new class owning one websocket to the realtime endpoint, following the
house worker pattern (lock, bounded status queue via `drain_status`, safe
teardown):

- **Dependency:** add `websocket-client` (synchronous) to the server's inline
  script deps. The connection runs in a dedicated daemon thread that parses
  server events and pushes them onto a bounded internal queue; the sidecar's
  20 ms `poll()` drains it. `handle_line` never blocks on the network:
  `append`/`commit`/`clear` are enqueued to the socket writer with a short
  bounded timeout, and any send failure marks the session dead.
- **Session config:** on connect, send `session.update` with
  `session.type: "transcription"`, `audio/pcm` at 24 kHz, `turn_detection:
  null`, model `STT_REALTIME_MODEL` (default `gpt-realtime-whisper`), delay
  `STT_REALTIME_DELAY` (default `low`), and `language` from `STT_LANGUAGE`
  when set. The API key comes from the existing env loading; it must never
  appear in statuses, logs, or wire messages.
- **Lifecycle:** connect lazily on the first `player_audio_begin`, keep the
  session warm across utterances (idle audio costs nothing since nothing is
  appended between utterances), close after `STT_STREAM_IDLE_CLOSE_S`
  (default 300 s) of no utterances, reconnect on demand with bounded backoff.
  A connect that races an in-progress utterance is fine: appends buffer
  locally until the socket is ready or the utterance's grace expires into
  batch fallback. Close cleanly on server shutdown without blocking exit.
- **Correlation:** on `player_audio_end` (non-silent) send
  `input_audio_buffer.commit`; record the `item_id` from the commit
  acknowledgement against the basename. Match
  `conversation.item.input_audio_transcription.completed` strictly by
  `item_id`. Support a small number of in-flight committed utterances
  (bounded at 4, matching the STT worker queue); overflow degrades the
  oldest to batch.

### Joining transcript and world position

`_handle_player_recording` gains one branch. When the basename has a live
streamed utterance:

- **Transcript already completed:** inject the result directly into the
  existing `_poll_transcriptions` result path (same request lifecycle,
  validation, `transcription_result`, frozen-position say, cleanup, statuses).
- **Committed, awaiting completion:** park the request with a grace deadline
  of `STT_STREAM_COMPLETION_GRACE_MS` (default 2000 ms), driven by the 20 ms
  poll loop. Completion within grace resolves it; expiry submits the WAV to
  the unchanged batch `_stt_worker` under the same `request_id`. A completion
  that arrives after its request was resolved is discarded (idempotent).
- **Stream already degraded/dead:** submit to batch immediately.

The reverse race — a transcript completing before `player_recording` arrives
(the Bevy hop is one frame) — is held for up to 5 s keyed by basename, then
dropped with buffer cleanup, covering a Bevy crash or a discard path whose
`player_audio_abort` was lost.

All existing transcript validation (500-char cap, control characters, empty
result) and WAV deletion stay exactly where they are: both paths converge in
`_poll_transcriptions`' result handling.

### Reaction priority

After a transcribed say is applied, call
`self.scheduler.prioritize(<nearest LLM-controlled recipient>)`, choosing by
the sim's stable distance-then-id ordering among the say's recipients; no-op
when there are none. This mirrors what targeted `debug_player_say` already
does and removes the round-robin wait between "heard" and "reacting".

### Statuses

Reuse the `stt` subsystem with `backend: "cloud"`: `connecting` on first
session bring-up, `transcribing` between begin and result (as today),
`degraded` with a bounded credential-safe message when a session dies and an
utterance falls back to batch, `idle` after each result. Fallbacks must be
visible but are not errors: the utterance still resolves.

## Configuration

`config.ron` (`smart_actors` block — behavior only, never secrets):

```ron
stt_streaming: true,            // stream cloud STT during capture
stt_trailing_silence_ms: 500,   // endpoint hangover, 300..=1500
```

`prompt_playgound/.env` (server side):

```text
STT_REALTIME_MODEL=gpt-realtime-whisper   # streaming model
STT_REALTIME_DELAY=low                    # minimal|low|medium|high|xhigh
STT_LANGUAGE=                             # optional hint, unset = auto
STT_STREAM_COMPLETION_GRACE_MS=2000
STT_STREAM_IDLE_CLOSE_S=300
STT_MODEL=gpt-4o-transcribe               # unchanged: batch fallback model
```

Fake mode (`--fake` / `SMART_ACTORS_FAKE_MODE`) never opens sockets:
streaming messages are accepted, validated, and resolved by the fake backend
so the offline suite exercises the full protocol.

## Cost and privacy

- Audio leaves the machine only during VAD-confirmed speech plus pre-roll and
  trailing window — the identical byte-span the batch path uploads today, so
  billed audio minutes are unchanged on the happy path.
- A degraded utterance is billed twice (streamed once, then batch retried
  once). This is bounded to a single batch attempt per utterance and only on
  failure; local mode is never a fallback target, and cloud streaming is
  never attempted when the local backend is selected.
- A warm idle websocket transfers no audio. Verify current realtime
  transcription pricing against batch at implementation time; the design
  does not depend on the exact figures.

## Failure handling

Report these stages distinctly and resolve every utterance exactly once:

- websocket connect/auth failure (streaming disabled until next utterance,
  batch used, `degraded` status once — not per chunk);
- mid-utterance socket death (remaining chunks dropped, batch fallback);
- commit acknowledged but completion beyond grace (batch fallback; late
  completion discarded);
- invalid provider events, oversized transcripts, malformed JSON (bounded
  safe messages, stream marked dead, batch fallback);
- chunk sequence violations or caps exceeded (stream invalidated, batch);
- `player_recording` for a basename whose stream and file are both gone
  (existing `missing_audio` error path, unchanged);
- abort/silent handling clears provider buffers so no orphaned commit can
  ever produce a say.

Provider exceptions become bounded, credential-free messages, as with TTS.

## Resource limits and cleanup

- Bounded everywhere: chunk size, chunks per utterance, in-flight committed
  utterances, internal event queue, status queue, socket write timeout.
- The WAV lifecycle is unchanged (created by the worker, deleted by the
  server after resolution or by `Discard`); streamed success deletes it at
  the same place batch success does.
- Session teardown on shutdown or bridge disconnect: clear pending
  utterances, close the socket with a bounded grace, never block Bevy exit.

## Instrumentation

One stderr line per resolved utterance (fake mode included), e.g.:

```text
[smart actors/stt] player-recording-7.wav: audio=3.42s path=stream
  endpoint->commit=21ms commit->transcript=243ms transcript->say=2ms
  endpoint->say=266ms
[smart actors/stt] player-recording-8.wav: audio=1.90s path=batch(fallback:socket)
  endpoint->say=2410ms
```

The acceptance numbers below are read from these lines; they are also the
regression tool for future latency work.

## Tests

### Python unit tests (injected fake websocket transport)

- Session config message matches the documented shape; the API key never
  appears in statuses or logs.
- Happy path: begin → chunks → end → commit → completed resolves through the
  existing result pipeline and deletes the WAV.
- Completion events matched by `item_id`, including out-of-order completions
  across two overlapping utterances.
- Grace expiry falls back to batch exactly once; a late completion after
  fallback is discarded without a second say.
- Socket death mid-utterance degrades that utterance and reconnects for the
  next; connect failure degrades without retry storms.
- Silent end and abort clear buffers and never commit or say.
- Transcript held awaiting `player_recording` expires and cleans up.
- Nearest-recipient prioritization after a transcribed say; no-op without
  LLM recipients.

### Protocol tests

- Strict schemas for the four new message types; unknown fields, bad
  sample rate/format, seq gaps, oversized chunks, and chunks after end are
  rejected or degrade exactly as specified.
- `player_recording` remains schema-identical and fully functional with no
  preceding stream (pure batch client).
- Fake mode resolves streamed utterances deterministically offline.

### Rust tests

- Resampler: exact 2:1 (48 k) and fractional (44.1 k) sample counts, DC and
  sine sanity, saturation on f32→s16.
- Chunker: 100 ms boundaries, pre-roll first, final partial flush,
  chunk_count correctness.
- Ordering invariant: all stream commands precede `RecordingFinished`.
- Full command queue degrades the stream without blocking or losing the WAV.
- Gate snapshots at `RecordingStarted`; mid-utterance toggles do not switch
  paths; local backend never streams.
- Every discard path emits `player_audio_abort` (or `silent` end) exactly
  once.
- `stt_trailing_silence_ms` is validated, clamped, and applied.

### Manual validation

1. Warm path: speak several utterances of varying length; verify
   `endpoint->say` stays flat (not growing with utterance length) and NPCs
   react on the next turn without round-robin wait.
2. Kill the network mid-sentence: utterance resolves via batch (if the
   network returns) or fails with today's error; the next utterance recovers.
3. Block the websocket entirely (firewall): every utterance transparently
   uses batch; one degraded status, no per-utterance error spam.
4. Switch to local STT with `Z`: no socket is ever opened; local behavior is
   byte-for-byte today's.
5. Leave the game idle past the session idle timeout, then speak: the
   reconnect overlaps the utterance and still resolves via stream or falls
   back within grace.
6. Speak past the 15 s cap and with sub-minimum utterances: cap commits
   normally, silent utterances neither commit nor say.

## Acceptance criteria

- With a warm session and residential-grade network, streamed utterances
  achieve **endpoint→say ≤ 700 ms median** and **T_hear (last word → say)
  ≤ 1.2 s median / ≤ 2.0 s p95**, independent of utterance length up to the
  15 s cap.
- No utterance ever resolves slower than the current batch path by more than
  the completion grace, and none resolves twice.
- Audio is transmitted only between VAD start (incl. pre-roll) and endpoint;
  an idle armed microphone transmits nothing.
- Local STT selection, fake mode, and the offline test suite
  (`uv run --offline --no-project python -m unittest discover -s tests`)
  work without network access.
- The addressed/nearest NPC's turn is prioritized after every transcribed
  say.
- No new work lands on the Bevy frame thread or the CPAL callback, and
  shutdown remains bounded.

## Measured results (2026-07-11, residential connection)

A 3.88 s synthesized utterance (24 kHz mono s16) streamed through
`RealtimeTranscriptionSession` against the live provider
(`gpt-realtime-whisper`, `delay=low`), exact transcript returned every run:

```text
cold session (connect + full upload after commit):  commit->transcript 2.151s
warm session, real-time-paced 100 ms chunks:        commit->transcript 0.623s
warm session, real-time-paced 100 ms chunks:        commit->transcript 0.784s
```

Warm-path in-game T_hear therefore lands at ≈ 0.5 s trailing silence +
0.6–0.8 s commit→transcript + ~0 s say-apply ≈ **1.1–1.3 s**, flat in
utterance length — against the previous batch pipeline's 0.7 s + upload +
1–3 s whole-clip inference (≈ 2.5–4.5 s, growing with length). Per-utterance
stderr timing lines (`[smart actors/stt] …`) are emitted for both paths for
in-game before/after readings.

## Suggested implementation order

1. Instrumentation first: per-utterance timing lines for the existing batch
   path, establishing the baseline the acceptance numbers are judged against.
2. `stt_trailing_silence_ms` config plumbing and the 500 ms default.
3. Protocol: the four message types with strict validation + fake-mode
   resolution and protocol tests (server accepts streams but still resolves
   via batch — safe to land alone).
4. Rust: resampler, chunker, worker tap, gate command, abort coverage, and
   the ordering invariant, feeding the now-accepting server.
5. `RealtimeTranscriptionSession` with injected-transport unit tests; wire
   `_handle_player_recording` joining, grace fallback, and cleanup.
6. Reaction priority for transcribed says.
7. Full offline suites, then the manual matrix against the live provider;
   record before/after timing lines in this file when moving it to
   `features/implemented/`.
