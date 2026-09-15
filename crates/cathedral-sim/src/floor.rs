//! Conversation floor arbitration (`server.py:2021-2083`).
//!
//! Ordinary NPC turns share one room. A voiced NPC line is *awaited* by event id
//! until the game reports it was presented; an unvoiced one just holds the
//! floor for as long as it takes to read. Player-audible and background holds
//! are tracked separately so a protected response to fresh player speech can
//! ignore an inaudible exchange elsewhere in the city. The player holds the
//! foreground too, through a rolling deadline bumped while the microphone
//! streams.
//!
//! Every hold is self-expiring. A lost `speech_presented`, a crashed client, a
//! dropped transcription — none of them may stall NPC turns forever, so
//! [`ConversationFloor::busy`] purges overdue entries before it answers.
//!
//! The floor gates *application*, never *submission*: a finished LLM turn is
//! held un-applied while the floor is busy (`scheduler.py:183-198`), which is
//! what lets the next speaker think ahead while the previous line is still on
//! screen.

pub mod checkpoint;
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
    awaiting: Vec<AwaitedSpeech>,
    /// Reading estimates and the post-utterance beat for speech the player can
    /// hear. These still pace an urgent response to the player's own words.
    foreground_floor_until: f64,
    /// Equivalent pacing for conversations outside the player's earshot.
    /// Ordinary city turns honor it; a protected player reaction does not.
    background_floor_until: f64,
    /// The player's rolling microphone/transcription hold.
    player_hold_until: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct AwaitedSpeech {
    event_id: SpeechEventId,
    deadline: f64,
    blocks_player_reaction: bool,
}

impl ConversationFloor {
    /// Audio execution never survives load. Keep existing reading/beat pacing;
    /// translate each old voiced wait to its surviving readable progress, capped
    /// by the original failsafe. A wait with no readable owner releases now.
    pub(crate) fn prepare_continuation(
        &mut self,
        now: f64,
        readable_rows: &[crate::checkpoint::host::RecordV1<String>],
    ) -> usize {
        use crate::checkpoint::host::RecordV1;
        let count = self.awaiting.len();
        for awaited in self.awaiting.drain(..) {
            let mut readable = now;
            for row in readable_rows {
                let (speech, deadline) = match row {
                    RecordV1::Subtitle {
                        speech,
                        minimum_seconds,
                        visible_since,
                        ..
                    } => (speech, visible_since.0.unwrap_or(now) + minimum_seconds),
                    RecordV1::Bubble {
                        speech, expires_at, ..
                    } => (speech, *expires_at),
                    RecordV1::UnreadSpeech { speech, .. } => {
                        (speech, now + speech_reading_seconds(&speech.text))
                    }
                    _ => continue,
                };
                if speech.event == awaited.event_id.0.as_str() {
                    readable = readable.max(deadline);
                }
            }
            let deadline = readable.min(awaited.deadline);
            let floor = if awaited.blocks_player_reaction {
                &mut self.foreground_floor_until
            } else {
                &mut self.background_floor_until
            };
            *floor = floor.max(deadline);
        }
        self.player_hold_until = 0.0;
        count
    }
    pub fn new() -> Self {
        Self::default()
    }

    /// True while a previous utterance is still being presented.
    ///
    /// Takes `&mut self` because it purges first: an overdue awaited entry is
    /// dropped here and *not* granted a post-utterance beat — the line is stale,
    /// nobody is listening to it any more.
    pub fn busy(&mut self, now: f64) -> bool {
        self.purge_expired(now);
        !self.awaiting.is_empty()
            || now < self.foreground_floor_until
            || now < self.background_floor_until
            || now < self.player_hold_until
    }

    /// Whether the floor should hold a response to fresh player speech.
    ///
    /// Inaudible conversations elsewhere in the city retain their own pacing,
    /// but they no longer serialize the nearby exchange the player initiated.
    /// Speech within the player's earshot and the microphone hold still block.
    pub fn busy_for_player_reaction(&mut self, now: f64) -> bool {
        self.purge_expired(now);
        self.awaiting
            .iter()
            .any(|speech| speech.blocks_player_reaction)
            || now < self.foreground_floor_until
            || now < self.player_hold_until
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
        self.acquire_scoped(now, event_id, text, tts_queued, true);
    }

    /// Hold either the player-audible or background conversation floor.
    ///
    /// `blocks_player_reaction` is true exactly when the player was among the
    /// speech event's recipients. This preserves normal global pacing while
    /// allowing a nearby answer to overlap an inaudible distant exchange.
    pub fn acquire_scoped(
        &mut self,
        now: f64,
        event_id: &SpeechEventId,
        text: &str,
        tts_queued: bool,
        blocks_player_reaction: bool,
    ) {
        if !tts_queued {
            let deadline = now + speech_reading_seconds(text);
            let floor_until = if blocks_player_reaction {
                &mut self.foreground_floor_until
            } else {
                &mut self.background_floor_until
            };
            *floor_until = floor_until.max(deadline);
            return;
        }
        // Insertion order is deadline order closely enough: this only ever
        // trims an already pathological backlog of unpresented lines
        // (server-core.md risk 5 — Python tolerates the same imprecision).
        while self.awaiting.len() >= MAX_FLOOR_AWAITING {
            self.awaiting.remove(0);
        }
        let deadline = now + floor_audio_failsafe_seconds(text);
        match self
            .awaiting
            .iter_mut()
            .find(|speech| speech.event_id == *event_id)
        {
            // Re-acquiring an id refreshes its deadline in place, like assigning
            // to an existing dict key.
            Some(entry) => {
                entry.deadline = deadline;
                entry.blocks_player_reaction = blocks_player_reaction;
            }
            None => self.awaiting.push(AwaitedSpeech {
                event_id: event_id.clone(),
                deadline,
                blocks_player_reaction,
            }),
        }
    }

    /// Release one awaited utterance. Idempotent (D26): duplicate acks and ids
    /// whose failsafe already expired are legitimately unknown, and must not
    /// re-arm the beat.
    pub fn release(&mut self, now: f64, event_id: &SpeechEventId) {
        let Some(index) = self
            .awaiting
            .iter()
            .position(|speech| speech.event_id == *event_id)
        else {
            return;
        };
        let released = self.awaiting.remove(index);
        if !self
            .awaiting
            .iter()
            .any(|speech| speech.blocks_player_reaction == released.blocks_player_reaction)
        {
            // A short beat so consecutive voices breathe.
            let floor_until = if released.blocks_player_reaction {
                &mut self.foreground_floor_until
            } else {
                &mut self.background_floor_until
            };
            *floor_until = floor_until.max(now + FLOOR_POST_UTTERANCE_BEAT_SECONDS);
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
        self.foreground_floor_until.max(self.background_floor_until)
    }

    pub fn is_awaiting(&self, event_id: &SpeechEventId) -> bool {
        self.awaiting
            .iter()
            .any(|speech| speech.event_id == *event_id)
    }

    pub fn awaiting_len(&self) -> usize {
        self.awaiting.len()
    }

    fn purge_expired(&mut self, now: f64) {
        // Python: `deadline <= now` expires, so a deadline exactly at `now` is
        // already gone. Expiry deliberately grants no post-utterance beat.
        self.awaiting.retain(|speech| speech.deadline > now);
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

    #[test]
    fn continuation_preserves_reading_pacing_reconciles_each_audio_scope_and_clears_microphone() {
        use crate::checkpoint::host::{Nullable, RecordV1, SpeechV1};
        let speech = |id: &str| SpeechV1 {
            sequence: 1,
            event: id.into(),
            speaker: "sv3n1".into(),
            label: "Sven".into(),
            target: Nullable(None),
            text: "readable words".into(),
            position: [0.0; 3],
            recipient_count: 1,
            expect_audio: true,
        };
        let mut f = ConversationFloor::new();
        f.acquire_scoped(8.0, &SpeechEventId("reading-fg".into()), "", false, true); // 11
        f.acquire_scoped(10.0, &SpeechEventId("reading-bg".into()), "", false, false); // 13
        f.acquire_scoped(9.0, &SpeechEventId("voiced-fg".into()), "", true, true); // 17
        f.acquire_scoped(9.0, &SpeechEventId("voiced-bg".into()), "", true, false); // 17
        f.acquire_scoped(9.0, &SpeechEventId("missing".into()), "", true, true);
        f.bump_player_hold(10.0, 60.0);
        let rows = [
            RecordV1::Subtitle {
                speech: speech("voiced-fg"),
                formatted_text: "Sven: readable words".into(),
                minimum_seconds: 5.0,
                visible_since: Nullable(Some(9.0)),
                audio_playing: true,
            },
            RecordV1::Bubble {
                speech: speech("voiced-bg"),
                text: "readable words".into(),
                world_anchor: [0.0; 3],
                expires_at: 30.0,
                audio_extended: true,
            },
        ];
        assert_eq!(f.prepare_continuation(10.0, &rows), 3);
        assert_eq!(f.awaiting_len(), 0);
        assert_eq!(f.player_hold_until(), 0.0);
        assert_eq!(f.foreground_floor_until, 14.0);
        assert_eq!(
            f.background_floor_until, 17.0,
            "original failsafe caps readable audio extension"
        );
        assert!(!f.busy_for_player_reaction(14.0));
        assert!(f.busy(14.0));
        let once = f.clone();
        assert_eq!(f.prepare_continuation(10.0, &rows), 0);
        assert_eq!(
            f, once,
            "second preparation neither resets nor re-arms reading"
        );
        f.release(16.0, &SpeechEventId("voiced-bg".into()));
        assert_eq!(
            f, once,
            "old presentation ack cannot re-arm a post-audio beat"
        );
    }
}
