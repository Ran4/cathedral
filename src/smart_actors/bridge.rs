//! Owned, non-blocking JSON-lines bridge to the Python authority process.
//!
//! Bevy only sends bounded commands and polls bounded events. Process startup,
//! blocking pipes, JSON I/O, generated-WAV reads, and cleanup all happen on
//! dedicated worker threads.

use std::{
    fs,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use bevy::prelude::Resource;
use crossbeam_channel::{Receiver, SendTimeoutError, Sender, TryRecvError, TrySendError, bounded};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::model::{ActorId, ItemId, Position};

pub const PROTOCOL_VERSION: u32 = 1;
/// Fixed provider format for streamed microphone audio; the capture worker
/// resamples every device rate down to this before chunking.
pub const STREAM_SAMPLE_RATE: u32 = 24_000;
const COMMAND_QUEUE_CAPACITY: usize = 128;
const EVENT_QUEUE_CAPACITY: usize = 256;
const MAX_PROTOCOL_LINE_BYTES: usize = 1_000_000;
const MAX_WAV_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PCM_CHUNK_BYTES: usize = 256 * 1024;
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);
const CHANNEL_POLL_INTERVAL: Duration = Duration::from_millis(20);
const WORKER_JOIN_GRACE: Duration = Duration::from_millis(250);
const SUBTHREAD_JOIN_GRACE: Duration = Duration::from_millis(150);

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct BridgeLaunchConfig {
    pub uv_binary: String,
    pub server_script: PathBuf,
    pub fake_backend: bool,
    pub tts_backend: String,
    /// Sound-percept settings forwarded to the sidecar's environment; the
    /// witness cone and rate limit are enforced Python-side.
    pub sounds_enabled: bool,
    pub view_cone_degrees: f32,
    pub min_seconds_between_player_sounds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionBackend {
    Cloud,
    Local,
}

impl TranscriptionBackend {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Local => "local",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsBackend {
    Cloud,
    Local,
    Off,
}

impl TtsBackend {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Local => "local",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BridgeCommand {
    Hello {
        position_m: Position,
        spatial_seq: u64,
    },
    SpatialUpdate {
        position_m: Position,
        spatial_seq: u64,
        /// Player compass bearing in radians (yaw 0 faces -Z). The sidecar
        /// runs the identical witness cone test against the player.
        facing_yaw: f32,
    },
    PlayerRecording {
        request_id: String,
        wav_basename: String,
        stt_backend: TranscriptionBackend,
        position_m: Position,
        spatial_seq: u64,
    },
    /// Start of a streamed copy of the utterance being recorded to
    /// `wav_basename`; chunks follow while the player is still speaking.
    PlayerAudioBegin {
        wav_basename: String,
    },
    PlayerAudioChunk {
        wav_basename: String,
        seq: u32,
        samples: Arc<[i16]>,
    },
    PlayerAudioEnd {
        wav_basename: String,
        chunk_count: u32,
        silent: bool,
    },
    PlayerAudioAbort {
        wav_basename: String,
    },
    DebugPlayerSay {
        request_id: String,
        text: String,
        target_id: Option<ActorId>,
        position_m: Position,
        spatial_seq: u64,
    },
    PlayerOffer {
        request_id: String,
        target_id: ActorId,
        item_id: ItemId,
        position_m: Position,
        spatial_seq: u64,
    },
    PlayerAccept {
        request_id: String,
        item_id: ItemId,
        position_m: Position,
        spatial_seq: u64,
    },
    PlayerDecline {
        request_id: String,
        item_id: ItemId,
        position_m: Position,
        spatial_seq: u64,
    },
    PlayerRetract {
        request_id: String,
        item_id: ItemId,
    },
    /// Fire-and-forget deliberate player noise (the F key). No request_id and
    /// no command_result: there is no failure the player can act on, and
    /// rate-limited sounds are dropped silently at the sidecar.
    PlayerSound {
        sound_id: String,
    },
    /// CATHEDRAL_DRIVE stand-in for world sounds the sim cannot cause yet
    /// (nothing rings the town bell: no clock, no calendar).
    DebugSound {
        sound_id: String,
        position_m: Position,
    },
    AudioConsumed {
        speech_event_id: String,
        wav_basename: String,
    },
    /// Fire-and-forget notice that a speech event's audio presentation reached
    /// a terminal state (played, skipped, dropped, failed, or cut off). Python
    /// frees the conversation floor on it; its failsafe covers a lost send.
    SpeechPresented {
        speech_event_id: String,
    },
    SetTtsBackend {
        request_id: String,
        backend: TtsBackend,
    },
    ResyncRequest {
        last_world_revision: u64,
    },
    Shutdown,
}

impl BridgeCommand {
    fn wire_parts(&self) -> (&'static str, Value) {
        match self {
            Self::Hello {
                position_m,
                spatial_seq,
            } => (
                "hello",
                json!({
                    "supported_protocol_version": PROTOCOL_VERSION,
                    "player_id": "player",
                    "position_m": position_m,
                    "spatial_seq": spatial_seq,
                }),
            ),
            Self::SpatialUpdate {
                position_m,
                spatial_seq,
                facing_yaw,
            } => (
                "spatial_update",
                json!({
                    "spatial_seq": spatial_seq,
                    "updates": [{
                        "actor_id": "player",
                        "position_m": position_m,
                        "facing_yaw": facing_yaw,
                    }],
                }),
            ),
            Self::PlayerRecording {
                request_id,
                wav_basename,
                stt_backend,
                position_m,
                spatial_seq,
            } => (
                "player_recording",
                json!({
                    "request_id": request_id,
                    "wav_basename": wav_basename,
                    "stt_backend": stt_backend.wire_name(),
                    // Microphone speech is always open. Retain the explicit
                    // null in protocol v1 so older peers fail closed if they
                    // still expect the settled field.
                    "target_id": null,
                    "position_m": position_m,
                    "spatial_seq": spatial_seq,
                }),
            ),
            Self::PlayerAudioBegin { wav_basename } => (
                "player_audio_begin",
                json!({
                    "wav_basename": wav_basename,
                    "sample_rate": STREAM_SAMPLE_RATE,
                    "format": "pcm_s16le",
                }),
            ),
            Self::PlayerAudioChunk {
                wav_basename,
                seq,
                samples,
            } => (
                "player_audio_chunk",
                json!({
                    "wav_basename": wav_basename,
                    "seq": seq,
                    // Base64 work stays on the writer thread with the rest of
                    // the wire encoding; producers only move sample buffers.
                    "pcm_s16le_base64": encode_pcm_chunk(samples),
                }),
            ),
            Self::PlayerAudioEnd {
                wav_basename,
                chunk_count,
                silent,
            } => (
                "player_audio_end",
                json!({
                    "wav_basename": wav_basename,
                    "chunk_count": chunk_count,
                    "silent": silent,
                }),
            ),
            Self::PlayerAudioAbort { wav_basename } => (
                "player_audio_abort",
                json!({"wav_basename": wav_basename}),
            ),
            Self::DebugPlayerSay {
                request_id,
                text,
                target_id,
                position_m,
                spatial_seq,
            } => (
                "debug_player_say",
                json!({
                    "request_id": request_id,
                    "text": text,
                    "target_id": target_id,
                    "position_m": position_m,
                    "spatial_seq": spatial_seq,
                }),
            ),
            Self::PlayerOffer {
                request_id,
                target_id,
                item_id,
                position_m,
                spatial_seq,
            } => (
                "player_offer",
                json!({
                    "request_id": request_id,
                    "target_id": target_id,
                    "item_id": item_id,
                    "position_m": position_m,
                    "spatial_seq": spatial_seq,
                }),
            ),
            Self::PlayerAccept {
                request_id,
                item_id,
                position_m,
                spatial_seq,
            } => (
                "player_accept",
                json!({
                    "request_id": request_id,
                    "item_id": item_id,
                    "position_m": position_m,
                    "spatial_seq": spatial_seq,
                }),
            ),
            Self::PlayerDecline {
                request_id,
                item_id,
                position_m,
                spatial_seq,
            } => (
                "player_decline",
                json!({
                    "request_id": request_id,
                    "item_id": item_id,
                    "position_m": position_m,
                    "spatial_seq": spatial_seq,
                }),
            ),
            Self::PlayerRetract {
                request_id,
                item_id,
            } => (
                "player_retract",
                json!({"request_id": request_id, "item_id": item_id}),
            ),
            Self::PlayerSound { sound_id } => {
                ("player_sound", json!({"sound_id": sound_id}))
            }
            Self::DebugSound {
                sound_id,
                position_m,
            } => (
                "debug_sound",
                json!({"sound_id": sound_id, "position_m": position_m}),
            ),
            Self::AudioConsumed {
                speech_event_id,
                wav_basename,
            } => (
                "audio_consumed",
                json!({
                    "speech_event_id": speech_event_id,
                    "wav_basename": wav_basename,
                }),
            ),
            Self::SpeechPresented { speech_event_id } => (
                "speech_presented",
                json!({"speech_event_id": speech_event_id}),
            ),
            Self::SetTtsBackend {
                request_id,
                backend,
            } => (
                "set_tts_backend",
                json!({
                    "request_id": request_id,
                    "backend": backend.wire_name(),
                }),
            ),
            Self::ResyncRequest {
                last_world_revision,
            } => (
                "resync_request",
                json!({"last_world_revision": last_world_revision}),
            ),
            Self::Shutdown => ("shutdown", json!({})),
        }
    }

    fn is_redundant_spatial(&self) -> bool {
        matches!(self, Self::SpatialUpdate { .. })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServerEnvelope {
    pub protocol_version: u32,
    pub session_id: String,
    pub message_id: String,
    pub event_seq: u64,
    #[serde(rename = "type")]
    pub message_type: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub enum BridgeEvent {
    ProcessStarted,
    Message(ServerEnvelope),
    TtsAudio {
        speech_event_id: String,
        wav_bytes: std::sync::Arc<[u8]>,
    },
    TtsPcmChunk {
        speech_event_id: String,
        chunk_seq: u32,
        sample_rate: u32,
        samples: Arc<[i16]>,
    },
    Degraded(String),
    Disconnected(String),
}

/// Non-blocking command endpoint plus immutable per-process identity.
#[derive(Resource)]
pub struct BridgeHandle {
    commands: Sender<BridgeCommand>,
    session_id: String,
    runtime_dir: PathBuf,
}

impl BridgeHandle {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    /// Clone of the bounded command sender for non-ECS producers. The
    /// microphone worker streams utterance audio through the same queue so
    /// its chunks and Bevy's later `player_recording` stay strictly ordered.
    pub fn command_sender(&self) -> Sender<BridgeCommand> {
        self.commands.clone()
    }

    /// Enqueue without ever waiting on the protocol worker.
    pub fn try_send(&self, command: BridgeCommand) -> Result<(), String> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(command) if command.is_redundant_spatial() => {
                    "spatial update coalesced because the bridge is busy".into()
                }
                TrySendError::Full(_) => "smart-actor command queue is full".into(),
                TrySendError::Disconnected(_) => "smart-actor process is offline".into(),
            })
    }
}

/// Polled once per frame; receiving never waits.
#[derive(Resource)]
pub struct BridgeInbox {
    events: Receiver<BridgeEvent>,
    disconnect_reported: AtomicBool,
}

impl BridgeInbox {
    pub fn try_recv(&self) -> Option<BridgeEvent> {
        match self.events.try_recv() {
            Ok(event) => {
                if matches!(event, BridgeEvent::Disconnected(_)) {
                    self.disconnect_reported.store(true, Ordering::Release);
                }
                Some(event)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                if self.disconnect_reported.swap(true, Ordering::AcqRel) {
                    None
                } else {
                    Some(BridgeEvent::Disconnected(
                        "smart-actor bridge worker stopped unexpectedly".into(),
                    ))
                }
            }
        }
    }
}

/// Owns the child-worker join. Its drop requests cancellation out of band and
/// waits only briefly; a stuck OS pipe can never hold the Bevy thread hostage.
#[derive(Resource)]
pub struct BridgeWorkerGuard {
    commands: Sender<BridgeCommand>,
    cancel: Sender<()>,
    cancelled: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for BridgeWorkerGuard {
    fn drop(&mut self) {
        // Give a healthy worker a chance to flush the protocol shutdown. The
        // separate cancellation path below remains available even when this
        // bounded queue is full or disconnected.
        let _ = self.commands.try_send(BridgeCommand::Shutdown);
        self.cancelled.store(true, Ordering::Release);
        let _ = self.cancel.try_send(());
        // The worker never holds this lock while writing stdin, so killing the
        // child here also releases a worker stuck inside an OS pipe write.
        if let Ok(mut child) = self.child.try_lock()
            && let Some(child) = child.as_mut()
        {
            let _ = child.kill();
        }
        if let Some(worker) = self.worker.take() {
            join_thread_until(worker, Instant::now() + WORKER_JOIN_GRACE);
        }
    }
}

pub fn spawn_sidecar(config: BridgeLaunchConfig) -> (BridgeHandle, BridgeInbox, BridgeWorkerGuard) {
    let session_id = new_session_id();
    let runtime_dir = std::env::temp_dir().join(format!("cathedral-smart-actors-{session_id}"));
    let (commands_tx, commands_rx) = bounded(COMMAND_QUEUE_CAPACITY);
    let (events_tx, events_rx) = bounded(EVENT_QUEUE_CAPACITY);
    let (cancel_tx, cancel_rx) = bounded(1);
    let cancelled = Arc::new(AtomicBool::new(false));
    let child = Arc::new(Mutex::new(None));
    let worker_session = session_id.clone();
    let worker_runtime = runtime_dir.clone();
    let worker_cancelled = cancelled.clone();
    let worker_child = child.clone();
    let worker_events = events_tx.clone();
    let worker = match thread::Builder::new()
        .name("smart-actor-bridge".into())
        .spawn(move || {
            run_bridge_worker(
                config,
                worker_session,
                worker_runtime,
                commands_rx,
                cancel_rx,
                worker_cancelled,
                worker_child,
                worker_events,
            )
        }) {
        Ok(worker) => Some(worker),
        Err(error) => {
            let _ = events_tx.try_send(BridgeEvent::Disconnected(format!(
                "could not start smart-actor bridge worker: {error}"
            )));
            None
        }
    };

    (
        BridgeHandle {
            commands: commands_tx.clone(),
            session_id,
            runtime_dir,
        },
        BridgeInbox {
            events: events_rx,
            disconnect_reported: AtomicBool::new(false),
        },
        BridgeWorkerGuard {
            commands: commands_tx,
            cancel: cancel_tx,
            cancelled,
            child,
            worker,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn run_bridge_worker(
    config: BridgeLaunchConfig,
    session_id: String,
    runtime_dir: PathBuf,
    commands: Receiver<BridgeCommand>,
    cancel: Receiver<()>,
    cancelled: Arc<AtomicBool>,
    child_slot: Arc<Mutex<Option<Child>>>,
    events: Sender<BridgeEvent>,
) {
    let events = WorkerEventSink::new(events, cancelled.clone());
    if let Err(error) = create_private_runtime_dir(&runtime_dir) {
        events.send(BridgeEvent::Disconnected(format!(
            "could not create audio session directory: {error}"
        )));
        return;
    }

    let mut process = Command::new(&config.uv_binary);
    // The sidecar archives every LLM prompt/answer under the session's
    // `prompts/` directory; absent (init failed) it simply does not archive.
    if let Some(session) = crate::session_log::paths() {
        process.env("CATHEDRAL_SESSION_DIR", &session.root);
    }
    process.env("SMART_ACTORS_UV_BINARY", &config.uv_binary);
    process.env("SMART_ACTORS_TTS_BACKEND", &config.tts_backend);
    process.env(
        "SMART_ACTORS_SOUNDS_ENABLED",
        if config.sounds_enabled { "1" } else { "0" },
    );
    process.env(
        "SMART_ACTORS_VIEW_CONE_DEGREES",
        config.view_cone_degrees.to_string(),
    );
    process.env(
        "SMART_ACTORS_SOUND_COOLDOWN_SECONDS",
        config.min_seconds_between_player_sounds.to_string(),
    );
    process.arg("run");
    if config.fake_backend {
        // Fake mode has no SDK imports or provider calls. Avoid resolving the
        // production PEP-723 dependencies so offline CI can exercise the real
        // persistent process and protocol with only uv + Python installed.
        process
            .arg("--offline")
            .arg("--no-project")
            .arg("python")
            .arg(&config.server_script);
        process.env("UV_CACHE_DIR", runtime_dir.join("uv-cache"));
    } else {
        process.arg("--script").arg(&config.server_script);
    }
    process
        .arg("--stdio")
        .arg("--runtime-dir")
        .arg(&runtime_dir);
    if config.fake_backend {
        process.arg("--fake");
    }
    let child = process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            events.send(BridgeEvent::Disconnected(format!(
                "could not launch Python actors: {error}"
            )));
            let _ = fs::remove_dir_all(&runtime_dir);
            return;
        }
    };

    let Some(stdin) = child.stdin.take() else {
        fail_child(&mut child, &events, "Python stdin was unavailable");
        let _ = fs::remove_dir_all(&runtime_dir);
        return;
    };
    let Some(stdout) = child.stdout.take() else {
        fail_child(&mut child, &events, "Python stdout was unavailable");
        let _ = fs::remove_dir_all(&runtime_dir);
        return;
    };
    let stderr = child.stderr.take();

    match child_slot.lock() {
        Ok(mut slot) => *slot = Some(child),
        Err(_) => {
            fail_child(&mut child, &events, "Python child lock was poisoned");
            let _ = fs::remove_dir_all(&runtime_dir);
            return;
        }
    }

    // Announce startup before the reader can report a very early protocol
    // failure. This keeps lifecycle events deterministic across the two worker
    // threads.
    if !events.send(BridgeEvent::ProcessStarted) {
        terminate_shared_child(&child_slot, false);
        let _ = fs::remove_dir_all(&runtime_dir);
        return;
    }

    let (audio_acks_tx, audio_acks_rx) = bounded(32);
    let stdout_events = events.clone();
    let stdout_session = session_id.clone();
    let stdout_runtime = runtime_dir.clone();
    let stdout_cancelled = cancelled.clone();
    let stdout_thread = match thread::Builder::new()
        .name("smart-actor-json-reader".into())
        .spawn(move || {
            read_protocol_output(
                stdout,
                &stdout_session,
                &stdout_runtime,
                &audio_acks_tx,
                &stdout_events,
                &stdout_cancelled,
            )
        }) {
        Ok(thread) => Some(thread),
        Err(error) => {
            events.send(BridgeEvent::Disconnected(format!(
                "could not start Python protocol reader: {error}"
            )));
            cancelled.store(true, Ordering::Release);
            None
        }
    };
    let stderr_thread = stderr.and_then(|stderr| {
        thread::Builder::new()
            .name("smart-actor-log-reader".into())
            .spawn(move || forward_stderr(stderr))
            .ok()
    });

    let mut writer = BufWriter::new(stdin);
    let mut message_number = 0_u64;

    loop {
        if cancelled.load(Ordering::Acquire) || cancel.try_recv().is_ok() {
            cancelled.store(true, Ordering::Release);
            break;
        }

        // Audio acknowledgements have their own small queue so the blocking
        // stdout reader never waits for room in the player-command queue.
        while let Ok(ack) = audio_acks_rx.try_recv() {
            message_number = message_number.saturating_add(1);
            if let Err(error) = write_command(&mut writer, &session_id, message_number, &ack) {
                events.send(BridgeEvent::Disconnected(format!(
                    "Python protocol write failed: {error}"
                )));
                cancelled.store(true, Ordering::Release);
                break;
            }
        }
        if cancelled.load(Ordering::Acquire) {
            break;
        }

        match commands.recv_timeout(CHANNEL_POLL_INTERVAL) {
            Ok(command) => {
                message_number = message_number.saturating_add(1);
                if let Err(error) =
                    write_command(&mut writer, &session_id, message_number, &command)
                {
                    events.send(BridgeEvent::Disconnected(format!(
                        "Python protocol write failed: {error}"
                    )));
                    break;
                }
                if matches!(command, BridgeCommand::Shutdown) {
                    break;
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
        }

        match try_wait_shared_child(&child_slot) {
            Ok(Some(status)) => {
                events.send(BridgeEvent::Disconnected(format!(
                    "Python actors exited with {status}"
                )));
                break;
            }
            Ok(None) => {}
            Err(error) => {
                events.send(BridgeEvent::Disconnected(format!(
                    "could not monitor Python actors: {error}"
                )));
                break;
            }
        }
    }

    cancelled.store(true, Ordering::Release);
    drop(writer);
    terminate_shared_child(&child_slot, true);
    if let Some(thread) = stdout_thread {
        join_thread_until(thread, Instant::now() + SUBTHREAD_JOIN_GRACE);
    }
    if let Some(thread) = stderr_thread {
        join_thread_until(thread, Instant::now() + SUBTHREAD_JOIN_GRACE);
    }
    let _ = fs::remove_dir_all(&runtime_dir);
}

fn write_command(
    writer: &mut impl Write,
    session_id: &str,
    message_number: u64,
    command: &BridgeCommand,
) -> Result<(), std::io::Error> {
    let (message_type, payload) = command.wire_parts();
    let envelope = json!({
        "protocol_version": PROTOCOL_VERSION,
        "session_id": session_id,
        "message_id": format!("rust-{message_number}"),
        "type": message_type,
        "payload": payload,
    });
    serde_json::to_writer(&mut *writer, &envelope).map_err(std::io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn read_protocol_output(
    stdout: impl Read,
    session_id: &str,
    runtime_dir: &Path,
    audio_acks: &Sender<BridgeCommand>,
    events: &WorkerEventSink,
    cancelled: &Arc<AtomicBool>,
) {
    let mut reader = BufReader::new(stdout);
    let mut line = Vec::new();
    loop {
        match read_bounded_line(&mut reader, &mut line, MAX_PROTOCOL_LINE_BYTES) {
            Ok(BoundedLine::Eof) => {
                if !cancelled.load(Ordering::Acquire) {
                    events.send(BridgeEvent::Disconnected(
                        "Python protocol output closed unexpectedly".into(),
                    ));
                    cancelled.store(true, Ordering::Release);
                }
                return;
            }
            Ok(BoundedLine::TooLong) => {
                if !events.send(BridgeEvent::Degraded(
                    "Python protocol line exceeded the size limit".into(),
                )) {
                    return;
                }
                continue;
            }
            Ok(BoundedLine::Line) => {}
            Err(error) => {
                events.send(BridgeEvent::Disconnected(format!(
                    "Python protocol read failed: {error}"
                )));
                cancelled.store(true, Ordering::Release);
                return;
            }
        }
        let line = match std::str::from_utf8(&line) {
            Ok(line) => line,
            Err(error) => {
                events.send(BridgeEvent::Disconnected(format!(
                    "Python protocol output was not UTF-8: {error}"
                )));
                cancelled.store(true, Ordering::Release);
                return;
            }
        };
        let envelope: ServerEnvelope = match serde_json::from_str(line.trim_end()) {
            Ok(envelope) => envelope,
            Err(error) => {
                if !events.send(BridgeEvent::Degraded(format!(
                    "malformed Python protocol JSON: {error}"
                ))) {
                    return;
                }
                continue;
            }
        };
        if envelope.protocol_version != PROTOCOL_VERSION {
            events.send(BridgeEvent::Disconnected(format!(
                "unsupported actor protocol version {}",
                envelope.protocol_version
            )));
            cancelled.store(true, Ordering::Release);
            return;
        }
        if envelope.session_id != session_id {
            if !events.send(BridgeEvent::Degraded(
                "discarded a message from an old actor session".into(),
            )) {
                return;
            }
            continue;
        }
        if !valid_wire_id(&envelope.message_id) || !valid_message_type(&envelope.message_type) {
            if !events.send(BridgeEvent::Degraded(
                "discarded a protocol message with an invalid ID".into(),
            )) {
                return;
            }
            continue;
        }

        let tts = (envelope.message_type == "tts_ready")
            .then(|| serde_json::from_value::<TtsReadyWire>(envelope.payload.clone()))
            .transpose();
        let tts_chunk = (envelope.message_type == "tts_chunk")
            .then(|| serde_json::from_value::<TtsChunkWire>(envelope.payload.clone()))
            .transpose();
        if !events.send(BridgeEvent::Message(envelope)) {
            return;
        }

        match tts {
            Ok(Some(tts)) if valid_wire_id(&tts.speech_event_id) => {
                match read_session_wav(runtime_dir, &tts.wav_basename) {
                    Ok(bytes) => {
                        if !events.send(BridgeEvent::TtsAudio {
                            speech_event_id: tts.speech_event_id.clone(),
                            wav_bytes: bytes.into(),
                        }) {
                            return;
                        }
                        if let Err(error) = audio_acks.try_send(BridgeCommand::AudioConsumed {
                            speech_event_id: tts.speech_event_id,
                            wav_basename: tts.wav_basename,
                        }) {
                            let detail = match error {
                                TrySendError::Full(_) => "audio acknowledgement queue is full",
                                TrySendError::Disconnected(_) => {
                                    "audio acknowledgement worker is offline"
                                }
                            };
                            if !events.send(BridgeEvent::Degraded(detail.into())) {
                                return;
                            }
                        }
                    }
                    Err(error) => {
                        if !events.send(BridgeEvent::Degraded(format!(
                            "could not copy generated speech: {error}"
                        ))) {
                            return;
                        }
                    }
                }
            }
            Ok(Some(_)) => {
                if !events.send(BridgeEvent::Degraded(
                    "invalid speech event ID in tts_ready".into(),
                )) {
                    return;
                }
            }
            Ok(None) => {}
            Err(error) => {
                if !events.send(BridgeEvent::Degraded(format!(
                    "invalid tts_ready payload: {error}"
                ))) {
                    return;
                }
            }
        }
        match tts_chunk {
            Ok(Some(chunk)) if valid_wire_id(&chunk.speech_event_id) => {
                match decode_pcm_chunk(&chunk) {
                    Ok(samples) => {
                        if !events.send(BridgeEvent::TtsPcmChunk {
                            speech_event_id: chunk.speech_event_id,
                            chunk_seq: chunk.chunk_seq,
                            sample_rate: chunk.sample_rate,
                            samples: samples.into(),
                        }) {
                            return;
                        }
                    }
                    Err(error) => {
                        if !events.send(BridgeEvent::Degraded(format!(
                            "invalid NPC PCM chunk: {error}"
                        ))) {
                            return;
                        }
                    }
                }
            }
            Ok(Some(_)) => {
                if !events.send(BridgeEvent::Degraded(
                    "invalid speech event ID in tts_chunk".into(),
                )) {
                    return;
                }
            }
            Ok(None) => {}
            Err(error) => {
                if !events.send(BridgeEvent::Degraded(format!(
                    "invalid tts_chunk payload: {error}"
                ))) {
                    return;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundedLine {
    Eof,
    Line,
    TooLong,
}

/// Reads and, when necessary, discards exactly one line without allowing the
/// allocation to grow beyond `maximum_bytes`.
fn read_bounded_line(
    reader: &mut impl BufRead,
    output: &mut Vec<u8>,
    maximum_bytes: usize,
) -> Result<BoundedLine, std::io::Error> {
    output.clear();
    let mut too_long = false;
    let mut saw_data = false;

    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(if !saw_data {
                BoundedLine::Eof
            } else if too_long {
                BoundedLine::TooLong
            } else {
                BoundedLine::Line
            });
        }

        saw_data = true;
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |index| index + 1);
        let content_len = newline.unwrap_or(available.len());
        if !too_long {
            let remaining = maximum_bytes.saturating_sub(output.len());
            if content_len <= remaining {
                output.extend_from_slice(&available[..content_len]);
            } else {
                too_long = true;
            }
        }
        reader.consume(consumed);

        if newline.is_some() {
            return Ok(if too_long {
                BoundedLine::TooLong
            } else {
                BoundedLine::Line
            });
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TtsReadyWire {
    speech_event_id: String,
    wav_basename: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TtsChunkWire {
    speech_event_id: String,
    chunk_seq: u32,
    sample_rate: u32,
    channels: u8,
    pcm_s16le_base64: String,
}

fn encode_pcm_chunk(samples: &[i16]) -> String {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    BASE64.encode(bytes)
}

fn decode_pcm_chunk(chunk: &TtsChunkWire) -> Result<Vec<i16>, String> {
    if chunk.channels != 1 || !(8_000..=48_000).contains(&chunk.sample_rate) {
        return Err("unsupported PCM format".into());
    }
    if chunk.pcm_s16le_base64.len() > MAX_PCM_CHUNK_BYTES * 2 {
        return Err("encoded PCM chunk is oversized".into());
    }
    let bytes = BASE64
        .decode(&chunk.pcm_s16le_base64)
        .map_err(|_| "PCM chunk is not valid base64".to_string())?;
    if bytes.is_empty() || bytes.len() > MAX_PCM_CHUNK_BYTES || bytes.len() % 2 != 0 {
        return Err("PCM chunk has an invalid byte length".into());
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
        .collect())
}

fn read_session_wav(runtime_dir: &Path, basename: &str) -> Result<Vec<u8>, String> {
    let path = safe_session_path(runtime_dir, basename)
        .ok_or_else(|| "unsafe WAV basename".to_string())?;
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("WAV does not exist: {error}"))?;
    let canonical_runtime = runtime_dir
        .canonicalize()
        .map_err(|error| format!("session directory is unavailable: {error}"))?;
    if canonical.parent() != Some(canonical_runtime.as_path()) {
        return Err("WAV escaped the session directory".into());
    }
    let file =
        fs::File::open(&canonical).map_err(|error| format!("could not open WAV: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("could not inspect WAV: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_WAV_BYTES {
        return Err("WAV is not a regular file or exceeds the size limit".into());
    }
    // Metadata is only advisory because the child can still mutate a file.
    // Take one extra byte so the allocation itself remains bounded and a file
    // that grows after the metadata check is rejected deterministically.
    let bytes = read_with_limit(file, MAX_WAV_BYTES)?;
    validate_wav_bytes(&bytes)?;
    Ok(bytes)
}

fn read_with_limit(reader: impl Read, maximum_bytes: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(maximum_bytes.min(64 * 1024) as usize);
    reader
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read WAV: {error}"))?;
    if bytes.len() as u64 > maximum_bytes {
        return Err("WAV exceeds the size limit while being read".into());
    }
    Ok(bytes)
}

fn validate_wav_bytes(bytes: &[u8]) -> Result<(), String> {
    let reader = hound::WavReader::new(std::io::Cursor::new(bytes))
        .map_err(|error| format!("invalid WAV header: {error}"))?;
    let spec = reader.spec();
    if reader.duration() == 0
        || spec.channels == 0
        || spec.channels > 8
        || !(8_000..=192_000).contains(&spec.sample_rate)
    {
        return Err("generated WAV has unsupported audio properties".into());
    }
    Ok(())
}

fn safe_session_path(runtime_dir: &Path, basename: &str) -> Option<PathBuf> {
    let path = Path::new(basename);
    if basename.is_empty()
        || basename.len() > 128
        || path.extension().and_then(|value| value.to_str()) != Some("wav")
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return None;
    }
    Some(runtime_dir.join(path))
}

#[derive(Clone)]
struct WorkerEventSink {
    events: Sender<BridgeEvent>,
    cancelled: Arc<AtomicBool>,
}

impl WorkerEventSink {
    fn new(events: Sender<BridgeEvent>, cancelled: Arc<AtomicBool>) -> Self {
        Self { events, cancelled }
    }

    /// Applies bounded backpressure without dropping protocol events. Only
    /// application teardown or a dropped inbox can abandon an undelivered
    /// event, and neither path blocks a Bevy system.
    fn send(&self, mut event: BridgeEvent) -> bool {
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return false;
            }
            match self.events.send_timeout(event, CHANNEL_POLL_INTERVAL) {
                Ok(()) => return true,
                Err(SendTimeoutError::Timeout(returned)) => event = returned,
                Err(SendTimeoutError::Disconnected(_)) => {
                    self.cancelled.store(true, Ordering::Release);
                    return false;
                }
            }
        }
    }
}

fn forward_stderr(stderr: impl std::io::Read) {
    for line in BufReader::new(stderr).lines() {
        match line {
            Ok(line) => {
                let line = truncate(&line, 2_000);
                eprintln!("[smart actors/python] {line}");
                crate::session_log::log_line("python", "INFO", line);
            }
            Err(error) => {
                let message = format!("stderr read failed: {error}");
                eprintln!("[smart actors/python] {message}");
                crate::session_log::log_line("python", "ERROR", &message);
                return;
            }
        }
    }
}

fn truncate(value: &str, maximum_bytes: usize) -> &str {
    if value.len() <= maximum_bytes {
        return value;
    }
    let mut boundary = maximum_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &value[..boundary]
}

fn fail_child(child: &mut Child, events: &WorkerEventSink, message: &str) {
    events.send(BridgeEvent::Disconnected(message.into()));
    let _ = child.kill();
    let _ = child.wait();
}

fn stop_child(child: &mut Child) {
    let deadline = std::time::Instant::now() + SHUTDOWN_GRACE;
    while std::time::Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn try_wait_shared_child(
    child: &Arc<Mutex<Option<Child>>>,
) -> Result<Option<std::process::ExitStatus>, std::io::Error> {
    let mut child = child
        .lock()
        .map_err(|_| std::io::Error::other("Python child lock was poisoned"))?;
    child
        .as_mut()
        .ok_or_else(|| std::io::Error::other("Python child was unavailable"))?
        .try_wait()
}

fn terminate_shared_child(child: &Arc<Mutex<Option<Child>>>, graceful: bool) {
    let child = child.lock().ok().and_then(|mut child| child.take());
    if let Some(mut child) = child {
        if graceful {
            stop_child(&mut child);
        } else {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// `JoinHandle::join` has no timeout. Polling `is_finished` lets resource drop
/// stay bounded; dropping an unfinished handle detaches the worker, whose
/// cancellation flag still drives child cleanup off the Bevy thread.
fn join_thread_until(thread: JoinHandle<()>, deadline: Instant) {
    while !thread.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    if thread.is_finished() {
        let _ = thread.join();
    }
}

fn create_private_runtime_dir(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)?;
    }
    #[cfg(not(unix))]
    fs::create_dir(path)?;
    Ok(())
}

fn new_session_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}-{:x}", std::process::id(), timestamp, counter)
}

fn valid_wire_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn valid_message_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smart_actors::model::WorldSnapshot;
    use std::io::Cursor;

    fn position() -> Position {
        Position::new(1.0, 2.0, 3.0).unwrap()
    }

    #[test]
    fn pcm_chunk_decoding_is_strict_and_little_endian() {
        let chunk = TtsChunkWire {
            speech_event_id: "speech-1".into(),
            chunk_seq: 0,
            sample_rate: 24_000,
            channels: 1,
            pcm_s16le_base64: BASE64.encode([0x34, 0x12, 0x00, 0x80]),
        };
        assert_eq!(decode_pcm_chunk(&chunk).unwrap(), [0x1234, i16::MIN]);

        let invalid = TtsChunkWire {
            pcm_s16le_base64: BASE64.encode([0_u8]),
            ..chunk
        };
        assert!(decode_pcm_chunk(&invalid).is_err());
    }

    #[test]
    fn recording_fixture_can_only_encode_open_speech() {
        let command = BridgeCommand::PlayerRecording {
            request_id: "request-1".into(),
            wav_basename: "recording-1.wav".into(),
            stt_backend: TranscriptionBackend::Local,
            position_m: position(),
            spatial_seq: 9,
        };
        let mut bytes = Vec::new();
        write_command(&mut bytes, "session", 2, &command).unwrap();
        let line = String::from_utf8(bytes).unwrap();
        assert!(line.ends_with('\n'));
        let value: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["type"], "player_recording");
        assert!(value["payload"]["target_id"].is_null());
        assert_eq!(value["payload"]["stt_backend"], "local");
    }

    #[test]
    fn player_audio_wire_shapes_are_strict_and_little_endian() {
        let samples: Arc<[i16]> = vec![0x1234_i16, -1].into();
        let (message_type, payload) = BridgeCommand::PlayerAudioChunk {
            wav_basename: "player-recording-3.wav".into(),
            seq: 7,
            samples,
        }
        .wire_parts();
        assert_eq!(message_type, "player_audio_chunk");
        assert_eq!(payload["wav_basename"], "player-recording-3.wav");
        assert_eq!(payload["seq"], 7);
        assert_eq!(
            payload["pcm_s16le_base64"],
            Value::String(BASE64.encode([0x34_u8, 0x12, 0xFF, 0xFF]))
        );

        let mut bytes = Vec::new();
        write_command(
            &mut bytes,
            "session",
            3,
            &BridgeCommand::PlayerAudioBegin {
                wav_basename: "player-recording-3.wav".into(),
            },
        )
        .unwrap();
        let line = String::from_utf8(bytes).unwrap();
        assert!(line.ends_with('\n'));
        let value: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["type"], "player_audio_begin");
        assert_eq!(value["message_id"], "rust-3");
        assert_eq!(value["payload"]["sample_rate"], 24_000);
        assert_eq!(value["payload"]["format"], "pcm_s16le");

        let (end_type, end_payload) = BridgeCommand::PlayerAudioEnd {
            wav_basename: "player-recording-3.wav".into(),
            chunk_count: 8,
            silent: false,
        }
        .wire_parts();
        assert_eq!(end_type, "player_audio_end");
        assert_eq!(
            end_payload,
            serde_json::json!({
                "wav_basename": "player-recording-3.wav",
                "chunk_count": 8,
                "silent": false,
            })
        );

        let (abort_type, abort_payload) = BridgeCommand::PlayerAudioAbort {
            wav_basename: "player-recording-3.wav".into(),
        }
        .wire_parts();
        assert_eq!(abort_type, "player_audio_abort");
        assert_eq!(
            abort_payload,
            serde_json::json!({"wav_basename": "player-recording-3.wav"})
        );
    }

    #[test]
    fn streaming_commands_are_never_coalesced_as_spatials() {
        assert!(
            !BridgeCommand::PlayerAudioChunk {
                wav_basename: "player-recording-1.wav".into(),
                seq: 0,
                samples: vec![0_i16].into(),
            }
            .is_redundant_spatial()
        );
        assert!(
            !BridgeCommand::PlayerAudioEnd {
                wav_basename: "player-recording-1.wav".into(),
                chunk_count: 1,
                silent: false,
            }
            .is_redundant_spatial()
        );
    }

    #[test]
    fn server_envelope_fixture_decodes_exact_protocol_shape() {
        let line = r#"{"protocol_version":1,"session_id":"s","message_id":"python-1","type":"status","payload":{"subsystem":"llm","state":"idle"},"event_seq":1}"#;
        let decoded: ServerEnvelope = serde_json::from_str(line).unwrap();
        assert_eq!(decoded.event_seq, 1);
        assert_eq!(decoded.message_type, "status");
    }

    #[test]
    fn wav_basenames_cannot_escape_the_private_session() {
        let runtime = Path::new("/tmp/session");
        assert_eq!(
            safe_session_path(runtime, "speech-1.wav"),
            Some(runtime.join("speech-1.wav"))
        );
        for name in ["../a.wav", "/tmp/a.wav", "a/b.wav", "a.mp3", ""] {
            assert!(safe_session_path(runtime, name).is_none(), "{name}");
        }
    }

    #[test]
    fn spatial_update_carries_the_player_facing() {
        let (message_type, payload) = BridgeCommand::SpatialUpdate {
            position_m: position(),
            spatial_seq: 4,
            facing_yaw: 1.25,
        }
        .wire_parts();
        assert_eq!(message_type, "spatial_update");
        assert_eq!(payload["updates"][0]["facing_yaw"], 1.25);
        assert_eq!(payload["updates"][0]["actor_id"], "player");
    }

    #[test]
    fn player_sound_command_has_the_strict_wire_shape() {
        let (message_type, payload) = BridgeCommand::PlayerSound {
            sound_id: "fart".into(),
        }
        .wire_parts();
        assert_eq!(message_type, "player_sound");
        assert_eq!(payload, serde_json::json!({"sound_id": "fart"}));

        let (message_type, payload) = BridgeCommand::DebugSound {
            sound_id: "town_bell".into(),
            position_m: position(),
        }
        .wire_parts();
        assert_eq!(message_type, "debug_sound");
        assert_eq!(payload["sound_id"], "town_bell");
        assert_eq!(payload["position_m"]["y"], 2.0);
    }

    #[test]
    fn spatial_updates_are_explicitly_safe_to_coalesce() {
        assert!(
            BridgeCommand::SpatialUpdate {
                position_m: position(),
                spatial_seq: 2,
                facing_yaw: 0.0,
            }
            .is_redundant_spatial()
        );
        assert!(
            !BridgeCommand::PlayerAccept {
                request_id: "r".into(),
                item_id: ItemId("coin".into()),
                position_m: position(),
                spatial_seq: 2,
            }
            .is_redundant_spatial()
        );
    }

    #[test]
    fn speech_presented_command_has_the_strict_wire_shape() {
        let (message_type, payload) = BridgeCommand::SpeechPresented {
            speech_event_id: "speech-7".into(),
        }
        .wire_parts();
        assert_eq!(message_type, "speech_presented");
        assert_eq!(payload, serde_json::json!({"speech_event_id": "speech-7"}));
    }

    #[test]
    fn tts_selection_command_has_the_strict_wire_shape() {
        let (message_type, payload) = BridgeCommand::SetTtsBackend {
            request_id: "tts-mode-1".into(),
            backend: TtsBackend::Local,
        }
        .wire_parts();
        assert_eq!(message_type, "set_tts_backend");
        assert_eq!(
            payload,
            serde_json::json!({"request_id": "tts-mode-1", "backend": "local"})
        );
    }

    #[test]
    fn truncated_wav_is_rejected_before_bevy_audio_decodes_it() {
        assert!(validate_wav_bytes(b"RIFF\0\0\0\0WAVE").is_err());
    }

    #[test]
    fn protocol_line_limit_discards_without_growing_the_buffer() {
        let mut input = vec![b'x'; 128];
        input.extend_from_slice(b"\nok\n");
        let mut reader = BufReader::new(Cursor::new(input));
        let mut output = Vec::new();

        assert_eq!(
            read_bounded_line(&mut reader, &mut output, 32).unwrap(),
            BoundedLine::TooLong
        );
        assert!(output.len() <= 32);
        assert_eq!(
            read_bounded_line(&mut reader, &mut output, 32).unwrap(),
            BoundedLine::Line
        );
        assert_eq!(output, b"ok");
    }

    #[test]
    fn wav_reader_enforces_the_limit_during_the_read() {
        assert_eq!(
            read_with_limit(Cursor::new(vec![7; 16]), 16).unwrap(),
            vec![7; 16]
        );
        assert!(read_with_limit(Cursor::new(vec![7; 17]), 16).is_err());
    }

    #[test]
    fn inbox_reports_worker_channel_disconnection_exactly_once() {
        let (sender, receiver) = bounded(1);
        let inbox = BridgeInbox {
            events: receiver,
            disconnect_reported: AtomicBool::new(false),
        };
        drop(sender);

        assert!(matches!(
            inbox.try_recv(),
            Some(BridgeEvent::Disconnected(_))
        ));
        assert!(inbox.try_recv().is_none());
    }

    #[test]
    fn authoritative_events_wait_for_bounded_queue_space_instead_of_dropping() {
        let (sender, receiver) = bounded(1);
        sender
            .send(BridgeEvent::Degraded("occupying slot".into()))
            .unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let sink = WorkerEventSink::new(sender, cancelled);
        let delivery = thread::spawn(move || {
            sink.send(BridgeEvent::Message(ServerEnvelope {
                protocol_version: PROTOCOL_VERSION,
                session_id: "session".into(),
                message_id: "python-1".into(),
                event_seq: 1,
                message_type: "status".into(),
                payload: json!({"subsystem": "llm", "state": "idle"}),
            }))
        });

        thread::sleep(Duration::from_millis(40));
        assert!(!delivery.is_finished());
        assert!(matches!(receiver.recv().unwrap(), BridgeEvent::Degraded(_)));
        assert!(matches!(receiver.recv().unwrap(), BridgeEvent::Message(_)));
        assert!(delivery.join().unwrap());
    }

    #[test]
    fn worker_guard_drop_is_bounded_when_queues_and_worker_are_stuck() {
        let (commands, command_receiver) = bounded(1);
        commands.try_send(BridgeCommand::Shutdown).unwrap();
        let (cancel, _cancel_receiver) = bounded(1);
        cancel.try_send(()).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let (release, wait_for_release) = std::sync::mpsc::channel();
        let (finished, did_finish) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = wait_for_release.recv();
            let _ = finished.send(());
        });
        let guard = BridgeWorkerGuard {
            commands,
            cancel,
            cancelled: cancelled.clone(),
            child: Arc::new(Mutex::new(None)),
            worker: Some(worker),
        };

        let started = Instant::now();
        drop(guard);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(cancelled.load(Ordering::Acquire));

        let _ = release.send(());
        did_finish.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(command_receiver);
    }

    #[test]
    fn missing_uv_reports_disconnect_and_removes_the_runtime_directory() {
        let (handle, inbox, guard) = spawn_sidecar(BridgeLaunchConfig {
            uv_binary: "__cathedralbevy_missing_uv__".into(),
            server_script: PathBuf::from("server.py"),
            fake_backend: false,
            tts_backend: "off".into(),
            sounds_enabled: true,
            view_cone_degrees: 135.0,
            min_seconds_between_player_sounds: 2.0,
        });
        let runtime_dir = handle.runtime_dir().to_path_buf();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut disconnected = false;
        while Instant::now() < deadline && !disconnected {
            disconnected = matches!(inbox.try_recv(), Some(BridgeEvent::Disconnected(_)));
            if !disconnected {
                thread::sleep(Duration::from_millis(5));
            }
        }

        assert!(disconnected, "a missing uv executable was not surfaced");
        drop(guard);
        assert!(!runtime_dir.exists());
    }

    #[test]
    fn fake_sidecar_runs_scripted_exchange_offline_and_cleans_up() {
        if Command::new("uv")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            // Missing uv is a supported runtime degradation, not a reason for
            // an otherwise offline test suite to fail on that machine.
            return;
        }

        let (handle, inbox, guard) = spawn_sidecar(BridgeLaunchConfig {
            uv_binary: "uv".into(),
            server_script: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("prompt_playgound/server.py"),
            fake_backend: true,
            tts_backend: "local".into(),
            sounds_enabled: true,
            view_cone_degrees: 135.0,
            min_seconds_between_player_sounds: 2.0,
        });
        let runtime_dir = handle.runtime_dir().to_path_buf();
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut hello_sent = false;
        let mut ready = false;
        while std::time::Instant::now() < deadline && !ready {
            match inbox.try_recv() {
                Some(BridgeEvent::ProcessStarted) => {
                    handle
                        .try_send(BridgeCommand::Hello {
                            position_m: position(),
                            spatial_seq: 0,
                        })
                        .unwrap();
                    hello_sent = true;
                }
                Some(BridgeEvent::Message(envelope)) if envelope.message_type == "ready" => {
                    assert_eq!(envelope.session_id, handle.session_id());
                    assert_eq!(envelope.payload["snapshot"]["player_id"], "player");
                    ready = true;
                }
                Some(BridgeEvent::Disconnected(error)) => {
                    panic!("fake sidecar disconnected during handshake: {error}")
                }
                Some(_) | None => thread::sleep(Duration::from_millis(5)),
            }
        }
        assert!(hello_sent, "bridge worker never started");
        assert!(ready, "fake sidecar never completed its handshake");

        let player_position = Position::new(0.0, 0.91, 111.0).unwrap();
        handle
            .try_send(BridgeCommand::DebugPlayerSay {
                request_id: "ask-name".into(),
                text: "What's your name?".into(),
                target_id: None,
                position_m: player_position,
                spatial_seq: 1,
            })
            .unwrap();
        let mut reply_seen = false;
        let mut result_seen = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        while std::time::Instant::now() < deadline && !(reply_seen && result_seen) {
            if let Some(BridgeEvent::Message(envelope)) = inbox.try_recv() {
                if envelope.message_type == "speech" && envelope.payload["speaker_id"] == "k0fb1" {
                    reply_seen = true;
                    // Act as the presentation layer: retire the utterance so
                    // the conversation floor frees for Ilse's next turn well
                    // inside this test's deadlines.
                    let event_id = envelope.payload["event_id"]
                        .as_str()
                        .expect("speech events carry an event_id")
                        .to_owned();
                    handle
                        .try_send(BridgeCommand::SpeechPresented {
                            speech_event_id: event_id,
                        })
                        .unwrap();
                }
                if envelope.message_type == "command_result"
                    && envelope.payload["request_id"] == "ask-name"
                    && envelope.payload["success"] == true
                {
                    result_seen = true;
                }
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(reply_seen && result_seen, "Ilse did not answer the player");

        handle
            .try_send(BridgeCommand::DebugPlayerSay {
                request_id: "ask-coin".into(),
                text: "Please offer me your coin".into(),
                target_id: None,
                position_m: player_position,
                spatial_seq: 1,
            })
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut coin_offered = false;
        while std::time::Instant::now() < deadline && !coin_offered {
            if let Some(BridgeEvent::Message(envelope)) = inbox.try_recv() {
                if envelope.message_type == "world_snapshot" {
                    let snapshot: WorldSnapshot = serde_json::from_value(envelope.payload).unwrap();
                    coin_offered = snapshot.offers.iter().any(|offer| {
                        offer.item_id.0 == "c0prs"
                            && offer.giver_id.0 == "k0fb1"
                            && offer.target_id.as_ref().is_some_and(|id| id.0 == "player")
                    });
                }
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(coin_offered, "Ilse did not create the pending coin offer");

        handle
            .try_send(BridgeCommand::PlayerAccept {
                request_id: "accept-coin".into(),
                item_id: ItemId("c0prs".into()),
                position_m: player_position,
                spatial_seq: 1,
            })
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        let mut coin_transferred = false;
        while std::time::Instant::now() < deadline && !coin_transferred {
            if let Some(BridgeEvent::Message(envelope)) = inbox.try_recv() {
                if envelope.message_type == "world_snapshot" {
                    let snapshot: WorldSnapshot = serde_json::from_value(envelope.payload).unwrap();
                    coin_transferred = snapshot
                        .actors
                        .iter()
                        .find(|actor| actor.id.0 == "player")
                        .is_some_and(|player| player.holds.iter().any(|item| item.0 == "c0prs"))
                        && snapshot
                            .offers
                            .iter()
                            .all(|offer| offer.item_id.0 != "c0prs");
                }
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(
            coin_transferred,
            "authoritative acceptance did not transfer the coin"
        );

        handle
            .try_send(BridgeCommand::PlayerOffer {
                request_id: "offer-conny".into(),
                target_id: ActorId("cb947".into()),
                item_id: ItemId("c0prs".into()),
                position_m: player_position,
                spatial_seq: 1,
            })
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        let mut reoffer_pending_without_transfer = false;
        while std::time::Instant::now() < deadline && !reoffer_pending_without_transfer {
            if let Some(BridgeEvent::Message(envelope)) = inbox.try_recv() {
                if envelope.message_type == "world_snapshot" {
                    let snapshot: WorldSnapshot = serde_json::from_value(envelope.payload).unwrap();
                    let player_still_holds = snapshot
                        .actors
                        .iter()
                        .find(|actor| actor.id.0 == "player")
                        .is_some_and(|player| player.holds.iter().any(|item| item.0 == "c0prs"));
                    let pending = snapshot.offers.iter().any(|offer| {
                        offer.item_id.0 == "c0prs"
                            && offer.giver_id.0 == "player"
                            && offer.target_id.as_ref().is_some_and(|id| id.0 == "cb947")
                    });
                    reoffer_pending_without_transfer = player_still_holds && pending;
                }
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(
            reoffer_pending_without_transfer,
            "re-offer mutated ownership before Conny accepted"
        );

        drop(guard);
        assert!(
            !runtime_dir.exists(),
            "bridge did not remove its session dir"
        );
    }
}
