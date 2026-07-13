# Conversation floor: presentation-paced NPC turn-taking

Status: implemented on 2026-07-12.

Related: `speech_turn_taking.md` covers the *social* rule (who should answer an
overheard line); this feature covers the *temporal* rule (when the next line
may be spoken at all).

## Problem

With three LLM NPCs near each other, they "speak in each other's mouths":
reply bubbles pop up over one NPC's head while another NPC's voice is still
mid-sentence, voices play back-to-back with no gap, and the whole thing braids
into three simultaneous 2-way conversations instead of one conversation where
people speak one at a time.

The turn pipeline was gated on **text emission**, not on **audio completion**:

```
t0: prompt sent -> t1: LLM reply applied (text visible, inboxes filled)
                -> t2: TTS starts playing -> t3: TTS finishes
```

Listeners read the line at t1 and their reply could be applied — bubble and
all — long before t3.

## Why it happened

Three independent causes, found by tracing the full pipeline:

1. **The turn engine paced on LLM latency, not speech.** `NpcScheduler`
   (`prompt_playgound/scheduler.py`) is a strict global round-robin with one
   LLM request in flight; the next turn started `minimum_delay_seconds`
   (default 1.0 s) after the previous *result arrived*. One spoken line was
   produced every ~LLM-latency + 1 s, while each line takes 5–15 s to actually
   play.

2. **Presentation was already serialized, so the backlog grew without bound.**
   Bevy (`src/smart_actors/speech.rs`) plays exactly one NPC voice at a time
   (`active_voice` + ordered `audio_order`), but bubbles spawn the moment the
   speech event arrives and the audio queue drained ~3x slower than the world
   produced lines. Voice N played while bubbles N+1..N+3 were already up, and
   the drift compounded forever. Python had no idea when playback ended —
   `audio_consumed` fires at WAV *load* time (file cleanup), not playback end;
   no playback-completion signal existed anywhere in the protocol.

3. **Round-robin interleaved conversations.** Turn allocation was mechanical
   (Sven, Conny, Ilse, repeat) rather than conversational, so A->B, C->A,
   B->C threads braided together.

## Implemented solution

Three cooperating changes, all gating the **authority** (Python), never the
presentation. Pacing the world means speech events are not even emitted until
the previous line has been heard, so bubbles, subtitles, inbox delivery, and
audio all serialize for free and the mirror cannot drift.

### 1. The floor (talking stick)

An NPC's parsed LLM reply may not be **applied** to the world until the
previous utterance has been fully presented. Submission is deliberately NOT
gated: the next speaker's prompt is rendered as soon as the previous line is
applied (its inbox already has the full sentence), so it "thinks while
listening" and its reply lands right as the speaker finishes —
`max(0, llm_latency - audio_remaining)` extra wait, usually ~0.

Timeline: A's say applies at T0, audio plays T0->T3. B's turn submits at
~T0+1 s (prompt includes A's line). B's result arrives at T2 and is **held**.
At T3 the floor frees (+ a 0.5 s post-utterance beat) and B's actions apply.
C submits. Repeat.

**Scheduler side** (`scheduler.py`): `NpcScheduler` gains
`floor_busy: Callable[[], bool] | None`. When the completion worker yields a
result while `floor_busy()` is True, the result is stashed in
`self._held_result` and `_in_flight_actor_id` stays set (so no new turn can
start). On a later free poll the held result flows through the unchanged
staleness/parse/`apply_action` path — deferring application is safe precisely
because actions were already revalidated against the then-latest world at
application time. `_next_turn_at` is therefore naturally computed at
application time too.

**Server side** (`server.py`): `SmartActorServer` owns the floor and passes
`floor_busy=self._floor_busy` into the scheduler.

- Acquired in `_flush_domain_events` for every speech event whose speaker is
  LLM-controlled, at apply time. `_queue_tts` now returns whether a TTS task
  was actually submitted:
  - TTS queued -> `_floor_awaiting[event_id] = now + min(8.0 + len(text)/10,
    45.0)` (`FLOOR_AUDIO_FAILSAFE_MAX_SECONDS`). This is a *failsafe*
    deadline, generous enough to cover synthesis latency + playback; the real
    release comes from the client (below). The dict is bounded
    (`MAX_FLOOR_AWAITING = 32`, oldest dropped).
  - TTS not queued (player out of earshot, voices off/unavailable, queue
    full) -> `_floor_until = max(_floor_until, now + clamp(2 + len/15, 3,
    10))`, mirroring Bevy's `speech_text_seconds` bubble formula. Side
    effect: even NPC conversations the player cannot hear now pace at human
    speaking speed instead of LLM speed.
- Released by the new `speech_presented` message (below): pops the entry;
  when the awaiting set empties, `_floor_until = max(..., now + 0.5)`
  (`FLOOR_POST_UTTERANCE_BEAT_SECONDS`) so consecutive voices do not
  machine-gun. `_send_tts_failed` releases the same way. Failsafe expiry in
  `_floor_busy` releases without the beat. `close()` clears everything.
  Unknown/late/duplicate event ids are silently ignored (legitimate after a
  failsafe expiry). Nothing can freeze NPC turns permanently.

**New protocol message** (client -> server, fire-and-forget like
`spatial_update`, no `request_id` / no `command_result`):

```
speech_presented { "speech_event_id": "<id>" }
```

`protocol.py` needed no change (`parse_envelope` has no message-type
whitelist); `server.py` gained `_handle_speech_presented` with the usual
`_exact_payload` + `validated_id` validation.

**Bevy side** (`src/smart_actors/bridge.rs`, `speech.rs`):
`BridgeCommand::SpeechPresented { speech_event_id }` plus a best-effort
`notify_speech_presented(Option<&BridgeHandle>, &str)` helper (errors ignored
— the Python failsafe covers loss; `Option<Res<...>>` because speech.rs unit
tests build Apps without a bridge). Sent at **every** path that retires an
event's audio presentation:

- `start_ready_audio`: active-voice terminal states — natural finish (sink
  empty / entity despawned) and both timeout paths (sink never started,
  playback overran);
- `start_ready_audio` queue drops: stale expectation, clip-wait timeout,
  player out of hearing range (`speech_gain` == None), microphone-suspension
  failure, streaming-audio-output unavailable;
- `stop_npc_speech_for_capture`: the stopped active voice and every drained
  `audio_order` entry (player barge-in releases the floor immediately);
- `receive_tts_failures` / out-of-order PCM / stream-end mismatch: each
  expectation actually removed (overlaps with Python's own `tts_failed`
  handling; harmless because release is idempotent);
- `clear_speech_presentation` (disconnect) sends nothing — the bridge is gone.

### 2. Addressee-driven turn-taking

Real conversation allocates turns by "current speaker selects next speaker".
In `NpcScheduler.poll()`'s action-application loop, a **successfully applied**
`say {"target": ...}` whose target is an LLM-controlled character now calls
`self.prioritize(target)` — so the person spoken to answers next instead of
whoever round-robin lands on. Deliberately **not** `immediate=True`: only the
*selection* changes; the inter-turn delay and the floor still govern timing.

- `prioritize()` already rejects the player, unknown ids, and non-LLM actors,
  so the call is best-effort with the return ignored.
- The target is extracted defensively (`action_args.get("target")`, must be a
  str) even though a successful `say` implies sim-side validation.
- Multiple targeted says in one reply: the last one wins.
- Broadcast says and says to the player leave round-robin untouched.
- Round-robin remains the fallback whenever a turn ends in `wait`, and the
  prompt's existing wait-discipline (see `speech_turn_taking.md`) is what
  breaks a two-NPC ping-pong from starving the third.

Player-directed speech uses a stronger form of this behavior: the nearest
listener enters a protected FIFO reaction lane. Ordinary NPC handoffs cannot
overwrite it if a background reply and STT completion land in the same poll.

### 3. Full-duplex politeness: hold the floor while the player speaks

The game runs full-duplex (mic streams while NPCs talk), so an NPC turn could
previously be applied mid-player-sentence. `_floor_busy()` now also holds
while the player is mid-utterance or their transcript is in flight, via a
**rolling deadline** `_player_hold_until` — chosen over explicit state
tracking because it is failsafe by construction: a hung STT call or dead
client simply stops bumping it and it expires on its own.

- `player_audio_begin` / each accepted chunk: bump to now + 2.0 s
  (`FLOOR_PLAYER_CHUNK_HOLD_SECONDS`; chunks arrive continuously while the
  player speaks, utterances are client-capped at 15 s). Chunks to a
  degraded-but-live stream still bump (the player is still audibly speaking;
  the utterance lands via batch fallback), but trailing chunks after
  abort/silent-end cannot resurrect a cleared hold.
- `player_audio_end`, `silent=false`: bump to now + 3.0 s
  (`FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS`) — the transcript and resulting `say`
  normally land within that.
- `player_recording` taking ownership with transcription pending (batch
  submit or parked-for-stream): bump to now + 8.0 s
  (`FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS`).
- Cleared to 0.0 on: silent end, abort of a live stream, and every path
  through `_resolve_transcription` (success, failure, empty, invalid — all
  transcription outcomes funnel through it via
  `_handle_transcription_outcome`). On success the applied `say` queues the
  nearest listener as a protected player reaction; only player-audible floor
  holds govern its completed reply from there.
- Bumps are always `max(current, new)` so an out-of-order older bump cannot
  resurrect an explicit clear.

Combined with the floor, a player barge-in now behaves naturally end to end:
Bevy stops the current voice and drains the queue (each drained event sends
`speech_presented`, releasing the NPC floor) while the player hold keeps NPCs
quiet until the player's line has landed.

## Files touched

| File | Change |
| --- | --- |
| `prompt_playgound/scheduler.py` | `floor_busy` gate + held-result application deferral; addressee `prioritize` on applied targeted say |
| `prompt_playgound/server.py` | floor state/acquire/release, `speech_presented` handler, `_queue_tts -> bool`, player rolling hold |
| `src/smart_actors/bridge.rs` | `BridgeCommand::SpeechPresented` + wire encoding |
| `src/smart_actors/speech.rs` | `notify_speech_presented` helper; notification at every audio-retirement path; bridge handle became `Option<Res<...>>` in the affected systems |
| `prompt_playgound/tests/*` | new coverage (below) |

## Validation

Full suites pass after all three changes: **Python 142** (was 133 before this
work) and **Rust 144**, `cargo clippy --all-targets` clean.

New tests:

- Scheduler: a completed result is held while `floor_busy` is True with no
  new submission, and applies once free; targeted say hands the next turn to
  the addressee; broadcast say and say-to-player leave round-robin unchanged.
- Server floor: hold acquired for a voiced NPC say and released by a
  `speech_presented` envelope (+ post-beat); failsafe expiry releases;
  text-only speech holds for the reading estimate; `tts_failed` releases;
  player speech never acquires the NPC floor.
- Player hold: streamed audio holds and expires unaided; silent end and abort
  release immediately (and a trailing chunk cannot resurrect the hold); a
  completed fake-mode transcription clears it; batch-failure clears it; an
  NPC reply finished during player speech is observably held and applied on
  the first poll after the hold clears.
- Rust: `SpeechPresented` wire-shape test; existing speech.rs unit tests keep
  running without a bridge handle.

Two existing tests were adjusted, with comments in place:

- `bridge.rs::fake_sidecar_runs_scripted_exchange_offline_and_cleans_up` now
  sends `speech_presented` when it observes Ilse's speech — that
  protocol-level test has no presentation layer, so the ~11 s audio failsafe
  would otherwise exceed its 8 s coin-offer deadline (it now also exercises
  the new message end to end against the real fake sidecar).
- `tests/test_end_to_end.py`: one 2 s wait became 8 s because the text-only
  reading-estimate floor legitimately paces Ilse's offering turn ~4 s behind
  her spoken reply.

The headless plugin integration test passed unchanged: with no real audio
device the sink-start timeout (2 s) fires and sends `speech_presented`, so
the floor adds ~2 s per line there and stays inside the existing deadlines.

## Tuning knobs and known follow-ups

- Feel constants worth tuning in a live run:
  `FLOOR_POST_UTTERANCE_BEAT_SECONDS` (0.5) and
  `FLOOR_PLAYER_CHUNK_HOLD_SECONDS` (2.0).
- A held NPC reply does not account for anything said *while* it was held
  (e.g. a player barge-in between its submission and application). First
  version applies it anyway — humans also commit to replies — and the new
  line arrives in the NPC's inbox for its next turn. Discard-and-resubmit on
  fresh speech inbox entries is the obvious refinement if this reads badly.
- Bubbles still spawn when the speech event arrives, a few hundred ms before
  the voice's first PCM chunk (synthesis latency). Much less jarring now that
  events themselves are paced; delaying bubble spawn until audio actually
  starts is optional polish.
- Ordinary NPC pacing is still global. Player-audible and background holds are
  distinguished, however: a protected response to fresh player speech ignores
  an inaudible background hold. Fully independent NPC conversation groups
  (connected components of the 20 m hearing graph) remain a possible follow-up.
