//! Production soundscape for the procedural city.
//!
//! This is deliberately separate from `smart_actors::sound`.  That module
//! presents authoritative sound *events* which NPCs can perceive; the sounds
//! here are render-side texture and must never fill actor inboxes or make every
//! nearby body turn its head.  Callers bridge genuine events (a sale, a cargo
//! handoff, a well draw, or a gate transition) through [`SoundscapeCue`].
//!
//! Long-running sources are virtualized.  Only the nearest useful emitters own
//! decoders, their gains fade rather than switch, and an active conversation or
//! microphone capture ducks them through the shared `AudioActivity` projection.

use std::collections::{HashMap, HashSet};

use bevy::{
    audio::{
        AudioPlayer, AudioPlugin, AudioSinkPlayback, AudioSource, PlaybackSettings,
        SpatialAudioSink, SpatialScale, Volume,
    },
    prelude::*,
};
use cathedral_sim::{CartLoadKind, Office, Weekday};

use crate::{
    city::CobbleRoadNetwork,
    controller::PlayerController,
    smart_actors::{
        AudioActivity, WorldClockState,
        actors::ActorView,
        model::{ActorId, MovementInbox, WorldMirror},
        road_carts::RoadCartView,
    },
};

const SOUND_ROOT: &str = "sounds/soundscape";
const MAX_LIVE_LOOPS: usize = 8;
const LOOP_ACTIVATION_HYSTERESIS: f32 = 1.15;
const STALLED_AUDIO_TIMEOUT_SECONDS: f64 = 20.0;
const SPATIAL_FULL_VOLUME_FRACTION: f32 = 1.0 / 8.0;
const TELEPORT_DISTANCE_M: f32 = 18.0;
const MIN_STEP_INTERVAL_SECONDS: f64 = 0.17;
const NPC_BODY_SOUND_GLOBAL_COOLDOWN_SECONDS: f64 = 12.0;
const GRAIN_SACK_CONTACT_DELAY_SECONDS: f64 = 0.20;
const CRATE_CONTACT_DELAY_SECONDS: f64 = 0.48;
const CHAIN_KNOCK_DELAY_SECONDS: f64 = 1.35;
const CHAIN_KNOCK_DURATION_SECONDS: f64 = 4.520;
const CROSSED_BUCKET_DELAY_SECONDS: f64 = 1.15;
const CROSSED_BUCKET_DURATION_SECONDS: f64 = 3.031;
const CUE_COOLDOWN_PRUNE_INTERVAL_SECONDS: f64 = 60.0;
const CUE_COOLDOWN_RETENTION_SECONDS: f64 = 120.0;

const WICKMARKET: Vec3 = Vec3::new(-25.0, 1.3, 355.0);
const TALLAGE_WEIGHBEAM: Vec3 = Vec3::new(-306.0, 1.5, 65.0);
const TALLAGE_SQUARE: Vec2 = Vec2::new(-305.0, 90.0);
const TALLAGE_MEASUREMENT_RADIUS_M: f32 = 62.0;
const COMMON_OVEN: Vec3 = Vec3::new(-165.875, 1.2, 439.625);
const CINDER_ROW: Vec3 = Vec3::new(-130.0, 1.2, 205.0);
const BURNT_COURT: Vec3 = Vec3::new(-172.0, 1.2, 232.0);
const STONE_GATE: Vec3 = Vec3::new(495.0, 5.0, 135.0);
const STONE_GATE_HOUSING: Vec3 = Vec3::new(483.5, 5.5, 135.0);
const RIVER_GATE: Vec3 = Vec3::new(-505.0, 4.0, -135.0);
const FORD_WELL: Vec3 = Vec3::new(88.0, 1.0, 35.0);
const THREE_CURB: Vec3 = Vec3::new(-131.0, 0.8, 166.0);
const CHAIN_WELL: Vec3 = Vec3::new(-197.0, -2.5, 73.0);

/// The integration boundary for genuine, externally observed activity.
///
/// The city/sim bridge emits these; this module decides gain, attenuation,
/// debounce, delays, loop lifetime and ducking.  Positions are world metres.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub(crate) enum SoundscapeCue {
    MarketCry {
        position: Vec3,
    },
    MarketMeasurement {
        position: Vec3,
    },
    /// Explicit prop-animation seam. Cart snapshot diffs use this same route;
    /// authored hoists/porters can emit it when their visible load touches down.
    CargoHandoff {
        position: Vec3,
        kind: CargoHandoffKind,
    },
    WellDraw {
        source: SpecialWell,
    },
    /// Optional authoritative production seam. The sim does not yet project
    /// work jobs to the host, so clock-gated emitters are today's fallback.
    #[allow(
        dead_code,
        reason = "authoritative work activity is not projected to the host yet"
    )]
    WorkActivity {
        kind: WorkActivityKind,
        position: Vec3,
        active: bool,
    },
    StoneGateClosing,
    RiverGateBarLift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CargoHandoffKind {
    GrainSack,
    Crate,
}

fn cargo_handoff_sound(kind: CargoHandoffKind) -> (SoundscapeSound, f64) {
    match kind {
        CargoHandoffKind::GrainSack => {
            (SoundscapeSound::SackDrop, GRAIN_SACK_CONTACT_DELAY_SECONDS)
        }
        CargoHandoffKind::Crate => (SoundscapeSound::CrateSetDown, CRATE_CONTACT_DELAY_SECONDS),
    }
}

fn cargo_handoff_shape(kind: CargoHandoffKind, position: Vec3, now: f64) -> (f32, f32) {
    if kind != CargoHandoffKind::GrainSack {
        return (1.0, 1.0);
    }
    // A narrow deterministic weight range keeps repeated porter work alive
    // without turning one reviewed recording into conspicuous random effects.
    let event_bucket = (now * 10.0).round() as i64;
    let seed = stable_hash(&format!(
        "sack-weight:{:.2}:{:.2}:{event_bucket}",
        position.x, position.z
    ));
    let gain = 0.95 + unit(seed) as f32 * 0.10;
    let speed = 0.965 + unit(seed.rotate_left(29)) as f32 * 0.07;
    (gain, speed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WorkActivityKind {
    Baking,
    EelSmoking,
    GlassFurnace,
    CulletSorting,
    Weaving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SpecialWell {
    Ford,
    Chain,
    ThreeCurb,
}

impl SpecialWell {
    const fn position(self) -> Vec3 {
        match self {
            Self::Ford => FORD_WELL,
            Self::Chain => CHAIN_WELL,
            Self::ThreeCurb => THREE_CURB,
        }
    }
}

/// Classify an authoritative generic draw event without leaking city geometry
/// into the smart-actor bridge.  Generic wells return `None` and keep their old
/// presentation.
pub(crate) fn classify_special_well(position: Vec3) -> Option<SpecialWell> {
    [
        SpecialWell::Ford,
        SpecialWell::Chain,
        SpecialWell::ThreeCurb,
    ]
    .into_iter()
    .filter_map(|well| {
        let delta = well.position().xz() - position.xz();
        (delta.length_squared() <= 12.0_f32.powi(2)).then_some((well, delta.length_squared()))
    })
    .min_by(|(_, a), (_, b)| a.total_cmp(b))
    .map(|(well, _)| well)
}

/// Convert a sale/coin sound inside the Tallage's bounded market area into the
/// authored weighbeam source.  The live stall pitch is about 25 m from the
/// beam, so testing only the beam itself would silently miss genuine sales;
/// the 62 m square radius covers its stalls without capturing the neighbouring
/// Weigh Ward streets wholesale.
pub(crate) fn tallage_measurement_anchor(position: Vec3) -> Option<Vec3> {
    (position.is_finite()
        && position.xz().distance_squared(TALLAGE_SQUARE) <= TALLAGE_MEASUREMENT_RADIUS_M.powi(2))
    .then_some(TALLAGE_WEIGHBEAM)
}

pub struct SoundscapePlugin;

/// Cross-plugin phases for soundscape cues.
///
/// Physical systems emit first, every interested presentation consumes the
/// same message frame second, and read-only animation state is projected last.
/// This keeps contact offsets exact without depending on plugin insertion
/// order or Bevy's parallel executor choices.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SoundscapeSet {
    EmitCues,
    IngestCues,
    ProjectActivity,
}

impl Plugin for SoundscapePlugin {
    fn build(&self, app: &mut App) {
        // These exist in headless apps as well: bridge and policy tests should
        // not need an output device merely to send a typed cue.
        app.add_message::<SoundscapeCue>()
            .init_resource::<ScheduledSounds>()
            .init_resource::<CueCooldowns>()
            .init_resource::<FootstepTracker>()
            .init_resource::<NpcSoundState>()
            .init_resource::<CartSoundState>()
            .init_resource::<WellSoundState>()
            .init_resource::<WellMechanismActivity>()
            .init_resource::<WorkSoundState>()
            .init_resource::<ClockSoundState>();

        app.configure_sets(
            Update,
            (
                SoundscapeSet::EmitCues,
                SoundscapeSet::IngestCues,
                SoundscapeSet::ProjectActivity,
            )
                .chain(),
        );

        if !app.is_plugin_added::<AudioPlugin>() {
            return;
        }

        app.add_systems(Startup, load_soundscape_assets)
            .add_systems(
                Update,
                (
                    update_cart_sounds.in_set(SoundscapeSet::EmitCues),
                    ingest_soundscape_cues.in_set(SoundscapeSet::IngestCues),
                    project_well_mechanism_activity.in_set(SoundscapeSet::ProjectActivity),
                    schedule_clock_sounds,
                    schedule_player_footsteps,
                    schedule_npc_body_sounds,
                    spawn_due_sounds,
                    update_virtualized_loops,
                    update_playing_one_shots,
                )
                    .chain(),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipMode {
    OneShot,
    Sequence,
    Loop,
}

#[derive(Debug, Clone, Copy)]
struct SoundDescriptor {
    sound: SoundscapeSound,
    file: &'static str,
    mode: ClipMode,
    gain: f32,
    radius_m: f32,
    /// Multiplier while NPC voice, STT capture, or typed chat owns attention.
    busy_gain: f32,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SoundscapeSound {
    CobbleFootstep,
    WorkshopCough,
    EveningYawn,
    GrainCart,
    CartRut,
    CrateSetDown,
    SackDrop,
    StoneGateClosing,
    RiverGateBarLift,
    WickmarketCrowd,
    BalancePans,
    WaresCall,
    DoughKneading,
    FlourSack,
    EelSmokeFire,
    CinderFurnace,
    CulletSorting,
    Loom,
    FordWindlass,
    FordSplash,
    ThreeCurbRopes,
    ChainWellKnock,
    CrossedBuckets,
}

const ALL_SOUNDS: [SoundscapeSound; 23] = [
    SoundscapeSound::CobbleFootstep,
    SoundscapeSound::WorkshopCough,
    SoundscapeSound::EveningYawn,
    SoundscapeSound::GrainCart,
    SoundscapeSound::CartRut,
    SoundscapeSound::CrateSetDown,
    SoundscapeSound::SackDrop,
    SoundscapeSound::StoneGateClosing,
    SoundscapeSound::RiverGateBarLift,
    SoundscapeSound::WickmarketCrowd,
    SoundscapeSound::BalancePans,
    SoundscapeSound::WaresCall,
    SoundscapeSound::DoughKneading,
    SoundscapeSound::FlourSack,
    SoundscapeSound::EelSmokeFire,
    SoundscapeSound::CinderFurnace,
    SoundscapeSound::CulletSorting,
    SoundscapeSound::Loom,
    SoundscapeSound::FordWindlass,
    SoundscapeSound::FordSplash,
    SoundscapeSound::ThreeCurbRopes,
    SoundscapeSound::ChainWellKnock,
    SoundscapeSound::CrossedBuckets,
];

const SOUND_DESCRIPTORS: [SoundDescriptor; 23] = [
    descriptor(
        SoundscapeSound::CobbleFootstep,
        "snd_001_soft_shoes_on_dry_cobbles.mp3",
        ClipMode::OneShot,
        1.00,
        6.0,
        0.65,
    ),
    descriptor(
        SoundscapeSound::WorkshopCough,
        "snd_034_dusty_workshop_cough.mp3",
        ClipMode::OneShot,
        0.72,
        24.0,
        0.08,
    ),
    descriptor(
        SoundscapeSound::EveningYawn,
        "snd_037_end_of_day_yawn.mp3",
        ClipMode::OneShot,
        0.58,
        22.0,
        0.08,
    ),
    descriptor(
        SoundscapeSound::GrainCart,
        "snd_042_grain_laden_cart_roll.wav",
        ClipMode::Loop,
        0.28,
        55.0,
        0.42,
    ),
    descriptor(
        SoundscapeSound::CartRut,
        "snd_046_wheel_drops_into_rut.mp3",
        ClipMode::OneShot,
        0.34,
        44.0,
        0.45,
    ),
    descriptor(
        SoundscapeSound::CrateSetDown,
        "snd_050_crate_set_down.mp3",
        ClipMode::OneShot,
        0.52,
        34.0,
        0.45,
    ),
    descriptor(
        SoundscapeSound::SackDrop,
        "snd_049_porter_sack_drop.mp3",
        ClipMode::OneShot,
        1.15,
        30.0,
        0.45,
    ),
    descriptor(
        SoundscapeSound::StoneGateClosing,
        "snd_062_stone_gate_closing.mp3",
        ClipMode::Sequence,
        0.28,
        180.0,
        0.55,
    ),
    descriptor(
        SoundscapeSound::RiverGateBarLift,
        "snd_063_river_gate_bar_lift.mp3",
        ClipMode::Sequence,
        0.40,
        95.0,
        0.55,
    ),
    descriptor(
        SoundscapeSound::WickmarketCrowd,
        "snd_081_wickmarket_highmarket_crowd_bed.wav",
        ClipMode::Loop,
        0.22,
        78.0,
        0.12,
    ),
    descriptor(
        SoundscapeSound::BalancePans,
        "snd_087_balance_pans_settle.mp3",
        ClipMode::OneShot,
        0.64,
        24.0,
        0.35,
    ),
    descriptor(
        SoundscapeSound::WaresCall,
        "snd_097_indistinct_wares_call.mp3",
        ClipMode::OneShot,
        0.34,
        35.0,
        0.05,
    ),
    descriptor(
        SoundscapeSound::DoughKneading,
        "snd_125_dough_kneading_on_board.wav",
        ClipMode::Loop,
        0.72,
        20.0,
        0.32,
    ),
    descriptor(
        SoundscapeSound::FlourSack,
        "snd_127_flour_sack_opened.mp3",
        ClipMode::Sequence,
        1.20,
        20.0,
        0.35,
    ),
    descriptor(
        SoundscapeSound::EelSmokeFire,
        "snd_137_eel_smoke_rack_fire.wav",
        ClipMode::Loop,
        0.38,
        27.0,
        0.45,
    ),
    descriptor(
        SoundscapeSound::CinderFurnace,
        "snd_161_cinder_furnace_bed.wav",
        ClipMode::Loop,
        0.22,
        42.0,
        0.42,
    ),
    descriptor(
        SoundscapeSound::CulletSorting,
        "snd_168_cullet_sorting.wav",
        ClipMode::Loop,
        0.15,
        25.0,
        0.32,
    ),
    descriptor(
        SoundscapeSound::Loom,
        "snd_181_loom_shuttle_run.wav",
        ClipMode::Loop,
        0.21,
        24.0,
        0.35,
    ),
    descriptor(
        SoundscapeSound::FordWindlass,
        "snd_201_ford_well_wooden_windlass.wav",
        ClipMode::Loop,
        0.28,
        48.0,
        0.42,
    ),
    descriptor(
        SoundscapeSound::FordSplash,
        "snd_202_ford_well_deep_bucket_splash.mp3",
        ClipMode::OneShot,
        0.27,
        52.0,
        0.45,
    ),
    descriptor(
        SoundscapeSound::ThreeCurbRopes,
        "snd_206_three_curb_triple_rope.wav",
        ClipMode::Loop,
        0.23,
        48.0,
        0.38,
    ),
    descriptor(
        SoundscapeSound::ChainWellKnock,
        "snd_205_chain_well_bucket_knock.mp3",
        ClipMode::OneShot,
        0.52,
        50.0,
        0.45,
    ),
    descriptor(
        SoundscapeSound::CrossedBuckets,
        "snd_207_three_curb_crossed_buckets.mp3",
        ClipMode::OneShot,
        0.20,
        52.0,
        0.45,
    ),
];

const fn descriptor(
    sound: SoundscapeSound,
    file: &'static str,
    mode: ClipMode,
    gain: f32,
    radius_m: f32,
    busy_gain: f32,
) -> SoundDescriptor {
    SoundDescriptor {
        sound,
        file,
        mode,
        gain,
        radius_m,
        busy_gain,
    }
}

impl SoundscapeSound {
    fn descriptor(self) -> &'static SoundDescriptor {
        let descriptor = &SOUND_DESCRIPTORS[self as usize];
        debug_assert_eq!(descriptor.sound, self);
        descriptor
    }

    fn asset_path(self) -> String {
        format!("{SOUND_ROOT}/{}", self.descriptor().file)
    }
}

#[derive(Resource)]
struct SoundscapeAssets(HashMap<SoundscapeSound, Handle<AudioSource>>);

impl SoundscapeAssets {
    fn get(&self, sound: SoundscapeSound) -> Handle<AudioSource> {
        self.0[&sound].clone()
    }
}

fn load_soundscape_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handles = ALL_SOUNDS
        .into_iter()
        .map(|sound| (sound, asset_server.load(sound.asset_path())))
        .collect();
    commands.insert_resource(SoundscapeAssets(handles));
}

#[derive(Debug, Clone, Copy)]
struct ScheduledSound {
    at: f64,
    sound: SoundscapeSound,
    position: Vec3,
    gain_scale: f32,
    speed: f32,
}

#[derive(Resource, Default)]
struct ScheduledSounds(Vec<ScheduledSound>);

impl ScheduledSounds {
    fn push(&mut self, at: f64, sound: SoundscapeSound, position: Vec3) {
        self.push_shaped(at, sound, position, 1.0, 1.0);
    }

    fn push_shaped(
        &mut self,
        at: f64,
        sound: SoundscapeSound,
        position: Vec3,
        gain_scale: f32,
        speed: f32,
    ) {
        if at.is_finite() && position.is_finite() && gain_scale.is_finite() && speed.is_finite() {
            self.0.push(ScheduledSound {
                at,
                sound,
                position,
                gain_scale: gain_scale.clamp(0.0, 4.0),
                speed: speed.clamp(0.5, 2.0),
            });
        }
    }
}

#[derive(Resource, Default)]
struct CueCooldowns {
    last_allowed: HashMap<u64, f64>,
    last_pruned_at: f64,
}

impl CueCooldowns {
    fn allow(&mut self, key: u64, now: f64, seconds: f64) -> bool {
        if now >= self.last_pruned_at
            && now - self.last_pruned_at >= CUE_COOLDOWN_PRUNE_INTERVAL_SECONDS
        {
            self.last_allowed
                .retain(|_, last| now >= *last && now - *last <= CUE_COOLDOWN_RETENTION_SECONDS);
            self.last_pruned_at = now;
        }
        let allowed = self
            .last_allowed
            .get(&key)
            .is_none_or(|last| now - *last >= seconds);
        if allowed {
            self.last_allowed.insert(key, now);
        }
        allowed
    }
}

#[derive(Resource, Default)]
struct WellSoundState {
    ford_until: f64,
    chain_until: f64,
    three_curb_until: f64,
    three_curb_paused_from: f64,
    three_curb_paused_until: f64,
    last_draw_at: HashMap<SpecialWell, f64>,
    crossed_bucket_day: Option<i64>,
}

/// A read-only view of the authored well mechanisms for city animation.
///
/// Soundscape owns the timers so the visuals and audio cannot independently
/// drift into contradictory states. Consumers can inspect this snapshot but
/// cannot mutate its fields.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WellMechanismActivity {
    ford_active: bool,
    chain_active: bool,
    three_curb_active: bool,
    three_curb_conflict: bool,
}

impl WellMechanismActivity {
    pub(crate) fn ford_active(self) -> bool {
        self.ford_active
    }

    pub(crate) fn chain_active(self) -> bool {
        self.chain_active
    }

    pub(crate) fn three_curb_active(self) -> bool {
        self.three_curb_active
    }

    pub(crate) fn three_curb_conflict(self) -> bool {
        self.three_curb_conflict
    }
}

impl WellSoundState {
    fn activity_at(&self, now: f64) -> WellMechanismActivity {
        WellMechanismActivity {
            ford_active: now < self.ford_until,
            chain_active: now < self.chain_until,
            three_curb_active: now < self.three_curb_until,
            three_curb_conflict: now >= self.three_curb_paused_from
                && now < self.three_curb_paused_until,
        }
    }
}

fn three_curb_loop_is_active(wells: &WellSoundState, now: f64) -> bool {
    now < wells.three_curb_until
        && !(now >= wells.three_curb_paused_from && now < wells.three_curb_paused_until)
}

fn project_well_mechanism_activity(
    time: Res<Time>,
    wells: Res<WellSoundState>,
    mut activity: ResMut<WellMechanismActivity>,
) {
    *activity = wells.activity_at(time.elapsed_secs_f64());
}

#[derive(Debug, Clone, Copy)]
struct WorkState {
    position: Vec3,
    active: bool,
}

#[derive(Resource, Default)]
struct WorkSoundState(HashMap<WorkActivityKind, WorkState>);

fn ingest_soundscape_cues(
    mut cues: MessageReader<SoundscapeCue>,
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    mut scheduled: ResMut<ScheduledSounds>,
    mut cooldowns: ResMut<CueCooldowns>,
    mut wells: ResMut<WellSoundState>,
    mut work: ResMut<WorkSoundState>,
) {
    let now = time.elapsed_secs_f64();
    for cue in cues.read().copied() {
        match cue {
            SoundscapeCue::MarketCry { position } => {
                let key = positional_cooldown_key(SoundscapeSound::WaresCall, position, 8.0);
                if cooldowns.allow(key, now, 10.0) {
                    scheduled.push(now, SoundscapeSound::WaresCall, position);
                }
            }
            SoundscapeCue::MarketMeasurement { position } => {
                let key = positional_cooldown_key(SoundscapeSound::BalancePans, position, 4.0);
                if cooldowns.allow(key, now, 0.75) {
                    scheduled.push(now, SoundscapeSound::BalancePans, position);
                }
            }
            SoundscapeCue::CargoHandoff { position, kind } => {
                let (sound, contact_delay) = cargo_handoff_sound(kind);
                let key = positional_cooldown_key(sound, position, 5.0);
                if cooldowns.allow(key, now, 0.8) {
                    let (gain, speed) = cargo_handoff_shape(kind, position, now);
                    scheduled.push_shaped(now + contact_delay, sound, position, gain, speed);
                }
            }
            SoundscapeCue::WellDraw { source } => {
                begin_well_draw(source, now, clock.as_deref(), &mut wells, &mut scheduled);
            }
            SoundscapeCue::WorkActivity {
                kind,
                position,
                active,
            } => {
                let was_active = work.0.get(&kind).is_some_and(|state| state.active);
                work.0.insert(kind, WorkState { position, active });
                if kind == WorkActivityKind::Baking && active && !was_active {
                    scheduled.push(now, SoundscapeSound::FlourSack, position);
                }
            }
            SoundscapeCue::StoneGateClosing => {
                let key = stable_hash("stone_gate_closing");
                if cooldowns.allow(key, now, 30.0) {
                    scheduled.push(now, SoundscapeSound::StoneGateClosing, STONE_GATE);
                    // A delayed, quieter copy carries the stone-throat boom
                    // into the inhabited wall rooms without making it sound
                    // like a second exterior gate.
                    scheduled.push_shaped(
                        now + 0.16,
                        SoundscapeSound::StoneGateClosing,
                        STONE_GATE_HOUSING,
                        0.34,
                        0.985,
                    );
                }
            }
            SoundscapeCue::RiverGateBarLift => {
                let key = stable_hash("river_gate_bar_lift");
                if cooldowns.allow(key, now, 30.0) {
                    scheduled.push(now, SoundscapeSound::RiverGateBarLift, RIVER_GATE);
                }
            }
        }
    }
}

fn begin_well_draw(
    source: SpecialWell,
    now: f64,
    clock: Option<&WorldClockState>,
    wells: &mut WellSoundState,
    scheduled: &mut ScheduledSounds,
) {
    let since_last = wells.last_draw_at.get(&source).map(|last| now - *last);

    // A second authoritative draw while Three-Curb's gear is still moving is
    // actual overlap, not decorative randomness. It is the only state that
    // can produce the rare crossed-bucket collision.
    if source == SpecialWell::ThreeCurb
        && now < wells.three_curb_until
        && since_last.is_some_and(|elapsed| (0.35..5.8).contains(&elapsed))
    {
        wells.last_draw_at.insert(source, now);
        wells.three_curb_until = wells.three_curb_until.max(now + 5.8);
        let day = clock
            .filter(|clock| clock.present)
            .map_or(0, |clock| clock.day);
        if wells.crossed_bucket_day != Some(day) {
            let collision_at = now + CROSSED_BUCKET_DELAY_SECONDS;
            wells.crossed_bucket_day = Some(day);
            wells.three_curb_paused_from = collision_at;
            wells.three_curb_paused_until = collision_at + CROSSED_BUCKET_DURATION_SECONDS + 0.08;
            wells.three_curb_until = wells
                .three_curb_until
                .max(wells.three_curb_paused_until + 0.45);
            scheduled.push(
                collision_at,
                SoundscapeSound::CrossedBuckets,
                THREE_CURB - Vec3::Y * 3.5,
            );
        }
        return;
    }
    if since_last.is_some_and(|elapsed| elapsed < 2.5) {
        return;
    }
    wells.last_draw_at.insert(source, now);
    match source {
        SpecialWell::Ford => {
            wells.ford_until = wells.ford_until.max(now + 5.8);
            // The recording itself contains the descent; this offset starts it
            // after the windlass has audibly released the bucket.
            scheduled.push(
                now + 0.9,
                SoundscapeSound::FordSplash,
                FORD_WELL - Vec3::Y * 5.0,
            );
        }
        SpecialWell::Chain => {
            wells.chain_until = wells
                .chain_until
                .max(now + CHAIN_KNOCK_DELAY_SECONDS + CHAIN_KNOCK_DURATION_SECONDS + 0.12);
            scheduled.push(
                now + CHAIN_KNOCK_DELAY_SECONDS,
                SoundscapeSound::ChainWellKnock,
                CHAIN_WELL,
            );
        }
        SpecialWell::ThreeCurb => {
            wells.three_curb_until = wells.three_curb_until.max(now + 5.8);
        }
    }
}

#[derive(Resource, Default)]
struct ClockSoundState {
    flour_day: Option<i64>,
}

fn schedule_clock_sounds(
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    work: Res<WorkSoundState>,
    mut state: ResMut<ClockSoundState>,
    mut scheduled: ResMut<ScheduledSounds>,
) {
    let Some(clock) = clock.filter(|clock| clock.present) else {
        return;
    };
    if work.0.contains_key(&WorkActivityKind::Baking) {
        return;
    }
    if clock.weekday != Weekday::Bellday
        && matches!(clock.office, Office::Watch | Office::Kindling)
        && state.flour_day != Some(clock.day)
    {
        state.flour_day = Some(clock.day);
        scheduled.push(
            time.elapsed_secs_f64(),
            SoundscapeSound::FlourSack,
            COMMON_OVEN,
        );
    }
}

#[derive(Resource, Default)]
struct FootstepTracker {
    previous_position: Option<Vec3>,
    carried_distance_m: f32,
    left: bool,
    sequence: u64,
    last_step_at: f64,
}

fn schedule_player_footsteps(
    time: Res<Time>,
    cobbles: Option<Res<CobbleRoadNetwork>>,
    player: Query<(&Transform, &PlayerController)>,
    mut tracker: ResMut<FootstepTracker>,
    mut scheduled: ResMut<ScheduledSounds>,
) {
    let Ok((transform, controller)) = player.single() else {
        tracker.previous_position = None;
        tracker.carried_distance_m = 0.0;
        return;
    };
    let position = transform.translation;
    let previous = tracker.previous_position.replace(position);
    let Some(previous) = previous else { return };
    let delta = (position - previous).xz().length();
    let on_cobbles = cobbles.is_some_and(|roads| roads.contains(position.xz()));
    let speed = controller.horizontal_speed();
    if !controller.is_grounded()
        || controller.flying
        || !on_cobbles
        || speed < 0.35
        || delta > TELEPORT_DISTANCE_M
    {
        tracker.carried_distance_m = 0.0;
        return;
    }

    tracker.carried_distance_m += delta;
    let stride = step_spacing(speed);
    let now = time.elapsed_secs_f64();
    // `ResMut` dereferences through Bevy's change-tick guard, so copy the two
    // independent scalar fields out before passing both mutably.
    let mut carried_distance_m = tracker.carried_distance_m;
    let mut last_step_at = tracker.last_step_at;
    let stepped = advance_footstep(&mut carried_distance_m, stride, &mut last_step_at, now);
    tracker.carried_distance_m = carried_distance_m;
    tracker.last_step_at = last_step_at;
    if stepped {
        tracker.left = !tracker.left;
        tracker.sequence = tracker.sequence.wrapping_add(1);
        let foot_pitch = if tracker.left { 0.965 } else { 1.035 };
        let jitter = signed_unit(stable_hash(&format!("step:{}", tracker.sequence))) * 0.018;
        let gain_jitter =
            0.96 + unit(stable_hash(&format!("step-gain:{}", tracker.sequence))) as f32 * 0.08;
        let source = Vec3::new(position.x, 0.08, position.z);
        scheduled.push_shaped(
            now,
            SoundscapeSound::CobbleFootstep,
            source,
            gain_jitter,
            foot_pitch + jitter,
        );
    }
}

fn step_spacing(speed_mps: f32) -> f32 {
    // The controller is intentionally quick (8/12 m/s), while this recording
    // is 0.836 s long. Target ~2.15 contacts/s walking and ~2.55 running so the
    // tails overlap naturally without building a four-voice machine-gun bed.
    let speed = speed_mps.clamp(0.0, 12.0);
    let contacts_per_second = (1.35 + speed * 0.10).clamp(1.55, 2.55);
    (speed / contacts_per_second).clamp(1.45, 4.75)
}

fn advance_footstep(
    carried_distance_m: &mut f32,
    spacing_m: f32,
    last_step_at: &mut f64,
    now: f64,
) -> bool {
    if *carried_distance_m < spacing_m || now - *last_step_at < MIN_STEP_INTERVAL_SECONDS {
        return false;
    }
    // Consume one stride only. A hitch must not repay missed footsteps as a
    // burst, but retaining a bounded remainder preserves cadence afterward.
    *carried_distance_m = (*carried_distance_m - spacing_m).min(spacing_m);
    *last_step_at = now;
    true
}

#[derive(Debug, Default)]
struct NpcTimer {
    next_cough_at: f64,
    yawn_day: Option<i64>,
    yawn_due_at: Option<f64>,
}

#[derive(Resource, Default)]
struct NpcSoundState {
    actors: HashMap<String, NpcTimer>,
    next_global_at: f64,
}

const DUSTY_WORK_ZONES: [(Vec2, f32); 4] = [
    (Vec2::new(255.0, 155.0), 58.0),
    (Vec2::new(-165.875, 439.625), 30.0),
    (Vec2::new(-130.0, 205.0), 68.0),
    (Vec2::new(120.0, 260.0), 105.0),
];

#[allow(clippy::too_many_arguments)]
fn schedule_npc_body_sounds(
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    movement: Option<Res<MovementInbox>>,
    mirror: Option<Res<WorldMirror>>,
    player: Query<&Transform, With<PlayerController>>,
    actors: Query<(&ActorId, &GlobalTransform), With<ActorView>>,
    mut state: ResMut<NpcSoundState>,
    mut scheduled: ResMut<ScheduledSounds>,
) {
    let Ok(player) = player.single() else { return };
    let now = time.elapsed_secs_f64();
    let day = clock
        .as_deref()
        .filter(|clock| clock.present)
        .map_or(0, |clock| clock.day);
    let office = clock
        .as_deref()
        .filter(|clock| clock.present)
        .map(|clock| clock.office);
    let weekday = clock
        .as_deref()
        .filter(|clock| clock.present)
        .map(|clock| clock.weekday);
    // Retain timers for every still-present actor, including somebody who
    // briefly walks or leaves earshot. Otherwise returning to the radius would
    // reset `yawn_day` and permit several "once per day" yawns.
    let seen: HashSet<String> = actors.iter().map(|(id, _)| id.0.clone()).collect();
    let mut candidates: Vec<_> = actors
        .iter()
        .filter_map(|(id, transform)| {
            let position = transform.translation();
            let close = position.distance_squared(player.translation) <= 70.0_f32.powi(2);
            let stationary = movement
                .as_deref()
                .and_then(|inbox| inbox.0.get(id))
                .is_none_or(|sample| sample.speed < 0.35);
            let snapshot = mirror.as_deref().and_then(|mirror| mirror.actor(id));
            let dusty_worker =
                snapshot.is_some_and(|actor| dusty_worker_outfit(actor.appearance.outfit));
            let weariness = snapshot.map_or(0.0, |actor| status_weariness(&actor.statuses));
            (close && stationary).then_some((id, position, dusty_worker, weariness))
        })
        .collect();
    candidates.sort_by(|(a, _, _, _), (b, _, _, _)| a.0.cmp(&b.0));

    for (id, _, _, _) in &candidates {
        state
            .actors
            .entry(id.0.clone())
            .or_insert_with(|| NpcTimer {
                next_cough_at: now
                    + 45.0
                    + unit(stable_hash(&format!("cough:{}:{day}", id.0))) * 150.0,
                ..default()
            });
    }

    if now >= state.next_global_at {
        let cough_hours = office.is_none_or(|office| {
            matches!(
                office,
                Office::Kindling | Office::Dayspring | Office::HighWick | Office::Waning
            )
        }) && weekday != Some(Weekday::Bellday);
        if cough_hours {
            for (id, position, dusty_worker, _) in &candidates {
                if !dusty_worker {
                    continue;
                }
                let in_dust = DUSTY_WORK_ZONES.iter().any(|(center, radius)| {
                    position.xz().distance_squared(*center) <= radius.powi(2)
                });
                let timer = state
                    .actors
                    .get_mut(&id.0)
                    .expect("candidate timer was initialized above");
                if in_dust && now >= timer.next_cough_at {
                    scheduled.push(
                        now,
                        SoundscapeSound::WorkshopCough,
                        *position + Vec3::Y * 1.35,
                    );
                    let cycle = stable_hash(&format!("cough-next:{}:{day}:{}", id.0, now.floor()));
                    timer.next_cough_at = now + 210.0 + unit(cycle) * 240.0;
                    state.next_global_at = now + NPC_BODY_SOUND_GLOBAL_COOLDOWN_SECONDS;
                    break;
                }
            }
        }
    }

    if now >= state.next_global_at && matches!(office, Some(Office::Lamplight | Office::Snuffing)) {
        let mut yawn_candidates: Vec<_> = candidates.iter().collect();
        yawn_candidates.sort_by(|(a_id, _, _, a_weariness), (b_id, _, _, b_weariness)| {
            b_weariness
                .total_cmp(a_weariness)
                .then_with(|| a_id.0.cmp(&b_id.0))
        });
        for (id, position, _, weariness) in yawn_candidates {
            let timer = state
                .actors
                .get_mut(&id.0)
                .expect("candidate timer was initialized above");
            if timer.yawn_day == Some(day) {
                continue;
            }
            let due = *timer.yawn_due_at.get_or_insert_with(|| {
                let deterministic_delay =
                    20.0 + unit(stable_hash(&format!("yawn:{}:{day}", id.0))) * 130.0;
                now + deterministic_delay * (1.0 - 0.55 * f64::from(*weariness))
            });
            if now >= due {
                scheduled.push(
                    now,
                    SoundscapeSound::EveningYawn,
                    *position + Vec3::Y * 1.35,
                );
                timer.yawn_day = Some(day);
                timer.yawn_due_at = None;
                state.next_global_at = now + NPC_BODY_SOUND_GLOBAL_COOLDOWN_SECONDS;
                break;
            }
        }
    }
    state.actors.retain(|id, _| seen.contains(id));
}

fn status_weariness(statuses: &[(cathedral_sim::StatusKind, f32)]) -> f32 {
    statuses
        .iter()
        .find_map(|(kind, value)| (*kind == cathedral_sim::StatusKind::Weariness).then_some(*value))
        .unwrap_or(0.0)
        .clamp(0.0, 1.0)
}

fn dusty_worker_outfit(outfit: cathedral_sim::OutfitClass) -> bool {
    matches!(
        outfit,
        cathedral_sim::OutfitClass::Craftsman | cathedral_sim::OutfitClass::Laborer
    )
}

const AUTHORED_RUTS: [Vec2; 5] = [
    Vec2::new(0.0, 202.0),
    Vec2::new(-304.0, 90.0),
    Vec2::new(475.0, 136.0),
    Vec2::new(-479.0, -134.0),
    Vec2::new(360.0, 335.0),
];

#[derive(Debug)]
struct CartTrack {
    position: Vec3,
    speed_mps: f32,
    loads: Vec<CartLoadKind>,
    rut_last_at: [f64; AUTHORED_RUTS.len()],
    last_seen_at: f64,
}

#[derive(Resource, Default)]
struct CartSoundState(HashMap<String, CartTrack>);

fn update_cart_sounds(
    time: Res<Time>,
    mirror: Option<Res<WorldMirror>>,
    carts: Query<(&RoadCartView, &GlobalTransform)>,
    mut state: ResMut<CartSoundState>,
    mut scheduled: ResMut<ScheduledSounds>,
    mut cues: MessageWriter<SoundscapeCue>,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs().max(1.0 / 240.0);
    let mirrored_loads: HashMap<&str, &[CartLoadKind]> = mirror
        .as_deref()
        .map(|mirror| {
            mirror
                .road_carts()
                .map(|cart| (cart.party_id.as_str(), cart.load.as_slice()))
                .collect()
        })
        .unwrap_or_default();

    for (view, transform) in &carts {
        let position = transform.translation();
        let loads = mirrored_loads
            .get(view.party_id.as_str())
            .copied()
            .unwrap_or_else(|| view.load())
            .to_vec();
        let Some(track) = state.0.get_mut(&view.party_id) else {
            state.0.insert(
                view.party_id.clone(),
                CartTrack {
                    position,
                    speed_mps: 0.0,
                    loads,
                    rut_last_at: [f64::NEG_INFINITY; AUTHORED_RUTS.len()],
                    last_seen_at: now,
                },
            );
            continue;
        };

        let previous = track.position;
        let travelled = (position - previous).xz().length();
        track.speed_mps = if travelled <= TELEPORT_DISTANCE_M {
            travelled / dt
        } else {
            0.0
        };
        track.position = position;
        track.last_seen_at = now;

        let lost_grain = track.loads.contains(&CartLoadKind::GrainSacks)
            && !loads.contains(&CartLoadKind::GrainSacks);
        let lost_other = track
            .loads
            .iter()
            .any(|load| matches!(load, CartLoadKind::WoolBales | CartLoadKind::ClothBolts))
            && !loads
                .iter()
                .any(|load| matches!(load, CartLoadKind::WoolBales | CartLoadKind::ClothBolts));
        if lost_grain {
            cues.write(SoundscapeCue::CargoHandoff {
                position: position + Vec3::Y * 0.3,
                kind: CargoHandoffKind::GrainSack,
            });
        }
        if lost_other {
            cues.write(SoundscapeCue::CargoHandoff {
                position: position + Vec3::Y * 0.3,
                kind: CargoHandoffKind::Crate,
            });
        }
        track.loads = loads;

        if travelled > 0.05 && travelled <= TELEPORT_DISTANCE_M {
            for (index, rut) in AUTHORED_RUTS.iter().enumerate() {
                if now - track.rut_last_at[index] >= 25.0
                    && segment_crosses_rut(previous.xz(), position.xz(), *rut, 2.4)
                {
                    track.rut_last_at[index] = now;
                    scheduled.push(now, SoundscapeSound::CartRut, Vec3::new(rut.x, 0.35, rut.y));
                }
            }
        }
    }
    // Reconciliation intentionally has a one-frame despawn/respawn on load
    // changes. Keep state through that gap so the handoff diff is not lost.
    state.0.retain(|_, track| now - track.last_seen_at <= 5.0);
}

fn segment_crosses_rut(from: Vec2, to: Vec2, rut: Vec2, radius: f32) -> bool {
    let segment = to - from;
    let length_sq = segment.length_squared();
    let t = if length_sq <= f32::EPSILON {
        0.0
    } else {
        ((rut - from).dot(segment) / length_sq).clamp(0.0, 1.0)
    };
    (from + segment * t).distance_squared(rut) <= radius * radius
}

#[derive(Component)]
struct PlayingSoundscapeOneShot {
    spawned_at: f64,
    base_gain: f32,
    busy_gain: f32,
    current_gain: f32,
}

fn spawn_due_sounds(
    mut commands: Commands,
    time: Res<Time>,
    activity: Option<Res<AudioActivity>>,
    assets: Res<SoundscapeAssets>,
    mut scheduled: ResMut<ScheduledSounds>,
) {
    let now = time.elapsed_secs_f64();
    let busy = activity.as_deref().is_some_and(|activity| activity.busy);
    let mut due = Vec::new();
    scheduled.0.retain(|sound| {
        if sound.at <= now {
            due.push(*sound);
            false
        } else {
            true
        }
    });
    for scheduled in due {
        let descriptor = scheduled.sound.descriptor();
        debug_assert_ne!(descriptor.mode, ClipMode::Loop);
        if descriptor.mode == ClipMode::Loop {
            continue;
        }
        let base_gain = descriptor.gain * scheduled.gain_scale;
        let initial_gain = base_gain * if busy { descriptor.busy_gain } else { 1.0 };
        commands.spawn((
            Name::new(format!("Soundscape: {}", descriptor.file)),
            PlayingSoundscapeOneShot {
                spawned_at: now,
                base_gain,
                busy_gain: descriptor.busy_gain,
                current_gain: initial_gain,
            },
            AudioPlayer::new(assets.get(scheduled.sound)),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(initial_gain))
                .with_speed(scheduled.speed)
                .with_spatial(true)
                .with_spatial_scale(spatial_scale(descriptor.radius_m)),
            Transform::from_translation(scheduled.position),
        ));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmitterSchedule {
    WickmarketHighmarket,
    Bakehouse,
    MarenLowmarket,
    LoomWork,
    WorkingDay,
}

#[derive(Debug, Clone, Copy)]
struct StaticEmitter {
    key: u64,
    sound: SoundscapeSound,
    position: Vec3,
    schedule: EmitterSchedule,
    priority: u8,
}

const STATIC_EMITTERS: [StaticEmitter; 12] = [
    StaticEmitter {
        key: 1,
        sound: SoundscapeSound::WickmarketCrowd,
        position: WICKMARKET,
        schedule: EmitterSchedule::WickmarketHighmarket,
        priority: 95,
    },
    StaticEmitter {
        key: 2,
        sound: SoundscapeSound::DoughKneading,
        position: COMMON_OVEN,
        schedule: EmitterSchedule::Bakehouse,
        priority: 76,
    },
    StaticEmitter {
        key: 3,
        sound: SoundscapeSound::EelSmokeFire,
        position: Vec3::new(-257.894, 0.7, -364.732),
        schedule: EmitterSchedule::MarenLowmarket,
        priority: 55,
    },
    StaticEmitter {
        key: 4,
        sound: SoundscapeSound::EelSmokeFire,
        position: Vec3::new(-349.648, 0.7, -356.714),
        schedule: EmitterSchedule::MarenLowmarket,
        priority: 55,
    },
    StaticEmitter {
        key: 5,
        sound: SoundscapeSound::EelSmokeFire,
        position: Vec3::new(-270.444, 0.7, -353.739),
        schedule: EmitterSchedule::MarenLowmarket,
        priority: 55,
    },
    StaticEmitter {
        key: 6,
        sound: SoundscapeSound::EelSmokeFire,
        position: Vec3::new(-323.471, 0.7, -336.611),
        schedule: EmitterSchedule::MarenLowmarket,
        priority: 55,
    },
    StaticEmitter {
        key: 7,
        sound: SoundscapeSound::EelSmokeFire,
        position: Vec3::new(-262.372, 0.7, -365.300),
        schedule: EmitterSchedule::MarenLowmarket,
        priority: 55,
    },
    StaticEmitter {
        key: 8,
        sound: SoundscapeSound::CinderFurnace,
        position: CINDER_ROW,
        schedule: EmitterSchedule::WorkingDay,
        priority: 70,
    },
    StaticEmitter {
        key: 10,
        sound: SoundscapeSound::CulletSorting,
        position: BURNT_COURT,
        schedule: EmitterSchedule::WorkingDay,
        priority: 58,
    },
    StaticEmitter {
        key: 11,
        sound: SoundscapeSound::Loom,
        position: Vec3::new(48.0, 1.5, 334.0),
        schedule: EmitterSchedule::LoomWork,
        priority: 62,
    },
    StaticEmitter {
        key: 12,
        sound: SoundscapeSound::Loom,
        position: Vec3::new(111.0, 1.5, 278.0),
        schedule: EmitterSchedule::LoomWork,
        priority: 62,
    },
    StaticEmitter {
        key: 13,
        sound: SoundscapeSound::Loom,
        position: Vec3::new(175.0, 1.5, 218.0),
        schedule: EmitterSchedule::LoomWork,
        priority: 62,
    },
];

fn schedule_is_active(schedule: EmitterSchedule, clock: Option<&WorldClockState>) -> bool {
    let Some(clock) = clock.filter(|clock| clock.present) else {
        return !matches!(
            schedule,
            EmitterSchedule::WickmarketHighmarket | EmitterSchedule::MarenLowmarket
        );
    };
    match schedule {
        EmitterSchedule::WickmarketHighmarket => {
            clock.weekday == Weekday::Highmarket
                && matches!(
                    clock.office,
                    Office::Dayspring | Office::HighWick | Office::Waning
                )
        }
        EmitterSchedule::Bakehouse => {
            clock.weekday != Weekday::Bellday
                && matches!(clock.office, Office::Watch | Office::Kindling)
        }
        EmitterSchedule::MarenLowmarket => {
            clock.weekday == Weekday::Lowmarket
                && matches!(
                    clock.office,
                    Office::Dayspring | Office::HighWick | Office::Waning
                )
        }
        EmitterSchedule::LoomWork => {
            clock.weekday != Weekday::Bellday
                && matches!(clock.office, Office::HighWick | Office::Waning)
        }
        EmitterSchedule::WorkingDay => {
            clock.weekday != Weekday::Bellday
                && matches!(
                    clock.office,
                    Office::Kindling | Office::Dayspring | Office::HighWick | Office::Waning
                )
        }
    }
}

fn work_kind_for_sound(sound: SoundscapeSound) -> Option<WorkActivityKind> {
    match sound {
        SoundscapeSound::DoughKneading => Some(WorkActivityKind::Baking),
        SoundscapeSound::EelSmokeFire => Some(WorkActivityKind::EelSmoking),
        SoundscapeSound::CinderFurnace => Some(WorkActivityKind::GlassFurnace),
        SoundscapeSound::CulletSorting => Some(WorkActivityKind::CulletSorting),
        SoundscapeSound::Loom => Some(WorkActivityKind::Weaving),
        _ => None,
    }
}

fn sound_for_work_kind(kind: WorkActivityKind) -> SoundscapeSound {
    match kind {
        WorkActivityKind::Baking => SoundscapeSound::DoughKneading,
        WorkActivityKind::EelSmoking => SoundscapeSound::EelSmokeFire,
        WorkActivityKind::GlassFurnace => SoundscapeSound::CinderFurnace,
        WorkActivityKind::CulletSorting => SoundscapeSound::CulletSorting,
        WorkActivityKind::Weaving => SoundscapeSound::Loom,
    }
}

#[derive(Debug, Clone)]
struct LoopDemand {
    key: u64,
    sound: SoundscapeSound,
    position: Vec3,
    gain: f32,
    radius_m: f32,
    speed: f32,
    priority: u8,
}

#[derive(Component)]
struct PlayingSoundscapeLoop {
    key: u64,
    spawned_at: f64,
    current_gain: f32,
    base_gain: f32,
    busy_gain: f32,
    speed: f32,
}

#[allow(clippy::too_many_arguments)]
fn update_virtualized_loops(
    mut commands: Commands,
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    activity: Option<Res<AudioActivity>>,
    assets: Res<SoundscapeAssets>,
    player: Query<&Transform, With<PlayerController>>,
    cart_views: Query<(&RoadCartView, &GlobalTransform)>,
    carts: Res<CartSoundState>,
    wells: Res<WellSoundState>,
    work: Res<WorkSoundState>,
    mut playing: Query<
        (
            Entity,
            &mut PlayingSoundscapeLoop,
            &mut Transform,
            Option<&mut SpatialAudioSink>,
        ),
        Without<PlayerController>,
    >,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    let busy = activity.as_deref().is_some_and(|activity| activity.busy);
    let player_position = player.single().ok().map(|transform| transform.translation);
    let existing_keys: HashSet<u64> = playing.iter().map(|(_, state, _, _)| state.key).collect();
    let mut demands = Vec::new();

    for emitter in STATIC_EMITTERS {
        let overridden =
            work_kind_for_sound(emitter.sound).is_some_and(|kind| work.0.contains_key(&kind));
        if !overridden && schedule_is_active(emitter.schedule, clock.as_deref()) {
            let descriptor = emitter.sound.descriptor();
            demands.push(LoopDemand {
                key: emitter.key,
                sound: emitter.sound,
                position: emitter.position,
                gain: descriptor.gain,
                radius_m: descriptor.radius_m,
                speed: 1.0,
                priority: emitter.priority,
            });
        }
    }
    for (kind, state) in &work.0 {
        if state.active {
            let sound = sound_for_work_kind(*kind);
            let descriptor = sound.descriptor();
            demands.push(LoopDemand {
                key: 1_000 + *kind as u64,
                sound,
                position: state.position,
                gain: descriptor.gain,
                radius_m: descriptor.radius_m,
                speed: 1.0,
                priority: 88,
            });
        }
    }
    if now < wells.ford_until {
        let descriptor = SoundscapeSound::FordWindlass.descriptor();
        demands.push(LoopDemand {
            key: 2_001,
            sound: SoundscapeSound::FordWindlass,
            position: FORD_WELL,
            gain: descriptor.gain,
            radius_m: descriptor.radius_m,
            speed: 1.0,
            priority: 92,
        });
    }
    if three_curb_loop_is_active(&wells, now) {
        let descriptor = SoundscapeSound::ThreeCurbRopes.descriptor();
        demands.push(LoopDemand {
            key: 2_002,
            sound: SoundscapeSound::ThreeCurbRopes,
            position: THREE_CURB,
            gain: descriptor.gain,
            radius_m: descriptor.radius_m,
            speed: 1.0,
            priority: 92,
        });
    }
    for (view, transform) in &cart_views {
        let Some(cart) = carts.0.get(&view.party_id) else {
            continue;
        };
        if !cart.loads.contains(&CartLoadKind::GrainSacks) || cart.speed_mps < 0.3 {
            continue;
        }
        let descriptor = SoundscapeSound::GrainCart.descriptor();
        let motion = (cart.speed_mps / 8.0).clamp(0.0, 1.0);
        demands.push(LoopDemand {
            key: 3_000_000 | (stable_hash(&view.party_id) & 0x000f_ffff),
            sound: SoundscapeSound::GrainCart,
            position: transform.translation(),
            gain: descriptor.gain * (0.72 + motion * 0.38),
            radius_m: descriptor.radius_m,
            speed: 0.82 + motion * 0.30,
            priority: 86,
        });
    }

    let mut selected = select_loop_demands(demands, player_position, &existing_keys);
    let selected_keys: HashSet<u64> = selected.iter().map(|demand| demand.key).collect();
    let mut represented = HashSet::new();
    for (entity, mut state, mut transform, sink) in &mut playing {
        if !represented.insert(state.key) {
            commands.entity(entity).despawn();
            continue;
        }
        let desired = selected.iter().find(|demand| demand.key == state.key);
        if let Some(desired) = desired {
            state.base_gain = desired.gain;
            state.speed = desired.speed;
            transform.translation = desired.position;
        }
        let target = if selected_keys.contains(&state.key) {
            state.base_gain * if busy { state.busy_gain } else { 1.0 }
        } else {
            0.0
        };
        state.current_gain =
            smooth_gain(state.current_gain, target, dt, target < state.current_gain);
        let has_sink = sink.is_some();
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(state.current_gain));
            sink.set_speed(state.speed);
        }
        if (!has_sink && now - state.spawned_at > STALLED_AUDIO_TIMEOUT_SECONDS)
            || (target <= 0.0 && state.current_gain <= 0.001 && now - state.spawned_at > 0.25)
        {
            commands.entity(entity).despawn();
        }
    }

    for demand in selected.drain(..) {
        if represented.contains(&demand.key) {
            continue;
        }
        let descriptor = demand.sound.descriptor();
        commands.spawn((
            Name::new(format!("Soundscape loop: {}", descriptor.file)),
            PlayingSoundscapeLoop {
                key: demand.key,
                spawned_at: now,
                current_gain: 0.0,
                base_gain: demand.gain,
                busy_gain: descriptor.busy_gain,
                speed: demand.speed,
            },
            AudioPlayer::new(assets.get(demand.sound)),
            PlaybackSettings::LOOP
                .with_volume(Volume::Linear(0.0))
                .with_speed(demand.speed)
                .with_spatial(true)
                .with_spatial_scale(spatial_scale(demand.radius_m)),
            Transform::from_translation(demand.position),
        ));
    }
}

fn select_loop_demands(
    demands: Vec<LoopDemand>,
    listener: Option<Vec3>,
    existing: &HashSet<u64>,
) -> Vec<LoopDemand> {
    let Some(listener) = listener else {
        return Vec::new();
    };
    let mut by_key = HashMap::<u64, LoopDemand>::new();
    for demand in demands {
        let limit = demand.radius_m
            * if existing.contains(&demand.key) {
                LOOP_ACTIVATION_HYSTERESIS
            } else {
                1.0
            };
        if demand.position.distance_squared(listener) > limit * limit {
            continue;
        }
        by_key
            .entry(demand.key)
            .and_modify(|old| {
                if demand.priority > old.priority {
                    *old = demand.clone();
                }
            })
            .or_insert(demand);
    }
    let mut demands: Vec<_> = by_key.into_values().collect();
    demands.sort_by(|a, b| {
        b.priority.cmp(&a.priority).then_with(|| {
            let a_distance = a.position.distance_squared(listener)
                - if existing.contains(&a.key) { 64.0 } else { 0.0 };
            let b_distance = b.position.distance_squared(listener)
                - if existing.contains(&b.key) { 64.0 } else { 0.0 };
            a_distance.total_cmp(&b_distance)
        })
    });
    let mut per_sound = HashMap::<SoundscapeSound, usize>::new();
    demands
        .into_iter()
        .filter(|demand| {
            if per_sound.get(&demand.sound).copied().unwrap_or(0) >= loop_cap(demand.sound) {
                return false;
            }
            *per_sound.entry(demand.sound).or_default() += 1;
            true
        })
        .take(MAX_LIVE_LOOPS)
        .collect()
}

fn loop_cap(sound: SoundscapeSound) -> usize {
    match sound {
        SoundscapeSound::EelSmokeFire | SoundscapeSound::Loom | SoundscapeSound::GrainCart => 2,
        _ => 1,
    }
}

fn update_playing_one_shots(
    mut commands: Commands,
    time: Res<Time>,
    activity: Option<Res<AudioActivity>>,
    mut sounds: Query<(
        Entity,
        &mut PlayingSoundscapeOneShot,
        Option<&mut SpatialAudioSink>,
    )>,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    let busy = activity.as_deref().is_some_and(|activity| activity.busy);
    for (entity, mut sound, sink) in &mut sounds {
        let target = sound.base_gain * if busy { sound.busy_gain } else { 1.0 };
        sound.current_gain =
            smooth_gain(sound.current_gain, target, dt, target < sound.current_gain);
        let has_sink = sink.is_some();
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(sound.current_gain));
        }
        if !has_sink && now - sound.spawned_at > STALLED_AUDIO_TIMEOUT_SECONDS {
            commands.entity(entity).despawn();
        }
    }
}

fn smooth_gain(current: f32, target: f32, dt: f32, attacking_duck: bool) -> f32 {
    let seconds = if attacking_duck { 0.10 } else { 0.55 };
    let alpha = 1.0 - (-dt.max(0.0) / seconds).exp();
    current + (target - current) * alpha
}

fn spatial_scale(radius_m: f32) -> SpatialScale {
    SpatialScale::new(1.0 / (radius_m.max(1.0) * SPATIAL_FULL_VOLUME_FRACTION))
}

fn positional_cooldown_key(sound: SoundscapeSound, position: Vec3, cell_m: f32) -> u64 {
    let x = (position.x / cell_m).round() as i32;
    let z = (position.z / cell_m).round() as i32;
    stable_hash(&format!("{}:{x}:{z}", sound as u8))
}

fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn unit(hash: u64) -> f64 {
    (hash >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
}

fn signed_unit(hash: u64) -> f32 {
    (unit(hash) as f32 * 2.0) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_twenty_three_routes_have_unique_assets_and_the_right_container() {
        assert_eq!(ALL_SOUNDS.len(), 23);
        let mut files = HashSet::new();
        for (index, sound) in ALL_SOUNDS.into_iter().enumerate() {
            let descriptor = sound.descriptor();
            assert_eq!(descriptor.sound, sound);
            assert_eq!(sound as usize, index);
            assert!(
                files.insert(descriptor.file),
                "duplicate {}",
                descriptor.file
            );
            match descriptor.mode {
                ClipMode::Loop => assert!(descriptor.file.ends_with(".wav")),
                ClipMode::OneShot | ClipMode::Sequence => {
                    assert!(descriptor.file.ends_with(".mp3"))
                }
            }
            assert!(descriptor.gain > 0.0 && descriptor.gain <= 1.2);
            assert!(descriptor.radius_m >= 6.0);
            assert!((0.0..=1.0).contains(&descriptor.busy_gain));
        }
    }

    #[test]
    fn every_descriptor_points_at_a_nonempty_reviewed_asset() {
        let assets: [&[u8]; 23] = [
            include_bytes!("../assets/sounds/soundscape/snd_001_soft_shoes_on_dry_cobbles.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_034_dusty_workshop_cough.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_037_end_of_day_yawn.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_042_grain_laden_cart_roll.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_046_wheel_drops_into_rut.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_050_crate_set_down.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_049_porter_sack_drop.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_062_stone_gate_closing.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_063_river_gate_bar_lift.mp3"),
            include_bytes!(
                "../assets/sounds/soundscape/snd_081_wickmarket_highmarket_crowd_bed.wav"
            ),
            include_bytes!("../assets/sounds/soundscape/snd_087_balance_pans_settle.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_097_indistinct_wares_call.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_125_dough_kneading_on_board.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_127_flour_sack_opened.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_137_eel_smoke_rack_fire.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_161_cinder_furnace_bed.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_168_cullet_sorting.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_181_loom_shuttle_run.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_201_ford_well_wooden_windlass.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_202_ford_well_deep_bucket_splash.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_206_three_curb_triple_rope.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_205_chain_well_bucket_knock.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_207_three_curb_crossed_buckets.mp3"),
        ];
        for (sound, bytes) in ALL_SOUNDS.into_iter().zip(assets) {
            assert!(
                bytes.len() > 10_000,
                "{} is unexpectedly small",
                sound.descriptor().file
            );
        }
    }

    #[test]
    fn implementation_manifest_contains_the_twenty_three_runtime_routes() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../features/more_sounds/sounds_to_implement.json"
        ))
        .expect("sound implementation manifest is valid JSON");
        let routed_ids: HashSet<_> = ALL_SOUNDS
            .into_iter()
            .map(|sound| {
                sound
                    .descriptor()
                    .file
                    .rsplit_once('.')
                    .expect("reviewed asset has an extension")
                    .0
            })
            .collect();
        let sounds = manifest["sounds"]
            .as_array()
            .expect("manifest sounds is an array");
        let manifested_ids: HashSet<_> = sounds
            .iter()
            .filter_map(|sound| {
                let id = sound["id"].as_str()?;
                if !routed_ids.contains(id) {
                    return None;
                }
                assert_eq!(sound["implemented_in_game"], true);
                assert_eq!(sound["generated_audio"]["sound_id"], id);
                assert_eq!(
                    sound["generated_audio"]["filename"],
                    format!(
                        "{}.{}",
                        id,
                        if sound["playback_mode"] == "loop" {
                            "wav"
                        } else {
                            "mp3"
                        }
                    )
                );
                Some(id)
            })
            .collect();
        assert_eq!(manifested_ids, routed_ids);

        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../features/more_sounds/more_sounds.json"))
                .expect("sound catalog is valid JSON");
        let catalog_implemented: HashSet<_> = catalog["sounds"]
            .as_array()
            .expect("catalog sounds is an array")
            .iter()
            .filter(|sound| sound["implemented_in_game"] == true)
            .filter_map(|sound| sound["id"].as_str())
            .collect();
        assert!(routed_ids.is_subset(&catalog_implemented));
    }

    fn clock(office: Office, weekday: Weekday) -> WorldClockState {
        WorldClockState {
            present: true,
            office,
            weekday,
            ..default()
        }
    }

    #[test]
    fn market_and_work_schedules_follow_office_and_weekday() {
        assert!(schedule_is_active(
            EmitterSchedule::WickmarketHighmarket,
            Some(&clock(Office::HighWick, Weekday::Highmarket))
        ));
        assert!(!schedule_is_active(
            EmitterSchedule::WickmarketHighmarket,
            Some(&clock(Office::HighWick, Weekday::Lowmarket))
        ));
        assert!(!schedule_is_active(
            EmitterSchedule::WickmarketHighmarket,
            Some(&clock(Office::Lamplight, Weekday::Highmarket))
        ));
        assert!(schedule_is_active(
            EmitterSchedule::Bakehouse,
            Some(&clock(Office::Watch, Weekday::Second))
        ));
        assert!(!schedule_is_active(
            EmitterSchedule::WorkingDay,
            Some(&clock(Office::Dayspring, Weekday::Bellday))
        ));
        assert!(schedule_is_active(
            EmitterSchedule::MarenLowmarket,
            Some(&clock(Office::Waning, Weekday::Lowmarket))
        ));
        assert!(!schedule_is_active(
            EmitterSchedule::MarenLowmarket,
            Some(&clock(Office::Waning, Weekday::Highmarket))
        ));
        assert!(!schedule_is_active(EmitterSchedule::MarenLowmarket, None));
        assert!(schedule_is_active(
            EmitterSchedule::LoomWork,
            Some(&clock(Office::HighWick, Weekday::Second))
        ));
        assert!(!schedule_is_active(
            EmitterSchedule::LoomWork,
            Some(&clock(Office::Dayspring, Weekday::Second))
        ));

        let furnaces: Vec<_> = STATIC_EMITTERS
            .iter()
            .filter(|emitter| emitter.sound == SoundscapeSound::CinderFurnace)
            .collect();
        assert_eq!(furnaces.len(), 1);
        assert_eq!(furnaces[0].position, CINDER_ROW);
    }

    #[test]
    fn special_well_classification_is_nearest_and_bounded() {
        assert_eq!(
            classify_special_well(FORD_WELL + Vec3::X * 3.0),
            Some(SpecialWell::Ford)
        );
        assert_eq!(classify_special_well(CHAIN_WELL), Some(SpecialWell::Chain));
        assert_eq!(
            classify_special_well(THREE_CURB + Vec3::Z * 4.0),
            Some(SpecialWell::ThreeCurb)
        );
        assert_eq!(classify_special_well(Vec3::ZERO), None);
    }

    #[test]
    fn well_mechanism_snapshot_observes_timer_boundaries_and_conflict_window() {
        let wells = WellSoundState {
            ford_until: 10.0,
            chain_until: 9.0,
            three_curb_until: 12.0,
            three_curb_paused_from: 7.5,
            three_curb_paused_until: 8.5,
            ..default()
        };

        let before_conflict = wells.activity_at(7.0);
        assert!(before_conflict.ford_active());
        assert!(before_conflict.chain_active());
        assert!(before_conflict.three_curb_active());
        assert!(!before_conflict.three_curb_conflict());
        assert!(three_curb_loop_is_active(&wells, 7.0));

        let during_conflict = wells.activity_at(8.0);
        assert!(during_conflict.three_curb_active());
        assert!(during_conflict.three_curb_conflict());
        assert!(!three_curb_loop_is_active(&wells, 8.0));

        let after_conflict = wells.activity_at(9.5);
        assert!(after_conflict.ford_active());
        assert!(!after_conflict.chain_active());
        assert!(after_conflict.three_curb_active());
        assert!(!after_conflict.three_curb_conflict());
        assert!(three_curb_loop_is_active(&wells, 9.5));

        let only_three_curb = wells.activity_at(10.5);
        assert!(!only_three_curb.ford_active());
        assert!(only_three_curb.three_curb_active());
        assert!(!wells.activity_at(12.0).three_curb_active());
    }

    #[test]
    fn well_audio_windows_cover_clips_and_conflicts_require_overlapping_draws() {
        let mut wells = WellSoundState::default();
        let mut scheduled = ScheduledSounds::default();
        let clock = WorldClockState {
            present: true,
            day: 4,
            ..default()
        };

        begin_well_draw(
            SpecialWell::Chain,
            2.0,
            Some(&clock),
            &mut wells,
            &mut scheduled,
        );
        let chain_end = 2.0 + CHAIN_KNOCK_DELAY_SECONDS + CHAIN_KNOCK_DURATION_SECONDS;
        assert!(wells.chain_until > chain_end);

        begin_well_draw(
            SpecialWell::ThreeCurb,
            10.0,
            Some(&clock),
            &mut wells,
            &mut scheduled,
        );
        assert!(!wells.activity_at(10.5).three_curb_conflict());
        assert!(
            scheduled
                .0
                .iter()
                .all(|sound| sound.sound != SoundscapeSound::CrossedBuckets)
        );

        begin_well_draw(
            SpecialWell::ThreeCurb,
            11.0,
            Some(&clock),
            &mut wells,
            &mut scheduled,
        );
        let collision_at = 11.0 + CROSSED_BUCKET_DELAY_SECONDS;
        let collision_end = collision_at + CROSSED_BUCKET_DURATION_SECONDS;
        assert!(wells.three_curb_paused_until > collision_end);
        assert_eq!(
            scheduled
                .0
                .iter()
                .filter(|sound| sound.sound == SoundscapeSound::CrossedBuckets)
                .count(),
            1
        );

        // A third draw on the same day extends activity but cannot manufacture
        // a second supposedly rare collision.
        begin_well_draw(
            SpecialWell::ThreeCurb,
            12.0,
            Some(&clock),
            &mut wells,
            &mut scheduled,
        );
        assert_eq!(
            scheduled
                .0
                .iter()
                .filter(|sound| sound.sound == SoundscapeSound::CrossedBuckets)
                .count(),
            1
        );
    }

    #[test]
    fn cargo_impacts_match_the_visible_prop_contact_delays() {
        assert_eq!(
            cargo_handoff_sound(CargoHandoffKind::GrainSack),
            (SoundscapeSound::SackDrop, 0.20)
        );
        assert_eq!(
            cargo_handoff_sound(CargoHandoffKind::Crate),
            (SoundscapeSound::CrateSetDown, 0.48)
        );

        let sack_a = cargo_handoff_shape(CargoHandoffKind::GrainSack, Vec3::ZERO, 1.0);
        let sack_b = cargo_handoff_shape(CargoHandoffKind::GrainSack, Vec3::X, 2.0);
        for (gain, speed) in [sack_a, sack_b] {
            assert!((0.95..=1.05).contains(&gain));
            assert!((0.965..=1.035).contains(&speed));
        }
        assert_ne!(sack_a, sack_b);
        assert_eq!(
            cargo_handoff_shape(CargoHandoffKind::Crate, Vec3::ZERO, 1.0),
            (1.0, 1.0)
        );
    }

    #[test]
    fn rut_crossing_survives_a_large_frame_step_without_false_infinite_lines() {
        let rut = Vec2::new(0.0, 0.0);
        assert!(segment_crosses_rut(
            Vec2::new(-8.0, 1.0),
            Vec2::new(9.0, 1.0),
            rut,
            1.1
        ));
        assert!(!segment_crosses_rut(
            Vec2::new(-8.0, 3.0),
            Vec2::new(9.0, 3.0),
            rut,
            1.1
        ));
        assert!(!segment_crosses_rut(
            Vec2::new(4.0, 4.0),
            Vec2::new(4.0, 4.0),
            rut,
            1.1
        ));
    }

    #[test]
    fn footstep_cadence_consumes_one_stride_and_respects_cooldown() {
        let mut distance = 5.0;
        let mut last = 0.0;
        assert!(advance_footstep(&mut distance, 2.0, &mut last, 1.0));
        assert_eq!(distance, 2.0, "hitch remainder is bounded to one stride");
        assert!(!advance_footstep(&mut distance, 2.0, &mut last, 1.05));
        assert!(advance_footstep(&mut distance, 2.0, &mut last, 1.3));
        assert_eq!(distance, 0.0);
        assert!(step_spacing(12.0) > step_spacing(4.0));
        let walk_contacts_per_second = 8.0 / step_spacing(8.0);
        let run_contacts_per_second = 12.0 / step_spacing(12.0);
        assert!((2.0..=2.7).contains(&walk_contacts_per_second));
        assert!((2.0..=2.7).contains(&run_contacts_per_second));
        assert!(run_contacts_per_second > walk_contacts_per_second);
        assert!(
            (run_contacts_per_second * 0.836).ceil() <= 3.0,
            "the reviewed clip should never build four concurrent footsteps"
        );
    }

    #[test]
    fn coughs_use_work_clothes_and_yawns_prefer_projected_weariness() {
        use cathedral_sim::{OutfitClass, StatusKind};

        assert!(dusty_worker_outfit(OutfitClass::Craftsman));
        assert!(dusty_worker_outfit(OutfitClass::Laborer));
        assert!(!dusty_worker_outfit(OutfitClass::Merchant));
        assert_eq!(
            status_weariness(&[
                (StatusKind::Drunkenness, 0.9),
                (StatusKind::Weariness, 0.72),
            ]),
            0.72
        );
        assert_eq!(status_weariness(&[(StatusKind::Weariness, 1.4)]), 1.0);
        assert_eq!(status_weariness(&[]), 0.0);
    }

    #[test]
    fn cue_cooldown_is_deterministic_and_boundary_inclusive() {
        let mut cooldowns = CueCooldowns::default();
        assert!(cooldowns.allow(7, 10.0, 3.0));
        assert!(!cooldowns.allow(7, 12.999, 3.0));
        assert!(cooldowns.allow(7, 13.0, 3.0));
        assert!(cooldowns.allow(8, 10.1, 3.0));

        for key in 100..140 {
            assert!(cooldowns.allow(key, 20.0, 1.0));
        }
        assert!(cooldowns.allow(999, 200.0, 1.0));
        assert_eq!(cooldowns.last_allowed.len(), 1);
    }

    #[test]
    fn headless_plugin_registers_contract_without_audio_output() {
        let mut app = App::new();
        app.add_plugins(SoundscapePlugin);
        assert!(app.world().contains_resource::<ScheduledSounds>());
        assert!(app.world().contains_resource::<WellSoundState>());
        assert!(app.world().contains_resource::<WellMechanismActivity>());
        assert!(!app.world().contains_resource::<SoundscapeAssets>());
    }

    #[test]
    fn loop_virtualizer_queries_are_disjoint_at_system_initialization() {
        use bevy::ecs::system::{IntoSystem, System};

        let mut world = World::new();
        let mut system = IntoSystem::into_system(update_virtualized_loops);
        system.initialize(&mut world);
    }

    #[test]
    fn tallage_sales_route_to_the_authored_weighbeam_only_inside_the_square() {
        assert_eq!(
            tallage_measurement_anchor(Vec3::new(-305.0, 0.0, 90.0)),
            Some(Vec3::new(-306.0, 1.5, 65.0))
        );
        // A representative stall pitch ~25 m north of the beam remains in the
        // market catchment, while the neighbouring Chain Well does not.
        assert_eq!(
            tallage_measurement_anchor(Vec3::new(-341.994, 0.0, 87.453)),
            Some(TALLAGE_WEIGHBEAM)
        );
        assert_eq!(tallage_measurement_anchor(CHAIN_WELL), None);
        assert_eq!(tallage_measurement_anchor(Vec3::splat(f32::NAN)), None);
    }
}
