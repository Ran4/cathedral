# Selectable cloud and local NPC voice audio

Status: implemented, including the later sub-300 ms streaming requirement.

## Problem

NPC dialogue is always presented as text in speech bubbles and the subtitle
HUD. Players should also be able to hear that dialogue, while retaining the
text path for accessibility and for graceful degradation.

As with player speech transcription, NPC text-to-speech (TTS) must support two
explicitly selectable modes:

- a cloud backend with usage costs; and
- a local backend with no per-utterance cost.

Changing TTS backends must not change simulation semantics. The Python world
continues to decide what an NPC says, and Bevy continues to decide whether and
how the player hears it.

## Existing foundation

The repository already contains an end-to-end cloud TTS path:

1. `speech_client.OpenAISpeechBackend` synthesizes NPC text with OpenAI
   `tts-1` and writes a WAV file.
2. `SmartActorServer` queues synthesis on its dedicated TTS worker and emits
   `tts_ready` when the completed file is available.
3. The Rust bridge confines and validates the runtime path, copies the WAV
   bytes, acknowledges consumption, and asks Python to remove the temporary
   file.
4. `speech.rs` pairs the clip with its speech event, preserves dialogue order,
   suspends microphone capture, and plays the clip as spatial Bevy audio from
   the NPC's position with distance attenuation.
5. Missing or late audio never removes the complete text presentation.

The missing pieces are a dedicated TTS provider abstraction, a local TTS
implementation, runtime backend selection, and sufficiently precise status
reporting to diagnose why a clip was not heard.

## Goals

- Hear every NPC line that the player is a recipient of and that arrives while
  the NPC is within the existing 20 metre hearing radius.
- Keep speech bubbles and subtitles as the authoritative, always-available
  presentation.
- Support `cloud`, `local`, and `off` TTS modes.
- Give each NPC a stable, distinct voice in every backend.
- Allow switching backends at runtime, similarly to the `Z` cloud/local STT
  toggle.
- Keep model loading, network requests, file I/O, and audio decoding off the
  Bevy frame thread.
- In warmed local mode, deliver the first PCM chunk in under 300 ms rather
  than waiting for a complete utterance file.
- Make first-use downloads, loading, synthesis, timeouts, and failures visible
  in the HUD without exposing credentials or unsafe provider responses.
- Never make a paid cloud request as an implicit fallback from local mode.

## Non-goals

- Speech-to-speech cognition or replacing the NPC's text-producing LLM call.
- Streaming partial LLM output into partial audio.
- Overlapping several NPC voices. Dialogue remains serialized for
  intelligibility and deterministic subtitle/audio ordering.
- Voice cloning in the first implementation.
- Lip synchronization or facial animation.
- Removing speech bubbles or subtitles when audio is enabled.

## Backend choices

### Cloud: OpenAI

Keep the existing OpenAI Speech API backend as the initial cloud provider. Its
current `tts-1` default already produces WAV output accepted by Bevy and is
documented as the speed-oriented TTS model:
<https://developers.openai.com/api/docs/models/tts-1>.

An ElevenLabs adapter may be added later without changing the protocol or
Bevy presentation path. Its streaming TTS endpoint and provider voice IDs are
documented at:
<https://elevenlabs.io/docs/api-reference/text-to-speech/stream>.

### Local: Pocket TTS

The implemented local backend is Kyutai Pocket TTS. It is CPU-oriented,
streams 24 kHz audio incrementally, and documents roughly 200 ms latency to
its first audio chunk on two CPU cores:
<https://kyutai-labs.github.io/pocket-tts/>.

Run Pocket TTS in an isolated, persistent `uv` script worker, following the
same lifecycle pattern as `canary_qwen_worker.py`:

- warm at game startup whenever local TTS is selected;
- let `uv` own the separate Python dependency environment;
- download model weights on first use and reuse the local cache thereafter;
- keep the loaded model and NPC voice states resident for later utterances;
- use stdin/stdout JSON lines as the machine-readable channel;
- flush each signed 16-bit PCM chunk as soon as the model produces it;
- forward human-readable dependency and model diagnostics through stderr; and
- shut down without allowing a stuck model worker to freeze the game.

Piper is a possible CPU-oriented alternative, but its current canonical
implementation is GPL-3.0. It should not become the distributed default
without an explicit licensing decision:
<https://github.com/OHF-voice/piper1-gpl>.

## Python backend design

Separate transcription and synthesis interfaces. The current combined
`SpeechBackend` makes cloud STT availability and TTS selection unnecessarily
coupled.

Conceptually:

```python
class TranscriptionBackend(Protocol):
    @property
    def stt_available(self) -> bool: ...

    def transcribe(self, wav_path: Path) -> str: ...


class TtsBackend(Protocol):
    name: str

    @property
    def tts_available(self) -> bool: ...

    def synthesize(
        self,
        text: str,
        voice_key: str,
        output_wav: Path,
    ) -> None: ...
```

The active implementations are `OpenAITranscriptionBackend`,
`OpenAITtsBackend`, and `PocketTtsBackend`. Tests may continue to inject fake
backends independently.

The server owns both TTS backend instances plus the currently selected mode.
Each `_TtsTask` captures the selected backend when it is enqueued. A later
toggle therefore affects only new speech and cannot redirect an in-flight
task.

Local synthesis uses a persistent wrapper in the sidecar and a separate model
process. The wrapper serializes access, assigns request IDs, validates
responses, publishes bounded status messages, and restarts cleanly after a
dead worker. The model process receives validated text and a logical voice
key, then returns sequential base64-encoded mono PCM chunks followed by a
completion containing the chunk count and measured first-chunk latency.

## Logical NPC voices

`Character.voice_key` remains provider-neutral. The simulation stores values
such as `sven`, `conny`, and `ilse`, not OpenAI or Pocket voice IDs.

Each provider resolves that logical key separately. Initial mappings can be:

| NPC key | OpenAI | Pocket TTS |
| --- | --- | --- |
| `sven` | `onyx` | `michael` |
| `conny` | `echo` | `george` |
| `ilse` | `nova` | `alba` |

Allow environment overrides with provider-qualified names, for example:

```text
TTS_OPENAI_VOICE_SVEN=onyx
TTS_POCKET_VOICE_SVEN=michael
```

Invalid or unknown mappings fail that utterance safely and leave its text
visible.

## Protocol and runtime state

Replace the single undifferentiated TTS capability with explicit backend
capabilities while retaining a derived aggregate for presentation logic:

```json
{
  "tts": true,
  "tts_cloud": true,
  "tts_local": true,
  "tts_selected": "local"
}
```

Add a strict client command such as:

```json
{
  "type": "set_tts_backend",
  "payload": {"backend": "cloud"}
}
```

Accepted values are exactly `cloud`, `local`, and `off`. The server validates
availability and replies with the normal command-result mechanism. Rust only
commits the displayed selection after acknowledgement; a rejected change
leaves the prior mode active.

`SmartActorRuntime` tracks the two capabilities and the acknowledged selected
mode. `PresentSpeech.expect_audio` is true only when the selected backend is
available and the speaker is an NPC. Existing event IDs remain the sole key
joining text, TTS output, and playback; no provider identifiers need to cross
the audio protocol boundary.

## Controls and configuration

Add `X` as the NPC voice mode control because it is currently unused:

```text
CLOUD -> LOCAL -> OFF -> CLOUD
```

Unavailable modes are skipped. If only one backend is available, `X` toggles
between that backend and `OFF`. The HUD should show both the current selection
and transient state, for example:

```text
NPC VOICES: POCKET TTS
Loading local voice model…
Preparing Ilse's voice…
NPC voice failed; text remains available
```

Add an explicit startup setting rather than inferring a paid fallback from
credentials:

```ron
smart_actors: (
    // existing settings...
    tts_backend: "local",
    // Set false when using headphones or external echo cancellation.
    pause_microphone_during_npc_voice: true,
)
```

If the configured mode is unavailable, prefer another free local backend only
when explicitly configured to do so; otherwise start in `off` and explain the
unavailable selection in the HUD. Never silently select cloud because an API
key happens to exist.

Environment variables continue to hold provider secrets, model overrides, and
per-provider voice mappings. Secrets must not be added to `config.ron`, the
wire protocol, status messages, or logs.

## Presentation and ordering

Cloud speech retains the complete-WAV pipeline. Local speech uses a distinct
streaming pipeline so playback does not wait for utterance completion:

```text
NPC speech event
  -> bubble/subtitle immediately
  -> persistent Pocket TTS worker
  -> tts_chunk (mono 24 kHz signed 16-bit PCM, flushed immediately)
  -> Rust validates sequence/format and feeds a custom Bevy audio source
  -> spatial playback begins from chunk 0
  -> tts_stream_end closes the source after its buffer drains
```

Playback stays globally serialized. Generated clips may complete out of
order, but `SpeechPresentationState.audio_order` waits for the corresponding
front subtitle and discards stale clips after a bounded timeout.

Associate each speech bubble with its event ID. When audio is expected, the
bubble remains visible until all of the following are true:

- its minimum readable duration has elapsed;
- its audio is no longer playing; and
- its audio is no longer waiting within the allowed TTS window.

This brings world bubbles in line with the existing subtitle queue instead of
allowing a delayed clip to outlive its bubble.

## Failure handling and diagnostics

Text presentation is never conditional on TTS success. Failures affect only
the optional clip associated with that event.

Report these stages distinctly:

- backend unavailable or unconfigured;
- local dependency/model download;
- local model loading;
- queued and synthesizing;
- provider timeout or rejected request;
- invalid, missing, oversized, late, or out-of-order audio data;
- bridge file-copy or acknowledgement failure;
- microphone-suspension failure; and
- audio-sink start or playback timeout.

Provider exceptions must be converted to bounded, credential-safe messages.
The current generic `speech synthesis failed` status is insufficient for
distinguishing an API problem from an audio-device problem. Logs may include a
safe exception type and provider request identifier, but never headers, API
keys, complete responses, or arbitrary provider HTML.

Local failure does not trigger cloud synthesis. Cloud failure does not trigger
a second charged request or an automatic local retry. A user may switch modes
and future utterances use the newly selected backend.

When `pause_microphone_during_npc_voice` is enabled and microphone suspension
cannot be acknowledged, do not play the NPC clip into an armed microphone.
Discard the optional clip, resume a consistent mic state, retain the text, and
report the specific reason. When disabled, capture and NPC playback are fully
independent; callers should use headphones or acoustic echo cancellation to
avoid transcribing speaker output as player speech.

## Resource limits and cleanup

- Retain the existing 500-character input limit, 16 MiB cloud WAV limit, and
  bounded per-chunk PCM limit.
- Keep the TTS request queue bounded.
- Permit only one active synthesis request per local model process.
- Validate WAV headers for cloud audio and strict mono PCM metadata, base64,
  size, and sequence for local audio.
- Keep all generated and temporary files inside the per-session runtime
  directory.
- Remove partial output after failure and generated output after
  `audio_consumed`.
- On disconnect or shutdown, cancel queued work, terminate the local worker
  with a bounded grace period, remove session audio, and never block Bevy
  shutdown indefinitely.

A bounded content-addressed cache may be added later. It must include backend,
model version, voice mapping, synthesis settings, and text in the cache key.
It is not required for the initial implementation.

## Tests

### Python unit tests

- Cloud and local TTS availability are independent of STT availability.
- Logical voice keys resolve through the selected provider's mapping.
- Invalid text, voice keys, output paths, and worker responses are rejected.
- A local worker starts lazily and is reused across utterances.
- A dead local worker becomes a safe, bounded `SpeechUnavailable` failure.
- Switching modes affects new tasks but not already queued tasks.
- Local mode never invokes the cloud fake and vice versa.
- `off` mode never queues synthesis.
- A successful local worker produces ordered `tts_chunk` events followed by
  `tts_stream_end`; failure preserves the speech event.
- Temporary and generated files are removed on failure, acknowledgement, and
  shutdown.

### Protocol tests

- `ready.capabilities` includes strict `tts_cloud`, `tts_local`, and
  `tts_selected` fields.
- `set_tts_backend` accepts only the three documented values.
- Selecting an unavailable backend is rejected without changing server state.
- Duplicate commands remain idempotent under the existing message-ID rules.
- Malformed capability or command payloads cannot partially update Rust
  runtime state.

### Rust tests

- `X` cycles through only the available TTS modes and updates after server
  acknowledgement.
- NPC speech expects audio only when the selected backend is usable.
- Ready clips remain matched by event ID and play in speech order.
- Spatial gain retains the existing 3–20 metre behavior.
- Invalid and oversized WAVs are ignored.
- Invalid, oversized, or out-of-order PCM chunks release the optional audio
  wait while retaining text.
- Late clips, sink-start timeouts, playback timeouts, and microphone suspension
  failures release their queue entries and keep text presentation live.
- Bubble lifetime follows the same event/audio state as the subtitle.
- Disconnect clears transient audio, bubble, selection-pending, and microphone
  suspension state.

### Manual validation

1. Start with local TTS selected and no provider credentials. Trigger one line
   from each NPC; observe first-use model status and hear three distinct
   spatial voices.
2. Restart and verify that the cached model starts without a full download.
3. Switch to cloud and hear subsequent, but not already queued, lines use the
   cloud voices.
4. Switch to off and verify that dialogue remains immediate and text-only.
5. Disable network access in cloud mode; verify a precise, safe error and
   uninterrupted text dialogue.
6. Break or terminate the local worker; verify bounded recovery, no cloud
   request, and no game-thread stall.
7. Speak while NPC audio is queued and verify that microphone suspension and
   resumption prevent feedback without leaving the mic disabled.
8. Walk from near an NPC to beyond 20 metres while a clip is pending and verify
   the existing distance policy is applied consistently.

## Acceptance criteria

- With local mode selected, NPC dialogue is audible after first-use setup with
  no cloud TTS request or per-utterance fee.
- Once Pocket TTS is warm, its measured time to first PCM chunk is under
  300 ms on a machine meeting Pocket TTS's documented CPU expectations.
- With cloud mode selected and valid credentials, NPC dialogue is audible
  through the configured paid provider.
- `X` switches modes without restarting the game.
- Sven, Conny, and Ilse have stable and perceptibly distinct voices in both
  initial backends.
- Audio is spatial, distance-attenuated, serialized, and paired with the
  correct bubble/subtitle.
- Text remains complete and readable through every backend, network, model,
  bridge, microphone, and playback failure.
- Local mode can never incur a cloud TTS charge through implicit fallback.
- No synthesis, model, file, subprocess, or audio operation blocks the Bevy
  frame thread or indefinite app shutdown.

## Suggested implementation order

1. Improve stage-specific diagnostics for the existing OpenAI-to-Bevy path and
   verify that paid TTS works end to end.
2. Split the Python STT and TTS protocols without changing behavior.
3. Add strict TTS capability fields and acknowledged runtime selection.
4. Add the persistent streaming Pocket TTS `uv` worker and local backend tests.
5. Add the `X` control, HUD state, and startup configuration.
6. Tie speech-bubble lifetime to audio event state.
7. Run the full offline fake-backend suites and the manual cloud/local matrix.
