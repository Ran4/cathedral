//! Engine-authoritative smart actors and their non-blocking Bevy projection.
//!
//! The authority is [`local_engine`], an in-process `cathedral_sim::Engine`.
//! The game writes [`bridge::BridgeCommand`]s into its queue and reads typed
//! `cathedral_sim::EngineMessage`s back out of its inbox; [`model::WorldMirror`]
//! projects the snapshots, and everything else here turns the engine's messages
//! into HUD toasts, speech bubbles and sound effects.

pub mod actors;
pub mod body;
pub mod bridge;
pub mod dogs;
pub mod local_engine;
pub mod model;
pub mod road_carts;

mod actor_sheet;
mod area_debug;
mod chat;
mod clock;
mod config_menu;
pub mod custody;
mod hands;
mod hud;
mod interaction;
mod inventory_ui;
mod lamps;
mod microphone;
mod sound;
mod speech;
mod targeting;

use bevy::audio::AddAudioSource;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use cathedral_sim::{
    Capabilities, CuriosityConfig, DEFAULT_STAGE_MAX_ACTORS, DEFAULT_STAGE_RADIUS_M, EngineMessage,
    IdleCognitionMode, NightOfficeConfig, StageConfig, StatusEvent, TtsBackendKind,
};
use serde::{Deserialize, Serialize};

use crate::{
    config::WeatherSettings,
    weather::{WeatherLightning, WorldWeatherState},
};

pub use area_debug::AreaDebugState;
pub use chat::{ChatInputSet, ChatInputState};
pub use clock::WorldClockState;
pub use config_menu::ConfigMenuState;
pub use inventory_ui::InventoryUiState;
pub use targeting::ActorFocus;

/// The one actor the game itself controls.
const PLAYER_ID: &str = "player";

pub const HEARING_RADIUS_M: f32 = 20.0;
pub const ITEM_INTERACTION_RADIUS_M: f32 = 4.0;
pub const PLAYER_SPEECH_MAX_SECONDS: u32 = 15;
pub const PLAYER_SPEECH_MAX_CHARS: usize = 500;
pub const POSITION_UPDATE_HZ: f32 = 10.0;

/// Non-secret client-side actor settings loaded from `config.ron`.
#[derive(Resource, Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SmartActorsConfig {
    pub enabled: bool,
    pub fake_backend: bool,
    /// Still needed: the local speech models (Canary-Qwen, Pocket TTS) run as
    /// `uv` worker subprocesses. Nothing else is spawned any more.
    pub uv_binary: String,
    pub tts_backend: String,
    /// Player transcription at startup: "cloud" (OpenAI) or "local"
    /// (Canary-Qwen FP16).
    pub stt_backend: String,
    pub pause_microphone_during_npc_voice: bool,
    /// Stream cloud transcription audio while the player is still speaking.
    pub stt_streaming: bool,
    /// Silence that ends an utterance. Clamped to a window where speech still
    /// ends promptly but deliberate mid-sentence pauses rarely split it.
    pub stt_trailing_silence_ms: u32,
    /// Non-speech sound percepts (features/sounds.md).
    pub sounds: SoundsConfig,
    /// Who gets to think when nothing has happened
    /// (features/gate_idle_cognition_on_proximity.md).
    pub idle_cognition: IdleCognitionSettings,
    /// What the cast does with the night (features/implemented/movement/05_the_llm_seam.md
    /// §4) — the second cognition lane.
    pub night_office: NightOfficeSettings,
    /// The world clock: the day/night cycle, the offices, the bell
    /// (features/implemented/movement/01_the_clock.md).
    pub clock: ClockSettings,
    /// The chalk on the walls (features/implemented/chalking_the_walls.md). Costs no
    /// tokens: marks are written by code, read by code, and reach an LLM only
    /// as one line on a turn that was going to happen anyway.
    pub marks: MarksSettings,
    /// What the city knows and is saying (features/knowledge_and_rumor/).
    /// Costs no tokens: facts are seeded and read by code, and reach an LLM
    /// only as up to three lines on a turn that was going to happen anyway.
    pub knowledge: KnowledgeSettings,
    /// How many *generated* ambient citizens to spread over the walkable city
    /// on top of the ~500 authored ones (`crates/cathedral-sim/src/crowd.rs`).
    /// `0` — the default — is the shipped city, unchanged down to the roster
    /// order. Clamped to [`cathedral_sim::MAX_EXTRA_AMBIENT_NPCS`];
    /// `CATHEDRAL_EXTRA_NPCS=n` sets it for one run without editing the file.
    ///
    /// These are not cast members: six-character ids, no authored sheet, no
    /// bed in `homes.json`, strangers to the player, and barred from the one
    /// civic post the round hands to whoever is nearest (see
    /// [`cathedral_sim::LoreProfile::generated`]). They cost no tokens by
    /// existing — the stage cap and the single in-flight cognition slot bound
    /// the spend however many people are standing about — but they do change
    /// *who* is nearest, and past a few thousand they are a frame-rate
    /// experiment rather than a setting.
    pub extra_ambient_npcs: u32,
}

/// Chalk marks: the ablation switch, the per-kind switches, and the decay dial
/// (`features/implemented/chalking_the_walls.md` §2.9).
#[derive(Resource, Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct MarksSettings {
    /// The whole layer. `CATHEDRAL_NO_MARKS=1` forces this off for one run.
    /// Turning it off stops new chalk and stops the readers; it does not
    /// scrub what is already on the walls.
    pub enabled: bool,
    /// The debtor's cross on a household door, and the stall refusal that
    /// reads it.
    pub cross: bool,
    /// The stroke-per-draw tally at a water source, and the well choice that
    /// reads it.
    pub tally: bool,
    /// The Night Office's ward-sign, and the ambient evening roll that reads
    /// it.
    pub ward_sign: bool,
    /// Multiplies elapsed time in the decay. `1.0` is the authored nine-day
    /// dry half-life; raise it to weather a wall inside a drive run.
    pub decay_scale: f64,
}

impl Default for MarksSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            cross: true,
            tally: true,
            ward_sign: true,
            decay_scale: 1.0,
        }
    }
}

/// What the city knows: the one ablation switch
/// (`features/knowledge_and_rumor/`, D49).
#[derive(Resource, Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KnowledgeSettings {
    /// The whole layer. `CATHEDRAL_NO_KNOWLEDGE=1` forces this off for one run.
    /// It gates **readers and writers both**, so a run with it off is a city
    /// with no knowledge layer rather than one accumulating state nobody reads.
    pub enabled: bool,
}

impl Default for KnowledgeSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// The gates on *idle* NPC turns: proximity, then novelty.
///
/// The three lanes that schedule an NPC turn are not equal: two of them fire
/// because of a real event (you spoke; a sound reached them), and the third
/// fired because time passed. Only the third is gated here, and only when
/// `mode` is `"stage"`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct IdleCognitionSettings {
    /// `"stage"` gates idle turns on the player's neighborhood; `"all"` restores
    /// the unconditional city-wide rotation. A rebuild-free A/B.
    pub mode: String,
    /// Deliberately wider than the 20 m hearing radius: an NPC should already
    /// have been thinking by the time you can hear them, rather than animating
    /// the instant you arrive.
    pub radius_m: f64,
    /// How many neighbours share the rotation. The scheduler still allows only
    /// one request in flight, so this caps how *thinly* the turns are spread,
    /// not how many run at once.
    pub max_actors: usize,
    /// Whether a neighbour also needs something to *react to*
    /// (features/gate_idle_cognition_on_novelty.md). Ignored under `mode: "all"`.
    ///
    /// Proximity alone did not reduce the turn rate — it only aimed it at the
    /// player's head. Standing still in a market still asked the six people
    /// around you, every three seconds, whether anything had changed, and paid a
    /// full prompt each time to hear that it had not. With this on, that costs
    /// nothing: they think when somebody speaks, when a sound reaches them, or
    /// when the crowd around them changes — and otherwise they are simply quiet.
    pub require_news: bool,
    /// Whether *speaking first* is a fact about the character rather than about
    /// the scheduler (features/gate_idle_cognition_on_novelty.md §2). Ignored
    /// unless `require_news`.
    ///
    /// News alone still let every one of the ~500 people you walk past think
    /// about you the moment you appeared. With this on, roughly a fifth of them
    /// do — beggars, hawkers and children rather more, the magistrate and the
    /// anchoress rather less. Nothing here touches how anyone *answers*: speak to
    /// the haughtiest man in the city and he replies at the same latency he
    /// always did.
    pub curiosity: bool,
    /// Multiplies every character's curiosity. `1.0` is the calibrated city (see
    /// the feature doc's measured table); raise it if the streets feel dead,
    /// drop it if they feel like a market of touts. `0.0` silences all unprompted
    /// initiative without touching a single reply.
    pub curiosity_scale: f64,
}

impl Default for IdleCognitionSettings {
    fn default() -> Self {
        Self {
            mode: IdleCognitionMode::Stage.as_str().into(),
            radius_m: DEFAULT_STAGE_RADIUS_M,
            max_actors: DEFAULT_STAGE_MAX_ACTORS,
            require_news: true,
            curiosity: true,
            curiosity_scale: 1.0,
        }
    }
}

impl IdleCognitionSettings {
    pub fn mode(&self) -> IdleCognitionMode {
        IdleCognitionMode::from_config(&self.mode)
    }

    pub fn stage(&self) -> StageConfig {
        StageConfig {
            radius_m: self.radius_m,
            // A rotation nobody is in would never idle at all; `mode: "all"` is
            // the way to say that on purpose.
            max_actors: self.max_actors.max(1),
        }
    }

    pub fn curiosity(&self) -> CuriosityConfig {
        CuriosityConfig {
            enabled: self.curiosity,
            scale: self.curiosity_scale,
        }
    }
}

/// The Night Office (movement M6): once a game day, at their own bedtime, a
/// character may rewrite their own agenda.
///
/// It runs on a **second** cognition lane that yields absolutely to the player:
/// one request in flight, never submitted while anyone is on stage, the floor
/// is busy, the microphone is open or a reply is owed — and dropped silently if
/// the night runs out. Turning it off costs you nothing but a city whose people
/// never change their minds; turning it on costs roughly 39 provider calls a
/// game day.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct NightOfficeSettings {
    pub enabled: bool,
    /// Individual reflection for the ~31 Majors, staggered across the night by
    /// their own bedtimes — 31 calls a game day. This is the tier that pays for
    /// itself: it is what makes an NPC's goal change overnight because of
    /// something that happened to them yesterday.
    pub majors: bool,
    /// Ward-batched reflection for the ~120 Minors, one prompt per ward at the
    /// curfew — 8 calls a game day. Returns a few sentences of mood that every
    /// Minor of that ward then carries.
    pub wards: bool,
    /// The ~350 ambients' code-rolled evening: no provider call at all, so
    /// there is no cost reason to turn this off — only a taste one, if you would
    /// rather the streets were the same every night.
    pub ambients: bool,
}

impl Default for NightOfficeSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            majors: true,
            wards: true,
            ambients: true,
        }
    }
}

impl NightOfficeSettings {
    pub fn config(&self) -> NightOfficeConfig {
        NightOfficeConfig {
            enabled: self.enabled,
            majors: self.majors,
            wards: self.wards,
            ambients: self.ambients,
        }
    }
}

/// The world clock (movement M0). A day/night cycle, the seven offices, and the
/// bell — all a pure projection of the engine's `now`, configured here and
/// resolved in the sim (`features/implemented/movement/01_the_clock.md`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ClockSettings {
    /// Real seconds per game day. 3600 is one game day per real hour (24×).
    pub seconds_per_day: f64,
    /// Which office the run opens on: `dayspring` puts you in a working morning.
    pub start_office: String,
    /// Which day the run opens on; day 0 is a Bellday, 2 a Highmarket.
    pub start_day: i64,
    /// Night's brightness floor. 0.05 is genuinely dark; raise it if the city
    /// becomes unnavigable rather than atmospheric.
    pub night_brightness: f64,
    /// Whether crossing an office rings the town bell for the player.
    pub ring_the_offices: bool,
}

impl Default for ClockSettings {
    fn default() -> Self {
        Self {
            seconds_per_day: 3600.0,
            start_office: "dayspring".into(),
            start_day: 0,
            night_brightness: 0.05,
            ring_the_offices: true,
        }
    }
}

/// Settings for non-speech sound percepts. Perception runs in the engine;
/// these values configure it at construction.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SoundsConfig {
    pub enabled: bool,
    /// Total horizontal FOV for the "saw who did it" test. 135° is a guess —
    /// the one number in this feature only play-testing can settle.
    pub view_cone_degrees: f32,
    /// Engine-side rate limit: sounds inside the cooldown are dropped
    /// silently, so holding F cannot flood NPC inboxes (and the LLM bill).
    pub min_seconds_between_player_sounds: f32,
}

impl Default for SoundsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            view_cone_degrees: 135.0,
            min_seconds_between_player_sounds: 2.0,
        }
    }
}

impl Default for SmartActorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fake_backend: false,
            uv_binary: "uv".into(),
            tts_backend: "local".into(),
            stt_backend: "cloud".into(),
            pause_microphone_during_npc_voice: true,
            stt_streaming: true,
            stt_trailing_silence_ms: 400,
            sounds: SoundsConfig::default(),
            idle_cognition: IdleCognitionSettings::default(),
            night_office: NightOfficeSettings::default(),
            clock: ClockSettings::default(),
            marks: MarksSettings::default(),
            knowledge: KnowledgeSettings::default(),
            extra_ambient_npcs: 0,
        }
    }
}

impl SmartActorsConfig {
    /// The crowd size actually built, with a garbage `config.ron` reported
    /// rather than obeyed. Returns the clamped count and, when it had to
    /// clamp, the line to log — the caller owns the logger.
    pub(crate) fn extra_ambient_npcs(&self) -> (u32, Option<String>) {
        if self.extra_ambient_npcs <= cathedral_sim::MAX_EXTRA_AMBIENT_NPCS {
            return (self.extra_ambient_npcs, None);
        }
        (
            cathedral_sim::MAX_EXTRA_AMBIENT_NPCS,
            Some(format!(
                "extra_ambient_npcs is {}, above the {} ceiling; building {} instead",
                self.extra_ambient_npcs,
                cathedral_sim::MAX_EXTRA_AMBIENT_NPCS,
                cathedral_sim::MAX_EXTRA_AMBIENT_NPCS
            )),
        )
    }

    fn initial_stt_backend(&self) -> bridge::TranscriptionBackend {
        if self.stt_backend.eq_ignore_ascii_case("local") {
            bridge::TranscriptionBackend::Local
        } else {
            bridge::TranscriptionBackend::Cloud
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SmartActorSet {
    DrainBridge,
    ReconcileMirror,
    UpdateFocus,
    CollectInput,
    Present,
}

pub struct SmartActorsPlugin {
    config: SmartActorsConfig,
    weather: WeatherSettings,
}

/// Read-only signal for audio systems that should yield while dialogue is active.
///
/// This deliberately reflects live presentation/input state rather than the
/// player's microphone preference: an armed but idle microphone must not keep
/// ambience permanently ducked.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct AudioActivity {
    pub busy: bool,
}

/// Connection/capability state shared by input and presentation systems.
#[derive(Resource, Debug, Clone)]
pub struct SmartActorRuntime {
    pub connected: bool,
    pub ready: bool,
    pub stt_available: bool,
    pub stt_cloud_available: bool,
    pub stt_local_available: bool,
    pub tts_available: bool,
    pub tts_cloud_available: bool,
    pub tts_local_available: bool,
    pub tts_selected: bridge::TtsBackend,
    tts_selection_pending: Option<(String, bridge::TtsBackend)>,
    /// A user-requested backend change was confirmed and awaits persistence
    /// to `config.ron`.
    tts_selection_dirty: bool,
    next_tts_request: u64,
    pub fake_backend: bool,
    pub mirror_revision: Option<u64>,
    thinking_actor_id: Option<model::ActorId>,
}

impl SmartActorRuntime {
    fn starting(fake_backend: bool) -> Self {
        Self {
            connected: false,
            ready: false,
            stt_available: false,
            stt_cloud_available: false,
            stt_local_available: false,
            tts_available: false,
            tts_cloud_available: false,
            tts_local_available: false,
            tts_selected: bridge::TtsBackend::Off,
            tts_selection_pending: None,
            tts_selection_dirty: false,
            next_tts_request: 0,
            fake_backend,
            mirror_revision: None,
            thinking_actor_id: None,
        }
    }

    pub fn interactions_enabled(&self) -> bool {
        self.connected && self.ready
    }

    fn thinking_actor(&self) -> Option<&model::ActorId> {
        self.thinking_actor_id.as_ref()
    }

    /// The scheduler has one global request slot. Actor-specific terminal rows
    /// only clear the actor they name, so a stale row cannot hide a newer turn.
    fn observe_llm_status(&mut self, state: &str, actor_id: Option<&model::ActorId>) {
        if state == "thinking" {
            self.thinking_actor_id = actor_id.cloned();
        } else if actor_id.is_some_and(|actor_id| self.thinking_actor() == Some(actor_id))
            || (actor_id.is_none() && state == "unavailable")
        {
            self.thinking_actor_id = None;
        }
    }
}

impl SmartActorsPlugin {
    #[allow(
        dead_code,
        reason = "kept for isolated host tests and embedders using clear weather"
    )]
    pub fn new(config: SmartActorsConfig) -> Self {
        Self {
            config,
            weather: WeatherSettings::default(),
        }
    }

    pub fn with_weather(config: SmartActorsConfig, weather: WeatherSettings) -> Self {
        Self { config, weather }
    }
}

impl Plugin for SmartActorsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());
        app.insert_resource(self.weather.clone())
            .init_resource::<WorldWeatherState>()
            .add_message::<WeatherLightning>();
        if app.is_plugin_added::<bevy::audio::AudioPlugin>() {
            app.add_audio_source::<speech::StreamingPcmSource>();
        } else {
            // Headless tests have no audio output, but still need the asset
            // storage used by the presentation systems.
            app.init_asset::<speech::StreamingPcmSource>();
        }
        app.init_resource::<hud::SmartActorHudState>()
            // Soundscape systems can always read this seam, including when
            // smart actors are disabled in config.ron.
            .init_resource::<AudioActivity>()
            .add_systems(Startup, hud::spawn_smart_actor_hud);

        // The law's hands on the player (`law_and_order.md` M4c/M4d). The
        // resource exists unconditionally because `controller.rs` reads its
        // tether every fixed step; with no engine it is simply always empty.
        app.add_plugins(custody::PlayerCustodyPlugin);

        // The Esc settings menu exists even when smart actors are disabled;
        // its rows then report the disabled state instead of toggling.
        app.init_resource::<config_menu::ConfigMenuState>()
            .add_systems(Startup, config_menu::spawn_config_menu)
            .add_systems(
                Update,
                (
                    config_menu::toggle_config_menu,
                    config_menu::handle_config_menu_buttons,
                    config_menu::persist_backend_selections,
                    config_menu::update_config_menu,
                )
                    .chain(),
            );

        // The `I` screen likewise exists whether or not smart actors run: with
        // no mirror it simply shows an empty pack. Only the systems that write
        // a `PlayerIntent` need the engine, and they are registered below.
        app.init_resource::<inventory_ui::InventoryUiState>()
            .add_systems(Startup, inventory_ui::spawn_inventory_ui)
            .add_systems(
                Update,
                (
                    // After the settings menu's own toggle, so an Escape that
                    // opens the menu closes the inventory in the same frame.
                    inventory_ui::toggle_inventory,
                    inventory_ui::handle_inventory_tile_clicks,
                    inventory_ui::refresh_inventory_ui,
                    inventory_ui::update_inventory_ui,
                )
                    .chain()
                    .after(config_menu::update_config_menu),
            );

        if !self.config.enabled {
            let mut hud = hud::SmartActorHudState::default();
            hud.connection = hud::ConnectionUiState::Disabled;
            hud.connection_detail = "Disabled in config.ron".into();
            hud.set_transcription_capabilities(false, false);
            hud.set_npc_voice_backend(bridge::TtsBackend::Off);
            app.insert_resource(hud)
                .add_systems(Update, hud::update_smart_actor_hud);
            return;
        }

        let (handle, inbox, worker, engine) = local_engine::spawn(&self.config, &self.weather);
        app.insert_non_send(engine);

        app.insert_resource(handle)
            .insert_resource(inbox)
            .insert_resource(worker)
            .init_resource::<model::WorldMirror>()
            .init_resource::<clock::WorldClockState>()
            .init_resource::<model::MovementInbox>()
            .init_resource::<lamps::CityLamps>()
            .init_resource::<dogs::DogInbox>()
            // Owned by `CityPlugin`, which draws from it, but the drain writes
            // it — and a headless test app that runs the drain without the city
            // must not panic for the want of a resource it never reads.
            .init_resource::<crate::city::ChalkStanding>()
            .insert_resource(SmartActorRuntime::starting(self.config.fake_backend))
            .init_resource::<area_debug::AreaDebugState>()
            .init_resource::<actor_sheet::InspectedActor>()
            .init_resource::<ActorFocus>()
            .init_resource::<interaction::InteractionState>()
            .init_resource::<interaction::PlayerSpatialState>()
            .init_resource::<body::ReflexState>()
            .init_resource::<chat::ChatInputState>()
            .insert_resource(interaction::MicrophoneInputState::with_backend(
                self.config.initial_stt_backend(),
            ))
            .init_resource::<speech::SpeechPresentationState>()
            .add_message::<interaction::PlayerIntent>()
            .add_message::<InjectPlayerTranscript>()
            // Idempotent when InputPlugin already registered them; needed for
            // the chat box in headless harnesses that skip Bevy's input plugin.
            // The box eats the layout-resolved button map as well as the
            // physical one, so both have to exist.
            .add_message::<bevy::input::keyboard::KeyboardInput>()
            .init_resource::<ButtonInput<bevy::input::keyboard::Key>>()
            .add_message::<speech::PresentSpeech>()
            .add_message::<speech::TtsClipReady>()
            .add_message::<speech::TtsClipFailed>()
            .add_message::<speech::TtsPcmChunkReady>()
            .add_message::<speech::TtsStreamFinished>()
            .add_message::<speech::StopNpcSpeech>()
            .add_message::<speech::ClearSpeechPresentation>()
            .add_message::<sound::PlaySoundEffect>()
            .add_message::<crate::soundscape::SoundscapeCue>()
            .add_message::<hands::HandoverFeedback>()
            .init_resource::<hands::GripHolds>()
            .add_message::<body::PresentGesture>()
            // The inventory's menu entries are the only part of the screen that
            // needs the engine (they write intents), so they live here rather
            // than in the always-on block above.
            .add_systems(
                Update,
                inventory_ui::handle_inventory_actions
                    .after(inventory_ui::handle_inventory_tile_clicks)
                    .before(inventory_ui::refresh_inventory_ui),
            )
            .configure_sets(
                PostUpdate,
                (
                    SmartActorSet::DrainBridge,
                    SmartActorSet::ReconcileMirror,
                    SmartActorSet::UpdateFocus,
                    SmartActorSet::CollectInput,
                    SmartActorSet::Present,
                )
                    // The *order* is the contract; the stop-the-world join
                    // between the sets is not. A plain `.chain()` makes Bevy
                    // mint an `ApplyDeferred` on every edge whose upstream owns
                    // a `Commands` — and `ApplyDeferred` is exclusive, so it
                    // also halts the heavy Bevy work that shares PostUpdate
                    // (visibility, light frusta, UI layout) for as long as the
                    // slowest of our systems takes. `chain_ignore_deferred`
                    // keeps every ordering edge and drops the barrier.
                    //
                    // Almost nothing across a set boundary reads an entity a
                    // previous set queued through `Commands`. The drain's only
                    // deferred write is inserting/removing the microphone
                    // service resource, which every reader takes as an `Option`
                    // and which is read for the first time a frame later either
                    // way (the worker takes far longer than a frame to open a
                    // device); every other hand-off between the sets is a
                    // resource or a component written in place, which is never
                    // deferred. The one exception is the pack: `sync_dogs`
                    // spawns a dog's rig and `drive_dog_bodies` gives it its
                    // `DogMotion`, both of which `dogs::animate_dog_gait` in
                    // `Present` now first sees a frame later — which costs a
                    // freshly spawned dog one frame in its rest pose, at the
                    // one moment of its life it is standing still anyway. The
                    // syncs that DO carry data are named and kept inside
                    // `ReconcileMirror` and `Present` below.
                    .chain_ignore_deferred()
                    .after(TransformSystems::Propagate),
            )
            .add_systems(
                Startup,
                (
                    hands::setup_item_prop_assets,
                    (body::setup_body_assets, body::spawn_body_lineup).chain(),
                    road_carts::setup_road_cart_assets,
                    area_debug::spawn_area_debug_ui,
                    actor_sheet::spawn_actor_sheet,
                    clock::spawn_clock_hud,
                    chat::spawn_chat_input,
                )
                    .after(hud::spawn_smart_actor_hud),
            )
            .add_systems(
                PreUpdate,
                // After input collection (and after drive-mode injection, which
                // orders itself before this set) so the box reads this frame's
                // keys and its `ButtonInput` reset hides them from everyone
                // running later.
                chat::collect_chat_input
                    .in_set(chat::ChatInputSet)
                    .after(bevy::input::InputSystems),
            )
            // The city marks its wells and cisterns during Startup; their loops
            // start once, after every fixture exists.
            .add_systems(PostStartup, sound::start_water_ambience)
            .add_systems(
                PostUpdate,
                // The engine polls first: a command written in this frame's
                // CollectInput is answered no later than the next frame's drain,
                // the same latency the sidecar had.
                (local_engine::pump_local_engine, drain_bridge_messages)
                    .chain()
                    .in_set(SmartActorSet::DrainBridge),
            )
            .add_systems(
                PostUpdate,
                (
                    // The only two edges in this set that carry queued
                    // `Commands` rather than mere order, and therefore the only
                    // `ApplyDeferred`s left in it. Reconcile spawns the bodies —
                    // hand anchors, pose state, name labels — that both hand
                    // systems then look up by entity; and
                    // `apply_handover_feedback` spawns the hand-over flight that
                    // `reconcile_hand_props` *must* see in the same frame, since
                    // a flight it cannot see is a prop it re-mints into the
                    // receiving hand, doubling the item for the flight's 0.3 s.
                    (
                        actors::reconcile_actor_views,
                        // Feedback first (the giver's in-hand prop must still exist
                        // to launch a hand-over flight from), then the hand-prop
                        // reconcile that would retire it (npc_bodies M2).
                        hands::apply_handover_feedback,
                        hands::reconcile_hand_props,
                    )
                        .chain(),
                    // From here down the ordering is about which write to a
                    // *transform* or a resource wins the frame, not about seeing
                    // a predecessor's commands: everything below queries bodies
                    // the reconcile above already published through the sync
                    // point, and the one place a system here does read an entity
                    // a predecessor in this same run spawned — the pack, below —
                    // is spelled out where it happens. `chain_ignore_deferred`
                    // keeps the order exactly and drops four more global
                    // barriers.
                    (
                        // The deliberate body (npc_bodies M4): start one-shot poses
                        // from gesture triggers and keep the looping-dance flag in
                        // step with the snapshot, on the roots reconcile just placed.
                        body::drive_gesture_pose,
                        interaction::reconcile_interaction_state,
                        // After reconcile, so it overrides the stale snapshot position
                        // reconcile writes for a mover between revisions with the live
                        // interpolated pose off the hot channel.
                        actors::drive_npc_bodies,
                        // …and after that in turn: a custody grip aims at where the
                        // prisoner is *this* frame, which for anyone being walked to
                        // a station is the hot channel's position, not the mirror's
                        // (`law_and_order.md` M4c).
                        hands::hold_the_seized,
                        road_carts::reconcile_road_carts,
                        // The dog pack (features/implemented/dogs.md): bodies stood up off the
                        // hot channel, then swept between its 20 Hz ticks like the
                        // human movers above. A dog is spawned already standing at
                        // its own sample, so the sweep picking it up a frame later
                        // moves nothing.
                        dogs::sync_dogs,
                        dogs::drive_dog_bodies,
                    )
                        .chain_ignore_deferred(),
                )
                    .chain_ignore_deferred()
                    .in_set(SmartActorSet::ReconcileMirror),
            )
            .add_systems(
                PostUpdate,
                targeting::update_actor_focus.in_set(SmartActorSet::UpdateFocus),
            )
            .add_systems(
                PostUpdate,
                (
                    interaction::select_inventory_item,
                    interaction::sync_player_position,
                    interaction::poll_microphone,
                    interaction::collect_item_interaction_input,
                    interaction::collect_sound_input,
                    interaction::update_microphone_toggle,
                    update_tts_backend_toggle,
                    collect_injected_transcripts,
                    forward_player_intents,
                )
                    // Order matters (the readers of `PlayerIntent` run last),
                    // but not one of these nine owns a `Commands`: they trade in
                    // resources, messages and the bridge channel only. A plain
                    // `.chain()` here would still mint a barrier, because it
                    // would be the first non-ignored edge downstream of the
                    // drain and the reconcile and would cash in *their* postponed
                    // commands mid-set. Anything added here that does defer must
                    // re-examine this line.
                    .chain_ignore_deferred()
                    .in_set(SmartActorSet::CollectInput),
            )
            .add_systems(
                PostUpdate,
                (
                    // Everything that *writes* the presentation: labels, props,
                    // poses, the speech intake and the two teardowns. The order
                    // between them is the contract, but not one of them reads an
                    // entity another spawns or despawns in the same run, so the
                    // whole block keeps its sequence without an `ApplyDeferred`
                    // per `Commands` user in it.
                    (
                        actors::position_actor_name_labels,
                        actors::update_thinking_indicators,
                        // Hand-over props mid-flight between two hands (M2).
                        // Its spawns and despawns are read next frame, by
                        // `hands::reconcile_hand_props`.
                        hands::animate_handover_flights,
                        // The reflex bookkeeping (npc_bodies M3) feeds the pose
                        // pipeline (M1) that runs right after it: who is talking
                        // until when and what recently made a sound, fed from the
                        // same presentation messages the bubbles and speakers
                        // consume — then the cosmetic part-level animation over
                        // the roots the ReconcileMirror set just placed; it never
                        // touches a root transform. (Nested tuple: the flat chain
                        // would exceed Bevy's 20-system tuple limit.)
                        (
                            body::track_reflex_signals,
                            body::animate_body_pose,
                            // The dogs' trot/wag — same cosmetics-only contract.
                            dogs::animate_dog_gait,
                        )
                            .chain_ignore_deferred(),
                        speech::receive_speech_events,
                        speech::receive_tts_clips,
                        speech::receive_tts_pcm_chunks,
                        speech::receive_tts_stream_ends,
                        speech::receive_tts_failures,
                        // Teardown order only — both take the voice apart through
                        // the same resource, and both despawn with `try_despawn`,
                        // so neither needs the other's commands applied. They
                        // stay on *this* side of the sync below because
                        // `update_speech_bubbles` despawns bubbles outright: with
                        // the teardown's sweep applied first, an expiring bubble
                        // it has already taken is simply gone from the query
                        // rather than despawned twice. The price is that a line
                        // spoken in the same drain as the engine's death is not
                        // in `clear_speech_presentation`'s sweep — its bubble
                        // hangs until it expires and the reaper takes the stack.
                        (
                            speech::clear_speech_presentation,
                            speech::stop_npc_speech_for_capture,
                        )
                            .chain_ignore_deferred(),
                    )
                        .chain_ignore_deferred(),
                    // The one real sync point left in `Present`, and it earns it
                    // twice over. `update_audio_activity` counts the `NpcVoice`
                    // entities the two teardowns above have just despawned, and
                    // would otherwise leave the soundscape ducked for a frame
                    // after a voice was cut. And `update_speech_bubbles` reaps
                    // every stack with no live bubble left in it — so a stack
                    // whose old line expires on the same frame a new one is
                    // spoken must see the new bubble, or it despawns the stack
                    // with the fresh line queued inside it and that line is never
                    // drawn at all. (`start_ready_audio` runs in Update, while
                    // the teardown pair above owns voice teardown; keeping this
                    // boundary here makes both edges visible before the later
                    // presentation and audio consumers.)
                    (
                        update_audio_activity,
                        speech::update_speech_bubbles,
                        speech::update_subtitle_hud,
                        sound::play_sound_effects,
                        sound::expire_stalled_sound_effects,
                        area_debug::update_area_debug_ui,
                        area_debug::update_actor_status_visibility,
                        actor_sheet::update_actor_sheet,
                        chat::update_chat_input_ui,
                        hud::update_smart_actor_hud,
                    )
                        // The readers, in order, and again with no barrier
                        // between them: a sound effect is spawned already at its
                        // final volume and a stalled one is only ever reaped a
                        // frame after it stalled, so nothing here needs a
                        // predecessor's commands. `Present` is the last set of
                        // the frame; what it queues is applied by the schedule's
                        // own apply at the end of PostUpdate, before anything
                        // renders or ticks again.
                        .chain_ignore_deferred(),
                )
                    .chain()
                    .in_set(SmartActorSet::Present),
            )
            .add_systems(
                Update,
                // Creating the player before PostUpdate guarantees Bevy's
                // audio playback systems see it in that same frame. When this
                // lived alongside those systems, a completed source handoff
                // could occasionally miss sink attachment.
                speech::start_ready_audio,
            )
            .add_systems(
                Update,
                // The world clock's consumers: rotate the sun, keep the readout
                // current, and cycle the debug time scale on the `T` key. They
                // read `WorldClockState`, refreshed each frame in the drain.
                (
                    clock::update_clock_hud,
                    clock::handle_time_scale_key,
                    // The lamp mirror (M7): stand the posts up and flip their
                    // glass and glow as the sim's set changes.
                    lamps::sync_lamp_props,
                ),
            );
        if app.is_plugin_added::<bevy::gizmos::GizmoPlugin>() {
            app.add_systems(
                PostUpdate,
                area_debug::draw_area_boxes
                    .in_set(SmartActorSet::Present)
                    // It draws the boxes the debug UI has just decided on — an
                    // order, not a command hand-off. A plain `.after` would be
                    // the one non-ignored edge left at the tail of `Present` and
                    // would cash in the whole set's postponed commands right
                    // there, putting a global barrier back on the last system of
                    // the frame.
                    .after_ignore_deferred(area_debug::update_area_debug_ui),
            );
        }
    }
}

fn update_audio_activity(
    npc_voices: Query<(), With<speech::NpcVoice>>,
    microphone: Res<interaction::MicrophoneInputState>,
    chat: Res<chat::ChatInputState>,
    mut activity: ResMut<AudioActivity>,
) {
    activity.busy =
        npc_voices.iter().next().is_some() || microphone.recording_active() || chat.open;
}

/// Developer/test-only transcript injection. The engine accepts it exclusively
/// in deterministic fake mode and still applies the normal `say` validator;
/// production has no typed-chat path.
#[derive(Message, Debug, Clone)]
pub struct InjectPlayerTranscript {
    pub text: String,
    pub target_id: Option<model::ActorId>,
}

/// The engine's two render-only "hot" channels, bundled so `drain_bridge_messages`
/// stays under Bevy's 16-parameter system limit.
///
/// **Every `ResMut` in here is wrapper-only.** Hand one to
/// [`process_engine_message`] as `&mut ResMut<_>`, never as `&mut _`: the
/// coercion Rust would perform for the bare form goes through `DerefMut` and
/// stamps the changed tick for *every* message, in *every* arm, and `Clock` and
/// `Weather` arrive on every poll. Bare, these resources are flagged changed
/// forever and every `is_changed()` gate that reads them is dead code. Deref
/// inside the arm that genuinely writes.
#[derive(SystemParam)]
struct HotChannels<'w, 's> {
    movement: ResMut<'w, model::MovementInbox>,
    lamps: ResMut<'w, lamps::CityLamps>,
    dogs: ResMut<'w, dogs::DogInbox>,
    weather: ResMut<'w, WorldWeatherState>,
    lightning: MessageWriter<'w, WeatherLightning>,
    /// The player's standing with the law (`law_and_order.md` M4). Hot for the
    /// same reason movement is: the tether it drives is clamped every frame.
    law: ResMut<'w, custody::PlayerCustodyState>,
    /// What the player's hand could chalk (`chalking_the_walls.md` M3). Hot
    /// because it changes with every step taken near a door.
    chalk: ResMut<'w, crate::city::ChalkStanding>,
    time: Res<'w, Time>,
    drain_timer: Local<'s, DrainTimer>,
}

#[derive(SystemParam)]
struct BridgePresentationWriters<'w> {
    speech: MessageWriter<'w, speech::PresentSpeech>,
    wav: MessageWriter<'w, speech::TtsClipReady>,
    pcm: MessageWriter<'w, speech::TtsPcmChunkReady>,
    failure: MessageWriter<'w, speech::TtsClipFailed>,
    stream_end: MessageWriter<'w, speech::TtsStreamFinished>,
    clear: MessageWriter<'w, speech::ClearSpeechPresentation>,
    sound_effects: MessageWriter<'w, sound::PlaySoundEffect>,
    soundscape: MessageWriter<'w, crate::soundscape::SoundscapeCue>,
    hands: MessageWriter<'w, hands::HandoverFeedback>,
    gestures: MessageWriter<'w, body::PresentGesture>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SoundscapeRoute {
    cue: Option<crate::soundscape::SoundscapeCue>,
    replace_standard: bool,
}

fn soundscape_route(sound_id: &str, position: Vec3) -> SoundscapeRoute {
    use crate::soundscape::SoundscapeCue;

    match sound_id {
        "market_cry" => SoundscapeRoute {
            cue: Some(SoundscapeCue::MarketCry { position }),
            replace_standard: true,
        },
        "draw_water" | "chain_windlass" => {
            match crate::soundscape::classify_special_well(position) {
                Some(source) => SoundscapeRoute {
                    cue: Some(SoundscapeCue::WellDraw { source }),
                    replace_standard: true,
                },
                None => SoundscapeRoute {
                    cue: None,
                    replace_standard: false,
                },
            }
        }
        "coin_clink" => SoundscapeRoute {
            cue: crate::soundscape::tallage_measurement_anchor(position)
                .map(|position| SoundscapeCue::MarketMeasurement { position }),
            // The balance pans supplement the coins; they do not replace them.
            replace_standard: false,
        },
        _ => SoundscapeRoute {
            cue: None,
            replace_standard: false,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn drain_bridge_messages(
    mut commands: Commands,
    config: Res<SmartActorsConfig>,
    inbox: Res<bridge::BridgeInbox>,
    handle: Res<bridge::BridgeHandle>,
    players: Query<&GlobalTransform, With<crate::controller::PlayerController>>,
    microphone: Option<Res<microphone::MicrophoneService>>,
    mut mirror: ResMut<model::WorldMirror>,
    mut runtime: ResMut<SmartActorRuntime>,
    mut hud: ResMut<hud::SmartActorHudState>,
    mut interaction: ResMut<interaction::InteractionState>,
    mut microphone_input: ResMut<interaction::MicrophoneInputState>,
    mut spatial: ResMut<interaction::PlayerSpatialState>,
    mut presentation: BridgePresentationWriters,
    mut world_clock: ResMut<clock::WorldClockState>,
    mut hot: HotChannels,
    // Speech presentation dedupes and orders by this (speech.rs), and the
    // engine's messages no longer carry a sequence of their own. Counting them
    // here gives the same monotonic, gap-free stream the envelope did.
    mut message_seq: Local<u64>,
) {
    let _span = crate::perf::span(crate::perf::Probe::BridgeDrain);
    let drain_started = std::time::Instant::now();
    // Resource insertions/removals are deferred. Track what this drain pass
    // has queued so several buffered engine messages cannot spawn several
    // microphone workers before commands are applied.
    let mut microphone_present = microphone.is_some();
    // Drain everything first: after a long frame the channel can hold several
    // polls' worth of messages, and a full Snapshot is a whole-world
    // replacement — validating each one costs an O(cast) rebuild. Only
    // *strictly consecutive* snapshots coalesce (keep the last of the run): a
    // message between two snapshots — a speech line, a gesture — may
    // reference actors the earlier snapshot introduced, so it must be
    // processed against that snapshot's mirror, not a newer one.
    let mut events = Vec::new();
    while let Some(event) = inbox.try_recv() {
        events.push(event);
    }
    let is_snapshot: Vec<bool> = events
        .iter()
        .map(|event| {
            matches!(event, bridge::BridgeEvent::Message(message)
                if matches!(**message, EngineMessage::Snapshot(_)))
        })
        .collect();
    for (event_index, event) in events.into_iter().enumerate() {
        match event {
            bridge::BridgeEvent::ProcessStarted => {
                runtime.connected = true;
                runtime.ready = false;
                runtime.thinking_actor_id = None;
                hud.connection = hud::ConnectionUiState::Starting;
                hud.connection_detail = "Actor engine starting; handshaking…".into();
                let Ok(player) = players.single() else {
                    hud.clear_transients_on_disconnect("Player transform is unavailable");
                    continue;
                };
                let position = player.translation();
                let Ok(position_m) = model::Position::try_from(position) else {
                    hud.clear_transients_on_disconnect("Player position is invalid");
                    continue;
                };
                let spatial_seq = spatial.mark_hello_position(position);
                if let Err(error) = handle.try_send(bridge::BridgeCommand::Hello {
                    position_m,
                    spatial_seq,
                }) {
                    hud.clear_transients_on_disconnect(error);
                }
            }
            bridge::BridgeEvent::Message(message) => {
                // Once the engine is known dead, never let a buffered late
                // message revive interactions against stale state; this slice
                // has no in-place restart.
                if !runtime.connected {
                    continue;
                }
                *message_seq += 1;
                if is_snapshot[event_index] && is_snapshot.get(event_index + 1) == Some(&true) {
                    continue;
                }
                process_engine_message(
                    *message,
                    *message_seq,
                    &mut mirror,
                    &mut runtime,
                    &mut hud,
                    &mut interaction,
                    &mut presentation,
                    &mut world_clock,
                    &mut hot.movement,
                    &mut hot.lamps,
                    &mut hot.dogs,
                    &mut hot.weather,
                    &mut hot.lightning,
                    &mut hot.law,
                    &mut hot.chalk,
                    hot.time.elapsed_secs_f64(),
                );
                // Do not open the default input device before the engine
                // handshake confirms that transcription is configured.
                if runtime.ready && runtime.stt_available && !microphone_present {
                    commands.insert_resource(microphone::MicrophoneService::spawn(
                        handle.runtime_dir().to_path_buf(),
                        handle.command_sender(),
                        microphone::clamped_trailing_silence(config.stt_trailing_silence_ms),
                    ));
                    microphone_present = true;
                }
            }
            bridge::BridgeEvent::Disconnected(message) => {
                runtime.connected = false;
                runtime.ready = false;
                runtime.stt_available = false;
                runtime.stt_cloud_available = false;
                runtime.stt_local_available = false;
                runtime.tts_available = false;
                runtime.tts_cloud_available = false;
                runtime.tts_local_available = false;
                runtime.tts_selected = bridge::TtsBackend::Off;
                runtime.tts_selection_pending = None;
                runtime.thinking_actor_id = None;
                interaction.clear_pending();
                microphone_input.clear_on_disconnect();
                // The hot channels freeze wherever the engine's last message
                // left them, which for three of them is merely stale scenery — a
                // walker stopped mid-street, a lamp left lit, a sky that stops
                // changing. The law's is not: it holds the player's feet, and
                // nothing above can ever release them again, so it is let go of
                // here (see [`custody::PlayerCustodyState::clear_on_disconnect`]).
                hot.law.clear_on_disconnect();
                hud.clear_transients_on_disconnect(truncate_owned(message, 300));
                presentation.clear.write(speech::ClearSpeechPresentation);
                if microphone_present {
                    commands.remove_resource::<microphone::MicrophoneService>();
                    microphone_present = false;
                }
            }
        }
    }

    // Rolling attribution (the pose system's pattern): the mirror rebuild on
    // snapshot arrival is the O(cast) cost this drain can hide.
    let elapsed_us = drain_started.elapsed().as_secs_f64() * 1e6;
    let now = hot.time.elapsed_secs_f64();
    let drain_timer = &mut *hot.drain_timer;
    drain_timer.accum_us += elapsed_us;
    drain_timer.max_us = drain_timer.max_us.max(elapsed_us);
    drain_timer.frames += 1;
    if now - drain_timer.window_start >= 5.0 {
        if drain_timer.window_start > 0.0 {
            info!(
                "[bridge drain] avg {:.0} us, max {:.0} us over {} frames",
                drain_timer.accum_us / f64::from(drain_timer.frames.max(1)),
                drain_timer.max_us,
                drain_timer.frames,
            );
        }
        *drain_timer = DrainTimer {
            window_start: now,
            ..Default::default()
        };
    }
}

#[derive(Default)]
struct DrainTimer {
    window_start: f64,
    accum_us: f64,
    max_us: f64,
    frames: u32,
}

/// One authoritative message, typed.
///
/// Every arm used to begin by deserializing a `serde_json::Value` and toasting
/// on failure. The message is now the engine's own value, so what remains is
/// the *sanitation* the projection still owes the UI: an NPC's line and a
/// backend's error string are the two texts nobody in this process wrote.
#[allow(clippy::too_many_arguments)]
fn process_engine_message(
    message: EngineMessage,
    message_seq: u64,
    // The `ResMut` wrapper, not `&mut WorldMirror`: coercing at the call site
    // would DerefMut-flag the mirror on EVERY message (Clock and Weather
    // arrive every frame), making every downstream `mirror.is_changed()` gate
    // permanently hot. Only an accepted snapshot below may flag it.
    mirror: &mut ResMut<model::WorldMirror>,
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
    interaction: &mut interaction::InteractionState,
    presentation: &mut BridgePresentationWriters,
    world_clock: &mut clock::WorldClockState,
    // Every one of the five hot channels below takes the `ResMut` wrapper for
    // the mirror's reason, and the whole of [`HotChannels`] is wrapper-only by
    // rule: `&mut hot.movement` deref-coerces through `DerefMut`, which stamps
    // the changed tick for *every* message in *every* arm, and `Clock` and
    // `Weather` arrive on every poll. Taken bare, these five are permanently
    // flagged and the two live gates that read them — `law_standing_hud`
    // (custody.rs) and `sync_dogs` (dogs.rs) — never skip a frame. Deref
    // explicitly, inside the arm that actually writes.
    movement_inbox: &mut ResMut<model::MovementInbox>,
    city_lamps: &mut ResMut<lamps::CityLamps>,
    dog_inbox: &mut ResMut<dogs::DogInbox>,
    weather: &mut ResMut<WorldWeatherState>,
    lightning: &mut MessageWriter<WeatherLightning>,
    law: &mut ResMut<custody::PlayerCustodyState>,
    // The `ResMut` wrapper for the same reason the mirror keeps one: the sign
    // picker resets on this resource's change flag, so it must be flagged when
    // what is within reach moves and not merely when a message arrives.
    chalk: &mut ResMut<crate::city::ChalkStanding>,
    received_at_seconds: f64,
) {
    match message {
        EngineMessage::Ready {
            capabilities,
            snapshot,
        } => {
            runtime.thinking_actor_id = None;
            // A rejected first snapshot means the seeded world itself is
            // unrenderable — a sim bug, not a lost message, and there is no
            // resync left to ask for. The handshake simply never completes and
            // the HUD keeps saying so.
            if accept_snapshot(
                &mut **mirror,
                runtime,
                hud,
                &mut **movement_inbox,
                &snapshot,
            ) {
                apply_ready_capabilities(runtime, hud, capabilities);
            }
        }
        EngineMessage::Snapshot(snapshot) => {
            accept_snapshot(
                &mut **mirror,
                runtime,
                hud,
                &mut **movement_inbox,
                &snapshot,
            );
        }
        EngineMessage::Clock {
            day,
            day_fraction,
            office,
            weekday,
            brightness,
            scale,
            seconds_per_day,
        } => {
            // Mirror the office and the debug time scale into `logs.jsonl` on
            // change, so a `CATHEDRAL_DRIVE` script can assert on the clock in
            // text instead of reading a screenshot (01_the_clock.md §6). One line
            // per change, never per poll.
            let office_changed = world_clock.office != office || !world_clock.present;
            let scale_changed = (world_clock.scale - scale).abs() > 0.5;
            if office_changed || scale_changed {
                let minutes = (day_fraction * 24.0 * 60.0).round() as i64;
                let (hour, minute) = (
                    minutes.div_euclid(60).rem_euclid(24),
                    minutes.rem_euclid(60),
                );
                crate::session_log::log_line(
                    "clock",
                    "INFO",
                    &format!(
                        "[clock] {} {hour:02}:{minute:02} · day {day} {} · {}×",
                        office.label(),
                        weekday.label(),
                        scale.round() as i64,
                    ),
                );
            }
            // The sim owns the clock; this is the game's read-only projection,
            // consumed by the sun and the HUD. No snapshot, no revision — the
            // clock changes every frame and must never touch the mirror.
            *world_clock = clock::WorldClockState {
                present: true,
                day,
                fraction: day_fraction,
                office,
                weekday,
                brightness,
                scale,
                seconds_per_day,
            };
        }
        EngineMessage::Weather(sample) => {
            (**weather).receive(sample, received_at_seconds);
        }
        EngineMessage::Lightning(strike) => {
            lightning.write(WeatherLightning(strike));
        }
        EngineMessage::Movement { moved } => {
            // The hot channel, like `Clock`: mover poses arrive every 20 Hz tick
            // and must never touch the mirror or bump a revision — routing them
            // through the cold snapshot would republish the whole world 20 times
            // a second (06_engineering.md, the hot/cold split). Each mover's
            // latest pose lands in a plain resource, `seq` bumped so
            // `actors::drive_npc_bodies` (which runs after the reconcile pass and
            // wins over the stale snapshot position it wrote) can interpolate
            // between successive ticks.
            for motion in moved {
                let actor_id = model::actor_id_from_sim(&motion.actor_id);
                let sample = (**movement_inbox).0.entry(actor_id).or_default();
                sample.position = model::vec3_from_sim(motion.position_m);
                sample.facing_yaw = motion.facing_yaw as f32;
                sample.speed = motion.speed;
                sample.gait_phase = motion.gait_phase;
                sample.seq = sample.seq.wrapping_add(1);
            }
        }
        EngineMessage::Lamps { lamps: set } => {
            // The lamp channel (M7), like `Clock`: a render-only mirror, no
            // snapshot, no revision. The sync system stands the posts up and
            // flips their glow from this resource. One `logs.jsonl` line per
            // change (the clock's pattern), so a drive script can assert on
            // the lighting in text instead of reading a screenshot.
            let lit = set.iter().filter(|lamp| lamp.lit).count();
            crate::session_log::log_line(
                "lamps",
                "INFO",
                &format!("[lamps] {lit}/{} lit", set.len()),
            );
            let city_lamps = &mut **city_lamps;
            city_lamps.lamps = set
                .into_iter()
                .map(|lamp| lamps::LampState {
                    position: model::vec3_from_sim(lamp.position_m),
                    lit: lamp.lit,
                    square: lamp.square,
                })
                .collect();
            city_lamps.revision += 1;
        }
        EngineMessage::Dogs { dogs: pack } => {
            // The dog channel, like `Movement`: whole-pack poses at 20 Hz, no
            // mirror, no revision. `seq` bumps per received sample so the
            // interpolation sweeps between successive ticks.
            let dog_inbox = &mut **dog_inbox;
            for view in pack {
                let sample = dog_inbox
                    .0
                    .entry(view.id.as_str().to_string())
                    .or_insert_with(|| dogs::DogSample {
                        name: view.name.clone(),
                        coat: view.coat,
                        build: view.build,
                        position: model::vec3_from_sim(view.position_m),
                        facing_yaw: view.facing_yaw as f32,
                        speed: view.speed,
                        gait_phase: view.gait_phase,
                        seq: 0,
                    });
                sample.position = model::vec3_from_sim(view.position_m);
                sample.facing_yaw = view.facing_yaw as f32;
                sample.speed = view.speed;
                sample.gait_phase = view.gait_phase;
                sample.seq = sample.seq.wrapping_add(1);
            }
        }
        EngineMessage::Speech {
            event_id,
            speaker_id,
            target_id,
            text,
            speaker_position_m,
            recipient_ids,
            speaker_name_for_player,
        } => {
            let speaker_id = model::actor_id_from_sim(&speaker_id);
            let recipients: Vec<model::ActorId> =
                recipient_ids.iter().map(model::actor_id_from_sim).collect();
            if !valid_ui_text(&speaker_name_for_player, 256)
                || !valid_ui_text(&text, PLAYER_SPEECH_MAX_CHARS)
                || mirror.actor(&speaker_id).is_none()
            {
                hud.toast("Discarded invalid speech data from the actor engine");
                return;
            }
            let player_heard =
                speaker_id.0 == PLAYER_ID || recipients.iter().any(|id| id.0 == PLAYER_ID);
            if !player_heard {
                return;
            }
            let recipient_count = recipients
                .iter()
                .filter(|recipient| **recipient != speaker_id && mirror.actor(recipient).is_some())
                .count();
            presentation.speech.write(speech::PresentSpeech {
                event_seq: message_seq,
                event_id: event_id.0,
                speaker_id: speaker_id.clone(),
                speaker_label: speaker_name_for_player,
                target_id: target_id.as_ref().map(model::actor_id_from_sim),
                text,
                speaker_position: model::vec3_from_sim(speaker_position_m),
                recipient_count,
                expect_audio: tts_selection_is_usable(runtime) && speaker_id.0 != PLAYER_ID,
            });
        }
        EngineMessage::Sound {
            event_id: _,
            sound_id,
            sound_class: _,
            actor_id,
            position_m,
            audible_distance,
            recipient_ids,
            witness_ids: _,
            text_for_player,
        } => {
            let audible_distance = audible_distance as f32;
            if !sound::valid_sound_id(&sound_id)
                || !audible_distance.is_finite()
                || audible_distance <= 0.0
                || audible_distance > 10_000.0
                || text_for_player
                    .as_deref()
                    .is_some_and(|text| !valid_ui_text(text, 300))
            {
                hud.toast("Discarded invalid sound data from the actor engine");
                return;
            }
            // The engine rendered the player's percept (or None when the player
            // is out of range); Bevy never decides what is known.
            if let Some(text) = text_for_player {
                hud.toast(text);
            }
            let player_made_it = actor_id
                .as_ref()
                .is_some_and(|actor_id| actor_id.as_str() == PLAYER_ID);
            let player_heard = player_made_it
                || recipient_ids
                    .iter()
                    .any(|recipient| recipient.as_str() == PLAYER_ID);
            if player_heard {
                let position = model::vec3_from_sim(position_m);
                // These genuine world events drive the richer environmental
                // recordings, but remain outside `PlaySoundEffect`: that
                // message also drives NPC gaze reflexes, and routine market /
                // well machinery must not make every nearby body stare at it.
                let route = soundscape_route(&sound_id, position);
                if let Some(cue) = route.cue {
                    presentation.soundscape.write(cue);
                }
                if !route.replace_standard {
                    presentation.sound_effects.write(sound::PlaySoundEffect {
                        sound_id,
                        position,
                        audible_distance,
                    });
                }
            }
        }
        EngineMessage::WorldEvent {
            event_id: _,
            kind,
            actor_id,
            target_id,
            item_id,
            quantity,
            recipient_ids,
        } => {
            // Presentation feedback only. Offers and ownership still reconcile
            // exclusively from authoritative snapshots.
            let actor = model::actor_id_from_sim(&actor_id);
            let target = target_id.as_ref().map(model::actor_id_from_sim);
            let item = item_id.as_ref().map(model::item_id_from_sim);
            // An offer of the player's that ended with nothing changing hands
            // gets the notice slot and its reason, not the 4 s toast the next
            // event would overwrite. Everything else — including two other
            // people refusing each other — toasts exactly as it always did.
            if let Some((headline, reason)) = describe_offer_rejection(
                &kind,
                &actor,
                target.as_ref(),
                item.as_ref(),
                quantity,
                mirror,
            ) {
                hud.show_offer_outcome(&headline, &reason);
            } else if let Some(text) = describe_world_event(
                &kind,
                &actor,
                target.as_ref(),
                item.as_ref(),
                quantity,
                &recipient_ids
                    .iter()
                    .map(model::actor_id_from_sim)
                    .collect::<Vec<_>>(),
                &**mirror,
            ) {
                hud.toast(text);
            }
            // The hand-over choreography (npc_bodies M2): the same events, as
            // body language. `sale` never reaches the toast above
            // (`describe_world_event` is player-scoped and the silent market
            // is NPC-only); here it plays the vendor→buyer hand-over.
            // The Scold's summons peal, which until now existed only as a
            // drive-mode stand-in, gets its real trigger (`law_and_order.md`
            // M4a): an officer out of patience calls the accused to answer by
            // the next bell, and the Bellstand says so over the whole city.
            if kind == "summon" {
                presentation
                    .soundscape
                    .write(crate::soundscape::SoundscapeCue::CivicBell(
                        crate::soundscape::BellPattern::ScoldSummons,
                    ));
            }
            // Being taken in charge has a sound of its own (M4c): the keys the
            // gate and watch keepers carry. Hung on the person *taken*, not on
            // the officer whose belt they are on: `seize` is a four-metre verb,
            // so the two are at arm's reach and either would place the rattle
            // inside its own falloff — but the officer is the one a seizure
            // moves. She closes on foot in the verb path, and the drive
            // stand-in teleports her beside the target inside this very poll,
            // so at this instant hers is the one position neither channel here
            // has heard about yet. The target's, nobody has touched.
            // Both channels are read through the immutable deref: flagging
            // either here would claim a mover moved when only a message
            // arrived.
            if kind == "seize"
                && let Some(position) = live_position(
                    target.as_ref().unwrap_or(&actor),
                    &**mirror,
                    &**movement_inbox,
                )
            {
                presentation
                    .soundscape
                    .write(crate::soundscape::SoundscapeCue::CustodyKeys { position });
            }
            // …and being shut in has the other (M5c). The sim emits `commit`
            // only for the Stone House — a gate arch has no leaf — so this needs
            // no station test of its own, and the door is a fixed fitting of the
            // city rather than something to read off a person: everyone this
            // event names has just been *put* in the room. The escort's arrival
            // is what commits, and the drive stand-in sets keeper and prisoner
            // to the gaol point in the same poll, so every pose the host tracks
            // still has them out in the street they walked in from.
            if kind == "commit" {
                presentation
                    .soundscape
                    .write(crate::soundscape::SoundscapeCue::GaolDoor {
                        position: crate::soundscape::STONE_HOUSE_DOOR,
                    });
            }
            match (kind.as_str(), target, item) {
                ("accept_offered_item", Some(giver), Some(item)) => {
                    presentation.hands.write(hands::HandoverFeedback::Accepted {
                        giver,
                        recipient: actor,
                        item,
                    });
                }
                ("decline_offer", Some(giver), _) => {
                    presentation.hands.write(hands::HandoverFeedback::Declined {
                        decliner: actor,
                        giver,
                    });
                }
                ("sale", Some(buyer), Some(item)) => {
                    presentation
                        .hands
                        .write(hands::HandoverFeedback::StallSale {
                            vendor: actor,
                            buyer,
                            item,
                        });
                }
                // Custody's hand (`law_and_order.md` M4c). `grab`, `let_go` and
                // `release` are the officer's acts, so the actor is the holder;
                // a prisoner who tears free is the actor of their own escape.
                ("grab", Some(prisoner), _) => {
                    presentation.hands.write(hands::HandoverFeedback::TookHold {
                        holder: actor,
                        prisoner,
                    });
                }
                // A hand coming off an arm without the custody ending, which is
                // every clock-driven one: the dead-man timer, the station cap,
                // and arriving at the station. `release` is the verb and
                // `broke_free` the struggle — neither of them is what the sim
                // sends when an escort simply reaches the door, so without this
                // arm the officer's hand stayed drawn on the prisoner for the
                // rest of the run.
                ("let_go", _, _) => {
                    presentation
                        .hands
                        .write(hands::HandoverFeedback::HandOff { holder: actor });
                }
                ("release", Some(prisoner), _) => {
                    presentation
                        .hands
                        .write(hands::HandoverFeedback::HandsOff { prisoner });
                }
                ("broke_free", _, _) => {
                    presentation
                        .hands
                        .write(hands::HandoverFeedback::HandsOff { prisoner: actor });
                }
                _ => {}
            }
        }
        EngineMessage::LawStanding { notices, custody } => {
            // Purely a projection: the sim decides, and every host-side answer
            // (the tether, the reflex, the strain meter) goes back to it as a
            // command rather than being applied locally. This is the one arm
            // that writes the standing, so it is the one arm that may flag it —
            // `law_standing_hud` redraws the line on exactly that flag.
            custody::apply_law_standing(
                &mut **law,
                &notices,
                custody.map(|custody| custody::CustodyView {
                    holder_ids: custody
                        .holder_ids
                        .iter()
                        .map(model::actor_id_from_sim)
                        .collect(),
                    officer_id: custody.officer_id.as_ref().map(model::actor_id_from_sim),
                    officer_name: custody.officer_name,
                    station_name: custody.station_name,
                    anchor_m: model::vec3_from_sim(custody.anchor_m),
                    closing: custody.closing,
                    strain_seconds: custody.strain_seconds as f32,
                    held: custody.held,
                    committed: custody.committed,
                    fee_sparks: custody.fee_sparks,
                    release_office: custody.release_office,
                    booked_as: custody.booked_as,
                }),
            );
        }
        EngineMessage::ChalkStanding { pen, anchors } => {
            // Pure projection, like `LawStanding`: the hold this feeds sends a
            // command back rather than deciding anything locally. Assigned only
            // when it differs so the resource's change flag — which resets the
            // sign picker — means "what is within reach moved", not "a message
            // arrived".
            let next = crate::city::ChalkStanding {
                pen,
                anchors: anchors
                    .into_iter()
                    .map(|anchor| crate::city::ChalkableAnchor {
                        handle: anchor.handle,
                        label: anchor.label,
                        kinds: anchor.kinds,
                    })
                    .collect(),
            };
            if **chalk != next {
                **chalk = next;
            }
        }
        EngineMessage::Gesture {
            event_id: _,
            actor_id,
            kind,
            target_id,
            recipient_ids: _,
        } => {
            // The deliberate body (npc_bodies M4): a transient trigger the pose
            // pipeline plays — and nothing else (no toast). The looping dance
            // also rides the snapshot, so a late-arriving player still sees it;
            // this fires the immediate one-shots and the dance's first frame.
            let actor_id = model::actor_id_from_sim(&actor_id);
            if mirror.actor(&actor_id).is_some() {
                presentation.gestures.write(body::PresentGesture {
                    actor_id,
                    kind,
                    target_id: target_id.as_ref().map(model::actor_id_from_sim),
                });
            }
        }
        EngineMessage::TranscriptionResult {
            request_id: _,
            text,
            error,
        } => {
            if let Some(text) = text.filter(|text| valid_ui_text(text, 500)) {
                // This is the earliest exact confirmation of what STT
                // understood. It has a dedicated bottom caption so later
                // status/world-event toasts cannot overwrite it.
                hud.show_player_transcript(&text);
            } else if let Some(error) = error {
                hud.toast(truncate_owned(error, 300));
            }
        }
        EngineMessage::CommandResult {
            request_id,
            success,
            error_code,
            message,
        } => {
            if runtime
                .tts_selection_pending
                .as_ref()
                .is_some_and(|(pending, _)| pending == &request_id)
            {
                let (_, requested) = runtime
                    .tts_selection_pending
                    .take()
                    .expect("pending selection was checked");
                if success {
                    runtime.tts_selected = requested;
                    runtime.tts_selection_dirty = true;
                    hud.set_npc_voice_backend(requested);
                    hud.toast(format!("NPC voices: {}", requested.name().to_uppercase()));
                } else {
                    let code = truncate_owned(error_code.unwrap_or_else(|| "error".into()), 64);
                    hud.toast(format!("{code}: {}", truncate_owned(message, 260)));
                }
                return;
            }
            let known = interaction.resolve_command(&request_id, success, runtime.mirror_revision);
            if known && !success {
                let code = truncate_owned(error_code.unwrap_or_else(|| "error".into()), 64);
                hud.toast(format!("{code}: {}", truncate_owned(message, 260)));
            }
        }
        EngineMessage::Status(status) => apply_status(status, mirror, runtime, hud),
        EngineMessage::TtsReady {
            event_id,
            wav_bytes,
        } => {
            presentation.wav.write(speech::TtsClipReady {
                event_id: event_id.0,
                wav_bytes,
            });
        }
        EngineMessage::TtsChunk {
            event_id,
            chunk_seq,
            sample_rate,
            samples,
            backend,
        } => {
            presentation.pcm.write(speech::TtsPcmChunkReady {
                event_id: event_id.0,
                chunk_seq,
                sample_rate,
                samples,
                backend,
            });
        }
        EngineMessage::TtsStreamEnd {
            event_id,
            chunk_count,
            first_chunk_ms,
        } => {
            if chunk_count > 0 && first_chunk_ms <= 600_000 {
                presentation.stream_end.write(speech::TtsStreamFinished {
                    event_id: event_id.0,
                    chunk_count,
                    first_chunk_ms,
                });
            } else {
                hud.toast("Discarded invalid NPC stream completion data");
            }
        }
        EngineMessage::TtsFailed { event_id, reason } => {
            if valid_ui_text(&reason, 160) {
                presentation.failure.write(speech::TtsClipFailed {
                    event_id: event_id.0,
                    reason,
                });
            } else {
                hud.toast("Discarded invalid NPC voice failure data");
            }
        }
        // Both are the host's business and never reach the ECS: `local_engine`
        // writes the prompt archive and the session log itself.
        EngineMessage::PromptExchange { .. } | EngineMessage::Diagnostic(_) => {}
    }
}

fn tts_selection_is_usable(runtime: &SmartActorRuntime) -> bool {
    match runtime.tts_selected {
        bridge::TtsBackend::Cloud => runtime.tts_cloud_available,
        bridge::TtsBackend::Local => runtime.tts_local_available,
        bridge::TtsBackend::Off => false,
    }
}

fn connection_detail_for_capabilities(llm: bool, stt: bool, tts: bool) -> String {
    let mut unavailable = Vec::new();
    if !llm {
        unavailable.push("NPC cognition");
    }
    if !stt {
        unavailable.push("microphone transcription");
    }
    if !tts {
        unavailable.push("NPC voice audio");
    }
    if unavailable.is_empty() {
        "Local actor engine connected".into()
    } else {
        format!("Connected; unavailable: {}", unavailable.join(", "))
    }
}

/// The engine's capability set is consistent by construction
/// (`Capabilities::new` derives the `stt`/`tts` ORs from the four probes), so
/// there is nothing left to validate here — only to apply.
fn apply_ready_capabilities(
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
    capabilities: Capabilities,
) {
    runtime.ready = true;
    runtime.connected = true;
    runtime.stt_available = capabilities.stt;
    runtime.stt_cloud_available = capabilities.stt_cloud;
    runtime.stt_local_available = capabilities.stt_local;
    runtime.tts_available = capabilities.tts;
    runtime.tts_cloud_available = capabilities.tts_cloud;
    runtime.tts_local_available = capabilities.tts_local;
    runtime.tts_selected = tts_backend_of(capabilities.tts_selected);
    runtime.tts_selection_pending = None;
    hud.connection = hud::ConnectionUiState::Online;
    if capabilities.stt {
        // Preserve an explicit pre-handshake MIC OFF choice; otherwise the
        // worker's Available event reveals the default-on state.
        hud.microphone_unavailable = false;
    } else {
        hud.microphone_available = false;
        hud.microphone_unavailable = true;
    }
    hud.listening = false;
    hud.set_transcription_capabilities(capabilities.stt_cloud, capabilities.stt_local);
    hud.set_npc_voice_backend(runtime.tts_selected);
    hud.connection_detail =
        connection_detail_for_capabilities(capabilities.llm, capabilities.stt, capabilities.tts);
}

fn tts_backend_of(kind: TtsBackendKind) -> bridge::TtsBackend {
    match kind {
        TtsBackendKind::Cloud => bridge::TtsBackend::Cloud,
        TtsBackendKind::Local => bridge::TtsBackend::Local,
        TtsBackendKind::Off => bridge::TtsBackend::Off,
    }
}

/// Project one authoritative snapshot. It is replaced whole or not at all; a
/// snapshot the projection rejects is a sim bug, and the next revision — the
/// engine publishes one per change — is the only recovery there is.
fn accept_snapshot(
    mirror: &mut model::WorldMirror,
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
    movement_inbox: &mut model::MovementInbox,
    snapshot: &cathedral_sim::PublicSnapshot,
) -> bool {
    match mirror.replace_snapshot(snapshot.into()) {
        Ok(revision) => {
            runtime.mirror_revision = Some(revision);
            retire_superseded_movement(mirror, movement_inbox);
            true
        }
        Err(error) => {
            hud.toast(format!("malformed snapshot: {error}"));
            false
        }
    }
}

/// Drops the hot-channel samples the snapshot just accepted has overruled.
///
/// The two channels are allowed to disagree in exactly one direction: between
/// revisions the mirror's position for a walker is stale, and
/// `actors::drive_npc_bodies` winning over it is the whole point of the split.
/// But the sim also moves people by paths that emit no 20 Hz `Movement` tick at
/// all — a `commit` teleport into the Stone House, a road party re-entering at
/// its gate — and then the snapshot is the *newer* of the two. Nothing else ever
/// removes an entry here, so without this the sample from before the reposition
/// would drag the body back out into the street it was last walking down and
/// keep it there: reconcile only runs on a revision bump, and the interpolation
/// clamps at the pose it already reached.
///
/// The rule is that a walker's two channels agree exactly. A poll steps the
/// movers before it takes the snapshot, and both readings come from the same
/// `position_m()` through the same `as f32`, so anyone whose sample does *not*
/// match was put where they now stand by something other than a walk. Somebody
/// who has left the mirror altogether — a departed road party, whose actor view
/// reconcile has just despawned — has no authoritative position to agree with,
/// and goes too: that is what stops a stale pre-departure sample sliding them
/// off the gate they re-enter at.
fn retire_superseded_movement(
    mirror: &model::WorldMirror,
    movement_inbox: &mut model::MovementInbox,
) {
    movement_inbox.0.retain(|actor_id, sample| {
        mirror
            .actor(actor_id)
            .is_some_and(|actor| Vec3::from(actor.position_m) == sample.position)
    });
}

/// Where somebody stands *now*, for the message arms that have to put a real
/// sound in a real place.
///
/// `ActorSnapshot::position_m` alone will not do it. It is the cold channel:
/// `World::step_movement` writes a walker's position without ever calling
/// `touch_public_state`, so between revisions the mirror simply does not know
/// they have moved — and `Engine::flush` pushes a poll's world events *before*
/// the snapshot that poll owes, so an event arm reads a mirror that is a
/// revision behind the very thing it is reacting to. The hot channel is the
/// 20 Hz `Movement` tick, written a few arms above in this same drain, and
/// [`retire_superseded_movement`] means a sample that is still here is one the
/// last snapshot agreed with. So: the sample when there is one, the snapshot
/// otherwise, and `None` for somebody the projection has never heard of.
///
/// Neither channel can answer for a *teleport* — the sim moving someone by a
/// path that ships no tick, whose snapshot is still behind us in the queue. A
/// caller whose event implies one (`commit`) must not ask this question at all.
fn live_position(
    actor_id: &model::ActorId,
    mirror: &model::WorldMirror,
    movement_inbox: &model::MovementInbox,
) -> Option<Vec3> {
    movement_inbox
        .0
        .get(actor_id)
        .map(|sample| sample.position)
        .or_else(|| mirror.actor(actor_id).map(|actor| actor.position_m.into()))
}

fn next_tts_backend(runtime: &SmartActorRuntime) -> bridge::TtsBackend {
    use bridge::TtsBackend::{Cloud, Local, Off};
    let modes = [Cloud, Local, Off];
    let current = modes
        .iter()
        .position(|mode| *mode == runtime.tts_selected)
        .unwrap_or(2);
    for offset in 1..=modes.len() {
        let candidate = modes[(current + offset) % modes.len()];
        let available = match candidate {
            Cloud => runtime.tts_cloud_available,
            Local => runtime.tts_local_available,
            Off => true,
        };
        if available {
            return candidate;
        }
    }
    Off
}

fn update_tts_backend_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    handle: Res<bridge::BridgeHandle>,
    mut runtime: ResMut<SmartActorRuntime>,
    mut hud: ResMut<hud::SmartActorHudState>,
) {
    if !keyboard.just_pressed(KeyCode::KeyX) {
        return;
    }
    let backend = next_tts_backend(&runtime);
    request_tts_backend(&mut runtime, &handle, &mut hud, backend);
}

/// Validated NPC-voice switch shared by the X key and the settings menu. The
/// selection only commits (and persists) once the engine confirms it.
fn request_tts_backend(
    runtime: &mut SmartActorRuntime,
    handle: &bridge::BridgeHandle,
    hud: &mut hud::SmartActorHudState,
    backend: bridge::TtsBackend,
) {
    if !runtime.interactions_enabled() {
        hud.toast("NPC voice selection is unavailable while actors are offline");
        return;
    }
    if runtime.tts_selection_pending.is_some() {
        hud.toast("NPC voice selection is still changing");
        return;
    }
    let available = match backend {
        bridge::TtsBackend::Cloud => runtime.tts_cloud_available,
        bridge::TtsBackend::Local => runtime.tts_local_available,
        bridge::TtsBackend::Off => true,
    };
    if !available {
        hud.toast(format!(
            "{} NPC voices are not available",
            backend.name().to_uppercase()
        ));
        return;
    }
    if runtime.tts_selected == backend {
        return;
    }
    runtime.next_tts_request = runtime.next_tts_request.wrapping_add(1).max(1);
    let request_id = format!("tts-mode-{}", runtime.next_tts_request);
    match handle.try_send(bridge::BridgeCommand::SetTtsBackend {
        request_id: request_id.clone(),
        backend,
    }) {
        Ok(()) => {
            runtime.tts_selection_pending = Some((request_id, backend));
            hud.toast(format!(
                "Switching NPC voices to {}…",
                backend.name().to_uppercase()
            ));
        }
        Err(error) => hud.toast(error),
    }
}

/// The engine's status rows drive the connection line and the STT pills.
///
/// `state` stays a free-form string: the speech backends add rows of their own
/// (`synthesizing`, `loading`, `selected`, …) and the HUD matches on them.
fn apply_status(
    status: StatusEvent,
    mirror: &model::WorldMirror,
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
) {
    let subsystem = status.subsystem.as_str();
    let state = truncate_owned(status.state, 64);
    let message = status.message.map(|message| truncate_owned(message, 300));
    let backend = status.backend.map(|backend| truncate_owned(backend, 16));
    let actor_id = status.actor_id.as_ref().map(model::actor_id_from_sim);
    if subsystem == "llm" {
        runtime.observe_llm_status(&state, actor_id.as_ref());
    }
    let actor = actor_id.as_ref().and_then(|id| mirror.actor(id));
    let actor_name = actor.map(|actor| actor.name_for_player.clone());
    let nearby_thinking_actor = actor
        .filter(|actor| actor_is_near_player(actor, mirror))
        .map(|actor| actor.name_for_player.clone());
    hud.connection_detail = match (subsystem, state.as_str(), actor_name.as_deref()) {
        ("llm", "thinking", _) => nearby_thinking_actor
            .map(|actor| format!("{actor} is thinking…"))
            .unwrap_or_else(|| "Background actors are thinking…".into()),
        ("stt", "transcribing", _) => "Transcribing your speech…".into(),
        ("tts", "synthesizing", Some(actor)) => format!("Preparing {actor}'s voice…"),
        (_, state, _) => message
            .clone()
            .unwrap_or_else(|| format!("{subsystem}: {state}")),
    };
    hud.connection_detail = truncate_owned(std::mem::take(&mut hud.connection_detail), 300);
    if subsystem == "stt"
        && let Some(backend) = backend.as_deref()
    {
        hud.apply_transcription_status(backend, &state, message.as_deref());
    }
    if matches!(state.as_str(), "degraded" | "unavailable")
        && let Some(message) = message
    {
        hud.toast(truncate_owned(message, 300));
    }
}

fn actor_is_near_player(actor: &model::ActorSnapshot, mirror: &model::WorldMirror) -> bool {
    let Some(player) = mirror
        .player_id()
        .and_then(|player_id| mirror.actor(player_id))
    else {
        return false;
    };
    let actor_position: Vec3 = actor.position_m.into();
    let player_position: Vec3 = player.position_m.into();
    actor_position.distance_squared(player_position) <= HEARING_RADIUS_M * HEARING_RADIUS_M
}

/// An offer the player was part of that ended with nothing changing hands, as
/// a headline and the **reason** for it — the two ways it can happen read
/// identically in the world (the item simply stays put), so the reason is the
/// whole point of saying anything.
///
/// `None` for everything else, including a refusal between two other people:
/// that is news, not feedback, and the toast already carries it.
fn describe_offer_rejection(
    kind: &str,
    actor_id: &model::ActorId,
    target_id: Option<&model::ActorId>,
    item_id: Option<&model::ItemId>,
    quantity: u32,
    mirror: &model::WorldMirror,
) -> Option<(String, String)> {
    if !matches!(kind, "decline_offer" | "lapse_offer") {
        return None;
    }
    let player_id = mirror.player_id()?;
    let target_id = target_id?;
    // Both events pair (actor, target): a decline runs decliner → giver, a
    // lapse giver → the one it was held out to. So the other party is whichever
    // of the two the player is not, and if he is neither, this is not his.
    let player_acted = actor_id == player_id;
    let other_id = if player_acted {
        target_id
    } else if target_id == player_id {
        actor_id
    } else {
        return None;
    };
    let other = mirror.actor(other_id)?.name_for_player.clone();
    let snapshot = mirror.item(item_id?)?;
    let count = quantity.max(1);
    let item = if count > 1 {
        format!("{count} {}", snapshot.display_plural)
    } else {
        snapshot.name.clone()
    };
    let apart = cathedral_sim::OFFER_LAPSE_RADIUS_M;
    match kind {
        "decline_offer" if player_acted => Some((
            "OFFER DECLINED".into(),
            format!("You refused the {item} {other} held out"),
        )),
        "decline_offer" => Some((
            "OFFER DECLINED".into(),
            format!("{other} refused the {item} you held out"),
        )),
        // Nobody refused anything here, so both sides get the same cause and
        // differ only in where the item ended up.
        "lapse_offer" if player_acted => Some((
            "OFFER LAPSED".into(),
            format!("You and {other} drifted more than {apart:.0} m apart — you keep the {item}"),
        )),
        "lapse_offer" => Some((
            "OFFER LAPSED".into(),
            format!(
                "You and {other} drifted more than {apart:.0} m apart — the {item} stays with them"
            ),
        )),
        _ => None,
    }
}

/// The player-facing sentence one world event deserves, or `None` when it is
/// none of his business.
fn describe_world_event(
    kind: &str,
    actor_id: &model::ActorId,
    target_id: Option<&model::ActorId>,
    item_id: Option<&model::ItemId>,
    quantity: u32,
    recipient_ids: &[model::ActorId],
    mirror: &model::WorldMirror,
) -> Option<String> {
    let player_id = mirror.player_id()?;
    if actor_id != player_id && !recipient_ids.contains(player_id) {
        return None;
    }
    let player_acted = actor_id == player_id;
    let actor = if player_acted {
        "You"
    } else {
        mirror.actor(actor_id)?.name_for_player.as_str()
    };
    // Two kinds name no item at all (`features/extra_pockets.md`): `expel`
    // empties whatever is down there, and `digest` is the gut announcing
    // itself. They are answered before the item lookup so they still toast.
    match kind {
        "expel" => {
            return Some(if player_acted {
                "You relieve yourself".into()
            } else {
                format!("{actor} relieves themself")
            });
        }
        // Only ever your own gut's business — the sim sends it with no
        // recipients, so a bystander never reaches this function for it.
        "digest" if player_acted => {
            return Some("Something has made its way through you".into());
        }
        "digest" => return None,
        _ => {}
    }
    let snapshot = item_id.and_then(|item_id| mirror.item(item_id))?;
    // Counted for a transient toast: "3 sparks", "3 loaves" — the plural is
    // catalog-derived host-side, so irregulars read correctly.
    let count = quantity.max(1);
    let item = if count > 1 {
        format!("{count} {}", snapshot.display_plural)
    } else {
        snapshot.name.clone()
    };
    let offer_verb = if player_acted { "offer" } else { "offers" };
    match kind {
        "offer_item" if target_id == Some(player_id) => {
            Some(format!("{actor} {offer_verb} you the {item}"))
        }
        "offer_item" if target_id.is_none() => {
            Some(format!("{actor} {offer_verb} the {item} openly"))
        }
        // A targeted offer between other actors is not feedback for the
        // player. In particular, do not let it overwrite the preceding
        // retract event when an offer is redirected away from the player.
        "offer_item" => None,
        "accept_offered_item" => Some(if player_acted {
            format!("You accept the {item}")
        } else {
            format!("{actor} accepts the {item}")
        }),
        "decline_offer" => Some(if player_acted {
            format!("You decline the {item}")
        } else {
            format!("{actor} declines the {item}")
        }),
        "retract_offer" => Some(if player_acted {
            format!("You withdraw the {item} offer")
        } else {
            format!("{actor} withdraws the {item} offer")
        }),
        "eat" => Some(if player_acted {
            format!("You eat the {item}")
        } else {
            format!("{actor} eats the {item}")
        }),
        // The body pockets (`features/extra_pockets.md`). Others see only the
        // transition, never the contents — "something", the same discretion the
        // sim's percepts keep.
        "pocket_item" => Some(if player_acted {
            format!("You tuck the {item} away")
        } else {
            format!("{actor} tucks something away")
        }),
        "retrieve_item" => Some(if player_acted {
            format!("You take the {item} back out")
        } else {
            format!("{actor} takes something out")
        }),
        "swallow" => Some(if player_acted {
            format!("You swallow the {item}")
        } else {
            format!("{actor} swallows the {item}")
        }),
        "spit" if target_id == Some(player_id) => Some(format!("{actor} spits the {item} at you!")),
        "spit" if player_acted => {
            let target = target_id
                .and_then(|target_id| mirror.actor(target_id))
                .map_or_else(
                    || "someone".to_string(),
                    |target| target.name_for_player.clone(),
                );
            Some(format!("You spit the {item} at {target}"))
        }
        "spit" => Some(format!("{actor} spits at someone")),
        "gargle" => Some(if player_acted {
            format!("You gargle the {item}")
        } else {
            format!("{actor} gargles")
        }),
        _ => None,
    }
}

/// LLM-authored text is the only untrusted input left. Bound it before it
/// reaches a UI node.
fn valid_ui_text(value: &str, max_chars: usize) -> bool {
    !value.is_empty()
        && value.chars().count() <= max_chars
        && value
            .chars()
            .all(|character| !character.is_control() || matches!(character, '\n' | '\t'))
}

fn truncate_owned(mut value: String, maximum_chars: usize) -> String {
    if let Some((byte, _)) = value.char_indices().nth(maximum_chars) {
        value.truncate(byte);
    }
    value
}

fn forward_player_intents(
    handle: Res<bridge::BridgeHandle>,
    microphone: Option<Res<microphone::MicrophoneService>>,
    runtime: Res<SmartActorRuntime>,
    mut intents: MessageReader<interaction::PlayerIntent>,
    mut interaction: ResMut<interaction::InteractionState>,
    mut spatial: ResMut<interaction::PlayerSpatialState>,
    mut hud: ResMut<hud::SmartActorHudState>,
) {
    for intent in intents.read() {
        let request_id = intent_request_id(intent).map(str::to_owned);
        let is_spatial = matches!(intent, interaction::PlayerIntent::SpatialUpdate { .. });
        let failed_recording = match intent {
            interaction::PlayerIntent::Recording { wav_basename, .. } => Some(wav_basename.clone()),
            _ => None,
        };
        if !runtime.interactions_enabled() {
            if is_spatial {
                spatial.retry_latest_position();
            }
            if let Some(request_id) = request_id {
                interaction.resolve_command(&request_id, false, None);
            }
            if let (Some(wav_basename), Some(microphone)) = (failed_recording, &microphone) {
                // Best-effort: release any streamed copy the engine holds.
                let _ = handle.try_send(bridge::BridgeCommand::PlayerAudioAbort {
                    wav_basename: wav_basename.clone(),
                });
                if let Err(error) = microphone.discard_recording(wav_basename) {
                    hud.toast(error);
                }
            }
            continue;
        }
        let delivery = intent_to_command(intent).and_then(|command| handle.try_send(command));
        if let Err(error) = delivery {
            if is_spatial {
                spatial.retry_latest_position();
            }
            if let Some(request_id) = request_id {
                interaction.resolve_command(&request_id, false, None);
            }
            if let (Some(wav_basename), Some(microphone)) = (failed_recording, &microphone) {
                let _ = handle.try_send(bridge::BridgeCommand::PlayerAudioAbort {
                    wav_basename: wav_basename.clone(),
                });
                if let Err(cleanup_error) = microphone.discard_recording(wav_basename) {
                    hud.toast(cleanup_error);
                }
            }
            if !error.contains("spatial update coalesced") {
                hud.toast(error);
            }
        }
    }
}

fn intent_to_command(intent: &interaction::PlayerIntent) -> Result<bridge::BridgeCommand, String> {
    let position = |value| {
        model::Position::try_from(value).map_err(|_| "player position is invalid".to_string())
    };
    Ok(match intent {
        interaction::PlayerIntent::SpatialUpdate {
            spatial_seq,
            position: value,
            facing_yaw,
        } => bridge::BridgeCommand::SpatialUpdate {
            spatial_seq: *spatial_seq,
            position_m: position(*value)?,
            facing_yaw: if facing_yaw.is_finite() {
                *facing_yaw
            } else {
                return Err("player facing is invalid".into());
            },
        },
        interaction::PlayerIntent::Recording {
            request_id,
            wav_basename,
            stt_backend,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerRecording {
            request_id: request_id.clone(),
            wav_basename: wav_basename.clone(),
            stt_backend: *stt_backend,
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Offer {
            request_id,
            target_id,
            item_id,
            quantity,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerOffer {
            request_id: request_id.clone(),
            target_id: target_id.clone(),
            item_id: item_id.clone(),
            quantity: *quantity,
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Accept {
            request_id,
            item_id,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerAccept {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Decline {
            request_id,
            item_id,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerDecline {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Retract {
            request_id,
            item_id,
        } => bridge::BridgeCommand::PlayerRetract {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
        },
        interaction::PlayerIntent::Pocket {
            request_id,
            item_id,
            slot,
        } => bridge::BridgeCommand::PlayerPocket {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
            slot: *slot,
        },
        interaction::PlayerIntent::Retrieve {
            request_id,
            item_id,
        } => bridge::BridgeCommand::PlayerRetrieve {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
        },
        interaction::PlayerIntent::Swallow {
            request_id,
            item_id,
        } => bridge::BridgeCommand::PlayerSwallow {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
        },
        interaction::PlayerIntent::Spit {
            request_id,
            item_id,
            target_id,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerSpit {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
            target_id: target_id.clone(),
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Gargle {
            request_id,
            item_id,
        } => bridge::BridgeCommand::PlayerGargle {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
        },
        interaction::PlayerIntent::Expel { request_id } => bridge::BridgeCommand::PlayerExpel {
            request_id: request_id.clone(),
        },
        interaction::PlayerIntent::Eat {
            request_id,
            item_id,
        } => bridge::BridgeCommand::PlayerEat {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
        },
        interaction::PlayerIntent::Sound { sound_id } => bridge::BridgeCommand::PlayerSound {
            sound_id: sound_id.clone(),
        },
        interaction::PlayerIntent::DebugSay {
            request_id,
            text,
            target_id,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::DebugPlayerSay {
            request_id: request_id.clone(),
            text: text.clone(),
            target_id: target_id.clone(),
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Say {
            request_id,
            text,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerSay {
            request_id: request_id.clone(),
            text: text.clone(),
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
    })
}

fn intent_request_id(intent: &interaction::PlayerIntent) -> Option<&str> {
    match intent {
        interaction::PlayerIntent::SpatialUpdate { .. }
        | interaction::PlayerIntent::Sound { .. } => None,
        interaction::PlayerIntent::Recording { request_id, .. }
        | interaction::PlayerIntent::Offer { request_id, .. }
        | interaction::PlayerIntent::Accept { request_id, .. }
        | interaction::PlayerIntent::Decline { request_id, .. }
        | interaction::PlayerIntent::Retract { request_id, .. }
        | interaction::PlayerIntent::Pocket { request_id, .. }
        | interaction::PlayerIntent::Retrieve { request_id, .. }
        | interaction::PlayerIntent::Swallow { request_id, .. }
        | interaction::PlayerIntent::Spit { request_id, .. }
        | interaction::PlayerIntent::Gargle { request_id, .. }
        | interaction::PlayerIntent::Expel { request_id }
        | interaction::PlayerIntent::Eat { request_id, .. }
        | interaction::PlayerIntent::DebugSay { request_id, .. }
        | interaction::PlayerIntent::Say { request_id, .. } => Some(request_id),
    }
}

fn collect_injected_transcripts(
    mut injected: MessageReader<InjectPlayerTranscript>,
    runtime: Res<SmartActorRuntime>,
    players: Query<&GlobalTransform, With<crate::controller::PlayerController>>,
    mut spatial: ResMut<interaction::PlayerSpatialState>,
    mut interaction: ResMut<interaction::InteractionState>,
    mut intents: MessageWriter<interaction::PlayerIntent>,
) {
    let Ok(player) = players.single() else { return };
    for injection in injected.read() {
        if let Some(intent) = interaction::inject_debug_say(
            injection.text.clone(),
            injection.target_id.clone(),
            player.translation(),
            &runtime,
            &mut spatial,
            &mut interaction,
        ) {
            intents.write(intent);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use bevy::{
        asset::{AssetApp, AssetPlugin},
        audio::AudioSource,
        input::mouse::AccumulatedMouseScroll,
        transform::TransformPlugin,
        window::{CursorGrabMode, CursorOptions, PrimaryWindow},
    };

    use super::*;

    /// A player-editable `config.ron` reaches the crowd generator directly, so
    /// a fat-fingered zero must be reported and cut rather than obeyed —
    /// 200,000 sheets is a loading screen that never ends.
    #[test]
    fn a_crowd_above_the_ceiling_is_reported_and_cut() {
        let with = |count: u32| SmartActorsConfig {
            extra_ambient_npcs: count,
            ..SmartActorsConfig::default()
        };
        assert_eq!(with(0).extra_ambient_npcs(), (0, None));
        assert_eq!(with(2_000).extra_ambient_npcs(), (2_000, None));
        // The ceiling itself is allowed through, silently.
        let ceiling = cathedral_sim::MAX_EXTRA_AMBIENT_NPCS;
        assert_eq!(with(ceiling).extra_ambient_npcs(), (ceiling, None));

        let (count, complaint) = with(200_000).extra_ambient_npcs();
        assert_eq!(count, ceiling);
        let complaint = complaint.expect("a cut crowd is worth a line");
        assert!(complaint.contains("200000"), "{complaint}");
        assert!(complaint.contains(&ceiling.to_string()), "{complaint}");
    }

    /// The projection's sanity ceilings are what a snapshot of a *full* crowd
    /// has to pass. They were sized for the authored cast, and a fixed 1,024
    /// rejected every snapshot at `extra_ambient_npcs: 2000` with "snapshot
    /// contains too many actors" — a city that rendered its buildings and none
    /// of its people. Tied to the sim's own ceiling here so they cannot drift
    /// apart again.
    #[test]
    fn the_projection_admits_a_full_crowd() {
        let crowd = cathedral_sim::MAX_EXTRA_AMBIENT_NPCS as usize;
        assert!(
            model::max_actors() > crowd,
            "a full crowd plus the authored cast must fit"
        );
        // One purse a head is seeded by the round, so the item ceiling has to
        // clear the crowd too, not merely the cast.
        assert!(model::max_items() > crowd);
    }

    #[test]
    fn audio_activity_tracks_live_voice_and_open_chat() {
        let mut app = App::new();
        app.init_resource::<AudioActivity>()
            .init_resource::<interaction::MicrophoneInputState>()
            .init_resource::<chat::ChatInputState>()
            .add_systems(Update, update_audio_activity);

        app.update();
        assert!(!app.world().resource::<AudioActivity>().busy);

        let voice = app.world_mut().spawn(speech::NpcVoice).id();
        app.update();
        assert!(app.world().resource::<AudioActivity>().busy);

        app.world_mut().despawn(voice);
        app.update();
        assert!(!app.world().resource::<AudioActivity>().busy);

        app.world_mut().resource_mut::<chat::ChatInputState>().open = true;
        app.update();
        assert!(app.world().resource::<AudioActivity>().busy);
    }

    #[test]
    fn routine_world_sounds_route_without_creating_npc_reflex_stimuli() {
        use crate::soundscape::{SoundscapeCue, SpecialWell};

        let market = Vec3::new(-14.9625, 0.91, 249.6375);
        assert_eq!(
            soundscape_route("market_cry", market),
            SoundscapeRoute {
                cue: Some(SoundscapeCue::MarketCry { position: market }),
                replace_standard: true,
            }
        );

        let ford = Vec3::new(89.375, 0.91, 36.125);
        assert_eq!(
            soundscape_route("draw_water", ford),
            SoundscapeRoute {
                cue: Some(SoundscapeCue::WellDraw {
                    source: SpecialWell::Ford,
                }),
                replace_standard: true,
            }
        );

        // The Tallage balance supplements the authoritative coin sound at its
        // exact authored beam; an ordinary market sale keeps coins alone.
        let tallage = soundscape_route("coin_clink", Vec3::new(-213.4125, 0.91, 63.0875));
        assert_eq!(
            tallage.cue,
            Some(SoundscapeCue::MarketMeasurement {
                position: Vec3::new(-214.2, 1.5, 45.5),
            })
        );
        assert!(!tallage.replace_standard);
        assert_eq!(soundscape_route("coin_clink", market).cue, None);

        assert_eq!(
            soundscape_route("fart", Vec3::ZERO),
            SoundscapeRoute {
                cue: None,
                replace_standard: false,
            }
        );
    }

    #[test]
    fn settled_spatial_constants_match_the_protocol_contract() {
        assert_eq!(HEARING_RADIUS_M, 20.0);
        assert_eq!(targeting::ACTOR_FOCUS_RADIUS_M, 20.0);
        assert_eq!(ITEM_INTERACTION_RADIUS_M, targeting::ITEM_FOCUS_RADIUS_M);
        assert_eq!(PLAYER_SPEECH_MAX_SECONDS, 15);
        assert_eq!(PLAYER_SPEECH_MAX_CHARS, 500);
        assert_eq!(POSITION_UPDATE_HZ, 10.0);
    }

    #[test]
    fn ready_hud_names_each_independently_missing_capability() {
        assert_eq!(
            connection_detail_for_capabilities(true, true, true),
            "Local actor engine connected"
        );
        assert_eq!(
            connection_detail_for_capabilities(true, false, false),
            "Connected; unavailable: microphone transcription, NPC voice audio"
        );
        assert!(connection_detail_for_capabilities(false, true, true).contains("NPC cognition"));
    }

    #[test]
    fn llm_status_tracks_the_actor_for_the_overhead_thinking_indicator() {
        let mirror = model::WorldMirror::default();
        let mut runtime = SmartActorRuntime::starting(false);
        let mut hud = hud::SmartActorHudState::default();
        let ilse = cathedral_sim::ActorId::from_raw("ilse");
        let sven = cathedral_sim::ActorId::from_raw("sven");

        apply_status(
            StatusEvent::llm("thinking", Some(ilse.clone()), None),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert_eq!(
            runtime.thinking_actor().map(|id| id.0.as_str()),
            Some("ilse")
        );

        // A late terminal row for a different actor cannot hide Ilse's newer
        // turn, but Ilse's own terminal row does.
        apply_status(
            StatusEvent::llm("idle", Some(sven), None),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert_eq!(
            runtime.thinking_actor().map(|id| id.0.as_str()),
            Some("ilse")
        );
        apply_status(
            StatusEvent::llm("idle", Some(ilse.clone()), None),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert!(runtime.thinking_actor().is_none());

        apply_status(
            StatusEvent::llm("thinking", Some(ilse), None),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        apply_status(
            StatusEvent::llm("unavailable", None, Some("offline".into())),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert!(runtime.thinking_actor().is_none());
    }

    #[test]
    fn thinking_hud_names_only_actors_within_conversation_range() {
        let mut mirror = model::WorldMirror::default();
        mirror
            .replace_snapshot(model::WorldSnapshot {
                world_revision: 1,
                player_id: model::ActorId("player".into()),
                actors: vec![
                    model::ActorSnapshot {
                        id: model::ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: model::ActorControl::Player,
                        position_m: model::Position::new(0.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                    model::ActorSnapshot {
                        id: model::ActorId("near".into()),
                        name_for_player: "Near".into(),
                        control: model::ActorControl::Llm,
                        position_m: model::Position::new(HEARING_RADIUS_M, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                    model::ActorSnapshot {
                        id: model::ActorId("far".into()),
                        name_for_player: "Far".into(),
                        control: model::ActorControl::Llm,
                        position_m: model::Position::new(HEARING_RADIUS_M + 0.01, 0.0, 0.0)
                            .unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                ],
                items: vec![],
                offers: vec![],
                road_carts: vec![],
                marks: Vec::new(),
            })
            .unwrap();
        let mut runtime = SmartActorRuntime::starting(false);
        let mut hud = hud::SmartActorHudState::default();

        apply_status(
            StatusEvent::llm(
                "thinking",
                Some(cathedral_sim::ActorId::from_raw("far")),
                None,
            ),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert_eq!(hud.connection_detail, "Background actors are thinking…");

        apply_status(
            StatusEvent::llm(
                "thinking",
                Some(cathedral_sim::ActorId::from_raw("near")),
                None,
            ),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert_eq!(hud.connection_detail, "Near is thinking…");
    }

    /// The cloud and local ears are independent: the streaming gate keys off
    /// the cloud flag alone, and the pill row off both.
    #[test]
    fn ready_capabilities_split_stt_availability_for_the_streaming_gate() {
        let mut runtime = SmartActorRuntime::starting(false);
        let mut hud = hud::SmartActorHudState::default();
        apply_ready_capabilities(
            &mut runtime,
            &mut hud,
            Capabilities::new(true, true, false, false, false, TtsBackendKind::Off),
        );

        assert!(runtime.ready && runtime.connected);
        assert!(runtime.stt_available && runtime.stt_cloud_available);
        assert!(!runtime.stt_local_available);
        assert!(!runtime.tts_available);
        assert_eq!(runtime.tts_selected, bridge::TtsBackend::Off);
        assert_eq!(hud.connection, hud::ConnectionUiState::Online);
        assert!(runtime.interactions_enabled());
    }

    #[test]
    fn npc_voice_cycle_skips_unavailable_backends_and_includes_off() {
        let mut runtime = SmartActorRuntime::starting(false);
        runtime.tts_selected = bridge::TtsBackend::Cloud;
        runtime.tts_cloud_available = true;
        runtime.tts_local_available = false;
        assert_eq!(next_tts_backend(&runtime), bridge::TtsBackend::Off);

        runtime.tts_selected = bridge::TtsBackend::Off;
        assert_eq!(next_tts_backend(&runtime), bridge::TtsBackend::Cloud);

        runtime.tts_local_available = true;
        runtime.tts_selected = bridge::TtsBackend::Cloud;
        assert_eq!(next_tts_backend(&runtime), bridge::TtsBackend::Local);
    }

    #[test]
    fn npc_speech_only_expects_audio_from_the_acknowledged_usable_mode() {
        let mut runtime = SmartActorRuntime::starting(false);
        runtime.tts_cloud_available = true;
        runtime.tts_local_available = false;
        runtime.tts_selected = bridge::TtsBackend::Off;
        assert!(!tts_selection_is_usable(&runtime));
        runtime.tts_selected = bridge::TtsBackend::Local;
        assert!(!tts_selection_is_usable(&runtime));
        runtime.tts_selected = bridge::TtsBackend::Cloud;
        assert!(tts_selection_is_usable(&runtime));
    }

    /// The player, Ilse (holding a copper coin) and Frans, close together.
    fn offer_feedback_mirror() -> (
        model::WorldMirror,
        model::ActorId,
        model::ActorId,
        model::ActorId,
        model::ItemId,
    ) {
        let player = model::ActorId("player".into());
        let giver = model::ActorId("giver".into());
        let other = model::ActorId("other".into());
        let coin = model::ItemId("coin".into());
        let mut mirror = model::WorldMirror::default();
        mirror
            .replace_snapshot(model::WorldSnapshot {
                world_revision: 1,
                player_id: player.clone(),
                actors: vec![
                    model::ActorSnapshot {
                        id: player.clone(),
                        name_for_player: "You".into(),
                        control: model::ActorControl::Player,
                        position_m: model::Position::new(0.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                    model::ActorSnapshot {
                        id: giver.clone(),
                        name_for_player: "Ilse".into(),
                        control: model::ActorControl::Llm,
                        position_m: model::Position::new(1.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![coin.clone()],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                    model::ActorSnapshot {
                        id: other.clone(),
                        name_for_player: "Frans".into(),
                        control: model::ActorControl::Llm,
                        position_m: model::Position::new(2.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                ],
                items: vec![model::ItemSnapshot {
                    id: coin.clone(),
                    kind: "spark".into(),
                    name: "copper coin".into(),
                    display_plural: "copper coins".into(),
                    visual_key: "coin".into(),
                    quantity: 1,
                    metadata: Default::default(),
                }],
                offers: vec![],
                road_carts: vec![],
                marks: Vec::new(),
            })
            .unwrap();
        (mirror, player, giver, other, coin)
    }

    #[test]
    fn redirected_offer_keeps_player_withdrawal_feedback_visible() {
        let (mirror, player, giver, other, coin) = offer_feedback_mirror();

        assert_eq!(
            describe_world_event(
                "retract_offer",
                &giver,
                Some(&player),
                Some(&coin),
                1,
                std::slice::from_ref(&player),
                &mirror,
            )
            .as_deref(),
            Some("Ilse withdraws the copper coin offer")
        );

        // The engine broadcasts observational world events to nearby actors,
        // including the player, even when the offer targets someone else — and
        // that must not overwrite the withdrawal toast above.
        assert_eq!(
            describe_world_event(
                "offer_item",
                &giver,
                Some(&other),
                Some(&coin),
                1,
                &[player, other.clone()],
                &mirror,
            ),
            None
        );
    }

    #[test]
    fn a_refused_offer_names_who_refused_and_which_way_it_went() {
        let (mirror, player, giver, other, coin) = offer_feedback_mirror();

        let refusal = |actor, target, quantity| {
            describe_offer_rejection(
                "decline_offer",
                actor,
                Some(target),
                Some(&coin),
                quantity,
                &mirror,
            )
        };

        // Ilse turning down what the player held out…
        assert_eq!(
            refusal(&giver, &player, 1),
            Some((
                "OFFER DECLINED".into(),
                "Ilse refused the copper coin you held out".into()
            ))
        );
        // …and the player turning down hers. Quantities are counted like the
        // toast's, so a partial purse reads honestly.
        assert_eq!(
            refusal(&player, &giver, 3),
            Some((
                "OFFER DECLINED".into(),
                "You refused the 3 copper coins Ilse held out".into()
            ))
        );

        // Two other people refusing each other is news, not feedback: it stays
        // the plain toast it has always been.
        assert_eq!(refusal(&giver, &other, 1), None);
        assert_eq!(
            describe_world_event(
                "decline_offer",
                &giver,
                Some(&other),
                Some(&coin),
                1,
                std::slice::from_ref(&player),
                &mirror,
            )
            .as_deref(),
            Some("Ilse declines the copper coin")
        );
    }

    #[test]
    fn a_lapsed_offer_blames_the_distance_and_says_where_the_item_went() {
        let (mirror, player, giver, _, coin) = offer_feedback_mirror();

        let lapse = |giver, target| {
            describe_offer_rejection("lapse_offer", giver, Some(target), Some(&coin), 1, &mirror)
        };

        // The player walked away from his own offer: he keeps the coin.
        assert_eq!(
            lapse(&player, &giver),
            Some((
                "OFFER LAPSED".into(),
                "You and Ilse drifted more than 20 m apart — you keep the copper coin".into()
            ))
        );
        // The same drift seen from the other end of it.
        assert_eq!(
            lapse(&giver, &player),
            Some((
                "OFFER LAPSED".into(),
                "You and Ilse drifted more than 20 m apart — the copper coin stays with them"
                    .into()
            ))
        );
        // A lapse is nobody's action, so it must never read as a refusal.
        assert_eq!(
            describe_world_event(
                "lapse_offer",
                &giver,
                Some(&player),
                Some(&coin),
                1,
                std::slice::from_ref(&player),
                &mirror,
            ),
            None
        );
    }

    /// The whole plugin on the in-process engine and the fake backends — no
    /// subprocess, no network, no `uv` — pumped until the cast is online.
    fn ready_fake_plugin_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .init_asset::<AudioSource>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseScroll>()
            .init_resource::<crate::controller::CollisionWorld>();
        app.world_mut().spawn((
            crate::controller::PlayerController::default(),
            Transform::from_xyz(0.0, 0.91, 111.0),
            GlobalTransform::from_translation(Vec3::new(0.0, 0.91, 111.0)),
        ));
        let camera_transform = Transform::from_xyz(0.0, 1.56, 111.0)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
        app.world_mut().spawn((
            crate::controller::PlayerCamera,
            camera_transform,
            GlobalTransform::from(camera_transform),
        ));
        app.world_mut().spawn((
            PrimaryWindow,
            CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                ..default()
            },
        ));
        app.add_plugins(SmartActorsPlugin::new(SmartActorsConfig {
            enabled: true,
            fake_backend: true,
            uv_binary: "uv".into(),
            tts_backend: "local".into(),
            pause_microphone_during_npc_voice: true,
            ..SmartActorsConfig::default()
        }));
        // Fake mode reports transcription available, so the plugin spawns the
        // capture worker — but a test may not open the developer's microphone
        // and put the room's noise into the scripted conversation. The worker
        // probes no device until it is enabled, so an explicit OFF (the V state)
        // keeps it inert while everything downstream of it stays wired.
        app.world_mut()
            .resource_mut::<interaction::MicrophoneInputState>()
            .enabled = false;

        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        while std::time::Instant::now() < deadline
            && !app.world().resource::<SmartActorRuntime>().ready
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(app.world().resource::<SmartActorRuntime>().ready);
        app.update();
        app
    }

    /// The hot channels reach [`process_engine_message`] as their `ResMut`
    /// wrapper and never bare, so an ordinary poll — a `Clock` and a `Weather`,
    /// which arrive on every single one — leaves the change ticks of the
    /// channels it says nothing about alone.
    ///
    /// Two gates hang off those ticks: `custody::law_standing_hud` skips the
    /// frame unless the standing moved, and `dogs::sync_dogs` unless the pack
    /// did. Handed bare `&mut`s, Rust coerces through `DerefMut` and stamps
    /// every channel on every message, so both gates were dead code that never
    /// skipped a frame in the game's life. Unflagged frames existing at all is
    /// the pin; how many is the engine's business, not this test's.
    #[test]
    fn an_ordinary_poll_leaves_the_law_and_the_pack_unflagged() {
        #[derive(Resource, Default)]
        struct FlaggedFrames {
            frames: u32,
            law: u32,
            dogs: u32,
        }

        fn count_flags(
            law: Res<custody::PlayerCustodyState>,
            dogs: Res<dogs::DogInbox>,
            mut counts: ResMut<FlaggedFrames>,
        ) {
            counts.frames += 1;
            counts.law += u32::from(law.is_changed());
            counts.dogs += u32::from(dogs.is_changed());
        }

        let mut app = ready_fake_plugin_app();
        app.init_resource::<FlaggedFrames>()
            .add_systems(Last, count_flags);
        // Enough real time for several engine polls — every one of them carries
        // a `Clock` and a `Weather`, which is exactly the traffic that used to
        // flag everything.
        for _ in 0..30 {
            app.update();
            thread::sleep(Duration::from_millis(2));
        }

        let counts = app.world().resource::<FlaggedFrames>();
        assert!(counts.frames >= 30, "the probe ran on every frame");
        assert!(
            counts.law < counts.frames,
            "the player's standing was flagged on all {} frames — some message \
             that does not write it is stamping it",
            counts.frames,
        );
        assert!(
            counts.dogs < counts.frames,
            "the pack was flagged on all {} frames — some message that does not \
             write it is stamping it",
            counts.frames,
        );
    }

    /// The rejection notice through the real plugin: Ilse holds her coin out to
    /// the player, the player walks off up the street, and the HUD says in its
    /// own slot why the coin never changed hands. The sim's half of this is
    /// `engine_tests::walking_out_of_earshot_lapses_the_offer_and_tells_the_host_why`;
    /// this is the wire from that event to the words on screen.
    #[test]
    fn walking_away_from_an_offer_puts_the_reason_and_its_cause_on_the_hud() {
        let mut app = ready_fake_plugin_app();
        let ilse = cathedral_sim::ActorId::from_raw("k0fb1");
        {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            let sim = engine.world_mut().expect("the engine is live");
            // Beside the player, so the offer is inside the 4 m it needs. Where
            // her round has her standing varies with the hour.
            sim.characters
                .get_mut(&ilse)
                .expect("Ilse is in the seeded cast")
                .state
                .position_m = cathedral_sim::Vec3::new(1.0, 0.91, 111.0);
            cathedral_sim::apply_action(
                sim,
                &ilse,
                "offer_item",
                &serde_json::json!({"item_id": "c0prs", "target": "player"}),
            )
            .expect("Ilse holds her coin out to the player");
        }

        // The card first: what the player is walking away from.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline
            && app
                .world()
                .resource::<hud::SmartActorHudState>()
                .offer_card
                .is_empty()
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            app.world()
                .resource::<hud::SmartActorHudState>()
                .offer_card
                .contains("offers you"),
            "the offer card shows before the player leaves"
        );

        // Forty metres up the street. The position rides the 10 Hz hot channel,
        // so this takes a few frames of real time to land.
        {
            let world = app.world_mut();
            let mut players =
                world.query_filtered::<&mut Transform, With<crate::controller::PlayerController>>();
            players
                .single_mut(world)
                .expect("the player exists")
                .translation = Vec3::new(0.0, 0.91, 151.0);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline
            && app
                .world()
                .resource::<hud::SmartActorHudState>()
                .offer_outcome_text()
                .is_none()
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }

        let hud_state = app.world().resource::<hud::SmartActorHudState>();
        assert_eq!(
            hud_state.offer_outcome_text(),
            Some(
                "OFFER LAPSED\nYou and Ilse drifted more than 20 m apart — the spark stays with them"
            )
        );
        // The card it replaces is gone: the offer really ended, and the HUD is
        // not still inviting a [Y] that would now fail.
        assert!(hud_state.offer_card.is_empty());
    }

    /// The law's hands reach the screen (`law_and_order.md` M4). A sergeant
    /// takes the player in charge in the sim, and the HUD's standing line says
    /// so — the whole wire, from `world.custody` through the hot
    /// [`EngineMessage::LawStanding`] channel to words the player can read.
    ///
    /// The line is not decoration: custody the player cannot see is custody they
    /// cannot answer, and every rung of this feature owes them a named door out.
    #[test]
    fn being_taken_in_charge_puts_the_law_standing_line_on_the_hud() {
        let mut app = ready_fake_plugin_app();
        let ashe = cathedral_sim::ActorId::from_raw("p009x");
        {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            let sim = engine.world_mut().expect("the engine is live");
            // Beside the player: `seize` is a four-metre verb, and its whole
            // point is that an officer must close on foot first.
            sim.characters
                .get_mut(&ashe)
                .expect("Havise Ashe is in the seeded cast")
                .state
                .position_m = cathedral_sim::Vec3::new(1.0, 0.91, 111.0);
        }
        app.world()
            .resource::<bridge::BridgeHandle>()
            .try_send(bridge::BridgeCommand::DebugSeize {
                officer: "Havise Ashe".into(),
                target: None,
            })
            .expect("the command queue has room");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline
            && app
                .world()
                .resource::<hud::SmartActorHudState>()
                .law_standing_text()
                .is_empty()
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }

        let line = app
            .world()
            .resource::<hud::SmartActorHudState>()
            .law_standing_text()
            .to_string();
        assert!(
            line.contains("TAKEN YOU IN CHARGE"),
            "the standing line names the custody, got {line:?}"
        );
        // …and where they are walking you. Which posting that is depends on
        // where the sergeant happened to be standing, so ask the sim rather
        // than naming one here: the wire is what is under test, not the
        // geometry that decided the nearest door.
        let station = {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            let sim = engine.world_mut().expect("the engine is live");
            sim.custody
                .iter()
                .find(|(prisoner, _)| prisoner.as_str() == PLAYER_ID)
                .expect("the player is in charge")
                .1
                .station
                .name
                .clone()
        };
        assert!(
            line.contains(&station),
            "…and where they are walking you ({station}), got {line:?}"
        );
        // The leash is explained the first time it is ever drawn: a player who
        // does not know they may step aside will not.
        assert!(
            line.contains("Walk with them"),
            "…and what the arrangement is, got {line:?}"
        );

        // …and it reaches the entity the player actually looks at, shown rather
        // than laid out at zero size. A standing line that lives only in a
        // resource is the same as no standing line at all.
        app.update();
        let world = app.world_mut();
        let (text, node) = world
            .query_filtered::<(&Text, &Node), With<hud::LawStandingText>>()
            .single(world)
            .expect("the standing line has a text entity");
        assert!(text.0.contains("TAKEN YOU IN CHARGE"), "got {:?}", text.0);
        assert_eq!(node.display, Display::Flex, "and it is not hidden");
    }

    /// …and so does the meter (`law_and_order.md` M4d). The strain bar is the
    /// host's half of that same line — the sim owns the words, the host owns the
    /// pull — and the two have to be composed by **one** writer, downstream of
    /// the drain, because the sim's half arrives constantly: `anchor_m` follows
    /// the grip, so every step an escorting officer takes republishes
    /// `LawStanding`. A bar written earlier in the frame is thrown away before
    /// anybody can see it, in precisely the situation the meter exists for.
    #[test]
    fn the_strain_bar_survives_the_standing_line_a_walking_officer_republishes() {
        let mut app = ready_fake_plugin_app();
        let ashe = cathedral_sim::ActorId::from_raw("p009x");
        let stand_ashe_at = |app: &mut App, at: cathedral_sim::Vec3| {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            let sim = engine.world_mut().expect("the engine is live");
            sim.characters
                .get_mut(&ashe)
                .expect("Havise Ashe is in the seeded cast")
                .state
                .position_m = at;
        };
        // The standing line the sim has settled on, once it says `needle`.
        let settle_on = |app: &mut App, needle: &str| {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while std::time::Instant::now() < deadline
                && !app
                    .world()
                    .resource::<hud::SmartActorHudState>()
                    .law_standing_text()
                    .contains(needle)
            {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }
            let line = app
                .world()
                .resource::<hud::SmartActorHudState>()
                .law_standing_text()
                .to_string();
            assert!(line.contains(needle), "expected {needle:?}, got {line:?}");
        };

        // Beside the player, taken in charge, and then a hand on the arm — the
        // command the host's own grab reflex sends.
        stand_ashe_at(&mut app, cathedral_sim::Vec3::new(1.0, 0.91, 111.0));
        app.world()
            .resource::<bridge::BridgeHandle>()
            .try_send(bridge::BridgeCommand::DebugSeize {
                officer: "Havise Ashe".into(),
                target: None,
            })
            .expect("the command queue has room");
        settle_on(&mut app, "TAKEN YOU IN CHARGE");
        app.world()
            .resource::<bridge::BridgeHandle>()
            .try_send(bridge::BridgeCommand::PlayerGrabbed {
                holder_id: model::ActorId("p009x".into()),
            })
            .expect("the command queue has room");
        settle_on(&mut app, "HELD BY");

        // Half a meter's worth of pulling. This harness has no controller
        // plugin, so `strain_meter` never pumps — its arithmetic is unit-tested
        // in `custody.rs`, and what is on trial here is only whether a meter the
        // player has filled survives to the screen.
        app.world_mut()
            .resource_mut::<custody::PlayerCustodyState>()
            .strain = 0.5;
        // Havise takes a step, which is all it takes: the grip point moved, so
        // the sim republishes the whole standing line in this frame's drain.
        stand_ashe_at(&mut app, cathedral_sim::Vec3::new(1.4, 0.91, 111.0));
        app.update();

        let line = app
            .world()
            .resource::<hud::SmartActorHudState>()
            .law_standing_text()
            .to_string();
        assert!(line.contains("HELD BY"), "got {line:?}");
        assert!(
            line.contains("[#####-----]"),
            "the meter the player filled is still on the line, got {line:?}"
        );
        let world = app.world_mut();
        let (text, _) = world
            .query_filtered::<(&Text, &Node), With<hud::LawStandingText>>()
            .single(world)
            .expect("the standing line has a text entity");
        assert!(text.0.contains("[#####-----]"), "got {:?}", text.0);
    }

    /// …and an engine that dies with hold of the player lets them go
    /// (`law_and_order.md` M4c). A sim panic is a real ending — `LocalEngine`
    /// catches it, drops the engine and disconnects — and custody is the one
    /// projection that would outlive it: the tether clamps the player's desired
    /// position every fixed step, around an anchor nobody is standing at any
    /// more, and the `LawStanding` that would end the hold can never arrive,
    /// because the drain refuses every message once the engine is known dead.
    #[test]
    fn an_engine_that_dies_holding_the_player_lets_go_of_them() {
        let mut app = ready_fake_plugin_app();
        let ashe = cathedral_sim::ActorId::from_raw("p009x");
        {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            let sim = engine.world_mut().expect("the engine is live");
            sim.characters
                .get_mut(&ashe)
                .expect("Havise Ashe is in the seeded cast")
                .state
                .position_m = cathedral_sim::Vec3::new(1.0, 0.91, 111.0);
        }
        let settle_on = |app: &mut App, needle: &str| {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while std::time::Instant::now() < deadline
                && !app
                    .world()
                    .resource::<hud::SmartActorHudState>()
                    .law_standing_text()
                    .contains(needle)
            {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }
            let line = app
                .world()
                .resource::<hud::SmartActorHudState>()
                .law_standing_text()
                .to_string();
            assert!(line.contains(needle), "expected {needle:?}, got {line:?}");
        };

        // Taken in charge, then a hand on the arm — the command the host's own
        // grab reflex sends. Only a *held* player is tethered.
        app.world()
            .resource::<bridge::BridgeHandle>()
            .try_send(bridge::BridgeCommand::DebugSeize {
                officer: "Havise Ashe".into(),
                target: None,
            })
            .expect("the command queue has room");
        settle_on(&mut app, "TAKEN YOU IN CHARGE");
        app.world()
            .resource::<bridge::BridgeHandle>()
            .try_send(bridge::BridgeCommand::PlayerGrabbed {
                holder_id: model::ActorId("p009x".into()),
            })
            .expect("the command queue has room");
        settle_on(&mut app, "HELD BY");
        assert!(
            app.world()
                .resource::<custody::PlayerCustodyState>()
                .tether(false)
                .is_some(),
            "the tether is on before the engine dies"
        );

        // The sim panics mid-poll.
        app.world_mut()
            .non_send_mut::<local_engine::LocalEngine>()
            .die_as_if_panicked();
        app.update();

        assert!(!app.world().resource::<SmartActorRuntime>().connected);
        assert!(
            app.world()
                .resource::<custody::PlayerCustodyState>()
                .tether(false)
                .is_none(),
            "a hold nobody is left to end must not outlive its engine"
        );
        // …and the line comes off the screen with it: an offline HUD claiming
        // the player is held names a door that no longer opens.
        assert_eq!(
            app.world()
                .resource::<hud::SmartActorHudState>()
                .law_standing_text(),
            ""
        );
        let world = app.world_mut();
        let (text, node) = world
            .query_filtered::<(&Text, &Node), With<hud::LawStandingText>>()
            .single(world)
            .expect("the standing line has a text entity");
        assert_eq!(text.0, "", "got {:?}", text.0);
        assert_eq!(node.display, Display::None, "and it is hidden");
    }

    /// …and the drawn arm comes off when the sim says the hand did
    /// (`law_and_order.md` M4c). An NPC's custody is in **no** snapshot — only
    /// the player's is projected — so the visible grip is fed entirely by world
    /// events: it goes on with `grab`, and the answering event is the only thing
    /// that can ever take it off again. Every clock-driven ending of a grip
    /// sends `let_go`, which this file's match used to drop into `_ => {}`, so
    /// an officer's arm stayed clamped to somebody nobody was holding any more
    /// for the rest of the run.
    ///
    /// The dead-man timer is the trigger because it leaves the pair standing
    /// exactly where they were. The 4 m grip-break backstop is the only other
    /// thing that can put an arm down, and a test it could explain would prove
    /// nothing at all.
    #[test]
    fn a_hand_the_sim_takes_off_an_arm_stops_being_drawn() {
        let mut app = ready_fake_plugin_app();
        let ilse = cathedral_sim::ActorId::from_raw("k0fb1");
        let ashe = cathedral_sim::ActorId::from_raw("p009x");

        // Where the officer's arm is aimed this frame, straight off the pose the
        // renderer reads — the thing the player actually sees.
        let grip_of = |app: &mut App, id: &str| -> Option<Vec3> {
            let world = app.world_mut();
            world
                .query::<(&model::ActorId, &body::BodyPoseState)>()
                .iter(world)
                .find(|(actor_id, _)| actor_id.0 == id)
                .and_then(|(_, pose)| pose.grip())
        };
        let body_at = |app: &mut App, id: &str| -> Vec3 {
            let world = app.world_mut();
            world
                .query::<(&model::ActorId, &Transform)>()
                .iter(world)
                .find(|(actor_id, _)| actor_id.0 == id)
                .map(|(_, transform)| transform.translation)
                .expect("the cast is spawned")
        };

        // Havise takes Ilse in charge — the stand-in stands her at arm's reach
        // first, exactly where an officer's own chase would end — and then puts
        // a hand on her arm, through the `grab` verb's own path.
        app.world()
            .resource::<bridge::BridgeHandle>()
            .try_send(bridge::BridgeCommand::DebugSeize {
                officer: "Havise Ashe".into(),
                target: Some("k0fb1".into()),
            })
            .expect("the command queue has room");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            let held = engine
                .world_mut()
                .expect("the engine is live")
                .custody
                .holds(&ilse);
            drop(engine);
            if held {
                break;
            }
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            let sim = engine.world_mut().expect("the engine is live");
            assert!(sim.custody.holds(&ilse), "the stand-in took her in charge");
            cathedral_sim::apply_action(
                sim,
                &ashe,
                "grab",
                &serde_json::json!({"person": "k0fb1"}),
            )
            .expect("an officer at arm's reach may take hold");
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline && grip_of(&mut app, "p009x").is_none() {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            grip_of(&mut app, "p009x").is_some(),
            "the `grab` event put her arm on Ilse's"
        );

        // Now starve the lane the holder's turn would come down: past
        // `CUSTODY_DEAD_MAN_SECONDS` the sim takes the hand off by itself, and
        // the only thing it says about it is `let_go`. Re-stamped every frame,
        // because a turn of Havise's own restarts that clock.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline && grip_of(&mut app, "p009x").is_some() {
            {
                let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
                let sim = engine.world_mut().expect("the engine is live");
                if let Some(record) = sim.custody.get_mut(&ilse) {
                    record.officer_last_turn = Some(-1.0e9);
                }
            }
            app.update();
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(
            grip_of(&mut app, "p009x"),
            None,
            "the hand came off, so the arm does"
        );
        // …and not merely because they drifted: the pair are still a pace apart,
        // nowhere near the backstop that would explain this without the event.
        let apart = body_at(&mut app, "p009x").distance(body_at(&mut app, "k0fb1"));
        assert!(
            apart <= hands::GRIP_BREAKS_AT_M,
            "still well within reach of each other ({apart} m)"
        );
        // And the custody itself stands — the dead-man timer ends a grip and
        // never a custody — so what came off the screen is the hand, not the law.
        let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
        let sim = engine.world_mut().expect("the engine is live");
        assert!(sim.custody.holds(&ilse), "she is still in the law's charge");
        assert!(!sim.custody.is_held(&ilse), "but nobody has hold of her");
    }

    /// Pumps frames until a soundscape cue the drain writes matches, and hands
    /// back what `pick` took out of it. Deliberately no `thread::sleep`: the
    /// local engine drains its command queue on the very next poll, and real
    /// seconds passing would let the round walk the subjects out from under the
    /// assertion.
    fn pump_for_cue<T>(
        app: &mut App,
        mut pick: impl FnMut(crate::soundscape::SoundscapeCue) -> Option<T>,
        note: &str,
    ) -> T {
        for _ in 0..600 {
            app.update();
            let cues: Vec<crate::soundscape::SoundscapeCue> = app
                .world_mut()
                .resource_mut::<Messages<crate::soundscape::SoundscapeCue>>()
                .drain()
                .collect();
            for cue in cues {
                if let Some(found) = pick(cue) {
                    return found;
                }
            }
        }
        panic!("{note}");
    }

    /// The two custody cues are real clips at a real `Vec3`
    /// (`soundscape.rs`: `GatekeeperKeyRing` and `StoneHouseCellDoor` are
    /// scheduled at exactly the position handed over), and both used to be read
    /// off `ActorSnapshot::position_m` — the *cold* channel. `step_movement`
    /// never bumps the world revision, and `Engine::flush` pushes a poll's world
    /// events before the snapshot that poll owes, so that reading is always at
    /// least one revision behind the event being reacted to. The drive stand-ins
    /// are the extreme of it and the reason this test uses them: `debug_seize`
    /// teleports the officer to arm's reach and `debug_commit` puts everybody in
    /// the gaol, both inside the very poll whose event is then placed from the
    /// pose they held *before* it.
    #[test]
    fn the_custody_cues_sound_where_the_scene_is_and_not_a_revision_behind_it() {
        let mut app = ready_fake_plugin_app();
        let ilse = cathedral_sim::ActorId::from_raw("k0fb1");
        let ashe = cathedral_sim::ActorId::from_raw("p009x");
        // A corner of the Wick Ward, and a beat right across the city from it:
        // whichever of the two the cue picks up is unmistakable.
        let corner = Vec3::new(12.0, 0.91, 140.0);
        let far_beat = Vec3::new(-214.0, 0.91, 45.0);
        // The gaol as the *sim* knows it, so the door assertion below is not
        // merely the host agreeing with its own constant.
        let gaol = {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            let sim = engine.world_mut().expect("the engine is live");
            let point = cathedral_sim::custody::stone_house(&sim.places)
                .expect("the Stone House is in the registry")
                .point;
            Vec3::new(point.x as f32, point.y as f32, point.z as f32)
        };

        hold_actor_at(&mut app, &ashe, far_beat, 3);
        hold_actor_at(&mut app, &ilse, corner, 3);

        app.world()
            .resource::<bridge::BridgeHandle>()
            .try_send(bridge::BridgeCommand::DebugSeize {
                officer: "Havise Ashe".into(),
                target: Some("k0fb1".into()),
            })
            .expect("the command queue has room");
        let keys = pump_for_cue(
            &mut app,
            |cue| match cue {
                crate::soundscape::SoundscapeCue::CustodyKeys { position } => Some(position),
                _ => None,
            },
            "the seizure never rattled any keys",
        );
        // At the woman being taken, which is where the officer now stands too —
        // `seize` is a four-metre verb — and not out on the beat she was walking
        // when the last snapshot went out.
        assert!(
            keys.distance(corner) < 1.5,
            "the keys rattled {:.0} m from the seizure, out at {:?}",
            keys.distance(corner),
            keys,
        );
        assert!(
            keys.distance(far_beat) > 100.0,
            "and emphatically not back on her old beat"
        );

        app.world()
            .resource::<bridge::BridgeHandle>()
            .try_send(bridge::BridgeCommand::DebugCommit {
                target: Some("k0fb1".into()),
            })
            .expect("the command queue has room");
        let door = pump_for_cue(
            &mut app,
            |cue| match cue {
                crate::soundscape::SoundscapeCue::GaolDoor { position } => Some(position),
                _ => None,
            },
            "the commitment never shut a door",
        );
        // The one door in the city that is a door, and it is a fitting of the
        // city: it shuts at the Stone House whatever the projection still
        // believes about where the prisoner is standing.
        assert!(
            door.distance(gaol) < 8.0,
            "the gaol door slammed {:.0} m from the gaol, out at {:?}",
            door.distance(gaol),
            door,
        );
    }

    /// Holds one of the cast at a fixed pose, exactly as `Engine::debug_commit`
    /// leaves somebody it has teleported: position, path and errand written in
    /// one poll, and the world revision bumped for it. Re-applied every frame
    /// because the round goes on running underneath — otherwise an errand laid
    /// mid-test would walk the subject off and the two channels would be
    /// disagreeing about something other than the thing under test.
    fn hold_actor_at(app: &mut App, actor_id: &cathedral_sim::ActorId, at: Vec3, frames: usize) {
        for _ in 0..frames {
            {
                let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
                let sim = engine.world_mut().expect("the engine is live");
                let character = sim
                    .characters
                    .get_mut(actor_id)
                    .expect("the actor is in the seeded cast");
                character.state.position_m =
                    cathedral_sim::Vec3::new(at.x.into(), at.y.into(), at.z.into());
                character.state.movement = None;
                character.state.intent = None;
                sim.touch_public_state();
            }
            app.update();
        }
    }

    /// Where the renderer is actually drawing somebody this frame, or `None`
    /// while the projection has no body for them at all.
    fn projected_body(app: &mut App, id: &str) -> Option<Vec3> {
        let world = app.world_mut();
        world
            .query_filtered::<(&model::ActorId, &Transform), With<actors::ActorView>>()
            .iter(world)
            .find(|(actor_id, _)| actor_id.0 == id)
            .map(|(_, transform)| transform.translation)
    }

    /// Asserts the drawn body stands where the authoritative pose says it does.
    /// Deliberately not an exact compare: interpolating between two samples of
    /// the same pose still costs `Vec3::lerp` a float ULP, and every failure
    /// these tests guard against is hundreds of metres wide.
    fn assert_body_stands_at(app: &mut App, id: &str, expected: Vec3, note: &str) {
        let body = projected_body(app, id).expect("the actor has a projected body");
        let drift = body.distance(expected);
        assert!(drift < 0.01, "{note}: {drift} m off {expected}");
    }

    /// The snapshot outranks a hot-channel sample it disagrees with.
    ///
    /// Between revisions the mirror's position for a walker is stale and
    /// `actors::drive_npc_bodies` overriding it is the whole point of the
    /// hot/cold split — but the sim also moves people by paths that ship no
    /// 20 Hz tick at all, and the documented one is the drive script's `commit`
    /// teleport into the Stone House. Nothing used to retire the sample from
    /// before such a move, so the body was written straight back out into the
    /// street it had been walking down and stood there until some errand
    /// happened to give it a new walk: `shot cell` photographed an empty cell.
    #[test]
    fn a_reposition_in_the_snapshot_retires_the_stale_movement_sample() {
        let mut app = ready_fake_plugin_app();
        let ilse = cathedral_sim::ActorId::from_raw("k0fb1");
        let street = Vec3::new(12.0, 0.91, 140.0);
        let gaol = Vec3::new(44.5, 0.91, -207.2);

        hold_actor_at(&mut app, &ilse, street, 4);
        assert_body_stands_at(
            &mut app,
            "k0fb1",
            street,
            "the projection has her out in the street",
        );

        // The walk she is on when the sim moves her: one 20 Hz sample, written
        // exactly as the `Movement` arm writes one.
        {
            let mut inbox = app.world_mut().resource_mut::<model::MovementInbox>();
            let sample = inbox.0.entry(model::ActorId("k0fb1".into())).or_default();
            sample.position = street;
            sample.speed = 2.1;
            sample.seq = sample.seq.wrapping_add(1);
        }
        app.update();
        app.update();
        assert_body_stands_at(
            &mut app,
            "k0fb1",
            street,
            "and the hot channel is what is driving her body",
        );

        // A revision bump that does not move her must not cost her that sample:
        // agreeing with the snapshot is precisely what an ordinary walker does.
        {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            engine
                .world_mut()
                .expect("the engine is live")
                .touch_public_state();
        }
        app.update();
        app.update();
        assert!(
            app.world()
                .resource::<model::MovementInbox>()
                .0
                .contains_key(&model::ActorId("k0fb1".into())),
            "a snapshot that agrees with the sample leaves it alone"
        );

        // …and now the teleport `commit` is: she is inside the Stone House, and
        // no movement tick ever said so.
        hold_actor_at(&mut app, &ilse, gaol, 4);
        assert_body_stands_at(
            &mut app,
            "k0fb1",
            gaol,
            &format!(
                "the snapshot outranks a sample from {} m up the street",
                street.distance(gaol).round()
            ),
        );

        // And the half-finished sweep went with the sample. Left behind, her
        // next step would open from the street pose it is still holding — and
        // its `seq` could even match the re-created channel's and be read as a
        // re-read of the tick that made it.
        let world = app.world_mut();
        assert!(
            world
                .query_filtered::<&model::ActorId, With<actors::NpcMotion>>()
                .iter(world)
                .all(|actor_id| actor_id.0 != "k0fb1"),
            "the interpolation the snapshot overruled was dropped with it"
        );
    }

    /// The same rule for the road parties, which is where it bites without a
    /// drive script. A carrier who leaves the city drops out of the snapshot and
    /// their actor view is despawned; when they re-enter at their gate they get
    /// a fresh entity standing on `party.gate_point`. The sample from the walk
    /// they departed on is still in the inbox, so the first thing the new body
    /// used to do was slide off the gate and back to wherever they had been days
    /// before.
    #[test]
    fn re_entering_at_a_gate_is_not_undone_by_a_pre_departure_sample() {
        let mut app = ready_fake_plugin_app();
        let ilse = cathedral_sim::ActorId::from_raw("k0fb1");
        let street = Vec3::new(12.0, 0.91, 140.0);
        let gate = Vec3::new(-4.0, 0.91, 246.0);

        hold_actor_at(&mut app, &ilse, street, 4);
        {
            let mut inbox = app.world_mut().resource_mut::<model::MovementInbox>();
            let sample = inbox.0.entry(model::ActorId("k0fb1".into())).or_default();
            sample.position = street;
            sample.speed = 2.1;
            sample.seq = sample.seq.wrapping_add(1);
        }
        app.update();
        assert_body_stands_at(
            &mut app,
            "k0fb1",
            street,
            "she is on the street with a live sample under her",
        );

        // Out through the gate with the party. She leaves the snapshot entirely,
        // and her body with it.
        {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            engine
                .world_mut()
                .expect("the engine is live")
                .transition_presence(
                    std::slice::from_ref(&ilse),
                    cathedral_sim::Presence::BeyondTheWalls,
                    &std::collections::BTreeMap::new(),
                )
                .expect("she may leave the city");
        }
        app.update();
        app.update();
        assert_eq!(
            projected_body(&mut app, "k0fb1"),
            None,
            "her actor view went with her"
        );
        assert!(
            !app.world()
                .resource::<model::MovementInbox>()
                .0
                .contains_key(&model::ActorId("k0fb1".into())),
            "and so did the sample: an actor the snapshot no longer carries has \
             no authoritative pose for one to agree with"
        );

        // …and back in days later, on the gate point the party re-enters at.
        {
            let mut engine = app.world_mut().non_send_mut::<local_engine::LocalEngine>();
            let mut entry = std::collections::BTreeMap::new();
            entry.insert(
                ilse.clone(),
                cathedral_sim::Vec3::new(gate.x.into(), gate.y.into(), gate.z.into()),
            );
            engine
                .world_mut()
                .expect("the engine is live")
                .transition_presence(
                    std::slice::from_ref(&ilse),
                    cathedral_sim::Presence::InCity,
                    &entry,
                )
                .expect("she may come home");
        }
        hold_actor_at(&mut app, &ilse, gate, 4);
        assert_body_stands_at(
            &mut app,
            "k0fb1",
            gate,
            "the new body stands on the gate it entered at",
        );
    }

    /// The seam's acceptance test: the whole plugin, the in-process engine, and
    /// the fake backends — no subprocess, no network, no `uv`.
    #[test]
    fn complete_plugin_reaches_ready_and_spawns_the_cast_headlessly() {
        let mut app = ready_fake_plugin_app();

        {
            let world = app.world_mut();
            let visibility = world
                .query_filtered::<&Visibility, With<hud::SmartActorStatusPanel>>()
                .single(world)
                .expect("the actor status panel exists");
            assert_eq!(*visibility, Visibility::Hidden);
        }

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        assert!(
            app.world()
                .resource::<area_debug::AreaDebugState>()
                .is_enabled()
        );
        {
            let world = app.world_mut();
            let visibility = world
                .query_filtered::<&Visibility, With<hud::SmartActorStatusPanel>>()
                .single(world)
                .expect("the actor status panel remains available");
            assert_eq!(*visibility, Visibility::Inherited);
        }
        assert_eq!(
            app.world()
                .resource::<area_debug::AreaDebugState>()
                .visible_area_ids()
                .len(),
            8
        );
        let expected_box_labels = app
            .world()
            .non_send::<local_engine::LocalEngine>()
            .area_map()
            .expect("the area map is loaded")
            .areas
            .iter()
            .map(|area| area.boxes.len())
            .sum::<usize>();
        let world = app.world_mut();
        assert_eq!(
            world
                .query::<&area_debug::AreaBoxLabel>()
                .iter(world)
                .count(),
            expected_box_labels
        );
        let (location_text, visibility) = world
            .query_filtered::<(&Text, &Visibility), With<area_debug::PlayerAreaDescription>>()
            .single(world)
            .expect("the area debug player label exists");
        assert!(location_text.0.contains("The Gradine"));
        assert_eq!(*visibility, Visibility::Inherited);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        assert!(
            !app.world()
                .resource::<area_debug::AreaDebugState>()
                .is_enabled()
        );
        let world = app.world_mut();
        let (_, visibility) = world
            .query_filtered::<(&Text, &Visibility), With<area_debug::PlayerAreaDescription>>()
            .single(world)
            .expect("the area debug player label remains available");
        assert_eq!(*visibility, Visibility::Hidden);
        let status_visibility = world
            .query_filtered::<&Visibility, With<hud::SmartActorStatusPanel>>()
            .single(world)
            .expect("the actor status panel remains available");
        assert_eq!(*status_visibility, Visibility::Hidden);

        let world = app.world_mut();
        let actor_count = world
            .query_filtered::<Entity, With<actors::ActorView>>()
            .iter(world)
            .count();
        let expected_cast = cathedral_backends::world_data::character_sources(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lore"),
        )
        .expect("the lore cast is readable")
        .len();
        let projected_npcs = world
            .resource::<model::WorldMirror>()
            .actors()
            .filter(|actor| actor.control == model::ActorControl::Llm)
            .count();
        assert_eq!(actor_count, projected_npcs);
        assert_eq!(
            expected_cast - projected_npcs,
            5,
            "the five fixed road actors begin beyond the walls"
        );
        let runtime_dir = world
            .resource::<bridge::BridgeHandle>()
            .runtime_dir()
            .to_path_buf();

        app.world_mut().write_message(InjectPlayerTranscript {
            text: "What's your name?".into(),
            target_id: Some(model::ActorId("k0fb1".into())),
        });
        let reply_deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut reply_bubble_seen = false;
        while std::time::Instant::now() < reply_deadline && !reply_bubble_seen {
            app.update();
            let world = app.world_mut();
            reply_bubble_seen = world
                .query_filtered::<Entity, With<speech::SpeechBubble>>()
                .iter(world)
                .next()
                .is_some();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(reply_bubble_seen, "Ilse's fake reply was not presented");
        assert!(
            !app.world()
                .resource::<hud::SmartActorHudState>()
                .subtitle
                .is_empty()
        );
        assert_eq!(
            app.world()
                .resource::<hud::SmartActorHudState>()
                .player_transcript_text(),
            Some("You: What's your name?  ·  heard by 3 nearby people")
        );

        app.world_mut().write_message(InjectPlayerTranscript {
            text: "Please offer me your coin".into(),
            target_id: Some(model::ActorId("k0fb1".into())),
        });
        let offer_deadline = std::time::Instant::now() + Duration::from_secs(8);
        while std::time::Instant::now() < offer_deadline
            && app
                .world()
                .resource::<model::WorldMirror>()
                .offer(&model::ItemId("c0prs".into()))
                .is_none()
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        app.update();
        let mirror = app.world().resource::<model::WorldMirror>();
        let coin_offer = mirror
            .offer(&model::ItemId("c0prs".into()))
            .expect("Ilse did not offer the coin");
        assert_eq!(coin_offer.giver_id.0, "k0fb1");
        assert_eq!(
            coin_offer.target_id.as_ref().map(|id| id.0.as_str()),
            Some("player")
        );
        assert!(
            mirror
                .actor(&model::ActorId("k0fb1".into()))
                .is_some_and(|ilse| ilse.holds.contains(&model::ItemId("c0prs".into())))
        );
        // npc_bodies M2: the offered coin sits in Ilse's RIGHT hand, not in
        // an above-head fan.
        let world = app.world_mut();
        let offered_coin_props: Vec<_> = world
            .query::<&hands::HeldProp>()
            .iter(world)
            .filter(|prop| prop.item_id.0 == "c0prs")
            .map(|prop| (prop.actor.0.clone(), prop.side))
            .collect();
        assert_eq!(
            offered_coin_props,
            vec![("k0fb1".to_string(), body::BodySide::Right)]
        );
        assert!(
            app.world()
                .resource::<hud::SmartActorHudState>()
                .offer_card
                .contains("spark")
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyY);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        let accept_deadline = std::time::Instant::now() + Duration::from_secs(4);
        while std::time::Instant::now() < accept_deadline
            && !app
                .world()
                .resource::<model::WorldMirror>()
                .actor(&model::ActorId("player".into()))
                .is_some_and(|player| player.holds.contains(&model::ItemId("c0prs".into())))
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        let mirror = app.world().resource::<model::WorldMirror>();
        assert!(
            mirror
                .actor(&model::ActorId("player".into()))
                .is_some_and(|player| player.holds.contains(&model::ItemId("c0prs".into())))
        );
        assert!(mirror.offer(&model::ItemId("c0prs".into())).is_none());

        app.update();
        assert_eq!(
            app.world()
                .resource::<ActorFocus>()
                .item
                .as_ref()
                .map(|focus| focus.actor_id.0.as_str()),
            Some("cb947")
        );
        // Select the coin by slot rather than relying on it being the only
        // thing the player holds. The player is seeded with a chalk pen
        // (`features/implemented/chalking_the_walls.md` M2), so slot 1 is the pen and the
        // spark this test is about is slot 2 — and the offer path, which is
        // what is under test here, has no opinion about inventory order.
        let coin_slot = app
            .world()
            .resource::<model::WorldMirror>()
            .actor(&model::ActorId("player".into()))
            .and_then(|player| {
                player
                    .holds
                    .iter()
                    .position(|held| held.0 == "c0prs")
            })
            .expect("the player holds the spark by now");
        let slot_key = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3][coin_slot];
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(slot_key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        let reoffer_deadline = std::time::Instant::now() + Duration::from_secs(4);
        while std::time::Instant::now() < reoffer_deadline
            && !app
                .world()
                .resource::<model::WorldMirror>()
                .offer(&model::ItemId("c0prs".into()))
                .is_some_and(|offer| {
                    offer.giver_id.0 == "player"
                        && offer.target_id.as_ref().is_some_and(|id| id.0 == "cb947")
                })
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        let mirror = app.world().resource::<model::WorldMirror>();
        assert!(
            mirror
                .offer(&model::ItemId("c0prs".into()))
                .is_some_and(|offer| {
                    offer.giver_id.0 == "player"
                        && offer.target_id.as_ref().is_some_and(|id| id.0 == "cb947")
                })
        );
        assert!(
            mirror
                .actor(&model::ActorId("player".into()))
                .is_some_and(|player| player.holds.contains(&model::ItemId("c0prs".into())))
        );

        drop(app);
        let cleanup_deadline = std::time::Instant::now() + Duration::from_secs(2);
        while runtime_dir.exists() && std::time::Instant::now() < cleanup_deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!runtime_dir.exists());
    }
}
