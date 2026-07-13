//! Conversation floor arbitration (`server.py:2021-2083`).
//!
//! One utterance at a time gets the room. A voiced NPC line is *awaited* by
//! event id until the game reports it was presented; an unvoiced one just holds
//! the floor for as long as it takes to read. The player holds it too, through a
//! rolling deadline bumped while the microphone streams.
//!
//! Every hold is self-expiring. A lost `speech_presented`, a crashed client, a
//! dropped transcription — none of them may stall NPC turns forever, so
//! [`ConversationFloor::busy`] purges overdue entries before it answers.
//!
//! The floor gates *application*, never *submission*: a finished LLM turn is
//! held un-applied while the floor is busy (`scheduler.py:183-198`), which is
//! what lets the next speaker think ahead while the previous line is still on
//! screen.

use crate::{
    FLOOR_AUDIO_FAILSAFE_MAX_SECONDS, FLOOR_POST_UTTERANCE_BEAT_SECONDS, MAX_FLOOR_AWAITING,
    ids::SpeechEventId,
};

/// How long the floor waits for a voiced line to be presented before assuming
/// the acknowledgement was lost (`server.py:167-173`).
///
/// Deliberately looser than the reading estimate: it bounds synthesis latency
/// *plus* playback, and only ever fires as a failsafe.
pub fn floor_audio_failsafe_seconds(text: &str) -> f64 {
    let characters = text.chars().count() as f64; // Unicode scalars, like Python `len` (D11).
    (8.0 + characters / 10.0).min(FLOOR_AUDIO_FAILSAFE_MAX_SECONDS)
}

/// How long an unvoiced line holds the floor (`server.py:176-178`).
///
/// The single home of the formula Bevy's subtitle timing (`speech_text_seconds`
/// in `speech.rs`) uses: text stays up as long as it takes to read, and the
/// conversation is paced to match.
pub fn speech_reading_seconds(text: &str) -> f64 {
    let characters = text.chars().count() as f64;
    (2.0 + characters / 15.0).clamp(3.0, 10.0)
}

/// Who currently owns the room.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConversationFloor {
    /// Voiced utterances awaiting their `speech_presented`, insertion-ordered
    /// with their failsafe deadlines (`OrderedDict` in Python).
    awaiting: Vec<(SpeechEventId, f64)>,
    /// Reading estimates and the post-utterance beat.
    floor_until: f64,
    /// The player's rolling microphone/transcription hold.
    player_hold_until: f64,
}

impl ConversationFloor {
    pub fn new() -> Self {
        Self::default()
    }

    /// True while a previous utterance is still being presented.
    ///
    /// Takes `&mut self` because it purges first: an overdue awaited entry is
    /// dropped here and *not* granted a post-utterance beat — the line is stale,
    /// nobody is listening to it any more.
    pub fn busy(&mut self, now: f64) -> bool {
        // Python: `deadline <= now` expires, so a deadline exactly at `now` is
        // already gone.
        self.awaiting.retain(|(_, deadline)| *deadline > now);
        !self.awaiting.is_empty() || now < self.floor_until || now < self.player_hold_until
    }

    /// Hold the floor for one non-player utterance.
    ///
    /// `tts_queued` is whether synthesis was *actually accepted* by a backend
    /// (`Tts::submit` returned `Ok`): only then will a `SpeechPresented` ever
    /// arrive, so only then is there something to await. Everything else —
    /// voices off, player out of earshot, a refused submission — paces on the
    /// reading estimate instead (D26: the floor never waits for an event that
    /// cannot be acknowledged).
    pub fn acquire(&mut self, now: f64, event_id: &SpeechEventId, text: &str, tts_queued: bool) {
        if !tts_queued {
            self.floor_until = self.floor_until.max(now + speech_reading_seconds(text));
            return;
        }
        // Insertion order is deadline order closely enough: this only ever
        // trims an already pathological backlog of unpresented lines
        // (server-core.md risk 5 — Python tolerates the same imprecision).
        while self.awaiting.len() >= MAX_FLOOR_AWAITING {
            self.awaiting.remove(0);
        }
        let deadline = now + floor_audio_failsafe_seconds(text);
        match self.awaiting.iter_mut().find(|(id, _)| id == event_id) {
            // Re-acquiring an id refreshes its deadline in place, like assigning
            // to an existing dict key.
            Some(entry) => entry.1 = deadline,
            None => self.awaiting.push((event_id.clone(), deadline)),
        }
    }

    /// Release one awaited utterance. Idempotent (D26): duplicate acks and ids
    /// whose failsafe already expired are legitimately unknown, and must not
    /// re-arm the beat.
    pub fn release(&mut self, now: f64, event_id: &SpeechEventId) {
        let Some(index) = self.awaiting.iter().position(|(id, _)| id == event_id) else {
            return;
        };
        self.awaiting.remove(index);
        if self.awaiting.is_empty() {
            // A short beat so consecutive voices breathe.
            self.floor_until = self
                .floor_until
                .max(now + FLOOR_POST_UTTERANCE_BEAT_SECONDS);
        }
    }

    /// Extend the player's hold. Rolling and never shrinking: a dead client
    /// simply stops bumping and the hold expires on its own.
    pub fn bump_player_hold(&mut self, now: f64, seconds: f64) {
        self.player_hold_until = self.player_hold_until.max(now + seconds);
    }

    /// The player's utterance resolved (or was abandoned): give the floor back
    /// at once instead of waiting the hold out.
    pub fn clear_player_hold(&mut self) {
        self.player_hold_until = 0.0;
    }

    pub fn player_hold_until(&self) -> f64 {
        self.player_hold_until
    }

    pub fn floor_until(&self) -> f64 {
        self.floor_until
    }

    pub fn is_awaiting(&self, event_id: &SpeechEventId) -> bool {
        self.awaiting.iter().any(|(id, _)| id == event_id)
    }

    pub fn awaiting_len(&self) -> usize {
        self.awaiting.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_formulas_count_unicode_scalars_not_bytes() {
        // 30 chars → 2 + 30/15 = 4 s; the same 30 chars as 60 bytes must not
        // read as 6 s (D11).
        assert_eq!(speech_reading_seconds(&"x".repeat(30)), 4.0);
        assert_eq!(speech_reading_seconds(&"é".repeat(30)), 4.0);
        assert_eq!(floor_audio_failsafe_seconds(&"é".repeat(20)), 10.0);
    }

    #[test]
    fn the_reading_estimate_is_clamped_and_the_failsafe_capped() {
        assert_eq!(speech_reading_seconds(""), 3.0);
        assert_eq!(speech_reading_seconds(&"x".repeat(1_000)), 10.0);
        assert_eq!(floor_audio_failsafe_seconds(""), 8.0);
        assert_eq!(
            floor_audio_failsafe_seconds(&"x".repeat(10_000)),
            FLOOR_AUDIO_FAILSAFE_MAX_SECONDS
        );
    }
}
