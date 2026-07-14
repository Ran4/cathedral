//! IO backends for the cathedral smart-actor engine.
//!
//! The impure half of the Python-sidecar port: the provider HTTP client, the
//! prompt archive, the speech workers (P6) and the private runtime directory.
//! cathedral-sim stays pure — it calls the [`Cognition`](cathedral_sim::Cognition)
//! / [`Transcription`](cathedral_sim::Transcription) / [`Tts`](cathedral_sim::Tts)
//! traits and receives results as plain values; everything that touches a socket,
//! a clock or the filesystem lives here (D7, D22, D23).
//!
//! Wiring, from the host's point of view:
//!
//! ```text
//!   Engine::poll  --trait call-->  HttpCognition / SttEngine / TtsEngine
//!                                        |  tokio task
//!   Engine::poll  <--EngineCommand--  BackendEvent channel  <----+
//! ```
//!
//! The host ([`LocalEngine`] in the game, the headless loop in the binary) holds
//! a [`BackendsHandle`], drains [`BackendEvent`]s once per pump, and hands them
//! to the engine. Dropping the handle stops every worker.
//!
//! [`LocalEngine`]: https://github.com/ (game crate, P5)

pub mod config;
pub mod events;
pub mod fake;
pub mod llm;
pub mod prompt_log;
pub mod runtime;
pub mod session_dir;
pub mod stt_cloud;
pub mod stt_local;
pub mod stt_realtime;
pub mod transcription;
pub mod tts;
pub mod tts_cloud;
pub mod tts_local;
pub mod wav;
pub mod worker;
pub mod world_data;

#[cfg(test)]
mod testing;

use std::{path::Path, sync::Arc};

use cathedral_sim::{Transcription, Tts, TtsBackendKind};
use crossbeam_channel::Receiver;

pub use config::{
    BackendCapabilities, BackendsConfig, BackendsOptions, Environment, LlmConfigError, LlmSettings,
    PROVIDERS, Provider, ProviderSpec, RealtimeSettings, SpeechSettings, select_tts_backend,
};
pub use events::{BackendEvent, BackendSender, backend_channel};
pub use fake::{DEFAULT_FAKE_TRANSCRIPT, FakeSpeech};
pub use llm::{HttpCognition, LlmClient, LlmError, PRICING, UsageLedger, pricing_for};
pub use prompt_log::{LocalTime, PromptExchange, PromptLog};
pub use runtime::BackendRuntime;
pub use session_dir::SessionDir;
pub use stt_cloud::CloudTranscriber;
pub use stt_local::CanaryTranscriber;
pub use stt_realtime::{RealtimeSttHandle, RealtimeTransport, TransportFactory};
pub use transcription::{STT_QUEUE_CAPACITY, SttEngine};
pub use tts::{PcmChunk, StreamCompletion, TTS_QUEUE_CAPACITY, TtsEngine};
pub use tts_cloud::CloudTts;
pub use tts_local::PocketTts;
pub use wav::{
    MAX_PCM_CHUNK_BYTES, MAX_WAV_BYTES, WavError, WavInfo, accept_wav_bytes, safe_session_path,
    validate_wav_bytes, wav_duration_seconds,
};
pub use worker::{LogSink, Worker, WorkerSpec, WorkerStep, set_log_sink};

/// Everything the host needs to run the backends: the runtime that owns their
/// tasks, the channel their results arrive on, the capability set the engine
/// reports at handshake, and the private audio directory.
///
/// Dropping it drops the runtime (tasks are abandoned, not awaited) and the
/// session directory (removed from disk).
pub struct BackendsHandle {
    runtime: Arc<BackendRuntime>,
    sender: BackendSender,
    events: Receiver<BackendEvent>,
    config: BackendsConfig,
    session_dir: Option<SessionDir>,
}

impl BackendsHandle {
    /// Start the runtime and the channel for a resolved configuration.
    ///
    /// Creating the session directory is optional: the headless runner and the
    /// tests have no audio at all.
    pub fn start(config: BackendsConfig, session_dir: Option<SessionDir>) -> std::io::Result<Self> {
        let runtime = BackendRuntime::new()?;
        let (sender, events) = backend_channel();
        Ok(Self {
            runtime,
            sender,
            events,
            config,
            session_dir,
        })
    }

    /// Load the environment, probe capabilities, and start (the normal path).
    pub fn launch(options: &BackendsOptions) -> std::io::Result<Self> {
        let config = BackendsConfig::load(options);
        let session_dir = SessionDir::create(&SessionDir::new_session_id()).ok();
        Self::start(config, session_dir)
    }

    pub fn config(&self) -> &BackendsConfig {
        &self.config
    }

    pub fn capabilities(&self) -> BackendCapabilities {
        self.config.capabilities()
    }

    pub fn runtime(&self) -> &Arc<BackendRuntime> {
        &self.runtime
    }

    /// A producer handle for a backend the host constructs itself.
    pub fn sender(&self) -> BackendSender {
        self.sender.clone()
    }

    pub fn events(&self) -> &Receiver<BackendEvent> {
        &self.events
    }

    /// Everything the backends have finished since the last pump.
    pub fn drain_events(&self) -> Vec<BackendEvent> {
        self.events.try_iter().collect()
    }

    /// Where recorded and synthesized audio lives (D28); `None` when the host
    /// asked for no audio.
    pub fn runtime_dir(&self) -> Option<&Path> {
        self.session_dir.as_ref().map(SessionDir::path)
    }

    /// The provider-backed cognition for this configuration. A misconfigured
    /// provider still yields a client — it fails per request, on the channel,
    /// exactly as the Python one failed per `complete()` call.
    pub fn cognition(&self) -> HttpCognition {
        HttpCognition::new(
            Arc::clone(&self.runtime),
            self.config.llm.clone(),
            self.sender(),
        )
    }

    /// The offline speech backends (`fake_backend: true`).
    pub fn fake_speech(&self) -> FakeSpeech {
        FakeSpeech::new(self.sender())
    }

    /// The player's ear: cloud batch, the local Canary worker, and the realtime
    /// stream — or the offline fake. Fake mode never opens a socket
    /// (`server.py:579-581`).
    pub fn transcription(&self) -> Box<dyn Transcription + Send> {
        if self.config.fake_mode {
            return Box::new(self.fake_speech());
        }
        Box::new(SttEngine::new(
            Arc::clone(&self.runtime),
            &self.config.speech,
            self.runtime_dir().map(Path::to_path_buf),
            self.sender(),
        ))
    }

    /// The cast's voice: streaming cloud WAVs and the local streaming Pocket
    /// worker — or silence-shaped fakes.
    pub fn tts(&self) -> Box<dyn Tts + Send> {
        if self.config.fake_mode {
            return Box::new(self.fake_speech());
        }
        Box::new(TtsEngine::new(
            Arc::clone(&self.runtime),
            &self.config.speech,
            self.sender(),
        ))
    }

    /// The startup voice backend, after the availability fallback, plus the
    /// message the HUD shows when it had to be forced off (`server.py:500-512`).
    pub fn tts_selection(&self) -> (TtsBackendKind, Option<String>) {
        if self.config.fake_mode {
            // The fakes can speak either way; the configured choice stands.
            return select_tts_backend(&self.config.speech.tts_backend, BackendCapabilities::all());
        }
        self.config.tts_selection()
    }

    /// How long a committed recording waits for its realtime transcript before
    /// falling back to batch — [`cathedral_sim::EngineConfig`] wants it.
    pub fn stream_grace_seconds(&self) -> f64 {
        self.config.speech.stream_grace_seconds
    }

    /// The prompt archive for this session's `prompts/` directory.
    ///
    /// `model` follows `server.py:534-543`: `"fake"` in fake mode, otherwise the
    /// provider's model name (`None` when misconfigured).
    pub fn prompt_log(&self, prompts_dir: Option<std::path::PathBuf>) -> PromptLog {
        let model = if self.config.fake_mode {
            Some("fake".to_string())
        } else {
            self.config.model_name()
        };
        PromptLog::new(prompts_dir, model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cathedral_sim::{Cognition, TranscriptionJobId};
    use std::collections::BTreeMap;

    fn config(fake_mode: bool) -> BackendsConfig {
        let options = BackendsOptions {
            dotenv_path: None,
            workers_dir: std::path::PathBuf::from("/nonexistent"),
            uv_binary: "uv".to_string(),
            fake_mode,
        };
        BackendsConfig::resolve(&Environment::from_map(BTreeMap::new()), &options)
    }

    #[test]
    fn a_fake_handle_reports_every_capability_and_speaks_offline() {
        let handle = BackendsHandle::start(config(true), None).expect("started");
        assert_eq!(handle.capabilities(), BackendCapabilities::all());
        assert!(handle.runtime_dir().is_none());

        let mut speech = handle.fake_speech();
        cathedral_sim::Transcription::submit_batch(
            &mut speech,
            TranscriptionJobId(1),
            std::path::PathBuf::from("x.wav"),
            cathedral_sim::SttBackendKind::Cloud,
        )
        .expect("accepted");

        let events = handle.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            BackendEvent::TranscriptionDone { result: Ok(text), .. } if text == DEFAULT_FAKE_TRANSCRIPT
        ));
    }

    #[test]
    fn the_prompt_log_model_is_fake_in_fake_mode_and_the_real_model_otherwise() {
        let handle = BackendsHandle::start(config(true), None).expect("started");
        let log = handle.prompt_log(None);
        assert!(!log.enabled(), "no directory, no archive");

        let mut vars = BTreeMap::new();
        vars.insert("MOONSHOT_API_KEY".to_string(), "sk-m".to_string());
        let configured = BackendsConfig::resolve(
            &Environment::from_map(vars),
            &BackendsOptions {
                dotenv_path: None,
                workers_dir: std::path::PathBuf::from("/nonexistent"),
                uv_binary: "uv".to_string(),
                fake_mode: false,
            },
        );
        assert_eq!(configured.model_name().as_deref(), Some("kimi-k2.5"));
        let handle = BackendsHandle::start(configured, None).expect("started");
        assert!(handle.capabilities().llm);
        assert_eq!(handle.cognition().model_name(), Some("kimi-k2.5"));
    }

    #[test]
    fn an_unconfigured_provider_still_yields_a_cognition_that_fails_per_request() {
        let handle = BackendsHandle::start(config(false), None).expect("started");
        assert!(!handle.capabilities().llm);

        let mut cognition = handle.cognition();
        assert_eq!(cognition.model_name(), None);
        cognition
            .request("the prompt".to_string())
            .expect("accepted anyway");

        let events = handle.drain_events();
        assert!(matches!(
            &events[..],
            [BackendEvent::LlmCompletion(completion)] if completion.result.is_err()
        ));
    }
}
