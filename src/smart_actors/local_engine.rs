//! The in-process actor authority: `cathedral_sim::Engine` pumped by a Bevy
//! system, wearing the bridge's clothes.
//!
//! It replaces the Python sidecar without touching a single consumer. The game
//! still sends [`BridgeCommand`]s down a bounded queue and still drains
//! [`BridgeEvent`]s from an inbox; the difference is that the other end of those
//! channels is now [`pump_local_engine`], a system running first in
//! `SmartActorSet::DrainBridge`, rather than a JSON-lines pipe to another
//! process (D8, D9).
//!
//! Per pump, in order:
//!
//! 1. drain the backends' completion channel (finished LLM turns, speech) and
//!    the fake cognition's staged replies → `EngineCommand`s;
//! 2. drain the game's command queue → `EngineCommand`s (this is the *same*
//!    ordered queue the microphone worker streams into, so `player_audio_end`
//!    still precedes `player_recording` — D27);
//! 3. `catch_unwind(Engine::poll)` — a panicked engine emits
//!    [`BridgeEvent::Disconnected`] and the game degrades exactly as it did
//!    when the sidecar died (D8, R23);
//! 4. hand each `EngineMessage` to the inbox as-is.
//!
//! Step 4 used to be an encoding step. It is not any more: with the sidecar
//! gone there is no wire, so the message the engine produced is the message the
//! ECS consumes — no envelope, no JSON, no sequence numbers to check.
//!
//! The engine is built on `Hello`, not at plugin build: `hello` *is* the first
//! spatial update (`server.py:826-828`), and the player's real spawn has to be
//! in the world before the first snapshot leaves it.

use std::{
    cell::RefCell,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use bevy::prelude::*;
use cathedral_backends::{
    BackendCapabilities, BackendEvent, BackendsConfig, BackendsHandle, BackendsOptions,
    PromptExchange, PromptLog, SessionDir, world_data::load_world_seed,
};
use cathedral_sim::{
    ActorId as SimActorId, AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineCommand,
    EngineConfig, EngineMessage, FakeCognition, ItemId as SimItemId, NavData, NullSight, Office,
    PromptEnv, RequestId, ShelterMap, SoundCatalog, SpatialActorUpdate, SpeechEventId,
    SttBackendKind, Transcription, Tts, TtsBackendKind, Vec3 as SimVec3,
    WeatherConfig as SimWeatherConfig, WeatherMode, WorldClock, WorldSeed,
};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

use super::{
    SmartActorsConfig,
    bridge::{
        BridgeCommand, BridgeEvent, BridgeHandle, BridgeInbox, COMMAND_QUEUE_CAPACITY,
        STREAM_SAMPLE_RATE, TranscriptionBackend, TtsBackend,
    },
    model::Position,
};
use crate::config::WeatherSettings;

/// The asset half of the sim's authored data. Assets and lore ship with the
/// repository, not the player's save, so both resolve against the crate root
/// rather than the working directory.
fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// The committed navigation artifact — the baked walkable surface and the welded
/// street graph the movers route on — embedded so the sim walks on exactly the
/// bytes the F7 overlay draws (`nav_overlay.rs`) and the city renderer reads
/// (`city/mod.rs`): one geometry, three consumers (features/implemented/movement/02_navigation.md).
const NAV_JSON: &str = include_str!("../../assets/world/navigation.json");
const NAV_BIN: &[u8] = include_bytes!("../../assets/world/navigation.bin");
const SHELTERS_JSON: &str = include_str!("../../assets/world/shelters.json");

/// Owns the backends, and through them the session's private audio directory.
/// Dropping it stops the speech workers and removes the directory with
/// everything the microphone recorded into it — the sidecar's
/// `BridgeWorkerGuard` did the same, minus a child process to kill (D28).
#[derive(Resource)]
pub struct EngineGuard {
    /// `None` only when the backends could not start; the game then runs with
    /// the cast offline, which the HUD already knows how to say.
    _backends: Option<BackendsHandle>,
}

/// A [`FakeCognition`] the engine can own while the pump keeps draining it.
///
/// The fake completes synchronously into a staging buffer, so *somebody* has to
/// hand those completions back as commands. That somebody is the host — here,
/// as in `e2e_fake.rs` and the headless runner.
#[derive(Clone, Default)]
struct SharedCognition(Rc<RefCell<FakeCognition>>);

impl Cognition for SharedCognition {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        self.0.borrow_mut().request(prompt)
    }

    fn request_with_budget(
        &mut self,
        prompt: String,
        max_output_tokens: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.0
            .borrow_mut()
            .request_with_budget(prompt, max_output_tokens)
    }

    /// Forwarded, not defaulted: the default refuses, and the fake completes
    /// synchronously, so an offline run exercises the whole night path.
    fn request_night(
        &mut self,
        prompt: String,
        max_output_tokens: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.0.borrow_mut().request_night(prompt, max_output_tokens)
    }
}

/// Everything `Engine::new` needs, held until `Hello` brings the last piece:
/// where the player is standing.
struct EngineSeed {
    seed: WorldSeed,
    areas: AreaMap,
    catalog: SoundCatalog,
    prompts: PromptEnv,
    config: EngineConfig,
    capabilities: Capabilities,
    cognition: Box<dyn Cognition>,
    transcription: Box<dyn Transcription>,
    tts: Box<dyn Tts>,
}

/// The authority. A `NonSend` resource: `Engine` owns `Box<dyn Cognition>` and
/// friends, which are deliberately not `Send` (the sim is single-threaded by
/// construction — D22).
pub struct LocalEngine {
    seed: Option<EngineSeed>,
    engine: Option<Engine>,
    /// Everything the backends finish — LLM turns, transcripts, voices —
    /// arrives here (D7). The handle they hang off lives in the [`EngineGuard`],
    /// so a dropped guard stops them; this receiver then simply runs dry.
    completions: Option<Receiver<BackendEvent>>,
    /// Present in fake mode only; the real provider answers on the backend
    /// channel instead.
    fake_cognition: Option<SharedCognition>,
    prompt_log: PromptLog,
    commands: Receiver<BridgeCommand>,
    events: Sender<BridgeEvent>,
    /// A dead engine still drains its command queue (so producers never block)
    /// but never polls again.
    dead: bool,
}

impl LocalEngine {
    /// The single parsed map moves from the pending seed into the live sim at
    /// handshake time. Debug rendering borrows it here instead of loading or
    /// maintaining another coordinate source.
    pub(super) fn area_map(&self) -> Option<&AreaMap> {
        self.engine
            .as_ref()
            .map(|engine| &engine.world().area_map)
            .or_else(|| self.seed.as_ref().map(|seed| &seed.areas))
    }

    /// The live simulation world, once the handshake has moved the pending seed
    /// into a running [`Engine`]. Borrowed for developer inspection (the
    /// character debug sheet) exactly as [`Self::area_map`] borrows the map: a
    /// view of the authoritative sim, never a copy. `None` until the engine is
    /// live — the seed alone has no characters — so the sheet simply shows
    /// nothing before the cast comes online.
    pub(super) fn world(&self) -> Option<&cathedral_sim::World> {
        self.engine.as_ref().map(|engine| engine.world())
    }

    /// The daily round, borrowed exactly like [`Self::world`]: the character
    /// debug sheet reads a walker's errand (destination, well queue standing)
    /// from it. `None` until the engine is live.
    pub(super) fn round(&self) -> Option<&cathedral_sim::Round> {
        self.engine.as_ref().map(|engine| engine.round())
    }

    /// The live world, mutably — **tests only**. The acceptance tests stage a
    /// world state (someone holding an item out to the player) that otherwise
    /// takes a scripted conversation and a scheduler turn to reach. Nothing in
    /// the game writes the world except through a command; keep it that way.
    #[cfg(test)]
    pub(super) fn world_mut(&mut self) -> Option<&mut cathedral_sim::World> {
        self.engine.as_mut().map(|engine| engine.world_mut())
    }

    /// Die exactly as a panicking poll does — **tests only**. [`Self::pump`]'s
    /// `catch_unwind` arm drops the engine and disconnects; a test that has to
    /// keep driving its own `App` afterwards cannot reach that arm (a real panic
    /// would have to come from inside the sim), and what the host does with a
    /// dead engine is precisely what wants covering.
    #[cfg(test)]
    pub(super) fn die_as_if_panicked(&mut self) {
        self.engine = None;
        self.fail("the actor engine panicked: staged by a test".to_string());
    }
}

/// Start the engine and hand the ECS its resources.
///
/// Nothing here can fail loudly: a missing asset, an unusable temp directory or
/// a seed without a player all become a [`BridgeEvent::Disconnected`], which the
/// HUD already knows how to render as an offline cast.
pub fn spawn(
    config: &SmartActorsConfig,
    weather: &WeatherSettings,
) -> (BridgeHandle, BridgeInbox, EngineGuard, LocalEngine) {
    let (commands_tx, commands_rx) = bounded(COMMAND_QUEUE_CAPACITY);
    // Unbounded, unlike the sidecar's 256-slot event queue: producer and
    // consumer are now the same schedule, one system apart. A bounded queue
    // could only ever deadlock the pump against itself.
    let (events_tx, events_rx) = unbounded();

    let session = SessionDir::create(&SessionDir::new_session_id()).ok();
    let runtime_dir = session
        .as_ref()
        .map_or_else(std::env::temp_dir, |session| session.path().to_path_buf());

    let mut engine = LocalEngine {
        seed: None,
        engine: None,
        completions: None,
        fake_cognition: None,
        prompt_log: PromptLog::new(None, None),
        commands: commands_rx,
        events: events_tx.clone(),
        dead: false,
    };
    let mut guard = EngineGuard { _backends: None };

    match build(config, weather, session) {
        Ok((seed, backends, fake_cognition, prompt_log)) => {
            engine.completions = Some(backends.events().clone());
            engine.seed = Some(seed);
            engine.fake_cognition = fake_cognition;
            engine.prompt_log = prompt_log;
            guard._backends = Some(backends);
            // The handshake still opens with the same event, so mod.rs's
            // ProcessStarted arm (which answers with `Hello`) is untouched.
            let _ = events_tx.send(BridgeEvent::ProcessStarted);
        }
        Err(error) => {
            engine.dead = true;
            let _ = events_tx.send(BridgeEvent::Disconnected(error));
        }
    }

    (
        BridgeHandle::new(commands_tx, runtime_dir),
        BridgeInbox::new(events_rx),
        guard,
        engine,
    )
}

type Built = (
    EngineSeed,
    BackendsHandle,
    Option<SharedCognition>,
    PromptLog,
);

/// Fill the streets: `config.ron: smart_actors.extra_ambient_npcs` generated
/// ambient citizens appended to the authored cast.
///
/// Off by default and *provably* off — a zero count returns the seed it was
/// handed, unvalidated and unallocated, so a stock run costs exactly what it
/// cost before the knob existed. Without a nav graph nobody walks anyway and
/// there is no walkable ground to stand a crowd on, so the count is refused
/// out loud rather than dropping twenty thousand people onto the origin.
fn with_extra_ambient(
    seed: WorldSeed,
    config: &SmartActorsConfig,
    nav: Option<&NavData>,
) -> Result<WorldSeed, String> {
    let (count, complaint) = config.extra_ambient_npcs();
    if let Some(complaint) = complaint {
        warn!("{complaint}");
    }
    if count == 0 {
        return Ok(seed);
    }
    let Some(nav) = nav else {
        warn!("extra_ambient_npcs is {count}, but the navigation graph did not load; no crowd");
        return Ok(seed);
    };
    let points = cathedral_sim::spread_over_walkable(nav, count as usize);
    let sheets = cathedral_sim::extra_ambient_sheets(nav, &points, 0);
    info!(
        "[smart actors] crowd: {} generated ambient citizens over {} nav nodes",
        sheets.len(),
        nav.node_count()
    );
    seed.with_extra_ambient(sheets)
        .map_err(|error| format!("invalid generated crowd: {error}"))
}

/// Load the assets, start the backends, and decide what this run can actually
/// do.
///
/// The capabilities are the probe's, unedited (`server.py:837-846`): a missing
/// `OPENAI_API_KEY` costs the player his microphone and the cast its cloud
/// voice, and nothing else — cognition, the local Canary worker and the local
/// Pocket voice each stand or fall on their own. `fake_backend` replaces every
/// provider with an offline stand-in and therefore reports everything available.
fn build(
    config: &SmartActorsConfig,
    weather: &WeatherSettings,
    session: Option<SessionDir>,
) -> Result<Built, String> {
    let assets = assets_dir();
    let lore = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lore");
    let read = |relative: &str| -> Result<String, String> {
        let path = assets.join(relative);
        std::fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))
    };
    // The walkable graph the engine steps its movers on. A parse failure is not
    // fatal: the engine keeps `nav: None`, nobody walks, and the rest of the cast
    // is none the wiser — exactly the frozen-fixture default (engine.rs). Only
    // handing it a `Some` turns movement on (features/implemented/movement/02_navigation.md).
    //
    // Loaded before the seed because the generated crowd is placed *on* it: a
    // citizen with nowhere walkable to stand is a citizen inside a wall.
    let nav = match NavData::from_parts(NAV_JSON, NAV_BIN) {
        Ok(nav) => Some(Arc::new(nav)),
        Err(error) => {
            warn!("navigation graph did not load; NPCs will not walk: {error}");
            None
        }
    };
    let seed = load_world_seed(&assets, &lore)?;
    let seed = with_extra_ambient(seed, config, nav.as_deref())?;
    let areas = AreaMap::from_json_str(&read("world/areas.json")?)
        .map_err(|error| format!("invalid world areas: {error}"))?;
    let catalog = SoundCatalog::from_toml_str(&read("sounds/catalog.toml")?)
        .map_err(|error| format!("invalid sound catalog: {error}"))?;
    let prompts = PromptEnv::new(
        &read("prompts/turn.j2")?,
        &read("prompts/night.j2")?,
        &read("prompts/strings.toml")?,
    )
    .map_err(|error| format!("invalid prompt assets: {error}"))?;

    let options = BackendsOptions {
        uv_binary: config.uv_binary.clone(),
        fake_mode: config.fake_backend,
        ..BackendsOptions::default()
    };
    let backends_config = BackendsConfig::load(&options);
    let turn_delay_seconds = backends_config.npc_turn_delay_seconds;
    let backends = BackendsHandle::start(backends_config, session)
        .map_err(|error| format!("could not start the actor backends: {error}"))?;

    let (cognition, fake_cognition): (Box<dyn Cognition>, Option<SharedCognition>) =
        if config.fake_backend {
            let fake = SharedCognition::default();
            (Box::new(fake.clone()), Some(fake))
        } else {
            (Box::new(backends.cognition()), None)
        };
    // Real STT/TTS engines (or their offline stand-ins): the handle picks by
    // `fake_mode`, so this one call site is both modes.
    let transcription: Box<dyn Transcription> = backends.transcription();
    let tts: Box<dyn Tts> = backends.tts();
    let (capabilities, tts_startup_message) =
        engine_capabilities(backends.capabilities(), &config.tts_backend);

    let prompt_log = backends
        .prompt_log(crate::session_log::paths().map(|session| session.root.join("prompts")));

    let shelters = Arc::new(
        ShelterMap::from_json_str(SHELTERS_JSON)
            .map_err(|error| format!("invalid world shelters: {error}"))?,
    );
    let weather_mode = WeatherMode::from_config_name(&weather.mode).unwrap_or_else(|| {
        warn!(
            "unknown weather.mode `{}` in config.ron; using timeline",
            weather.mode
        );
        WeatherMode::Timeline
    });
    let weather_frequency = if weather.frequency.is_finite() {
        weather.frequency.max(0.0)
    } else {
        warn!("non-finite weather.frequency in config.ron; using 1.0");
        1.0
    };

    let engine_config = EngineConfig {
        player_id: SimActorId::from_raw(PLAYER_ID),
        fake_mode: config.fake_backend,
        sounds_enabled: config.sounds.enabled,
        view_cone_degrees: f64::from(config.sounds.view_cone_degrees),
        sound_cooldown_seconds: f64::from(config.sounds.min_seconds_between_player_sounds),
        turn_delay_seconds,
        tts_selected: capabilities.tts_selected,
        tts_startup_message,
        stt_stream_grace_seconds: backends.stream_grace_seconds(),
        // The microphone worker writes its WAVs here (D28); the router names
        // them for the transcription backend, which is the only reader.
        runtime_dir: backends
            .runtime_dir()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        // Unlike the tests and the headless runner, the game has a player who is
        // actually standing somewhere — so it is the one caller that gates idle
        // turns on where that is, and on whether anything has happened there.
        idle_mode: config.idle_cognition.mode(),
        stage: config.idle_cognition.stage(),
        idle_requires_news: config.idle_cognition.require_news,
        idle_curiosity: config.idle_cognition.curiosity(),
        // …and, for the same reason, the one caller with a night worth having:
        // the second lane spends its calls in the hours the player is somewhere
        // quiet, which only exists when there is a player (M6).
        night_office: config.night_office.config(),
        marks_enabled: config.marks.enabled,
        mark_kinds: cathedral_sim::marks::MarkKindSwitches {
            cross: config.marks.cross,
            tally: config.marks.tally,
            ward_sign: config.marks.ward_sign,
        },
        marks_decay_scale: config.marks.decay_scale,
        // What the city knows (`features/knowledge_and_rumor/`). No packs from
        // the game host: a quest plants its own rows through
        // `EngineConfig::fact_packs`, which only the headless runner fills today.
        knowledge_enabled: config.knowledge.enabled,
        fact_packs: Vec::new(),
        clock: WorldClock::new(
            config.clock.seconds_per_day,
            Office::from_config_name(&config.clock.start_office).unwrap_or_else(|| {
                warn!(
                    "unknown clock.start_office `{}` in config.ron; opening at Dayspring",
                    config.clock.start_office
                );
                Office::Dayspring
            }),
            config.clock.start_day,
            config.clock.night_brightness,
        ),
        ring_the_offices: config.clock.ring_the_offices,
        weather: SimWeatherConfig {
            enabled: weather.enabled,
            seed: weather.seed,
            mode: weather_mode,
            frequency: weather_frequency,
        },
        shelters,
        nav,
        ..EngineConfig::default()
    };

    Ok((
        EngineSeed {
            seed,
            areas,
            catalog,
            prompts,
            config: engine_config,
            capabilities,
            cognition,
            transcription,
            tts,
        },
        backends,
        fake_cognition,
        prompt_log,
    ))
}

/// The handshake's honest answer, from the probe alone.
///
/// The voice is the one thing the host decides rather than reports: a configured
/// backend that cannot speak becomes `off` *with a sentence explaining itself*
/// (`server.py:493-513`), because a cast that is silently mute reads as a bug.
fn engine_capabilities(
    probed: BackendCapabilities,
    configured_tts: &str,
) -> (Capabilities, Option<String>) {
    let capabilities = Capabilities::new(
        probed.llm,
        probed.stt_cloud,
        probed.stt_local,
        probed.tts_cloud,
        probed.tts_local,
        TtsBackendKind::Off,
    );
    let (tts_selected, startup_message) = select_tts_backend(configured_tts, &capabilities);
    (
        Capabilities {
            tts_selected,
            ..capabilities
        },
        startup_message,
    )
}

const PLAYER_ID: &str = "player";

/// First in `SmartActorSet::DrainBridge`, so a command written in frame N's
/// `CollectInput` is polled — and its events drained by `drain_bridge_messages`
/// — in frame N+1, exactly the latency the sidecar had.
pub fn pump_local_engine(
    time: Res<Time>,
    mut engine: NonSendMut<LocalEngine>,
    mut timer: Local<PumpTimer>,
) {
    let _span = crate::perf::span(crate::perf::Probe::EnginePump);
    let started = std::time::Instant::now();
    engine.pump(time.elapsed_secs_f64());
    // The whole sim runs inside this call on the main thread; the same rolling
    // report the pose system keeps, so a slow poll is attributable from
    // `logs.jsonl` instead of a profiler.
    let elapsed_us = started.elapsed().as_secs_f64() * 1e6;
    timer.accum_us += elapsed_us;
    timer.max_us = timer.max_us.max(elapsed_us);
    timer.frames += 1;
    let now = time.elapsed_secs_f64();
    if now - timer.window_start >= 5.0 {
        if timer.window_start > 0.0 {
            info!(
                "[engine pump] avg {:.0} us, max {:.0} us over {} frames",
                timer.accum_us / f64::from(timer.frames.max(1)),
                timer.max_us,
                timer.frames,
            );
        }
        *timer = PumpTimer {
            window_start: now,
            ..default()
        };
    }
}

#[derive(Default)]
pub struct PumpTimer {
    window_start: f64,
    accum_us: f64,
    max_us: f64,
    frames: u32,
}

impl LocalEngine {
    /// One pump. `now` is monotonic seconds since app start; the sim reads no
    /// clock of its own.
    pub(super) fn pump(&mut self, now: f64) {
        if self.dead {
            // Keep the queue moving so `try_send` never wrongly reports a full
            // bridge to a player whose engine simply died.
            while self.commands.try_recv().is_ok() {}
            return;
        }

        let mut commands: Vec<EngineCommand> = Vec::new();
        self.collect_completions(&mut commands);

        while let Ok(command) = self.commands.try_recv() {
            match command {
                // The one command the engine cannot take: it *is* the engine's
                // construction.
                BridgeCommand::Hello {
                    position_m,
                    spatial_seq,
                } => self.start(position_m, spatial_seq, now),
                command => {
                    if let Some(command) = translate(command) {
                        commands.push(command);
                    }
                }
            }
        }

        let Some(engine) = self.engine.as_mut() else {
            // Pre-handshake, or a start that failed: a command with nowhere to
            // go is dropped, exactly as the sidecar dropped anything sent before
            // its `ready`.
            return;
        };

        match panic::catch_unwind(AssertUnwindSafe(|| engine.poll(now, commands))) {
            Ok(messages) => {
                for message in messages {
                    self.emit(message);
                }
            }
            Err(payload) => {
                // The engine may be half-way through a mutation. Drop it, and
                // let the game do what it already does when its authority dies.
                self.engine = None;
                let reason = panic_reason(payload.as_ref());
                self.fail(format!("the actor engine panicked: {reason}"));
            }
        }
    }

    /// The host's half of the backend contract (D7): everything the outside
    /// world finished since the last pump, handed back as commands.
    fn collect_completions(&mut self, commands: &mut Vec<EngineCommand>) {
        if let Some(fake) = &self.fake_cognition {
            for completion in fake.0.borrow_mut().drain_completions() {
                commands.push(EngineCommand::LlmCompletion(completion));
            }
        }
        let Some(completions) = &self.completions else {
            return;
        };
        for event in completions.try_iter() {
            let command = match event {
                BackendEvent::LlmCompletion(completion) => EngineCommand::LlmCompletion(completion),
                BackendEvent::Status(status) => EngineCommand::BackendStatus(status),
                event => match (
                    event.clone().into_transcription_outcome(),
                    event.into_tts_outcome(),
                ) {
                    (Some(outcome), _) => EngineCommand::Transcription(outcome),
                    (_, Some(outcome)) => EngineCommand::Tts(outcome),
                    _ => continue,
                },
            };
            commands.push(command);
        }
    }

    /// `Hello` carries the player's real spawn, which the world needs before it
    /// renders its first snapshot. The facing comes from the seed: the game only
    /// starts reporting a bearing with its first `spatial_update`.
    fn start(&mut self, position_m: Position, spatial_seq: u64, now: f64) {
        let Some(seed) = self.seed.take() else {
            // A second hello (there is no in-place restart) or a dead engine.
            return;
        };
        let EngineSeed {
            seed: world,
            areas,
            catalog,
            prompts,
            config,
            capabilities,
            cognition,
            transcription,
            tts,
        } = seed;
        let facing_yaw = world
            .character(&config.player_id)
            .map_or(0.0, |sheet| sheet.facing_yaw);
        let spawn = to_sim(position_m);

        match Engine::new(
            config,
            &world,
            areas,
            catalog,
            prompts,
            cognition,
            transcription,
            tts,
            Box::new(NullSight),
            capabilities,
            (spawn, facing_yaw),
            spatial_seq.min(i64::MAX as u64) as i64,
            now,
        ) {
            Ok(engine) => self.engine = Some(engine),
            Err(error) => self.fail(format!("the actor engine could not start: {error}")),
        }
    }

    // ------------------------------------------------------------- outbound

    /// The engine's own message, verbatim.
    ///
    /// The sidecar had to serialize here and the game had to parse there, and
    /// both halves had to agree on a shape that serde could not check. Nothing
    /// of that survives: the one exception the encoder used to make — the
    /// prompt archive, which is a filesystem contract rather than something the
    /// ECS consumes (D24) — is still handled here, and everything else simply
    /// travels.
    fn emit(&mut self, message: EngineMessage) {
        match message {
            EngineMessage::PromptExchange {
                actor_id,
                actor_name,
                prompt,
                answer,
                duration_seconds,
                error,
            } => self.prompt_log.record(PromptExchange {
                actor_id: actor_id.as_str().to_string(),
                actor_name,
                prompt,
                answer,
                duration_seconds,
                error,
            }),
            EngineMessage::Diagnostic(line) => {
                // The sim already bakes `[smart actors] ` into the payload, so
                // stderr prints the line as-is — prefixing again would both
                // double it and make stderr disagree with `logs.jsonl`.
                //
                // The `logs.jsonl` record is written here (it only touches a
                // buffer) and the stderr print is handed off: this runs inside
                // the engine pump, on the main thread, and `std::io::Stderr` is
                // unbuffered, so a terminal that is slow to drain would block
                // the middle of a frame. A poll that emits seven diagnostics
                // pays that seven times over.
                crate::session_log::log_line("engine", "INFO", &line);
                crate::session_log::print_line(line);
            }
            message => self.send(BridgeEvent::Message(Box::new(message))),
        }
    }

    fn send(&self, event: BridgeEvent) {
        // A closed inbox means the app is shutting down; there is nothing useful
        // to do about it here.
        let _ = self.events.send(event);
    }

    fn fail(&mut self, reason: String) {
        self.dead = true;
        crate::session_log::log_line("engine", "ERROR", &reason);
        self.send(BridgeEvent::Disconnected(reason));
    }
}

// --------------------------------------------------------------- translation

/// `BridgeCommand` → `EngineCommand`. `Hello` is the pump's own business (it
/// builds the engine) and is the only command that translates to nothing.
fn translate(command: BridgeCommand) -> Option<EngineCommand> {
    Some(match command {
        BridgeCommand::SpatialUpdate {
            position_m,
            spatial_seq,
            facing_yaw,
        } => EngineCommand::SpatialUpdate {
            spatial_seq: to_seq(spatial_seq),
            updates: vec![SpatialActorUpdate::new(
                SimActorId::from_raw(PLAYER_ID),
                to_sim(position_m),
                Some(f64::from(facing_yaw)),
            )],
        },
        BridgeCommand::PlayerRecording {
            request_id,
            wav_basename,
            stt_backend,
            position_m,
            spatial_seq,
        } => EngineCommand::PlayerRecording {
            request_id,
            wav_basename,
            stt_backend: match stt_backend {
                TranscriptionBackend::Cloud => SttBackendKind::Cloud,
                TranscriptionBackend::Local => SttBackendKind::Local,
            },
            position_m: to_sim(position_m),
            spatial_seq: to_seq(spatial_seq),
        },
        BridgeCommand::PlayerAudioBegin { wav_basename } => EngineCommand::PlayerAudioBegin {
            wav_basename,
            // The mic worker resamples every device to this before chunking.
            sample_rate: STREAM_SAMPLE_RATE,
        },
        BridgeCommand::PlayerAudioChunk {
            wav_basename,
            seq,
            samples,
        } => EngineCommand::PlayerAudioChunk {
            wav_basename,
            seq,
            samples,
        },
        BridgeCommand::PlayerAudioEnd {
            wav_basename,
            chunk_count,
            silent,
        } => EngineCommand::PlayerAudioEnd {
            wav_basename,
            chunk_count,
            silent,
        },
        BridgeCommand::PlayerAudioAbort { wav_basename } => {
            EngineCommand::PlayerAudioAbort { wav_basename }
        }
        BridgeCommand::DebugPlayerSay {
            request_id,
            text,
            target_id,
            position_m,
            spatial_seq,
        } => EngineCommand::DebugPlayerSay {
            request_id,
            text,
            target_id: target_id.map(|id| SimActorId::from_raw(id.0)),
            position_m: to_sim(position_m),
            spatial_seq: to_seq(spatial_seq),
        },
        BridgeCommand::PlayerSay {
            request_id,
            text,
            position_m,
            spatial_seq,
        } => EngineCommand::PlayerSay {
            request_id,
            text,
            position_m: to_sim(position_m),
            spatial_seq: to_seq(spatial_seq),
        },
        BridgeCommand::PlayerOffer {
            request_id,
            target_id,
            item_id,
            quantity,
            position_m,
            spatial_seq,
        } => EngineCommand::PlayerOffer {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
            target_id: SimActorId::from_raw(target_id.0),
            quantity,
            position_m: to_sim(position_m),
            spatial_seq: to_seq(spatial_seq),
        },
        BridgeCommand::PlayerAccept {
            request_id,
            item_id,
            position_m,
            spatial_seq,
        } => EngineCommand::PlayerAccept {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
            position_m: to_sim(position_m),
            spatial_seq: to_seq(spatial_seq),
        },
        BridgeCommand::PlayerDecline {
            request_id,
            item_id,
            position_m,
            spatial_seq,
        } => EngineCommand::PlayerDecline {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
            position_m: to_sim(position_m),
            spatial_seq: to_seq(spatial_seq),
        },
        BridgeCommand::PlayerRetract {
            request_id,
            item_id,
        } => EngineCommand::PlayerRetract {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
        },
        BridgeCommand::PlayerPocket {
            request_id,
            item_id,
            slot,
        } => EngineCommand::PlayerPocket {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
            slot,
        },
        BridgeCommand::PlayerRetrieve {
            request_id,
            item_id,
        } => EngineCommand::PlayerRetrieve {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
        },
        BridgeCommand::PlayerSwallow {
            request_id,
            item_id,
        } => EngineCommand::PlayerSwallow {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
        },
        BridgeCommand::PlayerSpit {
            request_id,
            item_id,
            target_id,
            position_m,
            spatial_seq,
        } => EngineCommand::PlayerSpit {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
            target_id: SimActorId::from_raw(target_id.0),
            position_m: to_sim(position_m),
            spatial_seq: to_seq(spatial_seq),
        },
        BridgeCommand::PlayerGargle {
            request_id,
            item_id,
        } => EngineCommand::PlayerGargle {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
        },
        BridgeCommand::PlayerExpel { request_id } => EngineCommand::PlayerExpel { request_id },
        BridgeCommand::PlayerEat {
            request_id,
            item_id,
        } => EngineCommand::PlayerEat {
            request_id,
            item_id: SimItemId::from_raw(item_id.0),
        },
        BridgeCommand::PlayerSound { sound_id } => EngineCommand::PlayerSound { sound_id },
        BridgeCommand::PlayerGrabbed { holder_id } => EngineCommand::PlayerGrabbed {
            holder_id: SimActorId::from_raw(holder_id.0),
        },
        BridgeCommand::PlayerStruggling => EngineCommand::PlayerStruggling,
        BridgeCommand::PlayerBrokeFree => EngineCommand::PlayerBrokeFree,
        BridgeCommand::DebugChalk { kind, anchor } => EngineCommand::DebugChalk { kind, anchor },
        BridgeCommand::PlayerScrubMark { mark_id } => EngineCommand::PlayerScrubMark { mark_id },
        BridgeCommand::PlayerDrawMark { kind, anchor } => {
            EngineCommand::PlayerDrawMark { kind, anchor }
        }
        BridgeCommand::DebugScrub { anchor } => EngineCommand::DebugScrub { anchor },
        BridgeCommand::DebugSeize { officer, target } => {
            EngineCommand::DebugSeize { officer, target }
        }
        BridgeCommand::DebugCommit { target } => EngineCommand::DebugCommit { target },
        BridgeCommand::DebugSound {
            sound_id,
            position_m,
        } => EngineCommand::DebugSound {
            sound_id,
            position_m: to_sim(position_m),
        },
        BridgeCommand::WorldSound {
            sound_id,
            position_m,
        } => EngineCommand::WorldSound {
            sound_id,
            position_m: to_sim(position_m),
        },
        BridgeCommand::DebugStatus { name, kind, value } => {
            EngineCommand::DebugSetStatus { name, kind, value }
        }
        BridgeCommand::CycleTimeScale => EngineCommand::CycleTimeScale,
        BridgeCommand::SetWeatherOverride { kind, intensity } => {
            EngineCommand::SetWeatherOverride { kind, intensity }
        }
        BridgeCommand::ClearWeatherOverride => EngineCommand::ClearWeatherOverride,
        BridgeCommand::SpeechPresented { speech_event_id } => EngineCommand::SpeechPresented {
            event_id: SpeechEventId(speech_event_id),
        },
        BridgeCommand::SetTtsBackend {
            request_id,
            backend,
        } => EngineCommand::SetTtsBackend {
            request_id,
            backend: match backend {
                TtsBackend::Cloud => TtsBackendKind::Cloud,
                TtsBackend::Local => TtsBackendKind::Local,
                TtsBackend::Off => TtsBackendKind::Off,
            },
        },
        BridgeCommand::Hello { .. } => return None,
    })
}

/// The one f32 → f64 boundary (D1).
fn to_sim(position: Position) -> SimVec3 {
    SimVec3::new(
        f64::from(position.x),
        f64::from(position.y),
        f64::from(position.z),
    )
}

fn to_seq(spatial_seq: u64) -> i64 {
    spatial_seq.min(i64::MAX as u64) as i64
}

/// `server.py:493-513`: resolve the configured voice against what this run can
/// actually speak, and say why when the answer is "nothing".
///
/// The two messages are Python's, verbatim — the HUD renders them as a toast, so
/// a mute cast is explained rather than merely mute.
fn select_tts_backend(
    configured: &str,
    capabilities: &Capabilities,
) -> (TtsBackendKind, Option<String>) {
    let configured = configured.trim().to_lowercase();
    let wanted = match configured.as_str() {
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
    let available = match wanted {
        TtsBackendKind::Cloud => capabilities.tts_cloud,
        TtsBackendKind::Local => capabilities.tts_local,
        TtsBackendKind::Off => true,
    };
    if available {
        (wanted, None)
    } else {
        (
            TtsBackendKind::Off,
            Some(format!(
                "Configured {configured} NPC voice backend is unavailable; voices are off"
            )),
        )
    }
}

fn panic_reason(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown cause".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cathedral_sim::{DEFAULT_STAGE_MAX_ACTORS, DEFAULT_STAGE_RADIUS_M, IdleCognitionMode};

    use crate::smart_actors::{
        IdleCognitionSettings, SmartActorsConfig,
        model::{ActorId, ItemId, WorldSnapshot},
    };

    /// The player's spawn in the preserved demo exchange: within 20 m of the
    /// original trio and within 4 m of Ilse.
    fn player_position() -> Position {
        Position::new(0.0, 0.91, 111.0).unwrap()
    }

    fn fake_config() -> SmartActorsConfig {
        SmartActorsConfig {
            fake_backend: true,
            tts_backend: "local".into(),
            ..SmartActorsConfig::default()
        }
    }

    /// Drives the engine without Bevy: the pump is a plain method taking `now`.
    struct Harness {
        handle: BridgeHandle,
        inbox: BridgeInbox,
        engine: LocalEngine,
        guard: EngineGuard,
        now: f64,
    }

    impl Harness {
        fn new() -> Self {
            let (handle, inbox, guard, engine) = spawn(&fake_config(), &WeatherSettings::default());
            Self {
                handle,
                inbox,
                engine,
                guard,
                now: 0.0,
            }
        }

        /// One frame: pump, then hand back everything the drain would have seen.
        fn step(&mut self) -> Vec<BridgeEvent> {
            self.engine.pump(self.now);
            self.now += 0.5;
            let mut events = Vec::new();
            while let Some(event) = self.inbox.try_recv() {
                events.push(event);
            }
            events
        }

        /// Pump until `found` says yes, answering speech events as the
        /// presentation layer would (otherwise the conversation floor holds the
        /// cast for the full reading estimate).
        fn run_until(&mut self, what: &str, mut found: impl FnMut(&EngineMessage) -> bool) {
            for _ in 0..400 {
                let mut hit = false;
                for event in self.step() {
                    let message = match event {
                        BridgeEvent::Message(message) => *message,
                        BridgeEvent::Disconnected(reason) => {
                            panic!("the engine disconnected while waiting for {what}: {reason}")
                        }
                        BridgeEvent::ProcessStarted => continue,
                    };
                    if let EngineMessage::Speech { event_id, .. } = &message {
                        self.handle
                            .try_send(BridgeCommand::SpeechPresented {
                                speech_event_id: event_id.0.clone(),
                            })
                            .expect("the command queue is drained every pump");
                    }
                    hit |= found(&message);
                }
                if hit {
                    return;
                }
            }
            panic!("timed out waiting for {what}");
        }

        fn send(&self, command: BridgeCommand) {
            self.handle.try_send(command).expect("queue has room");
        }
    }

    fn snapshot_of(message: &EngineMessage) -> Option<WorldSnapshot> {
        match message {
            EngineMessage::Snapshot(snapshot) => Some(snapshot.into()),
            _ => None,
        }
    }

    /// The flagship: the whole scripted conversation, offline, through the same
    /// channels the game uses — no sidecar, no uv, no network.
    #[test]
    fn fake_local_engine_runs_scripted_exchange_offline_and_cleans_up() {
        let mut harness = Harness::new();
        let runtime_dir = harness.handle.runtime_dir().to_path_buf();
        assert!(runtime_dir.is_dir(), "the session audio directory exists");

        // The handshake: ProcessStarted, then hello, then ready.
        assert!(matches!(
            harness.inbox.try_recv(),
            Some(BridgeEvent::ProcessStarted)
        ));
        harness.send(BridgeCommand::Hello {
            position_m: player_position(),
            spatial_seq: 0,
        });
        harness.run_until("ready", |message| {
            let EngineMessage::Ready {
                capabilities,
                snapshot,
            } = message
            else {
                return false;
            };
            assert_eq!(snapshot.player_id.as_str(), "player");
            assert!(capabilities.llm);
            assert_eq!(capabilities.tts_selected, TtsBackendKind::Local);
            true
        });

        // Ilse (k0fb1) answers, and the say is acknowledged.
        harness.send(BridgeCommand::DebugPlayerSay {
            request_id: "ask-name".into(),
            text: "What's your name?".into(),
            target_id: None,
            position_m: player_position(),
            spatial_seq: 1,
        });
        let mut replied = false;
        let mut acknowledged = false;
        harness.run_until("Ilse's answer", |message| {
            match message {
                EngineMessage::Speech { speaker_id, .. } if speaker_id.as_str() == "k0fb1" => {
                    replied = true;
                }
                EngineMessage::CommandResult {
                    request_id,
                    success: true,
                    ..
                } if request_id == "ask-name" => acknowledged = true,
                _ => {}
            }
            replied && acknowledged
        });

        // …and offers the coin when asked, without giving it away.
        harness.send(BridgeCommand::DebugPlayerSay {
            request_id: "ask-coin".into(),
            text: "Please offer me your coin".into(),
            target_id: None,
            position_m: player_position(),
            spatial_seq: 1,
        });
        harness.run_until("Ilse's coin offer", |message| {
            snapshot_of(message).is_some_and(|snapshot| {
                snapshot.offers.iter().any(|offer| {
                    offer.item_id.0 == "c0prs"
                        && offer.giver_id.0 == "k0fb1"
                        && offer.target_id.as_ref().is_some_and(|id| id.0 == "player")
                })
            })
        });

        // The player accepts: authoritative transfer, offer cleared.
        harness.send(BridgeCommand::PlayerAccept {
            request_id: "accept-coin".into(),
            item_id: ItemId("c0prs".into()),
            position_m: player_position(),
            spatial_seq: 1,
        });
        harness.run_until("the coin transfer", |message| {
            snapshot_of(message).is_some_and(|snapshot| {
                let holds = snapshot
                    .actors
                    .iter()
                    .find(|actor| actor.id.0 == "player")
                    .is_some_and(|player| player.holds.iter().any(|item| item.0 == "c0prs"));
                holds
                    && snapshot
                        .offers
                        .iter()
                        .all(|offer| offer.item_id.0 != "c0prs")
            })
        });

        // Re-offering it to Conny creates a pending offer — an offer is not a
        // transfer.
        harness.send(BridgeCommand::PlayerOffer {
            request_id: "offer-conny".into(),
            target_id: ActorId("cb947".into()),
            item_id: ItemId("c0prs".into()),
            quantity: None,
            position_m: player_position(),
            spatial_seq: 1,
        });
        harness.run_until("the pending re-offer", |message| {
            snapshot_of(message).is_some_and(|snapshot| {
                let still_holds = snapshot
                    .actors
                    .iter()
                    .find(|actor| actor.id.0 == "player")
                    .is_some_and(|player| player.holds.iter().any(|item| item.0 == "c0prs"));
                let pending = snapshot.offers.iter().any(|offer| {
                    offer.item_id.0 == "c0prs"
                        && offer.giver_id.0 == "player"
                        && offer.target_id.as_ref().is_some_and(|id| id.0 == "cb947")
                });
                still_holds && pending
            })
        });

        // The guard owns the audio directory; dropping it takes the recordings
        // with it.
        drop(harness.guard);
        assert!(
            !runtime_dir.exists(),
            "the engine did not remove its session directory"
        );
    }

    /// A dead engine must look exactly like a dead sidecar: the game keeps
    /// running, offline.
    #[test]
    fn a_failed_start_disconnects_instead_of_crashing_the_game() {
        let (handle, inbox, _guard, mut engine) =
            spawn(&fake_config(), &WeatherSettings::default());
        assert!(matches!(
            inbox.try_recv(),
            Some(BridgeEvent::ProcessStarted)
        ));

        // No hello: nothing can be polled, and nothing is emitted.
        engine.pump(0.0);
        assert!(inbox.try_recv().is_none());

        engine.fail("the actor engine panicked: boom".to_string());
        assert!(matches!(
            inbox.try_recv(),
            Some(BridgeEvent::Disconnected(reason)) if reason.contains("panicked")
        ));

        // A dead engine still drains its queue, so producers never see a full
        // bridge and block.
        handle
            .try_send(BridgeCommand::PlayerSound {
                sound_id: "fart".into(),
            })
            .expect("room in the queue");
        engine.pump(1.0);
        assert!(inbox.try_recv().is_none());
    }

    #[test]
    fn every_player_command_translates_into_the_engine_vocabulary() {
        // `Hello` is the pump's own business: it builds the engine rather than
        // being fed to it.
        assert!(
            translate(BridgeCommand::Hello {
                position_m: player_position(),
                spatial_seq: 0,
            })
            .is_none()
        );

        let translated = translate(BridgeCommand::SpatialUpdate {
            position_m: Position::new(1.0, 2.0, 3.0).unwrap(),
            spatial_seq: 7,
            facing_yaw: 0.5,
        })
        .expect("a spatial update");
        let EngineCommand::SpatialUpdate {
            spatial_seq,
            updates,
        } = translated
        else {
            panic!("expected a spatial update");
        };
        assert_eq!(spatial_seq, 7);
        assert_eq!(updates.len(), 1, "only the player ever moves");
        assert_eq!(updates[0].actor_id.as_str(), "player");
        assert_eq!(updates[0].position_m, SimVec3::new(1.0, 2.0, 3.0));
        assert_eq!(updates[0].facing_yaw, Some(0.5));

        let translated = translate(BridgeCommand::PlayerAudioBegin {
            wav_basename: "player-recording-1.wav".into(),
        })
        .expect("an audio begin");
        assert!(matches!(
            translated,
            EngineCommand::PlayerAudioBegin { sample_rate, .. } if sample_rate == STREAM_SAMPLE_RATE
        ));
    }

    /// The microphone worker writes into the directory the handle names, and
    /// the guard owns it: dropping the guard must take the recordings with it.
    #[test]
    fn the_handle_names_a_private_directory_the_guard_owns() {
        let (handle, _inbox, guard, _engine) = spawn(&fake_config(), &WeatherSettings::default());
        let directory = handle.runtime_dir().to_path_buf();
        assert!(directory.is_dir());
        assert!(
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("cathedral-smart-actors-")),
            "{} is not a session audio directory",
            directory.display()
        );

        drop(guard);
        assert!(!directory.exists());
    }

    fn speaks(cloud: bool, local: bool) -> Capabilities {
        Capabilities::new(true, false, false, cloud, local, TtsBackendKind::Off)
    }

    /// The two wordings are Python's (`server.py:500-512`), and both have to
    /// reach the player: a silent cast with no explanation is the bug.
    #[test]
    fn an_unavailable_or_invalid_voice_falls_back_to_off_and_says_why() {
        // Available: selected, nothing to report.
        assert_eq!(
            select_tts_backend("Local", &speaks(false, true)),
            (TtsBackendKind::Local, None)
        );
        assert_eq!(
            select_tts_backend(" cloud ", &speaks(true, false)),
            (TtsBackendKind::Cloud, None)
        );
        // Explicitly off: silent by request, so no complaint.
        assert_eq!(
            select_tts_backend("off", &speaks(true, true)),
            (TtsBackendKind::Off, None)
        );
        // Configured but unavailable.
        assert_eq!(
            select_tts_backend("local", &speaks(true, false)),
            (
                TtsBackendKind::Off,
                Some("Configured local NPC voice backend is unavailable; voices are off".into())
            )
        );
        assert_eq!(
            select_tts_backend("cloud", &speaks(false, true)),
            (
                TtsBackendKind::Off,
                Some("Configured cloud NPC voice backend is unavailable; voices are off".into())
            )
        );
        // A typo is not a backend.
        assert_eq!(
            select_tts_backend("automatic", &speaks(true, true)),
            (
                TtsBackendKind::Off,
                Some("Configured NPC voice mode is invalid; voices are off".into())
            )
        );
    }

    /// The handshake reports the probe, not a wish: each subsystem degrades on
    /// its own, and a voice the run cannot produce is `off` *with a reason*.
    ///
    /// Driven from the probe rather than from `build`, because a real-mode build
    /// on a developer machine would report whatever keys happen to be in the
    /// environment — and open a websocket to say so.
    #[test]
    fn the_handshake_reports_exactly_what_the_probe_found() {
        // Nothing configured: no key, no worker scripts.
        let (capabilities, message) = engine_capabilities(BackendCapabilities::default(), "local");
        assert!(!capabilities.llm && !capabilities.stt && !capabilities.tts);
        assert_eq!(capabilities.tts_selected, TtsBackendKind::Off);
        assert_eq!(
            message.as_deref(),
            Some("Configured local NPC voice backend is unavailable; voices are off")
        );

        // A cloud key alone: the microphone works, the cloud voice works, and
        // cognition (a *different* provider's key) is untouched by its absence.
        let cloud_only = BackendCapabilities {
            stt_cloud: true,
            tts_cloud: true,
            ..BackendCapabilities::default()
        };
        let (capabilities, message) = engine_capabilities(cloud_only, "cloud");
        assert!(!capabilities.llm, "speech never speaks for cognition");
        assert!(capabilities.stt && capabilities.stt_cloud && !capabilities.stt_local);
        assert!(capabilities.tts && capabilities.tts_cloud && !capabilities.tts_local);
        assert_eq!(capabilities.tts_selected, TtsBackendKind::Cloud);
        assert!(message.is_none(), "the configured voice works");

        // A local worker but no key: the local voice is selectable, the cloud
        // one is not.
        let local_only = BackendCapabilities {
            llm: true,
            stt_local: true,
            tts_local: true,
            ..BackendCapabilities::default()
        };
        let (capabilities, _) = engine_capabilities(local_only, "local");
        assert!(capabilities.llm && capabilities.stt_local && capabilities.tts_local);
        assert_eq!(capabilities.tts_selected, TtsBackendKind::Local);
    }

    /// Fake mode listens and speaks: the Rust fakes stand in for every provider,
    /// so the microphone worker is spawned and the cast has a (silent) voice.
    #[test]
    fn fake_mode_reports_every_capability_and_selects_the_configured_voice() {
        let (seed, _backends, fake, _log) =
            build(&fake_config(), &WeatherSettings::default(), None).expect("the shipped assets");
        assert!(fake.is_some());
        assert!(seed.capabilities.llm);
        assert!(
            seed.capabilities.stt && seed.capabilities.stt_cloud && seed.capabilities.stt_local
        );
        assert!(
            seed.capabilities.tts && seed.capabilities.tts_cloud && seed.capabilities.tts_local
        );
        assert_eq!(seed.capabilities.tts_selected, TtsBackendKind::Local);
        assert!(seed.config.tts_startup_message.is_none(), "the voice works");
        assert_eq!(
            seed.config.stt_stream_grace_seconds, 2.0,
            "the grace window the router parks a committed recording for"
        );
    }

    /// The game is the one caller whose player is actually standing somewhere,
    /// so it is the one caller that gates idle turns
    /// (features/gate_idle_cognition_on_proximity.md, then
    /// features/gate_idle_cognition_on_novelty.md). The sim defaults to ungated
    /// for the tests and the headless runner, so a broken mapping here would
    /// silently restore the ~1,100 calls/hour these features remove — nothing
    /// else would fail.
    #[test]
    fn the_game_gates_idle_cognition_on_the_players_neighborhood_and_on_news() {
        let (seed, _backends, _fake, _log) =
            build(&fake_config(), &WeatherSettings::default(), None).expect("the shipped assets");
        assert_eq!(seed.config.idle_mode, IdleCognitionMode::Stage);
        assert_eq!(seed.config.stage.radius_m, DEFAULT_STAGE_RADIUS_M);
        assert_eq!(seed.config.stage.max_actors, DEFAULT_STAGE_MAX_ACTORS);
        assert!(seed.config.idle_requires_news);

        // `mode: "all"` is the documented rebuild-free way back to the old
        // city-wide clock, and `require_news: false` the way back to paying for
        // silence. Both have to actually reach the engine.
        let ungated = SmartActorsConfig {
            idle_cognition: IdleCognitionSettings {
                mode: "all".into(),
                require_news: false,
                ..IdleCognitionSettings::default()
            },
            ..fake_config()
        };
        let (seed, _backends, _fake, _log) =
            build(&ungated, &WeatherSettings::default(), None).expect("the shipped assets");
        assert_eq!(seed.config.idle_mode, IdleCognitionMode::All);
        assert!(!seed.config.idle_requires_news);
    }

    /// The transcription backend is handed a path, and the microphone worker
    /// writes to one: they must be the same directory (D28), or the first
    /// utterance resolves as a missing file.
    #[test]
    fn the_engine_transcribes_from_the_directory_the_microphone_records_into() {
        let session_id = SessionDir::new_session_id();
        let session = SessionDir::create(&session_id).expect("a private audio directory");
        let path = session.path().to_path_buf();

        let (seed, backends, _fake, _log) =
            build(&fake_config(), &WeatherSettings::default(), Some(session))
                .expect("the shipped assets");
        assert_eq!(seed.config.runtime_dir, path);
        assert_eq!(backends.runtime_dir(), Some(path.as_path()));

        drop(backends);
        assert!(!path.exists(), "the backends own the directory");
    }
}
