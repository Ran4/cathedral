# Quicker nearby NPC responses

Status: in progress. Cloud TTS streaming and the 400 ms fixed endpoint fallback
are implemented; smarter endpointing, foreground cognition, and LLM streaming
remain design/work items.

## Goal

Optimize the common case: the player is standing near one NPC, says one short
sentence, and expects that NPC to begin answering promptly.

The useful metric is not provider completion time. It is:

```text
last player voice -> first audible NPC PCM
```

Text/subtitle availability and final playback completion are supporting metrics.
Background city simulation must not create surprising head-of-line delay in this
foreground exchange.

## Actual pipeline

| Stage | Current behavior |
| --- | --- |
| Player begins speaking | The microphone's adaptive energy detector confirms voice and retains 250 ms pre-roll. With cloud streaming enabled, captured audio is sent to realtime STT while the player is still speaking. |
| Player stops speaking | The local acoustic detector waits for the configured continuous non-voice interval. This pass changes the fixed/default interval from 500 to 400 ms. |
| STT resolves | Cloud realtime STT manually commits the already-streamed audio and returns one final transcript. It does not expose partial words to the simulation. Local STT and cloud fallback process the complete WAV in batch. |
| Player `say` is applied | The final transcript is validated, applied at the frozen speaking position, and delivered to nearby actors. The nearest LLM listener gets a protected reaction slot. |
| NPC cognition starts | If the single cognition worker is idle, the full character/world prompt is submitted immediately. If an unrelated city request is already running, the protected reaction waits for that request to complete. |
| NPC cognition resolves | The current chat-completions client buffers and decodes the whole response. The scheduler then parses complete `VERB {json}` lines and validates/applies their actions. |
| NPC voice starts | A validated `say` sends its complete text to the selected TTS backend. Pocket TTS and, after this pass, the normal cloud PCM16 WAV path both send ordered PCM chunks to Bevy as they are synthesized. Playback can start at chunk zero. |
| NPC voice finishes | A keyed stream end closes the dynamic audio source after buffered samples drain, then the presentation acknowledgement releases the conversation floor. |

## Measured baseline

Session 76 provides a useful baseline before these improvements:

- Cloud realtime STT committed at the detected endpoint in 0 ms.
- Commit to final transcript took 613-879 ms.
- Transcript to authoritative player `say` took 0 ms.
- LLM calls in the surrounding conversation generally took about 0.8-2.5 s;
  the relevant Gile answer took 2.467 s.
- The old logs begin at the detected endpoint, so the former fixed 500 ms
  endpoint wait is absent from those numbers.
- Cloud TTS had no first-PCM measurement because it collected the complete WAV
  before Bevy saw any audio.

The new path reports first-PCM latency for both cloud and local streaming voices.

## Implemented first: progressive cloud TTS

The cloud Speech API already transfers the generated WAV incrementally, using
`0xFFFFFFFF` placeholders for the unknown RIFF and data lengths. The old
in-process client called `response.bytes()` and waited for EOF before repairing
the sizes and sending a complete clip to Bevy.

The client now:

1. consumes the HTTP response body as a byte stream;
2. incrementally parses the RIFF and `fmt`/`data` headers;
3. selects streaming only for the provider's open-ended PCM16 WAV shape;
4. validates mono/stereo, 8-48 kHz, 16-bit PCM and block alignment before
   releasing samples;
5. downmixes stereo to mono and emits bounded, contiguous `i16` chunks;
6. sends chunk zero immediately, before response EOF;
7. runs the existing complete-WAV sanity gate at EOF and sends a keyed stream
   end; and
8. retains the former complete-WAV path for honest finite WAVs and unusual but
   otherwise supported formats.

Transport or validation failure after partial delivery follows the existing
stream-failure behavior: finish the partial source, release the keyed floor,
retain the complete text, and show a bounded failure message. Pocket's deliberate
1.05 playback multiplier remains local-only; cloud PCM plays at provider speed.

Regression coverage pins early incremental decoding, ordered chunk/end events,
provider identity through the stream, finite-WAV compatibility, invalid-audio
failure, and existing presentation ordering/floor behavior.

## Endpointing: VAD is not turn completion

The current detector is adaptive, but it is not a learned conversational
endpoint model. It learns the ambient noise floor and classifies each microphone
buffer from RMS/peak energy. Once speech is active, every non-voice buffer adds
to `silent_frames`; reaching the configured duration ends the utterance.

Modern acoustic VAD can improve speech/noise classification, but it still needs
a silence hangover. It cannot know whether silence follows a finished sentence
or a mid-thought pause. The smarter layer is semantic endpointing/end-of-turn
detection.

The cloud realtime transcription session deliberately sets `turn_detection` to
`null` and commits manually. OpenAI documents provider `server_vad` and
`semantic_vad`, but its current realtime transcription guidance for the model
used here calls for manual commit. Enabling provider VAD is therefore not a safe
configuration flip; it would also give cloud and local STT different endpoint
semantics and require a new correlation state machine.

Relevant primary references:

- [OpenAI Realtime VAD guide](https://developers.openai.com/api/docs/guides/realtime-vad)
- [OpenAI Realtime transcription guide](https://developers.openai.com/api/docs/guides/realtime-transcription)
- [Silero VAD](https://github.com/snakers4/silero-vad)
- [Pipecat Smart Turn](https://github.com/pipecat-ai/smart-turn)
- [Pipecat Smart Turn design](https://docs.pipecat.ai/api-reference/server/utilities/turn-detection/smart-turn-overview)

### Recommended local hybrid

Keep endpoint authority local and use two stages:

1. Keep the existing acoustic detector initially. Replace it with Silero only
   if recorded evidence shows noise/speech classification is the problem.
2. After about 200 ms continuous acoustic silence, submit the current utterance
   to a bounded, asynchronous local end-of-turn classifier.
3. If it says complete, finish immediately.
4. If it says incomplete, keep listening until speech resumes or a hard
   1.0-1.2 s silence ceiling.
5. If the classifier is absent, overloaded, slow, or failed, fall back to the
   fixed 400 ms endpoint.
6. If speech resumes while inference is running, discard the stale result by
   utterance ID.

Conceptual configuration:

```ron
stt_endpointing_mode: "hybrid",
stt_trailing_silence_ms: 400,       // deterministic failure fallback
stt_endpoint_candidate_ms: 200,
stt_endpoint_max_silence_ms: 1100,
```

A small quantized Smart Turn-style classifier could reduce clean endpoint time
to roughly 200 ms plus inference while deliberately waiting longer through
hesitations. Model language coverage must be evaluated against actual player
languages; the currently documented Smart Turn language list does not include
Swedish.

Changing 500 to 400 ms remains worthwhile as interim latency tuning: it saves a
deterministic 100 ms on clean turns. It is not smarter endpointing, and it will
split pauses between 400 and 500 ms that previously remained one utterance.

## Foreground cognition lane

Partly addressed by `features/implemented/gate_idle_cognition_on_proximity.md`
§4, which took the cheap first move: no background turn *starts* while the player
is composing (microphone hot, STT in flight, or inside the router's grace
window), so "nothing was in flight when your words landed" is now the common
case. What follows remains the fallback for the turns that were already out —
measure before building it.

The protected reaction queue prevents player speech from being overwritten, but
cognition still has one global in-flight request. A background actor that began
thinking just before the transcript lands can add the remainder of its provider
call to foreground latency.

Evaluate one of these policies:

- cancel a background request and restore its drained percepts when a protected
  player reaction arrives;
- reserve a separate capacity-one foreground cognition lane; or
- allow two provider calls only while one is a protected player reaction.

Cancellation is cheapest but provider cancellation/cost accounting and prompt
archive semantics must be explicit. A reserved lane is simpler semantically but
permits controlled concurrency and can reorder background completions.

## Streaming LLM output into TTS

The cognition HTTP request currently waits for a complete chat-completions JSON
response. Streaming can overlap generation with later work, but raw tokens
cannot be sent directly to TTS. Model output is an action program, for example:

```text
say {"target":"player","text":"The stair is by the bridge."}
remember {"memory":"I warned the traveler about the stair"}
```

Action syntax, target IDs, JSON escapes, comments, and non-speech verbs must not
be spoken.

Two implementation levels are possible:

1. **Complete-action streaming (safe, modest benefit).** Parse provider SSE and
   apply a `say` as soon as its entire JSON line validates. This can overlap TTS
   with later actions, but a one-line trivial reply closes near the end of the
   response and therefore saves little.
2. **Clause streaming (fast, invasive).** Incrementally decode the `text` JSON
   string, buffer to a safe clause boundary, and synthesize segments while the
   model continues. This improves time-to-first-audio, but already-spoken text
   cannot be rolled back if the remaining action is invalid. It also requires
   gap buffering, prosody evaluation, subtitle/history assembly as one logical
   speech event, and segmented TTS support.

Instrument streaming before choosing the aggressive form. Record provider
time-to-first-token, time-to-first-valid-action, time-to-first-safe-clause, TTS
submission, first PCM, and playback start. If the first valid clause arrives
almost with EOF for the configured model, complexity will not buy useful time.

## Recommended work order

1. **Done:** stream the normal cloud TTS WAV into Bevy PCM playback.
2. **Done as interim tuning:** reduce the deterministic endpoint fallback from
   500 to 400 ms.
3. Add end-to-end latency instrumentation, especially `last_voice->endpoint`
   and `last_voice->first_npc_pcm`.
4. Remove foreground head-of-line blocking with an explicit cognition policy.
5. Prototype and evaluate local hybrid semantic endpointing on retained,
   opt-in recordings.
6. Add LLM SSE plus complete-action parsing and measure the real overlap.
7. Consider clause-to-TTS streaming only if those measurements justify its
   semantic and presentation complexity.

## Evaluation and acceptance

Build an opt-in local evaluation set containing short complete commands,
200-1200 ms mid-sentence pauses, fillers, trailing conjunctions,
self-corrections, quiet speech, breaths, keyboard/walking noise, changing
ambience, NPC playback leakage, English, and every intended player language.

Record:

- last acoustic voice frame;
- first endpoint candidate;
- endpoint classifier request/result and inference duration;
- endpoint source (`fixed`, `semantic_complete`, `semantic_timeout`,
  `max_duration`, or `failure_fallback`);
- endpoint to final transcript and transcript to applied `say`;
- player `say` to NPC cognition submit;
- cognition time-to-first-token and completion;
- TTS submit to first PCM and playback start; and
- speech resuming soon after an endpoint as a possible false split.

Acceptance should consider premature-split rate, excessively late endpoints,
missed speech, STT quality, response correctness, audio underruns/gaps, and
median/p95 last-voice-to-first-NPC-audio latency. Worker-unavailable,
slow-worker, stale-result, queue-overload, network failure, partial cloud audio,
and backend-switch-in-flight paths must continue degrading to complete text
without stalling the conversation floor.
