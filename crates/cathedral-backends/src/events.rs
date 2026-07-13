//! The one channel every backend result travels on (D7).
//!
//! Submission is a synchronous trait call into cathedral-sim's `Cognition` /
//! `Transcription` / `Tts`; completion comes back here. The host drains this
//! receiver once per frame and feeds the values into `Engine::poll` as
//! `EngineCommand`s — the sim never sees a channel.

use std::sync::Arc;

use cathedral_sim::{
    Completion, RealtimeResult, SpeechError, SpeechEventId, StatusEvent, TranscriptionJobId,
    TranscriptionOutcome, TtsOutcome,
};
use crossbeam_channel::{Receiver, Sender, unbounded};

/// One finished piece of backend work.
///
/// Variants mirror ARCHITECTURE §1.2 one-for-one; the payloads are the sim's own
/// types so the host's mapping into `EngineCommand` stays mechanical.
#[derive(Debug, Clone, PartialEq)]
pub enum BackendEvent {
    LlmCompletion(Completion),
    TranscriptionDone {
        job: TranscriptionJobId,
        result: Result<String, SpeechError>,
    },
    RealtimeResult(RealtimeResult),
    TtsChunk {
        event_id: SpeechEventId,
        seq: u32,
        sample_rate: u32,
        samples: Arc<[i16]>,
    },
    TtsStreamEnd {
        event_id: SpeechEventId,
        chunk_count: u32,
        first_chunk_ms: u32,
    },
    TtsDone {
        event_id: SpeechEventId,
        /// The whole WAV, in memory.
        result: Result<Arc<[u8]>, SpeechError>,
    },
    /// Worker/warmup health, passed straight through to the HUD.
    Status(StatusEvent),
}

impl BackendEvent {
    /// The speech-input half, ready for `EngineCommand::Transcription`.
    pub fn into_transcription_outcome(self) -> Option<TranscriptionOutcome> {
        match self {
            Self::TranscriptionDone { job, result } => {
                Some(TranscriptionOutcome::Done { job, result })
            }
            Self::RealtimeResult(result) => Some(TranscriptionOutcome::Realtime(result)),
            _ => None,
        }
    }

    /// The speech-output half, ready for `EngineCommand::Tts`.
    pub fn into_tts_outcome(self) -> Option<TtsOutcome> {
        match self {
            Self::TtsChunk {
                event_id,
                seq,
                sample_rate,
                samples,
            } => Some(TtsOutcome::Chunk {
                event_id,
                seq,
                sample_rate,
                samples,
            }),
            Self::TtsStreamEnd {
                event_id,
                chunk_count,
                first_chunk_ms,
            } => Some(TtsOutcome::StreamEnd {
                event_id,
                chunk_count,
                first_chunk_ms,
            }),
            Self::TtsDone { event_id, result } => Some(TtsOutcome::Done { event_id, result }),
            _ => None,
        }
    }
}

impl From<Completion> for BackendEvent {
    fn from(completion: Completion) -> Self {
        Self::LlmCompletion(completion)
    }
}

impl From<StatusEvent> for BackendEvent {
    fn from(status: StatusEvent) -> Self {
        Self::Status(status)
    }
}

impl From<TranscriptionOutcome> for BackendEvent {
    fn from(outcome: TranscriptionOutcome) -> Self {
        match outcome {
            TranscriptionOutcome::Done { job, result } => Self::TranscriptionDone { job, result },
            TranscriptionOutcome::Realtime(result) => Self::RealtimeResult(result),
        }
    }
}

impl From<TtsOutcome> for BackendEvent {
    fn from(outcome: TtsOutcome) -> Self {
        match outcome {
            TtsOutcome::Chunk {
                event_id,
                seq,
                sample_rate,
                samples,
            } => Self::TtsChunk {
                event_id,
                seq,
                sample_rate,
                samples,
            },
            TtsOutcome::StreamEnd {
                event_id,
                chunk_count,
                first_chunk_ms,
            } => Self::TtsStreamEnd {
                event_id,
                chunk_count,
                first_chunk_ms,
            },
            TtsOutcome::Done { event_id, result } => Self::TtsDone { event_id, result },
        }
    }
}

/// Unbounded on purpose: a backend must never block (or drop a completion)
/// because the game skipped a frame. Volume is bounded by the work in flight —
/// one LLM turn, one utterance, a few hundred PCM chunks.
pub fn backend_channel() -> (BackendSender, Receiver<BackendEvent>) {
    let (sender, receiver) = unbounded();
    (BackendSender(sender), receiver)
}

/// The producer half, cloned into every backend task.
#[derive(Debug, Clone)]
pub struct BackendSender(Sender<BackendEvent>);

impl BackendSender {
    /// Fire-and-forget: a closed receiver means the host is shutting down, and
    /// a backend task that is mid-flight then has nothing useful to do about it.
    pub fn send(&self, event: impl Into<BackendEvent>) {
        let _ = self.0.send(event.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cathedral_sim::{CognitionError, RequestId};

    #[test]
    fn completions_travel_the_channel_and_survive_a_closed_receiver() {
        let (sender, receiver) = backend_channel();
        sender.send(Completion {
            request_id: RequestId(7),
            result: Err(CognitionError::new("LlmTransportError")),
            duration_seconds: 1.5,
        });

        let event = receiver.try_recv().expect("one event");
        let BackendEvent::LlmCompletion(completion) = event else {
            panic!("expected a completion");
        };
        assert_eq!(completion.request_id, RequestId(7));

        drop(receiver);
        sender.send(StatusEvent::llm("idle", None, None)); // must not panic
    }

    #[test]
    fn speech_outcomes_round_trip_through_the_event_enum() {
        let outcome = TtsOutcome::Chunk {
            event_id: SpeechEventId("speech-3".to_string()),
            seq: 0,
            sample_rate: 24_000,
            samples: Arc::from(vec![0i16; 4].as_slice()),
        };
        let event = BackendEvent::from(outcome.clone());
        assert_eq!(event.into_tts_outcome(), Some(outcome));

        let outcome = TranscriptionOutcome::Realtime(RealtimeResult::Transcript {
            key: "speech-1".to_string(),
            text: "hello".to_string(),
        });
        let event = BackendEvent::from(outcome.clone());
        assert_eq!(event.clone().into_transcription_outcome(), Some(outcome));
        assert_eq!(event.into_tts_outcome(), None);
    }
}
