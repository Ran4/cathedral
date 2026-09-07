//! The one channel every backend result travels on (D7).
//!
//! Submission is a synchronous trait call into cathedral-sim's `Cognition` /
//! `Transcription` / `Tts`; completion comes back here. The host drains this
//! receiver once per frame and feeds the values into `Engine::poll` as
//! `EngineCommand`s — the sim never sees a channel.

use std::sync::Arc;

pub use crate::mailbox::{BackendReceiver, BackendSender, backend_channel, backend_channel_for};
use cathedral_sim::{
    CognitionError, Completion, RealtimeResult, SpeechError, SpeechEventId, StatusEvent,
    TranscriptionJobId, TranscriptionOutcome, TtsOutcome,
};

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
    pub(crate) fn is_terminal(&self) -> bool {
        !matches!(self, Self::TtsChunk { .. } | Self::Status(_))
    }

    pub(crate) fn terminal_budget(&self) -> usize {
        match self {
            Self::LlmCompletion(_) => 401_024,
            Self::TtsDone { .. } | Self::TtsStreamEnd { .. } => crate::wav::MAX_WAV_BYTES + 1024,
            _ => 16 * 1024,
        }
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        let text = |value: &String| value.capacity();
        let error = |value: &SpeechError| value.presentable.capacity();
        std::mem::size_of::<Self>()
            + match self {
                Self::LlmCompletion(c) => match &c.result {
                    Ok(reply) => reply.capacity(),
                    Err(error) => error.allocated_bytes(),
                },
                Self::TranscriptionDone { result, .. } => match result {
                    Ok(t) => text(t),
                    Err(e) => error(e),
                },
                Self::RealtimeResult(RealtimeResult::Transcript { key, text: value }) => {
                    text(key) + text(value)
                }
                Self::RealtimeResult(RealtimeResult::Failure { key, reason }) => {
                    key.as_ref().map_or(0, text) + text(reason)
                }
                Self::TtsChunk {
                    event_id, samples, ..
                } => {
                    event_id.0.capacity()
                        + 2 * std::mem::size_of::<usize>()
                        + std::mem::size_of_val(samples.as_ref())
                }
                Self::TtsStreamEnd { event_id, .. } => event_id.0.capacity(),
                Self::TtsDone { event_id, result } => {
                    event_id.0.capacity()
                        + match result {
                            Ok(wav) => wav.len() + 2 * std::mem::size_of::<usize>(),
                            Err(e) => error(e),
                        }
                }
                Self::Status(status) => {
                    status.state.capacity()
                        + status.message.as_ref().map_or(0, text)
                        + status.backend.as_ref().map_or(0, text)
                        + status
                            .actor_id
                            .as_ref()
                            .map_or(0, |id| id.allocated_bytes())
                }
            }
    }

    pub(crate) fn valid_nonterminal(&self) -> bool {
        match self {
            Self::TtsChunk {
                event_id,
                samples,
                sample_rate,
                ..
            } => {
                event_id.0.len() <= 256
                    && std::mem::size_of_val(samples.as_ref()) <= crate::wav::MAX_PCM_CHUNK_BYTES
                    && (8_000..=192_000).contains(sample_rate)
            }
            Self::Status(_) => self.retained_bytes() <= 4096,
            _ => false,
        }
    }

    pub(crate) fn same_job(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::LlmCompletion(a), Self::LlmCompletion(b)) => a.request_id == b.request_id,
            (Self::TranscriptionDone { job: a, .. }, Self::TranscriptionDone { job: b, .. }) => {
                a == b
            }
            (Self::RealtimeResult(a), Self::RealtimeResult(b)) => {
                let key = |r: &RealtimeResult| match r {
                    RealtimeResult::Transcript { key, .. } => Some(key.clone()),
                    RealtimeResult::Failure { key, .. } => key.clone(),
                };
                key(a) == key(b)
            }
            (
                Self::TtsDone { event_id: a, .. }
                | Self::TtsStreamEnd { event_id: a, .. }
                | Self::TtsChunk { event_id: a, .. },
                Self::TtsDone { event_id: b, .. } | Self::TtsStreamEnd { event_id: b, .. },
            ) => a == b,
            _ => false,
        }
    }

    pub(crate) fn cancellation(&self) -> Self {
        let message = "backend delivery cancelled: producer ended or result exceeded capacity";
        match self {
            Self::LlmCompletion(c) => Self::LlmCompletion(Completion {
                request_id: c.request_id,
                result: Err(CognitionError::new(message)),
                duration_seconds: 0.0,
            }),
            Self::TranscriptionDone { job, .. } => Self::TranscriptionDone {
                job: *job,
                result: Err(SpeechError::new(message)),
            },
            Self::RealtimeResult(result) => Self::RealtimeResult(RealtimeResult::Failure {
                key: match result {
                    RealtimeResult::Transcript { key, .. } => Some(key.clone()),
                    RealtimeResult::Failure { key, .. } => key.clone(),
                },
                reason: message.into(),
            }),
            Self::TtsDone { event_id, .. } | Self::TtsStreamEnd { event_id, .. } => Self::TtsDone {
                event_id: event_id.clone(),
                result: Err(SpeechError::new(message)),
            },
            _ => self.clone(),
        }
    }

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
