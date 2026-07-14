//! The backend boundary: everything the sim asks the outside world to do.
//!
//! Submission is a synchronous, non-blocking trait call that may **refuse** —
//! the scheduler's "cognition worker is busy" branch and the conversation
//! floor both need an immediate accept/refuse answer (D7). Results travel the
//! other way as plain values: the backend pushes them onto its own channel, the
//! host drains that channel and hands the values back into the next `poll`. The
//! sim therefore owns no channels, no threads and no clock.
//!
//! The speech traits are declared here in full so the boundary is one file;
//! only [`Cognition`] has an implementation before P6 ([`crate::fake`], and
//! `HttpCognition` in cathedral-backends).

use std::{
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    ids::{ActorId, RequestId, SpeechEventId},
    math::Vec3,
};

// ------------------------------------------------------------------ cognition

/// Why a cognition request failed. All failures are treated identically by the
/// scheduler (backoff, percept restore, `system:` inbox line); the two strings
/// exist only for the two places Python logged a failure, and they are not the
/// same string there:
///
/// * [`kind`](Self::kind) is the port of `type(error).__name__` — the short name
///   the stderr diagnostic shows (`scheduler.py:242-246`);
/// * [`detail`](Self::detail) is the port of `repr(error)` — the full text the
///   prompt archive keeps in `meta.error` (`scheduler.py:205-213`, prompt.md
///   §5.2). Without it a 401, a rate limit and an outage are indistinguishable
///   in the log.
///
/// [`CognitionError::new`] sets both to the kind, which is right for the errors
/// the sim raises itself (they have no further story to tell).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CognitionError {
    kind: String,
    detail: String,
}

impl CognitionError {
    /// A failure whose kind name *is* everything there is to say about it.
    pub fn new(kind: impl Into<String>) -> Self {
        let kind = kind.into();
        Self {
            detail: kind.clone(),
            kind,
        }
    }

    /// A failure that carries a status code, a provider message, or both.
    pub fn detailed(kind: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            detail: detail.into(),
        }
    }

    /// The short kind name, for the one-line diagnostic.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The full text, for the prompt archive.
    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn as_str(&self) -> &str {
        &self.kind
    }
}

impl fmt::Display for CognitionError {
    /// The kind: this is what the `[smart actors] LLM request for … failed: …`
    /// diagnostic interpolates, and Python printed the kind there.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.kind)
    }
}

impl std::error::Error for CognitionError {}

/// The backend cannot take a request right now (`queue.Full`, dead worker).
///
/// Defensive: the scheduler never submits while a request is in flight, so a
/// well-behaved backend with a capacity-1 queue can never return this. The
/// branch exists — with its percept restore — because Python's did
/// (`scheduler.py:336-351`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CognitionBusy;

impl fmt::Display for CognitionBusy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the cognition worker is busy")
    }
}

impl std::error::Error for CognitionBusy {}

/// One finished cognition request, matched back to its submission by
/// [`RequestId`] (D10 — strictly stronger than Python's actor-id echo).
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub request_id: RequestId,
    pub result: Result<String, CognitionError>,
    /// Wall time of the provider call, measured by the backend: the sim never
    /// reads a clock.
    pub duration_seconds: f64,
}

pub trait Cognition {
    /// Non-blocking submit. The backend measures the duration and later pushes
    /// a [`Completion`] carrying this [`RequestId`]; the host feeds it back
    /// into [`NpcScheduler::poll`](crate::scheduler::NpcScheduler::poll).
    /// `Err(CognitionBusy)` takes the "cognition worker is busy" branch.
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy>;

    /// Submit with a host-side completion cap. Test fakes and legacy backends
    /// may ignore it; provider-backed cognition overrides this method. Keeping
    /// the budget outside the prompt prevents characters from learning their
    /// authoring significance.
    fn request_with_budget(
        &mut self,
        prompt: String,
        max_output_tokens: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        let _ = max_output_tokens;
        self.request(prompt)
    }
}

// ----------------------------------------------------------------- speech I/O

/// A backend failure worded for the player (≤160 chars, API keys scrubbed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechError {
    pub presentable: String,
}

impl SpeechError {
    pub fn new(presentable: impl Into<String>) -> Self {
        Self {
            presentable: presentable.into(),
        }
    }
}

impl fmt::Display for SpeechError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.presentable)
    }
}

impl std::error::Error for SpeechError {}

/// Which transcription backend an utterance was recorded for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttBackendKind {
    Cloud,
    Local,
}

impl SttBackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Local => "local",
        }
    }
}

/// Correlates a batch transcription submission with its outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TranscriptionJobId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttSubmitError {
    QueueFull,
    Unavailable,
}

pub trait Transcription {
    fn available(&self, kind: SttBackendKind) -> bool;

    /// Hand one completed recording to the batch/local pipeline.
    fn submit_batch(
        &mut self,
        job: TranscriptionJobId,
        wav_path: PathBuf,
        kind: SttBackendKind,
    ) -> Result<(), SttSubmitError>;

    /// Streaming session (cloud only). The bools mirror Python: `false` means
    /// "not streaming — fall back to batch", never an error.
    fn realtime_begin(&mut self, key: &str) -> bool;
    fn realtime_append(&mut self, key: &str, samples: &[i16]) -> bool;
    fn realtime_commit(&mut self, key: &str) -> bool;
    fn realtime_clear(&mut self, key: &str);

    /// How long the recording is, for the utterance-latency probe
    /// (`_wav_duration_seconds`, `server.py:181-215`, fed into
    /// `_begin_utterance_timing` at `server.py:1503-1508`). The sim may not open
    /// files (D22), so the backend that owns the WAV answers. `None` — the
    /// default, and any malformed header — prints `audio=?` and is never an
    /// error: a probe must not be able to break an utterance.
    fn recording_seconds(&self, wav_path: &Path) -> Option<f64> {
        let _ = wav_path;
        None
    }

    /// The recording has been heard, or will never be: delete it.
    ///
    /// Python unlinked the WAV on *every* resolution path
    /// (`_resolve_transcription`, `server.py:1594-1602`), including the utterance
    /// a realtime transcript resolved without ever uploading anything. That path
    /// hands the file to no backend at all, so without this the player's voice
    /// accumulates in the runtime directory — which is `/tmp`, i.e. RAM — for the
    /// whole session. Idempotent: the batch pipeline has usually deleted it
    /// already.
    fn discard_recording(&mut self, wav_path: &Path) {
        let _ = wav_path;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptionOutcome {
    Done {
        job: TranscriptionJobId,
        result: Result<String, SpeechError>,
    },
    Realtime(RealtimeResult),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RealtimeResult {
    Transcript {
        key: String,
        text: String,
    },
    /// A session-wide failure has no key: every parked utterance falls back.
    Failure {
        key: Option<String>,
        reason: String,
    },
}

/// The voice backend an utterance is synthesized with — captured when the
/// utterance is queued, not when it completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsBackendKind {
    Cloud,
    Local,
    Off,
}

impl TtsBackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Local => "local",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtsSubmitError {
    QueueFull,
    Unavailable,
    PathInUse,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TtsRequest {
    pub event_id: SpeechEventId,
    pub text: String,
    pub voice_key: String,
    pub kind: TtsBackendKind,
}

pub trait Tts {
    fn available(&self, kind: TtsBackendKind) -> bool;

    /// Synchronous accept/refuse: the floor only awaits an utterance whose
    /// synthesis was actually taken (`server.py:1958-2009`, R10).
    fn submit(&mut self, request: TtsRequest) -> Result<(), TtsSubmitError>;

    /// Pay the model-load cost before the first line, not during it.
    fn warm(&mut self, kind: TtsBackendKind);
}

/// The deaf [`Transcription`]: the headless runner and the text-only tests have
/// no microphone at all. Every probe says "unavailable", which is exactly what
/// the engine's degradation paths expect.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullTranscription;

impl Transcription for NullTranscription {
    fn available(&self, _kind: SttBackendKind) -> bool {
        false
    }

    fn submit_batch(
        &mut self,
        _job: TranscriptionJobId,
        _wav_path: PathBuf,
        _kind: SttBackendKind,
    ) -> Result<(), SttSubmitError> {
        Err(SttSubmitError::Unavailable)
    }

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

/// The mute [`Tts`]: the cast has text, no voice. With
/// [`TtsBackendKind::Off`] selected the engine never even asks it — the type
/// exists so a host that *has* no voice backend can still build an [`Engine`].
///
/// [`Engine`]: crate::engine::Engine
#[derive(Debug, Clone, Copy, Default)]
pub struct NullTts;

impl Tts for NullTts {
    fn available(&self, _kind: TtsBackendKind) -> bool {
        false
    }

    fn submit(&mut self, _request: TtsRequest) -> Result<(), TtsSubmitError> {
        Err(TtsSubmitError::Unavailable)
    }

    fn warm(&mut self, _kind: TtsBackendKind) {}
}

#[derive(Debug, Clone, PartialEq)]
pub enum TtsOutcome {
    Chunk {
        event_id: SpeechEventId,
        seq: u32,
        sample_rate: u32,
        samples: Arc<[i16]>,
    },
    StreamEnd {
        event_id: SpeechEventId,
        chunk_count: u32,
        first_chunk_ms: u32,
    },
    Done {
        event_id: SpeechEventId,
        /// The whole WAV, in memory.
        result: Result<Arc<[u8]>, SpeechError>,
    },
}

// --------------------------------------------------------------------- sight

/// What an NPC's eyes see, for a future multimodal prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct PovFrame {
    pub png: Arc<[u8]>,
    pub captured_at: f64,
}

/// The renderer's view of the world, for occlusion and NPC-eye screenshots.
///
/// Plumbed but unconsumed: [`crate::perception::sees`] is deliberately NOT
/// routed through this yet (ARCHITECTURE §2.6). The hook exists so wiring it in
/// later is a one-line change instead of a signature change everywhere.
pub trait Sight {
    /// STUB: always true.
    fn line_of_sight(&self, from: Vec3, to: Vec3) -> bool {
        let _ = (from, to);
        true
    }

    /// STUB: never a frame.
    fn npc_pov_frame(&mut self, actor: &ActorId) -> Option<PovFrame> {
        let _ = actor;
        None
    }
}

/// The headless/default [`Sight`]: nothing occludes, no camera exists.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullSight;

impl Sight for NullSight {}
