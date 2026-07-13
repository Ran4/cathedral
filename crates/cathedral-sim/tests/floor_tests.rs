//! Conversation floor arbitration (`test_protocol_server.py::ConversationFloorTests`
//! 58-62 and `PlayerFloorHoldTests` 63-68).
//!
//! The Python tests drive the floor through a whole server (speech events, mic
//! stream messages, a fake TTS backend); the floor logic they are actually
//! pinning is pure and clock-stepped, so here it is exercised directly. The
//! triggers — which event bumps which hold — belong to the engine and the speech
//! router, and are tested there (P4/P6).

mod prompt_support;

use cathedral_sim::{
    Cognition, ConversationFloor, FLOOR_PLAYER_CHUNK_HOLD_SECONDS,
    FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS, FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS,
    FLOOR_POST_UTTERANCE_BEAT_SECONDS, FakeCognition, MAX_FLOOR_AWAITING, NpcScheduler,
    SpeechEventId, floor_audio_failsafe_seconds, llm_turn_order, speech_reading_seconds,
};
use prompt_support::{prompt_env, seed_world};

fn event(sequence: u32) -> SpeechEventId {
    SpeechEventId(format!("speech-{sequence}"))
}

/// 58. `test_queued_tts_holds_the_floor_until_speech_presented`
#[test]
fn queued_tts_holds_the_floor_until_speech_presented() {
    let mut floor = ConversationFloor::new();
    let mut now = 1_000.0;
    let line = event(1);

    floor.acquire(now, &line, "Hold the floor", true);
    assert!(floor.is_awaiting(&line));
    assert!(floor.busy(now));

    floor.release(now, &line);
    assert_eq!(floor.awaiting_len(), 0);
    // The post-utterance beat still separates consecutive voices.
    assert!(floor.busy(now));

    now += FLOOR_POST_UTTERANCE_BEAT_SECONDS + 0.01;
    assert!(!floor.busy(now));

    // Fire-and-forget: a late duplicate ack is a no-op and must not re-arm the
    // beat (D26).
    floor.release(now, &line);
    assert!(!floor.busy(now));
}

/// 59. `test_failsafe_deadline_frees_a_lost_presentation`
#[test]
fn failsafe_deadline_frees_a_lost_presentation() {
    let mut floor = ConversationFloor::new();
    let mut now = 1_000.0;
    let line = event(1);
    let text = "x".repeat(20);

    floor.acquire(now, &line, &text, true);
    assert!(floor.is_awaiting(&line));

    let failsafe = 8.0 + 20.0 / 10.0;
    assert_eq!(floor_audio_failsafe_seconds(&text), failsafe);

    now += failsafe - 0.01;
    assert!(floor.busy(now));

    now += 0.02;
    // Expiry releases without the post-utterance beat: the line is stale, and a
    // lost `speech_presented` must never stall NPC turns.
    assert!(!floor.busy(now));
    assert_eq!(floor.awaiting_len(), 0);
}

/// 60. `test_text_only_speech_holds_for_the_reading_estimate`
#[test]
fn text_only_speech_holds_for_the_reading_estimate() {
    let mut floor = ConversationFloor::new();
    let mut now = 1_000.0;
    let text = "x".repeat(30); // 2 + 30/15 = 4 seconds
    assert_eq!(speech_reading_seconds(&text), 4.0);

    floor.acquire(now, &event(1), &text, false);
    // Nothing to await: no audio will ever be presented.
    assert_eq!(floor.awaiting_len(), 0);
    assert!(floor.busy(now));

    now += 3.99;
    assert!(floor.busy(now));
    now += 0.02;
    assert!(!floor.busy(now));
}

/// 61. `test_tts_failure_releases_the_awaited_floor_with_the_beat`
#[test]
fn tts_failure_releases_the_awaited_floor_with_the_beat() {
    let mut floor = ConversationFloor::new();
    let mut now = 1_000.0;
    let line = event(1);

    floor.acquire(now, &line, "Never synthesized", true);
    assert!(floor.is_awaiting(&line));

    // Synthesis failed: the engine releases the entry (the text was already
    // delivered, so the line still gets its beat).
    floor.release(now, &line);
    assert_eq!(floor.awaiting_len(), 0);
    assert!(floor.busy(now));

    now += FLOOR_POST_UTTERANCE_BEAT_SECONDS + 0.01;
    assert!(!floor.busy(now));
}

/// 62. `test_player_speech_never_holds_the_floor`
#[test]
fn player_speech_never_holds_the_floor() {
    // The engine only calls `acquire` for non-player speakers (server.py:2047):
    // the player's own line is not something the NPCs have to wait through, and
    // his microphone hold is a different mechanism entirely. From the floor's
    // side that means: never acquired, never busy.
    let mut floor = ConversationFloor::new();
    assert!(!floor.busy(1_000.0));
    assert_eq!(floor.awaiting_len(), 0);
    assert_eq!(floor.floor_until(), 0.0);
    assert_eq!(floor.player_hold_until(), 0.0);
}

/// 63. `test_streamed_audio_holds_the_floor_and_expires_on_its_own`
#[test]
fn streamed_audio_holds_the_floor_and_expires_on_its_own() {
    let mut floor = ConversationFloor::new();
    let mut now = 1_000.0;
    assert!(!floor.busy(now));

    // `player_audio_begin`
    floor.bump_player_hold(now, FLOOR_PLAYER_CHUNK_HOLD_SECONDS);
    assert!(floor.busy(now));

    // Each chunk re-bumps the rolling deadline while the player speaks.
    now += FLOOR_PLAYER_CHUNK_HOLD_SECONDS - 0.01;
    floor.bump_player_hold(now, FLOOR_PLAYER_CHUNK_HOLD_SECONDS);
    now += FLOOR_PLAYER_CHUNK_HOLD_SECONDS - 0.01;
    assert!(floor.busy(now));

    // A dead client just stops bumping; the hold expires with no clear.
    now += 0.02;
    assert!(!floor.busy(now));
}

/// 64. `test_silent_end_releases_the_hold_immediately`
/// 65. `test_abort_releases_the_hold_immediately`
#[test]
fn a_silent_end_or_an_abort_releases_the_hold_immediately() {
    let mut floor = ConversationFloor::new();
    let now = 1_000.0;

    floor.bump_player_hold(now, FLOOR_PLAYER_CHUNK_HOLD_SECONDS);
    assert!(floor.busy(now));

    // `player_audio_end {silent: true}` / `player_audio_abort`: the player said
    // nothing, so the NPCs get the floor back at once.
    floor.clear_player_hold();
    assert_eq!(floor.player_hold_until(), 0.0);
    assert!(!floor.busy(now));

    // Trailing chunks for a stream that no longer exists cannot resurrect the
    // hold: the engine drops them without bumping.
    assert!(!floor.busy(now));
}

/// 66. `test_completed_transcription_clears_the_hold`
#[test]
fn a_completed_transcription_clears_the_hold() {
    let mut floor = ConversationFloor::new();
    let now = 1_000.0;

    // The endpoint is reached: hold while the say is still on its way.
    floor.bump_player_hold(now, FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS);
    assert_eq!(
        floor.player_hold_until(),
        now + FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS
    );
    assert!(floor.busy(now));

    // The transcription resolved and the say applied — and the player's own
    // speech never acquires the NPC floor, so nothing is left holding it.
    floor.clear_player_hold();
    assert_eq!(floor.player_hold_until(), 0.0);
    assert!(!floor.busy(now));
}

/// 67. `test_batch_transcription_failure_clears_the_hold`
#[test]
fn a_failed_transcription_clears_the_hold() {
    let mut floor = ConversationFloor::new();
    let now = 1_000.0;

    // A batch round-trip can take seconds; the hold covers it.
    floor.bump_player_hold(now, FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS);
    assert_eq!(
        floor.player_hold_until(),
        now + FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS
    );
    assert!(floor.busy(now));

    // Failure resolves the utterance just as well as success does.
    floor.clear_player_hold();
    assert!(!floor.busy(now));
}

/// A rolling hold never shrinks: a short bump behind a long one cannot cut the
/// long one short (`_bump_player_hold` is a `max`).
#[test]
fn a_shorter_bump_never_shortens_a_longer_hold() {
    let mut floor = ConversationFloor::new();
    let now = 1_000.0;

    floor.bump_player_hold(now, FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS);
    floor.bump_player_hold(now, FLOOR_PLAYER_CHUNK_HOLD_SECONDS);
    assert_eq!(
        floor.player_hold_until(),
        now + FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS
    );
}

/// The awaiting map is a safety valve, not a queue: a pathological backlog of
/// unpresented lines evicts oldest-first instead of growing forever.
#[test]
fn the_awaiting_backlog_is_bounded() {
    let mut floor = ConversationFloor::new();
    let now = 1_000.0;

    for sequence in 0..(MAX_FLOOR_AWAITING as u32 + 5) {
        floor.acquire(now, &event(sequence), "x", true);
    }
    assert_eq!(floor.awaiting_len(), MAX_FLOOR_AWAITING);
    assert!(!floor.is_awaiting(&event(0)), "the oldest was evicted");
    assert!(floor.is_awaiting(&event(MAX_FLOOR_AWAITING as u32 + 4)));

    // Re-acquiring a live id refreshes its deadline in place instead of
    // enqueuing it twice — but the eviction pass runs first, exactly as in
    // Python (`while len(...) >= MAX: del oldest`), so a full backlog still
    // loses its oldest entry to the refresh. Quirk, ported.
    let live = event(MAX_FLOOR_AWAITING as u32 + 4);
    floor.acquire(now, &live, "x", true);
    assert_eq!(floor.awaiting_len(), MAX_FLOOR_AWAITING - 1);
    floor.release(now, &live);
    assert!(!floor.is_awaiting(&live), "the id was never enqueued twice");
}

/// 68. `test_npc_reply_finished_during_player_speech_waits_for_the_hold` — the
///     flagship arbitration test: scheduler and floor together.
#[test]
fn an_npc_reply_finished_during_player_speech_waits_for_the_hold() {
    let mut world = seed_world();
    let env = prompt_env();
    let mut cognition = FakeCognition::new();
    let mut floor = ConversationFloor::new();
    let mut transcript: Vec<String> = Vec::new();
    let mut scheduler = NpcScheduler::new(llm_turn_order(&world), 0.0, 60.0, 1_000.0);

    let mut now = 1_000.0;
    scheduler.start(now);

    // The player starts speaking: `player_audio_begin` + a chunk.
    floor.bump_player_hold(now, FLOOR_PLAYER_CHUNK_HOLD_SECONDS);
    assert!(floor.busy(now));

    let poll = |now: f64,
                world: &mut _,
                transcript: &mut _,
                floor: &mut ConversationFloor,
                scheduler: &mut NpcScheduler,
                cognition: &mut FakeCognition| {
        let mut completions = cognition.drain_completions();
        let busy = floor.busy(now);
        scheduler.poll(
            now,
            world,
            transcript,
            &mut completions,
            busy,
            cognition as &mut dyn Cognition,
            &env,
        );
    };

    // A busy floor does NOT gate submission: the next speaker keeps thinking
    // while the player talks.
    poll(
        now,
        &mut world,
        &mut transcript,
        &mut floor,
        &mut scheduler,
        &mut cognition,
    );
    assert!(scheduler.in_flight_actor_id().is_some());

    // The turn finishes mid-utterance: it is held, not applied.
    poll(
        now,
        &mut world,
        &mut transcript,
        &mut floor,
        &mut scheduler,
        &mut cognition,
    );
    assert!(scheduler.has_held_result());
    assert!(transcript.is_empty());

    // Polling again changes nothing while the player still holds the floor.
    poll(
        now,
        &mut world,
        &mut transcript,
        &mut floor,
        &mut scheduler,
        &mut cognition,
    );
    assert!(scheduler.has_held_result());
    assert!(transcript.is_empty());

    // Once the hold expires, the next poll applies the turn.
    now += FLOOR_PLAYER_CHUNK_HOLD_SECONDS + 0.01;
    poll(
        now,
        &mut world,
        &mut transcript,
        &mut floor,
        &mut scheduler,
        &mut cognition,
    );
    assert!(!scheduler.has_held_result());
    assert_eq!(transcript.len(), 1, "{transcript:?}");
}
