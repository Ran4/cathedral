//! Cloud transcription — `POST /v1/audio/transcriptions`
//! (`OpenAISpeechBackend.transcribe`, `speech_client.py:137-178`).
//!
//! Two details are load-bearing:
//!
//! * the multipart part **must** carry a `.wav` filename, because that is how
//!   the endpoint sniffs the format (the Python SDK sent the file's basename);
//! * player recordings are **32-bit float PCM (format tag 3)** and must be
//!   uploaded byte-for-byte — re-encoding them here would be a silent quality
//!   loss and a whole class of bugs (speech-python.md §2.1, risk 4).
//!
//! Availability is a key check, never a network probe: an absent
//! `OPENAI_API_KEY` degrades cloud speech and nothing else.

use std::{path::Path, time::Duration};

use cathedral_sim::SpeechError;
use serde::Deserialize;

use crate::{
    config::{SpeechSettings, timeout_duration},
    wav::{MAX_WAV_BYTES, require_existing_wav},
    worker::truncate,
};

/// `speech_client.py:156-158`.
pub const NO_KEY_MESSAGE: &str = "OPENAI_API_KEY is not configured for speech services";
/// `speech_client.py:177`.
pub const NO_TEXT_MESSAGE: &str = "transcription service returned no text";

/// One retry, exactly like the SDK's `max_retries=1` (`speech_client.py:163`).
const MAX_ATTEMPTS: u32 = 2;
const RETRY_BACKOFF: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    #[serde(default)]
    text: Option<String>,
}

/// The cloud STT client: stateless, one request per utterance.
#[derive(Debug, Clone)]
pub struct CloudTranscriber {
    http: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    model: String,
    timeout: Duration,
}

impl CloudTranscriber {
    pub fn new(settings: &SpeechSettings) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: settings.api_key.clone(),
            base_url: settings.base_url.trim_end_matches('/').to_string(),
            model: settings.stt_model.clone(),
            timeout: timeout_duration(settings.timeout_seconds),
        }
    }

    /// A key exists. No network is touched (`speech_client.py:146-148`).
    pub fn available(&self) -> bool {
        self.api_key.is_some()
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Transcribe one finished recording.
    pub async fn transcribe(&self, wav_path: &Path) -> Result<String, SpeechError> {
        let Some(api_key) = self.api_key.clone() else {
            return Err(SpeechError::new(NO_KEY_MESSAGE));
        };
        // Both checks happen locally, before any request: a missing file is the
        // caller's bug, not the provider's problem.
        require_existing_wav(wav_path)?;
        let bytes = std::fs::read(wav_path)
            .map_err(|_| SpeechError::new("transcription input could not be read"))?;
        if bytes.len() > MAX_WAV_BYTES {
            return Err(SpeechError::new("recording exceeds 16 MiB"));
        }
        // The endpoint sniffs the format from the filename part.
        let filename = wav_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("recording.wav")
            .to_string();

        let url = format!("{}/audio/transcriptions", self.base_url);
        let mut attempt = 0;
        loop {
            attempt += 1;
            let form = reqwest::multipart::Form::new()
                .text("model", self.model.clone())
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(bytes.clone())
                        .file_name(filename.clone())
                        .mime_str("audio/wav")
                        .map_err(|_| SpeechError::new("transcription upload is malformed"))?,
                );

            let response = self
                .http
                .post(&url)
                .bearer_auth(&api_key)
                .timeout(self.timeout)
                .multipart(form)
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(RETRY_BACKOFF).await;
                        continue;
                    }
                    return Err(transport_error("transcription", &error));
                }
            };

            let status = response.status();
            if !status.is_success() {
                if attempt < MAX_ATTEMPTS && is_retryable(status.as_u16()) {
                    tokio::time::sleep(RETRY_BACKOFF).await;
                    continue;
                }
                return Err(SpeechError::new(truncate(
                    &format!("cloud transcription failed (HTTP {})", status.as_u16()),
                    160,
                )));
            }

            let payload: TranscriptionResponse = response
                .json()
                .await
                .map_err(|_| SpeechError::new(NO_TEXT_MESSAGE))?;
            return payload
                .text
                .ok_or_else(|| SpeechError::new(NO_TEXT_MESSAGE));
        }
    }
}

/// A timeout is worth its own sentence — it is the failure the player is most
/// likely to see, and "failed" would tell them nothing.
pub(crate) fn transport_error(what: &str, error: &reqwest::Error) -> SpeechError {
    if error.is_timeout() {
        SpeechError::new(format!("cloud speech provider timed out ({what})"))
    } else {
        SpeechError::new(truncate(
            &format!("cloud {what} failed to reach the provider"),
            160,
        ))
    }
}

/// The SDK's retryable set (`llm.rs` uses the same one).
pub(crate) fn is_retryable(status: u16) -> bool {
    matches!(status, 408 | 409 | 429) || (500..600).contains(&status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{BackendsConfig, BackendsOptions, Environment},
        runtime::BackendRuntime,
        testing::MockServer,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn settings(pairs: &[(&str, &str)]) -> SpeechSettings {
        let vars: BTreeMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        BackendsConfig::resolve(
            &Environment::from_map(vars),
            &BackendsOptions {
                dotenv_path: None,
                workers_dir: PathBuf::from("/nonexistent"),
                uv_binary: "uv".to_string(),
                fake_mode: false,
            },
        )
        .speech
    }

    /// A float32 recording, as the microphone writes them.
    fn recording(directory: &Path) -> PathBuf {
        let path = directory.join("input.wav");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&44u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&24_000u32.to_le_bytes());
        bytes.extend_from_slice(&96_000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 8]);
        std::fs::write(&path, bytes).expect("a recording");
        path
    }

    fn tempdir(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cathedral-stt-{tag}-{}",
            crate::session_dir::SessionDir::new_session_id()
        ));
        std::fs::create_dir_all(&path).expect("temp dir");
        path
    }

    /// speech-python.md test 1 + 3: the model comes from `STT_MODEL`, defaulting
    /// to the high-accuracy one, and the upload is a `.wav`-named multipart part.
    #[test]
    fn a_recording_is_uploaded_as_a_wav_named_multipart_part() {
        let server = MockServer::start(vec![MockServer::ok(r#"{"text": "understood"}"#)]);
        let directory = tempdir("upload");
        let path = recording(&directory);

        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTranscriber::new(&settings(&[
            ("OPENAI_API_KEY", "sk-speech"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));
        assert!(client.available());
        assert_eq!(client.model(), "gpt-4o-transcribe", "the default STT model");

        let text = runtime
            .block_on(client.transcribe(&path))
            .expect("a transcript");
        assert_eq!(text, "understood");

        let request = server.request(0);
        assert_eq!(request.path, "/v1/audio/transcriptions");
        assert_eq!(
            request.header("authorization").as_deref(),
            Some("Bearer sk-speech")
        );
        assert!(
            request
                .header("content-type")
                .unwrap_or_default()
                .starts_with("multipart/form-data"),
            "{:?}",
            request.header("content-type")
        );
        assert!(request.body.contains("name=\"model\""), "{}", request.body);
        assert!(
            request.body.contains("gpt-4o-transcribe"),
            "{}",
            request.body
        );
        assert!(
            request.body.contains("filename=\"input.wav\""),
            "the endpoint sniffs the format from the filename: {}",
            request.body
        );
        assert!(request.body.contains("audio/wav"), "{}", request.body);
        // The float32 header reached the provider untouched (risk 4).
        assert!(request.body.contains("RIFF"), "{}", request.body);

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn the_model_is_configurable() {
        let server = MockServer::start(vec![MockServer::ok(r#"{"text": "ok"}"#)]);
        let directory = tempdir("model");
        let path = recording(&directory);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTranscriber::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
            ("STT_MODEL", "test-transcribe"),
        ]));
        assert_eq!(client.model(), "test-transcribe");
        runtime
            .block_on(client.transcribe(&path))
            .expect("a transcript");
        assert!(server.request(0).body.contains("test-transcribe"));
        std::fs::remove_dir_all(&directory).ok();
    }

    /// speech-python.md test 12: no key, no availability, and no network call.
    #[test]
    fn a_missing_key_is_unavailable_without_touching_the_network() {
        let directory = tempdir("nokey");
        let path = recording(&directory);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTranscriber::new(&settings(&[]));
        assert!(!client.available());
        assert_eq!(
            runtime
                .block_on(client.transcribe(&path))
                .expect_err("no key")
                .presentable,
            NO_KEY_MESSAGE
        );
        std::fs::remove_dir_all(&directory).ok();
    }

    /// speech-python.md test 14: a missing file is rejected locally.
    #[test]
    fn a_missing_or_non_wav_file_is_rejected_before_the_request() {
        let server = MockServer::start(vec![MockServer::ok(r#"{"text": "never"}"#)]);
        let directory = tempdir("missing");
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTranscriber::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));

        for path in ["missing.wav", "present.mp3"] {
            let path = directory.join(path);
            assert!(
                runtime.block_on(client.transcribe(&path)).is_err(),
                "{path:?}"
            );
        }
        assert_eq!(server.request_count(), 0, "no provider call was made");
        std::fs::remove_dir_all(&directory).ok();
    }

    /// speech-python.md test 13: a provider failure reaches the boundary as a
    /// presentable degrade, after exactly one retry.
    #[test]
    fn a_server_error_is_retried_once_and_then_degrades() {
        let server = MockServer::start(vec![
            MockServer::status(500, "upstream exploded"),
            MockServer::status(500, "still down"),
            MockServer::ok(r#"{"text": "never reached"}"#),
        ]);
        let directory = tempdir("retry");
        let path = recording(&directory);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTranscriber::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));

        let error = runtime
            .block_on(client.transcribe(&path))
            .expect_err("the provider is down");
        assert_eq!(error.presentable, "cloud transcription failed (HTTP 500)");
        assert_eq!(server.request_count(), 2, "one attempt plus one retry");

        // A 400 is the caller's fault: no retry.
        let server = MockServer::start(vec![MockServer::status(400, "bad request")]);
        let client = CloudTranscriber::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));
        assert!(runtime.block_on(client.transcribe(&path)).is_err());
        assert_eq!(server.request_count(), 1);

        std::fs::remove_dir_all(&directory).ok();
    }

    /// speech-python.md test 13, the other half: a provider that accepts the
    /// connection and then says nothing is a *timeout*, and the player is told
    /// exactly that rather than "transcription failed".
    #[test]
    fn a_provider_that_never_answers_times_out() {
        let server = MockServer::start(vec![MockServer::hang(), MockServer::hang()]);
        let directory = tempdir("timeout");
        let path = recording(&directory);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTranscriber::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
            ("SPEECH_TIMEOUT_SECONDS", "0.3"),
        ]));

        let error = runtime
            .block_on(client.transcribe(&path))
            .expect_err("the provider never answers");
        assert_eq!(
            error.presentable,
            "cloud speech provider timed out (transcription)"
        );
        assert_eq!(server.request_count(), 2, "a timeout is retried once");
        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_reply_without_text_is_a_failure() {
        let server = MockServer::start(vec![MockServer::ok(r#"{"duration": 1.0}"#)]);
        let directory = tempdir("notext");
        let path = recording(&directory);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTranscriber::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));
        assert_eq!(
            runtime
                .block_on(client.transcribe(&path))
                .expect_err("no text")
                .presentable,
            NO_TEXT_MESSAGE
        );
        std::fs::remove_dir_all(&directory).ok();
    }
}
