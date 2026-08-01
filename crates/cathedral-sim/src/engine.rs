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
//! snapshot inline, *before* their `CommandResult`, so a side effect that landed
//! reaches the game even when the action it came with fails
//! (`server.py:1013-1025`). A player *position* update is the exception: it
//! rides the hot channel and no longer bumps the revision at all
//! ([`World::update_positions`]), so it neither needs nor triggers that flush.

use std::{
    collections::{BTreeSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};

use serde_json::{Value, json};

use crate::{
    HEARING_RADIUS_M, MAX_BELL_CATCHUP_OFFICES, MAX_MOVEMENT_CATCHUP_SLICES, MOVEMENT_TICK_SECONDS,
    OFFER_LAPSE_RADIUS_M, PLACE_ARRIVE_RADIUS_M,
    actions::{self, apply_action},
    areas::AreaMap,
    attention::{
        CuriosityConfig, IdleCognitionMode, IdleGate, Novelty, STAGE_PARTNER_MEMORY_SECONDS,
        StageConfig, WarmExchanges, on_stage,
    },
    character::{BodySlot, Control, GutEntry, IntentTarget, PocketedUnit, StatusKind, TravelIntent},
    clock::{Office, Weekday, WorldClock, stroke_times},
    custody,
    error::{CommandError, CommandErrorCode, EngineInitError},
    event::{DomainEvent, EventType},
    floor::ConversationFloor,
    gesture::{self, GestureKind},
    ids::{ActorId, ItemId, SpeechEventId},
    inventory::StockSpec,
    item::{CONDITION_METADATA_KEY, CONDITION_POOPSTAINED, POOP_KIND},
    math::Vec3,
    nav::NavData,
    night::{NightGate, NightOffice, NightOfficeConfig, stage_occupied},
    notices,
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
/// number (`features/implemented/movement/01_the_clock.md` §9).
pub const DEFAULT_SECONDS_PER_DAY: f64 = 3_600.0;
/// Default night brightness floor — genuinely dark, lifted only by lamps and the
/// moon (`features/implemented/movement/01_the_clock.md` §5).
pub const DEFAULT_NIGHT_BRIGHTNESS: f64 = 0.05;
/// The world sound the offices ring. Audible at 600 m — most of the city — which
/// is exactly why it reaches the player as a bell but never as a percept: the
/// office is a clock, not an event (`assets/sounds/catalog.toml`,
/// `features/implemented/movement/01_the_clock.md` §7).
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
    /// (`features/implemented/movement/01_the_clock.md`). A pure projection of the `now` the
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
    /// (`features/implemented/movement/02_navigation.md`).
    pub nav: Option<Arc<NavData>>,
    /// The Night Office (movement M6): the second cognition lane, and the three
    /// tiers it serves ([`crate::night`]).
    ///
    /// Defaults to **off**, like every gate before it, so the whole test suite,
    /// the frozen fixtures and any library embedder keep their exact behaviour
    /// until they ask for a night. `config.ron` turns it on for the game and
    /// `--night-office` for the headless runner.
    pub night_office: NightOfficeConfig,
    /// Whether the authored street-dog pack is seeded ([`crate::dogs`]).
    ///
    /// Defaults to **on** — dogs cost no tokens and no snapshot traffic, so
    /// unlike the cognition gates there is nothing to protect by default-off —
    /// but they only exist in a world with a nav graph, which is why every
    /// nav-less test and the frozen fixtures are unchanged by the default.
    /// `config.ron: smart_actors.dogs_enabled` and `CATHEDRAL_NO_DOGS` turn
    /// them off for ablation.
    pub dogs_enabled: bool,
    /// Whether hands may chalk the walls ([`crate::marks`]).
    ///
    /// Defaults to **on**, like the dogs and for the same reason: marks cost
    /// no tokens, and the layer is inert until something writes one — every
    /// nav-less test and every frozen fixture keeps a bare wall either way.
    /// `config.ron: smart_actors.marks.enabled` and `CATHEDRAL_NO_MARKS` turn
    /// them off for ablation.
    pub marks_enabled: bool,
    /// The per-kind chalk switches, so one writer can be silenced without
    /// losing the medium ([`crate::marks::MarkKindSwitches`]).
    pub mark_kinds: crate::marks::MarkKindSwitches,
    /// Multiplies elapsed time in the chalk decay. `1.0` in the game; a test
    /// or a drive run raises it to weather a wall in seconds instead of days.
    pub marks_decay_scale: f64,
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
            night_office: NightOfficeConfig::default(),
            dogs_enabled: true,
            marks_enabled: true,
            mark_kinds: crate::marks::MarkKindSwitches::default(),
            marks_decay_scale: 1.0,
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
    /// The player's own inventory verbs (`features/extra_pockets.md` M1): the
    /// right-click menu's counterparts to the LLM's `pocket_item`/`retrieve_item`
    /// /`swallow`/`gargle`/`expel`/`eat`. All position-free — what you do with
    /// your own body needs nobody else nearby — except [`Self::PlayerSpit`].
    PlayerPocket {
        request_id: String,
        item_id: ItemId,
        slot: BodySlot,
    },
    PlayerRetrieve {
        request_id: String,
        item_id: ItemId,
    },
    PlayerSwallow {
        request_id: String,
        item_id: ItemId,
    },
    /// Spitting is aimed at somebody within 4 m, so it carries a position like
    /// an offer does.
    PlayerSpit {
        request_id: String,
        item_id: ItemId,
        target_id: ActorId,
        position_m: Vec3,
        spatial_seq: i64,
    },
    PlayerGargle {
        request_id: String,
        item_id: ItemId,
    },
    PlayerExpel {
        request_id: String,
    },
    /// Eating through the same verb the cast uses — the player's hunger is not
    /// modelled, but the item is consumed and the gut clock starts either way.
    PlayerEat {
        request_id: String,
        item_id: ItemId,
    },
    /// The three things the host's grab reflex tells the sim
    /// (`law_and_order.md` M4c/M4d). The reflex itself is host-side code
    /// watching the real distance every frame, with no provider round trip in
    /// it — that is the entire promise of the mechanic — so it is these
    /// commands, never a sim-side distance check, that earn the holder their
    /// percept and priority turn.
    ///
    /// Fire-and-forget, like [`Self::PlayerSound`]: a grab is not a request.
    PlayerGrabbed {
        holder_id: ActorId,
    },
    /// The player has begun pulling against the hands on them. Sent **once**
    /// when the pulling starts, not per frame: there is exactly one LLM turn in
    /// flight across the entire cast, so across a ~5 s struggle the holder gets
    /// one turn however often they are poked, and a per-second percept would buy
    /// five near-identical lines competing for one `since_your_last_turn`.
    PlayerStruggling,
    /// …and once if it succeeds.
    PlayerBrokeFree,
    /// Fire-and-forget: no `CommandResult` ever (`server.py:1066-1092`).
    PlayerSound {
        sound_id: String,
    },
    /// The drive-mode stand-in for world causes the sim does not model.
    DebugSound {
        sound_id: String,
        position_m: Vec3,
    },
    /// A world cause the *game* models, announcing itself: the first is a
    /// boiling rat colony (`features/rats.md` M2). Identical to
    /// [`Self::DebugSound`] in every respect but the name, which is the point —
    /// a shipped feature should not ride in on the debug verb. Unattributed
    /// (`emit_sound(world, None, …)`), so it is never anybody's act; the inbox
    /// coalescing counter bounds a cause that repeats.
    WorldSound {
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
    /// The drive-mode stand-in for an arrest (`law_and_order.md` M4). Every
    /// judgement above `seize` is an LLM's, which is right — and which means a
    /// scripted run cannot reliably *reach* a seizure to look at the thing M4c
    /// and M4d actually build: the tether, the reflex, the strain meter. This is
    /// the same kind of poke `DebugSetStatus` is (the ale the sim does not
    /// model), and it goes through the same [`actions::take_into_charge`] a real
    /// arrest does, so what it stages is not a special case.
    ///
    /// `officer` is a name or an id; `target` defaults to the player, who is the
    /// case worth eyeballing. No `CommandResult`; a handle matching nobody is a
    /// `Diagnostic`, never a fault.
    DebugSeize {
        officer: String,
        target: Option<String>,
    },
    /// CATHEDRAL_DRIVE `commit` (`law_and_order.md` M5): finish the escort at
    /// the Stone House, so a scripted run can look at the inside of the gaol —
    /// the booking, the posted fee, the bell, and what walking out costs.
    /// `target` defaults to the player.
    DebugCommit { target: Option<String> },
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
    /// the sheet, never here (`features/implemented/movement/01_the_clock.md` §7).
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
    /// (`features/implemented/movement/06_engineering.md`, the hot/cold split). The host
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
    /// The street dogs ([`crate::dogs`]) — the whole pack, republished on the
    /// movement tick whenever any dog changed pose (and once at the start so
    /// the host can spawn resting bodies). The `Lamps` shape: ten entries,
    /// cheaper republished whole than deltaed, and it never bumps
    /// `world_revision` — the host mirrors it into quadruped puppets and
    /// nothing else reads it. The prompt reads `World::dogs` directly.
    Dogs {
        dogs: Vec<crate::dogs::DogView>,
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
    /// Where the player stands with the law (`law_and_order.md` M4), on the
    /// **hot** channel — like [`Self::Clock`] and [`Self::Movement`], and never
    /// a `world_revision` bump: a wanted list is not carriage state, and
    /// republishing every actor and item because a notice aged would be pure
    /// waste. Republished only when its content changes.
    ///
    /// Two things ride it and both have to be exact at frame rate. The HUD's
    /// standing line, which must **always** name what would clear a brand — a
    /// brand with a visible door is a story, a brand with no door is a bug. And
    /// the tether: the host clamps the player's *desired* position against
    /// [`PlayerCustody::anchor_m`] before its swept solve, and decides the grab
    /// reflex against these radii. That reflex cannot live in the sim, because
    /// the sim reads the player at `POSITION_UPDATE_HZ = 10` — 1.2 m of travel
    /// per sample at run speed, before the return trip — and a 3 m radius
    /// decided sim-side would be wrong by most of its own radius.
    LawStanding {
        /// The live words naming the player, worst rung first.
        notices: Vec<PlayerNotice>,
        /// The law's hands, while they are on you.
        custody: Option<PlayerCustody>,
    },
    /// A former `[smart actors] …` stderr line; the host logs it.
    Diagnostic(String),
}

/// One live word against the player, as the HUD reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerNotice {
    pub notice_id: u64,
    /// The wrong as the ward says it, rung included.
    pub line: String,
    pub rung: notices::Rung,
    /// What would end it, named plainly. Never a mystery box.
    pub clears_when: String,
}

/// The law's hands on the player, as the host needs them: who, where the grip
/// point is, and how far it reaches.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerCustody {
    /// Everyone with a hand on the player's arm — empty while merely in charge.
    /// Two holders is much worse to pull against, which is why it is a list.
    pub holder_ids: Vec<ActorId>,
    pub officer_id: Option<ActorId>,
    /// "Havise Ashe", or the stranger phrasing — resolved here, because the
    /// game never decides what the player knows.
    pub officer_name: String,
    pub station_name: String,
    /// The grip point the tether clamps against: the nearest holder, or the
    /// escorting officer while nobody has hold.
    pub anchor_m: Vec3,
    /// How far the player may drift while merely in charge before the officer
    /// closes ([`crate::custody::CUSTODY_LEASH_M`]).
    pub leash_m: f64,
    /// How far a *held* player may move around the grip point.
    pub tether_m: f64,
    /// Arm's reach: the radius the host's grab reflex fires at.
    pub reach_m: f64,
    /// The leash has been broken and the officer is coming to take hold
    /// ([`crate::custody::CustodyRecord::closing`]). The host's reflex fires at
    /// [`Self::reach_m`] when this is set *or* when the player is actively
    /// moving away — the latch is what makes "walk off, then stand still" a
    /// losing move rather than a free one.
    pub closing: bool,
    /// How long the player must pull without stopping to tear free of this
    /// grip ([`crate::custody::strain_seconds`]). The meter itself is host-side
    /// — it is a 20 Hz input affair and the sim has no clock — but every
    /// modifier in this number is the sim's, so a drunk player and a drunk NPC
    /// are hard to hold for exactly the same reasons.
    pub strain_seconds: f64,
    pub held: bool,
    pub committed: bool,
    /// The posted gaol fee ([`crate::custody::GAOL_FEE_SPARKS`]), so the HUD
    /// line can always name a door instead of a mystery box. A brand with a
    /// visible door is a story; a brand with no door is a bug.
    pub fee_sparks: u32,
    /// The bell they were told they go at, as the city says it — `"Lamplight"`
    /// — or `None` at a station, where the honest answer is "when the keeper
    /// says". Constant for the life of the record, like every other field here:
    /// this rides a hot channel that republishes on change, so a live countdown
    /// would make the message new on every single poll.
    pub release_office: Option<String>,
    /// What the keeper's book says, which is never a name — nobody in this city
    /// knows the player. `None` when no word named them.
    pub booked_as: Option<String>,
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
    /// (`features/implemented/movement/03_the_ladder.md`).
    round: Round,
    /// The second cognition lane (movement M6): the Night Office. Inert unless
    /// `config.night_office.enabled`; when it is, it is polled *before* the
    /// scheduler so it can take its own completion out of the queue before the
    /// turn stream discards it as stale ([`crate::night`]).
    night: NightOffice,
    /// The `now` at or after which the deterministic round may tick again. The
    /// ladder and its services are a 20 Hz behaviour, not a per-frame one
    /// ([`MOVEMENT_TICK_SECONDS`]): running its ~13 whole-cast passes on every
    /// render frame is pure waste at 60 Hz. The dt/office math inside
    /// [`round::tick`] spans real elapsed time (`round.last_game_days`,
    /// `round.last_office_now`), so a coarser cadence changes when it runs, never
    /// how much time it accounts for.
    next_round_tick_at: f64,
    /// The round's lamp revision as of the last `Lamps` publish, so the set is
    /// resent exactly when something changed. 0 = never sent; the seed puts the
    /// round at 1, so the first poll always announces the posts.
    lamp_revision_sent: u64,
    /// Whether the dog pack has been published at least once — the first
    /// `Dogs` message goes out even if every dog is still resting, so the host
    /// can spawn the bodies.
    dogs_published: bool,
    /// Diagnostics raised at construction (the M2 mover seeding), emitted with
    /// the first poll's `Ready` because `Engine::new` has no `out` to push to.
    startup_diagnostics: Vec<String>,
    ready_emitted: bool,
    /// The last [`EngineMessage::LawStanding`] published, so the hot channel
    /// resends exactly when the player's standing changes — which, for a player
    /// who has not annoyed anybody, is never.
    last_law_standing: Option<EngineMessage>,
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

        // The street dogs (`features/implemented/dogs.md`): the authored pack,
        // seeded only into a walkable world — the frozen fixtures and every
        // nav-less test keep an empty kennel and identical bytes.
        if config.dogs_enabled && let Some(nav) = config.nav.as_deref() {
            world.dogs = crate::dogs::seed_pack(nav);
        }

        // The chalk (`features/chalking_the_walls.md`). Nothing to seed — the
        // walls start bare and stay bare until a hand writes — so this is only
        // the ablation switch and the decay dial reaching the world.
        world.marks_enabled = config.marks_enabled;
        world.mark_kinds = config.mark_kinds;
        world.marks.decay_scale = config.marks_decay_scale;

        // The Night Office reads its bedtimes off the seeded round, so it is
        // built here and not a line earlier (M6). Off by default, and then this
        // is two map lookups and a `None`.
        let mut night = NightOffice::new(config.night_office, now);
        startup_diagnostics.extend(night.seed(&world, &round));

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
            night,
            // The first poll's `now` is >= this, so the round ticks on the first
            // poll exactly as it did every poll before the gate.
            next_round_tick_at: now,
            lamp_revision_sent: 0,
            dogs_published: false,
            startup_diagnostics,
            ready_emitted: false,
            last_law_standing: None,
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

        // The ward's word (law_and_order.md M3): drop notices whose life has
        // run out, then serve the face-to-face percept to any law-cast carrier
        // now within hearing of an accused. Both are no-ops while no notice is
        // live, which is almost always; the decay clock lives here because the
        // sim itself is clock-free.
        self.world.notices.expire(self.clock.game_days(now));
        // …and, on the same clock, the one rung that nobody chooses (M4a): a
        // summons still live when its named bell rings becomes a warrant. That
        // is the whole of the deadline — settlement is what discharges it, and
        // nothing else was ever tracked.
        self.issue_warrants(now);
        notices::confront(&mut self.world);
        // Weather the chalk on the same clock, and on the same principle: the
        // sim itself has none. Gated inside to once a game-minute — strength
        // moves in days and this runs at ~60 Hz — and it bumps the revision
        // only when something actually moved, so a bare wall never churns the
        // snapshot chain.
        if crate::marks::sweep(&mut self.world, self.clock.game_days(now)) {
            self.world.touch_public_state();
        }
        // …and the law's hands, whose every clock is a way custody ends: the
        // dead-man timer, the station's four minutes, walking off, and the
        // officer closing on a broken leash (M4).
        self.tick_custody(now, &mut out);

        // Drive the water round on the same beat: decay thirst, run the ladder,
        // work the queues at the wells. It sets the routes the *next*
        // `tick_movement` walks and buffers any well sounds into `world.events`,
        // which `flush` fans out below — so, like the mover pipeline, it needs a
        // nav graph and is otherwise inert.
        //
        // Gated to at most one tick per movement slice (20 Hz): a 60 Hz host
        // would otherwise pay the round's ~13 whole-cast passes three times per
        // slice for nothing. The real elapsed span is preserved — `round::tick`
        // reads game-days and office crossings off its own stored anchors — so a
        // skipped poll costs no simulated time, only a re-evaluation.
        if now >= self.next_round_tick_at
            && let Some(nav) = self.config.nav.clone()
        {
            self.next_round_tick_at = now + MOVEMENT_TICK_SECONDS;
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
                // An escort who has left the city cannot walk anybody anywhere,
                // so their prisoners are simply free — the same answer the
                // dead-man timer gives, for the same reason (M4) — and they are
                // *told* so, with the reason, like every other release: a hold
                // that ends in silence looks exactly like one that did not.
                for gone in &departed {
                    custody::forget_departed(&mut self.world, gone);
                }
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

        // A promise nobody can answer is not a promise: a targeted offer whose
        // two parties have drifted past `OFFER_LAPSE_RADIUS_M` ends here. After
        // the commands, so the player's own position update this poll counts;
        // before the scheduler, so a percept it produces is on the sheet of
        // anyone prompted in the same poll. Its events ride the flush below.
        //
        // Deliberately no priority nudge, unlike an answered offer
        // (`player_offer_reply`): a lapse fires exactly when the two are far
        // apart, which is when the stage gate is right to leave the giver
        // unprompted. The percept keeps in their inbox until they next think.
        actions::lapse_distant_offers(&mut self.world);

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

        // The second lane, before the first (M6). Two reasons for the order,
        // and both are load-bearing: the scheduler drains `completions` and
        // discards anything it is not waiting on, so a night reply left behind
        // it would be logged as a stale result and lost — and a `set_round` the
        // night applies is part of the world any turn submitted this same poll
        // is prompted from.
        let mut night_events: Vec<SchedulerEvent> = Vec::new();
        self.night.ring(
            now,
            &mut self.world,
            &mut self.round,
            &self.clock,
            &mut night_events,
        );
        // `wants_slot` first, so the stage question — a `characters_within`
        // scan — is asked on the handful of polls a night owes a reflection,
        // not on every frame of every run.
        //
        // And only under `Stage`: choosing `All` is the host declaring that it
        // has no player neighborhood worth reasoning about (the headless
        // runner's "player" is a fixture standing in the middle of the cast and
        // would block every night forever), which is the same declaration that
        // turns the idle gate off. A host with a real player uses `Stage`, and
        // gets the real gate.
        let gate = NightGate {
            floor_busy,
            player_composing: self.speech_router.player_composing(),
            stage_occupied: self.config.idle_mode == IdleCognitionMode::Stage
                && self.night.wants_slot(now)
                && stage_occupied(
                    &self.world,
                    &self.config.player_id,
                    self.conversation_partner(now),
                    &self.config.stage,
                ),
            player_reaction: self.scheduler.player_reaction_pending(),
        };
        night_events.extend(self.night.poll(
            now,
            &mut self.world,
            &self.clock,
            &mut completions,
            gate,
            self.cognition.as_mut(),
            &self.env,
        ));
        for event in night_events {
            out.push(scheduler_message(event));
        }

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
        if let Some(actor_id) = self.scheduler.take_submitted() {
            if self.config.idle_requires_news {
                self.novelty.told(now, &self.world, &actor_id);
            }
            // The other half of the dead-man timer (M4c): a holder who is
            // thinking is not a starved one. Stamped on submission, because
            // that is the moment the lane actually reached them.
            for prisoner in self.world.custody.prisoners_of(&actor_id) {
                if let Some(record) = self.world.custody.get_mut(&prisoner) {
                    record.officer_last_turn = Some(now);
                }
            }
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

        // The gut, on the same terms: the action layer has no clock, so the
        // poop clock lapses here (`extra_pockets.md` M3) and the urgency it
        // ramps rides the very next snapshot.
        self.digest(now);

        // The scheduler's turn produced domain events in this same poll; the
        // floor they acquire here gates the *next* one.
        self.flush(now, &mut out);

        // Last: the clock, reflecting any `CycleTimeScale` applied above, and
        // the player's standing with the law — hot like the clock, and resent
        // only when it changes.
        self.publish_clock(now, &mut out);
        self.publish_law_standing(&mut out);
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

    /// The second cognition lane (M6), for tests and the headless tracer.
    pub fn night(&self) -> &NightOffice {
        &self.night
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
            } => self.player_offer_reply(
                now,
                &request_id,
                "accept_offered_item",
                &item_id,
                position_m,
                spatial_seq,
                out,
            ),

            EngineCommand::PlayerDecline {
                request_id,
                item_id,
                position_m,
                spatial_seq,
            } => self.player_offer_reply(
                now,
                &request_id,
                "decline_offer",
                &item_id,
                position_m,
                spatial_seq,
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

            EngineCommand::PlayerPocket {
                request_id,
                item_id,
                slot,
            } => self.player_action(
                now,
                &request_id,
                "pocket_item",
                json!({"item_id": item_id.as_str(), "slot": slot.as_str()}),
                None,
                out,
            ),

            EngineCommand::PlayerRetrieve {
                request_id,
                item_id,
            } => self.player_action(
                now,
                &request_id,
                "retrieve_item",
                json!({"item_id": item_id.as_str()}),
                None,
                out,
            ),

            EngineCommand::PlayerSwallow {
                request_id,
                item_id,
            } => self.player_action(
                now,
                &request_id,
                "swallow",
                json!({"item_id": item_id.as_str()}),
                None,
                out,
            ),

            EngineCommand::PlayerSpit {
                request_id,
                item_id,
                target_id,
                position_m,
                spatial_seq,
            } => self.player_action(
                now,
                &request_id,
                "spit",
                json!({"item_id": item_id.as_str(), "target": target_id.as_str()}),
                Some((spatial_seq, position_m)),
                out,
            ),

            EngineCommand::PlayerGargle {
                request_id,
                item_id,
            } => self.player_action(
                now,
                &request_id,
                "gargle",
                json!({"item_id": item_id.as_str()}),
                None,
                out,
            ),

            EngineCommand::PlayerExpel { request_id } => {
                self.player_action(now, &request_id, "expel", json!({}), None, out)
            }

            EngineCommand::PlayerEat {
                request_id,
                item_id,
            } => self.player_action(
                now,
                &request_id,
                "eat",
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

            EngineCommand::PlayerGrabbed { holder_id } => self.player_grabbed(now, &holder_id),
            EngineCommand::PlayerStruggling => self.player_struggling(now),
            EngineCommand::PlayerBrokeFree => self.player_broke_free(now),

            EngineCommand::PlayerSound { sound_id } => self.player_sound(now, &sound_id, out),

            EngineCommand::DebugSound {
                sound_id,
                position_m,
            } => self.world_sound(now, "debug_sound", &sound_id, position_m, out),

            EngineCommand::WorldSound {
                sound_id,
                position_m,
            } => self.world_sound(now, "world_sound", &sound_id, position_m, out),

            EngineCommand::DebugSetStatus { name, kind, value } => {
                self.debug_set_status(now, &name, kind, value, out)
            }

            EngineCommand::DebugSeize { officer, target } => {
                self.debug_seize(now, &officer, target.as_deref(), out)
            }

            EngineCommand::DebugCommit { target } => {
                self.debug_commit(now, target.as_deref(), out)
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

    /// The player answering an offer (`accept_offered_item` / `decline_offer`),
    /// plus the offerer's wake-up. The apply puts the percept in the offerer's
    /// inbox, but the plain `player_action` path schedules nobody: a silent
    /// accept-and-walk under the stage gate leaves the offerer off stage
    /// carrying an unread acceptance, and they never think again until the
    /// player wanders back (law_and_order.md, problem 3). So the offerer gets
    /// the same priority handoff an addressed `say` and a `go_to` arrival grant
    /// — the lane is deliberately ungated by proximity. Not immediate: the
    /// inter-turn delay and the floor still govern when, only selection changes.
    #[allow(clippy::too_many_arguments)]
    fn player_offer_reply(
        &mut self,
        now: f64,
        request_id: &str,
        verb: &str,
        item_id: &ItemId,
        position_m: Vec3,
        spatial_seq: i64,
        out: &mut Vec<EngineMessage>,
    ) {
        // Resolved before the apply: a successful accept removes the offer.
        let offerer_id = self
            .world
            .offers
            .get(item_id)
            .map(|offer| offer.giver_id.clone());
        let result = self.apply_player_action(
            verb,
            &json!({"item_id": item_id.as_str()}),
            Some((spatial_seq, position_m)),
        );
        if result.is_ok()
            && let Some(offerer_id) = &offerer_id
        {
            self.scheduler
                .prioritize(&self.world, offerer_id, false, now);
        }
        self.finish_player_action(now, request_id, result, out);
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

    /// `_handle_debug_sound` (`server.py:1094-1112`) — the drive-mode town bell,
    /// and since `features/rats.md` M2 the boil as well: one funnel for both
    /// [`EngineCommand::DebugSound`] and [`EngineCommand::WorldSound`], which
    /// differ only in the name the caller uses and the one a bad id is
    /// diagnosed under. Any catalog row, no cooldown, no actor: world sounds
    /// are never attributed.
    fn world_sound(
        &mut self,
        now: f64,
        verb: &str,
        sound_id: &str,
        position_m: Vec3,
        out: &mut Vec<EngineMessage>,
    ) {
        let Some(sound) = self.world.sound_catalog.get(sound_id).cloned() else {
            out.push(EngineMessage::Diagnostic(format!(
                "[smart actors] invalid {verb}: there is no sound '{sound_id}'"
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

    /// The drive-mode arrest (`law_and_order.md` M4). Resolves both handles the
    /// way `debug_set_status` does — name first, then id — and then does exactly
    /// what the verb does, minus the four preconditions the verb exists to
    /// enforce. A poke at nobody is a logged `Diagnostic`, never a fault: a typo
    /// must not abort a drive script.
    fn debug_seize(
        &mut self,
        now: f64,
        officer: &str,
        target: Option<&str>,
        out: &mut Vec<EngineMessage>,
    ) {
        let Some(officer_id) = self.world.resolve_debug_handle(officer) else {
            out.push(EngineMessage::Diagnostic(format!(
                "[smart actors] invalid debug_seize: no character with the name or id '{officer}'"
            )));
            return;
        };
        let target_id = match target {
            Some(target) => match self.world.resolve_debug_handle(target) {
                Some(id) => id,
                None => {
                    out.push(EngineMessage::Diagnostic(format!(
                        "[smart actors] invalid debug_seize: no character with the name or id '{target}'"
                    )));
                    return;
                }
            },
            None => self.config.player_id.clone(),
        };
        // Put the officer at arm's reach first. Not a convenience: the verb
        // requires four metres precisely *because* an officer has to close on
        // foot, and `tick_custody` frees anyone whose escort is more than
        // `OFFER_LAPSE_RADIUS_M` away — so a poke that seized from across the
        // city would create a custody the very next poll dissolved, and show
        // nothing. Staging the approach is exactly what the stand-in is for.
        if let Some(at) = self
            .world
            .characters
            .get(&target_id)
            .map(|target| target.position_m())
        {
            let beside = Vec3::new(at.x + custody::CUSTODY_ESCORT_CONTACT_M, at.y, at.z);
            if let Some(officer) = self.world.characters.get_mut(&officer_id) {
                officer.state.position_m = beside;
                officer.state.movement = None;
            }
        }
        match actions::take_into_charge(
            &mut self.world,
            &officer_id,
            &target_id,
            None,
            "a wrong nobody wrote down (drive-mode stand-in)",
        ) {
            Ok(line) => {
                // The verb path gets this from the ladder's own escort sweep a
                // tick later; the stand-in has the `Round` in hand, so the
                // officer's committed errands — a well queue place, a stall
                // visit, a claimed shelter — are dropped before the very next
                // pump can walk the delivery off the leash.
                self.round
                    .abandon_bodily_errands(&mut self.world, &officer_id);
                out.push(EngineMessage::Diagnostic(format!("[smart actors] {line}")));
                self.scheduler
                    .prioritize(&self.world, &officer_id, false, now);
                self.flush(now, out);
            }
            Err(error) => out.push(EngineMessage::Diagnostic(format!(
                "[smart actors] debug_seize refused: {error}"
            ))),
        }
    }

    /// CATHEDRAL_DRIVE `commit` (`law_and_order.md` M5). The stand-in for an
    /// *arrival*, and it exists for the same reason `debug_seize` does: every
    /// judgement above `seize` is deliberately an LLM's, and the escort only
    /// commits on reaching a station, so a scripted run can otherwise never see
    /// the inside of the Stone House at all — not the booking, not the posted
    /// fee, not the bell, and not what walking out costs.
    ///
    /// It forces the gaol rather than whatever posting the seizure picked,
    /// because the gaol is the thing being looked at; everything after that is
    /// the same `Custody::commit` + [`Self::announce_commitment`] a real arrival
    /// runs, so what it stages is not a special case.
    fn debug_commit(&mut self, now: f64, target: Option<&str>, out: &mut Vec<EngineMessage>) {
        let target_id = match target {
            Some(target) => match self.world.resolve_debug_handle(target) {
                Some(id) => id,
                None => {
                    out.push(EngineMessage::Diagnostic(format!(
                        "[smart actors] invalid debug_commit: no character with the name or id '{target}'"
                    )));
                    return;
                }
            },
            None => self.config.player_id.clone(),
        };
        if self.world.custody.get(&target_id).is_none() {
            out.push(EngineMessage::Diagnostic(
                "[smart actors] debug_commit refused: nobody has them in charge - `seize` first"
                    .to_string(),
            ));
            return;
        }
        if self.world.custody.is_confined(&target_id) {
            // Refuse *before* rewriting the station and walking people around:
            // `Custody::commit` returns false for an already-committed record,
            // so without this a second `commit` would teleport the keeper, move
            // the station under them and then say nothing at all.
            out.push(EngineMessage::Diagnostic(
                "[smart actors] debug_commit refused: they are already committed".to_string(),
            ));
            return;
        }
        let Some(gaol) = custody::stone_house(&self.world.places) else {
            out.push(EngineMessage::Diagnostic(
                "[smart actors] debug_commit refused: there is no Stone House in the registry"
                    .to_string(),
            ));
            return;
        };
        // The keeper stands at the threshold — confinement here is a person, and
        // a station whose keeper is twenty metres off keeps nobody. A prisoner
        // who is not the player is walked in too, exactly as the escort would
        // have; the player's feet are never the sim's, so a drive script has to
        // `tp` them here itself, and one committed elsewhere is judged to have
        // walked out on the next poll — which is the mechanic working.
        let officer = self.world.custody.get(&target_id).and_then(|record| record.officer.clone());
        for who in officer.iter().chain(
            (target_id != self.config.player_id).then_some(&target_id),
        ) {
            if let Some(character) = self.world.characters.get_mut(who) {
                character.state.position_m = gaol.point;
                character.state.movement = None;
                // The errand too, not just the path: `take_into_charge` lays the
                // officer their own walk to the station the seizure picked, and
                // `apply_intents` would re-lay it and march the keeper straight
                // back out of the room the poke just put them in.
                character.state.intent = None;
            }
        }
        if let Some(record) = self.world.custody.get_mut(&target_id) {
            record.station = gaol.clone();
        }
        if let Some(released) = self.world.custody.commit(&target_id, now) {
            self.announce_commitment(now, &target_id, &released);
            out.push(EngineMessage::Diagnostic(format!(
                "[smart actors] {target_id} is committed to {}",
                gaol.name
            )));
            self.flush(now, out);
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
        // answer, not three (`features/implemented/movement/06_engineering.md` §4).
        let stage = self
            .world
            .characters
            .get(&self.config.player_id)
            .map(|player| player.position_m());

        let mut moved_ids: BTreeSet<ActorId> = BTreeSet::new();
        let mut dogs_moved = false;
        let mut slices = 0usize;
        while self.movement_now + MOVEMENT_TICK_SECONDS <= now
            && slices < MAX_MOVEMENT_CATCHUP_SLICES
        {
            for id in self.world.step_movement(MOVEMENT_TICK_SECONDS, &nav, stage) {
                moved_ids.insert(id);
            }
            dogs_moved |=
                crate::dogs::step_dogs(&mut self.world.dogs, MOVEMENT_TICK_SECONDS, &nav);
            self.movement_now += MOVEMENT_TICK_SECONDS;
            slices += 1;
        }
        // Overflowed the catch-up budget: drop the backlog and snap forward.
        if self.movement_now + MOVEMENT_TICK_SECONDS <= now {
            self.movement_now = now;
        }

        // The escorted walk after the movers, not with them: a led body has no
        // path of its own, it stands where the officer's shoulder is
        // (`law_and_order.md` M4b′). This is the whole of NPC custody's motion,
        // and it is why the cast arresting each other costs almost nothing.
        let escort = custody::follow_escorts(&mut self.world, now);
        for id in escort.moved {
            moved_ids.insert(id);
        }
        for (prisoner, released) in escort.committed {
            self.announce_commitment(now, &prisoner, &released);
        }

        // The dogs' own hot channel, before the human movers' early-out: a
        // city where only the dogs are abroad still animates.
        if !self.world.dogs.is_empty() && (dogs_moved || !self.dogs_published) {
            self.dogs_published = true;
            out.push(EngineMessage::Dogs {
                dogs: self.world.dogs.iter().map(crate::dogs::Dog::view).collect(),
            });
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
    /// nothing and costs no tokens (`features/implemented/movement/01_the_clock.md` §7).
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

    /// The poop clock (`features/extra_pockets.md` M3), ticked beside the
    /// gesture deadlines and for the same reason: `actions.rs` has no clock, so
    /// a queued meal can only *land* here. Two passes, for everyone — the round
    /// enrols a fraction of the cast, and the player is not enrolled at all:
    ///
    /// 1. **Formation.** Every gut entry whose game-day stamp has lapsed
    ///    becomes one held stack-unit riding the butt slot. A full slot
    ///    displaces one of its occupants (deterministically chosen — no RNG);
    ///    what forms beside a stool stains, and so does whatever was already
    ///    there.
    /// 2. **Urgency.** While a stool rides a pocket the `urgency` carriage
    ///    status ramps over [`crate::URGENCY_RAMP_GAME_DAYS`], quantized to
    ///    sixteenths so a smooth ramp does not republish the snapshot every
    ///    poll. `expel` clears both the stool and the status. A debug-forced
    ///    `urgency` ([`crate::character::CharacterState::debug_urgency`])
    ///    outranks the ramp until it is given back with a `0`.
    fn digest(&mut self, now: f64) {
        let game_days = self.clock.game_days(now);
        let actor_ids: Vec<ActorId> = self.world.characters.keys().cloned().collect();
        let mut changed = false;
        for actor_id in &actor_ids {
            changed |= self.form_gut_contents(actor_id, game_days);
            changed |= self.ramp_urgency(actor_id, game_days);
        }
        if changed {
            self.world.touch_public_state();
        }
    }

    /// Pass one: land everything the gut has finished with. Returns whether the
    /// public snapshot changed.
    fn form_gut_contents(&mut self, actor_id: &ActorId, game_days: f64) -> bool {
        let Some(actor) = self.world.characters.get_mut(actor_id) else {
            return false;
        };
        if actor.state.gut.is_empty() {
            return false;
        }
        let (due, pending): (Vec<GutEntry>, Vec<GutEntry>) = std::mem::take(&mut actor.state.gut)
            .into_iter()
            .partition(|entry| entry.due_game_days <= game_days);
        actor.state.gut = pending;
        if due.is_empty() {
            return false;
        }

        for entry in due {
            // A full slot loses one of its occupants — it simply stops being
            // hidden, the item itself is untouched (the resolved open question:
            // "one of those two will be dropped").
            let occupied = self.world.characters[actor_id].pocketed_in_slot(BodySlot::Butt);
            if occupied >= crate::POCKET_SLOT_CAPACITY {
                let roll = crate::world::hash01(
                    "gut_displace",
                    actor_id,
                    self.world.event_sequence.unsigned_abs(),
                );
                let nth = ((roll * occupied as f64) as usize).min(occupied.saturating_sub(1));
                let pockets = &mut self
                    .world
                    .characters
                    .get_mut(actor_id)
                    .expect("checked above")
                    .state
                    .pockets;
                let displaced = pockets
                    .iter()
                    .enumerate()
                    .filter(|(_, unit)| unit.slot == BodySlot::Butt)
                    .map(|(index, _)| index)
                    .nth(nth)
                    .map(|index| pockets.remove(index));
                // A displaced *stool* is not carried in the hands afterwards —
                // it falls where the owner stands, exactly as `expel` leaves it
                // (the world has no ground items). Anything else was in `holds`
                // all along and simply stops being hidden.
                if let Some(unit) = displaced
                    && self
                        .world
                        .items
                        .get(&unit.item_id)
                        .is_some_and(|item| item.kind.as_str() == POOP_KIND)
                {
                    let _ = self
                        .world
                        .consume_item_quantity(actor_id, &unit.item_id, 1);
                }
            }

            // A stool is never stamped; anything else comes back the way the
            // journey left it.
            let mut metadata = entry.metadata.clone();
            if entry.kind.as_str() != POOP_KIND {
                metadata.insert(
                    CONDITION_METADATA_KEY.to_string(),
                    CONDITION_POOPSTAINED.to_string(),
                );
            }
            let stock = StockSpec {
                kind: entry.kind.clone(),
                metadata,
                quantity: 1,
            };
            let key = format!("digest:{actor_id}:{}", self.world.event_sequence);
            let Ok(item_id) = self.world.add_stock(actor_id, &stock, &key) else {
                continue;
            };
            self.world
                .characters
                .get_mut(actor_id)
                .expect("checked above")
                .state
                .pockets
                .push(PocketedUnit {
                    slot: BodySlot::Butt,
                    item_id: item_id.clone(),
                });
            self.stain_lower_slot(actor_id);

            let formed = self.world.items[&item_id].clone();
            let line = if entry.kind.as_str() == POOP_KIND {
                "system: your gut has done its work - something rides in your breeches; expel it in a fitting place.".to_string()
            } else {
                format!(
                    "system: the {} you swallowed has come through - it rides in your breeches.",
                    self.world.item_catalog.display_name(&formed)
                )
            };
            let position = self.world.characters[actor_id].position_m();
            if let Some(actor) = self.world.characters.get_mut(actor_id) {
                actor.notify_percept(line);
            }
            self.world.emit(DomainEvent::world_event(
                "digest",
                actor_id.clone(),
                None,
                Some(item_id),
                1,
                position,
                Vec::new(),
            ));
        }
        self.world.assert_invariants();
        true
    }

    /// Everything sharing a lower slot with a stool is stained (M2's metadata
    /// economy). A pocketed unit is committed, so each one leaves its slot for
    /// exactly as long as the restamp takes.
    fn stain_lower_slot(&mut self, actor_id: &ActorId) {
        let carries_a_stool = self.world.characters[actor_id].pockets().iter().any(|unit| {
            unit.slot == BodySlot::Butt
                && self
                    .world
                    .items
                    .get(&unit.item_id)
                    .is_some_and(|item| item.kind.as_str() == POOP_KIND)
        });
        if !carries_a_stool {
            return;
        }
        for _ in 0..crate::POCKET_SLOT_CAPACITY {
            let target = self.world.characters[actor_id]
                .pockets()
                .iter()
                .position(|unit| {
                    unit.slot == BodySlot::Butt
                        && self.world.items.get(&unit.item_id).is_some_and(|item| {
                            item.kind.as_str() != POOP_KIND
                                && item.metadata.get(CONDITION_METADATA_KEY).map(String::as_str)
                                    != Some(CONDITION_POOPSTAINED)
                        })
                });
            let Some(index) = target else {
                return;
            };
            let unit = self
                .world
                .characters
                .get_mut(actor_id)
                .expect("the actor is in the world")
                .state
                .pockets
                .remove(index);
            let key = format!("digest_stain:{actor_id}:{}", self.world.event_sequence);
            let stained = self.world.restamp_metadata(
                actor_id,
                &unit.item_id,
                1,
                CONDITION_METADATA_KEY,
                CONDITION_POOPSTAINED,
                &key,
            );
            // A restamp that cannot happen puts the unit back untouched, and
            // stops — retrying it on the next poll would only spin.
            let failed = stained.is_err();
            let item_id = stained.unwrap_or_else(|_| unit.item_id.clone());
            self.world
                .characters
                .get_mut(actor_id)
                .expect("the actor is in the world")
                .state
                .pockets
                .push(PocketedUnit {
                    slot: unit.slot,
                    item_id,
                });
            if failed {
                return;
            }
        }
    }

    /// Pass two: the pressure. Returns whether the public snapshot changed.
    fn ramp_urgency(&mut self, actor_id: &ActorId, game_days: f64) -> bool {
        let carries_a_stool = self.world.characters[actor_id].pockets().iter().any(|unit| {
            self.world
                .items
                .get(&unit.item_id)
                .is_some_and(|item| item.kind.as_str() == POOP_KIND)
        });
        let Some(actor) = self.world.characters.get_mut(actor_id) else {
            return false;
        };
        if !carries_a_stool {
            actor.state.urgency_since_game_days = None;
        }
        // The clock's own reading first, whether or not it is what gets
        // published: the ramp's anchor keeps being stamped (and cleared) under
        // a debug override, so lifting one hands the key back mid-ramp instead
        // of restarting the two hours.
        let ramped = carries_a_stool.then(|| {
            let since = *actor.state.urgency_since_game_days.get_or_insert(game_days);
            let urgency = ((game_days - since) / crate::URGENCY_RAMP_GAME_DAYS).clamp(0.0, 1.0);
            // Sixteenths: the carriage cannot show more, and every change here
            // republishes the whole snapshot.
            (urgency * 16.0).round() / 16.0
        });
        // A forced `urgency` outranks it (`npc_bodies.md` §8). This pass owns
        // the whole key — it rewrites it for a stool-carrier and removes it
        // from everyone else — so without this the debug poke was deleted in
        // the very poll that wrote it, and the clenched walk a developer asked
        // to eyeball never rendered.
        match actor.state.debug_urgency.or(ramped) {
            Some(urgency) => {
                if actor.state.statuses.get(&StatusKind::Urgency).copied() == Some(urgency) {
                    return false;
                }
                actor.state.statuses.insert(StatusKind::Urgency, urgency);
                true
            }
            None => actor.state.statuses.remove(&StatusKind::Urgency).is_some(),
        }
    }

    fn flush(&mut self, now: f64, out: &mut Vec<EngineMessage>) {
        for event in self.world.drain_events() {
            match event.event_type {
                EventType::Speech => self.flush_speech(now, &event, out),
                EventType::Sound => self.flush_sound(now, &event, out),
                EventType::WorldEvent => {
                    self.hold_for_handoff(now, &event);
                    self.nudge_pocket_witness(now, &event);
                    self.nudge_restitution_acceptor(now, &event);
                    self.nudge_custody(now, &event);
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

    /// Somebody hoisting their clothes an arm's length away is not something
    /// you finish selling your fish through: the plain-sight witnesses of a
    /// lower-slot `pocket_item` / `retrieve_item` (`extra_pockets.md` M2 — the
    /// verb decides who those are, and lists nobody for a mouth) get the next
    /// turn, exactly as the nearest witness of a sound does. Percepts alone
    /// wait for the idle rotation, which is a long time to hold a straight face.
    ///
    /// One nudge per act — the turn stream is global and single — and the
    /// player is never a candidate: he is not prompted, he reacts himself.
    fn nudge_pocket_witness(&mut self, now: f64, event: &DomainEvent) {
        if !matches!(event.kind.as_str(), "pocket_item" | "retrieve_item") {
            return;
        }
        for candidate in &event.witness_ids {
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

    /// The wake-up behind `settle_notice` (`law_and_order.md` M3.5). Taking a
    /// purse from someone the ward's word names is a question — is this the
    /// restitution? — and the acceptor has already spent the turn in which they
    /// took it. `actions::offer_restitution` put the question in their inbox;
    /// without the turn to answer it, a paid-up player would look no different
    /// from an unpaid one until the idle rotation happened to come round, which
    /// off stage is never (the M0 accept-nudge argument, from the other side of
    /// the exchange). Ungated by proximity like every priority handoff, and not
    /// immediate: the inter-turn delay and the floor still govern when.
    fn nudge_restitution_acceptor(&mut self, now: f64, event: &DomainEvent) {
        if event.kind != "accept_offered_item" {
            return;
        }
        let (Some(acceptor_id), Some(giver_id)) = (&event.actor_id, &event.target_id) else {
            return;
        };
        if notices::restitution_candidates(&self.world, giver_id, acceptor_id).is_empty() {
            return;
        }
        let acceptor_id = acceptor_id.clone();
        self.scheduler
            .prioritize(&self.world, &acceptor_id, false, now);
    }

    /// The one rung nobody chooses (`law_and_order.md` M4a): a summons still
    /// live when its named bell rings becomes a warrant, on the clock edge. The
    /// ward is told, the accused is told, and — because the word has changed
    /// rung — every law carrier's `served` flag has just been cleared, so the
    /// next `confront` pass tells them to their faces too.
    ///
    /// A no-op on every poll of every run that has no summons outstanding,
    /// which is almost all of them.
    fn issue_warrants(&mut self, now: f64) {
        let issued = self.world.notices.issue_warrants(self.clock.game_days(now));
        for notice_id in issued {
            let Some(notice) = self.world.notices.get(notice_id) else {
                continue;
            };
            let (line, accused) = (notice.line(), notice.accused.clone());
            let raiser = notice.raised_by.clone();
            let mut lines: Vec<(ActorId, String)> =
                notices::carrier_ids(&self.world, notice_id, &raiser)
                    .into_iter()
                    .filter(|carrier| Some(carrier) != accused.as_ref())
                    .map(|carrier| (carrier, format!("the bell has rung and the word is now a warrant: {line}")))
                    .collect();
            // The raiser is a carrier of their own word only by way of `except`;
            // tell them too, since it is their summons that just hardened.
            if self.world.characters.contains_key(&raiser)
                && self.world.characters[&raiser].control().is_llm()
                && Some(&raiser) != accused.as_ref()
            {
                lines.push((
                    raiser,
                    format!("the bell has rung and the word is now a warrant: {line}"),
                ));
            }
            if let Some(accused) = &accused
                && self
                    .world
                    .characters
                    .get(accused)
                    .is_some_and(|character| character.control().is_llm())
            {
                lines.push((
                    accused.clone(),
                    format!(
                        "the bell you were called to answer by has rung and you did not answer; a warrant now stands against you: {line}"
                    ),
                ));
            }
            for (recipient, text) in lines {
                if let Some(character) = self.world.characters.get_mut(&recipient) {
                    character.notify_percept(text);
                }
            }
            // The accused gets the turn to react to being wanted, wherever they
            // are: the priority lane is ungated by proximity for exactly this.
            if let Some(accused) = accused.filter(|id| {
                self.world
                    .characters
                    .get(id)
                    .is_some_and(|character| character.control().is_llm())
            }) {
                self.scheduler.prioritize(&self.world, &accused, false, now);
            }
        }
    }

    // ------------------------------------------------------------- custody

    /// Everything about the law's hands that needs a clock (`law_and_order.md`
    /// M4). The sim itself is clock-free by design, so — exactly as the notice
    /// decay does — the timers live here and the state lives in
    /// [`crate::custody`].
    ///
    /// Four things, in the order they matter, and every one of them is a way
    /// custody *ends* rather than a way it continues. Custody must always be
    /// draining: nothing else in the sim removes a person from the world, and
    /// the economy is made of named individuals.
    fn tick_custody(&mut self, now: f64, out: &mut Vec<EngineMessage>) {
        if self.world.custody.is_empty() {
            return;
        }
        // Start the dead-man clock of anything the action layer has just made or
        // just touched — a seizure, and a hand landing on an arm, are both its
        // work and it has no clock to stamp them with. Exactly what
        // `round::tick_intents` does for a fresh `go_to`'s expiry, and it has to
        // happen here, above the timer that reads it: judged against a stamp
        // nobody had set, every grip that landed more than
        // `CUSTODY_DEAD_MAN_SECONDS` into the session was let go on the very
        // next poll, and the tether never engaged at all.
        for record in self.world.custody.records_mut() {
            record.officer_last_turn.get_or_insert(now);
        }
        let mut freed: Vec<(ActorId, &'static str)> = Vec::new();
        let mut ungripped: Vec<ActorId> = Vec::new();
        let mut closing: Vec<(ActorId, ActorId, Vec3)> = Vec::new();
        let mut walked_out: Vec<ActorId> = Vec::new();
        let mut relay: Vec<(ActorId, custody::Station)> = Vec::new();

        for (prisoner, record) in self.world.custody.iter() {
            let Some(prisoner_character) = self.world.characters.get(prisoner) else {
                continue;
            };
            // 1. The dead-man timer, non-negotiable: a player must never be
            //    pinned by a provider outage, a killed process, or plain lane
            //    starvation. With one turn in flight across the whole cast a
            //    busy scene can starve a holder past a minute with nothing
            //    broken at all — releasing then is correct, not a false
            //    positive, and the officer can always take hold again.
            if record.is_held()
                && record
                    .officer_last_turn
                    .is_some_and(|last_turn| now - last_turn > custody::CUSTODY_DEAD_MAN_SECONDS)
            {
                ungripped.push(prisoner.clone());
            }
            // 2. The station cap: four real minutes at a gate arch, six in the
            //    Stone House, regardless of what the models do. A gate has
            //    nobody to talk to but the keeper; a cell has eight people.
            //
            //    It is a ceiling on *this session's* custody, not a sentence, so
            //    the authored inmates (M5b) are exempt: the city was already
            //    holding them when the run began, they are not waiting on
            //    anything the player can watch, and freeing eight lore prisoners
            //    six minutes into every session — with the percept "the keeper
            //    wants the room" — would empty the one room the Stone House
            //    exists to fill. `settle_notice` and `release` still free them,
            //    which is the point: their door is a person, like everyone's.
            if let Some(committed_at) = record.committed_at
                && !record.authored
                && now - committed_at > record.station.hold_seconds()
            {
                freed.push((prisoner.clone(), "the keeper wants the room"));
                continue;
            }
            // 2b. The sentence, in the city's own clock (M5c). *"You go at
            //     Lamplight"* is what the keeper said, so it has to be what
            //     happens — and it is only the Stone House that says it, which
            //     is also why this sits *below* the ceiling: the six minutes are
            //     the backstop for a bell that is 8.5 real minutes off, and
            //     whichever comes first wins.
            //
            //     Above the officer bind, because a committed prisoner usually
            //     has nobody walking them anywhere any more.
            if let Some(due) = record.sentence_due_game_days
                && self
                    .world
                    .current_time
                    .is_some_and(|time| time.day as f64 + time.fraction >= due)
            {
                freed.push((prisoner.clone(), "your time is served"));
                continue;
            }
            // 2c. Walking out (M5d). Confinement here is a keeper at a
            //     threshold, not a lock — most stations are not lockable rooms
            //     and the Stone House's own lock is broken, which is Ede Clove's
            //     authored goal and why the doorway has no leaf. So leaving is
            //     possible, and it *costs*: the fifth door out compounds into
            //     the same unanswerable word the struggle raises, no `wronged`
            //     and no `taken`, which only a law officer's `settle_notice` can
            //     ever end. Escape closes the "you could have just paid the fee"
            //     door, and that is what makes the choice a choice.
            //
            //     Planar, to agree with `follow_escorts`'s arrival test — a
            //     threshold must not behave oddly because of a step.
            //
            //     Unreachable for the cast: rung 0 of `round::decide` and the
            //     `go_to` refusal mean an NPC never takes a step of their own
            //     while held, the round's `set_route` refuses a prisoner so no
            //     mechanical mover re-lays one either, and `follow_escorts`
            //     clears whatever walk the seizure interrupted. This is the
            //     player's door.
            if record.state == custody::Confinement::Committed {
                let at = prisoner_character.position_m();
                let strayed = f64::hypot(
                    at.x - record.station.point.x,
                    at.z - record.station.point.z,
                );
                if strayed > custody::COMMITTED_ROAM_M {
                    walked_out.push(prisoner.clone());
                    continue;
                }
            }
            let Some(officer) = record
                .officer
                .as_ref()
                .and_then(|officer| self.world.characters.get(officer))
            else {
                continue;
            };
            if record.state != custody::Confinement::InCharge {
                continue;
            }
            let separation = officer.position_m().distance(prisoner_character.position_m());
            // 3. Gone. A gap this wide is almost always the player's doing —
            //    8 m/s against 1.8 needs no cleverness at all, and the word
            //    against you does not go anywhere — but the sim does not
            //    adjudicate whose feet opened it: the officer's side has its
            //    own movers, gated but not provably immovable, and a release
            //    that names a culprit it never established would lie to one
            //    party or the other. The arrangement is simply over; say that.
            if separation > OFFER_LAPSE_RADIUS_M {
                freed.push((prisoner.clone(), "you were parted and the arrangement lapsed"));
                continue;
            }
            // 4. Cross the leash and the officer closes, on their own two feet.
            //    That walk is the whole warning system: nobody is ever grabbed
            //    out of nowhere.
            if separation > custody::CUSTODY_LEASH_M {
                closing.push((
                    officer.id().clone(),
                    prisoner.clone(),
                    prisoner_character.position_m(),
                ));
            }
            // 5. The station walk itself must always be live. The intent the
            //    seizure laid can die with the delivery half-done — the budget
            //    burns down through conversation holds, and the closing chase
            //    above *replaces* it with a Person-follow that ends at the grab
            //    — and the ladder deliberately stands an escort with no intent
            //    (`round::decide`, rung 0's other side) rather than letting the
            //    lower rungs walk him away. So whenever the record is still in
            //    charge and the officer's feet are owed to nobody, re-aim them
            //    at the station exactly as the seize verb did. Never while the
            //    chase is on — latched, or breaking this very poll: that walk
            //    aims at the prisoner on purpose, and the grab that ends it
            //    clears the latch, which is what lets this rung take over on
            //    the very next poll. And never within the arrival radius:
            //    `tick_intents` ends a Place intent standing inside it, so a
            //    re-lay there would say "You have arrived" (and burn a priority
            //    nudge) every poll an uncommitted prisoner — the player, walked
            //    by nobody but themself — kept the officer waiting at the door.
            else if !record.closing
                && self.world.is_present(officer.id())
                && officer.state.intent.is_none()
                && officer.position_m().distance(record.station.point) > PLACE_ARRIVE_RADIUS_M
            {
                relay.push((officer.id().clone(), record.station.clone()));
            }
        }

        // Out through the broken lock (M5d). The release is not the story — the
        // word is. Escape buys a brand no payment can answer, and the gates are
        // deliberately *not* shut to you afterwards (only two have leaves and
        // they answer to the clock, not the law), so what you bought is a
        // permanent warrant rather than a locked city.
        for prisoner in walked_out {
            let Some(record) = self.world.custody.release(&prisoner) else {
                continue;
            };
            let keeper = record
                .officer
                .clone()
                .or_else(|| self.nearest_law_to(record.station.point));
            if let Some(prisoner_character) = self.world.characters.get_mut(&prisoner)
                && prisoner_character.control().is_llm()
            {
                prisoner_character.notify_percept(format!(
                    "you are out of {} and nobody's hand is on you; the ward will hear of it",
                    record.station.name
                ));
            }
            let raised = actions::raise_escape_notice(
                &mut self.world,
                &prisoner,
                keeper.as_slice(),
            );
            if let Some(keeper) = keeper.clone() {
                if let Some(keeper_character) = self.world.characters.get_mut(&keeper) {
                    keeper_character.notify_percept(format!(
                        "they are gone from {} — the door stood open and they took it",
                        record.station.name
                    ));
                }
                self.scheduler.prioritize(&self.world, &keeper, false, now);
            }
            // A ward already carrying eight warrants refuses the raise
            // (`Notices::raise`), and escape must not be silently free because
            // the board is full: say so rather than let it pass unremarked.
            if raised.is_none() {
                out.push(EngineMessage::Diagnostic(format!(
                    "[smart actors] {prisoner} walked out of {} and no word was raised - nobody of the law was within earshot, or the ward's board is full of warrants",
                    record.station.name
                )));
            }
            self.world.touch_public_state();
        }

        // The grip lapses visibly, never silently: a hand that came off because
        // the lane starved must read to everyone present exactly as a hand that
        // came off because its owner chose to let go.
        for prisoner in ungripped {
            let holders = self
                .world
                .custody
                .get(&prisoner)
                .map(|record| record.holders.clone())
                .unwrap_or_default();
            self.world.custody.release_grip(&prisoner);
            for holder in holders {
                actions::announce_grip(&mut self.world, &holder, &prisoner, false);
            }
        }
        // Re-lay the station walk (clause 5). Before the closing loop, so an
        // officer with two prisoners — one beside him earning a re-lay, one
        // strayed and owed a chase — ends the poll aimed at the runaway: the
        // chase below overwrites whatever this lays, which is the precedence
        // the two walks have always had.
        for (officer_id, station) in relay {
            let budget = actions::route_budget_for(&self.world, &officer_id, station.point);
            if let Some(officer) = self.world.characters.get_mut(&officer_id) {
                officer.state.places_known.insert(station.place_id.clone());
                officer.state.intent = Some(TravelIntent {
                    target: IntentTarget::Place {
                        place_id: station.place_id,
                        name: station.name,
                        point: station.point,
                    },
                    budget_seconds: budget,
                    deadline: None,
                });
            }
        }
        for (officer_id, prisoner_id, last_seen) in closing {
            // Latch it: from here the reflex may fire at arm's reach even
            // against somebody standing perfectly still, because they broke the
            // arrangement and this walk is the consequence. A plain distance
            // test cannot express that — 3 m is *inside* the 8 m leash, so
            // "at reach while outside the leash" is nowhere at all.
            if let Some(record) = self.world.custody.get_mut(&prisoner_id) {
                record.closing = true;
            }
            let follows_already = self
                .world
                .characters
                .get(&officer_id)
                .and_then(|officer| officer.state.intent.as_ref())
                .is_some_and(|intent| {
                    matches!(&intent.target, IntentTarget::Person { actor_id, .. } if *actor_id == prisoner_id)
                });
            if follows_already {
                continue;
            }
            let budget = actions::route_budget_for(&self.world, &officer_id, last_seen);
            if let Some(officer) = self.world.characters.get_mut(&officer_id) {
                officer.state.intent = Some(TravelIntent {
                    target: IntentTarget::Person {
                        actor_id: prisoner_id,
                        last_seen,
                        visible: true,
                    },
                    budget_seconds: budget,
                    deadline: None,
                });
            }
        }
        for (prisoner_id, why) in freed {
            let Some(record) = self.world.custody.release(&prisoner_id) else {
                continue;
            };
            // Every hand comes off audibly, whichever clock ended the custody.
            for holder in &record.holders {
                actions::announce_grip(&mut self.world, holder, &prisoner_id, false);
            }
            if let Some(officer_id) = &record.officer
                && let Some(officer) = self.world.characters.get_mut(officer_id)
            {
                officer.notify_percept(format!(
                    "the one you had in charge for {}: {why}",
                    record.station.name
                ));
                let officer_id = officer_id.clone();
                self.scheduler
                    .prioritize(&self.world, &officer_id, false, now);
            }
            if let Some(prisoner) = self.world.characters.get_mut(&prisoner_id)
                && prisoner.control().is_llm()
            {
                // With the reason, not without it. A hold that ends in silence
                // reads exactly like one that did not, and the difference
                // between *your time is served* and *the keeper wants the room*
                // is the whole of what the sentence promised (M5c).
                prisoner.notify_percept(format!("you are out of the law's hands: {why}"));
            }
            self.world.touch_public_state();
        }
    }

    /// The end of the escort (`law_and_order.md` M4b): the walk is over and the
    /// keeper's threshold begins.
    ///
    /// Arriving is the one moment in custody that nobody chose — the officer
    /// walked, the destination came — so it is the moment most likely to pass in
    /// silence if nothing says otherwise. The prisoner is told what would free
    /// them, because that promise is owed on every rung, and the officer gets
    /// the turn in which to hand them over or think better of it.
    ///
    /// `released` is what [`custody::Custody::commit`] handed back: the hands
    /// arriving took off the arm. It is an argument rather than something read
    /// off the record here because the commit already dropped them — read after
    /// the fact, the list is always empty and the loop below never ran at all.
    fn announce_commitment(&mut self, now: f64, prisoner_id: &ActorId, released: &[ActorId]) {
        let Some(record) = self.world.custody.get(prisoner_id) else {
            return;
        };
        let (station, officer) = (record.station.name.clone(), record.officer.clone());
        let gaol = record.station.stone_house;
        let notice_id = record.notice_id;
        // Every hand comes off — arriving ends the walk. Say so, or the
        // presented arm stays reaching at somebody nobody is holding any more:
        // a hold that ends in silence looks exactly like one that did not.
        for holder in released {
            actions::announce_grip(&mut self.world, holder, prisoner_id, false);
        }

        // The sentence, said in the city's own clock (M5c). Only in the gaol:
        // at a gate arch the honest answer is "when the keeper says", and it is
        // the Stone House where the waiting is the content rather than the price
        // of it. Stamped once, so the keeper cannot be asked twice and answer
        // differently — and so nothing time-varying joins the hot channel.
        let sentence = gaol
            .then(|| self.world.current_time.map(|time| time.next_bell()))
            .flatten();
        if let Some((office, due)) = sentence
            && let Some(record) = self.world.custody.get_mut(prisoner_id)
        {
            record.sentence_office = Some(office);
            record.sentence_due_game_days = Some(due);
        }

        // Booked as a *description*, never a name: nobody in this city knows the
        // player, and the keeper's book is the "unknown people" rule paying for
        // itself one more time. The word already carries the description a
        // stranger could act on, so when there is one it is what goes in the
        // book — the same words the ward has been repeating.
        let booked_as = notice_id
            .and_then(|notice_id| self.world.notices.get(notice_id))
            .map(|notice| notice.about.clone());

        if let Some(prisoner) = self.world.characters.get_mut(prisoner_id)
            && prisoner.control().is_llm()
        {
            let mut line = format!(
                "you have been brought to {station}, and here you are kept until the word against you is settled or the law lets you go"
            );
            if let Some((office, _)) = sentence {
                line.push_str(&format!(
                    "; you were told you go at {}, and the bell rings over this very roof",
                    office.label()
                ));
            }
            prisoner.notify_percept(line);
        }
        if let Some(officer_id) = officer.clone() {
            if let Some(officer) = self.world.characters.get_mut(&officer_id) {
                let mut line = format!("you have brought them to {station}");
                if let Some(booked_as) = &booked_as {
                    line.push_str(&format!("; you enter them in the book as {booked_as}"));
                }
                if let Some((office, _)) = sentence {
                    line.push_str(&format!(", to go at {}", office.label()));
                }
                officer.notify_percept(line);
            }
            self.scheduler
                .prioritize(&self.world, &officer_id, false, now);
        }

        self.confiscate_the_taking(prisoner_id, notice_id, officer.as_ref());
        if gaol
            && let Some(at) = self
                .world
                .characters
                .get(prisoner_id)
                .map(|prisoner| prisoner.position_m())
        {
            // The one door in the city that is a door (M5c). The host cues the
            // gaol door on this, and only here — a gate arch has no leaf to shut.
            self.world.emit(crate::event::DomainEvent::world_event(
                "commit",
                prisoner_id.clone(),
                officer,
                None,
                0,
                at,
                Vec::new(),
            ));
        }
        self.world.touch_public_state();
    }

    /// Confiscation, and it is **narrow on purpose**: exactly the thing the word
    /// says was taken, and nothing else in the pockets.
    ///
    /// M3.5 already models the specific stolen item (`WardNotice::taken`), and
    /// returning it settles the word by itself (`notices::settle_on_return`), so
    /// the gaol taking it back is the same fact arriving by the other road. A
    /// general inventory sweep would be a rage mechanic and it would fight the
    /// offer machinery the fee is paid through — the design says so in as many
    /// words, and this is the whole of the enforcement.
    ///
    /// With no keeper to hand it to, nothing is taken. An item that vanished
    /// into the fabric of the building could never be given back.
    fn confiscate_the_taking(
        &mut self,
        prisoner_id: &ActorId,
        notice_id: Option<u64>,
        officer: Option<&ActorId>,
    ) {
        let Some(taken) = notice_id
            .and_then(|notice_id| self.world.notices.get(notice_id))
            .and_then(|notice| notice.taken.clone())
        else {
            return;
        };
        let Some(prisoner) = self.world.characters.get(prisoner_id) else {
            return;
        };
        if !prisoner.holds().contains(&taken) {
            return;
        }
        let Some(keeper) = officer.cloned().or_else(|| {
            self.world
                .custody
                .get(prisoner_id)
                .map(|record| record.station.point)
                .and_then(|point| self.nearest_law_to(point))
        }) else {
            return;
        };
        let sequence = self.world.event_sequence + 1;
        // A plain transfer, not the offer path: nobody offered anything, and a
        // thing the prisoner has already promised to somebody else stays
        // promised — `transfer_item_quantity` respects every live commitment,
        // which is what keeps confiscation from quietly voiding a trade.
        if self
            .world
            .transfer_item_quantity(
                prisoner_id,
                &keeper,
                &taken,
                1,
                &format!("gaol_confiscation:{sequence}:{taken}"),
            )
            .is_err()
        {
            return;
        }
        if let Some(prisoner) = self.world.characters.get_mut(prisoner_id) {
            prisoner.notify_percept(format!(
                "the thing the word says you took (id {taken}) is lifted off you and set on the keeper's counter"
            ));
        }
        if let Some(keeper) = self.world.characters.get_mut(&keeper) {
            keeper.notify_percept(format!(
                "you take the thing the word names (id {taken}) off them and enter it in the book; it is not yours, it is held"
            ));
        }
    }

    /// The nearest law-cast character to a point, within earshot of it — the
    /// keeper standing at a threshold nobody was named the officer of.
    fn nearest_law_to(&self, point: Vec3) -> Option<ActorId> {
        self.world
            .characters
            .iter()
            .filter(|(_, character)| crate::notices::is_law(character))
            .map(|(id, character)| (id, character.position_m().distance(point)))
            .filter(|(_, distance)| *distance <= HEARING_RADIUS_M)
            .min_by(|(left_id, left), (right_id, right)| {
                left.total_cmp(right).then_with(|| left_id.cmp(right_id))
            })
            .map(|(id, _)| id.clone())
    }

    /// The host's grab reflex fired (M4c). It is this command — never a sim-side
    /// distance check — that earns the holder their percept and priority turn,
    /// because the host is the only place a 3 m radius can be decided exactly.
    fn player_grabbed(&mut self, now: f64, holder_id: &ActorId) {
        let player_id = self.config.player_id.clone();
        let Some(record) = self.world.custody.get(&player_id) else {
            return;
        };
        if record.officer.as_ref() != Some(holder_id) && !record.holders.contains(holder_id) {
            return;
        }
        if record.holders.contains(holder_id) {
            return;
        }
        self.world.custody.grab(&player_id, holder_id.clone());
        // The holder needs the turn here, not the prisoner: against the player
        // the grab is a *reflex* the officer never decided on, so this is their
        // first chance to say anything about the hand they just put out.
        actions::announce_grip(&mut self.world, holder_id, &player_id, true);
        self.scheduler.prioritize(&self.world, holder_id, false, now);
        self.world.touch_public_state();
    }

    /// The player has begun to pull — the first of the struggle's two moments.
    ///
    /// The strain meter is host-side (a 20 Hz input meter, and the sim has no
    /// clock by design), so all the sim ever hears is *started* and, maybe,
    /// *succeeded*. Both go through [`actions::announce_struggle`], the same
    /// call the NPC `struggle` verb makes: one prose implementation, one wake-up
    /// rule, and the cast and the player heard the same way.
    fn player_struggling(&mut self, _now: f64) {
        let player_id = self.config.player_id.clone();
        let Some(record) = self.world.custody.get(&player_id) else {
            return;
        };
        if !record.is_held() {
            return;
        }
        let holders = record.holders.clone();
        actions::announce_struggle(
            &mut self.world,
            &player_id,
            &holders,
            actions::StruggleMoment::Started,
        );
    }

    /// …and once if it succeeds. The escape notice both paths raise is
    /// [`actions::raise_escape_notice`]'s, unanswerable by restitution — escape
    /// closes the "you could have just paid the fee" door, and that is the cost
    /// that makes the choice a choice.
    fn player_broke_free(&mut self, _now: f64) {
        let player_id = self.config.player_id.clone();
        let Some(record) = self.world.custody.get(&player_id) else {
            return;
        };
        if !record.is_held() {
            return;
        }
        let holders = record.holders.clone();
        self.world.custody.release(&player_id);
        actions::raise_escape_notice(&mut self.world, &player_id, &holders);
        actions::announce_struggle(
            &mut self.world,
            &player_id,
            &holders,
            actions::StruggleMoment::BrokeFree,
        );
        self.world.touch_public_state();
    }

    /// The wake-up every rung of custody owes somebody (`law_and_order.md`
    /// M4b–M4d), and the reason both the cast's path and the player's converge
    /// on one place: whichever of them produced the event, the *other* party has
    /// just had something done to them and has not spent a turn on it.
    ///
    /// Ungated by proximity, like every priority handoff — an officer walking
    /// somebody across the city is normally nowhere near the stage — and not
    /// immediate: the inter-turn delay and the floor still govern when.
    fn nudge_custody(&mut self, now: f64, event: &DomainEvent) {
        // `target_id` is the counterpart in every one of these: the prisoner
        // for the officer's acts, the holder for the prisoner's.
        if !matches!(
            event.kind.as_str(),
            "seize" | "grab" | "let_go" | "release" | "struggle" | "broke_free"
        ) {
            return;
        }
        let Some(target_id) = event.target_id.clone() else {
            return;
        };
        if self
            .world
            .characters
            .get(&target_id)
            .is_some_and(|character| character.control().is_llm())
        {
            self.scheduler
                .prioritize(&self.world, &target_id, false, now);
        }
    }

    /// The player's standing with the law, on the hot channel — republished
    /// only when it changes, which is almost never.
    fn publish_law_standing(&mut self, out: &mut Vec<EngineMessage>) {
        let player_id = &self.config.player_id;
        let notices: Vec<PlayerNotice> = self
            .world
            .notices
            .against(player_id)
            .into_iter()
            .map(|notice| PlayerNotice {
                notice_id: notice.id,
                line: notice.line(),
                rung: notice.rung(),
                // A brand with a visible door is a story; a brand with no door
                // is a bug. Say the door out loud, every time.
                clears_when: match (&notice.taken, &notice.wronged) {
                    (Some(_), Some(_)) => {
                        "give back what was taken, or satisfy the law".to_string()
                    }
                    (_, Some(_)) => {
                        "make it right with the one you wronged, or satisfy the law".to_string()
                    }
                    _ => "only the law can end this one — go and answer for it".to_string(),
                },
            })
            .collect();
        let custody = self.world.custody.get(player_id).map(|record| {
            let anchor_of = |id: &ActorId| {
                self.world
                    .characters
                    .get(id)
                    .map(|character| character.position_m())
            };
            let officer_name = record
                .officer
                .as_ref()
                .and_then(|officer| self.world.characters.get(officer))
                .map_or_else(
                    || "the keeper".to_string(),
                    |officer| {
                        self.world.characters[player_id]
                            .knows()
                            .contains(officer.id())
                            .then(|| officer.name().to_string())
                            .unwrap_or_else(|| "a stranger who serves the law".to_string())
                    },
                );
            PlayerCustody {
                // The grip point: whoever actually has hold, else the escort.
                anchor_m: record
                    .holders
                    .first()
                    .and_then(&anchor_of)
                    .or_else(|| record.officer.as_ref().and_then(&anchor_of))
                    .unwrap_or(record.station.point),
                holder_ids: record.holders.clone(),
                officer_id: record.officer.clone(),
                officer_name,
                station_name: record.station.name.clone(),
                leash_m: custody::CUSTODY_LEASH_M,
                tether_m: custody::CUSTODY_TETHER_M,
                reach_m: custody::CUSTODY_REACH_M,
                closing: record.closing,
                strain_seconds: custody::strain_seconds(&self.world, player_id, &record.holders),
                held: record.is_held(),
                committed: record.state == custody::Confinement::Committed,
                fee_sparks: custody::GAOL_FEE_SPARKS,
                release_office: record
                    .sentence_office
                    .map(|office| office.label().to_string()),
                booked_as: record
                    .notice_id
                    .and_then(|notice_id| self.world.notices.get(notice_id))
                    .map(|notice| notice.about.clone()),
            }
        });
        let message = EngineMessage::LawStanding { notices, custody };
        if self.last_law_standing.as_ref() != Some(&message) {
            self.last_law_standing = Some(message.clone());
            out.push(message);
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
