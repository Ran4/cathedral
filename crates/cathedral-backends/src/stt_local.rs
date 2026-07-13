//! Local transcription — the Canary-Qwen worker
//! (`CanaryQwenSpeechBackend`, `speech_client.py:686-898`).
//!
//! The worker stays Python: NeMo's GPU dependency tree lives in its own PEP-723
//! script so that *choosing cloud transcription never installs it*. Rust only
//! starts it, hands it absolute WAV paths, and reads back transcripts.
//!
//! The launch line is test-pinned, including the two things that look optional
//! and are not:
//!
//! * `--resolution highest` — the script's pins are ceilings, not floors;
//! * `--index pytorch=<url>` — a `LOCAL_STT_TORCH_INDEX` override *replaces* the
//!   named CUDA index the script's metadata pins, and must therefore sit
//!   immediately before `--script`.

use std::path::Path;

use cathedral_sim::{SpeechError, Subsystem};
use serde_json::{Map, Value};

use crate::{
    config::SpeechSettings,
    events::BackendSender,
    wav::require_existing_wav,
    worker::{Worker, WorkerMessages, WorkerSpec, WorkerStep},
};

/// `speech_client.py:735-776`, worded exactly as the player sees them.
const MESSAGES: WorkerMessages = WorkerMessages {
    start_failed: "could not start local Canary-Qwen; make sure uv is available",
    unavailable: "local Canary-Qwen worker script is unavailable",
    exited: "local Canary-Qwen worker exited; check the smart-actor log",
    invalid_json: "local Canary-Qwen returned an invalid response",
    invalid_response: "local Canary-Qwen returned an invalid response",
    write_failed: "local Canary-Qwen worker stopped; press Z to use cloud transcription",
    load_failed: "local Canary-Qwen failed to load; check CUDA and available VRAM",
};

const FAILED: &str = "local Canary-Qwen transcription failed";

/// The Canary-Qwen driver.
#[derive(Debug)]
pub struct CanaryTranscriber {
    worker: Worker,
}

impl CanaryTranscriber {
    pub fn new(settings: &SpeechSettings, events: BackendSender) -> Self {
        Self {
            worker: Worker::new(spec(settings), events),
        }
    }

    /// The script exists and uv is configured. **No CUDA probe**: a machine
    /// without a GPU still reports local STT available and fails at first use,
    /// exactly as Python does (`speech_client.py:731-733`).
    pub fn available(&self) -> bool {
        self.worker.available()
    }

    /// Transcribe one recording. Blocking: this runs on the STT engine's worker
    /// thread, which is the whole point of that thread.
    pub fn transcribe(&self, wav_path: &Path) -> Result<String, SpeechError> {
        require_existing_wav(wav_path)?;
        // The worker validates the path too, and it has a different cwd.
        let absolute = std::fs::canonicalize(wav_path)
            .unwrap_or_else(|_| wav_path.to_path_buf())
            .display()
            .to_string();

        self.worker
            .publish_status("transcribing", "Transcribing with local Canary-Qwen FP16");

        let mut body = Map::new();
        body.insert("wav_path".to_string(), Value::from(absolute));

        let mut transcript = None;
        self.worker.request(body, |message| {
            let text = message.get("text").and_then(Value::as_str);
            match (message.get("type").and_then(Value::as_str), text) {
                (Some("result"), Some(text)) => {
                    transcript = Some(text.to_string());
                    WorkerStep::Done
                }
                _ => WorkerStep::Fail {
                    // The worker's own wording ("press Z to use cloud
                    // transcription") is the actionable one; keep it.
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
        transcript.ok_or_else(|| SpeechError::new(FAILED))
    }

    pub fn close(&self) {
        self.worker.close();
    }

    #[cfg(test)]
    fn spawn_count(&self) -> u64 {
        self.worker.spawn_count()
    }
}

/// `speech_client.py:797-817` — the launch line, argument for argument.
fn spec(settings: &SpeechSettings) -> WorkerSpec {
    let script = settings.canary_script();
    let mut args = vec![
        "run".to_string(),
        "--python".to_string(),
        settings.local_stt_python.clone(),
        "--resolution".to_string(),
        "highest".to_string(),
    ];
    if let Some(index) = settings
        .local_stt_torch_index
        .as_deref()
        .filter(|index| !index.trim().is_empty())
    {
        args.push("--index".to_string());
        args.push(format!("pytorch={index}"));
    }
    args.push("--script".to_string());
    args.push(script.display().to_string());

    // `.env` first (a child inherits the process environment, and `.env` is not
    // in it — see `Environment::worker_env`; `HF_TOKEN` is what the model
    // download needs), then the model id, which is ours to decide and therefore
    // wins over anything `.env` says about it.
    let mut env = settings.worker_env.clone();
    env.retain(|(key, _)| key != "LOCAL_STT_MODEL");
    // The worker reads the model id from its environment, not its argv.
    env.push((
        "LOCAL_STT_MODEL".to_string(),
        settings.local_stt_model.clone(),
    ));

    WorkerSpec {
        program: settings.uv_binary.clone(),
        args,
        env,
        script,
        log_source: "stt",
        messages: MESSAGES,
        subsystem: Subsystem::Stt,
        backend: "local",
        loading_message: "Preparing local dependencies and Canary-Qwen FP16",
        ready_message: "Local Canary-Qwen FP16 is loaded",
        // A first run downloads about 5 GB; uv's chatter is the only progress
        // bar the player gets.
        install_progress_statuses: true,
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
    use std::{collections::BTreeMap, path::PathBuf};

    fn settings(stub: &StubWorker, pairs: &[(&str, &str)]) -> SpeechSettings {
        let vars: BTreeMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        let mut speech = BackendsConfig::resolve(
            &Environment::from_map(vars),
            &BackendsOptions {
                dotenv_path: None,
                workers_dir: stub.directory.clone(),
                uv_binary: stub.program.display().to_string(),
                fake_mode: false,
            },
        )
        .speech;
        // The stub stands in for `canary_qwen_worker.py`.
        std::fs::rename(&stub.script, speech.canary_script()).expect("worker script");
        speech.workers_dir = stub.directory.clone();
        speech
    }

    fn recording(directory: &Path) -> PathBuf {
        let path = directory.join("player-recording-1.wav");
        std::fs::write(&path, b"RIFF....WAVE").expect("a recording");
        path
    }

    /// speech-python.md test 4: lazy, reused, ids from 1, and the documented uv
    /// command line.
    #[test]
    fn the_worker_is_lazy_reused_and_launched_the_documented_way() {
        let stub = StubWorker::new(
            "canary",
            &[
                r#"{"type":"ready","model":"nvidia/canary-qwen-2.5b","precision":"fp16"}"#,
                r#"{"type":"result","request_id":1,"text":"first local"}"#,
                r#"{"type":"result","request_id":2,"text":"second local"}"#,
            ],
        );
        let settings = settings(&stub, &[]);
        let (sender, _events) = backend_channel();
        let canary = CanaryTranscriber::new(&settings, sender);
        assert!(canary.available());

        let wav = recording(&stub.directory);
        assert_eq!(
            canary.transcribe(&wav).expect("a transcript"),
            "first local"
        );
        assert_eq!(
            canary.transcribe(&wav).expect("a transcript"),
            "second local"
        );
        assert_eq!(canary.spawn_count(), 1, "one worker, two utterances");

        assert_eq!(
            stub.argv(),
            vec![
                "run".to_string(),
                "--python".to_string(),
                "3.12".to_string(),
                "--resolution".to_string(),
                "highest".to_string(),
                "--script".to_string(),
                settings.canary_script().display().to_string(),
            ]
        );
        let requests = stub.requests();
        assert_eq!(
            requests
                .iter()
                .map(|request| request["request_id"].as_u64().expect("an id"))
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            requests[0]["wav_path"],
            Value::from(
                std::fs::canonicalize(&wav)
                    .expect("canonical")
                    .display()
                    .to_string()
            ),
            "the worker has a different cwd: the path must be absolute"
        );
        canary.close();
    }

    /// speech-python.md test 5: an override replaces the script's own named CUDA
    /// index, immediately before `--script`.
    #[test]
    fn a_torch_index_override_lands_right_before_the_script() {
        let stub = StubWorker::new(
            "torch-index",
            &[
                r#"{"type":"ready"}"#,
                r#"{"type":"result","request_id":1,"text":"cpu wheels"}"#,
            ],
        );
        let settings = settings(
            &stub,
            &[(
                "LOCAL_STT_TORCH_INDEX",
                "https://download.pytorch.org/whl/cpu",
            )],
        );
        let (sender, _events) = backend_channel();
        let canary = CanaryTranscriber::new(&settings, sender);
        canary
            .transcribe(&recording(&stub.directory))
            .expect("a transcript");

        let argv = stub.argv();
        assert_eq!(
            &argv[5..8],
            &[
                "--index".to_string(),
                "pytorch=https://download.pytorch.org/whl/cpu".to_string(),
                "--script".to_string(),
            ]
        );
        canary.close();
    }

    /// The model id reaches the child in its environment, where the worker reads
    /// it (`speech_client.py:816`).
    #[test]
    fn the_model_id_is_passed_in_the_child_environment() {
        let stub = StubWorker::new("model-env", &[r#"{"type":"ready"}"#]);
        let settings = settings(&stub, &[("LOCAL_STT_MODEL", "nvidia/canary-qwen-1b")]);
        let (sender, _events) = backend_channel();
        let canary = CanaryTranscriber::new(&settings, sender);
        assert_eq!(
            spec(&settings).env,
            vec![(
                "LOCAL_STT_MODEL".to_string(),
                "nvidia/canary-qwen-1b".to_string()
            )]
        );
        canary.close();
    }

    /// A GPU that runs out of VRAM answers with an `error` line and *stays
    /// alive*: the next utterance may still work (or the player presses Z).
    #[test]
    fn a_failed_transcription_keeps_the_worker_and_surfaces_its_message() {
        let stub = StubWorker::new(
            "gpu-failure",
            &[
                r#"{"type":"ready"}"#,
                r#"{"type":"error","request_id":1,"error":"local Canary-Qwen transcription failed; press Z to use cloud transcription"}"#,
                r#"{"type":"result","request_id":2,"text":"recovered"}"#,
            ],
        );
        let settings = settings(&stub, &[]);
        let (sender, _events) = backend_channel();
        let canary = CanaryTranscriber::new(&settings, sender);
        let wav = recording(&stub.directory);

        assert_eq!(
            canary
                .transcribe(&wav)
                .expect_err("the GPU said no")
                .presentable,
            "local Canary-Qwen transcription failed; press Z to use cloud transcription"
        );
        assert_eq!(
            canary.transcribe(&wav).expect("the worker survived"),
            "recovered"
        );
        assert_eq!(canary.spawn_count(), 1);
        canary.close();
    }

    #[test]
    fn a_missing_recording_never_reaches_the_worker() {
        let stub = StubWorker::new("missing", &[r#"{"type":"ready"}"#]);
        let settings = settings(&stub, &[]);
        let (sender, _events) = backend_channel();
        let canary = CanaryTranscriber::new(&settings, sender);
        assert!(canary.transcribe(&stub.directory.join("gone.wav")).is_err());
        assert_eq!(canary.spawn_count(), 0, "not even a spawn");
        canary.close();
    }

    /// The Canary worker downloads ~5 GB from Hugging Face on its first run, and
    /// `HF_TOKEN` — documented in the checked-in `.example.env` — is what keeps
    /// that from being an anonymous, rate-limited fetch. A `.env` key reaches a
    /// Rust child only if we hand it over.
    #[test]
    fn the_dotenv_reaches_the_child_without_displacing_the_model_we_chose() {
        let mut vars = BTreeMap::new();
        vars.insert("HF_TOKEN".to_string(), "hf_secret".to_string());
        vars.insert("LOCAL_STT_MODEL".to_string(), "nvidia/other".to_string());
        let speech = BackendsConfig::resolve(
            &Environment::from_dotenv_map(vars),
            &BackendsOptions {
                dotenv_path: None,
                workers_dir: PathBuf::from("/nonexistent"),
                uv_binary: "uv".to_string(),
                fake_mode: false,
            },
        )
        .speech;

        let spawned = spec(&speech);
        assert!(
            spawned
                .env
                .contains(&("HF_TOKEN".to_string(), "hf_secret".to_string())),
            "{:?}",
            spawned.env
        );
        let models: Vec<&String> = spawned
            .env
            .iter()
            .filter(|(key, _)| key == "LOCAL_STT_MODEL")
            .map(|(_, value)| value)
            .collect();
        assert_eq!(
            models,
            vec!["nvidia/other"],
            "exactly one model id, and it is the resolved one",
        );
    }
}
