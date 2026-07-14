//! The voice of the cast: backend selection, the synthesis queue, and the
//! validation both providers share (`server.py:1819-1956`, `_queue_tts`).
//!
//! [`TtsEngine`] is the [`Tts`] the engine holds. Its one rule is D7: **submit
//! is synchronous and may refuse**. The conversation floor only waits for an
//! utterance whose synthesis was actually taken, so an accept/refuse that
//! arrived a frame later would either strand the floor (waiting for audio that
//! was never queued) or cut a line off mid-word. Everything after the accept is
//! asynchronous and arrives on the backend channel.
//!
//! Synthesis runs on **one** worker thread, capacity 32, exactly like Python's
//! `_DaemonWorker`: utterances must be spoken in the order the world produced
//! them, and the Pocket worker can only serve one request at a time anyway.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use cathedral_sim::{
    SpeechError, SpeechEventId, StatusEvent, Subsystem, Tts, TtsBackendKind, TtsRequest,
    TtsSubmitError,
};
use crossbeam_channel::{Sender, TrySendError, bounded};

use crate::{
    config::{DEFAULT_OPENAI_VOICES, LOGICAL_NPC_VOICES, SpeechSettings},
    events::{BackendEvent, BackendSender},
    runtime::BackendRuntime,
    tts_cloud::{CloudSynthesis, CloudTts},
    tts_local::PocketTts,
};

/// `server.py:73` — the synthesis queue.
pub const TTS_QUEUE_CAPACITY: usize = 32;
/// `speech_client.py:95` — the cap the sim enforces on player speech too.
pub const MAX_SPEECH_TEXT_CHARS: usize = 500;
/// A resolved provider voice may not be a path, a flag, or a novel.
const MAX_VOICE_CHARS: usize = 64;

/// One decoded mono PCM chunk, ready for the game's streaming audio sink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmChunk {
    pub seq: u32,
    pub sample_rate: u32,
    pub samples: Arc<[i16]>,
}

/// What a finished streaming synthesis produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamCompletion {
    pub chunk_count: u32,
    /// Time from request submission to the first decoded audio sample.
    pub first_chunk_ms: u32,
}

// ------------------------------------------------------------- shared checks

/// `_validate_tts_text` (`speech_client.py:92-107`).
///
/// `\n` and `\t` are allowed (they shape the reading, and the models take them);
/// `\r`, DEL and the C1 block are not — they are what a prompt-injection or a
/// broken decoder looks like. Rust has no lone surrogates, so that clause of the
/// Python predicate is structurally impossible here.
pub fn validate_tts_text(text: &str) -> Result<(), SpeechError> {
    if text.trim().is_empty() {
        return Err(SpeechError::new("speech text must not be empty"));
    }
    if text.chars().count() > MAX_SPEECH_TEXT_CHARS {
        return Err(SpeechError::new(
            "speech text exceeds the 500 character limit",
        ));
    }
    let has_control = text.chars().any(|character| {
        let code = character as u32;
        (code < 0x20 && character != '\n' && character != '\t') || (0x7F..=0x9F).contains(&code)
    });
    if has_control {
        return Err(SpeechError::new("speech text contains control characters"));
    }
    Ok(())
}

/// The logical voice key of one of the three NPCs, normalized
/// (`speech_client.py:116-120`). Pocket takes this key as-is — it resolves the
/// actual voice inside the worker.
pub fn logical_voice(voice_key: &str) -> Result<String, SpeechError> {
    let key = voice_key.trim().to_lowercase();
    if !LOGICAL_NPC_VOICES.contains(&key.as_str()) {
        return Err(SpeechError::new("unknown logical NPC voice"));
    }
    Ok(key)
}

/// `_resolve_voice` for provider `openai` (`speech_client.py:110-134`).
///
/// Precedence: `TTS_OPENAI_VOICE_<KEY>`, then the legacy `TTS_VOICE_<KEY>`
/// (cloud only — it predates there being more than one provider), then the
/// default cast. The result is validated: a voice name is `[A-Za-z0-9_-]{1,64}`,
/// which is what stops a configured voice from being a path traversal.
pub fn resolve_openai_voice(
    settings: &SpeechSettings,
    voice_key: &str,
) -> Result<String, SpeechError> {
    let key = logical_voice(voice_key)?;
    let upper = key.to_uppercase();

    let voice = settings
        .voice_overrides
        .get(&format!("TTS_OPENAI_VOICE_{upper}"))
        .or_else(|| settings.voice_overrides.get(&format!("TTS_VOICE_{upper}")))
        .cloned()
        .unwrap_or_else(|| {
            DEFAULT_OPENAI_VOICES
                .iter()
                .find(|(logical, _)| *logical == key)
                .map(|(_, voice)| (*voice).to_string())
                .expect("every logical voice has a default")
        });

    let valid = !voice.is_empty()
        && voice.chars().count() <= MAX_VOICE_CHARS
        && voice.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        });
    if !valid {
        return Err(SpeechError::new(
            "configured voice contains invalid characters",
        ));
    }
    Ok(voice)
}

// ------------------------------------------------------------------- the engine

enum Job {
    Synthesize(TtsRequest),
    /// Load the model before the first line, not during it.
    Warm,
}

/// The [`Tts`] the engine speaks through.
pub struct TtsEngine {
    cloud: Option<Arc<CloudTts>>,
    local: Option<Arc<PocketTts>>,
    jobs: Sender<Job>,
    /// Event ids currently queued or synthesizing. Python reserved the output
    /// *path* (`<event_id>.wav`); in-process there is no file, but the same
    /// collision is still a bug worth refusing (`PathInUse`).
    in_flight: Arc<Mutex<HashSet<SpeechEventId>>>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for TtsEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TtsEngine")
            .field("cloud", &self.cloud.is_some())
            .field("local", &self.local.is_some())
            .finish_non_exhaustive()
    }
}

impl TtsEngine {
    pub fn new(
        runtime: Arc<BackendRuntime>,
        settings: &SpeechSettings,
        events: BackendSender,
    ) -> Self {
        let cloud = Some(Arc::new(CloudTts::new(settings)));
        let local = Some(Arc::new(PocketTts::new(settings, events.clone())));
        Self::with_backends(runtime, cloud, local, events)
    }

    fn with_backends(
        runtime: Arc<BackendRuntime>,
        cloud: Option<Arc<CloudTts>>,
        local: Option<Arc<PocketTts>>,
        events: BackendSender,
    ) -> Self {
        let (jobs, inbox) = bounded::<Job>(TTS_QUEUE_CAPACITY);
        let in_flight: Arc<Mutex<HashSet<SpeechEventId>>> = Arc::new(Mutex::new(HashSet::new()));

        let worker = {
            let cloud = cloud.clone();
            let local = local.clone();
            let in_flight = Arc::clone(&in_flight);
            std::thread::Builder::new()
                .name("cathedral-tts".to_string())
                .spawn(move || {
                    // The channel closing (the engine dropped) ends the thread.
                    for job in inbox {
                        match job {
                            Job::Warm => warm(local.as_deref(), &events),
                            Job::Synthesize(request) => {
                                let event_id = request.event_id.clone();
                                synthesize(
                                    &runtime,
                                    cloud.as_deref(),
                                    local.as_deref(),
                                    request,
                                    &events,
                                );
                                in_flight.lock().expect("tts in flight").remove(&event_id);
                            }
                        }
                    }
                })
                .expect("a synthesis thread")
        };

        Self {
            cloud,
            local,
            jobs,
            in_flight,
            worker: Some(worker),
        }
    }
}

/// `_warm_local_tts` (`server.py:1796-1817`): a warmup failure is a degraded
/// pill, never a dead engine — the player can still switch to cloud or off.
fn warm(local: Option<&PocketTts>, events: &BackendSender) {
    let Some(local) = local else { return };
    if let Err(error) = local.warm() {
        events.send(StatusEvent {
            subsystem: Subsystem::Tts,
            state: "degraded".to_string(),
            actor_id: None,
            message: Some(if error.presentable.is_empty() {
                "local Pocket TTS warmup failed".to_string()
            } else {
                error.presentable
            }),
            backend: Some("local".to_string()),
        });
    }
}

fn synthesize(
    runtime: &BackendRuntime,
    cloud: Option<&CloudTts>,
    local: Option<&PocketTts>,
    request: TtsRequest,
    events: &BackendSender,
) {
    let TtsRequest {
        event_id,
        text,
        voice_key,
        kind,
    } = request;

    match kind {
        TtsBackendKind::Cloud => {
            let Some(cloud) = cloud else {
                return fail(events, &event_id, unavailable(kind), kind);
            };
            let streamed_to = event_id.clone();
            let outcome = runtime.block_on(cloud.synthesize_stream(&text, &voice_key, |chunk| {
                events.send(BackendEvent::TtsChunk {
                    event_id: streamed_to.clone(),
                    seq: chunk.seq,
                    sample_rate: chunk.sample_rate,
                    samples: chunk.samples,
                });
            }));
            match outcome {
                Ok(CloudSynthesis::Streamed(completion)) => {
                    events.send(BackendEvent::TtsStreamEnd {
                        event_id,
                        chunk_count: completion.chunk_count,
                        first_chunk_ms: completion.first_chunk_ms,
                    })
                }
                Ok(CloudSynthesis::Buffered(wav)) => events.send(BackendEvent::TtsDone {
                    event_id,
                    result: Ok(wav),
                }),
                Err(error) => fail(events, &event_id, error, kind),
            }
        }
        TtsBackendKind::Local => {
            let Some(local) = local else {
                return fail(events, &event_id, unavailable(kind), kind);
            };
            let streamed_to = event_id.clone();
            let outcome = local.synthesize_stream(&text, &voice_key, |chunk| {
                // Chunks reach the game the moment they exist: this is the whole
                // point of the local voice.
                events.send(BackendEvent::TtsChunk {
                    event_id: streamed_to.clone(),
                    seq: chunk.seq,
                    sample_rate: chunk.sample_rate,
                    samples: chunk.samples,
                });
            });
            match outcome {
                Ok(completion) => events.send(BackendEvent::TtsStreamEnd {
                    event_id,
                    chunk_count: completion.chunk_count,
                    first_chunk_ms: completion.first_chunk_ms,
                }),
                // A stream that died halfway is still a failure: the game has
                // played some samples, and the floor must be released.
                Err(error) => fail(events, &event_id, error, kind),
            }
        }
        TtsBackendKind::Off => fail(events, &event_id, unavailable(kind), kind),
    }
}

fn unavailable(kind: TtsBackendKind) -> SpeechError {
    SpeechError::new(format!(
        "{} NPC voice backend is unavailable",
        kind.as_str()
    ))
}

/// The **only** thing a finished synthesis publishes is its outcome.
///
/// Not a status: the terminal `tts` rows (`idle`, `degraded`, and the
/// "First cloud/local PCM in N ms" one) are the speech router's to emit, exactly as
/// they were `_poll_tts`'s in Python (`server.py:1854-1956`) and never the
/// driver's. Emitting them here as well doubled every row in `logs.jsonl` and
/// every HUD pill update. The driver still publishes its *own* lifecycle —
/// `loading` / `ready` / `synthesizing` from `worker.rs`, and the warmup
/// degrade above — which is what Python's driver queue published.
///
/// `TtsDone(Err)` is what releases the conversation floor (the engine turns it
/// into `TtsFailed`): a failed utterance must never hold the cast silent.
fn fail(
    events: &BackendSender,
    event_id: &SpeechEventId,
    error: SpeechError,
    kind: TtsBackendKind,
) {
    let _ = kind;
    events.send(BackendEvent::TtsDone {
        event_id: event_id.clone(),
        result: Err(error),
    });
}

impl Tts for TtsEngine {
    fn available(&self, kind: TtsBackendKind) -> bool {
        match kind {
            TtsBackendKind::Cloud => self.cloud.as_ref().is_some_and(|cloud| cloud.available()),
            TtsBackendKind::Local => self.local.as_ref().is_some_and(|local| local.available()),
            TtsBackendKind::Off => false,
        }
    }

    /// Synchronous accept/refuse (D7). Every refusal is a `tts_failed` upstream,
    /// which releases the floor — the cast reads the line instead of speaking it.
    fn submit(&mut self, request: TtsRequest) -> Result<(), TtsSubmitError> {
        if request.kind == TtsBackendKind::Off || !self.available(request.kind) {
            return Err(TtsSubmitError::Unavailable);
        }
        let event_id = request.event_id.clone();
        {
            let mut in_flight = self.in_flight.lock().expect("tts in flight");
            if !in_flight.insert(event_id.clone()) {
                return Err(TtsSubmitError::PathInUse);
            }
        }
        match self.jobs.try_send(Job::Synthesize(request)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.in_flight
                    .lock()
                    .expect("tts in flight")
                    .remove(&event_id);
                Err(TtsSubmitError::QueueFull)
            }
            Err(TrySendError::Disconnected(_)) => {
                self.in_flight
                    .lock()
                    .expect("tts in flight")
                    .remove(&event_id);
                Err(TtsSubmitError::Unavailable)
            }
        }
    }

    fn warm(&mut self, kind: TtsBackendKind) {
        if kind == TtsBackendKind::Local && self.available(kind) {
            // Queued rather than threaded: the warmup and the first utterance
            // share the one worker, so there is nothing to race.
            let _ = self.jobs.try_send(Job::Warm);
        }
    }
}

impl Drop for TtsEngine {
    fn drop(&mut self) {
        // Kill the child first: the worker thread may be blocked reading its
        // stdout, and closing that pipe is what unblocks it.
        if let Some(local) = &self.local {
            local.close();
        }
        // Closing the queue ends the thread once it has drained. Deliberately
        // **not** joined: a provider call with 30 seconds left on its timeout
        // must not hold the game's exit open (Python joined for 0.1 s and moved
        // on for the same reason).
        let (dead, _) = bounded(0);
        drop(std::mem::replace(&mut self.jobs, dead));
        self.worker.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{BackendsConfig, BackendsOptions, Environment},
        events::backend_channel,
        worker::tests::StubWorker,
    };
    use crossbeam_channel::Receiver;
    use std::{collections::BTreeMap, path::PathBuf, time::Duration};

    fn settings(pairs: &[(&str, &str)], workers_dir: PathBuf, uv: &str) -> SpeechSettings {
        let vars: BTreeMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        BackendsConfig::resolve(
            &Environment::from_map(vars),
            &BackendsOptions {
                dotenv_path: None,
                workers_dir,
                uv_binary: uv.to_string(),
                fake_mode: false,
            },
        )
        .speech
    }

    fn event(sequence: i64) -> SpeechEventId {
        SpeechEventId(format!("speech-{sequence}"))
    }

    fn request(sequence: i64, kind: TtsBackendKind) -> TtsRequest {
        TtsRequest {
            event_id: event(sequence),
            text: "Two coppers, and not one less.".to_string(),
            voice_key: "sven".to_string(),
            kind,
        }
    }

    fn streamed_wav_body() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&24_000u32.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&100i16.to_le_bytes());
        bytes.extend_from_slice(&(-200i16).to_le_bytes());
        bytes
    }

    fn next(events: &Receiver<BackendEvent>) -> BackendEvent {
        events
            .recv_timeout(Duration::from_secs(10))
            .expect("a backend event")
    }

    /// The whole cloud path: submit accepts synchronously, PCM arrives first,
    /// and the keyed stream end arrives last. The terminal `tts` status still
    /// belongs to the speech router; a driver that also publishes one writes
    /// every row twice.
    #[test]
    fn a_cloud_utterance_streams_pcm_chunks_on_the_channel() {
        let server = crate::testing::MockServer::start(vec![crate::testing::MockServer::ok_bytes(
            &streamed_wav_body(),
        )]);
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, events) = backend_channel();
        let speech = settings(
            &[
                ("OPENAI_API_KEY", "sk"),
                ("OPENAI_BASE_URL", &server.base_url()),
            ],
            PathBuf::from("/nonexistent"),
            "uv",
        );
        let mut tts = TtsEngine::new(runtime, &speech, sender);

        assert!(tts.available(TtsBackendKind::Cloud));
        assert!(!tts.available(TtsBackendKind::Local), "no worker script");
        assert!(!tts.available(TtsBackendKind::Off));

        tts.submit(request(1, TtsBackendKind::Cloud))
            .expect("accepted");

        let BackendEvent::TtsChunk {
            event_id,
            seq,
            sample_rate,
            samples,
        } = next(&events)
        else {
            panic!("expected the first cloud PCM");
        };
        assert_eq!(event_id, event(1));
        assert_eq!(seq, 0);
        assert_eq!(sample_rate, 24_000);
        assert_eq!(samples.as_ref(), &[100, -200]);
        assert!(matches!(
            next(&events),
            BackendEvent::TtsStreamEnd {
                event_id,
                chunk_count: 1,
                ..
            } if event_id == event(1)
        ));
        assert_eq!(
            events.recv_timeout(Duration::from_millis(200)),
            Err(crossbeam_channel::RecvTimeoutError::Timeout),
            "no second `idle` row behind the router's own",
        );
    }

    /// A provider failure must still release the floor: `TtsDone(Err)` is how —
    /// and it is the only thing published. The `degraded` pill is the router's.
    #[test]
    fn a_failed_utterance_reports_a_failure_the_floor_can_release() {
        let server = crate::testing::MockServer::start(vec![
            crate::testing::MockServer::status(503, "down"),
            crate::testing::MockServer::status(503, "down"),
        ]);
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, events) = backend_channel();
        let speech = settings(
            &[
                ("OPENAI_API_KEY", "sk"),
                ("OPENAI_BASE_URL", &server.base_url()),
            ],
            PathBuf::from("/nonexistent"),
            "uv",
        );
        let mut tts = TtsEngine::new(runtime, &speech, sender);
        tts.submit(request(2, TtsBackendKind::Cloud))
            .expect("accepted");

        assert!(matches!(
            next(&events),
            BackendEvent::TtsDone { event_id, result } if event_id == event(2) && result.is_err()
        ));
        assert_eq!(
            events.recv_timeout(Duration::from_millis(200)),
            Err(crossbeam_channel::RecvTimeoutError::Timeout),
            "no second `degraded` row behind the router's own",
        );
    }

    /// The local voice streams: chunks first, stream end last, and the pill
    /// reports the number the local backend lives by.
    #[test]
    fn a_local_utterance_streams_pcm_chunks_and_a_stream_end() {
        let stub = StubWorker::new(
            "tts-engine",
            &[
                r#"{"type":"ready","sample_rate":24000}"#,
                r#"{"type":"chunk","request_id":1,"chunk_seq":0,"sample_rate":24000,"pcm_s16le_base64":"AAAAAA=="}
{"type":"chunk","request_id":1,"chunk_seq":1,"sample_rate":24000,"pcm_s16le_base64":"AQACAA=="}
{"type":"result","request_id":1,"chunk_count":2,"first_chunk_ms":187}"#,
            ],
        );
        let speech = settings(
            &[],
            stub.directory.clone(),
            &stub.program.display().to_string(),
        );
        std::fs::rename(&stub.script, speech.pocket_script()).expect("worker script");

        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, events) = backend_channel();
        let mut tts = TtsEngine::new(runtime, &speech, sender);
        assert!(tts.available(TtsBackendKind::Local));

        tts.submit(request(3, TtsBackendKind::Local))
            .expect("accepted");

        let mut chunks = Vec::new();
        let mut stream_end = None;
        while stream_end.is_none() {
            match next(&events) {
                BackendEvent::TtsChunk {
                    event_id,
                    seq,
                    sample_rate,
                    samples,
                } => {
                    assert_eq!(event_id, event(3));
                    assert_eq!(sample_rate, 24_000);
                    chunks.push((seq, samples));
                }
                BackendEvent::TtsStreamEnd {
                    event_id,
                    chunk_count,
                    first_chunk_ms,
                } => {
                    assert_eq!(event_id, event(3));
                    stream_end = Some((chunk_count, first_chunk_ms));
                }
                BackendEvent::Status(_) => {}
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(stream_end, Some((2, 187)));
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks[1].1.as_ref(), &[1i16, 2], "decoded, not base64");
    }

    /// D7 in one test: the queue's capacity is enforced *at submit time*, because
    /// the floor decides whether to wait on that answer alone.
    #[test]
    fn submit_refuses_synchronously_when_the_queue_is_full() {
        // A provider that accepts the connection and never answers: the worker
        // thread stays inside the first utterance for the whole test.
        let server = crate::testing::MockServer::start(vec![crate::testing::MockServer::hang()]);
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, _events) = backend_channel();
        let speech = settings(
            &[
                ("OPENAI_API_KEY", "sk"),
                ("OPENAI_BASE_URL", &server.base_url()),
            ],
            PathBuf::from("/nonexistent"),
            "uv",
        );
        let mut tts = TtsEngine::new(runtime, &speech, sender);

        // The provider never answers, so the worker never comes back for more:
        // one utterance in its hands, thirty-two in the queue, and no more.
        let mut accepted = 0;
        let mut refusals = Vec::new();
        for sequence in 0..(TTS_QUEUE_CAPACITY as i64 + 4) {
            match tts.submit(request(sequence, TtsBackendKind::Cloud)) {
                Ok(()) => accepted += 1,
                Err(error) => refusals.push(error),
            }
        }
        assert!(
            accepted <= TTS_QUEUE_CAPACITY + 1,
            "thirty-two queued plus the one being spoken: {accepted}"
        );
        assert!(!refusals.is_empty(), "the queue has a bottom");
        assert!(
            refusals
                .iter()
                .all(|error| *error == TtsSubmitError::QueueFull),
            "{refusals:?}"
        );
    }

    /// The same utterance twice is a bug in the caller, and a refusal here.
    #[test]
    fn the_same_event_cannot_be_synthesized_twice_at_once() {
        let server = crate::testing::MockServer::start(vec![crate::testing::MockServer::hang()]);
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, _events) = backend_channel();
        let speech = settings(
            &[
                ("OPENAI_API_KEY", "sk"),
                ("OPENAI_BASE_URL", &server.base_url()),
            ],
            PathBuf::from("/nonexistent"),
            "uv",
        );
        let mut tts = TtsEngine::new(runtime, &speech, sender);

        tts.submit(request(7, TtsBackendKind::Cloud))
            .expect("accepted");
        assert_eq!(
            tts.submit(request(7, TtsBackendKind::Cloud)),
            Err(TtsSubmitError::PathInUse),
        );
    }

    #[test]
    fn an_unavailable_backend_refuses_before_it_queues_anything() {
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, events) = backend_channel();
        // No key, no worker script: neither backend exists.
        let speech = settings(&[], PathBuf::from("/nonexistent"), "uv");
        let mut tts = TtsEngine::new(runtime, &speech, sender);

        assert_eq!(
            tts.submit(request(1, TtsBackendKind::Cloud)),
            Err(TtsSubmitError::Unavailable)
        );
        assert_eq!(
            tts.submit(request(1, TtsBackendKind::Local)),
            Err(TtsSubmitError::Unavailable)
        );
        assert_eq!(
            tts.submit(request(1, TtsBackendKind::Off)),
            Err(TtsSubmitError::Unavailable)
        );
        assert!(events.try_recv().is_err(), "a refusal emits nothing itself");
    }

    #[test]
    fn text_validation_is_the_python_predicate() {
        assert!(validate_tts_text("Hello").is_ok());
        assert!(
            validate_tts_text("two\nlines\tindented").is_ok(),
            "newline and tab pass"
        );
        assert!(validate_tts_text("").is_err());
        assert!(validate_tts_text("   ").is_err());
        assert!(
            validate_tts_text("bad\rreturn").is_err(),
            "carriage return does not"
        );
        assert!(validate_tts_text("bad\0nul").is_err());
        assert!(validate_tts_text("bad\u{7f}delete").is_err());
        assert!(validate_tts_text("bad\u{9f}c1").is_err());
        assert!(validate_tts_text(&"x".repeat(500)).is_ok());
        assert!(validate_tts_text(&"x".repeat(501)).is_err());
        // Unicode scalar values, not bytes: 500 é are 500 characters.
        assert!(validate_tts_text(&"é".repeat(500)).is_ok());
    }

    #[test]
    fn voice_resolution_prefers_the_qualified_override_and_rejects_hostile_names() {
        let plain = settings(&[], PathBuf::from("/nonexistent"), "uv");
        assert_eq!(
            resolve_openai_voice(&plain, "ilse").expect("a voice"),
            "nova"
        );
        assert_eq!(
            resolve_openai_voice(&plain, " SVEN ").expect("a voice"),
            "onyx"
        );
        assert_eq!(
            resolve_openai_voice(&plain, "conny").expect("a voice"),
            "echo"
        );
        assert!(resolve_openai_voice(&plain, "gandalf").is_err());
        assert!(resolve_openai_voice(&plain, "").is_err());

        let overridden = settings(
            &[
                ("TTS_VOICE_SVEN", "alloy"),
                ("TTS_OPENAI_VOICE_ILSE", "shimmer"),
            ],
            PathBuf::from("/nonexistent"),
            "uv",
        );
        assert_eq!(
            resolve_openai_voice(&overridden, "sven").expect("a voice"),
            "alloy"
        );
        assert_eq!(
            resolve_openai_voice(&overridden, "ilse").expect("a voice"),
            "shimmer"
        );

        let hostile = settings(
            &[("TTS_VOICE_ILSE", "../../bad")],
            PathBuf::from("/nonexistent"),
            "uv",
        );
        assert_eq!(
            resolve_openai_voice(&hostile, "ilse")
                .expect_err("a path is not a voice")
                .presentable,
            "configured voice contains invalid characters"
        );
    }
}
