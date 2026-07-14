//! Local streaming synthesis — the Pocket TTS worker
//! (`PocketTtsBackend`, `speech_client.py:238-468`).
//!
//! Pocket produces PCM chunks directly: the first one arrives long before the
//! sentence is finished, which is what makes a local voice feel immediate. The
//! chunks are validated at this boundary and decoded here — base64 exists only
//! on the worker's stdout, never in the game (D7/§7.1). Cloud TTS now shares the
//! downstream PCM presentation path by incrementally decoding its WAV body.
//!
//! Two asymmetries with the cloud path, both deliberate:
//!
//! * the **worker** resolves `TTS_POCKET_VOICE_*` itself; the parent sends the
//!   logical key (`sven`/`conny`/`ilse`) and nothing else;
//! * a malformed *chunk* kills the worker (the stream can no longer be trusted),
//!   while a malformed *completion* only fails the utterance
//!   (`speech_client.py:331-350`).

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use cathedral_sim::{SpeechError, Subsystem};
use serde_json::{Map, Value};

use crate::{
    config::SpeechSettings,
    events::BackendSender,
    tts::{PcmChunk, StreamCompletion, logical_voice, validate_tts_text},
    wav::MAX_PCM_CHUNK_BYTES,
    worker::{Worker, WorkerMessages, WorkerSpec, WorkerStep},
};

/// `speech_client.py:274-357`.
const MESSAGES: WorkerMessages = WorkerMessages {
    start_failed: "could not start local Pocket TTS; make sure uv is available",
    unavailable: "local Pocket TTS worker script is unavailable",
    exited: "local Pocket TTS worker exited; check the actor log",
    invalid_json: "local Pocket TTS returned invalid JSON",
    invalid_response: "local Pocket TTS returned an invalid response",
    write_failed: "local Pocket TTS worker stopped",
    load_failed: "local Pocket TTS failed to load",
};

const INVALID_CHUNK: &str = "local Pocket TTS returned an invalid audio chunk";
const INVALID_COMPLETION: &str = "local Pocket TTS returned an invalid completion";
const FAILED: &str = "local Pocket TTS synthesis failed";

/// The sample rates a voice model may plausibly speak at
/// (`speech_client.py:326`).
const MIN_SAMPLE_RATE: u64 = 8_000;
const MAX_SAMPLE_RATE: u64 = 48_000;
/// The base64 field's own ceiling, before decoding (`speech_client.py:329`).
const MAX_CHUNK_BASE64_CHARS: usize = 256_000;

/// The Pocket TTS driver.
#[derive(Debug)]
pub struct PocketTts {
    worker: Worker,
}

impl PocketTts {
    pub fn new(settings: &SpeechSettings, events: BackendSender) -> Self {
        Self {
            worker: Worker::new(spec(settings), events),
        }
    }

    pub fn available(&self) -> bool {
        self.worker.available()
    }

    /// Start the worker and load the model *before* the first line is spoken
    /// (`speech_client.py:270-272`).
    pub fn warm(&self) -> Result<(), SpeechError> {
        self.worker.warm()
    }

    /// Synthesize one utterance, handing each chunk to `on_chunk` the moment it
    /// arrives. Blocking: it runs on the TTS engine's worker thread.
    pub fn synthesize_stream(
        &self,
        text: &str,
        voice_key: &str,
        mut on_chunk: impl FnMut(PcmChunk),
    ) -> Result<StreamCompletion, SpeechError> {
        validate_tts_text(text)?;
        let voice = logical_voice(voice_key)?;

        self.worker
            .publish_status("synthesizing", "Streaming with local Pocket TTS");

        // The worker rejects any request whose key set differs from exactly
        // these three plus `request_id` (R15).
        let mut body = Map::new();
        body.insert("text".to_string(), Value::from(text));
        body.insert("voice_key".to_string(), Value::from(voice));

        let mut expected_seq: u32 = 0;
        let mut completion = None;

        self.worker.request(body, |message| {
            match message.get("type").and_then(Value::as_str) {
                Some("chunk") => match parse_chunk(message, expected_seq) {
                    Ok(chunk) => {
                        expected_seq += 1;
                        on_chunk(chunk);
                        WorkerStep::Continue
                    }
                    // A chunk we cannot trust means a stream we cannot trust:
                    // the contiguity of `chunk_seq` is the game's only ordering
                    // guarantee, so the worker goes.
                    Err(message) => WorkerStep::Fail {
                        message,
                        forget: true,
                    },
                },
                Some("result") => match parse_completion(message, expected_seq) {
                    Ok(finished) => {
                        completion = Some(finished);
                        WorkerStep::Done
                    }
                    // Only this utterance is lost; the worker is still coherent.
                    Err(message) => WorkerStep::Fail {
                        message,
                        forget: false,
                    },
                },
                _ => WorkerStep::Fail {
                    message: message
                        .get("error")
                        .and_then(Value::as_str)
                        .filter(|error| !error.trim().is_empty())
                        .unwrap_or(FAILED)
                        .to_string(),
                    forget: false,
                },
            }
        })?;

        completion.ok_or_else(|| SpeechError::new(INVALID_COMPLETION))
    }

    pub fn close(&self) {
        self.worker.close();
    }

    #[cfg(test)]
    fn spawn_count(&self) -> u64 {
        self.worker.spawn_count()
    }
}

/// `speech_client.py:318-336` — contiguous seq, a believable rate, a bounded
/// non-empty payload that decodes to whole 16-bit samples.
fn parse_chunk(message: &Map<String, Value>, expected_seq: u32) -> Result<PcmChunk, String> {
    let invalid = || INVALID_CHUNK.to_string();

    let seq = message
        .get("chunk_seq")
        .and_then(Value::as_u64)
        .ok_or_else(invalid)?;
    if seq != u64::from(expected_seq) {
        return Err(invalid());
    }
    let sample_rate = message
        .get("sample_rate")
        .and_then(Value::as_u64)
        .filter(|rate| (MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(rate))
        .ok_or_else(invalid)?;
    let encoded = message
        .get("pcm_s16le_base64")
        .and_then(Value::as_str)
        .filter(|encoded| !encoded.is_empty() && encoded.len() <= MAX_CHUNK_BASE64_CHARS)
        .ok_or_else(invalid)?;

    let bytes = STANDARD.decode(encoded).map_err(|_| invalid())?;
    if bytes.is_empty() || bytes.len() % 2 != 0 || bytes.len() > MAX_PCM_CHUNK_BYTES {
        return Err(invalid());
    }
    let samples: Vec<i16> = bytes
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect();

    Ok(PcmChunk {
        seq: expected_seq,
        sample_rate: sample_rate as u32,
        samples: Arc::from(samples.as_slice()),
    })
}

/// `speech_client.py:338-351` — the worker must agree with us about how many
/// chunks it sent, and there must have been at least one.
fn parse_completion(message: &Map<String, Value>, seen: u32) -> Result<StreamCompletion, String> {
    let invalid = || INVALID_COMPLETION.to_string();

    let chunk_count = message
        .get("chunk_count")
        .and_then(Value::as_u64)
        .ok_or_else(invalid)?;
    if chunk_count != u64::from(seen) || seen == 0 {
        return Err(invalid());
    }
    let first_chunk_ms = message
        .get("first_chunk_ms")
        .and_then(Value::as_u64)
        .ok_or_else(invalid)?;

    Ok(StreamCompletion {
        chunk_count: seen,
        first_chunk_ms: first_chunk_ms.min(u64::from(u32::MAX)) as u32,
    })
}

/// `speech_client.py:388-403` — no `--resolution`, and no *extra* environment of
/// our own: the worker reads `TTS_POCKET_VOICE_<NAME>` out of its environment
/// itself (`pocket_tts_worker.py:64-69`). What it cannot read by itself is
/// `.env` — Python's `load_dotenv` had put those keys into `os.environ` before
/// the spawn, so they must be passed along here (`Environment::worker_env`).
fn spec(settings: &SpeechSettings) -> WorkerSpec {
    let script = settings.pocket_script();
    WorkerSpec {
        program: settings.uv_binary.clone(),
        args: vec![
            "run".to_string(),
            "--python".to_string(),
            settings.local_tts_python.clone(),
            "--script".to_string(),
            script.display().to_string(),
        ],
        env: settings.worker_env.clone(),
        script,
        log_source: "tts",
        messages: MESSAGES,
        subsystem: Subsystem::Tts,
        backend: "local",
        loading_message: "Preparing local dependencies, model, and Pocket TTS voices",
        ready_message: "Local Pocket TTS is loaded and streaming-ready",
        // Pocket's install is small; Python publishes no uv progress for it.
        install_progress_statuses: false,
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
    use std::collections::BTreeMap;

    fn settings(stub: &StubWorker, pairs: &[(&str, &str)]) -> SpeechSettings {
        let vars: BTreeMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        let speech = BackendsConfig::resolve(
            &Environment::from_map(vars),
            &BackendsOptions {
                dotenv_path: None,
                workers_dir: stub.directory.clone(),
                uv_binary: stub.program.display().to_string(),
                fake_mode: false,
            },
        )
        .speech;
        std::fs::rename(&stub.script, speech.pocket_script()).expect("worker script");
        speech
    }

    /// Two silent 16-bit samples.
    const SILENCE: &str = "AAAAAA==";

    /// speech-python.md test 8: two streaming synths on one process, chunks
    /// delivered as they arrive, logical voice keys on the wire, ids 1 and 2.
    #[test]
    fn the_worker_streams_chunks_and_is_reused() {
        let stub = StubWorker::new(
            "pocket",
            &[
                r#"{"type":"ready","sample_rate":24000}"#,
                &format!(
                    r#"{{"type":"chunk","request_id":1,"chunk_seq":0,"sample_rate":24000,"pcm_s16le_base64":"{SILENCE}"}}
{{"type":"result","request_id":1,"chunk_count":1,"first_chunk_ms":187}}"#
                ),
                &format!(
                    r#"{{"type":"chunk","request_id":2,"chunk_seq":0,"sample_rate":24000,"pcm_s16le_base64":"{SILENCE}"}}
{{"type":"result","request_id":2,"chunk_count":1,"first_chunk_ms":191}}"#
                ),
            ],
        );
        let settings = settings(&stub, &[]);
        let (sender, _events) = backend_channel();
        let pocket = PocketTts::new(&settings, sender);
        assert!(pocket.available());

        let mut chunks = Vec::new();
        assert_eq!(
            pocket
                .synthesize_stream("First", "sven", |chunk| chunks.push(chunk))
                .expect("streamed"),
            StreamCompletion {
                chunk_count: 1,
                first_chunk_ms: 187
            }
        );
        assert_eq!(
            pocket
                .synthesize_stream("Second", "ilse", |chunk| chunks.push(chunk))
                .expect("streamed"),
            StreamCompletion {
                chunk_count: 1,
                first_chunk_ms: 191
            }
        );
        assert_eq!(pocket.spawn_count(), 1, "one worker, two utterances");

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| (chunk.seq, chunk.sample_rate))
                .collect::<Vec<_>>(),
            vec![(0, 24_000), (0, 24_000)]
        );
        // Base64 dies at this boundary: the game gets samples.
        assert_eq!(chunks[0].samples.as_ref(), &[0i16, 0]);

        let requests = stub.requests();
        assert_eq!(
            requests
                .iter()
                .map(|request| request["voice_key"].as_str().expect("a voice"))
                .collect::<Vec<_>>(),
            vec!["sven", "ilse"],
            "the parent sends the logical key; the worker resolves the voice"
        );
        assert_eq!(
            requests
                .iter()
                .map(|request| request["request_id"].as_u64().expect("an id"))
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            requests[0].keys().collect::<Vec<_>>(),
            vec!["request_id", "text", "voice_key"],
            "the worker rejects any other key set"
        );

        assert_eq!(
            stub.argv()[..4],
            [
                "run".to_string(),
                "--python".to_string(),
                "3.12".to_string(),
                "--script".to_string(),
            ],
            "no --resolution for Pocket"
        );
        pocket.close();
    }

    /// A chunk stream we cannot trust is not a stream: the worker dies with it.
    #[test]
    fn a_malformed_chunk_kills_the_worker() {
        for bad in [
            // A gap in the sequence: the game would play the audio out of order.
            r#"{"type":"chunk","request_id":1,"chunk_seq":1,"sample_rate":24000,"pcm_s16le_base64":"AAAAAA=="}"#,
            // A rate no voice model produces.
            r#"{"type":"chunk","request_id":1,"chunk_seq":0,"sample_rate":96000,"pcm_s16le_base64":"AAAAAA=="}"#,
            // Not base64.
            r#"{"type":"chunk","request_id":1,"chunk_seq":0,"sample_rate":24000,"pcm_s16le_base64":"!!!!"}"#,
            // Three bytes are not whole 16-bit samples.
            r#"{"type":"chunk","request_id":1,"chunk_seq":0,"sample_rate":24000,"pcm_s16le_base64":"AAAA"}"#,
            // Empty audio.
            r#"{"type":"chunk","request_id":1,"chunk_seq":0,"sample_rate":24000,"pcm_s16le_base64":""}"#,
        ] {
            let stub = StubWorker::new("bad-chunk", &[r#"{"type":"ready"}"#, bad]);
            let settings = settings(&stub, &[]);
            let (sender, _events) = backend_channel();
            let pocket = PocketTts::new(&settings, sender);

            let error = pocket
                .synthesize_stream("Hello", "conny", |_| panic!("no chunk may be delivered"))
                .expect_err("a malformed chunk");
            assert_eq!(error.presentable, INVALID_CHUNK, "{bad}");
            assert_eq!(pocket.spawn_count(), 1);
            // Forgotten: the next call would spawn a fresh child.
            pocket.close();
        }
    }

    /// A completion that disagrees with the stream fails the utterance — but the
    /// worker is still coherent and keeps its model resident.
    #[test]
    fn a_malformed_completion_fails_only_the_utterance() {
        let stub = StubWorker::new(
            "bad-completion",
            &[
                r#"{"type":"ready"}"#,
                &format!(
                    r#"{{"type":"chunk","request_id":1,"chunk_seq":0,"sample_rate":24000,"pcm_s16le_base64":"{SILENCE}"}}
{{"type":"result","request_id":1,"chunk_count":7,"first_chunk_ms":10}}"#
                ),
                &format!(
                    r#"{{"type":"chunk","request_id":2,"chunk_seq":0,"sample_rate":24000,"pcm_s16le_base64":"{SILENCE}"}}
{{"type":"result","request_id":2,"chunk_count":1,"first_chunk_ms":12}}"#
                ),
            ],
        );
        let settings = settings(&stub, &[]);
        let (sender, _events) = backend_channel();
        let pocket = PocketTts::new(&settings, sender);

        assert_eq!(
            pocket
                .synthesize_stream("Hello", "sven", |_| {})
                .expect_err("the count is a lie")
                .presentable,
            INVALID_COMPLETION
        );
        assert_eq!(
            pocket
                .synthesize_stream("Hello again", "sven", |_| {})
                .expect("the worker survived"),
            StreamCompletion {
                chunk_count: 1,
                first_chunk_ms: 12
            }
        );
        assert_eq!(pocket.spawn_count(), 1, "the model stayed resident");
        pocket.close();
    }

    /// Zero chunks is a failure, not a silent utterance the floor waits out.
    #[test]
    fn a_synthesis_with_no_audio_is_a_failure() {
        let stub = StubWorker::new(
            "no-audio",
            &[
                r#"{"type":"ready"}"#,
                r#"{"type":"error","request_id":1,"error":"Pocket TTS produced no audio"}"#,
            ],
        );
        let settings = settings(&stub, &[]);
        let (sender, _events) = backend_channel();
        let pocket = PocketTts::new(&settings, sender);
        assert_eq!(
            pocket
                .synthesize_stream("Hello", "ilse", |_| {})
                .expect_err("no audio")
                .presentable,
            "Pocket TTS produced no audio"
        );
        pocket.close();
    }

    #[test]
    fn invalid_text_and_voices_never_reach_the_worker() {
        let stub = StubWorker::new("local-validation", &[r#"{"type":"ready"}"#]);
        let settings = settings(&stub, &[]);
        let (sender, _events) = backend_channel();
        let pocket = PocketTts::new(&settings, sender);

        assert!(pocket.synthesize_stream("", "sven", |_| {}).is_err());
        assert!(
            pocket
                .synthesize_stream("bad\0text", "sven", |_| {})
                .is_err()
        );
        assert!(
            pocket
                .synthesize_stream("Hello", "gandalf", |_| {})
                .is_err()
        );
        assert_eq!(pocket.spawn_count(), 0, "no worker was even started");
        pocket.close();
    }

    /// `pocket_tts_worker.py:64-69` reads `TTS_POCKET_VOICE_<NAME>` out of its
    /// own environment, and `huggingface_hub` reads `HF_TOKEN` out of it —
    /// neither is anything this crate interprets. Under Python they arrived by
    /// `os.environ.copy()` after `load_dotenv`; here they have to be handed over,
    /// or Ilse silently falls back to the worker's built-in voice.
    #[test]
    fn the_dotenv_keys_the_worker_reads_itself_reach_the_child() {
        let mut vars = BTreeMap::new();
        vars.insert("TTS_POCKET_VOICE_ILSE".to_string(), "af_bella".to_string());
        vars.insert("HF_TOKEN".to_string(), "hf_secret".to_string());
        let speech = BackendsConfig::resolve(
            &Environment::from_dotenv_map(vars),
            &BackendsOptions {
                dotenv_path: None,
                workers_dir: std::path::PathBuf::from("/nonexistent"),
                uv_binary: "uv".to_string(),
                fake_mode: false,
            },
        )
        .speech;

        let spawned = spec(&speech);
        assert!(
            spawned
                .env
                .contains(&("TTS_POCKET_VOICE_ILSE".to_string(), "af_bella".to_string())),
            "{:?}",
            spawned.env
        );
        assert!(
            spawned
                .env
                .contains(&("HF_TOKEN".to_string(), "hf_secret".to_string())),
            "{:?}",
            spawned.env
        );
    }
}
