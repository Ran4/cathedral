//! Backend configuration and capability probing (`llm_client.py:7-15, 48-123`,
//! `speech_client.py`, `server.py:830-861`).
//!
//! Two rules carried over from Python and easy to break:
//!
//! * **Real environment variables win over `prompt_playgound/.env`**
//!   (python-dotenv's no-override default; llm-headless.md §1.1). The `.env`
//!   keeps being read from that directory so the ML workers and the Rust engine
//!   never disagree about where the keys live (risk 9).
//! * **Capabilities are probed without touching the network** (llm-headless.md
//!   §1.8, risk 13): a key that exists but is wrong still reports `llm: true`
//!   and degrades at the first completion, exactly as today.

use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

use cathedral_sim::TtsBackendKind;

/// `prompt_playgound/.env`, relative to the repo root (the game's cwd).
pub const DEFAULT_DOTENV_PATH: &str = "prompt_playgound/.env";
/// Where the uv worker scripts live until P7 relocates them.
pub const DEFAULT_WORKERS_DIR: &str = "prompt_playgound";
/// `speech_client.py:706` — local STT worker.
pub const CANARY_WORKER_SCRIPT: &str = "canary_qwen_worker.py";
/// `speech_client.py:253` — local streaming TTS worker.
pub const POCKET_WORKER_SCRIPT: &str = "pocket_tts_worker.py";

/// `llm_client.py:80` — per-attempt provider timeout.
pub const DEFAULT_LLM_TIMEOUT_SECONDS: f64 = 45.0;
/// `server.py:515-517` — the scheduler's inter-turn delay.
pub const DEFAULT_NPC_TURN_DELAY_SECONDS: f64 = 1.0;

// ------------------------------------------------------------- speech defaults

/// `speech_client.py:33-49`, verbatim.
pub const DEFAULT_STT_MODEL: &str = "gpt-4o-transcribe";
pub const DEFAULT_LOCAL_STT_MODEL: &str = "nvidia/canary-qwen-2.5b";
pub const DEFAULT_REALTIME_STT_MODEL: &str = "gpt-realtime-whisper";
pub const DEFAULT_REALTIME_URL: &str = "wss://api.openai.com/v1/realtime?intent=transcription";
pub const DEFAULT_TTS_MODEL: &str = "tts-1";
/// The OpenAI SDK's default base URL, spelled out (it is a field so the tests
/// can point the client at a loopback mock).
pub const DEFAULT_SPEECH_BASE_URL: &str = "https://api.openai.com/v1";
/// `speech_client.py:162` — the whole-request budget for a cloud speech call.
pub const DEFAULT_SPEECH_TIMEOUT_SECONDS: f64 = 30.0;
/// `speech_client.py:993-997`.
pub const DEFAULT_REALTIME_IDLE_CLOSE_SECONDS: f64 = 300.0;
/// `speech_client.py:982` — unresolved commits the provider may owe us.
pub const DEFAULT_REALTIME_MAX_IN_FLIGHT: usize = 4;
/// `server.py:570-576` — how long a committed recording waits for its realtime
/// transcript before falling back to batch.
pub const DEFAULT_STREAM_GRACE_SECONDS: f64 = 2.0;
/// The same line's minimum: a grace shorter than this is a fallback storm.
pub const MIN_STREAM_GRACE_SECONDS: f64 = 0.2;
/// The three logical NPC voices (`speech_client.py:38-43`). Order is the
/// prompt's cast order.
pub const LOGICAL_NPC_VOICES: [&str; 3] = ["sven", "conny", "ilse"];
/// `DEFAULT_OPENAI_VOICES` (`speech_client.py:38-42`).
pub const DEFAULT_OPENAI_VOICES: [(&str, &str); 3] =
    [("sven", "onyx"), ("conny", "echo"), ("ilse", "nova")];
/// Python pins both workers to 3.12 by default (`speech_client.py:258, 711`).
pub const DEFAULT_WORKER_PYTHON: &str = "3.12";

// --------------------------------------------------------------- provider table

/// The two providers of `llm_client.py:17-31`. Values are wire-exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    Moonshot,
    Openai,
}

/// One row of Python's `PROVIDERS` dict.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProviderSpec {
    pub name: &'static str,
    /// The OpenAI SDK's default (`base_url: None`) spelled out.
    pub base_url: &'static str,
    pub key_env: &'static str,
    pub default_model: &'static str,
    /// Moonshot's instant mode: `temperature: 0.6`.
    pub temperature: Option<f64>,
    /// Moonshot's instant mode: top-level `"thinking": {"type": "disabled"}`
    /// (the SDK's `extra_body` merges into the body root — llm-headless.md §1.2).
    pub thinking_disabled: bool,
}

pub const MOONSHOT: ProviderSpec = ProviderSpec {
    name: "moonshot",
    base_url: "https://api.moonshot.ai/v1",
    key_env: "MOONSHOT_API_KEY",
    default_model: "kimi-k2.5",
    temperature: Some(0.6),
    thinking_disabled: true,
};

pub const OPENAI: ProviderSpec = ProviderSpec {
    name: "openai",
    base_url: "https://api.openai.com/v1",
    key_env: "OPENAI_API_KEY",
    default_model: "gpt-5.6-luna",
    temperature: None,
    thinking_disabled: false,
};

/// Declaration order is the order Python's error message lists them in.
pub const PROVIDERS: [ProviderSpec; 2] = [MOONSHOT, OPENAI];

impl Provider {
    pub fn spec(self) -> ProviderSpec {
        match self {
            Self::Moonshot => MOONSHOT,
            Self::Openai => OPENAI,
        }
    }

    /// `LLM_PROVIDER` lookup after `.strip().lower()` (`llm_client.py:49`).
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "moonshot" => Some(Self::Moonshot),
            "openai" => Some(Self::Openai),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        self.spec().name
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// `unknown LLM_PROVIDER 'x' (expected one of: moonshot, openai)`
/// (`llm_client.py:51-54`), reproduced verbatim.
pub fn unknown_provider_message(value: &str) -> String {
    let names: Vec<&str> = PROVIDERS.iter().map(|spec| spec.name).collect();
    format!(
        "unknown LLM_PROVIDER '{value}' (expected one of: {})",
        names.join(", ")
    )
}

// ---------------------------------------------------------------- environment

/// The resolved variable set: process environment first, `.env` only for keys
/// the process environment does not define.
#[derive(Debug, Clone, Default)]
pub struct Environment {
    vars: BTreeMap<String, String>,
    /// The entries `.env` contributed that the *process* environment does not
    /// have. Python's `load_dotenv` wrote these into `os.environ`, so the uv
    /// workers — spawned with `os.environ.copy()` — inherited them and read
    /// `TTS_POCKET_VOICE_*` and `HF_TOKEN` straight out of their own
    /// environment. A Rust child inherits the real process environment, which
    /// has never heard of `.env`, so these have to be handed over explicitly:
    /// see [`Environment::worker_env`].
    dotenv_only: BTreeMap<String, String>,
    /// `~`, for the `~/.config/moonshot/key` fallback. `None` disables it.
    home: Option<PathBuf>,
}

impl Environment {
    /// Process env + `.env` (process wins). A missing/unreadable `.env` is not
    /// an error — most runs have their keys in the real environment.
    pub fn from_process(dotenv_path: Option<&Path>) -> Self {
        let mut vars: BTreeMap<String, String> = std::env::vars().collect();
        let mut dotenv_only = BTreeMap::new();
        if let Some(path) = dotenv_path
            && let Ok(entries) = dotenvy::from_path_iter(path)
        {
            for (key, value) in entries.flatten() {
                if let std::collections::btree_map::Entry::Vacant(slot) = vars.entry(key.clone()) {
                    slot.insert(value.clone());
                    dotenv_only.insert(key, value);
                }
            }
        }
        Self {
            vars,
            dotenv_only,
            home: home_dir(),
        }
    }

    /// A deterministic environment for tests.
    pub fn from_map(vars: BTreeMap<String, String>) -> Self {
        Self {
            vars,
            dotenv_only: BTreeMap::new(),
            home: None,
        }
    }

    /// A test environment whose variables came from `.env` rather than from the
    /// process — the distinction the worker children can feel.
    pub fn from_dotenv_map(vars: BTreeMap<String, String>) -> Self {
        Self {
            dotenv_only: vars.clone(),
            vars,
            home: None,
        }
    }

    /// What a spawned worker must be told, because it cannot inherit it: every
    /// `.env` entry the process environment does not already carry.
    ///
    /// Deliberately the *whole* `.env`, not a known-keys allowlist: the worker
    /// scripts read variables this crate has never heard of
    /// (`pocket_tts_worker.py` reads `TTS_POCKET_VOICE_<NAME>`, and both workers
    /// let `huggingface_hub` read `HF_TOKEN`), and Python passed them all.
    pub fn worker_env(&self) -> Vec<(String, String)> {
        self.dotenv_only
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    pub fn with_home(mut self, home: Option<PathBuf>) -> Self {
        self.home = home;
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(String::as_str)
    }

    /// Put a value *above* the process environment.
    ///
    /// The one thing that outranks a real env var: an explicit command-line
    /// flag (`cathedral-headless --provider/--model`). Everything else composes
    /// the other way round — `.env` only fills gaps.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(key.into(), value.into());
    }

    /// `os.environ.get(key, "").strip()` with empty treated as unset — the
    /// idiom behind `LLM_MODEL`'s "empty string counts as unset".
    pub fn trimmed(&self, key: &str) -> Option<&str> {
        self.get(key)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn float(&self, key: &str, default: f64) -> f64 {
        self.trimmed(key)
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite())
            .unwrap_or(default)
    }

    pub fn bool(&self, key: &str, default: bool) -> bool {
        match self.trimmed(key).map(str::to_lowercase).as_deref() {
            Some("1" | "true" | "yes" | "on") => true,
            Some("0" | "false" | "no" | "off") => false,
            _ => default,
        }
    }

    /// The legacy `~/.config/moonshot/key` file (`llm_client.py:67-71`).
    pub fn moonshot_key_file(&self) -> Option<PathBuf> {
        Some(self.home.as_ref()?.join(".config/moonshot/key"))
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

// ------------------------------------------------------------------- settings

/// Everything the LLM client needs, frozen at construction.
///
/// Python re-reads `LLM_PROVIDER`/`LLM_MODEL` per call but freezes the key,
/// base URL and timeout at first use; nothing mutates those mid-run, so we
/// freeze the lot (llm-headless.md risk 7).
#[derive(Debug, Clone, PartialEq)]
pub struct LlmSettings {
    pub provider: Provider,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub timeout_seconds: f64,
    /// Send `content` as a multimodal parts array (the day-one default and the
    /// end goal: attaching NPC-eye screenshots later). Flip to `false` — env
    /// `LLM_CONTENT_PARTS=0` — to fall back to a plain string if a provider
    /// rejects the array (llm-headless.md §1.10, risk 1).
    pub content_parts: bool,
}

impl LlmSettings {
    /// `POST {base_url}/chat/completions`.
    pub fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

/// Non-network availability, as reported at the handshake (`server.py:837-846`).
///
/// Each subsystem degrades independently: a missing `MOONSHOT_API_KEY` never
/// takes speech down, and a missing `OPENAI_API_KEY` never takes cognition down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BackendCapabilities {
    pub llm: bool,
    pub stt_cloud: bool,
    pub stt_local: bool,
    pub tts_cloud: bool,
    pub tts_local: bool,
}

impl BackendCapabilities {
    /// Fake mode declares everything available (`server.py:355-363`).
    pub fn all() -> Self {
        Self {
            llm: true,
            stt_cloud: true,
            stt_local: true,
            tts_cloud: true,
            tts_local: true,
        }
    }

    pub fn stt(self) -> bool {
        self.stt_cloud || self.stt_local
    }

    pub fn tts(self) -> bool {
        self.tts_cloud || self.tts_local
    }
}

// --------------------------------------------------------------- speech config

/// The realtime transcription websocket (`speech_client.py:986-1041`).
///
/// Every field is env-overridable because the API shape is documented as
/// volatile (R16): if OpenAI moves the model or the frame, a run can be fixed
/// without a rebuild, and a wrong guess degrades to batch instead of breaking.
#[derive(Debug, Clone, PartialEq)]
pub struct RealtimeSettings {
    pub url: String,
    pub model: String,
    pub delay: String,
    pub language: Option<String>,
    pub idle_close_seconds: f64,
    pub max_in_flight: usize,
}

impl Default for RealtimeSettings {
    fn default() -> Self {
        Self {
            url: DEFAULT_REALTIME_URL.to_string(),
            model: DEFAULT_REALTIME_STT_MODEL.to_string(),
            delay: "low".to_string(),
            language: None,
            idle_close_seconds: DEFAULT_REALTIME_IDLE_CLOSE_SECONDS,
            max_in_flight: DEFAULT_REALTIME_MAX_IN_FLIGHT,
        }
    }
}

/// Everything the speech backends read from the environment
/// (`speech-python.md` §6), resolved once at startup.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechSettings {
    /// Cloud STT/TTS/realtime all hang off this one key; without it they are
    /// unavailable and the local workers carry on regardless.
    pub api_key: Option<String>,
    pub base_url: String,
    pub stt_model: String,
    pub tts_model: String,
    pub timeout_seconds: f64,
    pub realtime: RealtimeSettings,
    /// Raw `TTS_OPENAI_VOICE_*` / legacy `TTS_VOICE_*` overrides, resolved and
    /// validated per request (`_resolve_voice`, `speech_client.py:110-134`).
    pub voice_overrides: BTreeMap<String, String>,
    /// uv worker launch knobs.
    pub uv_binary: String,
    pub workers_dir: PathBuf,
    pub local_stt_python: String,
    pub local_stt_model: String,
    pub local_stt_torch_index: Option<String>,
    pub local_tts_python: String,
    /// `STT_STREAM_COMPLETION_GRACE_MS`, in seconds and clamped.
    pub stream_grace_seconds: f64,
    /// `SMART_ACTORS_TTS_BACKEND`, before the availability fallback.
    pub tts_backend: String,
    /// Handed to every uv worker on spawn: the `.env` entries that a child could
    /// not inherit (`Environment::worker_env`). Python's workers read
    /// `TTS_POCKET_VOICE_*` and `HF_TOKEN` out of the environment themselves.
    pub worker_env: Vec<(String, String)>,
}

impl SpeechSettings {
    fn resolve(environment: &Environment, options: &BackendsOptions) -> Self {
        let mut voice_overrides = BTreeMap::new();
        for key in LOGICAL_NPC_VOICES {
            let upper = key.to_uppercase();
            for variable in [
                format!("TTS_OPENAI_VOICE_{upper}"),
                format!("TTS_VOICE_{upper}"),
            ] {
                if let Some(value) = environment.trimmed(&variable) {
                    voice_overrides.insert(variable, value.to_string());
                }
            }
        }

        let grace_milliseconds = environment.float(
            "STT_STREAM_COMPLETION_GRACE_MS",
            DEFAULT_STREAM_GRACE_SECONDS * 1_000.0,
        );

        Self {
            api_key: environment.trimmed(OPENAI.key_env).map(str::to_string),
            base_url: environment
                .trimmed("OPENAI_BASE_URL")
                .unwrap_or(DEFAULT_SPEECH_BASE_URL)
                .to_string(),
            stt_model: environment
                .trimmed("STT_MODEL")
                .unwrap_or(DEFAULT_STT_MODEL)
                .to_string(),
            tts_model: environment
                .trimmed("TTS_MODEL")
                .unwrap_or(DEFAULT_TTS_MODEL)
                .to_string(),
            timeout_seconds: environment
                .float("SPEECH_TIMEOUT_SECONDS", DEFAULT_SPEECH_TIMEOUT_SECONDS),
            realtime: RealtimeSettings {
                url: environment
                    .trimmed("STT_REALTIME_URL")
                    .unwrap_or(DEFAULT_REALTIME_URL)
                    .to_string(),
                model: environment
                    .trimmed("STT_REALTIME_MODEL")
                    .unwrap_or(DEFAULT_REALTIME_STT_MODEL)
                    .to_string(),
                delay: environment
                    .trimmed("STT_REALTIME_DELAY")
                    .unwrap_or("low")
                    .to_string(),
                language: environment.trimmed("STT_LANGUAGE").map(str::to_string),
                idle_close_seconds: environment.float(
                    "STT_STREAM_IDLE_CLOSE_S",
                    DEFAULT_REALTIME_IDLE_CLOSE_SECONDS,
                ),
                max_in_flight: environment
                    .trimmed("STT_REALTIME_MAX_IN_FLIGHT")
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|count| *count > 0)
                    .unwrap_or(DEFAULT_REALTIME_MAX_IN_FLIGHT),
            },
            voice_overrides,
            uv_binary: options.uv_binary.clone(),
            workers_dir: options.workers_dir.clone(),
            local_stt_python: environment
                .trimmed("LOCAL_STT_PYTHON")
                .unwrap_or(DEFAULT_WORKER_PYTHON)
                .to_string(),
            local_stt_model: environment
                .trimmed("LOCAL_STT_MODEL")
                .unwrap_or(DEFAULT_LOCAL_STT_MODEL)
                .to_string(),
            local_stt_torch_index: environment
                .trimmed("LOCAL_STT_TORCH_INDEX")
                .map(str::to_string),
            local_tts_python: environment
                .trimmed("LOCAL_TTS_PYTHON")
                .unwrap_or(DEFAULT_WORKER_PYTHON)
                .to_string(),
            stream_grace_seconds: (grace_milliseconds / 1_000.0).max(MIN_STREAM_GRACE_SECONDS),
            tts_backend: environment
                .trimmed("SMART_ACTORS_TTS_BACKEND")
                .unwrap_or("local")
                .to_lowercase(),
            worker_env: environment.worker_env(),
        }
    }

    pub fn canary_script(&self) -> PathBuf {
        self.workers_dir.join(CANARY_WORKER_SCRIPT)
    }

    pub fn pocket_script(&self) -> PathBuf {
        self.workers_dir.join(POCKET_WORKER_SCRIPT)
    }
}

/// The startup TTS selection (`server.py:500-512, 831-861`): the configured
/// backend, or `off` with the exact sentence the HUD shows for why.
pub fn select_tts_backend(
    configured: &str,
    capabilities: BackendCapabilities,
) -> (TtsBackendKind, Option<String>) {
    let selected = match configured.trim().to_lowercase().as_str() {
        "cloud" => TtsBackendKind::Cloud,
        "local" => TtsBackendKind::Local,
        "off" => return (TtsBackendKind::Off, None),
        _ => {
            return (
                TtsBackendKind::Off,
                Some("Configured NPC voice mode is invalid; voices are off".to_string()),
            );
        }
    };
    let available = match selected {
        TtsBackendKind::Cloud => capabilities.tts_cloud,
        TtsBackendKind::Local => capabilities.tts_local,
        TtsBackendKind::Off => true,
    };
    if available {
        (selected, None)
    } else {
        (
            TtsBackendKind::Off,
            Some(format!(
                "Configured {} NPC voice backend is unavailable; voices are off",
                selected.as_str()
            )),
        )
    }
}

/// How to find the things a probe needs on disk.
#[derive(Debug, Clone)]
pub struct BackendsOptions {
    /// `.env` to merge under the process environment; `None` skips it.
    pub dotenv_path: Option<PathBuf>,
    /// Directory holding `canary_qwen_worker.py` / `pocket_tts_worker.py`.
    pub workers_dir: PathBuf,
    /// `uv`, or a configured absolute path (`config.ron: uv_binary`).
    pub uv_binary: String,
    /// Rust fakes instead of providers (`config.ron: fake_backend`).
    pub fake_mode: bool,
}

impl Default for BackendsOptions {
    fn default() -> Self {
        Self {
            dotenv_path: Some(PathBuf::from(DEFAULT_DOTENV_PATH)),
            workers_dir: PathBuf::from(DEFAULT_WORKERS_DIR),
            uv_binary: "uv".to_string(),
            fake_mode: false,
        }
    }
}

/// The resolved backend configuration.
#[derive(Debug, Clone)]
pub struct BackendsConfig {
    /// `Err` only for an unknown `LLM_PROVIDER` or a missing key — both are
    /// *lazy* failures in Python (raised inside `complete()`, not at startup),
    /// so the engine still boots and the HUD shows cognition as unavailable.
    pub llm: Result<LlmSettings, LlmConfigError>,
    pub openai_api_key: Option<String>,
    pub local_stt_model: String,
    pub npc_turn_delay_seconds: f64,
    pub workers_dir: PathBuf,
    pub uv_binary: String,
    pub fake_mode: bool,
    /// Everything the STT/TTS backends need (P6).
    pub speech: SpeechSettings,
    capabilities: BackendCapabilities,
}

/// A configuration problem, worded exactly as `LLMConfigurationError`
/// (`llm_client.py:44-45, 51-54, 73-76`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmConfigError(pub String);

impl fmt::Display for LlmConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for LlmConfigError {}

impl BackendsConfig {
    /// Read the process environment (plus `.env`) and probe the disk.
    pub fn load(options: &BackendsOptions) -> Self {
        let environment = Environment::from_process(options.dotenv_path.as_deref());
        Self::resolve(&environment, options)
    }

    /// The testable core: no process environment, no globals.
    pub fn resolve(environment: &Environment, options: &BackendsOptions) -> Self {
        let llm = resolve_llm(environment, options);
        let openai_api_key = environment.trimmed(OPENAI.key_env).map(str::to_string);

        let capabilities = if options.fake_mode {
            BackendCapabilities::all()
        } else {
            let cloud_speech = openai_api_key.is_some();
            let uv = !options.uv_binary.trim().is_empty();
            BackendCapabilities {
                llm: llm.is_ok(),
                stt_cloud: cloud_speech,
                stt_local: uv && options.workers_dir.join(CANARY_WORKER_SCRIPT).is_file(),
                tts_cloud: cloud_speech,
                tts_local: uv && options.workers_dir.join(POCKET_WORKER_SCRIPT).is_file(),
            }
        };

        Self {
            llm,
            openai_api_key,
            local_stt_model: environment
                .trimmed("LOCAL_STT_MODEL")
                .unwrap_or(DEFAULT_LOCAL_STT_MODEL)
                .to_string(),
            npc_turn_delay_seconds: environment
                .float("NPC_TURN_DELAY_SECONDS", DEFAULT_NPC_TURN_DELAY_SECONDS),
            workers_dir: options.workers_dir.clone(),
            uv_binary: options.uv_binary.clone(),
            fake_mode: options.fake_mode,
            speech: SpeechSettings::resolve(environment, options),
            capabilities,
        }
    }

    pub fn capabilities(&self) -> BackendCapabilities {
        self.capabilities
    }

    /// The startup voice backend and, when it had to be forced off, the message
    /// that says why (`server.py:500-512`).
    pub fn tts_selection(&self) -> (TtsBackendKind, Option<String>) {
        select_tts_backend(&self.speech.tts_backend, self.capabilities)
    }

    /// The model `complete()` would use, or `None` when misconfigured
    /// (`llm_client.py:103-109`). Feeds the prompt log's `meta.model`; fake mode
    /// overrides it with `"fake"` at the call site (`server.py:534-543`).
    pub fn model_name(&self) -> Option<String> {
        self.llm
            .as_ref()
            .ok()
            .map(|settings| settings.model.clone())
    }
}

/// `_config()` + `_get_client()`'s key resolution, minus the SDK
/// (`llm_client.py:48-83`).
fn resolve_llm(
    environment: &Environment,
    options: &BackendsOptions,
) -> Result<LlmSettings, LlmConfigError> {
    let raw_provider = environment.get("LLM_PROVIDER").unwrap_or("moonshot");
    let provider = Provider::parse(raw_provider).ok_or_else(|| {
        LlmConfigError(unknown_provider_message(
            raw_provider.trim().to_lowercase().as_str(),
        ))
    })?;
    let spec = provider.spec();

    // `os.environ.get("LLM_MODEL") or default`: an empty override is no override.
    let model = environment
        .trimmed("LLM_MODEL")
        .unwrap_or(spec.default_model)
        .to_string();

    let mut api_key = environment
        .trimmed(spec.key_env)
        .unwrap_or_default()
        .to_string();
    if api_key.is_empty() && provider == Provider::Moonshot {
        // Legacy fallback from before `.env` existed (`llm_client.py:67-71`).
        if let Some(key_file) = environment.moonshot_key_file()
            && let Ok(contents) = std::fs::read_to_string(&key_file)
        {
            api_key = contents.trim().to_string();
        }
    }
    if api_key.is_empty() {
        let dotenv = options
            .dotenv_path
            .as_deref()
            .unwrap_or(Path::new(DEFAULT_DOTENV_PATH));
        return Err(LlmConfigError(format!(
            "{} not set - put it in {} or the environment",
            spec.key_env,
            dotenv.display()
        )));
    }

    Ok(LlmSettings {
        provider,
        model,
        base_url: spec.base_url.to_string(),
        api_key,
        timeout_seconds: environment.float("LLM_TIMEOUT_SECONDS", DEFAULT_LLM_TIMEOUT_SECONDS),
        content_parts: environment.bool("LLM_CONTENT_PARTS", true),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Environment {
        Environment::from_map(
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect(),
        )
    }

    fn options() -> BackendsOptions {
        BackendsOptions {
            dotenv_path: Some(PathBuf::from("prompt_playgound/.env")),
            workers_dir: PathBuf::from("/nonexistent-workers"),
            uv_binary: "uv".to_string(),
            fake_mode: false,
        }
    }

    #[test]
    fn moonshot_is_the_default_provider_with_its_default_model() {
        let config = BackendsConfig::resolve(&env(&[("MOONSHOT_API_KEY", "sk-m")]), &options());
        let settings = config.llm.expect("configured");
        assert_eq!(settings.provider, Provider::Moonshot);
        assert_eq!(settings.model, "kimi-k2.5");
        assert_eq!(settings.base_url, "https://api.moonshot.ai/v1");
        assert_eq!(settings.api_key, "sk-m");
        assert_eq!(settings.timeout_seconds, 45.0);
        assert!(settings.content_parts);
        assert_eq!(
            settings.chat_completions_url(),
            "https://api.moonshot.ai/v1/chat/completions"
        );
    }

    #[test]
    fn provider_names_are_trimmed_and_lowercased() {
        let config = BackendsConfig::resolve(
            &env(&[("LLM_PROVIDER", "  OpenAI \n"), ("OPENAI_API_KEY", "sk-o")]),
            &options(),
        );
        let settings = config.llm.expect("configured");
        assert_eq!(settings.provider, Provider::Openai);
        assert_eq!(settings.model, "gpt-5.6-luna");
        assert_eq!(settings.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn an_empty_model_override_is_no_override() {
        let config = BackendsConfig::resolve(
            &env(&[("MOONSHOT_API_KEY", "sk-m"), ("LLM_MODEL", "   ")]),
            &options(),
        );
        assert_eq!(config.model_name().as_deref(), Some("kimi-k2.5"));

        let config = BackendsConfig::resolve(
            &env(&[("MOONSHOT_API_KEY", "sk-m"), ("LLM_MODEL", "kimi-k3")]),
            &options(),
        );
        assert_eq!(config.model_name().as_deref(), Some("kimi-k3"));
    }

    #[test]
    fn an_unknown_provider_is_a_configuration_error_and_has_no_model() {
        let config = BackendsConfig::resolve(&env(&[("LLM_PROVIDER", "llama")]), &options());
        assert_eq!(
            config.llm.unwrap_err(),
            LlmConfigError(
                "unknown LLM_PROVIDER 'llama' (expected one of: moonshot, openai)".to_string()
            )
        );
        assert_eq!(
            BackendsConfig::resolve(&env(&[("LLM_PROVIDER", "llama")]), &options()).model_name(),
            None
        );
    }

    #[test]
    fn a_missing_key_names_the_env_var_and_the_dotenv_path() {
        let config = BackendsConfig::resolve(&env(&[("LLM_PROVIDER", "openai")]), &options());
        assert_eq!(
            config.llm.unwrap_err(),
            LlmConfigError(
                "OPENAI_API_KEY not set - put it in prompt_playgound/.env or the environment"
                    .to_string()
            )
        );
    }

    #[test]
    fn moonshot_falls_back_to_the_legacy_key_file() {
        let home = tempdir("moonshot-key");
        let key_directory = home.join(".config/moonshot");
        std::fs::create_dir_all(&key_directory).expect("key directory");
        std::fs::write(key_directory.join("key"), "  sk-file\n").expect("key file");

        let environment = env(&[]).with_home(Some(home.clone()));
        let config = BackendsConfig::resolve(&environment, &options());
        assert!(config.capabilities().llm);
        assert_eq!(config.llm.expect("configured").api_key, "sk-file");

        // openai gets no such fallback.
        let environment = env(&[("LLM_PROVIDER", "openai")]).with_home(Some(home.clone()));
        let config = BackendsConfig::resolve(&environment, &options());
        assert!(config.llm.is_err());
        assert!(!config.capabilities().llm);

        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn capabilities_probe_keys_and_worker_scripts_without_the_network() {
        let workers = tempdir("workers");
        std::fs::create_dir_all(&workers).expect("workers dir");
        std::fs::write(workers.join(POCKET_WORKER_SCRIPT), "#").expect("pocket worker");

        let mut options = options();
        options.workers_dir = workers.clone();

        let config = BackendsConfig::resolve(&env(&[("OPENAI_API_KEY", "sk-o")]), &options);
        let capabilities = config.capabilities();
        // No MOONSHOT key and the default provider is moonshot: cognition is out,
        // speech is not.
        assert!(!capabilities.llm);
        assert!(capabilities.stt_cloud && capabilities.tts_cloud);
        assert!(capabilities.tts_local, "pocket worker script exists");
        assert!(!capabilities.stt_local, "canary worker script does not");
        assert!(capabilities.stt() && capabilities.tts());

        // No uv binary: both local workers are unreachable.
        options.uv_binary = "  ".to_string();
        let config = BackendsConfig::resolve(&env(&[("OPENAI_API_KEY", "sk-o")]), &options);
        assert!(!config.capabilities().tts_local);

        std::fs::remove_dir_all(&workers).ok();
    }

    /// The speech environment (`speech-python.md` §6), resolved once.
    #[test]
    fn the_speech_settings_come_from_the_documented_environment() {
        let speech = BackendsConfig::resolve(&env(&[]), &options()).speech;
        assert_eq!(speech.api_key, None, "no key, no cloud speech");
        assert_eq!(speech.stt_model, "gpt-4o-transcribe");
        assert_eq!(speech.tts_model, "tts-1");
        assert_eq!(speech.timeout_seconds, 30.0);
        assert_eq!(speech.base_url, "https://api.openai.com/v1");
        assert_eq!(speech.local_stt_python, "3.12");
        assert_eq!(speech.local_tts_python, "3.12");
        assert_eq!(speech.local_stt_model, "nvidia/canary-qwen-2.5b");
        assert_eq!(speech.local_stt_torch_index, None);
        assert_eq!(speech.tts_backend, "local", "voices are local by default");
        assert_eq!(speech.stream_grace_seconds, 2.0);
        assert_eq!(speech.realtime, RealtimeSettings::default());
        assert_eq!(speech.realtime.model, "gpt-realtime-whisper");
        assert_eq!(
            speech.realtime.url,
            "wss://api.openai.com/v1/realtime?intent=transcription"
        );

        let speech = BackendsConfig::resolve(
            &env(&[
                ("OPENAI_API_KEY", " sk-speech \n"),
                ("STT_MODEL", "whisper-next"),
                ("TTS_MODEL", "tts-2"),
                ("SPEECH_TIMEOUT_SECONDS", "12"),
                ("LOCAL_STT_PYTHON", "3.13"),
                ("LOCAL_TTS_PYTHON", "3.11"),
                (
                    "LOCAL_STT_TORCH_INDEX",
                    "https://download.pytorch.org/whl/cpu",
                ),
                ("STT_REALTIME_MODEL", "gpt-realtime-next"),
                ("STT_REALTIME_DELAY", "high"),
                ("STT_LANGUAGE", "sv"),
                ("STT_STREAM_IDLE_CLOSE_S", "42"),
                ("STT_REALTIME_MAX_IN_FLIGHT", "9"),
                ("STT_STREAM_COMPLETION_GRACE_MS", "3500"),
                ("SMART_ACTORS_TTS_BACKEND", "  CLOUD "),
                ("TTS_VOICE_SVEN", "alloy"),
            ]),
            &options(),
        )
        .speech;
        assert_eq!(speech.api_key.as_deref(), Some("sk-speech"), "trimmed");
        assert_eq!(speech.stt_model, "whisper-next");
        assert_eq!(speech.tts_model, "tts-2");
        assert_eq!(speech.timeout_seconds, 12.0);
        assert_eq!(speech.local_stt_python, "3.13");
        assert_eq!(speech.local_tts_python, "3.11");
        assert_eq!(
            speech.local_stt_torch_index.as_deref(),
            Some("https://download.pytorch.org/whl/cpu")
        );
        assert_eq!(speech.realtime.model, "gpt-realtime-next");
        assert_eq!(speech.realtime.delay, "high");
        assert_eq!(speech.realtime.language.as_deref(), Some("sv"));
        assert_eq!(speech.realtime.idle_close_seconds, 42.0);
        assert_eq!(speech.realtime.max_in_flight, 9);
        assert_eq!(speech.stream_grace_seconds, 3.5);
        assert_eq!(speech.tts_backend, "cloud", "trimmed and lowercased");
        assert_eq!(
            speech
                .voice_overrides
                .get("TTS_VOICE_SVEN")
                .map(String::as_str),
            Some("alloy")
        );
    }

    /// A grace window shorter than 200 ms is a fallback storm (`server.py:570-576`).
    #[test]
    fn the_stream_grace_has_a_floor() {
        let speech =
            BackendsConfig::resolve(&env(&[("STT_STREAM_COMPLETION_GRACE_MS", "0")]), &options())
                .speech;
        assert_eq!(speech.stream_grace_seconds, 0.2);

        // Nonsense reads as "unset", not as zero.
        let speech = BackendsConfig::resolve(
            &env(&[("STT_STREAM_COMPLETION_GRACE_MS", "soon")]),
            &options(),
        )
        .speech;
        assert_eq!(speech.stream_grace_seconds, 2.0);
    }

    /// A commit gate of zero would never let an utterance through, so zero and
    /// nonsense both read as "unset".
    #[test]
    fn the_realtime_commit_gate_ignores_zero_and_nonsense() {
        let speech =
            BackendsConfig::resolve(&env(&[("STT_REALTIME_MAX_IN_FLIGHT", "0")]), &options())
                .speech;
        assert_eq!(speech.realtime.max_in_flight, 4);

        let speech =
            BackendsConfig::resolve(&env(&[("STT_REALTIME_MAX_IN_FLIGHT", "many")]), &options())
                .speech;
        assert_eq!(speech.realtime.max_in_flight, 4);
    }

    /// `server.py:500-512` — an unavailable or nonsense voice backend is forced
    /// off, and says so in the exact words the HUD shows.
    #[test]
    fn the_tts_selection_falls_back_to_off_with_a_reason() {
        let none = BackendCapabilities::default();
        let cloud_only = BackendCapabilities {
            tts_cloud: true,
            ..BackendCapabilities::default()
        };

        assert_eq!(
            select_tts_backend("local", BackendCapabilities::all()),
            (TtsBackendKind::Local, None)
        );
        assert_eq!(
            select_tts_backend("cloud", cloud_only),
            (TtsBackendKind::Cloud, None)
        );
        assert_eq!(select_tts_backend("off", none), (TtsBackendKind::Off, None));
        assert_eq!(
            select_tts_backend("local", cloud_only),
            (
                TtsBackendKind::Off,
                Some(
                    "Configured local NPC voice backend is unavailable; voices are off".to_string()
                )
            )
        );
        assert_eq!(
            select_tts_backend("kokoro", BackendCapabilities::all()),
            (
                TtsBackendKind::Off,
                Some("Configured NPC voice mode is invalid; voices are off".to_string())
            ),
            "Kokoro is gone (D6), and an unknown mode is never guessed at"
        );

        // The whole-config path: no key, no worker script, so `local` is out.
        let config = BackendsConfig::resolve(&env(&[]), &options());
        assert_eq!(
            config.tts_selection(),
            (
                TtsBackendKind::Off,
                Some(
                    "Configured local NPC voice backend is unavailable; voices are off".to_string()
                )
            )
        );
    }

    #[test]
    fn fake_mode_declares_every_capability() {
        let mut options = options();
        options.fake_mode = true;
        let config = BackendsConfig::resolve(&env(&[]), &options);
        assert_eq!(config.capabilities(), BackendCapabilities::all());
        assert!(config.capabilities().stt() && config.capabilities().tts());
        // The provider is still unconfigured underneath — fake cognition replaces it.
        assert!(config.llm.is_err());
    }

    #[test]
    fn the_content_parts_flag_and_timeout_come_from_the_environment() {
        let config = BackendsConfig::resolve(
            &env(&[
                ("MOONSHOT_API_KEY", "sk-m"),
                ("LLM_CONTENT_PARTS", "0"),
                ("LLM_TIMEOUT_SECONDS", "12.5"),
            ]),
            &options(),
        );
        let settings = config.llm.expect("configured");
        assert!(!settings.content_parts);
        assert_eq!(settings.timeout_seconds, 12.5);
    }

    #[test]
    fn the_turn_delay_defaults_to_one_second() {
        let config = BackendsConfig::resolve(&env(&[]), &options());
        assert_eq!(config.npc_turn_delay_seconds, 1.0);
        let config =
            BackendsConfig::resolve(&env(&[("NPC_TURN_DELAY_SECONDS", "0.25")]), &options());
        assert_eq!(config.npc_turn_delay_seconds, 0.25);
    }

    #[test]
    fn real_environment_variables_win_over_the_dotenv_file() {
        let directory = tempdir("dotenv");
        std::fs::create_dir_all(&directory).expect("dotenv dir");
        let dotenv = directory.join(".env");
        std::fs::write(&dotenv, "MOONSHOT_API_KEY=from-file\nLLM_MODEL=from-file\n")
            .expect("dotenv");

        // Simulate `from_process`: process vars first, `.env` fills the gaps.
        let mut vars: BTreeMap<String, String> = BTreeMap::new();
        vars.insert("MOONSHOT_API_KEY".to_string(), "from-env".to_string());
        for entry in dotenvy::from_path_iter(&dotenv)
            .expect("readable")
            .flatten()
        {
            vars.entry(entry.0).or_insert(entry.1);
        }

        let config = BackendsConfig::resolve(&Environment::from_map(vars), &options());
        let settings = config.llm.expect("configured");
        assert_eq!(settings.api_key, "from-env", "process env wins");
        assert_eq!(settings.model, "from-file", "the file fills what env omits");

        std::fs::remove_dir_all(&directory).ok();
    }

    /// The `.env` keys the *workers* read — `TTS_POCKET_VOICE_*` inside
    /// `pocket_tts_worker.py`, `HF_TOKEN` inside `huggingface_hub` — have to be
    /// handed to the child, because a Rust child inherits the process
    /// environment and `.env` was never in it. Python's `load_dotenv` mutated
    /// `os.environ`, so its `os.environ.copy()` spawn carried them for free.
    #[test]
    fn dotenv_variables_are_handed_to_the_worker_children() {
        let directory = tempdir("worker-env");
        let dotenv = directory.join(".env");
        std::fs::write(
            &dotenv,
            "TTS_POCKET_VOICE_SVEN=alba\nHF_TOKEN=hf_secret\nPATH=/nowhere\n",
        )
        .expect("dotenv");

        let environment = Environment::from_process(Some(&dotenv));
        let worker_env = environment.worker_env();
        assert!(
            worker_env.contains(&("TTS_POCKET_VOICE_SVEN".to_string(), "alba".to_string())),
            "the voice override the worker script reads: {worker_env:?}"
        );
        assert!(
            worker_env.contains(&("HF_TOKEN".to_string(), "hf_secret".to_string())),
            "the download token: {worker_env:?}"
        );
        assert!(
            !worker_env.iter().any(|(key, _)| key == "PATH"),
            "a variable the process already has is inherited, not re-sent",
        );
        assert_eq!(
            environment.trimmed("TTS_POCKET_VOICE_SVEN"),
            Some("alba"),
            "and the parent still resolves it as before",
        );

        // It rides all the way into the resolved speech settings, which is what
        // the two worker specs build their environment from.
        let config = BackendsConfig::resolve(
            &environment,
            &BackendsOptions {
                dotenv_path: Some(dotenv.clone()),
                ..options()
            },
        );
        assert!(
            config
                .speech
                .worker_env
                .contains(&("HF_TOKEN".to_string(), "hf_secret".to_string()))
        );

        std::fs::remove_dir_all(&directory).ok();
    }

    fn tempdir(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cathedral-config-{tag}-{unique}"));
        std::fs::create_dir_all(&path).expect("temp dir");
        path
    }
}
