//! The engine (`server.py`'s `SmartActorServer`, minus the wire).
//!
//! One authoritative [`World`], one [`NpcScheduler`] that mutates it, one
//! [`ConversationFloor`] that paces it, and one pump — [`Engine::poll`] — that
//! the host calls every frame with the commands it collected and the clock it
//! reads. Everything the engine wants to say comes back as a `Vec` of
//! [`EngineMessage`]; everything it wants done goes out through the backend
//! traits. No channels, no threads, no clock, no IO (D22).
//!
//! The pump order is Python's (`server.py:726-739`) and it is load-bearing:
//! commands, then the scheduler (which may apply a finished turn and emit its
//! domain events), then the speech router, then the event fan-out (which
//! acquires the floor that gates the *next* poll's scheduler), then the
//! snapshot. Player commands that mutate the world flush their own events and
//! snapshot inline, *before* their `CommandResult` — a position update can
//! succeed while the action it came with fails, and that revision bump has to
//! reach the game either way (`server.py:1013-1025`).

use std::{path::PathBuf, sync::Arc};

use serde_json::{Value, json};

use crate::{
    actions::apply_action,
    character::Control,
    error::{CommandError, CommandErrorCode, EngineInitError},
    event::{DomainEvent, EventType},
    floor::ConversationFloor,
    ids::{ActorId, ItemId, SpeechEventId},
    math::Vec3,
    perception::{cap_first, emit_sound, identify},
    prompt::PromptEnv,
    scheduler::{NpcScheduler, SchedulerEvent, llm_turn_order},
    seed::{WorldConfig, WorldSeed, build_world},
    snapshot::PublicSnapshot,
    sounds::SoundCatalog,
    speech_router::{SpeechContext, SpeechRouter},
    status::{STATE_UNAVAILABLE, StatusEvent, Subsystem},
    traits::{
        Cognition, Completion, Sight, SttBackendKind, Transcription, TranscriptionOutcome, Tts,
        TtsBackendKind, TtsOutcome,
    },
    world::{SpatialActorUpdate, World},
};

/// The borrows the speech router needs, taken from disjoint [`Engine`] fields.
///
/// A macro rather than a method: a `fn speech_context(&mut self)` would borrow
/// *all* of `self`, and the router — which is also a field of `self` — has to be
/// the receiver of the very call it is an argument to.
macro_rules! speech_context {
    ($engine:ident) => {
        SpeechContext {
            world: &mut $engine.world,
            floor: &mut $engine.floor,
            scheduler: &mut $engine.scheduler,
            transcript: &mut $engine.transcript,
            transcription: $engine.transcription.as_mut(),
            tts: $engine.tts.as_mut(),
            player_id: &$engine.config.player_id,
            runtime_dir: $engine.config.runtime_dir.as_path(),
            fake_mode: $engine.config.fake_mode,
            tts_selected: $engine.tts_selected,
        }
    };
}

/// `SMART_ACTORS_SOUND_COOLDOWN_SECONDS` (`server.py:452-454`).
pub const DEFAULT_SOUND_COOLDOWN_SECONDS: f64 = 2.0;
pub const MAX_SOUND_COOLDOWN_SECONDS: f64 = 3_600.0;
/// `NPC_TURN_DELAY_SECONDS` (`server.py:514-518`).
pub const DEFAULT_TURN_DELAY_SECONDS: f64 = 1.0;
/// The provider-backoff ceiling (`scheduler.py:128-130`).
pub const DEFAULT_MAXIMUM_BACKOFF_SECONDS: f64 = 60.0;
/// `STT_STREAM_COMPLETION_GRACE_MS` (speech spec; carried, unused until P6).
pub const DEFAULT_STT_STREAM_GRACE_SECONDS: f64 = 2.0;
pub const MIN_STT_STREAM_GRACE_SECONDS: f64 = 0.2;
/// `command_result.message` is truncated here (`server.py:980`).
pub const MAX_COMMAND_MESSAGE_CHARS: usize = 300;
/// `tts_failed.reason` is truncated here (`server.py:2018`).
pub const MAX_TTS_FAILURE_REASON_CHARS: usize = 160;

/// Status state the voice selection adds to the scheduler's `STATE_*` set.
const STATE_SELECTED: &str = "selected";

/// What this engine can actually do (`server.py:837-849`).
///
/// Three independent capabilities, computed once at construction: a missing
/// `OPENAI_API_KEY` must take voices down without taking cognition with it. The
/// derived `stt`/`tts` ORs are kept so the HUD does not re-derive them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub llm: bool,
    pub stt: bool,
    pub stt_cloud: bool,
    pub stt_local: bool,
    pub tts: bool,
    pub tts_cloud: bool,
    pub tts_local: bool,
    /// The selection *after* the availability fallback (`server.py:493-513`).
    pub tts_selected: TtsBackendKind,
}

impl Default for Capabilities {
    /// Nothing is configured — the all-false block `test_ready_contains_…`
    /// (test 5) pins.
    fn default() -> Self {
        Self {
            llm: false,
            stt: false,
            stt_cloud: false,
            stt_local: false,
            tts: false,
            tts_cloud: false,
            tts_local: false,
            tts_selected: TtsBackendKind::Off,
        }
    }
}

impl Capabilities {
    /// Derive `stt`/`tts` from the four independent probes, so the ORs can never
    /// drift from the booleans they summarize.
    pub fn new(
        llm: bool,
        stt_cloud: bool,
        stt_local: bool,
        tts_cloud: bool,
        tts_local: bool,
        tts_selected: TtsBackendKind,
    ) -> Self {
        Self {
            llm,
            stt: stt_cloud || stt_local,
            stt_cloud,
            stt_local,
            tts: tts_cloud || tts_local,
            tts_cloud,
            tts_local,
            tts_selected,
        }
    }
}

/// Everything the host configures (`config.ron` → here; no env reads in the sim).
#[derive(Debug, Clone, PartialEq)]
pub struct EngineConfig {
    /// `"player"`. The engine refuses to start without this character.
    pub player_id: ActorId,
    /// Gates `DebugPlayerSay` (`server.py:1028-1031`).
    pub fake_mode: bool,
    pub sounds_enabled: bool,
    /// Clamped to `[1, 360]` by `build_world`.
    pub view_cone_degrees: f64,
    /// Clamped to `[0, 3600]` here.
    pub sound_cooldown_seconds: f64,
    pub turn_delay_seconds: f64,
    pub maximum_backoff_seconds: f64,
    /// The configured voice backend, after the host's availability fallback.
    pub tts_selected: TtsBackendKind,
    /// Why the configured voice backend is *not* the one above — set by the host
    /// when its availability fallback silenced an invalid or unavailable choice
    /// (`server.py:500-512`). Emitted once as a `tts`/`unavailable` status right
    /// after `ready` (`server.py:860-861`), so a player whose cast is mute is
    /// told why instead of being left to guess.
    pub tts_startup_message: Option<String>,
    /// Clamped to at least [`MIN_STT_STREAM_GRACE_SECONDS`].
    pub stt_stream_grace_seconds: f64,
    /// The private directory the microphone worker writes recordings into
    /// (D28 — cathedral-backends creates and owns it).
    ///
    /// The sim only *names* files in it: a basename from the game becomes the
    /// `PathBuf` a [`Transcription`] backend is handed. Nothing here opens a
    /// file (D22); an empty path simply yields the bare basename, which is what
    /// the text-only tests and the headless runner want.
    pub runtime_dir: PathBuf,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            player_id: ActorId::from_raw("player"),
            fake_mode: false,
            sounds_enabled: true,
            view_cone_degrees: crate::DEFAULT_VIEW_CONE_DEGREES,
            sound_cooldown_seconds: DEFAULT_SOUND_COOLDOWN_SECONDS,
            turn_delay_seconds: DEFAULT_TURN_DELAY_SECONDS,
            maximum_backoff_seconds: DEFAULT_MAXIMUM_BACKOFF_SECONDS,
            tts_selected: TtsBackendKind::Off,
            tts_startup_message: None,
            stt_stream_grace_seconds: DEFAULT_STT_STREAM_GRACE_SECONDS,
            runtime_dir: PathBuf::new(),
        }
    }
}

/// Everything the host asks the engine to do, in one ordered queue (D27).
#[derive(Debug, Clone, PartialEq)]
pub enum EngineCommand {
    // -------- player / game (formerly the `BridgeCommand` wire types)
    SpatialUpdate {
        spatial_seq: i64,
        updates: Vec<SpatialActorUpdate>,
    },
    PlayerOffer {
        request_id: String,
        item_id: ItemId,
        /// Mandatory and non-null, even though the sim supports broadcast offers
        /// (D13): the game UI always has a gaze target.
        target_id: ActorId,
        position_m: Vec3,
        spatial_seq: i64,
    },
    PlayerAccept {
        request_id: String,
        item_id: ItemId,
        position_m: Vec3,
        spatial_seq: i64,
    },
    PlayerDecline {
        request_id: String,
        item_id: ItemId,
        position_m: Vec3,
        spatial_seq: i64,
    },
    /// The one player action with no position: you can withdraw an offer from
    /// anywhere.
    PlayerRetract {
        request_id: String,
        item_id: ItemId,
    },
    /// Fire-and-forget: no `CommandResult` ever (`server.py:1066-1092`).
    PlayerSound {
        sound_id: String,
    },
    /// The drive-mode stand-in for world causes the sim does not model.
    DebugSound {
        sound_id: String,
        position_m: Vec3,
    },
    DebugPlayerSay {
        request_id: String,
        text: String,
        target_id: Option<ActorId>,
        position_m: Vec3,
        spatial_seq: i64,
    },
    SpeechPresented {
        event_id: SpeechEventId,
    },
    SetTtsBackend {
        request_id: String,
        backend: TtsBackendKind,
    },
    PlayerRecording {
        request_id: String,
        wav_basename: String,
        stt_backend: SttBackendKind,
        position_m: Vec3,
        spatial_seq: i64,
    },
    PlayerAudioBegin {
        wav_basename: String,
        sample_rate: u32,
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
    // -------- backend completions (the host drains its channels into these)
    LlmCompletion(Completion),
    Transcription(TranscriptionOutcome),
    Tts(TtsOutcome),
    /// Worker/warmup statuses the backends raise on their own.
    BackendStatus(StatusEvent),
}

/// Everything the engine tells the host.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineMessage {
    /// Once, first, on the first poll — the handshake's `ready`.
    Ready {
        capabilities: Capabilities,
        snapshot: PublicSnapshot,
    },
    /// On every public-revision increase (`server.py:2223-2229`).
    Snapshot(PublicSnapshot),
    Speech {
        event_id: SpeechEventId,
        speaker_id: ActorId,
        target_id: Option<ActorId>,
        text: String,
        speaker_position_m: Vec3,
        recipient_ids: Vec<ActorId>,
        /// `"You"`, the name, or `"a stranger (id …)"` — resolved here so the
        /// game never decides what the player knows.
        speaker_name_for_player: String,
    },
    Sound {
        event_id: String,
        sound_id: String,
        sound_class: String,
        /// **Fail dark**: withheld unless the player made the sound or witnessed
        /// it (`server.py:2197`). An unattributed sound must not leak its actor.
        actor_id: Option<ActorId>,
        position_m: Vec3,
        audible_distance: f64,
        recipient_ids: Vec<ActorId>,
        witness_ids: Vec<ActorId>,
        text_for_player: Option<String>,
    },
    WorldEvent {
        event_id: String,
        kind: String,
        actor_id: ActorId,
        target_id: Option<ActorId>,
        item_id: Option<ItemId>,
        recipient_ids: Vec<ActorId>,
    },
    CommandResult {
        request_id: String,
        success: bool,
        error_code: Option<String>,
        message: String,
    },
    TranscriptionResult {
        request_id: String,
        text: Option<String>,
        error: Option<String>,
    },
    Status(StatusEvent),
    TtsReady {
        event_id: SpeechEventId,
        wav_bytes: Arc<[u8]>,
    },
    TtsChunk {
        event_id: SpeechEventId,
        chunk_seq: u32,
        sample_rate: u32,
        samples: Arc<[i16]>,
    },
    TtsStreamEnd {
        event_id: SpeechEventId,
        chunk_count: u32,
        first_chunk_ms: u32,
    },
    TtsFailed {
        event_id: SpeechEventId,
        reason: String,
    },
    /// One archived LLM exchange; the host writes the files (D24).
    PromptExchange {
        actor_id: ActorId,
        actor_name: String,
        prompt: String,
        answer: Option<String>,
        duration_seconds: f64,
        error: Option<String>,
    },
    /// A former `[smart actors] …` stderr line; the host logs it.
    Diagnostic(String),
}

pub struct Engine {
    world: World,
    /// The omniscient run transcript. It lives here, not in [`World`]: it is a
    /// presentation artifact of *this session*, not world state (sim.md risk 10).
    transcript: Vec<String>,
    scheduler: NpcScheduler,
    floor: ConversationFloor,
    speech_router: SpeechRouter,
    env: PromptEnv,
    cognition: Box<dyn Cognition>,
    transcription: Box<dyn Transcription>,
    tts: Box<dyn Tts>,
    sight: Box<dyn Sight>,
    config: EngineConfig,
    capabilities: Capabilities,
    /// Mutable at runtime (the X key / the Esc menu), unlike the rest of config.
    tts_selected: TtsBackendKind,
    last_snapshot_revision: i64,
    /// −∞ so the very first `player_sound` is never inside the cooldown
    /// (`server.py:455`).
    last_player_sound_at: f64,
    ready_emitted: bool,
}

impl Engine {
    /// Build the world from `seed`, put the player at `player_spawn`, and start
    /// the turn stream if there is any cognition to run it with.
    ///
    /// This *is* the handshake (`server.py:802-861`): construction and start
    /// coincide, so the pre-handshake event-suppression guard has nothing to
    /// suppress and does not exist.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: EngineConfig,
        seed: &WorldSeed,
        catalog: SoundCatalog,
        env: PromptEnv,
        cognition: Box<dyn Cognition>,
        transcription: Box<dyn Transcription>,
        tts: Box<dyn Tts>,
        sight: Box<dyn Sight>,
        capabilities: Capabilities,
        player_spawn: (Vec3, f64),
        spatial_seq: i64,
        now: f64,
    ) -> Result<Self, EngineInitError> {
        let mut world = build_world(
            seed,
            WorldConfig {
                sounds_enabled: config.sounds_enabled,
                view_cone_degrees: config.view_cone_degrees,
                sound_catalog: catalog,
            },
        );
        if !world.characters.contains_key(&config.player_id) {
            return Err(EngineInitError::MissingPlayer(config.player_id.clone()));
        }

        // `hello` *is* the first spatial update (`server.py:826-828`) — and it
        // carries the spawn facing too, so the very first snapshot ships the
        // real cone the witness test will use.
        let (position_m, facing_yaw) = player_spawn;
        world
            .update_positions(
                spatial_seq,
                &[SpatialActorUpdate::new(
                    config.player_id.clone(),
                    position_m,
                    Some(facing_yaw),
                )],
            )
            .map_err(EngineInitError::PlayerSpawn)?;

        let mut capabilities = capabilities;
        // One source of truth for the selection: the config's, which the host
        // has already run through the availability fallback.
        capabilities.tts_selected = config.tts_selected;

        let mut scheduler = NpcScheduler::new(
            llm_turn_order(&world),
            config.turn_delay_seconds,
            config.maximum_backoff_seconds,
            now,
        );
        if capabilities.llm {
            scheduler.start(now);
        }

        let sound_cooldown_seconds = config
            .sound_cooldown_seconds
            .clamp(0.0, MAX_SOUND_COOLDOWN_SECONDS);
        let stt_stream_grace_seconds = config
            .stt_stream_grace_seconds
            .max(MIN_STT_STREAM_GRACE_SECONDS);
        let tts_selected = config.tts_selected;
        let config = EngineConfig {
            sound_cooldown_seconds,
            stt_stream_grace_seconds,
            ..config
        };

        // A run that *starts* on the local voice warms it now, not inside the
        // cast's first line (`server.py:594-599`). Without this the Pocket model
        // load lands in the first `submit`, and a slow enough cold start can
        // outlast the floor's failsafe deadline and drop the line's audio.
        let mut tts = tts;
        if tts_selected == TtsBackendKind::Local {
            tts.warm(TtsBackendKind::Local);
        }

        Ok(Self {
            world,
            transcript: Vec::new(),
            scheduler,
            floor: ConversationFloor::new(),
            speech_router: SpeechRouter::new(stt_stream_grace_seconds),
            env,
            cognition,
            transcription,
            tts,
            sight,
            config,
            capabilities,
            tts_selected,
            last_snapshot_revision: 0,
            last_player_sound_at: f64::NEG_INFINITY,
            ready_emitted: false,
        })
    }

    /// The single pump (`server.py:726-739`).
    pub fn poll(&mut self, now: f64, commands: Vec<EngineCommand>) -> Vec<EngineMessage> {
        let mut out: Vec<EngineMessage> = Vec::new();

        if !self.ready_emitted {
            self.ready_emitted = true;
            out.push(EngineMessage::Ready {
                capabilities: self.capabilities,
                snapshot: self.snapshot(),
            });
            self.last_snapshot_revision = self.world.world_revision;
            if !self.capabilities.llm {
                // Said once, after ready, exactly as Python did
                // (`server.py:856-859`) — the HUD shows the cast as offline
                // rather than merely quiet.
                out.push(EngineMessage::Status(StatusEvent::llm(
                    STATE_UNAVAILABLE,
                    None,
                    Some("text cognition is not configured".to_string()),
                )));
            }
            // …and then, in the same order Python sent them
            // (`server.py:860-861`), why the configured voice is silent.
            if let Some(message) = self.config.tts_startup_message.clone() {
                out.push(EngineMessage::Status(StatusEvent {
                    subsystem: Subsystem::Tts,
                    state: STATE_UNAVAILABLE.to_string(),
                    actor_id: None,
                    message: Some(message),
                    backend: None,
                }));
            }
        }

        let mut completions: Vec<Completion> = Vec::new();
        for command in commands {
            self.apply_command(now, command, &mut completions, &mut out);
        }

        // Computed once per poll (D20): the floor's expiry purge is a side
        // effect, and the scheduler must not be able to change how often it runs.
        let floor_busy = self.floor.busy(now);
        let events = self.scheduler.poll(
            now,
            &mut self.world,
            &mut self.transcript,
            &mut completions,
            floor_busy,
            self.cognition.as_mut(),
            &self.env,
        );
        for event in events {
            out.push(scheduler_message(event));
        }

        // Grace windows and held transcripts: the two deadlines nobody else
        // watches. A resolution here can apply a player `say`, whose events the
        // flush below fans out.
        self.speech_router
            .poll(now, &mut speech_context!(self), &mut out);

        // The scheduler's turn produced domain events in this same poll; the
        // floor they acquire here gates the *next* one.
        self.flush(now, &mut out);
        out
    }

    pub fn snapshot(&self) -> PublicSnapshot {
        self.world.public_snapshot(&self.config.player_id)
    }

    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// The omniscient transcript: every applied action's terminal line, `wait`
    /// excepted.
    pub fn transcript(&self) -> &[String] {
        &self.transcript
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// For tests and the headless runner, which drive NPC actions directly.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn scheduler(&self) -> &NpcScheduler {
        &self.scheduler
    }

    /// The microphone/voice state machines, for tests and diagnostics.
    pub fn speech_router(&self) -> &SpeechRouter {
        &self.speech_router
    }

    /// Purges overdue awaited utterances, then answers.
    pub fn floor_busy(&mut self, now: f64) -> bool {
        self.floor.busy(now)
    }

    /// The renderer hook (occlusion, NPC-eye frames). Plumbed, unconsumed.
    pub fn sight_mut(&mut self) -> &mut dyn Sight {
        self.sight.as_mut()
    }

    /// Owned by the engine so P6's router can reach it without a re-plumb.
    pub fn transcription_mut(&mut self) -> &mut dyn Transcription {
        self.transcription.as_mut()
    }

    /// Stop submitting new turns. An in-flight result is still applied.
    pub fn close(&mut self) {
        self.scheduler.close();
    }

    // ------------------------------------------------------------- commands

    fn apply_command(
        &mut self,
        now: f64,
        command: EngineCommand,
        completions: &mut Vec<Completion>,
        out: &mut Vec<EngineMessage>,
    ) {
        match command {
            EngineCommand::SpatialUpdate {
                spatial_seq,
                updates,
            } => self.spatial_update(spatial_seq, &updates, out),

            EngineCommand::PlayerOffer {
                request_id,
                item_id,
                target_id,
                position_m,
                spatial_seq,
            } => self.player_action(
                now,
                &request_id,
                "offer_item",
                json!({"item_id": item_id.as_str(), "target": target_id.as_str()}),
                Some((spatial_seq, position_m)),
                out,
            ),

            EngineCommand::PlayerAccept {
                request_id,
                item_id,
                position_m,
                spatial_seq,
            } => self.player_action(
                now,
                &request_id,
                "accept_offered_item",
                json!({"item_id": item_id.as_str()}),
                Some((spatial_seq, position_m)),
                out,
            ),

            EngineCommand::PlayerDecline {
                request_id,
                item_id,
                position_m,
                spatial_seq,
            } => self.player_action(
                now,
                &request_id,
                "decline_offer",
                json!({"item_id": item_id.as_str()}),
                Some((spatial_seq, position_m)),
                out,
            ),

            EngineCommand::PlayerRetract {
                request_id,
                item_id,
            } => self.player_action(
                now,
                &request_id,
                "retract_offer",
                json!({"item_id": item_id.as_str()}),
                None,
                out,
            ),

            EngineCommand::DebugPlayerSay {
                request_id,
                text,
                target_id,
                position_m,
                spatial_seq,
            } => self.debug_player_say(
                now,
                &request_id,
                &text,
                target_id,
                position_m,
                spatial_seq,
                out,
            ),

            EngineCommand::PlayerSound { sound_id } => self.player_sound(now, &sound_id, out),

            EngineCommand::DebugSound {
                sound_id,
                position_m,
            } => self.debug_sound(now, &sound_id, position_m, out),

            // Fire-and-forget, and idempotent by contract (D26): duplicates and
            // ids whose failsafe already expired are legitimately unknown.
            EngineCommand::SpeechPresented { event_id } => self.floor.release(now, &event_id),

            EngineCommand::SetTtsBackend {
                request_id,
                backend,
            } => self.set_tts_backend(&request_id, backend, out),

            EngineCommand::LlmCompletion(completion) => completions.push(completion),

            EngineCommand::BackendStatus(status) => out.push(EngineMessage::Status(status)),

            EngineCommand::Tts(outcome) => {
                self.speech_router
                    .on_tts(now, outcome, &mut speech_context!(self), out)
            }

            EngineCommand::Transcription(outcome) => {
                self.speech_router
                    .on_transcription(now, outcome, &mut speech_context!(self), out)
            }

            EngineCommand::PlayerRecording {
                request_id,
                wav_basename,
                stt_backend,
                position_m,
                spatial_seq,
            } => self.speech_router.on_recording(
                now,
                &request_id,
                &wav_basename,
                stt_backend,
                position_m,
                spatial_seq,
                &mut speech_context!(self),
                out,
            ),

            EngineCommand::PlayerAudioBegin {
                wav_basename,
                sample_rate,
            } => self.speech_router.on_audio_begin(
                now,
                &wav_basename,
                sample_rate,
                &mut speech_context!(self),
                out,
            ),

            EngineCommand::PlayerAudioChunk {
                wav_basename,
                seq,
                samples,
            } => self.speech_router.on_audio_chunk(
                now,
                &wav_basename,
                seq,
                &samples,
                &mut speech_context!(self),
                out,
            ),

            EngineCommand::PlayerAudioEnd {
                wav_basename,
                chunk_count,
                silent,
            } => self.speech_router.on_audio_end(
                now,
                &wav_basename,
                chunk_count,
                silent,
                &mut speech_context!(self),
                out,
            ),

            EngineCommand::PlayerAudioAbort { wav_basename } => self
                .speech_router
                .on_audio_abort(&wav_basename, &mut speech_context!(self)),
        }
    }

    /// `_handle_spatial_update` (`server.py:907-946`). Fire-and-forget: a bad
    /// update is a bug in the sender, not something the player can act on.
    fn spatial_update(
        &mut self,
        spatial_seq: i64,
        updates: &[SpatialActorUpdate],
        out: &mut Vec<EngineMessage>,
    ) {
        if updates.is_empty() {
            out.push(EngineMessage::Diagnostic(
                "[smart actors] invalid spatial_update: updates must be non-empty".to_string(),
            ));
            return;
        }
        // Only the player moves. NPC positions are static world state; a client
        // that tries to move one is confused, and letting it would silently
        // rewrite the cast's geometry (`server.py:924-929`).
        if let Some(other) = updates
            .iter()
            .find(|update| update.actor_id != self.config.player_id)
        {
            out.push(EngineMessage::Diagnostic(format!(
                "[smart actors] invalid spatial_update: spatial updates may only move the player, not '{}'",
                other.actor_id
            )));
            return;
        }
        if let Err(error) = self.world.update_positions(spatial_seq, updates) {
            out.push(EngineMessage::Diagnostic(format!(
                "[smart actors] invalid spatial_update: {error}"
            )));
        }
    }

    /// `_handle_player_action` (`server.py:987-1025`), including the ordering
    /// that matters: on failure the events and the snapshot go out **first**,
    /// because the position update that came with the command may well have
    /// succeeded.
    fn player_action(
        &mut self,
        now: f64,
        request_id: &str,
        verb: &str,
        args: Value,
        position: Option<(i64, Vec3)>,
        out: &mut Vec<EngineMessage>,
    ) {
        let result = self.apply_player_action(verb, &args, position);
        self.finish_player_action(now, request_id, result, out);
    }

    fn apply_player_action(
        &mut self,
        verb: &str,
        args: &Value,
        position: Option<(i64, Vec3)>,
    ) -> Result<String, CommandError> {
        if let Some((spatial_seq, position_m)) = position {
            self.world.update_positions(
                spatial_seq,
                &[SpatialActorUpdate::new(
                    self.config.player_id.clone(),
                    position_m,
                    None,
                )],
            )?;
        }
        let player_id = self.config.player_id.clone();
        Ok(apply_action(&mut self.world, &player_id, verb, args)?)
    }

    fn finish_player_action(
        &mut self,
        now: f64,
        request_id: &str,
        result: Result<String, CommandError>,
        out: &mut Vec<EngineMessage>,
    ) {
        match result {
            Ok(line) => {
                self.transcript.push(line.clone());
                self.flush(now, out);
                out.push(EngineMessage::CommandResult {
                    request_id: request_id.to_string(),
                    success: true,
                    error_code: None,
                    message: truncate_chars(&line, MAX_COMMAND_MESSAGE_CHARS),
                });
            }
            Err(error) => {
                self.flush(now, out);
                out.push(command_failure(request_id, error.code, &error.message));
            }
        }
    }

    /// `_handle_debug_player_say` (`server.py:1027-1064`) — fake mode only, full
    /// sim validation, and the addressee gets the next *selection* slot (not an
    /// immediate turn: the inter-turn delay and the floor still govern when).
    #[allow(clippy::too_many_arguments)]
    fn debug_player_say(
        &mut self,
        now: f64,
        request_id: &str,
        text: &str,
        target_id: Option<ActorId>,
        position_m: Vec3,
        spatial_seq: i64,
        out: &mut Vec<EngineMessage>,
    ) {
        if !self.config.fake_mode {
            out.push(command_failure(
                request_id,
                CommandErrorCode::Forbidden,
                "debug_player_say is available only in fake mode",
            ));
            return;
        }
        let args = json!({
            "text": text,
            "target": target_id.as_ref().map(ActorId::as_str),
        });
        let result = self.apply_player_action("say", &args, Some((spatial_seq, position_m)));
        if result.is_ok()
            && let Some(target_id) = &target_id
        {
            self.scheduler
                .prioritize(&self.world, target_id, false, now);
        }
        self.finish_player_action(now, request_id, result, out);
    }

    /// `_handle_player_sound` (`server.py:1066-1092`) — the F key.
    fn player_sound(&mut self, now: f64, sound_id: &str, out: &mut Vec<EngineMessage>) {
        let sound = self
            .world
            .sound_catalog
            .get(sound_id)
            .filter(|sound| sound.actor_emittable)
            .cloned();
        let Some(sound) = sound else {
            // No request_id, so nothing to fail: Python raised a ProtocolError
            // that surfaced only as a stderr line plus a `protocol degraded`
            // status. The status subsystem dies with the wire; the line stays.
            out.push(EngineMessage::Diagnostic(format!(
                "[smart actors] invalid player_sound: there is no player-emittable sound '{sound_id}'"
            )));
            return;
        };
        if !self.world.sounds_enabled {
            return;
        }
        // Dropped silently, not queued: percepts are prompt tokens, and holding
        // F must not become a denial-of-service on the LLM bill.
        if now - self.last_player_sound_at < self.config.sound_cooldown_seconds {
            return;
        }
        self.last_player_sound_at = now;
        let player_id = self.config.player_id.clone();
        let line = emit_sound(&mut self.world, Some(&player_id), &sound, None);
        self.transcript.push(line);
        self.flush(now, out);
    }

    /// `_handle_debug_sound` (`server.py:1094-1112`) — the drive-mode town bell.
    /// Any catalog row, no cooldown, no actor: world sounds are never attributed.
    fn debug_sound(
        &mut self,
        now: f64,
        sound_id: &str,
        position_m: Vec3,
        out: &mut Vec<EngineMessage>,
    ) {
        let Some(sound) = self.world.sound_catalog.get(sound_id).cloned() else {
            out.push(EngineMessage::Diagnostic(format!(
                "[smart actors] invalid debug_sound: there is no sound '{sound_id}'"
            )));
            return;
        };
        if !self.world.sounds_enabled {
            return;
        }
        let line = emit_sound(&mut self.world, None, &sound, Some(position_m));
        self.transcript.push(line);
        self.flush(now, out);
    }

    /// `_handle_set_tts_backend` (`server.py:874-905`).
    ///
    /// The `invalid_tts_backend` branch is unreachable here: the backend arrives
    /// as a [`TtsBackendKind`], so a bogus string is rejected by the host that
    /// decodes it. The strictness survives; only its home moved.
    fn set_tts_backend(
        &mut self,
        request_id: &str,
        backend: TtsBackendKind,
        out: &mut Vec<EngineMessage>,
    ) {
        if backend != TtsBackendKind::Off && !self.tts.available(backend) {
            // State unchanged — a failed selection must not silence the cast.
            out.push(command_failure(
                request_id,
                CommandErrorCode::TtsUnavailable,
                &format!("{} NPC voice backend is unavailable", backend.as_str()),
            ));
            return;
        }
        self.tts_selected = backend;
        self.capabilities.tts_selected = backend;
        if backend == TtsBackendKind::Local {
            // Pay the model-load cost now, not inside the first line.
            self.tts.warm(TtsBackendKind::Local);
        }
        out.push(EngineMessage::CommandResult {
            request_id: request_id.to_string(),
            success: true,
            error_code: None,
            message: format!("NPC voice backend set to {}", backend.as_str()),
        });
        out.push(EngineMessage::Status(StatusEvent {
            subsystem: Subsystem::Tts,
            state: STATE_SELECTED.to_string(),
            actor_id: None,
            message: Some(backend.as_str().to_string()),
            backend: Some(backend.as_str().to_string()),
        }));
    }

    // ------------------------------------------------------------- fan-out

    /// `_flush_domain_events` (`server.py:2114-2164`) + `_send_snapshot_if_changed`.
    fn flush(&mut self, now: f64, out: &mut Vec<EngineMessage>) {
        for event in self.world.drain_events() {
            match event.event_type {
                EventType::Speech => self.flush_speech(now, &event, out),
                EventType::Sound => self.flush_sound(now, &event, out),
                EventType::WorldEvent => flush_world_event(&event, out),
            }
        }
        if self.world.world_revision > self.last_snapshot_revision {
            out.push(EngineMessage::Snapshot(self.snapshot()));
            self.last_snapshot_revision = self.world.world_revision;
        }
    }

    fn flush_speech(&mut self, now: f64, event: &DomainEvent, out: &mut Vec<EngineMessage>) {
        let (Some(actor_id), Some(text), Some(position_m)) =
            (event.actor_id.clone(), event.text.clone(), event.position_m)
        else {
            return;
        };
        let Some(speaker) = self.world.characters.get(&actor_id) else {
            return;
        };
        let speaker_is_player = speaker.control() == Control::Player;
        let speaker_name_for_player = if actor_id == self.config.player_id {
            "You".to_string()
        } else {
            identify(&self.world.characters[&self.config.player_id], speaker)
        };

        let event_id = event.speech_event_id();
        out.push(EngineMessage::Speech {
            event_id: event_id.clone(),
            speaker_id: actor_id,
            target_id: event.target_id.clone(),
            text: text.clone(),
            speaker_position_m: position_m,
            recipient_ids: event.recipient_ids.clone(),
            speaker_name_for_player,
        });

        // Text first, then audio: the game's speech.rs needs the envelope before
        // the WAV it belongs to (R11). The gating predicate — voice key,
        // audibility, selection — is the router's (`_queue_tts`); what it
        // answers decides how the floor paces this line.
        let queued = self
            .speech_router
            .queue_tts(now, event, &mut speech_context!(self), out);
        if !speaker_is_player {
            // The player's own line is not something the NPCs must wait through;
            // his microphone hold is an entirely different mechanism.
            self.floor.acquire(now, &event_id, &text, queued);
        }
    }

    /// `_send_sound_event` (`server.py:2166-2221`): the player's percept, the
    /// fail-dark redaction, and the one reaction nudge.
    fn flush_sound(&mut self, now: f64, event: &DomainEvent, out: &mut Vec<EngineMessage>) {
        let (Some(sound_id), Some(position_m)) = (event.sound_id.as_deref(), event.position_m)
        else {
            return;
        };
        let Some(sound) = self.world.sound_catalog.get(sound_id).cloned() else {
            return;
        };
        let player_id = &self.config.player_id;
        // A sound whose actor has left the world is an unattributed sound.
        let actor_id = event
            .actor_id
            .as_ref()
            .filter(|id| self.world.characters.contains_key(*id));

        let player_is_actor = actor_id == Some(player_id);
        let player_is_witness = event.witness_ids.contains(player_id);
        let player_is_recipient = event.recipient_ids.contains(player_id);

        let text_for_player = if player_is_actor {
            // HUD confirmation even with nobody in range, or F feels broken —
            // the player need not be a recipient of his own sound.
            Some(match &sound.seen {
                Some(seen) => seen.replace("{actor}", "You"),
                None => sound.heard.clone(),
            })
        } else {
            match (player_is_witness, actor_id, &sound.seen) {
                (true, Some(actor_id), Some(seen)) => {
                    let name = identify(
                        &self.world.characters[player_id],
                        &self.world.characters[actor_id],
                    );
                    // The stranger label may start the sentence.
                    Some(cap_first(&seen.replace("{actor}", &name)))
                }
                _ if player_is_recipient => Some(sound.heard.clone()),
                _ => None,
            }
        };

        out.push(EngineMessage::Sound {
            event_id: event.event_id(),
            sound_id: sound.sound_id.clone(),
            sound_class: sound.sound_class.clone(),
            // Fail dark.
            actor_id: match player_is_actor || player_is_witness {
                true => actor_id.cloned(),
                false => None,
            },
            position_m,
            // From the catalog, not the event.
            audible_distance: sound.audible_distance,
            recipient_ids: event.recipient_ids.clone(),
            witness_ids: event.witness_ids.clone(),
            text_for_player,
        });

        // A percept sitting in an inbox does nothing until that actor's next
        // turn. Hand the next slot to the nearest witness (falling back to the
        // nearest mere hearer) so the reaction lands promptly. Exactly one nudge
        // per sound: the turn stream is global and single.
        for group in [&event.witness_ids, &event.recipient_ids] {
            for candidate in group {
                let is_llm = self
                    .world
                    .characters
                    .get(candidate)
                    .is_some_and(|character| character.control() == Control::Llm);
                if is_llm {
                    self.scheduler.prioritize(&self.world, candidate, true, now);
                    return;
                }
            }
        }
    }
}

fn flush_world_event(event: &DomainEvent, out: &mut Vec<EngineMessage>) {
    let Some(actor_id) = event.actor_id.clone() else {
        return;
    };
    out.push(EngineMessage::WorldEvent {
        event_id: event.event_id(),
        kind: event.kind.clone(),
        actor_id,
        target_id: event.target_id.clone(),
        item_id: event.item_id.clone(),
        recipient_ids: event.recipient_ids.clone(),
    });
}

fn scheduler_message(event: SchedulerEvent) -> EngineMessage {
    match event {
        SchedulerEvent::Status(status) => EngineMessage::Status(status),
        SchedulerEvent::Diagnostic(line) => EngineMessage::Diagnostic(line),
        SchedulerEvent::PromptExchange {
            actor_id,
            actor_name,
            prompt,
            answer,
            duration_seconds,
            error,
        } => EngineMessage::PromptExchange {
            actor_id,
            actor_name,
            prompt,
            answer,
            duration_seconds,
            error,
        },
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

/// Python's `message[:300]` — Unicode scalar values, not bytes (D11).
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
    fn message_truncation_counts_characters_not_bytes() {
        // 300 two-byte characters are 600 bytes; slicing by bytes would cut the
        // message in half (and could split a code point).
        let long = "é".repeat(400);
        assert_eq!(
            truncate_chars(&long, MAX_COMMAND_MESSAGE_CHARS)
                .chars()
                .count(),
            MAX_COMMAND_MESSAGE_CHARS
        );
        assert_eq!(truncate_chars("short", MAX_COMMAND_MESSAGE_CHARS), "short");
    }

    #[test]
    fn capabilities_derive_the_summary_flags() {
        let capabilities = Capabilities::new(true, false, true, false, false, TtsBackendKind::Off);
        assert!(capabilities.stt, "local STT alone makes STT available");
        assert!(!capabilities.tts);
        assert_eq!(Capabilities::default().tts_selected, TtsBackendKind::Off);
    }
}
