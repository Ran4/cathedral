//! Player speech in, NPC speech out (`server.py` §5.3–5.7).
//!
//! Two state machines share this file because they share one clock and one
//! floor:
//!
//! * **Player STT.** The microphone streams into a realtime session while it is
//!   still speaking, and the finished WAV arrives afterwards as a separate
//!   `player_recording`. The streamed copy is *pure optimisation*: every failure
//!   in it — a bad sample rate, a gap in the sequence, a dead websocket — merely
//!   marks the stream degraded, and the recording resolves through the batch
//!   upload as if the stream had never existed. That is why almost nothing here
//!   raises: a chunk that arrives after the end is a bug in nobody, and the
//!   player must still be heard.
//! * **NPC TTS.** Whether a line gets a voice at all is a *sim* question (does
//!   the speaker have a voice key, can the player hear him, are voices on?), and
//!   the answer decides how the conversation floor paces the cast. A submission
//!   the backend refuses must therefore behave exactly like a line that was
//!   never voiced — never like a line whose audio is still coming (D7/R10).
//!
//! The router is pure: no clock, no channels, no IO. Backends are called through
//! [`Transcription`]/[`Tts`], which accept or refuse *synchronously*; their
//! results come back later as [`EngineCommand`](crate::EngineCommand) variants
//! the host feeds into the next poll.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use serde_json::json;

use crate::{
    FLOOR_PLAYER_CHUNK_HOLD_SECONDS, FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS,
    FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS, HEARING_RADIUS_M, MAX_ACTIVE_STREAMS,
    MAX_UTTERANCE_TIMINGS, PLAYER_SPEECH_MAX_CHARS, STREAM_SAMPLE_RATE,
    STT_STREAM_HELD_TRANSCRIPT_SECONDS, STT_STREAM_MAX_CHUNK_SAMPLES, STT_STREAM_MAX_CHUNKS,
    actions::apply_action_at,
    character::Control,
    engine::{EngineMessage, MAX_COMMAND_MESSAGE_CHARS, MAX_TTS_FAILURE_REASON_CHARS},
    error::{CommandError, CommandErrorCode},
    event::DomainEvent,
    floor::ConversationFloor,
    ids::{ActorId, SpeechEventId},
    math::Vec3,
    pyfmt::py_strip,
    scheduler::NpcScheduler,
    status::{STATE_DEGRADED, STATE_IDLE, STATE_UNAVAILABLE, StatusEvent, Subsystem},
    traits::{
        RealtimeResult, SpeechError, SttBackendKind, SttSubmitError, Transcription,
        TranscriptionJobId, TranscriptionOutcome, Tts, TtsBackendKind, TtsOutcome, TtsRequest,
        TtsSubmitError,
    },
    world::{SpatialActorUpdate, World},
};

/// `stt` states the speech paths add to the shared `STATE_*` set.
const STATE_TRANSCRIBING: &str = "transcribing";
const STATE_LOADING: &str = "loading";
const STATE_SYNTHESIZING: &str = "synthesizing";

/// The one status the local STT backend needs a whole sentence for: the first
/// use downloads a 5 GB model, and a silent five-minute stall reads as a hang
/// (`server.py:1535`).
const LOCAL_STT_LOADING_MESSAGE: &str =
    "Loading local Canary-Qwen FP16; first use may download about 5 GB";

/// `transcription_result.error` and `command_result.message` share Python's
/// 300-char cap (`server.py:1609`).
const MAX_TRANSCRIPTION_ERROR_CHARS: usize = MAX_COMMAND_MESSAGE_CHARS;

/// How many utterances may have their queue-time voice backend remembered. One
/// per un-completed synthesis; the TTS queue itself is capacity 32.
const MAX_TRACKED_TTS_BACKENDS: usize = 64;

// --------------------------------------------------------------------- context

/// Everything outside the router that resolving one utterance needs.
///
/// The router owns *its* state (streams, parked recordings, timings) and borrows
/// the rest from the [`Engine`](crate::Engine) per call. That keeps the world,
/// the floor and the backends single-owner — and it is why the router can be
/// exercised from `Engine` alone in tests, with no second source of truth.
pub struct SpeechContext<'a> {
    pub world: &'a mut World,
    pub floor: &'a mut ConversationFloor,
    pub scheduler: &'a mut NpcScheduler,
    /// The omniscient run transcript; an applied player `say` appends to it.
    pub transcript: &'a mut Vec<String>,
    pub transcription: &'a mut dyn Transcription,
    pub tts: &'a mut dyn Tts,
    pub player_id: &'a ActorId,
    /// Where the microphone worker writes recordings. Joined with a basename to
    /// name a WAV for the batch backend; never opened here (D22).
    pub runtime_dir: &'a Path,
    /// Fake mode has no realtime session at all (`server.py:579-581`): the
    /// stream is transcribed by the batch backend at its endpoint instead.
    pub fake_mode: bool,
    /// The voice backend *right now* — captured into each request at queue time,
    /// so a mid-utterance switch cannot re-route a line already in flight.
    pub tts_selected: TtsBackendKind,
}

impl SpeechContext<'_> {
    /// Whether a realtime session exists at all. Python created one iff the run
    /// is non-fake and the cloud key is set (`server.py:577-584`); the cloud
    /// probe is the same condition.
    fn has_realtime(&self) -> bool {
        !self.fake_mode && self.transcription.available(SttBackendKind::Cloud)
    }

    fn realtime_clear(&mut self, key: &str) {
        if !self.fake_mode {
            self.transcription.realtime_clear(key);
        }
    }
}

// --------------------------------------------------------------- degrade reasons

/// Why a streamed utterance fell back to the batch upload.
///
/// The reason is *first-wins* per stream and appears verbatim in the player's
/// status line, so these strings are behavior (`server.py:1151-1170`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DegradeReason {
    /// Not 24 kHz `pcm_s16le` — the realtime session only speaks one format.
    BadFormat,
    /// No realtime session is configured (no cloud key). Expected; no status.
    NoSession,
    /// The session refused to start or commit this utterance (backoff, in-flight
    /// cap, closing). Expected; no status.
    SessionUnavailable,
    ChunkAfterEnd,
    SeqGap,
    TooManyChunks,
    OversizedChunk,
    /// Unreachable in Rust — the wire carries typed `i16` samples, not base64.
    /// Kept because the reason string is part of the status contract.
    BadBase64,
    /// The realtime session could not take the chunk; the stream is now holed.
    Backpressure,
    BadEnd,
    CountMismatch,
    /// Fake mode transcribes the stream through the batch backend; this is that
    /// backend refusing (`server.py:1299-1301`).
    FakeTranscriptionFailed,
    /// The recording arrived while the stream was still mid-flight.
    IncompleteStream,
    /// The parked recording waited out its grace window.
    Grace,
    /// A reason the realtime session itself reported (`socket`, `connect_failed`,
    /// `protocol`).
    Session(String),
}

impl DegradeReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::BadFormat => "bad_format",
            Self::NoSession => "no_session",
            Self::SessionUnavailable => "session_unavailable",
            Self::ChunkAfterEnd => "chunk_after_end",
            Self::SeqGap => "seq_gap",
            Self::TooManyChunks => "too_many_chunks",
            Self::OversizedChunk => "oversized_chunk",
            Self::BadBase64 => "bad_base64",
            Self::Backpressure => "backpressure",
            Self::BadEnd => "bad_end",
            Self::CountMismatch => "count_mismatch",
            Self::FakeTranscriptionFailed => "fake_transcription_failed",
            Self::IncompleteStream => "incomplete_stream",
            Self::Grace => "grace",
            Self::Session(reason) => reason,
        }
    }

    /// Waiting for a session — or for its reconnect backoff — is not damage, and
    /// the session publishes its own transitions. Only genuine stream damage
    /// earns a per-utterance status (`server.py:1164`).
    fn is_expected(&self) -> bool {
        matches!(self, Self::NoSession | Self::SessionUnavailable)
    }
}

impl fmt::Display for DegradeReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ------------------------------------------------------------------ router state

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamPhase {
    Streaming,
    Committed,
    Completed,
    Degraded,
}

/// One streamed player utterance, keyed by its recording basename.
#[derive(Debug, Clone, PartialEq)]
struct StreamState {
    phase: StreamPhase,
    next_seq: u32,
    decoded_bytes: u64,
    end_at: Option<f64>,
    commit_at: Option<f64>,
    completed_at: Option<f64>,
    transcript: Option<String>,
    degrade_reason: Option<DegradeReason>,
    status_sent: bool,
}

impl StreamState {
    fn new() -> Self {
        Self {
            phase: StreamPhase::Streaming,
            next_seq: 0,
            decoded_bytes: 0,
            end_at: None,
            commit_at: None,
            completed_at: None,
            transcript: None,
            degrade_reason: None,
            status_sent: false,
        }
    }

    /// Seconds of 24 kHz mono `i16` audio the stream carried.
    fn audio_seconds(&self) -> f64 {
        self.decoded_bytes as f64 / 2.0 / f64::from(STREAM_SAMPLE_RATE)
    }
}

/// One `player_recording` on its way to a transcript.
#[derive(Debug, Clone, PartialEq)]
struct TranscriptionTask {
    request_id: String,
    basename: String,
    /// Where the player stood when he *started* the utterance — the say is
    /// applied here even if he has walked away since (`server.py:1701-1717`).
    position_m: Vec3,
    backend: SttBackendKind,
}

/// A recording briefly waiting for the transcript the provider already owns.
#[derive(Debug, Clone, PartialEq)]
struct ParkedRecording {
    task: TranscriptionTask,
    deadline: f64,
}

/// Latency probes for one utterance, endpoint to applied say.
#[derive(Debug, Clone, PartialEq)]
struct UtteranceTiming {
    path: String,
    endpoint_at: f64,
    audio_seconds: Option<f64>,
    commit_at: Option<f64>,
    completed_at: Option<f64>,
}

/// The player-speech and NPC-voice state machines.
///
/// Every collection is an insertion-ordered `Vec` of pairs rather than a map:
/// each is tiny (≤ 8 streams, ≤ 64 timings) and Python's were `OrderedDict`s
/// whose *order* is load-bearing — stream eviction is FIFO, and the timing LRU
/// drops the oldest.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpeechRouter {
    /// How long a committed recording waits for its realtime transcript before
    /// paying for a batch upload.
    stt_stream_grace_seconds: f64,
    streams: Vec<(String, StreamState)>,
    parked: Vec<(String, ParkedRecording)>,
    timings: Vec<(String, UtteranceTiming)>,
    /// Batch jobs that will resolve a `player_recording`.
    recording_jobs: Vec<(TranscriptionJobId, TranscriptionTask)>,
    /// Batch jobs standing in for the realtime session in fake mode: their
    /// result completes a *stream*, not a recording.
    stream_jobs: Vec<(TranscriptionJobId, String)>,
    /// The voice backend each in-flight utterance was queued with (test 19).
    tts_backends: Vec<(SpeechEventId, TtsBackendKind)>,
    next_job: u64,
}

impl SpeechRouter {
    pub fn new(stt_stream_grace_seconds: f64) -> Self {
        Self {
            stt_stream_grace_seconds,
            ..Self::default()
        }
    }

    pub fn stt_stream_grace_seconds(&self) -> f64 {
        self.stt_stream_grace_seconds
    }

    /// Streams still being spoken, committed, or holding a transcript.
    pub fn active_stream_count(&self) -> usize {
        self.streams.len()
    }

    /// Recordings waiting out the grace window for a streamed transcript.
    pub fn parked_count(&self) -> usize {
        self.parked.len()
    }

    /// Utterances submitted to a transcription backend and not yet resolved.
    pub fn pending_transcription_count(&self) -> usize {
        self.recording_jobs.len()
    }

    // ------------------------------------------------------------------ poll

    /// `_poll_streaming` (`server.py:1325-1353`): the two deadlines nobody else
    /// watches.
    pub fn poll(&mut self, now: f64, ctx: &mut SpeechContext<'_>, out: &mut Vec<EngineMessage>) {
        // The provider is taking too long: stop paying it rent and upload.
        while let Some(index) = self
            .parked
            .iter()
            .position(|(_, parked)| now >= parked.deadline)
        {
            let (key, parked) = self.parked.remove(index);
            ctx.realtime_clear(&key);
            self.submit_parked_batch(now, parked, DegradeReason::Grace, ctx, out);
        }

        // The owning `player_recording` never arrived (the game died, or an
        // abort was lost). A held transcript must never turn into a late say.
        self.streams.retain(|(_, stream)| {
            !matches!(
                (stream.phase, stream.completed_at),
                (StreamPhase::Completed, Some(completed_at))
                    if now - completed_at > STT_STREAM_HELD_TRANSCRIPT_SECONDS
            )
        });
    }

    // ------------------------------------------------------- microphone stream

    /// `player_audio_begin` (`server.py:1180-1209`).
    pub fn on_audio_begin(
        &mut self,
        now: f64,
        basename: &str,
        sample_rate: u32,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        if let Err(error) = check_basename(basename) {
            out.push(diagnostic("player_audio_begin", &error));
            return;
        }
        // Re-beginning a live basename replaces it outright: the mic worker has
        // restarted the utterance and the old sequence expectations are void.
        if self.take_stream(basename).is_some() {
            ctx.realtime_clear(basename);
        }
        while self.streams.len() >= MAX_ACTIVE_STREAMS {
            let (evicted, _) = self.streams.remove(0);
            ctx.realtime_clear(&evicted);
        }
        self.streams
            .push((basename.to_string(), StreamState::new()));

        // The player has started speaking: hold NPC turns while chunks flow.
        ctx.floor
            .bump_player_hold(now, FLOOR_PLAYER_CHUNK_HOLD_SECONDS);

        // A degraded stream is still *tracked* — the batch fallback needs it.
        if sample_rate != STREAM_SAMPLE_RATE {
            self.degrade(basename, DegradeReason::BadFormat, out);
            return;
        }
        if ctx.fake_mode {
            return;
        }
        if !ctx.has_realtime() {
            self.degrade(basename, DegradeReason::NoSession, out);
            return;
        }
        if !ctx.transcription.realtime_begin(basename) {
            self.degrade(basename, DegradeReason::SessionUnavailable, out);
        }
    }

    /// `player_audio_chunk` (`server.py:1211-1256`).
    pub fn on_audio_chunk(
        &mut self,
        now: f64,
        basename: &str,
        seq: u32,
        samples: &[i16],
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        // Trailing chunks after an abort or a silent end must never resurrect a
        // released player hold, so an unknown basename is silence, not an error.
        let Some(stream) = self.stream_mut(basename) else {
            return;
        };
        let phase = stream.phase;
        // Even a chunk bound for a degraded stream means the player is audibly
        // mid-utterance; the recording still lands via the batch fallback.
        ctx.floor
            .bump_player_hold(now, FLOOR_PLAYER_CHUNK_HOLD_SECONDS);

        match phase {
            // The game cannot know a stream degraded; trailing chunks are
            // expected and must not re-report it.
            StreamPhase::Degraded => return,
            StreamPhase::Streaming => {}
            _ => {
                self.degrade(basename, DegradeReason::ChunkAfterEnd, out);
                return;
            }
        }

        let stream = self.stream_mut(basename).expect("checked above");
        if seq != stream.next_seq {
            self.degrade(basename, DegradeReason::SeqGap, out);
            return;
        }
        if stream.next_seq >= STT_STREAM_MAX_CHUNKS {
            self.degrade(basename, DegradeReason::TooManyChunks, out);
            return;
        }
        if samples.is_empty() || samples.len() > STT_STREAM_MAX_CHUNK_SAMPLES {
            self.degrade(basename, DegradeReason::OversizedChunk, out);
            return;
        }
        stream.next_seq += 1;
        stream.decoded_bytes += (samples.len() * 2) as u64;

        if ctx.has_realtime() && !ctx.transcription.realtime_append(basename, samples) {
            // The session dropped audio: what it holds is now holed, so it can
            // never produce a faithful transcript of this utterance.
            self.degrade(basename, DegradeReason::Backpressure, out);
            ctx.realtime_clear(basename);
        }
    }

    /// `player_audio_end` (`server.py:1258-1312`).
    pub fn on_audio_end(
        &mut self,
        now: f64,
        basename: &str,
        chunk_count: u32,
        silent: bool,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        let Some(stream) = self.stream_mut(basename) else {
            return;
        };
        if silent {
            // The worker discards sub-minimum utterances locally; nothing may
            // ever be committed or said for them, so give the floor back at once.
            ctx.floor.clear_player_hold();
            self.take_stream(basename);
            ctx.realtime_clear(basename);
            return;
        }

        // Endpoint reached: the transcript and the resulting say normally land
        // inside this window (`player_recording` extends it further).
        let phase = stream.phase;
        ctx.floor
            .bump_player_hold(now, FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS);
        match phase {
            StreamPhase::Degraded => return,
            StreamPhase::Streaming => {}
            _ => {
                self.degrade(basename, DegradeReason::BadEnd, out);
                return;
            }
        }

        let stream = self.stream_mut(basename).expect("checked above");
        if chunk_count != stream.next_seq || stream.next_seq == 0 {
            self.degrade(basename, DegradeReason::CountMismatch, out);
            return;
        }
        stream.end_at = Some(now);

        if ctx.fake_mode {
            // No websocket exists offline, so the batch backend plays the
            // provider: the utterance is transcribed *now*, at its endpoint,
            // and the result completes the stream exactly as a realtime
            // transcript would (`server.py:1293-1304`).
            stream.commit_at = Some(now);
            let job = self.next_job();
            let path = ctx.runtime_dir.join(basename);
            match ctx
                .transcription
                .submit_batch(job, path, SttBackendKind::Cloud)
            {
                Ok(()) => {
                    self.stream_mut(basename).expect("checked above").phase =
                        StreamPhase::Committed;
                    self.stream_jobs.push((job, basename.to_string()));
                }
                Err(_) => self.degrade(basename, DegradeReason::FakeTranscriptionFailed, out),
            }
            return;
        }
        if !ctx.has_realtime() {
            self.degrade(basename, DegradeReason::NoSession, out);
            return;
        }
        if ctx.transcription.realtime_commit(basename) {
            let stream = self.stream_mut(basename).expect("checked above");
            stream.phase = StreamPhase::Committed;
            stream.commit_at = stream.end_at;
        } else {
            self.degrade(basename, DegradeReason::SessionUnavailable, out);
        }
    }

    /// `player_audio_abort` (`server.py:1314-1323`).
    ///
    /// A *parked* recording is deliberately untouched: it belongs to an
    /// in-flight `player_recording` whose grace timer owns its resolution.
    pub fn on_audio_abort(&mut self, basename: &str, ctx: &mut SpeechContext<'_>) {
        if self.take_stream(basename).is_some() {
            ctx.floor.clear_player_hold();
            ctx.realtime_clear(basename);
        }
    }

    // --------------------------------------------------------- player_recording

    /// `player_recording` (`server.py:1401-1539`) — the batch entry point and the
    /// hand-off from a streamed utterance.
    #[allow(clippy::too_many_arguments)]
    pub fn on_recording(
        &mut self,
        now: f64,
        request_id: &str,
        basename: &str,
        stt_backend: SttBackendKind,
        position_m: Vec3,
        spatial_seq: i64,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        match self.accept_recording(
            now,
            request_id,
            basename,
            stt_backend,
            position_m,
            spatial_seq,
            ctx,
            out,
        ) {
            Ok(()) => {}
            Err(error) => {
                out.push(EngineMessage::TranscriptionResult {
                    request_id: request_id.to_string(),
                    text: None,
                    error: Some(truncate_chars(
                        &error.message,
                        MAX_TRANSCRIPTION_ERROR_CHARS,
                    )),
                });
                out.push(command_failure(request_id, error.code, &error.message));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn accept_recording(
        &mut self,
        now: f64,
        request_id: &str,
        basename: &str,
        stt_backend: SttBackendKind,
        position_m: Vec3,
        spatial_seq: i64,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) -> Result<(), CommandError> {
        check_basename(basename)?;
        // The WAV's existence, its confinement to the runtime directory and its
        // ownership by another audio task are file facts: cathedral-backends
        // owns them (ARCHITECTURE §1.2). What is *not* a file fact — can this
        // backend transcribe at all — is decided here, before anything is
        // charged for.
        if !ctx.transcription.available(stt_backend) {
            return Err(CommandError::new(
                CommandErrorCode::SttUnavailable,
                format!(
                    "{} speech transcription is unavailable",
                    stt_backend.as_str()
                ),
            ));
        }

        // The position the payload carries *is* the utterance position: applying
        // it now both keeps the world current and pins where the say will land.
        ctx.world.update_positions(
            spatial_seq,
            &[SpatialActorUpdate::new(
                ctx.player_id.clone(),
                position_m,
                None,
            )],
        )?;
        let utterance_position = ctx.world.characters[ctx.player_id].position_m();

        let task = TranscriptionTask {
            request_id: request_id.to_string(),
            basename: basename.to_string(),
            position_m: utterance_position,
            backend: stt_backend,
        };

        let mut stream = self.take_stream(basename);
        if stream.is_some() && stt_backend != SttBackendKind::Cloud {
            // The player switched to local transcription mid-utterance; the
            // streamed copy is irrelevant now.
            ctx.realtime_clear(basename);
            stream = None;
        }

        match stream.as_ref().map(|stream| stream.phase) {
            // The transcript is already here: no upload, no wait.
            Some(StreamPhase::Completed) => {
                let stream = stream.expect("matched above");
                self.begin_timing(
                    now,
                    basename,
                    "stream",
                    Some(stream.audio_seconds()),
                    stream.end_at,
                );
                if let Some(timing) = self.timing_mut(basename) {
                    timing.commit_at = stream.commit_at;
                    timing.completed_at = stream.completed_at;
                }
                let transcript = stream.transcript.unwrap_or_default();
                self.resolve(now, task, Ok(transcript), ctx, out);
                Ok(())
            }
            // The provider already holds all the audio: wait briefly for its
            // transcript instead of paying for a batch upload of the same bytes.
            Some(StreamPhase::Committed) => {
                let stream = stream.expect("matched above");
                self.parked.push((
                    basename.to_string(),
                    ParkedRecording {
                        task,
                        deadline: now + self.stt_stream_grace_seconds,
                    },
                ));
                // The transcript is in flight (and may still fall back to a
                // batch round-trip); keep NPC turns held meanwhile.
                ctx.floor
                    .bump_player_hold(now, FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS);
                self.begin_timing(
                    now,
                    basename,
                    "stream",
                    Some(stream.audio_seconds()),
                    stream.end_at,
                );
                if let Some(timing) = self.timing_mut(basename) {
                    timing.commit_at = stream.commit_at;
                }
                out.push(EngineMessage::Status(stt_status(
                    STATE_TRANSCRIBING,
                    None,
                    Some(SttBackendKind::Cloud),
                )));
                Ok(())
            }
            // No stream, or one that never made it: upload the WAV.
            _ => {
                let wav_path = ctx.runtime_dir.join(basename);
                // Python measured the file itself here (`_wav_duration_seconds`,
                // `server.py:1503-1508`) — the probe exists to normalise latency
                // against utterance length, and `audio=?` makes it useless. The
                // sim may not open the file (D22), so the backend that owns it
                // answers; a streamed sample count stands in if it cannot.
                let measured = ctx.transcription.recording_seconds(&wav_path);
                let (path, audio_seconds, endpoint_at) = match &stream {
                    Some(stream) => (
                        format!(
                            "batch(fallback:{})",
                            stream
                                .degrade_reason
                                .clone()
                                .unwrap_or(DegradeReason::IncompleteStream)
                        ),
                        measured.or_else(|| Some(stream.audio_seconds())),
                        stream.end_at,
                    ),
                    None => ("batch".to_string(), measured, None),
                };
                let job = self.next_job();
                submit_batch(ctx, job, wav_path, stt_backend)?;
                self.recording_jobs.push((job, task));
                // A batch round-trip can take seconds; keep NPC turns held until
                // it resolves (or the hold expires on its own).
                ctx.floor
                    .bump_player_hold(now, FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS);
                self.begin_timing(now, basename, &path, audio_seconds, endpoint_at);
                out.push(EngineMessage::Status(match stt_backend {
                    SttBackendKind::Local => stt_status(
                        STATE_LOADING,
                        Some(LOCAL_STT_LOADING_MESSAGE.to_string()),
                        Some(SttBackendKind::Local),
                    ),
                    SttBackendKind::Cloud => {
                        stt_status(STATE_TRANSCRIBING, None, Some(SttBackendKind::Cloud))
                    }
                }));
                Ok(())
            }
        }
    }

    // ------------------------------------------------------- backend completions

    /// A transcription backend finished — either a recording's batch job, or (in
    /// fake mode) the batch job standing in for the realtime session.
    pub fn on_transcription(
        &mut self,
        now: f64,
        outcome: TranscriptionOutcome,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        match outcome {
            TranscriptionOutcome::Done { job, result } => {
                if let Some(index) = self
                    .recording_jobs
                    .iter()
                    .position(|(candidate, _)| *candidate == job)
                {
                    let (_, task) = self.recording_jobs.remove(index);
                    self.resolve(now, task, result, ctx, out);
                    return;
                }
                let Some(index) = self
                    .stream_jobs
                    .iter()
                    .position(|(candidate, _)| *candidate == job)
                else {
                    // A job nobody is waiting for: a late completion after the
                    // utterance already fell back. Discarded, never a second say.
                    return;
                };
                let (_, key) = self.stream_jobs.remove(index);
                let result = match result {
                    Ok(text) => RealtimeResult::Transcript { key, text },
                    Err(_) => RealtimeResult::Failure {
                        key: Some(key),
                        reason: DegradeReason::FakeTranscriptionFailed.as_str().to_string(),
                    },
                };
                self.on_realtime(now, result, ctx, out);
            }
            TranscriptionOutcome::Realtime(result) => self.on_realtime(now, result, ctx, out),
        }
    }

    /// `_apply_realtime_result` (`server.py:1355-1387`).
    fn on_realtime(
        &mut self,
        now: f64,
        result: RealtimeResult,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        match result {
            RealtimeResult::Transcript { key, text } => {
                if let Some(parked) = self.take_parked(&key) {
                    if let Some(timing) = self.timing_mut(&key) {
                        timing.completed_at = Some(now);
                    }
                    self.resolve(now, parked.task, Ok(text), ctx, out);
                    return;
                }
                if let Some(stream) = self.stream_mut(&key)
                    && stream.phase == StreamPhase::Committed
                {
                    stream.phase = StreamPhase::Completed;
                    stream.transcript = Some(text);
                    stream.completed_at = Some(now);
                }
                // An unknown key is a late completion after a fallback: dropped.
            }
            // A session-wide failure has no key: every live streamed utterance
            // falls back, or the player is simply never heard.
            RealtimeResult::Failure { key: None, reason } => {
                let reason = DegradeReason::Session(reason);
                while !self.parked.is_empty() {
                    let (_, parked) = self.parked.remove(0);
                    self.submit_parked_batch(now, parked, reason.clone(), ctx, out);
                }
                let live: Vec<String> = self
                    .streams
                    .iter()
                    .filter(|(_, stream)| {
                        matches!(
                            stream.phase,
                            StreamPhase::Streaming | StreamPhase::Committed
                        )
                    })
                    .map(|(key, _)| key.clone())
                    .collect();
                for key in live {
                    self.degrade(&key, reason.clone(), out);
                }
            }
            RealtimeResult::Failure {
                key: Some(key),
                reason,
            } => {
                let reason = match reason.as_str() {
                    "fake_transcription_failed" => DegradeReason::FakeTranscriptionFailed,
                    _ => DegradeReason::Session(reason),
                };
                if let Some(parked) = self.take_parked(&key) {
                    self.submit_parked_batch(now, parked, reason, ctx, out);
                    return;
                }
                if self.stream_mut(&key).is_some() {
                    self.degrade(&key, reason, out);
                }
            }
        }
    }

    /// `_submit_parked_batch` (`server.py:1389-1399`).
    fn submit_parked_batch(
        &mut self,
        now: f64,
        parked: ParkedRecording,
        reason: DegradeReason,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        let task = parked.task;
        if let Some(timing) = self.timing_mut(&task.basename) {
            timing.path = format!("batch(fallback:{reason})");
        }
        let job = self.next_job();
        let wav_path = ctx.runtime_dir.join(&task.basename);
        match submit_batch(ctx, job, wav_path, task.backend) {
            Ok(()) => self.recording_jobs.push((job, task)),
            // Nothing will ever transcribe this: resolve it as a failure rather
            // than leave the player's pending request — and his floor hold —
            // hanging forever.
            Err(error) => self.resolve(now, task, Err(SpeechError::new(error.message)), ctx, out),
        }
    }

    // ----------------------------------------------------------- the resolution

    /// `_handle_transcription_outcome` (`server.py:1573-1583`): every path — the
    /// batch worker, the streamed hand-off, a queue that was full — converges
    /// here, and every one of them logs its timing exactly once.
    fn resolve(
        &mut self,
        now: f64,
        task: TranscriptionTask,
        result: Result<String, SpeechError>,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        let basename = task.basename.clone();
        // `_resolve_transcription` began by unlinking the recording, whichever
        // road it came down (`server.py:1594-1602`). The batch pipeline deletes
        // the WAV as it finishes with it, but an utterance the realtime socket
        // resolved was never handed to any backend — nobody else will ever delete
        // that file, and it is a recording of the player's voice.
        let recording = ctx.runtime_dir.join(&basename);
        ctx.transcription.discard_recording(&recording);
        self.resolve_transcription(now, task, result, ctx, out);
        self.log_timing(now, &basename, out);
    }

    /// `_resolve_transcription` (`server.py:1585-1749`).
    fn resolve_transcription(
        &mut self,
        now: f64,
        task: TranscriptionTask,
        result: Result<String, SpeechError>,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        // Whatever the outcome, the player's utterance is no longer pending: on
        // success the applied say plus the NPC floor govern pacing from here; on
        // failure nothing will be said, so NPC turns may resume at once.
        ctx.floor.clear_player_hold();

        let backend = task.backend;
        let text = match result {
            Err(error) => {
                let message = truncate_chars(&error.presentable, MAX_TRANSCRIPTION_ERROR_CHARS);
                out.push(EngineMessage::Diagnostic(format!(
                    "[smart actors] transcription failed: {message}"
                )));
                out.push(transcription_error(&task.request_id, &message));
                out.push(EngineMessage::Status(stt_status(
                    STATE_DEGRADED,
                    Some(message.clone()),
                    Some(backend),
                )));
                out.push(command_failure(
                    &task.request_id,
                    CommandErrorCode::TranscriptionFailed,
                    &message,
                ));
                return;
            }
            Ok(text) => text,
        };

        let text = py_strip(&text);
        if text.is_empty() {
            out.push(transcription_error(&task.request_id, "no speech detected"));
            out.push(EngineMessage::Status(stt_status(
                STATE_IDLE,
                Some("no speech detected".to_string()),
                Some(backend),
            )));
            out.push(command_failure(
                &task.request_id,
                CommandErrorCode::EmptyTranscription,
                "no speech detected",
            ));
            return;
        }
        if text.chars().count() > PLAYER_SPEECH_MAX_CHARS {
            let message =
                format!("transcription exceeds the {PLAYER_SPEECH_MAX_CHARS} character limit");
            out.push(transcription_error(&task.request_id, &message));
            out.push(EngineMessage::Status(stt_status(
                STATE_IDLE,
                None,
                Some(backend),
            )));
            out.push(command_failure(
                &task.request_id,
                CommandErrorCode::TextTooLong,
                &message,
            ));
            return;
        }
        if has_unsupported_characters(text) {
            let message = "transcription contains unsupported characters";
            out.push(transcription_error(&task.request_id, message));
            out.push(EngineMessage::Status(stt_status(
                STATE_IDLE,
                None,
                Some(backend),
            )));
            out.push(command_failure(
                &task.request_id,
                CommandErrorCode::InvalidTranscription,
                message,
            ));
            return;
        }

        out.push(EngineMessage::TranscriptionResult {
            request_id: task.request_id.clone(),
            text: Some(text.to_string()),
            error: None,
        });

        // Frozen position: transcription can finish after newer spatial updates
        // have landed. The utterance is applied where it was *spoken*, without
        // rewinding the authoritative player position (speech-python.md risk 7).
        let said = apply_action_at(
            ctx.world,
            ctx.player_id,
            "say",
            &json!({"text": text}),
            Some(task.position_m),
        );
        let line = match said {
            Ok(line) => line,
            Err(error) => {
                out.push(EngineMessage::Status(stt_status(
                    STATE_IDLE,
                    None,
                    Some(backend),
                )));
                let error: CommandError = error.into();
                out.push(command_failure(
                    &task.request_id,
                    error.code,
                    &error.message,
                ));
                return;
            }
        };
        ctx.transcript.push(line.clone());

        // Being heard should be followed by the earliest possible reaction: the
        // nearest LLM listener takes the next turn without waiting out the
        // round-robin or the inter-turn delay.
        let nearest = ctx
            .world
            .characters_within(task.position_m, HEARING_RADIUS_M, Some(ctx.player_id))
            .into_iter()
            .find(|candidate| {
                ctx.world
                    .characters
                    .get(candidate)
                    .is_some_and(|character| character.control() == Control::Llm)
            });
        if let Some(nearest) = nearest {
            ctx.scheduler
                .prioritize_player_reaction(ctx.world, &nearest, now);
        }

        out.push(EngineMessage::Status(stt_status(
            STATE_IDLE,
            None,
            Some(backend),
        )));
        out.push(EngineMessage::CommandResult {
            request_id: task.request_id,
            success: true,
            error_code: None,
            message: truncate_chars(&line, MAX_COMMAND_MESSAGE_CHARS),
        });
    }

    // -------------------------------------------------------------------- voices

    /// `_queue_tts` (`server.py:1958-2009`).
    ///
    /// Returns whether synthesis was *actually accepted*, which is exactly what
    /// the floor needs: only an accepted utterance will ever be presented, so
    /// only an accepted one is worth awaiting (D26/R10). Every refusal path here
    /// returns `false` **and** emits `TtsFailed`, so a full queue paces the cast
    /// on the reading estimate instead of stalling it for the failsafe window.
    pub fn queue_tts(
        &mut self,
        now: f64,
        event: &DomainEvent,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) -> bool {
        let Some(actor_id) = event.actor_id.as_ref() else {
            return false;
        };
        let Some(speaker) = ctx.world.characters.get(actor_id) else {
            return false;
        };
        // Silently not-audio, and not a failure: a mute speaker, a line the
        // player cannot hear, or his own voice coming back at him.
        let (Some(voice_key), Some(text)) = (speaker.voice_key(), event.text.as_deref()) else {
            return false;
        };
        if speaker.control() == Control::Player || !event.recipient_ids.contains(ctx.player_id) {
            return false;
        }
        let selected = ctx.tts_selected;
        if selected == TtsBackendKind::Off {
            return false;
        }

        let event_id = event.speech_event_id();
        let request = TtsRequest {
            event_id: event_id.clone(),
            text: text.to_string(),
            voice_key: voice_key.to_string(),
            // Queue-time capture (test 19): a mode switch while this line is
            // synthesizing must not re-route it.
            kind: selected,
        };

        if !ctx.tts.available(selected) {
            let reason = format!("{} NPC voice backend is unavailable", selected.as_str());
            out.push(EngineMessage::Status(tts_status(
                STATE_UNAVAILABLE,
                None,
                Some(reason.clone()),
                Some(selected),
            )));
            self.tts_failed(now, &event_id, &reason, ctx, out);
            return false;
        }

        match ctx.tts.submit(request) {
            Ok(()) => {
                self.remember_tts_backend(event_id, selected);
                out.push(EngineMessage::Status(tts_status(
                    STATE_SYNTHESIZING,
                    Some(actor_id.clone()),
                    None,
                    Some(selected),
                )));
                true
            }
            Err(error) => {
                let reason = match error {
                    TtsSubmitError::QueueFull => "speech queue is full".to_string(),
                    TtsSubmitError::PathInUse => "speech output path is already in use".to_string(),
                    TtsSubmitError::Unavailable => {
                        format!("{} NPC voice backend is unavailable", selected.as_str())
                    }
                };
                let (state, backend) = match error {
                    // Python reported the queue-full and path-collision rows
                    // without a `backend` field (`server.py:1991, 2004`).
                    TtsSubmitError::QueueFull | TtsSubmitError::PathInUse => (STATE_DEGRADED, None),
                    TtsSubmitError::Unavailable => (STATE_UNAVAILABLE, Some(selected)),
                };
                out.push(EngineMessage::Status(tts_status(
                    state,
                    None,
                    Some(reason.clone()),
                    backend,
                )));
                self.tts_failed(now, &event_id, &reason, ctx, out);
                false
            }
        }
    }

    /// `_poll_tts` (`server.py:1854-1956`), minus the WAV validation that is now
    /// a file fact and lives in cathedral-backends.
    pub fn on_tts(
        &mut self,
        now: f64,
        outcome: TtsOutcome,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        match outcome {
            TtsOutcome::Chunk {
                event_id,
                seq,
                sample_rate,
                samples,
            } => out.push(EngineMessage::TtsChunk {
                event_id,
                chunk_seq: seq,
                sample_rate,
                samples,
            }),
            TtsOutcome::StreamEnd {
                event_id,
                chunk_count,
                first_chunk_ms,
            } => {
                self.forget_tts_backend(&event_id);
                out.push(EngineMessage::TtsStreamEnd {
                    event_id,
                    chunk_count,
                    first_chunk_ms,
                });
                // The whole point of the local streaming path is the first-PCM
                // latency; show it.
                out.push(EngineMessage::Status(tts_status(
                    STATE_IDLE,
                    None,
                    Some(format!("First local PCM in {first_chunk_ms} ms")),
                    Some(TtsBackendKind::Local),
                )));
            }
            TtsOutcome::Done {
                event_id,
                result: Ok(wav_bytes),
            } => {
                let backend = self.forget_tts_backend(&event_id);
                out.push(EngineMessage::TtsReady {
                    event_id,
                    wav_bytes,
                });
                out.push(EngineMessage::Status(tts_status(
                    STATE_IDLE, None, None, backend,
                )));
            }
            TtsOutcome::Done {
                event_id,
                result: Err(error),
            } => {
                let backend = self.forget_tts_backend(&event_id);
                let message = truncate_chars(&error.presentable, MAX_TTS_FAILURE_REASON_CHARS);
                out.push(EngineMessage::Status(tts_status(
                    STATE_DEGRADED,
                    None,
                    Some(message.clone()),
                    backend,
                )));
                self.tts_failed(now, &event_id, &message, ctx, out);
            }
        }
    }

    /// `_send_tts_failed` (`server.py:2011-2019`). The text was already
    /// delivered and the game keeps it on screen for its own reading time, so the
    /// only thing to undo is the awaited floor entry — without which a refused
    /// submission would stall the cast for the whole failsafe window.
    fn tts_failed(
        &mut self,
        now: f64,
        event_id: &SpeechEventId,
        reason: &str,
        ctx: &mut SpeechContext<'_>,
        out: &mut Vec<EngineMessage>,
    ) {
        ctx.floor.release(now, event_id);
        out.push(EngineMessage::TtsFailed {
            event_id: event_id.clone(),
            reason: truncate_chars(reason, MAX_TTS_FAILURE_REASON_CHARS),
        });
    }

    fn remember_tts_backend(&mut self, event_id: SpeechEventId, kind: TtsBackendKind) {
        self.forget_tts_backend(&event_id);
        while self.tts_backends.len() >= MAX_TRACKED_TTS_BACKENDS {
            self.tts_backends.remove(0);
        }
        self.tts_backends.push((event_id, kind));
    }

    fn forget_tts_backend(&mut self, event_id: &SpeechEventId) -> Option<TtsBackendKind> {
        let index = self
            .tts_backends
            .iter()
            .position(|(candidate, _)| candidate == event_id)?;
        Some(self.tts_backends.remove(index).1)
    }

    // ---------------------------------------------------------------- internals

    /// `_degrade_stream` (`server.py:1151-1170`): mark once, report at most once,
    /// and never un-complete a stream that already produced a transcript.
    fn degrade(&mut self, key: &str, reason: DegradeReason, out: &mut Vec<EngineMessage>) {
        let Some(stream) = self.stream_mut(key) else {
            return;
        };
        if stream.phase == StreamPhase::Completed {
            // The utterance already resolved; late noise cannot un-complete it.
            return;
        }
        stream.phase = StreamPhase::Degraded;
        if stream.degrade_reason.is_none() {
            stream.degrade_reason = Some(reason.clone());
        }
        if stream.status_sent {
            return;
        }
        stream.status_sent = true;
        if reason.is_expected() {
            return;
        }
        out.push(EngineMessage::Status(stt_status(
            STATE_DEGRADED,
            Some(format!("streamed audio fell back to batch ({reason})")),
            Some(SttBackendKind::Cloud),
        )));
    }

    fn stream_mut(&mut self, key: &str) -> Option<&mut StreamState> {
        self.streams
            .iter_mut()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, stream)| stream)
    }

    fn take_stream(&mut self, key: &str) -> Option<StreamState> {
        let index = self
            .streams
            .iter()
            .position(|(candidate, _)| candidate == key)?;
        Some(self.streams.remove(index).1)
    }

    fn take_parked(&mut self, key: &str) -> Option<ParkedRecording> {
        let index = self
            .parked
            .iter()
            .position(|(candidate, _)| candidate == key)?;
        Some(self.parked.remove(index).1)
    }

    fn next_job(&mut self) -> TranscriptionJobId {
        self.next_job += 1;
        TranscriptionJobId(self.next_job)
    }

    // ------------------------------------------------------------ timing probe

    fn begin_timing(
        &mut self,
        now: f64,
        basename: &str,
        path: &str,
        audio_seconds: Option<f64>,
        endpoint_at: Option<f64>,
    ) {
        let timing = UtteranceTiming {
            path: path.to_string(),
            endpoint_at: endpoint_at.unwrap_or(now),
            audio_seconds,
            commit_at: None,
            completed_at: None,
        };
        if let Some(index) = self
            .timings
            .iter()
            .position(|(candidate, _)| candidate == basename)
        {
            self.timings[index].1 = timing;
            return;
        }
        self.timings.push((basename.to_string(), timing));
        while self.timings.len() > MAX_UTTERANCE_TIMINGS {
            self.timings.remove(0);
        }
    }

    fn timing_mut(&mut self, basename: &str) -> Option<&mut UtteranceTiming> {
        self.timings
            .iter_mut()
            .find(|(candidate, _)| candidate == basename)
            .map(|(_, timing)| timing)
    }

    /// `_log_utterance_timing` (`server.py:1768-1794`). Exactly one line per
    /// resolved utterance, whatever path it took.
    fn log_timing(&mut self, now: f64, basename: &str, out: &mut Vec<EngineMessage>) {
        let Some(index) = self
            .timings
            .iter()
            .position(|(candidate, _)| candidate == basename)
        else {
            return;
        };
        let (_, timing) = self.timings.remove(index);
        let audio = match timing.audio_seconds {
            Some(seconds) => format!("{seconds:.2}s"),
            None => "?".to_string(),
        };
        let mut segments = vec![format!("audio={audio}"), format!("path={}", timing.path)];
        if let Some(commit_at) = timing.commit_at {
            segments.push(format!(
                "endpoint->commit={}ms",
                elapsed_ms(timing.endpoint_at, commit_at)
            ));
            if let Some(completed_at) = timing.completed_at {
                segments.push(format!(
                    "commit->transcript={}ms",
                    elapsed_ms(commit_at, completed_at)
                ));
            }
        }
        if let Some(completed_at) = timing.completed_at {
            segments.push(format!(
                "transcript->say={}ms",
                elapsed_ms(completed_at, now)
            ));
        }
        segments.push(format!(
            "endpoint->say={}ms",
            elapsed_ms(timing.endpoint_at, now)
        ));
        out.push(EngineMessage::Diagnostic(format!(
            "[smart actors/stt] {basename}: {}",
            segments.join(" ")
        )));
    }
}

// -------------------------------------------------------------------- helpers

/// Hand one recording to a transcription backend, mapping the refusal to the
/// code the player's `command_result` carries.
fn submit_batch(
    ctx: &mut SpeechContext<'_>,
    job: TranscriptionJobId,
    wav_path: PathBuf,
    kind: SttBackendKind,
) -> Result<(), CommandError> {
    ctx.transcription
        .submit_batch(job, wav_path, kind)
        .map_err(|error| match error {
            SttSubmitError::QueueFull => {
                CommandError::new(CommandErrorCode::Overloaded, "transcription queue is full")
            }
            SttSubmitError::Unavailable => CommandError::new(
                CommandErrorCode::SttUnavailable,
                format!("{} speech transcription is unavailable", kind.as_str()),
            ),
        })
}

/// `_safe_basename` (`server.py:152-160`) — the shape half, which is pure. The
/// *resolution* half (does it exist, does it stay inside the runtime directory)
/// is a file fact and belongs to cathedral-backends.
fn check_basename(name: &str) -> Result<(), CommandError> {
    if !crate::ids::is_valid_id(name) {
        return Err(CommandError::new(
            CommandErrorCode::InvalidRequest,
            format!(
                "wav_basename must be a non-empty string of at most {} characters",
                crate::MAX_ID_CHARS
            ),
        ));
    }
    if name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(CommandError::new(
            CommandErrorCode::InvalidPath,
            "WAV path must be a basename inside the runtime directory",
        ));
    }
    if !name.to_lowercase().ends_with(".wav") {
        return Err(CommandError::new(
            CommandErrorCode::InvalidPath,
            "audio basename must end in .wav",
        ));
    }
    Ok(())
}

/// The text a voice may carry (`speech_client.py:92-107`, `server.py:1673-1677`).
///
/// `\n` and `\t` are fine; `\r`, the other C0 controls, DEL and the C1 block are
/// not — they are terminal-escape material, not speech. Lone surrogates cannot
/// exist in a Rust `str`, so Python's `UnicodeEncodeError` clause drops out.
pub fn has_unsupported_characters(text: &str) -> bool {
    text.chars().any(|character| {
        let code = character as u32;
        (code < 0x20 && character != '\n' && character != '\t') || (0x7F..=0x9F).contains(&code)
    })
}

fn elapsed_ms(start: f64, end: f64) -> i64 {
    (((end - start) * 1000.0).round() as i64).max(0)
}

fn stt_status(
    state: &str,
    message: Option<String>,
    backend: Option<SttBackendKind>,
) -> StatusEvent {
    StatusEvent {
        subsystem: Subsystem::Stt,
        state: state.to_string(),
        actor_id: None,
        message,
        backend: backend.map(|kind| kind.as_str().to_string()),
    }
}

fn tts_status(
    state: &str,
    actor_id: Option<ActorId>,
    message: Option<String>,
    backend: Option<TtsBackendKind>,
) -> StatusEvent {
    StatusEvent {
        subsystem: Subsystem::Tts,
        state: state.to_string(),
        actor_id,
        message,
        backend: backend.map(|kind| kind.as_str().to_string()),
    }
}

fn transcription_error(request_id: &str, message: &str) -> EngineMessage {
    EngineMessage::TranscriptionResult {
        request_id: request_id.to_string(),
        text: None,
        error: Some(truncate_chars(message, MAX_TRANSCRIPTION_ERROR_CHARS)),
    }
}

fn command_failure(request_id: &str, code: CommandErrorCode, message: &str) -> EngineMessage {
    EngineMessage::CommandResult {
        request_id: request_id.to_string(),
        success: false,
        error_code: Some(code.as_str().to_string()),
        message: truncate_chars(message, MAX_COMMAND_MESSAGE_CHARS),
    }
}

/// A malformed microphone command has no `request_id` to fail: Python raised a
/// `ProtocolError` that surfaced only as a stderr line. The line stays.
fn diagnostic(command: &str, error: &CommandError) -> EngineMessage {
    EngineMessage::Diagnostic(format!(
        "[smart actors] invalid {command}: {}",
        error.message
    ))
}

/// Python's `message[:N]` — Unicode scalar values, not bytes (D11).
fn truncate_chars(value: &str, limit: usize) -> String {
    match value.char_indices().nth(limit) {
        Some((index, _)) => value[..index].to_string(),
        None => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_control_character_predicate_allows_newline_and_tab_only() {
        assert!(!has_unsupported_characters("hello\nworld\there"));
        assert!(has_unsupported_characters("bell\u{7}"));
        assert!(has_unsupported_characters("carriage\rreturn"));
        assert!(has_unsupported_characters("del\u{7f}"));
        assert!(has_unsupported_characters("c1\u{9f}"));
        assert!(!has_unsupported_characters("café ünïcode 漢字"));
    }

    #[test]
    fn basenames_must_be_bare_wav_names() {
        assert!(check_basename("utterance.wav").is_ok());
        assert!(check_basename("UTTERANCE.WAV").is_ok());
        assert_eq!(
            check_basename("../secret.wav").unwrap_err().code,
            CommandErrorCode::InvalidPath
        );
        assert_eq!(
            check_basename("dir/utterance.wav").unwrap_err().code,
            CommandErrorCode::InvalidPath
        );
        assert_eq!(
            check_basename("utterance.mp3").unwrap_err().code,
            CommandErrorCode::InvalidPath
        );
        assert_eq!(
            check_basename("").unwrap_err().code,
            CommandErrorCode::InvalidRequest
        );
    }

    #[test]
    fn a_chunk_cap_of_256_matches_the_microphone_workers() {
        assert_eq!(STT_STREAM_MAX_CHUNKS, 256);
        // 32 000 base64 characters decode to at most 24 000 bytes = 12 000
        // `i16` samples: the wire cap, in the units the wire now uses.
        assert_eq!(STT_STREAM_MAX_CHUNK_SAMPLES, 12_000);
    }

    #[test]
    fn elapsed_milliseconds_never_go_backwards() {
        assert_eq!(elapsed_ms(1.0, 1.25), 250);
        assert_eq!(elapsed_ms(2.0, 1.0), 0);
    }
}
