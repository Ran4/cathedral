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

use std::{
    collections::{BTreeSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};

use serde_json::{Value, json};

use crate::{
    HEARING_RADIUS_M, MAX_BELL_CATCHUP_OFFICES, MAX_MOVEMENT_CATCHUP_SLICES, MOVEMENT_TICK_SECONDS,
    actions::apply_action,
    areas::AreaMap,
    attention::{
        CuriosityConfig, IdleCognitionMode, IdleGate, Novelty, STAGE_PARTNER_MEMORY_SECONDS,
        StageConfig, WarmExchanges, on_stage,
    },
    character::{Control, StatusKind},
    clock::{Office, Weekday, WorldClock, stroke_times},
    error::{CommandError, CommandErrorCode, EngineInitError},
    event::{DomainEvent, EventType},
    floor::ConversationFloor,
    gesture::{self, GestureKind},
    ids::{ActorId, ItemId, SpeechEventId},
    math::Vec3,
    nav::NavData,
    perception::{cap_first, emit_sound, identify},
    prompt::PromptEnv,
    round::{self, Census, Round},
    scheduler::{NpcScheduler, SchedulerEvent, background_turn_order, stage_turn_order},
    seed::{WorldConfig, WorldSeed, build_world},
    snapshot::PublicSnapshot,
    sounds::SoundCatalog,
    speech_router::{SpeechContext, SpeechRouter},
    status::{STATE_UNAVAILABLE, StatusEvent, Subsystem},
    traits::{
        Cognition, Completion, Sight, SttBackendKind, Transcription, TranscriptionOutcome, Tts,
        TtsBackendKind, TtsOutcome,
    },
    weather::{
        LightningStrike, ShelterMap, WeatherConfig, WeatherKind, WeatherSample, WeatherTimeline,
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

/// Default real seconds per game day: one game day per real hour (24×), Skyrim's
/// number (`features/movement/01_the_clock.md` §9).
pub const DEFAULT_SECONDS_PER_DAY: f64 = 3_600.0;
/// Default night brightness floor — genuinely dark, lifted only by lamps and the
/// moon (`features/movement/01_the_clock.md` §5).
pub const DEFAULT_NIGHT_BRIGHTNESS: f64 = 0.05;
/// The world sound the offices ring. Audible at 600 m — most of the city — which
/// is exactly why it reaches the player as a bell but never as a percept: the
/// office is a clock, not an event (`assets/sounds/catalog.toml`,
/// `features/movement/01_the_clock.md` §7).
const TOWN_BELL_SOUND_ID: &str = "town_bell";

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
    /// Whether the round-robin lane is gated on the player's neighborhood
    /// (`features/gate_idle_cognition_on_proximity.md`).
    ///
    /// Defaults to [`IdleCognitionMode::All`] — the pre-gate behavior — so the
    /// tests and the headless runner keep exercising the whole cast without
    /// having to fake proximity. `config.ron` puts the *game* on `Stage`, and
    /// setting it back to `"all"` there is a rebuild-free A/B.
    ///
    /// It also decides the rotation's weights at construction: `All` keeps
    /// `background_turn_order`'s Major ×4 / Ambient ×0, `Stage` takes
    /// `stage_turn_order`'s flattened ones.
    pub idle_mode: IdleCognitionMode,
    /// The neighborhood itself, when `idle_mode` is [`IdleCognitionMode::Stage`].
    pub stage: StageConfig,
    /// Whether an on-stage actor also needs *news* to take an idle turn
    /// (`features/gate_idle_cognition_on_novelty.md`).
    ///
    /// Proximity decides who is worth simulating; this decides whether there is
    /// anything to simulate. Without it, standing still in a market re-asks the
    /// six people around you every three seconds whether anything has changed,
    /// and pays ~2.2k input tokens each time to be told it has not.
    ///
    /// Only consulted under [`IdleCognitionMode::Stage`]: `All` exists to
    /// reproduce the pre-gate behavior exactly, and hashing the whole city's
    /// perspective every poll would be a strange thing for the mode whose entire
    /// point is that nothing is gated.
    ///
    /// Defaults to off for the same reason `idle_mode` defaults to `All` — the
    /// tests and the headless runner keep the old behavior unless they ask for
    /// this, and `config.ron` turns it on for the game as a rebuild-free A/B.
    pub idle_requires_news: bool,
    /// Whether *unprompted* initiative is also a fact about the character
    /// (`features/gate_idle_cognition_on_novelty.md` §2).
    ///
    /// News decided whether there is anything to think about; this decides who
    /// bothers. Without it, all ~500 people you walk past think about you the
    /// moment you enter their street — one turn each rather than a rotation,
    /// which the novelty gate made affordable but no less silly.
    ///
    /// Rides on `idle_requires_news`, which is the only path that consults it,
    /// and defaults to off for the same reason: nothing but `config.ron` changes
    /// behavior that the shipped tests pin.
    pub idle_curiosity: CuriosityConfig,
    /// The world's clock — the seven offices, the week, the sun's brightness
    /// (`features/movement/01_the_clock.md`). A pure projection of the `now` the
    /// host already passes to [`Engine::poll`], so it keeps the sim clock-free.
    pub clock: WorldClock,
    /// Whether crossing an office rings the town bell for the player. On by
    /// default; the bell is a *sound*, never a percept, so it costs no tokens.
    pub ring_the_offices: bool,
    /// Pure deterministic weather authority. Presentation-only quality knobs
    /// never enter the sim config.
    pub weather: WeatherConfig,
    /// Named, navigable social shelter destinations loaded by the host.
    pub shelters: Arc<ShelterMap>,
    /// The baked walkable surface and street graph, when the host has one to
    /// give. `None` — the default — means nobody walks: every existing caller
    /// builds config as `EngineConfig { .., ..default() }`, so the whole test
    /// suite and the frozen fixtures inherit `None` and are byte-for-byte
    /// unchanged. Movement only happens once a host sets this
    /// (`features/movement/02_navigation.md`).
    pub nav: Option<Arc<NavData>>,
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
            idle_mode: IdleCognitionMode::All,
            stage: StageConfig::default(),
            idle_requires_news: false,
            idle_curiosity: CuriosityConfig::default(),
            clock: WorldClock::new(
                DEFAULT_SECONDS_PER_DAY,
                Office::Dayspring,
                0,
                DEFAULT_NIGHT_BRIGHTNESS,
            ),
            ring_the_offices: true,
            // Library embedders and frozen fixtures get the documented stable
            // clear fallback. Shipped hosts pass their explicit (enabled)
            // weather settings instead of inheriting this compatibility default.
            weather: WeatherConfig {
                enabled: false,
                ..WeatherConfig::default()
            },
            shelters: Arc::new(ShelterMap::default()),
            nav: None,
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
        /// How many units of a stack to hold out — `None` offers the whole
        /// stack. The offer UI only sets this for the coin purse, where the
        /// player picks a count (`features/food_and_items/05_the_llm_seam.md`
        /// §7); every other item stays whole-stack in v1.
        quantity: Option<u32>,
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
    /// Debug carriage write (`features/npc_bodies.md` §8): set a body status on
    /// the named character to a clamped `0..=1` value. The `cathedral-headless
    /// --status` flag and the drive-mode `status` action both arrive as this;
    /// there is no `CommandResult` (a fire-and-forget developer poke), and a
    /// name that matches nobody is a `Diagnostic`, not a fault.
    DebugSetStatus {
        name: String,
        kind: StatusKind,
        value: f64,
    },
    /// Advance the debug time scale to the next of 1× / 10× / 60× (the `T` key).
    /// Fire-and-forget: the host learns the new scale from the next
    /// [`EngineMessage::Clock`], so there is no `CommandResult`.
    CycleTimeScale,
    /// Developer forcing through the authoritative sim path.
    SetWeatherOverride {
        kind: WeatherKind,
        intensity: Option<f64>,
    },
    ClearWeatherOverride,
    DebugPlayerSay {
        request_id: String,
        text: String,
        target_id: Option<ActorId>,
        position_m: Vec3,
        spatial_seq: i64,
    },
    /// The typed-chat box (the Enter key). Unlike [`Self::DebugPlayerSay`] it
    /// is available in every mode: typing is a first-class way to speak, not a
    /// test hook. No target — like a transcribed utterance, whoever is in
    /// hearing range hears it.
    PlayerSay {
        request_id: String,
        text: String,
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

/// One mover's pose on the hot channel: where it is, which way it faces, and
/// the gait state. `f32` for the render-only scalars — the host wants them that
/// way and no sim boundary is tested against them. (`speed`/`gait_phase` are
/// carried but not yet animated: M7 shipped the crowd, not the visual gait.)
#[derive(Debug, Clone, PartialEq)]
pub struct ActorMotion {
    pub actor_id: ActorId,
    pub position_m: Vec3,
    pub facing_yaw: f64,
    pub speed: f32,
    pub gait_phase: f32,
}

/// One street lamp, as the host sees it (M7): where the post stands and whether
/// it burns. The full set rides `EngineMessage::Lamps` whenever anything about
/// it changes — twenty-odd entries, so republishing whole is cheaper than a
/// delta protocol.
#[derive(Debug, Clone, PartialEq)]
pub struct LampView {
    pub position_m: Vec3,
    pub lit: bool,
    /// The square the post stands in, e.g. `"The Wickmarket"`.
    pub square: String,
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
    /// The world clock, republished every poll so the host can drive the sun and
    /// the HUD smoothly. Cheap by design — a handful of scalars, no allocation,
    /// no world-revision bump — because the clock changes every frame and must
    /// never re-trigger the snapshot chain. The office reaches the LLM through
    /// the sheet, never here (`features/movement/01_the_clock.md` §7).
    Clock {
        /// Whole days since the epoch; day 0 is a Bellday.
        day: i64,
        /// `[0, 1)` through the day; 0.0 is midnight, 0.5 is noon.
        day_fraction: f64,
        /// The last office whose bell has rung.
        office: Office,
        /// Which of the seven weekdays.
        weekday: Weekday,
        /// 0.0 (the night floor) to 1.0 (full day) — the sun and the ladder read
        /// the same number.
        brightness: f64,
        /// The live debug time multiplier (1.0 in a normal game).
        scale: f64,
        /// Real seconds per game day, before `scale`.
        seconds_per_day: f64,
    },
    /// The current authoritative weather sample. This is a hot scalar message:
    /// it is deliberately independent of the public actor snapshot revision.
    Weather(WeatherSample),
    /// A deterministic transient crossed on the game-time weather timeline.
    Lightning(LightningStrike),
    /// The NPCs that moved this poll, on the HOT channel — republished only when
    /// at least one mover actually changed pose. Like [`EngineMessage::Clock`] it
    /// is cheap by design and must **never** bump `world_revision` or re-trigger
    /// the snapshot chain: positions change every tick, and routing them through
    /// the cold public snapshot would republish the whole world 20 times a second
    /// (`features/movement/06_engineering.md`, the hot/cold split). The host
    /// interpolates between successive samples to render smoothly.
    Movement {
        moved: Vec<ActorMotion>,
    },
    /// The squares' street lamps (M7) — the whole set, republished only when a
    /// lamp changes (the seed counts, so the host learns the positions before
    /// dusk). Like the clock, it never bumps `world_revision`: the host mirrors
    /// it into lantern props and point lights and nothing else reads it.
    Lamps {
        lamps: Vec<LampView>,
    },
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
        /// Units moved (offer/accept/eat); 1 for single-item traffic, so the HUD
        /// toast can pluralize.
        quantity: u32,
        recipient_ids: Vec<ActorId>,
    },
    /// A deliberate body-language act (`features/npc_bodies.md` §7), the
    /// transient trigger the host plays the pose from. Presented like
    /// [`Self::Speech`]: `recipient_ids` are the witnesses within the social
    /// radius, the player among them only when in range. `target_id` is `Some`
    /// only for a person target (a place-pointed gesture carries none). A
    /// looping `dance` also rides `ActorSnapshot::active_gesture`, so a player
    /// who arrives mid-loop still sees it.
    Gesture {
        event_id: String,
        actor_id: ActorId,
        kind: GestureKind,
        target_id: Option<ActorId>,
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
        /// Captured when the utterance was queued; runtime selection may have
        /// changed while synthesis was in flight.
        backend: Option<TtsBackendKind>,
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
    /// The NPC the player last exchanged a *targeted* line with, and when.
    ///
    /// The stage's second member: it keeps a conversation partner eligible for
    /// an idle turn after the player has backed out of `stage.radius_m`
    /// mid-exchange. Speech is the whole signal — the player addressed them, or
    /// they addressed the player — and it lapses after
    /// [`STAGE_PARTNER_MEMORY_SECONDS`], because a partner who never expired
    /// would keep one NPC thinking in an empty field for the rest of the run.
    ///
    /// Broadcast lines need no entry here: to reach the player at all they came
    /// from inside the 20 m hearing radius, which the stage radius contains.
    last_player_exchange: Option<(ActorId, f64)>,
    /// The warm NPC↔NPC exchanges, pair-keyed — the same courtesy the slot
    /// above extends to the player, generalized to the rest of the cast: while
    /// a pair is warm the round holds both of them where they stand
    /// (`features/npcs_stop_walking_when_talking_to_each_other.md`). Kept
    /// separate from `last_player_exchange` so the player pair's behaviour —
    /// the stage's reserved seat, the hot-channel snapshot — stays exactly as
    /// it was.
    npc_exchanges: WarmExchanges,
    /// What each on-stage actor had already been told when they last thought.
    ///
    /// The stage's third question, after "who is near?" and "who is talking to
    /// me?": *has anything happened to them since?* Empty and inert unless
    /// `config.idle_requires_news`.
    novelty: Novelty,
    /// The live world clock. Initialized from `config.clock`; the debug time
    /// scale (the `T` key) mutates this copy, never the config's.
    clock: WorldClock,
    weather: WeatherTimeline,
    last_weather_days: f64,
    last_weather_sample: WeatherSample,
    /// The `now` at which the offices were last checked for a bell, so a span —
    /// never an instant — is tested and no office can be missed or double-rung.
    last_clock_now: f64,
    /// Future bell strokes owed to the player, in real `now`-seconds. An office
    /// enqueues its ordinal here (the Watch one, the Snuffing seven) and each
    /// poll drains the ones now due.
    bell_strokes: VecDeque<f64>,
    /// Monotonic id for the bell sound events, so each stroke is distinct.
    bell_seq: u64,
    /// The `now` up to which movement has been stepped, so a fixed 20 Hz slice —
    /// never a variable frame — decides how far anyone walks. Starts at the
    /// construction `now`, like `last_clock_now`.
    movement_now: f64,
    /// The M3 water round: thirst, the behaviour ladder, the queues at the wells.
    /// Empty and inert unless the host supplied a nav graph
    /// (`features/movement/03_the_ladder.md`).
    round: Round,
    /// The round's lamp revision as of the last `Lamps` publish, so the set is
    /// resent exactly when something changed. 0 = never sent; the seed puts the
    /// round at 1, so the first poll always announces the posts.
    lamp_revision_sent: u64,
    /// Diagnostics raised at construction (the M2 mover seeding), emitted with
    /// the first poll's `Ready` because `Engine::new` has no `out` to push to.
    startup_diagnostics: Vec<String>,
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
        area_map: AreaMap,
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
                area_map,
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

        // The street graph is world context (like the area map): `go_to` prices
        // and validates its route against it at intent time (M5).
        world.nav = config.nav.clone();
        world.shelters = config.shelters.clone();
        let weather = WeatherTimeline::new(config.weather);
        let last_weather_days = config.clock.game_days(now);
        let last_weather_sample = weather.sample(last_weather_days);
        world.current_weather = Some(last_weather_sample);

        // The daily round (M4, subsuming M3's water and M2's hard-coded pacer):
        // homes, workplaces, the ladder, the queues — and, since M5, the
        // wayfinding registry and each townsperson's `places_you_know`. It only
        // runs if the host gave us a nav graph — the frozen fixtures pass `None`
        // and nobody moves — and reports rather than panics on anything that
        // does not resolve; the notices ride out with the first poll's `Ready`.
        let mut startup_diagnostics: Vec<String> = Vec::new();
        let mut round = Round::new();
        if let Some(nav) = config.nav.as_deref() {
            startup_diagnostics.extend(round.seed(&mut world, nav, now, &config.clock));
        }

        let mut capabilities = capabilities;
        // One source of truth for the selection: the config's, which the host
        // has already run through the availability fallback.
        capabilities.tts_selected = config.tts_selected;

        // The rotation is frozen at construction, so the weights are chosen here
        // — and the gate changes which weights are *right*. Ungated, the
        // rotation rations scarce global compute across 500 people and ambient
        // NPCs get none of it. Gated, it hands out turns among the six people in
        // front of the player, where the ambient market crowd is most of the
        // scene (attention.rs, `stage_turn_order`).
        let order = match config.idle_mode {
            IdleCognitionMode::All => background_turn_order(&world),
            IdleCognitionMode::Stage => stage_turn_order(&world),
        };
        let mut scheduler = NpcScheduler::new(
            order,
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
        // Captured before `config` is moved into the struct below (`WorldClock`
        // is `Copy`); the live clock starts as the configured one.
        let clock = config.clock;

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
            last_player_exchange: None,
            npc_exchanges: WarmExchanges::default(),
            novelty: Novelty::default(),
            clock,
            weather,
            last_weather_days,
            last_weather_sample,
            // The construction `now`: the first poll's span opens here, so the
            // office the run *starts* in is never rung, only entered.
            last_clock_now: now,
            bell_strokes: VecDeque::new(),
            bell_seq: 0,
            // The first movement span opens at construction, mirroring the clock.
            movement_now: now,
            round,
            lamp_revision_sent: 0,
            startup_diagnostics,
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
            // Any construction-time notice (the M2 mover seeding) rides out here,
            // since `Engine::new` had no `out` to push it to.
            for line in std::mem::take(&mut self.startup_diagnostics) {
                out.push(EngineMessage::Diagnostic(line));
            }
        }

        // Ring the offices before commands, so a `CycleTimeScale` this same poll
        // cannot retroactively move the bell span already tested here. The clock
        // itself is published at the end of the poll, where it reflects that
        // command.
        self.ring_offices(now, &mut out);
        self.update_weather(now, true, &mut out);

        // Step the movers on the fixed slice before this poll's stage is
        // computed, so `context_hash` and `characters_within` see where everyone
        // stands right now. Positions ride the hot channel below, never the cold
        // snapshot the scheduler block emits.
        self.tick_movement(now, &mut out);

        // Drive the water round on the same beat: decay thirst, run the ladder,
        // work the queues at the wells. It sets the routes the *next*
        // `tick_movement` walks and buffers any well sounds into `world.events`,
        // which `flush` fans out below — so, like the mover pipeline, it needs a
        // nav graph and is otherwise inert.
        if let Some(nav) = self.config.nav.clone() {
            // Everyone in a warm exchange (each pair warm for the same 30 s the
            // stage reserves the player's partner a seat) keeps their round on
            // hold: no new errand walks them away mid-exchange.
            let mut in_conversation = self.npc_exchanges.warm_actors(now);
            if let Some(partner) = self.conversation_partner(now) {
                in_conversation.insert(partner.clone());
            }
            let nudges = round::tick(
                &mut self.round,
                &mut self.world,
                &nav,
                &self.clock,
                now,
                &self.config.player_id,
                &in_conversation,
            );
            // A `go_to` arrival or lapse grants the same priority handoff an
            // addressed `say` does — off stage there is no idle rotation to
            // render the percept, and without the nudge the errand chain dies
            // silently (05_the_llm_seam.md §3). Not immediate: the inter-turn
            // delay and the floor still govern when, only selection changes.
            for actor_id in nudges {
                self.scheduler
                    .prioritize(&self.world, &actor_id, false, now);
            }
            let departed = self.round.drain_departed();
            if !departed.is_empty() {
                self.scheduler.actors_departed(&departed);
                self.novelty.forget(&departed);
                self.npc_exchanges.forget(&departed);
                if self
                    .last_player_exchange
                    .as_ref()
                    .is_some_and(|(actor, _)| departed.contains(actor))
                {
                    self.last_player_exchange = None;
                }
            }
            // The lamp channel (M7): republish the set exactly when a lamp
            // changed — the seed, a lighting, the dawn snuff.
            if self.round.lamp_revision() != self.lamp_revision_sent {
                self.lamp_revision_sent = self.round.lamp_revision();
                out.push(EngineMessage::Lamps {
                    lamps: self
                        .round
                        .lamps()
                        .iter()
                        .map(|lamp| LampView {
                            position_m: lamp.position,
                            lit: lamp.lit,
                            square: lamp.square.clone(),
                        })
                        .collect(),
                });
            }
        }

        let mut completions: Vec<Completion> = Vec::new();
        for command in commands {
            self.apply_command(now, command, &mut completions, &mut out);
        }

        // Computed once per poll (D20): the floor's expiry purge is a side
        // effect, and the scheduler must not be able to change how often it runs.
        let floor_busy = if self.scheduler.in_flight_is_player_reaction() {
            self.floor.busy_for_player_reaction(now)
        } else {
            self.floor.busy(now)
        };
        // The stage is computed here for the same reason, and it answers three
        // questions in order. Who is close enough to be worth a thought? Nobody
        // in the empty field behind you is. Then: has anything happened to them
        // since they last thought? A man standing beside you with nothing to
        // react to costs a full prompt to say `wait {}`, and saying it changes
        // nothing — so the next poll would ask him again. Silence is the one
        // thing that must be free. And last: is this a person who would say
        // something about it unbidden? Most are not, and a street where they all
        // are is not a street. The third question is never asked of somebody who
        // was *spoken to* (`Novelty::admits_idle`) — an aloof NPC never opens,
        // but always answers.
        let stage = match self.config.idle_mode {
            IdleCognitionMode::All => None,
            IdleCognitionMode::Stage => {
                let mut stage = on_stage(
                    &self.world,
                    &self.config.player_id,
                    self.conversation_partner(now),
                    &self.config.stage,
                );
                if self.config.idle_requires_news {
                    self.novelty.observe(now, &stage);
                    stage.retain(|actor_id| {
                        self.novelty
                            .admits_idle(&self.world, actor_id, &self.config.idle_curiosity)
                    });
                }
                Some(stage)
            }
        };
        let idle = if self.speech_router.player_composing() {
            IdleGate::Suppressed
        } else {
            match &stage {
                None => IdleGate::All,
                Some(stage) => IdleGate::Stage(stage),
            }
        };
        let events = self.scheduler.poll(
            now,
            &mut self.world,
            &mut self.transcript,
            &mut completions,
            floor_busy,
            idle,
            self.cognition.as_mut(),
            &self.env,
        );
        // A prompt has just gone out showing somebody the world as it stands, so
        // nothing in it is news to them any more. Taken unconditionally to keep
        // the slot drained; recorded only when the gate is on.
        //
        // Every lane counts, not just the idle one: an NPC who has just answered
        // the player has seen exactly what an idle turn would have shown him,
        // and must not then be handed one for the privilege.
        if let Some(actor_id) = self.scheduler.take_submitted()
            && self.config.idle_requires_news
        {
            self.novelty.told(now, &self.world, &actor_id);
        }
        for event in events {
            out.push(scheduler_message(event));
        }

        // Grace windows and held transcripts: the two deadlines nobody else
        // watches. A resolution here can apply a player `say`, whose events the
        // flush below fans out.
        self.speech_router
            .poll(now, &mut speech_context!(self), &mut out);

        // Stamp and expire looping gestures before the flush, so a loop that
        // ran out this poll bumps the revision the flush then publishes.
        self.expire_gestures(now);

        // The scheduler's turn produced domain events in this same poll; the
        // floor they acquire here gates the *next* one.
        self.flush(now, &mut out);

        // Last: the clock, reflecting any `CycleTimeScale` applied above.
        self.publish_clock(now, &mut out);
        out
    }

    pub fn snapshot(&self) -> PublicSnapshot {
        let mut snapshot = self.world.public_snapshot(&self.config.player_id);
        snapshot.road_carts = self.round.road_carts(&self.world);
        snapshot
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

    /// The daily round (M4, subsuming M3's water), for tests and tracing.
    pub fn round(&self) -> &Round {
        &self.round
    }

    /// A one-line census of the water round for `--trace-water`.
    pub fn water_summary(&self) -> String {
        self.round.water_summary(&self.world)
    }

    /// A one-line census of the cast's hunger for `--trace-food` (food & items M2).
    pub fn food_summary(&self) -> String {
        self.round.food_summary(&self.world)
    }

    /// Drain the economy's `[food]` trace: legacy restock, sales, road traffic,
    /// transforms, and household settlement. Buffered by the round each
    /// poll and drained here by the host; the game host never calls it, so the
    /// buffer stays capped.
    pub fn drain_food_log(&mut self) -> Vec<String> {
        self.round.drain_food_log()
    }

    /// A behavioural census of the enrolled cast for `--census-by-area`.
    pub fn round_census(&self, now: f64) -> Census {
        self.round.census(&self.world, &self.clock, now)
    }

    /// The microphone/voice state machines, for tests and diagnostics.
    pub fn speech_router(&self) -> &SpeechRouter {
        &self.speech_router
    }

    /// The NPC the player is currently in an exchange with, while it is still
    /// warm — the stage's reserved seat (see [`Engine::last_player_exchange`]).
    pub fn conversation_partner(&self, now: f64) -> Option<&ActorId> {
        self.last_player_exchange
            .as_ref()
            .filter(|(_, spoke_at)| now - spoke_at < STAGE_PARTNER_MEMORY_SECONDS)
            .map(|(actor_id, _)| actor_id)
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
                quantity,
                position_m,
                spatial_seq,
            } => {
                // A `None` quantity offers the whole stack — the arg is simply
                // omitted, exactly as an LLM `offer_item` without `quantity`.
                let mut args = json!({"item_id": item_id.as_str(), "target": target_id.as_str()});
                if let Some(quantity) = quantity {
                    args["quantity"] = json!(quantity);
                }
                self.player_action(
                    now,
                    &request_id,
                    "offer_item",
                    args,
                    Some((spatial_seq, position_m)),
                    out,
                )
            }

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

            EngineCommand::PlayerSay {
                request_id,
                text,
                position_m,
                spatial_seq,
            } => self.player_say(now, &request_id, &text, position_m, spatial_seq, out),

            EngineCommand::PlayerSound { sound_id } => self.player_sound(now, &sound_id, out),

            EngineCommand::DebugSound {
                sound_id,
                position_m,
            } => self.debug_sound(now, &sound_id, position_m, out),

            EngineCommand::DebugSetStatus { name, kind, value } => {
                self.debug_set_status(now, &name, kind, value, out)
            }

            // Continuity-preserving (see `WorldClock::with_scale`): time speeds
            // up without jumping. The next poll's `Clock` message carries the new
            // scale to the HUD.
            EngineCommand::CycleTimeScale => {
                self.clock = self.clock.cycle_scale(now);
                out.push(EngineMessage::Diagnostic(format!(
                    "[clock] debug time scale ×{}",
                    self.clock.scale()
                )));
            }

            EngineCommand::SetWeatherOverride { kind, intensity } => {
                let game_days = self.clock.game_days(now);
                self.weather.set_override(kind, intensity, game_days);
                self.update_weather(now, false, out);
            }

            EngineCommand::ClearWeatherOverride => {
                self.weather.clear_override();
                self.update_weather(now, false, out);
            }

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

    /// The typed-chat `say` (the Enter box), available in every mode. From the
    /// applied `say` onward this is the transcription path
    /// (`speech_router::resolve_transcription`): full sim validation, and being
    /// heard is followed by the earliest possible reaction — the nearest LLM
    /// listener takes the next protected player-reaction slot.
    fn player_say(
        &mut self,
        now: f64,
        request_id: &str,
        text: &str,
        position_m: Vec3,
        spatial_seq: i64,
        out: &mut Vec<EngineMessage>,
    ) {
        let result = self.apply_player_action(
            "say",
            &json!({"text": text}),
            Some((spatial_seq, position_m)),
        );
        if result.is_ok() {
            let player_id = self.config.player_id.clone();
            let utterance_position = self.world.characters[&player_id].position_m();
            let nearest = self
                .world
                .characters_within(utterance_position, HEARING_RADIUS_M, Some(&player_id))
                .into_iter()
                .find(|candidate| {
                    self.world
                        .characters
                        .get(candidate)
                        .is_some_and(|character| character.control() == Control::Llm)
                });
            if let Some(nearest) = nearest {
                self.scheduler
                    .prioritize_player_reaction(&self.world, &nearest, now);
            }
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

    /// Debug carriage write (`features/npc_bodies.md` §8): set a body status on
    /// the named character. The single sim entry point the `cathedral-headless
    /// --status` flag and the drive-mode `status` action share. It bumps the
    /// world revision (via [`World::debug_set_status`]) so the flushed snapshot
    /// carries the new value; a name that matches nobody is a diagnostic.
    fn debug_set_status(
        &mut self,
        now: f64,
        who: &str,
        kind: StatusKind,
        value: f64,
        out: &mut Vec<EngineMessage>,
    ) {
        if self.world.debug_set_status(who, kind, value) {
            self.flush(now, out);
        } else {
            // A poke at nobody is a logged Diagnostic (host: `logs.jsonl` source
            // `engine` + stderr; headless: stderr), never a fault — a typo must
            // not abort a drive script or a headless run.
            out.push(EngineMessage::Diagnostic(format!(
                "[smart actors] invalid debug_status: no character with the name or id '{who}'"
            )));
        }
    }

    /// Ring the offices whose bells fell in the span since the last check, and
    /// sound any strokes now due. Called once per poll, before commands, so the
    /// span tested here is stable no matter what the commands do to the scale.
    fn ring_offices(&mut self, now: f64, out: &mut Vec<EngineMessage>) {
        // Publish the resolved time onto the world before this poll's turns are
        // rendered, so `you_are.the_hour` reads the current office. A plain field
        // write: it must not bump `world_revision`. Cycling the scale preserves
        // `at(now)`, so a `CycleTimeScale` later this poll cannot change it.
        self.world.current_time = Some(self.clock.at(now));

        if self.config.ring_the_offices && self.world.sounds_enabled {
            let mut crossed = self.clock.offices_crossed(self.last_clock_now, now);
            // Bound the catch-up like `tick_movement` does: a huge `now` jump —
            // a resume from a long pause, or a hitch at a high debug scale —
            // crosses many offices at once, and ringing every skipped stroke in
            // one poll is a wall of bells nobody can count. Keep only the most
            // recent [`MAX_BELL_CATCHUP_OFFICES`] crossings, and drop the queued
            // backlog with the rest — those strokes belong to offices older
            // still than the ones being skipped.
            if crossed.len() > MAX_BELL_CATCHUP_OFFICES {
                crossed.drain(..crossed.len() - MAX_BELL_CATCHUP_OFFICES);
                self.bell_strokes.clear();
            }
            let mut queued_any = false;
            for (instant, office) in crossed {
                for stroke_at in stroke_times(office, instant) {
                    self.bell_strokes.push_back(stroke_at);
                    queued_any = true;
                }
            }
            // Adjacent offices' strokes interleave in time — High Wick's fourth
            // stroke (noon + 9 s) falls *after* the Waning's first (three game
            // hours later, only ~7.5 s away at 60 s/day) — and a single poll span
            // can cross more than one office at a high debug scale or a slow
            // frame. A plain per-office FIFO then strands one office's late
            // strokes ahead of another's already-due ones, so the front-only
            // drain below rings them delayed and clumped instead of on the
            // three-second beat. Keep the queue ordered by due time so the
            // earliest owed stroke is always at the front.
            if queued_any {
                self.bell_strokes
                    .make_contiguous()
                    .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
        self.last_clock_now = now;

        // Sound every stroke now due. `offices_crossed` never returns a future
        // instant, so an office's first stroke rings the poll it is scheduled and
        // the rest ring their own later polls, three seconds apart. The queue is
        // due-time ordered, so the front is always the earliest owed stroke.
        while self.bell_strokes.front().is_some_and(|&due| due <= now) {
            self.bell_strokes.pop_front();
            self.emit_bell(out);
        }
    }

    /// Sample one weather answer before deterministic behaviour runs. The
    /// scalar is hot; only semantic transitions are logged, and neither path
    /// touches `world_revision` or actor inboxes.
    fn update_weather(
        &mut self,
        now: f64,
        include_crossed_lightning: bool,
        out: &mut Vec<EngineMessage>,
    ) {
        let game_days = self.clock.game_days(now);
        if include_crossed_lightning {
            let strikes = self
                .weather
                .lightning_crossed(self.last_weather_days, game_days);
            for strike in &strikes {
                self.round.note_lightning(&self.world, strike, now);
            }
            out.extend(strikes.into_iter().map(EngineMessage::Lightning));
        }
        let sample = self.weather.sample(game_days);
        if sample.semantic_revision != self.last_weather_sample.semantic_revision {
            let time = self.clock.at(now);
            let (hour, minute) = time.hour_minute();
            out.push(EngineMessage::Diagnostic(format!(
                "[weather] day {} {hour:02}:{minute:02} {} -> {}, wind {:.1} m/s {}, visibility {:.0} m",
                time.day,
                self.last_weather_sample.kind,
                sample.kind,
                sample.wind_speed_mps(),
                sample.wind_from_label(),
                sample.visibility_m,
            )));
        }
        self.world.current_weather = Some(sample);
        self.last_weather_sample = sample;
        self.last_weather_days = game_days;
        out.push(EngineMessage::Weather(sample));
    }

    /// Advance every mover by whole [`MOVEMENT_TICK_SECONDS`] slices up to `now`,
    /// then publish the ones that moved on the hot channel. A no-op with no nav.
    ///
    /// Fixed slices, not the poll's variable span, so a slow frame or a fast one
    /// never changes how far anyone walks; [`MAX_MOVEMENT_CATCHUP_SLICES`] bounds
    /// a resume-from-pause so a huge `now` jump snaps forward instead of spinning.
    fn tick_movement(&mut self, now: f64, out: &mut Vec<EngineMessage>) {
        let Some(nav) = self.config.nav.clone() else {
            return;
        };

        // Where the player stands, for the on-stage separation steering — the
        // same "near the player" the attention gate uses, so there is one
        // answer, not three (`features/movement/06_engineering.md` §4).
        let stage = self
            .world
            .characters
            .get(&self.config.player_id)
            .map(|player| player.position_m());

        let mut moved_ids: BTreeSet<ActorId> = BTreeSet::new();
        let mut slices = 0usize;
        while self.movement_now + MOVEMENT_TICK_SECONDS <= now
            && slices < MAX_MOVEMENT_CATCHUP_SLICES
        {
            for id in self.world.step_movement(MOVEMENT_TICK_SECONDS, &nav, stage) {
                moved_ids.insert(id);
            }
            self.movement_now += MOVEMENT_TICK_SECONDS;
            slices += 1;
        }
        // Overflowed the catch-up budget: drop the backlog and snap forward.
        if self.movement_now + MOVEMENT_TICK_SECONDS <= now {
            self.movement_now = now;
        }

        if moved_ids.is_empty() {
            return;
        }
        let moved = moved_ids
            .into_iter()
            .map(|actor_id| {
                let character = &self.world.characters[&actor_id];
                ActorMotion {
                    position_m: character.position_m(),
                    facing_yaw: character.facing_yaw(),
                    speed: character.speed() as f32,
                    gait_phase: character
                        .state
                        .movement
                        .as_ref()
                        .map_or(0.0, |movement| movement.gait_phase)
                        as f32,
                    actor_id,
                }
            })
            .collect();
        out.push(EngineMessage::Movement { moved });
    }

    /// Republish the clock for the host's sun and HUD — a handful of scalars,
    /// every poll, so the day reads smoothly and no world-revision is bumped.
    /// Emitted at the end of the poll so it reflects a `CycleTimeScale` applied
    /// this frame.
    fn publish_clock(&self, now: f64, out: &mut Vec<EngineMessage>) {
        let time = self.clock.at(now);
        out.push(EngineMessage::Clock {
            day: time.day,
            day_fraction: time.fraction,
            office: time.office,
            weekday: time.weekday,
            brightness: self.clock.brightness(now),
            scale: self.clock.scale(),
            seconds_per_day: self.clock.seconds_per_day(),
        });
    }

    /// Ring one stroke of the town bell — a sound *for the player only*, from the
    /// Lanthorn. It reaches the player's ears (its lone recipient) but no LLM
    /// inbox and nudges nobody: the office is a clock, not an event, so it queues
    /// nothing and costs no tokens (`features/movement/01_the_clock.md` §7).
    /// Deviations — the Ruin, the name-knell — stay real percepts, emitted the
    /// ordinary way.
    fn emit_bell(&mut self, out: &mut Vec<EngineMessage>) {
        let Some(sound) = self.world.sound_catalog.get(TOWN_BELL_SOUND_ID).cloned() else {
            return;
        };
        self.bell_seq += 1;
        out.push(EngineMessage::Sound {
            event_id: format!("bell-{}", self.bell_seq),
            sound_id: sound.sound_id.clone(),
            sound_class: sound.sound_class.clone(),
            // A world sound is never attributed to anyone.
            actor_id: None,
            // The Lanthorn, near the heart of the city.
            position_m: Vec3::new(0.0, 20.0, -10.0),
            audible_distance: sound.audible_distance,
            // The player alone: the host plays the sound for him, and no
            // character is given a percept or handed a turn.
            recipient_ids: vec![self.config.player_id.clone()],
            witness_ids: Vec::new(),
            // No toast; the persistent HUD readout already shows the office.
            text_for_player: None,
        });
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
    /// Stamp a looping gesture's 60 s deadline on its first sighting — the
    /// action layer has no clock, exactly like a `TravelIntent`'s deadline
    /// (`character.rs`) — and clear any that has run out. The other end of the
    /// rule ("until the actor's next non-`wait` action") is enforced in
    /// `actions.rs`; a loop cleared here bumps the public revision so the next
    /// snapshot drops it and the host stops the dance.
    fn expire_gestures(&mut self, now: f64) {
        let mut cleared = false;
        for character in self.world.characters.values_mut() {
            let Some(active) = character.state.active_gesture.as_mut() else {
                continue;
            };
            match active.deadline {
                None => active.deadline = Some(now + gesture::DANCE_MAX_SECONDS),
                Some(deadline) if now >= deadline => {
                    character.state.active_gesture = None;
                    cleared = true;
                }
                Some(_) => {}
            }
        }
        if cleared {
            self.world.touch_public_state();
        }
    }

    fn flush(&mut self, now: f64, out: &mut Vec<EngineMessage>) {
        for event in self.world.drain_events() {
            match event.event_type {
                EventType::Speech => self.flush_speech(now, &event, out),
                EventType::Sound => self.flush_sound(now, &event, out),
                EventType::WorldEvent => {
                    self.hold_for_handoff(now, &event);
                    flush_world_event(&event, out);
                }
                EventType::Gesture => flush_gesture(&event, out),
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
        let Some(speaker) = self
            .world
            .characters
            .get(&actor_id)
            .filter(|_| self.world.is_present(&actor_id))
        else {
            return;
        };
        let speaker_is_player = speaker.control() == Control::Player;
        let player_can_hear = event.recipient_ids.contains(&self.config.player_id);
        let speaker_name_for_player = if actor_id == self.config.player_id {
            "You".to_string()
        } else {
            identify(&self.world.characters[&self.config.player_id], speaker)
        };

        // Who the player is talking with, from the only signal that means it: a
        // line one of them addressed to the other. It survives him walking out
        // of the stage radius, and it lapses on its own.
        //
        // The partner also stops walking on the spot — before the next movement
        // slice, or a round errand started this same poll would carry them out
        // of interaction range while the words are still in the air. The round's
        // tick keeps them held for as long as the exchange stays warm.
        if speaker_is_player {
            if let Some(target_id) = &event.target_id {
                self.last_player_exchange = Some((target_id.clone(), now));
                round::interrupt_for_conversation(&mut self.round, &mut self.world, target_id);
            }
        } else if event.target_id.as_ref() == Some(&self.config.player_id) {
            self.last_player_exchange = Some((actor_id.clone(), now));
            round::interrupt_for_conversation(&mut self.round, &mut self.world, &actor_id);
        } else if let Some(target_id) = &event.target_id {
            // An NPC→NPC targeted line gets the same courtesy, pair-keyed:
            // both stop walking — the speaker may themselves be mid-errand —
            // and the round holds both while the exchange stays warm.
            // Broadcast lines fall through all three branches and hold nobody.
            self.npc_exchanges.note(&actor_id, target_id, now);
            round::interrupt_for_conversation(&mut self.round, &mut self.world, &actor_id);
            round::interrupt_for_conversation(&mut self.round, &mut self.world, target_id);
        }

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
            self.floor
                .acquire_scoped(now, &event_id, &text, queued, player_can_hear);
        }
    }

    /// A targeted item handoff is at least as conversation-shaped as a line —
    /// it is the fish-and-coin-at-full-stride case this exists for — so
    /// `offer_item` / `accept_offered_item` stop giver and receiver exactly as
    /// a targeted line does (`require_interaction_range` already forces
    /// proximity; this adds *standing*). An untargeted "to anyone" offer, like
    /// a broadcast line, holds nobody.
    ///
    /// A handoff with the player warms no pair: the player slot's machinery is
    /// speech-only and stays byte-identical — but the NPC party still stops on
    /// the spot for the exchange itself.
    fn hold_for_handoff(&mut self, now: f64, event: &DomainEvent) {
        if !is_handoff_kind(&event.kind) {
            return;
        }
        let (Some(actor_id), Some(target_id)) = (&event.actor_id, &event.target_id) else {
            return;
        };
        let player_id = &self.config.player_id;
        if actor_id != player_id && target_id != player_id {
            self.npc_exchanges.note(actor_id, target_id, now);
        }
        round::interrupt_for_conversation(&mut self.round, &mut self.world, actor_id);
        round::interrupt_for_conversation(&mut self.round, &mut self.world, target_id);
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
            .filter(|id| self.world.is_present(id));

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

/// The world-event kinds that stop both parties like a targeted line. The
/// round's presentation-only events (`sale`) are deliberately not here:
/// the silent stall purchase already parks the buyer itself, and priming the
/// scheduler off it would spend LLM turns on a zero-token act (npc_bodies M2).
fn is_handoff_kind(kind: &str) -> bool {
    matches!(kind, "offer_item" | "accept_offered_item")
}

fn flush_gesture(event: &DomainEvent, out: &mut Vec<EngineMessage>) {
    // `kind` is the gesture verb string the action wrote; an unknown one is
    // impossible (the action validated it), so a miss is silently dropped.
    let (Some(actor_id), Some(kind)) =
        (event.actor_id.clone(), GestureKind::from_verb(&event.kind))
    else {
        return;
    };
    out.push(EngineMessage::Gesture {
        event_id: event.event_id(),
        actor_id,
        kind,
        target_id: event.target_id.clone(),
        recipient_ids: event.recipient_ids.clone(),
    });
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
        quantity: event.quantity,
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

    /// npc_bodies M2 purity contract: the deliberate offer verbs hold both
    /// parties for the exchange, but the round's presentation-only `sale`
    /// must never trip the handoff (no interrupt, no NPC-exchange warmth, and —
    /// because `flush` only nudges the scheduler for sounds — no turn either).
    #[test]
    fn sale_is_not_a_handoff_kind() {
        assert!(is_handoff_kind("offer_item"));
        assert!(is_handoff_kind("accept_offered_item"));
        assert!(!is_handoff_kind("sale"));
        assert!(!is_handoff_kind("decline_offer"));
        assert!(!is_handoff_kind("retract_offer"));
        assert!(!is_handoff_kind("eat"));
    }
}
