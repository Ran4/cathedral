//! Offline speech backends (`server.py:354-390` `FakeSpeechBackend`).
//!
//! `config.ron: fake_backend: true` and the integration tests run on these: a
//! canned transcript instead of a microphone, and silence instead of a voice —
//! but the *shapes* are real (a valid WAV, a real PCM chunk stream), so the
//! router, the floor and the game's audio consumers all take their production
//! code paths.
//!
//! Cognition's fake lives in cathedral-sim (`FakeCognition`, D25) because the
//! sim's own tests need it; speech's lives here because it needs `hound`.

use std::{io::Cursor, sync::Arc};

use cathedral_sim::{
    SpeechEventId, SttBackendKind, SttSubmitError, Transcription, TranscriptionJobId, Tts,
    TtsBackendKind, TtsRequest, TtsSubmitError,
};

use crate::events::{BackendEvent, BackendSender};

/// What the fake microphone hears (`server.py:368`).
pub const DEFAULT_FAKE_TRANSCRIPT: &str = "What's your name?";
/// Env override, so a test can choose a transcript without embedding text in an
/// audio file.
pub const FAKE_TRANSCRIPT_ENV: &str = "SMART_ACTORS_FAKE_TRANSCRIPT";

/// `server.py:370-377` — the whole-WAV (cloud-shaped) synthesis.
const CLOUD_SAMPLE_RATE: u32 = 16_000;
/// `server.py:385-390` — the streaming (local-shaped) synthesis.
const LOCAL_SAMPLE_RATE: u32 = 24_000;

/// Silence, sized from the text exactly as Python sizes it: a quarter second at
/// most, 800 samples at least, 40 samples per character in between.
fn silent_sample_count(text: &str, sample_rate: u32) -> usize {
    let ceiling = (sample_rate / 4) as usize;
    let from_text = (text.chars().count() * 40).max(800);
    ceiling.min(from_text)
}

/// A mono 16-bit WAV of `sample_count` zero samples.
fn silent_wav(sample_count: usize, sample_rate: u32) -> Arc<[u8]> {
    let specification = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer =
            hound::WavWriter::new(&mut buffer, specification).expect("an in-memory WAV header");
        for _ in 0..sample_count {
            writer.write_sample(0i16).expect("in-memory write");
        }
        writer.finalize().expect("in-memory finalize");
    }
    Arc::from(buffer.into_inner().as_slice())
}

/// The offline [`Transcription`] + [`Tts`] pair.
///
/// Both submits complete *immediately* onto the backend channel, so a fake-mode
/// run needs no threads and no clock — the host picks the results up on its very
/// next drain, and tests stay deterministic.
#[derive(Debug, Clone)]
pub struct FakeSpeech {
    events: BackendSender,
    transcript: String,
}

impl FakeSpeech {
    pub fn new(events: BackendSender) -> Self {
        let transcript = std::env::var(FAKE_TRANSCRIPT_ENV)
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_FAKE_TRANSCRIPT.to_string());
        Self { events, transcript }
    }

    pub fn with_transcript(events: BackendSender, transcript: impl Into<String>) -> Self {
        Self {
            events,
            transcript: transcript.into(),
        }
    }

    pub fn transcript(&self) -> &str {
        &self.transcript
    }
}

impl Transcription for FakeSpeech {
    fn available(&self, _kind: SttBackendKind) -> bool {
        true
    }

    fn submit_batch(
        &mut self,
        job: TranscriptionJobId,
        _wav_path: std::path::PathBuf,
        _kind: SttBackendKind,
    ) -> Result<(), SttSubmitError> {
        // The WAV is never opened: the fake microphone's "audio" carries no words.
        self.events.send(BackendEvent::TranscriptionDone {
            job,
            result: Ok(self.transcript.clone()),
        });
        Ok(())
    }

    /// No realtime session offline — `false` means "fall back to batch", which is
    /// exactly the path fake mode should exercise.
    fn realtime_begin(&mut self, _key: &str) -> bool {
        false
    }

    fn realtime_append(&mut self, _key: &str, _samples: &[i16]) -> bool {
        false
    }

    fn realtime_commit(&mut self, _key: &str) -> bool {
        false
    }

    fn realtime_clear(&mut self, _key: &str) {}
}

impl Tts for FakeSpeech {
    fn available(&self, kind: TtsBackendKind) -> bool {
        kind != TtsBackendKind::Off
    }

    fn submit(&mut self, request: TtsRequest) -> Result<(), TtsSubmitError> {
        let TtsRequest {
            event_id,
            text,
            kind,
            ..
        } = request;
        match kind {
            TtsBackendKind::Cloud => {
                let samples = silent_sample_count(&text, CLOUD_SAMPLE_RATE);
                self.events.send(BackendEvent::TtsDone {
                    event_id,
                    result: Ok(silent_wav(samples, CLOUD_SAMPLE_RATE)),
                });
                Ok(())
            }
            TtsBackendKind::Local => {
                // One chunk, then the stream end — the shape the local Pocket
                // worker produces, minus the model.
                let samples = silent_sample_count(&text, LOCAL_SAMPLE_RATE);
                self.events.send(BackendEvent::TtsChunk {
                    event_id: event_id.clone(),
                    seq: 0,
                    sample_rate: LOCAL_SAMPLE_RATE,
                    samples: Arc::from(vec![0i16; samples].as_slice()),
                });
                self.events.send(BackendEvent::TtsStreamEnd {
                    event_id,
                    chunk_count: 1,
                    first_chunk_ms: 1,
                });
                Ok(())
            }
            TtsBackendKind::Off => Err(TtsSubmitError::Unavailable),
        }
    }

    fn warm(&mut self, _kind: TtsBackendKind) {}
}

/// A [`SpeechEventId`] helper for hosts assembling fake requests in tests.
pub fn speech_event_id(sequence: i64) -> SpeechEventId {
    SpeechEventId(format!("speech-{sequence}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::backend_channel;
    use std::path::PathBuf;

    #[test]
    fn a_recording_transcribes_to_the_canned_line() {
        let (sender, receiver) = backend_channel();
        let mut speech = FakeSpeech::with_transcript(sender, "What's your name?");
        assert!(Transcription::available(&speech, SttBackendKind::Cloud));
        assert!(Transcription::available(&speech, SttBackendKind::Local));

        speech
            .submit_batch(
                TranscriptionJobId(3),
                PathBuf::from("/does/not/exist.wav"),
                SttBackendKind::Cloud,
            )
            .expect("accepted");

        assert_eq!(
            receiver.try_recv().expect("a result"),
            BackendEvent::TranscriptionDone {
                job: TranscriptionJobId(3),
                result: Ok("What's your name?".to_string()),
            }
        );
        assert!(!speech.realtime_begin("speech-1"), "batch fallback offline");
    }

    #[test]
    fn cloud_synthesis_produces_a_readable_silent_wav() {
        let (sender, receiver) = backend_channel();
        let mut speech = FakeSpeech::new(sender);
        speech
            .submit(TtsRequest {
                event_id: speech_event_id(2),
                text: "hello".to_string(),
                voice_key: "ilse".to_string(),
                kind: TtsBackendKind::Cloud,
            })
            .expect("accepted");

        let BackendEvent::TtsDone { event_id, result } = receiver.try_recv().expect("a result")
        else {
            panic!("expected a finished WAV");
        };
        assert_eq!(event_id, SpeechEventId("speech-2".to_string()));
        let wav = result.expect("synthesized");

        let reader = hound::WavReader::new(Cursor::new(wav.to_vec())).expect("a valid WAV");
        let specification = reader.spec();
        assert_eq!(specification.sample_rate, 16_000);
        assert_eq!(specification.channels, 1);
        assert_eq!(specification.bits_per_sample, 16);
        // max(800, 5 chars * 40) == 800, below the 4000-sample ceiling.
        assert_eq!(reader.len(), 800);
    }

    #[test]
    fn local_synthesis_streams_one_chunk_and_a_stream_end() {
        let (sender, receiver) = backend_channel();
        let mut speech = FakeSpeech::new(sender);
        speech
            .submit(TtsRequest {
                event_id: speech_event_id(4),
                // 200 chars * 40 = 8000, over the 6000-sample ceiling.
                text: "x".repeat(200),
                voice_key: "sven".to_string(),
                kind: TtsBackendKind::Local,
            })
            .expect("accepted");

        let BackendEvent::TtsChunk {
            event_id,
            seq,
            sample_rate,
            samples,
        } = receiver.try_recv().expect("a chunk")
        else {
            panic!("expected a chunk");
        };
        assert_eq!(event_id, SpeechEventId("speech-4".to_string()));
        assert_eq!(seq, 0);
        assert_eq!(sample_rate, 24_000);
        assert_eq!(samples.len(), 6_000);
        assert!(samples.iter().all(|sample| *sample == 0), "silence");

        assert_eq!(
            receiver.try_recv().expect("a stream end"),
            BackendEvent::TtsStreamEnd {
                event_id: SpeechEventId("speech-4".to_string()),
                chunk_count: 1,
                first_chunk_ms: 1,
            }
        );
    }

    #[test]
    fn the_off_backend_refuses_and_reports_itself_unavailable() {
        let (sender, receiver) = backend_channel();
        let mut speech = FakeSpeech::new(sender);
        assert!(!Tts::available(&speech, TtsBackendKind::Off));
        assert_eq!(
            speech.submit(TtsRequest {
                event_id: speech_event_id(1),
                text: "hi".to_string(),
                voice_key: "conny".to_string(),
                kind: TtsBackendKind::Off,
            }),
            Err(TtsSubmitError::Unavailable),
        );
        assert!(receiver.try_recv().is_err(), "a refusal emits nothing");
    }

    #[test]
    fn the_sample_count_formula_matches_python() {
        // min(rate // 4, max(800, len(text) * 40))
        assert_eq!(silent_sample_count("", 16_000), 800);
        assert_eq!(silent_sample_count(&"x".repeat(30), 16_000), 1_200);
        assert_eq!(silent_sample_count(&"x".repeat(1_000), 16_000), 4_000);
        assert_eq!(silent_sample_count(&"x".repeat(1_000), 24_000), 6_000);
        // Unicode scalar values, not bytes.
        assert_eq!(silent_sample_count("åäö", 16_000), 800);
    }
}
