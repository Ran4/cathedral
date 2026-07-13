//! Cloud synthesis — `POST /v1/audio/speech` (`OpenAISpeechBackend.synthesize`,
//! `speech_client.py:180-205`).
//!
//! Python streamed the response into `<event>.wav.part` and renamed it over the
//! target, because the game read the finished file off disk. In-process the WAV
//! never needs to exist: the bytes go straight onto the backend channel as
//! `TtsOutcome::Done`, which removes the temp file, the rename, the reservation
//! and the `audio_consumed` handshake in one go. The sanity gate that guarded
//! that file (`server.py:1927-1948`) stays — it now guards the bytes.

use std::{sync::Arc, time::Duration};

use cathedral_sim::SpeechError;
use serde::Serialize;

use crate::{
    config::SpeechSettings,
    stt_cloud::{NO_KEY_MESSAGE, is_retryable, transport_error},
    tts::{resolve_openai_voice, validate_tts_text},
    wav::accept_wav_bytes,
    worker::truncate,
};

const MAX_ATTEMPTS: u32 = 2;
const RETRY_BACKOFF: Duration = Duration::from_millis(500);

#[derive(Debug, Serialize)]
struct SpeechRequest<'a> {
    model: &'a str,
    voice: &'a str,
    input: &'a str,
    /// WAV, not MP3: the game decodes it with `hound` and plays raw samples.
    response_format: &'static str,
}

/// The cloud TTS client.
#[derive(Debug, Clone)]
pub struct CloudTts {
    http: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    model: String,
    timeout: Duration,
    settings: SpeechSettings,
}

impl CloudTts {
    pub fn new(settings: &SpeechSettings) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: settings.api_key.clone(),
            base_url: settings.base_url.trim_end_matches('/').to_string(),
            model: settings.tts_model.clone(),
            timeout: Duration::from_secs_f64(settings.timeout_seconds.max(0.001)),
            settings: settings.clone(),
        }
    }

    /// `speech_client.py:150-152` — the same key as cloud STT, probed the same
    /// way (not at all).
    pub fn available(&self) -> bool {
        self.api_key.is_some()
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// One utterance, as WAV bytes.
    ///
    /// Text and voice are validated **locally first** (`speech_client.py:180-183`):
    /// an empty line, a control character or a hostile voice override must never
    /// become a provider call.
    pub async fn synthesize(&self, text: &str, voice_key: &str) -> Result<Arc<[u8]>, SpeechError> {
        validate_tts_text(text)?;
        let voice = resolve_openai_voice(&self.settings, voice_key)?;
        let Some(api_key) = self.api_key.clone() else {
            return Err(SpeechError::new(NO_KEY_MESSAGE));
        };

        let url = format!("{}/audio/speech", self.base_url);
        let body = SpeechRequest {
            model: &self.model,
            voice: &voice,
            input: text,
            response_format: "wav",
        };

        let mut attempt = 0;
        let bytes = loop {
            attempt += 1;
            let response = self
                .http
                .post(&url)
                .bearer_auth(&api_key)
                .timeout(self.timeout)
                .json(&body)
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(RETRY_BACKOFF).await;
                        continue;
                    }
                    return Err(transport_error("synthesis", &error));
                }
            };

            let status = response.status();
            if !status.is_success() {
                if attempt < MAX_ATTEMPTS && is_retryable(status.as_u16()) {
                    tokio::time::sleep(RETRY_BACKOFF).await;
                    continue;
                }
                return Err(SpeechError::new(truncate(
                    &format!("cloud speech provider failed (HTTP {})", status.as_u16()),
                    160,
                )));
            }

            match response.bytes().await {
                Ok(bytes) => break bytes,
                Err(error) => return Err(transport_error("synthesis", &error)),
            }
        };

        // The same gate the file used to pass (`server.py:1927-1948`): the game's
        // audio sink must never be handed something it cannot play. The provider
        // streams its answer, so the WAV it sends declares 0xFFFFFFFF for its own
        // length; what leaves here is the repaired copy, because rodio is as
        // strict about that as the gate is.
        let (playable, _) = accept_wav_bytes(&bytes)?;
        Ok(Arc::from(playable.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{BackendsConfig, BackendsOptions, Environment},
        runtime::BackendRuntime,
        testing::MockServer,
    };
    use serde_json::Value;
    use std::{collections::BTreeMap, io::Cursor, path::PathBuf};

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

    /// A real, playable WAV — the mock provider has to answer with one, because
    /// the client refuses anything the audio sink could not play.
    fn wav_body() -> Vec<u8> {
        let specification = hound::WavSpec {
            channels: 1,
            sample_rate: 24_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut buffer, specification).expect("header");
            for _ in 0..240 {
                writer.write_sample(0i16).expect("sample");
            }
            writer.finalize().expect("finalize");
        }
        buffer.into_inner()
    }

    /// speech-python.md test 10: model `tts-1`, Ilse's default voice `nova`,
    /// `response_format: "wav"`.
    #[test]
    fn synthesis_asks_for_wav_in_the_voice_the_character_was_cast_with() {
        let server = MockServer::start(vec![MockServer::ok_bytes(&wav_body())]);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk-speech"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));
        assert!(client.available());
        assert_eq!(client.model(), "tts-1");

        let wav = runtime
            .block_on(client.synthesize("Greetings", "ilse"))
            .expect("synthesized");
        assert_eq!(&wav[..4], b"RIFF");

        let request = server.request(0);
        assert_eq!(request.path, "/v1/audio/speech");
        let body: Value = request.json();
        assert_eq!(body["model"], "tts-1");
        assert_eq!(body["voice"], "nova");
        assert_eq!(body["input"], "Greetings");
        assert_eq!(body["response_format"], "wav");
    }

    /// speech-python.md test 11: the legacy `TTS_VOICE_*` override still wins
    /// for the cloud provider.
    #[test]
    fn a_configured_voice_overrides_the_default() {
        let server = MockServer::start(vec![MockServer::ok_bytes(&wav_body())]);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
            ("TTS_VOICE_SVEN", "alloy"),
        ]));
        runtime
            .block_on(client.synthesize("Hello", "sven"))
            .expect("synthesized");
        assert_eq!(server.request(0).json()["voice"], "alloy");

        // The provider-qualified variable outranks the legacy one.
        let server = MockServer::start(vec![MockServer::ok_bytes(&wav_body())]);
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
            ("TTS_VOICE_SVEN", "alloy"),
            ("TTS_OPENAI_VOICE_SVEN", "onyx"),
        ]));
        runtime
            .block_on(client.synthesize("Hello", "sven"))
            .expect("synthesized");
        assert_eq!(server.request(0).json()["voice"], "onyx");
    }

    /// speech-python.md test 14: empty text, control characters and a
    /// path-traversal voice are all refused **before** the provider is called.
    #[test]
    fn hostile_text_and_voices_never_reach_the_provider() {
        let server = MockServer::start(vec![MockServer::ok_bytes(&wav_body())]);
        let runtime = BackendRuntime::new().expect("runtime");
        let base = server.base_url();

        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &base),
        ]));
        assert!(runtime.block_on(client.synthesize("", "ilse")).is_err());
        assert!(runtime.block_on(client.synthesize("   ", "ilse")).is_err());
        assert!(
            runtime
                .block_on(client.synthesize("bad\0speech", "ilse"))
                .is_err()
        );
        assert!(
            runtime
                .block_on(client.synthesize("bad\rspeech", "ilse"))
                .is_err()
        );
        assert!(
            runtime
                .block_on(client.synthesize(&"x".repeat(501), "ilse"))
                .is_err(),
            "the 500-character limit"
        );
        assert!(
            runtime
                .block_on(client.synthesize("Hello", "gandalf"))
                .is_err(),
            "only the three cast voices exist"
        );

        let hostile = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &base),
            ("TTS_VOICE_ILSE", "../../bad"),
        ]));
        assert!(
            runtime
                .block_on(hostile.synthesize("Hello", "ilse"))
                .is_err()
        );

        assert_eq!(server.request_count(), 0, "not one provider call");
    }

    #[test]
    fn a_missing_key_is_unavailable_and_a_provider_error_degrades() {
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[]));
        assert!(!client.available());
        assert_eq!(
            runtime
                .block_on(client.synthesize("Hello", "ilse"))
                .expect_err("no key")
                .presentable,
            NO_KEY_MESSAGE
        );

        let server = MockServer::start(vec![
            MockServer::status(503, "down"),
            MockServer::status(503, "still down"),
        ]);
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));
        assert_eq!(
            runtime
                .block_on(client.synthesize("Hello", "ilse"))
                .expect_err("the provider is down")
                .presentable,
            "cloud speech provider failed (HTTP 503)"
        );
        assert_eq!(server.request_count(), 2, "one retry");
    }

    /// The real provider streams its answer and therefore cannot fill in the
    /// lengths: RIFF and `data` both come back as 0xFFFFFFFF. Every cloud line
    /// was silent until this was handled — and what the client returns has to be
    /// playable by rodio, which is `hound`, which is what refused it.
    #[test]
    fn the_streaming_wav_the_provider_actually_sends_is_playable_when_it_arrives() {
        let mut streamed = Vec::new();
        streamed.extend_from_slice(b"RIFF");
        streamed.extend_from_slice(&u32::MAX.to_le_bytes());
        streamed.extend_from_slice(b"WAVE");
        streamed.extend_from_slice(b"fmt ");
        streamed.extend_from_slice(&16u32.to_le_bytes());
        streamed.extend_from_slice(&1u16.to_le_bytes());
        streamed.extend_from_slice(&1u16.to_le_bytes());
        streamed.extend_from_slice(&24_000u32.to_le_bytes());
        streamed.extend_from_slice(&48_000u32.to_le_bytes());
        streamed.extend_from_slice(&2u16.to_le_bytes());
        streamed.extend_from_slice(&16u16.to_le_bytes());
        streamed.extend_from_slice(b"data");
        streamed.extend_from_slice(&u32::MAX.to_le_bytes());
        streamed.extend(std::iter::repeat_n(0u8, 480));

        let server = MockServer::start(vec![MockServer::ok_bytes(&streamed)]);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));

        let wav = runtime
            .block_on(client.synthesize("Greetings", "ilse"))
            .expect("the provider's own bytes are accepted");
        let reader = hound::WavReader::new(Cursor::new(wav.as_ref())).expect("the game can play it");
        assert_eq!(reader.duration(), 240);
    }

    /// The provider answering with something unplayable is a degrade, not a
    /// crash in the audio sink three frames later.
    #[test]
    fn audio_the_game_could_not_play_is_refused_here() {
        let server = MockServer::start(vec![MockServer::ok("this is not a WAV")]);
        let runtime = BackendRuntime::new().expect("runtime");
        let client = CloudTts::new(&settings(&[
            ("OPENAI_API_KEY", "sk"),
            ("OPENAI_BASE_URL", &server.base_url()),
        ]));
        assert_eq!(
            runtime
                .block_on(client.synthesize("Hello", "ilse"))
                .expect_err("not a WAV")
                .presentable,
            "generated WAV is invalid"
        );
    }
}
