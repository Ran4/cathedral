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
//!
//! There are three ways a loop earns a decoder.  A [`StaticEmitter`] is a point
//! in the city on a clock schedule.  An [`AreaBed`] is a named place out of
//! `assets/world/areas.json`, and follows the part of that place nearest the
//! listener, so one bed can be a kilometre of corridor or a twelve-metre
//! passage.  The rest are raised by live state: work, wells, carts, and the
//! church interiors.  One-shots come from [`SoundscapeCue`] and the clock.
//!
//! The two civic bells are the exception to "texture": their stroke *counts*
//! mean something a player is expected to read by ear, so patterns are
//! assembled from one reviewed stroke rather than baked
//! (`lore/second_sun/design/06` §1) and are never pitch-shifted.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bevy::{
    audio::{
        AudioPlayer, AudioPlugin, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings,
        SpatialAudioSink, SpatialScale, Volume,
    },
    prelude::*,
};
use cathedral_sim::{BELL_STROKE_INTERVAL_SECONDS, CartLoadKind, Office, WeatherKind, Weekday};

use crate::{
    city::CobbleRoadNetwork,
    controller::PlayerController,
    smart_actors::{
        AudioActivity, WorldClockState,
        actors::ActorView,
        model::{ActorId, MovementInbox, WorldMirror},
        road_carts::RoadCartView,
    },
    weather::{CoverMaterial, PrecipitationOcclusionMap, WeatherLightning, WorldWeatherState},
};

const SOUND_ROOT: &str = "sounds/soundscape";
/// Concurrent decoders the virtualizer will own. Raised from eight with the
/// named-place beds: a bed is the identity of wherever you are standing, and it
/// must never win its slot by evicting the market, loom or furnace detail that
/// the place is made of.
const MAX_LIVE_LOOPS: usize = 10;
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
/// Exact decoded duration of `assets/sounds/town_bell.mp3`, rounded up.
const TOWN_BELL_CLIP_SECONDS: f64 = 9.04;
const MARKET_DOG_GLOBAL_COOLDOWN_SECONDS: f64 = 150.0;
const SPARR_DOG_COOLDOWN_SECONDS: f64 = 240.0;
const GEESE_GLOBAL_COOLDOWN_SECONDS: f64 = 210.0;
const CAT_MIN_INTERVAL_SECONDS: f64 = 135.0;
const CAT_INTERVAL_JITTER_SECONDS: f64 = 165.0;
/// The minutes between Evenblow's seventh office and the Scold's curfew — the
/// city's dusk grace, in the same real seconds the office strokes use.
const DUSK_GRACE_SECONDS: f64 = 8.0;
const SPEED_OF_SOUND_MPS: f32 = 343.0;
const WEATHER_AUDIO_SAMPLE_RATE: u32 = 22_050;

const WICKMARKET: Vec3 = Vec3::new(-17.5, 1.3, 248.5);
const TALLAGE_WEIGHBEAM: Vec3 = Vec3::new(-214.2, 1.5, 45.5);
const TALLAGE_SQUARE: Vec2 = Vec2::new(-213.5, 63.0);
const TALLAGE_MEASUREMENT_RADIUS_M: f32 = 43.0;
const COMMON_OVEN: Vec3 = Vec3::new(-116.1125, 1.2, 307.7375);
const CINDER_ROW: Vec3 = Vec3::new(-91.0, 1.2, 143.5);
const BURNT_COURT: Vec3 = Vec3::new(-123.4, 1.2, 166.4);
const STONE_GATE: Vec3 = Vec3::new(346.5, 5.0, 94.5);
const STONE_GATE_HOUSING: Vec3 = Vec3::new(335.0, 5.5, 94.5);
const RIVER_GATE: Vec3 = Vec3::new(-353.5, 4.0, -94.5);
/// The Stone House doorway (`law_and_order.md` M5c), taken off
/// `city::build_stone_house`: the court face is x = 39 and the throat is the
/// 1.6 m gap between z -207.6 and z -206, with the leaf hung against the jamb
/// beside it. A gaol door is a fitting of the city like the two gates above and
/// never moves, which is the whole reason the [`SoundscapeCue::GaolDoor`]
/// caller reads it from here rather than off one of the people the `commit`
/// event names — see the note there.
pub(crate) const STONE_HOUSE_DOOR: Vec3 = Vec3::new(39.4, 1.2, -206.8);
const FORD_WELL: Vec3 = Vec3::new(88.0, 1.0, 35.0);
const THREE_CURB: Vec3 = Vec3::new(-91.7, 0.8, 116.2);
const CHAIN_WELL: Vec3 = Vec3::new(-123.8, -2.5, 63.1);
// The city plan's compass is north +x, east -z. This is the north half of
// the west front, high enough that 3D attenuation carries the calls down from
// the canonical unfinished tower rather than making them sound street-level.
const NORTH_TOWER_NESTS: Vec3 = Vec3::new(34.0, 46.0, 75.0);
// The fish landing sits at the eastern end of the built outer-wharf strip,
// close to the Reed Postern but some 200 m from the dry Cut.
const OUTER_FISH_WHARF: Vec3 = Vec3::new(-415.8, 13.0, -282.8);
const SPARR_FURNACE_YARD: Vec3 = Vec3::new(-112.0, 2.0, 270.0);
const SAINT_MARENS_CHURCH: Vec3 = Vec3::new(-140.5, 3.0, -275.6);
/// Maren Smallvoice hangs in Saint Maren's own small tower, lifted to the
/// louvres so the knell carries over the Reed Ward roofs rather than out of a
/// doorway at street level (`saint_marens_church` in `areas.json`).
const SMALLVOICE_TOWER: Vec3 = Vec3::new(-140.5, 17.0, -275.6);
/// The Scold hangs in the Bellstand's watch-bell tower — the civic bell, east
/// of the Lanthorn (`bellstand_tower`, whose box rises to y = 30).
const SCOLD_TOWER: Vec3 = Vec3::new(44.8, 24.0, -189.0);
/// Clemence Skep's honey pitch: the north-row Wickmarket stall beside her
/// husband's family wax stand (`lore/families/family_vell.md` — the wax-house
/// married the honey-seller), one stall east of Osanne Vell's.
const HONEY_STALL: Vec3 = Vec3::new(11.83, 1.3, 251.58);

// Occupied courts and gate-edge holdings that keep a few birds: quiet lanes in
// five different wards, each well clear of a market bed and of the swept
// Gradine. Taken from real door points in the post-shrink
// `assets/world/homes.json` bake.
const HEN_YARD_ANCHORS: [Vec3; 5] = [
    Vec3::new(-29.375, 1.0, -177.375), // Fabric Ward, off the fixed precinct's south edge
    Vec3::new(14.375, 1.0, 306.125),   // Wick Ward, a Wool Gate holding
    Vec3::new(148.625, 1.0, 39.375),   // Wallwright Ward, a Malt Passage food yard
    Vec3::new(-183.625, 1.0, -190.875), // Reed Ward, by Reed Cistern
    Vec3::new(300.375, 1.0, 102.375),  // Cloth Ward, a Stone Gate holding
];

const MARKET_DOG_ANCHORS: [Vec3; 6] = [
    Vec3::new(-43.4, 1.3, 248.5),   // Wickmarket edge
    Vec3::new(-222.6, 1.3, 63.0),   // Tallage edge
    Vec3::new(249.4, 1.3, 129.5),   // Coswald's Yard (scaled + the yard's 45 m east shift)
    Vec3::new(-221.2, 1.3, -255.5), // Maren's Green edge
    STONE_GATE_HOUSING,
    RIVER_GATE,
];

const CAT_ROOF_ANCHORS: [Vec3; 5] = [
    Vec3::new(-275.5, 10.5, -328.6), // Eelback Alley / fish lanes
    Vec3::new(-123.4, 11.5, 166.4),  // Burnt Court
    Vec3::new(107.1, 11.0, -9.1),    // Crookneck Lane
    Vec3::new(25.9, 11.0, 300.3),    // Slate Cistern back lanes
    Vec3::new(-157.8, 10.5, 15.1),   // Gaunt Passage roofs
];

// These sit beyond the four principal gatehouses, where a farm flock can use
// a wet cart rut without suggesting a permanent ornamental pond in the city.
const GATE_GEESE_ANCHORS: [Vec3; 4] = [
    Vec3::new(-24.5, 0.7, 377.0),
    Vec3::new(371.5, 0.7, 94.5),
    Vec3::new(10.5, 0.7, -480.5),
    Vec3::new(-373.5, 0.7, -94.5),
];

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
    /// Somebody has been taken in charge (`law_and_order.md` M4c): the officer's
    /// keys, at the officer.
    CustodyKeys {
        position: Vec3,
    },
    /// Somebody has been committed to the Stone House (`law_and_order.md` M5c):
    /// the one door in this city that is a door. Cued only for the gaol — a gate
    /// arch has no leaf to shut, and confinement there is a keeper standing at a
    /// threshold. The catalogue reserved this clip for exactly this moment.
    GaolDoor {
        position: Vec3,
    },
    /// Ring one of the two civic bells. Patterns are data, never recordings
    /// (`lore/second_sun/design/06` §1): the sequence is assembled here from a
    /// single reviewed stroke, so a knell's count is a number in the caller's
    /// transaction and can never disagree with what the player hears.
    CivicBell(BellPattern),
}

/// An assembled bronze sequence. Each variant carries its own meaning; the
/// stroke count, interval and source come from [`BellPattern::plan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BellPattern {
    /// The Scold's legal Snuffing, following Evenblow's seventh office.
    ScoldCurfew,
    /// The Scold gathering the Bellstand; a proclamation or crying follows.
    ScoldSummons,
    /// Maren Smallvoice: one slow stroke per year of the life.
    NameKnell { years: u16 },
}

/// The most strokes any one pattern may queue. A knell counts a human life, so
/// the cap is generous, but it is a cap: no caller can schedule an unbounded
/// tail of one-shots into [`ScheduledSounds`].
const MAX_BELL_STROKES: u16 = 120;

#[derive(Debug, Clone, Copy)]
struct BellPlan {
    sound: SoundscapeSound,
    position: Vec3,
    strokes: u16,
    interval_seconds: f64,
    /// The word that goes in the evidence line, so drive scripts can assert a
    /// count from `logs.jsonl` without a HUD.
    label: &'static str,
}

impl BellPattern {
    /// The oldest life a name-knell will count out. Callers validate against
    /// this so a bad age fails where it is written, not silently at the rope.
    pub(crate) const MAX_KNELL_YEARS: u16 = MAX_BELL_STROKES;

    fn plan(self) -> BellPlan {
        match self {
            // Nine strokes: longer than any office ring can be (the Snuffing,
            // the greatest, is seven), so a counted curfew can never be
            // mistaken for a counted hour.
            Self::ScoldCurfew => BellPlan {
                sound: SoundscapeSound::ScoldStroke,
                position: SCOLD_TOWER,
                strokes: 9,
                interval_seconds: BELL_STROKE_INTERVAL_SECONDS,
                label: "scold curfew",
            },
            // Fast and short — an urgency the offices never have, and far too
            // quick to be counted as an hour.
            Self::ScoldSummons => BellPlan {
                sound: SoundscapeSound::ScoldStroke,
                position: SCOLD_TOWER,
                strokes: 5,
                interval_seconds: 1.15,
                label: "scold summons",
            },
            Self::NameKnell { years } => BellPlan {
                sound: SoundscapeSound::SmallvoiceStroke,
                position: SMALLVOICE_TOWER,
                strokes: years.clamp(1, MAX_BELL_STROKES),
                interval_seconds: BELL_STROKE_INTERVAL_SECONDS,
                label: "smallvoice knell",
            },
        }
    }
}

/// Queue one assembled peal.
///
/// Every stroke is the same reviewed recording, displaced by 20–40 ms of seeded
/// jitter so the bronze sounds pulled by hands rather than fired by a sequencer
/// (`design/06` §1). The jitter never touches playback *speed*: retuning a bell
/// the player is expected to count would be a worse lie than a metronome.
fn schedule_bell_pattern(
    pattern: BellPattern,
    first_stroke_at: f64,
    seed_prefix: &str,
    scheduled: &mut ScheduledSounds,
) -> BellPlan {
    let plan = pattern.plan();
    for stroke in 0..plan.strokes {
        let seed = stable_hash(&format!("{seed_prefix}:{}:{stroke}", plan.label));
        let magnitude = 0.020 + unit(seed) * 0.020;
        let jitter = if seed & 1 == 0 { magnitude } else { -magnitude };
        let at = first_stroke_at + f64::from(stroke) * plan.interval_seconds + jitter;
        // Bronze pulled by a rope is not struck identically twice; a narrow
        // gain range keeps the count honest while removing the copy-paste.
        let gain = 0.94 + unit(seed.rotate_left(23)) as f32 * 0.12;
        scheduled.push_shaped(
            at.max(first_stroke_at),
            plan.sound,
            plan.position,
            gain,
            1.0,
        );
    }
    plan
}

/// The rope a peal holds, and for how long after its first stroke.
///
/// One peal per bell at a time: a second summons on top of a ringing one would
/// make the strokes uncountable, which is the one thing these bells may never
/// be. The key is therefore the *bell* rather than the pattern, and both ropes
/// into it — the cue and the daily curfew — claim the same entry, or the guard
/// would cover only half the bronze. The window runs a second past the last
/// stroke, so no peal is answered on its own tail.
fn bell_occupancy(plan: &BellPlan) -> (u64, f64) {
    (
        stable_hash(&format!("civic-bell:{}", plan.sound as u8)),
        f64::from(plan.strokes.saturating_sub(1)) * plan.interval_seconds + 1.0,
    )
}

/// Leave the evidence a drive script asserts on: one line per peal, carrying
/// the count, in `logs/latest_session/logs.jsonl` under source `drive`.
fn log_bell_peal(plan: &BellPlan, context: &str) {
    let message = format!(
        "[bell] {}: {} strokes at {:.2}s{context}",
        plan.label, plan.strokes, plan.interval_seconds
    );
    info!("{message}");
    crate::session_log::log_line("drive", "INFO", &message);
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
/// authored weighbeam source.  The live stall pitch is about 30 m from the
/// beam, so testing only the beam itself would silently miss genuine sales;
/// the 43 m square radius covers its stalls without capturing the neighbouring
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
            .init_resource::<ClockSoundState>()
            .init_resource::<CivicBellState>()
            .init_resource::<UrbanNatureState>()
            .init_resource::<WeatherAudioState>()
            .init_resource::<WorldWeatherState>()
            .init_resource::<PrecipitationOcclusionMap>()
            .add_message::<WeatherLightning>();

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
                    schedule_curfew_bell,
                    schedule_player_footsteps,
                    schedule_npc_body_sounds,
                    schedule_weather_thunder,
                    update_weather_audio,
                    schedule_urban_nature_sounds,
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
    NorthTowerRavens,
    SparrowsUnderEaves,
    SwallowsOverCourt,
    RiverWharfGulls,
    MarketDogBark,
    SparrYardDogs,
    AlleyCat,
    FliesAtWaste,
    GateGeese,
    LightningOverLanthorn,
    LanthornNaveAir,
    CongregationPrayer,
    HensInYard,
    BeesAtHoneyStall,
    SmallvoiceStroke,
    ScoldStroke,
    GradineOrdinaryDay,
    WickmarketAtLamplight,
    CoswaldsYardEarly,
    TallageWeighingHour,
    MarensGreenBeforeDayspring,
    DrapersReachInRain,
    TenterhookLaneWorkday,
    CinderRowBehindShutters,
    CutFreightCorridor,
    GauntPassageByDay,
    HungryOxDoorway,
    OldSluiceInDaylight,
    SkinnersCourtLife,
    SevenLoftsGrain,
    GatekeeperKeyRing,
    StoneHouseCellDoor,
}

const ALL_SOUNDS: [SoundscapeSound; 55] = [
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
    SoundscapeSound::NorthTowerRavens,
    SoundscapeSound::SparrowsUnderEaves,
    SoundscapeSound::SwallowsOverCourt,
    SoundscapeSound::RiverWharfGulls,
    SoundscapeSound::MarketDogBark,
    SoundscapeSound::SparrYardDogs,
    SoundscapeSound::AlleyCat,
    SoundscapeSound::FliesAtWaste,
    SoundscapeSound::GateGeese,
    SoundscapeSound::LightningOverLanthorn,
    SoundscapeSound::LanthornNaveAir,
    SoundscapeSound::CongregationPrayer,
    SoundscapeSound::HensInYard,
    SoundscapeSound::BeesAtHoneyStall,
    SoundscapeSound::SmallvoiceStroke,
    SoundscapeSound::ScoldStroke,
    SoundscapeSound::GradineOrdinaryDay,
    SoundscapeSound::WickmarketAtLamplight,
    SoundscapeSound::CoswaldsYardEarly,
    SoundscapeSound::TallageWeighingHour,
    SoundscapeSound::MarensGreenBeforeDayspring,
    SoundscapeSound::DrapersReachInRain,
    SoundscapeSound::TenterhookLaneWorkday,
    SoundscapeSound::CinderRowBehindShutters,
    SoundscapeSound::CutFreightCorridor,
    SoundscapeSound::GauntPassageByDay,
    SoundscapeSound::HungryOxDoorway,
    SoundscapeSound::OldSluiceInDaylight,
    SoundscapeSound::SkinnersCourtLife,
    SoundscapeSound::SevenLoftsGrain,
    SoundscapeSound::GatekeeperKeyRing,
    SoundscapeSound::StoneHouseCellDoor,
];

const SOUND_DESCRIPTORS: [SoundDescriptor; 55] = [
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
    descriptor(
        SoundscapeSound::NorthTowerRavens,
        "snd_241_north_tower_ravens.wav",
        ClipMode::Loop,
        0.24,
        125.0,
        0.20,
    ),
    descriptor(
        SoundscapeSound::SparrowsUnderEaves,
        "snd_243_sparrows_under_the_eaves.wav",
        ClipMode::Loop,
        0.13,
        44.0,
        0.18,
    ),
    descriptor(
        SoundscapeSound::SwallowsOverCourt,
        "snd_244_swallows_over_a_court.wav",
        ClipMode::Loop,
        0.18,
        68.0,
        0.18,
    ),
    descriptor(
        SoundscapeSound::RiverWharfGulls,
        "snd_245_river_gulls_at_the_outer_wharf.wav",
        ClipMode::Loop,
        0.22,
        110.0,
        0.24,
    ),
    descriptor(
        SoundscapeSound::MarketDogBark,
        "snd_246_market_dog_warning_bark.mp3",
        ClipMode::OneShot,
        0.44,
        48.0,
        0.16,
    ),
    descriptor(
        SoundscapeSound::SparrYardDogs,
        "snd_247_sparr_furnace_yard_dogs.mp3",
        ClipMode::Sequence,
        0.46,
        62.0,
        0.16,
    ),
    descriptor(
        SoundscapeSound::AlleyCat,
        "snd_248_alley_cat_on_slate.mp3",
        ClipMode::OneShot,
        0.36,
        42.0,
        0.15,
    ),
    descriptor(
        SoundscapeSound::FliesAtWaste,
        "snd_259_flies_at_eel_smoke_and_offal.wav",
        ClipMode::Loop,
        0.12,
        14.0,
        0.08,
    ),
    descriptor(
        SoundscapeSound::GateGeese,
        "snd_257_geese_at_a_gate_pond_rut.mp3",
        ClipMode::OneShot,
        0.36,
        55.0,
        0.14,
    ),
    descriptor(
        SoundscapeSound::LightningOverLanthorn,
        "snd_268_lightning_over_the_lanthorn.mp3",
        ClipMode::OneShot,
        0.78,
        1_400.0,
        0.52,
    ),
    descriptor(
        SoundscapeSound::LanthornNaveAir,
        "snd_281_lanthorn_nave_air.wav",
        ClipMode::Loop,
        0.34,
        58.0,
        0.30,
    ),
    descriptor(
        SoundscapeSound::CongregationPrayer,
        "snd_292_congregation_prayer_murmur.wav",
        ClipMode::Loop,
        0.26,
        58.0,
        0.04,
    ),
    descriptor(
        SoundscapeSound::HensInYard,
        "snd_256_hens_in_a_domestic_yard.wav",
        ClipMode::Loop,
        0.15,
        26.0,
        0.12,
    ),
    descriptor(
        SoundscapeSound::BeesAtHoneyStall,
        "snd_258_bees_at_the_honey_stall.wav",
        ClipMode::Loop,
        0.13,
        12.0,
        0.06,
    ),
    // The two civic bells. Their radii are the canonical ones from
    // `lore/second_sun/design/06` §2 — the knell carries 300 m across the Reed
    // Ward, the Scold 500 m over the eastern city — and both stay below the
    // Lanthorn's 1 400 m storm voice in scale, as that document requires.
    descriptor(
        SoundscapeSound::SmallvoiceStroke,
        "snd_307_smallvoice_single_stroke.mp3",
        ClipMode::OneShot,
        0.60,
        300.0,
        0.45,
    ),
    descriptor(
        SoundscapeSound::ScoldStroke,
        "snd_308_scold_single_stroke.mp3",
        ClipMode::OneShot,
        0.68,
        500.0,
        0.45,
    ),
    // The named-place beds. Each radius is the sound's own carry; how far
    // outside its area the bed can be *demanded* at all is the separate
    // `spill_m` in `AREA_BEDS`, and is always the smaller of the two.
    descriptor(
        SoundscapeSound::GradineOrdinaryDay,
        "snd_321_gradine_ordinary_day_texture.wav",
        ClipMode::Loop,
        0.20,
        62.0,
        0.10,
    ),
    descriptor(
        SoundscapeSound::WickmarketAtLamplight,
        "snd_322_wickmarket_at_lamplight.wav",
        ClipMode::Loop,
        0.20,
        72.0,
        0.10,
    ),
    descriptor(
        SoundscapeSound::CoswaldsYardEarly,
        "snd_323_coswalds_yard_before_the_hammers.wav",
        ClipMode::Loop,
        0.22,
        66.0,
        0.14,
    ),
    descriptor(
        SoundscapeSound::TallageWeighingHour,
        "snd_324_tallage_weighing_hour.wav",
        ClipMode::Loop,
        0.22,
        70.0,
        0.12,
    ),
    descriptor(
        SoundscapeSound::MarensGreenBeforeDayspring,
        "snd_325_marens_green_before_dayspring.wav",
        ClipMode::Loop,
        0.22,
        60.0,
        0.12,
    ),
    descriptor(
        SoundscapeSound::DrapersReachInRain,
        "snd_328_drapers_reach_in_rain.wav",
        ClipMode::Loop,
        0.26,
        40.0,
        0.14,
    ),
    descriptor(
        SoundscapeSound::TenterhookLaneWorkday,
        "snd_329_tenterhook_lane_workday.wav",
        ClipMode::Loop,
        0.24,
        42.0,
        0.14,
    ),
    descriptor(
        SoundscapeSound::CinderRowBehindShutters,
        "snd_330_cinder_row_behind_shutters.wav",
        ClipMode::Loop,
        0.22,
        44.0,
        0.14,
    ),
    descriptor(
        SoundscapeSound::CutFreightCorridor,
        "snd_332_dry_cut_freight_corridor.wav",
        ClipMode::Loop,
        0.20,
        56.0,
        0.14,
    ),
    descriptor(
        SoundscapeSound::GauntPassageByDay,
        "snd_333_gaunt_passage_by_day.wav",
        ClipMode::Loop,
        0.24,
        30.0,
        0.14,
    ),
    descriptor(
        SoundscapeSound::HungryOxDoorway,
        "snd_337_hungry_ox_doorway_spill.wav",
        ClipMode::Loop,
        0.30,
        30.0,
        0.06,
    ),
    descriptor(
        SoundscapeSound::OldSluiceInDaylight,
        "snd_338_old_sluice_in_daylight.wav",
        ClipMode::Loop,
        0.20,
        46.0,
        0.14,
    ),
    descriptor(
        SoundscapeSound::SkinnersCourtLife,
        "snd_339_skinners_court_work_and_home.wav",
        ClipMode::Loop,
        0.22,
        42.0,
        0.12,
    ),
    descriptor(
        SoundscapeSound::SevenLoftsGrain,
        "snd_340_seven_lofts_grain_interior.wav",
        ClipMode::Loop,
        0.22,
        68.0,
        0.14,
    ),
    // The keys a gate or watch keeper carries, catalogued for "watchmen,
    // gatekeepers, and prisoner escorts" and cued when somebody is taken in
    // charge (`law_and_order.md` M4c). Audible about as far as the seizure
    // itself is: the hue and cry is public, but a key ring is not a bell.
    descriptor(
        SoundscapeSound::GatekeeperKeyRing,
        "snd_065_gatekeepers_key_ring.mp3",
        ClipMode::OneShot,
        0.70,
        24.0,
        0.35,
    ),
    // The gaol door, iron-bound oak through a close masonry corridor: hinge
    // groan, dense wooden impact, bolt and key. Louder and further-carrying than
    // the key ring, because being shut in is public — the Bellstand square hears
    // it — and it is the last thing a committed prisoner hears before the room.
    descriptor(
        SoundscapeSound::StoneHouseCellDoor,
        "snd_080_stone_house_cell_door.mp3",
        ClipMode::OneShot,
        0.85,
        34.0,
        0.4,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
enum WeatherStem {
    LightExterior,
    HeavyExterior,
    HardRoof,
    SoftRoof,
    StormWind,
    Runoff,
    MuffledExterior,
}

impl WeatherStem {
    const ALL: [Self; 7] = [
        Self::LightExterior,
        Self::HeavyExterior,
        Self::HardRoof,
        Self::SoftRoof,
        Self::StormWind,
        Self::Runoff,
        Self::MuffledExterior,
    ];
}

#[derive(Resource)]
struct WeatherAudioAssets {
    stems: [Handle<AudioSource>; 7],
    wet_step: Handle<AudioSource>,
    puddle_splash: Handle<AudioSource>,
}

impl WeatherAudioAssets {
    fn stem(&self, stem: WeatherStem) -> Handle<AudioSource> {
        self.stems[stem as usize].clone()
    }
}

#[derive(Component)]
struct PlayingWeatherLoop {
    stem: WeatherStem,
    current_gain: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct WeatherMix {
    gains: [f32; 7],
    sheltered: bool,
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
struct WeatherAudioState {
    mix: WeatherMix,
}

fn load_soundscape_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
) {
    let handles = ALL_SOUNDS
        .into_iter()
        .map(|sound| (sound, asset_server.load(sound.asset_path())))
        .collect();
    commands.insert_resource(SoundscapeAssets(handles));
    commands.insert_resource(AreaBedGeometry::from_shipped_map());

    // Weather beds are original deterministic procedural recordings. Keeping
    // the generated PCM in shared assets gives rodio normal loop/duck controls
    // without shipping seven near-identical megabyte binaries or synthesizing
    // anything on the audio thread.
    let stems = WeatherStem::ALL.map(|stem| {
        audio_sources.add(AudioSource {
            bytes: Arc::from(weather_wav(WeatherClip::Stem(stem))),
        })
    });
    let weather_assets = WeatherAudioAssets {
        stems,
        wet_step: audio_sources.add(AudioSource {
            bytes: Arc::from(weather_wav(WeatherClip::WetStep)),
        }),
        puddle_splash: audio_sources.add(AudioSource {
            bytes: Arc::from(weather_wav(WeatherClip::PuddleSplash)),
        }),
    };
    for stem in WeatherStem::ALL {
        commands.spawn((
            Name::new(format!("Weather audio: {stem:?}")),
            PlayingWeatherLoop {
                stem,
                current_gain: 0.0,
            },
            AudioPlayer::new(weather_assets.stem(stem)),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }
    commands.insert_resource(weather_assets);
}

#[derive(Debug, Clone, Copy)]
enum WeatherClip {
    Stem(WeatherStem),
    WetStep,
    PuddleSplash,
}

/// Build a mono 16-bit PCM WAV. The inexpensive filters and impact envelopes
/// are evaluated once at startup; all runtime weather work is gain crossfade.
fn weather_wav(clip: WeatherClip) -> Vec<u8> {
    let looped = matches!(clip, WeatherClip::Stem(_));
    let seconds = if looped { 4.0 } else { 0.62 };
    let sample_count = (WEATHER_AUDIO_SAMPLE_RATE as f32 * seconds) as usize;
    let mut seed = 0x789a_bcde_0123_4567_u64
        ^ match clip {
            WeatherClip::Stem(stem) => u64::from(stem as u8) * 0x9e37_79b9,
            WeatherClip::WetStep => 0x51e7_0001,
            WeatherClip::PuddleSplash => 0x5a1a_0002,
        };
    let mut slow = 0.0_f32;
    let mut medium = 0.0_f32;
    let mut impact = 0.0_f32;
    let mut samples = Vec::with_capacity(sample_count);
    for index in 0..sample_count {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let white = (((seed >> 40) as u32) as f32 / 16_777_215.0) * 2.0 - 1.0;
        medium += (white - medium) * 0.16;
        slow += (white - slow) * 0.008;
        if seed & 0x7ff == 0 {
            impact = 1.0;
        }
        impact *= 0.91;
        let t = index as f32 / WEATHER_AUDIO_SAMPLE_RATE as f32;
        let sample = match clip {
            WeatherClip::Stem(WeatherStem::LightExterior) => {
                medium * 0.13 + impact * (white * 0.34 + 0.15)
            }
            WeatherClip::Stem(WeatherStem::HeavyExterior) => {
                white * 0.18 + medium * 0.48 + slow * 0.18 + impact * 0.13
            }
            WeatherClip::Stem(WeatherStem::HardRoof) => {
                medium * 0.12 + impact * (0.54 + (t * std::f32::consts::TAU * 780.0).sin() * 0.28)
            }
            WeatherClip::Stem(WeatherStem::SoftRoof) => medium * 0.30 + slow * 0.28 + impact * 0.12,
            WeatherClip::Stem(WeatherStem::StormWind) => {
                slow * 1.55 + medium * 0.18 + (t * std::f32::consts::TAU * 0.23).sin() * 0.08
            }
            WeatherClip::Stem(WeatherStem::Runoff) => {
                medium * 0.34
                    + white * 0.07
                    + (t * std::f32::consts::TAU * 7.0).sin() * 0.045
                    + impact * 0.09
            }
            WeatherClip::Stem(WeatherStem::MuffledExterior) => slow * 0.72 + medium * 0.16,
            WeatherClip::WetStep => {
                let envelope = (-t * 10.5).exp();
                envelope
                    * (medium * 0.62
                        + (t * std::f32::consts::TAU * 92.0).sin() * 0.34
                        + slow * 0.25)
            }
            WeatherClip::PuddleSplash => {
                let envelope = (-t * 7.5).exp();
                envelope
                    * (white * 0.24
                        + medium * 0.66
                        + (t * std::f32::consts::TAU * 44.0).sin() * 0.18)
            }
        };
        samples.push(sample);
    }
    let peak = samples.iter().copied().map(f32::abs).fold(0.001, f32::max);
    let scale = 0.78 / peak.max(0.78);
    for sample in &mut samples {
        *sample *= scale;
    }
    if looped && samples.len() >= 3 {
        let first = samples[0];
        let penultimate = samples[1];
        let len = samples.len();
        samples[len - 2] = penultimate;
        samples[len - 1] = first;
    }
    pcm16_wav(&samples, WEATHER_AUDIO_SAMPLE_RATE)
}

fn pcm16_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let encoded = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        bytes.extend_from_slice(&encoded.to_le_bytes());
    }
    bytes
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
    /// When each key is free again, rather than when it last fired: a peal is
    /// queued whole, well before its first stroke, so occupancy has to be
    /// expressible as a window that has not started yet.
    free_at: HashMap<u64, f64>,
    last_pruned_at: f64,
}

impl CueCooldowns {
    fn allow(&mut self, key: u64, now: f64, seconds: f64) -> bool {
        if now >= self.last_pruned_at
            && now - self.last_pruned_at >= CUE_COOLDOWN_PRUNE_INTERVAL_SECONDS
        {
            // A window that has not closed yet is a sound still to be heard,
            // however stale the entry looks against the retention clock.
            self.free_at.retain(|_, free_at| {
                now < *free_at || now - *free_at <= CUE_COOLDOWN_RETENTION_SECONDS
            });
            self.last_pruned_at = now;
        }
        let allowed = self.free_at.get(&key).is_none_or(|free_at| now >= *free_at);
        if allowed {
            self.free_at.insert(key, now + seconds);
        }
        allowed
    }

    /// Take a key for a window that begins later — the only way to claim one,
    /// since `allow` can only measure from the moment it is asked. A hold
    /// already in place is never shortened: the longer sound keeps the floor.
    fn hold(&mut self, key: u64, free_at: f64) {
        let entry = self.free_at.entry(key).or_insert(free_at);
        *entry = entry.max(free_at);
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
    // set_if_neq: an unconditional write would mark the resource changed
    // every frame and defeat downstream change detection.
    activity.set_if_neq(wells.activity_at(time.elapsed_secs_f64()));
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
    let _span = crate::perf::span(crate::perf::Probe::Soundscape);
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
            SoundscapeCue::CustodyKeys { position } => {
                // One seizure is one rattle: the cooldown is the length of the
                // clip, so a second officer taking a second person in the same
                // scuffle is heard, and a re-sent event is not.
                let key =
                    positional_cooldown_key(SoundscapeSound::GatekeeperKeyRing, position, 3.0);
                if cooldowns.allow(key, now, 1.5) {
                    scheduled.push(now, SoundscapeSound::GatekeeperKeyRing, position);
                }
            }
            SoundscapeCue::GaolDoor { position } => {
                // Positional, on the same idiom as the keys: two people
                // committed within a few seconds of each other is one door.
                let key = positional_cooldown_key(
                    SoundscapeSound::StoneHouseCellDoor,
                    position,
                    3.0,
                );
                if cooldowns.allow(key, now, 4.0) {
                    scheduled.push(now, SoundscapeSound::StoneHouseCellDoor, position);
                }
            }
            SoundscapeCue::RiverGateBarLift => {
                let key = stable_hash("river_gate_bar_lift");
                if cooldowns.allow(key, now, 30.0) {
                    scheduled.push(now, SoundscapeSound::RiverGateBarLift, RIVER_GATE);
                }
            }
            SoundscapeCue::CivicBell(pattern) => {
                let plan = pattern.plan();
                // The rope is taken for the length of the peal, so nothing
                // rings between these strokes.
                let (key, occupies) = bell_occupancy(&plan);
                if cooldowns.allow(key, now, occupies) {
                    let plan = schedule_bell_pattern(
                        pattern,
                        now,
                        &format!("cue:{:.0}", now * 10.0),
                        &mut scheduled,
                    );
                    log_bell_peal(&plan, "");
                }
            }
        }
    }
}

/// The Scold's curfew: the *legal* Snuffing, rung from the Bellstand after
/// Evenblow's seventh office has finished at the Lanthorn. The office is
/// prayer, the Scold is law, and the gap between them is the city's dusk grace
/// (`lore/second_sun/design/06` §3).
fn schedule_curfew_bell(
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    mut state: ResMut<CivicBellState>,
    mut scheduled: ResMut<ScheduledSounds>,
    mut cooldowns: ResMut<CueCooldowns>,
) {
    let now = time.elapsed_secs_f64();
    let Some(clock) = clock.filter(|clock| clock.present) else {
        state.observed_office = None;
        return;
    };
    let previous = state.observed_office.replace(clock.office);
    // The first projection after startup is not a bell being rung, and neither
    // is midnight arriving partway through the Snuffing: only the edge *into*
    // the seventh office is.
    if previous.is_none_or(|previous| previous == clock.office) {
        return;
    }
    if clock.office != Office::Snuffing || state.curfew_day == Some(clock.day) {
        return;
    }
    state.curfew_day = Some(clock.day);
    // Wait out the Lanthorn's seven strokes, then the grace, then the law.
    let first_stroke_at = now + office_bell_span_seconds(Office::Snuffing) + DUSK_GRACE_SECONDS;
    let plan = schedule_bell_pattern(
        BellPattern::ScoldCurfew,
        first_stroke_at,
        &format!("curfew:{}", clock.day),
        &mut scheduled,
    );
    // The law's peal is never refused, but it must still take the rope, and
    // take it from here rather than from the first stroke: the whole peal is
    // queued now, and a summons cried anywhere in the grace or the nine
    // strokes would fall between them at its own faster tempo.
    let (key, occupies) = bell_occupancy(&plan);
    cooldowns.hold(key, first_stroke_at + occupies);
    log_bell_peal(&plan, &format!(", day {}", clock.day));
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

/// Which day's curfew has already been rung, and the office the edge detector
/// compares against.
///
/// The office alone, never `(day, office)`: the Snuffing runs 21:00 to 02:00,
/// so the day number changes *inside* it, and a day-keyed edge would ring the
/// city's curfew a second time at midnight.
#[derive(Resource, Default)]
struct CivicBellState {
    observed_office: Option<Office>,
    curfew_day: Option<i64>,
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

#[allow(
    clippy::too_many_arguments,
    reason = "Bevy system parameters are independent ECS access declarations"
)]
fn schedule_player_footsteps(
    mut commands: Commands,
    time: Res<Time>,
    cobbles: Option<Res<CobbleRoadNetwork>>,
    weather: Option<Res<WorldWeatherState>>,
    cover: Option<Res<PrecipitationOcclusionMap>>,
    weather_assets: Option<Res<WeatherAudioAssets>>,
    activity: Option<Res<AudioActivity>>,
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
        let exposed = cover
            .as_deref()
            .is_none_or(|cover| !cover.is_sheltered(position));
        let wetness = weather
            .as_deref()
            .map_or(0.0, |weather| weather.current.surface_wetness as f32);
        let wet_mix = if exposed {
            ((wetness - 0.16) / 0.64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        scheduled.push_shaped(
            now,
            SoundscapeSound::CobbleFootstep,
            source,
            gain_jitter * (1.0 - wet_mix * 0.42),
            foot_pitch + jitter,
        );
        if wet_mix > 0.0
            && let Some(assets) = weather_assets.as_deref()
        {
            let busy = activity.as_deref().is_some_and(|activity| activity.busy);
            spawn_weather_footstep(
                &mut commands,
                now,
                source,
                assets.wet_step.clone(),
                0.20 + wet_mix * 0.34,
                if busy { 0.34 } else { 1.0 },
                foot_pitch + jitter * 0.5,
                "wet cobble contact",
            );
            let standing_water = weather
                .as_deref()
                .map_or(0.0, |weather| weather.current.standing_water as f32);
            let splash_roll =
                unit(stable_hash(&format!("step-splash:{}", tracker.sequence))) as f32;
            if standing_water > 0.08 && splash_roll < standing_water * 0.55 {
                spawn_weather_footstep(
                    &mut commands,
                    now,
                    source,
                    assets.puddle_splash.clone(),
                    0.12 + standing_water * 0.24,
                    if busy { 0.30 } else { 1.0 },
                    0.94 + jitter,
                    "shallow puddle splash",
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_weather_footstep(
    commands: &mut Commands,
    now: f64,
    position: Vec3,
    source: Handle<AudioSource>,
    gain: f32,
    initial_scale: f32,
    speed: f32,
    label: &'static str,
) {
    commands.spawn((
        Name::new(format!("Weather sound: {label}")),
        PlayingSoundscapeOneShot {
            spawned_at: now,
            base_gain: gain,
            busy_gain: 0.32,
            current_gain: gain * initial_scale,
        },
        AudioPlayer::new(source),
        PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(gain * initial_scale))
            .with_speed(speed)
            .with_spatial(true)
            .with_spatial_scale(spatial_scale(9.0)),
        Transform::from_translation(position),
    ));
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
    yawn_evening: Option<i64>,
    yawn_due_at: Option<f64>,
}

#[derive(Resource, Default)]
struct NpcSoundState {
    actors: HashMap<String, NpcTimer>,
    next_global_at: f64,
    /// Body sounds trigger on 45 s+ timers; scanning the whole cast (with a
    /// String clone per actor) every frame bought nothing. 4 Hz is plenty.
    next_scan_at: f64,
    /// The office the last scan read, and how many evenings have opened since
    /// the session began — what [`NpcTimer::yawn_evening`] counts in.
    ///
    /// Never the day, for the reason [`CivicBellState`] gives: the yawning
    /// hours run Lamplight to the end of the Snuffing, 18:00 to 02:00, so the
    /// day number changes *inside* them and a day-keyed "once a day" would
    /// re-arm at midnight. Edges into the window cannot move while it is open.
    observed_office: Option<Office>,
    evening: i64,
}

const DUSTY_WORK_ZONES: [(Vec2, f32); 4] = [
    (Vec2::new(223.5, 108.5), 40.6),
    (Vec2::new(-116.1125, 307.7375), 21.0),
    (Vec2::new(-91.0, 143.5), 47.6),
    (Vec2::new(100.0, 220.0), 73.5),
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
    let _span = crate::perf::span(crate::perf::Probe::Soundscape);
    let Ok(player) = player.single() else { return };
    let now = time.elapsed_secs_f64();
    if now < state.next_scan_at {
        return;
    }
    state.next_scan_at = now + 0.25;
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
    // A new evening opens the moment the window does. A clock that has gone
    // quiet is not an edge, so the office last read is kept until it speaks
    // again rather than counted as having left and returned.
    if let Some(office) = office {
        let previous = state.observed_office.replace(office);
        if evening_hours(Some(office)) && !evening_hours(previous) {
            state.evening += 1;
        }
    }
    let evening = state.evening;
    // Retain timers for every still-present actor, including somebody who
    // briefly walks or leaves earshot. Otherwise returning to the radius would
    // reset `yawn_evening` and permit several "once an evening" yawns.
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

    if now >= state.next_global_at && evening_hours(office) {
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
            if timer.yawn_evening == Some(evening) {
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
                timer.yawn_evening = Some(evening);
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

/// The yawning hours: sunset through curfew, 18:00 to 02:00. A clock that has
/// not spoken yet is no hour at all — an evening the player cannot be told is
/// happening is not one to yawn in.
fn evening_hours(office: Option<Office>) -> bool {
    matches!(office, Some(Office::Lamplight | Office::Snuffing))
}

fn dusty_worker_outfit(outfit: cathedral_sim::OutfitClass) -> bool {
    matches!(
        outfit,
        cathedral_sim::OutfitClass::Craftsman | cathedral_sim::OutfitClass::Laborer
    )
}

fn lightning_sound_delay(listener: Vec3, origin: Vec3) -> f64 {
    f64::from(listener.distance(origin) / SPEED_OF_SOUND_MPS)
}

/// Consume the sim's crossed strike exactly once for audio. The weather
/// renderer has its own reader and flashes in the message frame; this schedules
/// thunder in real seconds, so clock acceleration never cheats propagation.
fn schedule_weather_thunder(
    time: Res<Time>,
    mut strikes: MessageReader<WeatherLightning>,
    player: Query<&Transform, With<PlayerController>>,
    mut scheduled: ResMut<ScheduledSounds>,
) {
    let Ok(player) = player.single() else { return };
    let now = time.elapsed_secs_f64();
    for strike in strikes.read() {
        let origin = Vec3::new(
            strike.0.origin_m[0] as f32,
            strike.0.origin_m[1] as f32,
            strike.0.origin_m[2] as f32,
        );
        scheduled.push(
            now + lightning_sound_delay(player.translation, origin),
            SoundscapeSound::LightningOverLanthorn,
            origin,
        );
    }
}

fn weather_mix(
    sample: cathedral_sim::WeatherSample,
    material: CoverMaterial,
    sheltered: bool,
    deep_in_lanthorn: bool,
) -> WeatherMix {
    let rain = sample.precipitation as f32;
    let heavy = ((rain - 0.28) / 0.72).clamp(0.0, 1.0);
    let wet = sample.surface_wetness as f32;
    let mut gains = [0.0; 7];
    if deep_in_lanthorn {
        gains[WeatherStem::MuffledExterior as usize] = rain * 0.26;
        gains[WeatherStem::HardRoof as usize] = rain * (0.10 + heavy * 0.12);
    } else if sheltered {
        gains[WeatherStem::MuffledExterior as usize] = rain * (0.34 + heavy * 0.15);
        match material {
            CoverMaterial::Slate
            | CoverMaterial::Tile
            | CoverMaterial::Stone
            | CoverMaterial::Glass => {
                gains[WeatherStem::HardRoof as usize] = rain * (0.35 + heavy * 0.55);
            }
            CoverMaterial::Thatch | CoverMaterial::Canvas | CoverMaterial::Timber => {
                gains[WeatherStem::SoftRoof as usize] = rain * (0.32 + heavy * 0.48);
            }
            CoverMaterial::Open => {}
        }
    } else {
        gains[WeatherStem::LightExterior as usize] = rain * (1.0 - heavy * 0.76) * 0.72;
        gains[WeatherStem::HeavyExterior as usize] = heavy * (0.28 + rain * 0.72);
    }
    let wind_speed = (sample.wind_xz_mps[0].hypot(sample.wind_xz_mps[1]) as f32 - 2.0) / 8.0;
    gains[WeatherStem::StormWind as usize] =
        (wind_speed.max(0.0) + sample.gust as f32 * 0.42 + sample.thunder as f32 * 0.68)
            .clamp(0.0, 1.0)
            * if deep_in_lanthorn { 0.18 } else { 0.48 };
    gains[WeatherStem::Runoff as usize] = ((wet - 0.34) / 0.66).clamp(0.0, 1.0)
        * (0.12 + rain * 0.68)
        * if deep_in_lanthorn {
            0.12
        } else if sheltered {
            0.52
        } else {
            0.30
        };
    WeatherMix { gains, sheltered }
}

fn update_weather_audio(
    time: Res<Time>,
    weather: Res<WorldWeatherState>,
    cover: Res<PrecipitationOcclusionMap>,
    activity: Option<Res<AudioActivity>>,
    player: Query<&Transform, With<PlayerController>>,
    mut state: ResMut<WeatherAudioState>,
    mut loops: Query<(&mut PlayingWeatherLoop, Option<&mut AudioSink>)>,
) {
    let _span = crate::perf::span(crate::perf::Probe::Soundscape);
    let mix = player
        .single()
        .ok()
        .map_or_else(WeatherMix::default, |player| {
            let position = player.translation;
            let cover_sample = cover.sample(position.x, position.z);
            let sheltered =
                cover_sample.sheltered_listener && position.y < cover_sample.impact_y - 0.15;
            weather_mix(
                weather.current,
                cover_sample.material,
                sheltered,
                inside_lanthorn_interior(position),
            )
        });
    state.mix = mix;
    let busy_scale = if activity.as_deref().is_some_and(|activity| activity.busy) {
        0.24
    } else {
        1.0
    };
    let dt = time.delta_secs();
    for (mut playing, sink) in &mut loops {
        let target = mix.gains[playing.stem as usize] * busy_scale;
        playing.current_gain = smooth_gain(
            playing.current_gain,
            target,
            dt,
            target < playing.current_gain,
        );
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(playing.current_gain));
        }
    }
}

/// Heavy rain sends the animals under cover — the one threshold the whole game
/// asks "have the animals gone quiet?" with.
///
/// `pub(crate)` because it is no longer only about sound: `city::vermin` thins
/// the *visible* rat colonies through this same predicate (`features/rats.md`
/// §3, "matching the animals going quiet"), so the birds falling silent and the
/// rats going to ground are one decision, not two that have to be kept in step
/// by hand. Do not inline it back into either caller.
pub(crate) fn wildlife_suppressed(weather: Option<&WorldWeatherState>) -> bool {
    weather.is_some_and(|weather| {
        weather.current.precipitation >= 0.62
            || matches!(
                weather.current.kind,
                WeatherKind::Downpour | WeatherKind::Thunderstorm
            )
    })
}

#[derive(Resource, Default)]
struct UrbanNatureState {
    approaches: HashMap<u64, bool>,
    next_market_dog_at: f64,
    next_sparr_dogs_at: f64,
    next_geese_at: f64,
    next_cat_at: Option<f64>,
    event_sequence: u64,
    observed_office: Option<(i64, Office)>,
    ravens_silent_until: f64,
}

impl UrbanNatureState {
    fn observe_office_bell(&mut self, now: f64, clock: Option<&WorldClockState>) {
        let Some(clock) = clock.filter(|clock| clock.present) else {
            self.observed_office = None;
            return;
        };
        let current = (clock.day, clock.office);
        if self
            .observed_office
            .is_some_and(|previous| previous != current)
        {
            self.ravens_silent_until = now + office_bell_span_seconds(clock.office);
        }
        self.observed_office = Some(current);
    }

    fn next_sequence(&mut self) -> u64 {
        self.event_sequence = self.event_sequence.wrapping_add(1);
        self.event_sequence
    }
}

fn office_bell_span_seconds(office: Office) -> f64 {
    f64::from(office.ordinal().saturating_sub(1)) * BELL_STROKE_INTERVAL_SECONDS
        + TOWN_BELL_CLIP_SECONDS
}

fn daylight_animals_active(clock: Option<&WorldClockState>) -> bool {
    clock
        .filter(|clock| clock.present)
        .is_none_or(|clock| clock.brightness > 0.22)
}

fn market_dogs_active(clock: Option<&WorldClockState>) -> bool {
    clock.filter(|clock| clock.present).is_none_or(|clock| {
        matches!(
            clock.office,
            Office::Kindling | Office::Dayspring | Office::HighWick | Office::Waning
        )
    })
}

fn dusk_or_night(clock: Option<&WorldClockState>) -> bool {
    clock.filter(|clock| clock.present).is_some_and(|clock| {
        matches!(
            clock.office,
            Office::Lamplight | Office::Snuffing | Office::Watch
        )
    })
}

fn proximity_entered(
    approaches: &mut HashMap<u64, bool>,
    key: u64,
    listener: Vec3,
    source: Vec3,
    enter_radius_m: f32,
    leave_radius_m: f32,
) -> bool {
    debug_assert!(leave_radius_m > enter_radius_m);
    let was_inside = approaches.get(&key).copied().unwrap_or(false);
    let radius = if was_inside {
        leave_radius_m
    } else {
        enter_radius_m
    };
    let inside = listener.distance_squared(source) <= radius.powi(2);
    approaches.insert(key, inside);
    inside && !was_inside
}

fn schedule_urban_nature_sounds(
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    player: Query<&Transform, With<PlayerController>>,
    weather: Option<Res<WorldWeatherState>>,
    mut state: ResMut<UrbanNatureState>,
    mut scheduled: ResMut<ScheduledSounds>,
) {
    let _span = crate::perf::span(crate::perf::Probe::Soundscape);
    let now = time.elapsed_secs_f64();
    state.observe_office_bell(now, clock.as_deref());
    let Ok(player) = player.single() else { return };
    let listener = player.translation;

    for (index, source) in MARKET_DOG_ANCHORS.into_iter().enumerate() {
        let entered = proximity_entered(
            &mut state.approaches,
            10_000 + index as u64,
            listener,
            source,
            27.0,
            42.0,
        );
        if entered && market_dogs_active(clock.as_deref()) && now >= state.next_market_dog_at {
            let sequence = state.next_sequence();
            let gain =
                0.92 + unit(stable_hash(&format!("market-dog-gain:{sequence}"))) as f32 * 0.12;
            let speed =
                0.975 + unit(stable_hash(&format!("market-dog-speed:{sequence}"))) as f32 * 0.05;
            scheduled.push_shaped(now, SoundscapeSound::MarketDogBark, source, gain, speed);
            state.next_market_dog_at = now + MARKET_DOG_GLOBAL_COOLDOWN_SECONDS;
        }
    }

    let entered_sparr_yard = proximity_entered(
        &mut state.approaches,
        11_000,
        listener,
        SPARR_FURNACE_YARD,
        36.0,
        52.0,
    );
    if entered_sparr_yard && dusk_or_night(clock.as_deref()) && now >= state.next_sparr_dogs_at {
        scheduled.push(now, SoundscapeSound::SparrYardDogs, SPARR_FURNACE_YARD);
        state.next_sparr_dogs_at = now + SPARR_DOG_COOLDOWN_SECONDS;
    }

    for (index, source) in GATE_GEESE_ANCHORS.into_iter().enumerate() {
        let entered = proximity_entered(
            &mut state.approaches,
            12_000 + index as u64,
            listener,
            source,
            34.0,
            52.0,
        );
        if entered
            && daylight_animals_active(clock.as_deref())
            && !wildlife_suppressed(weather.as_deref())
            && now >= state.next_geese_at
        {
            let sequence = state.next_sequence();
            let gain = 0.94 + unit(stable_hash(&format!("geese-gain:{sequence}"))) as f32 * 0.12;
            let speed = 0.975 + unit(stable_hash(&format!("geese-speed:{sequence}"))) as f32 * 0.05;
            scheduled.push_shaped(now, SoundscapeSound::GateGeese, source, gain, speed);
            state.next_geese_at = now + GEESE_GLOBAL_COOLDOWN_SECONDS;
        }
    }

    if !dusk_or_night(clock.as_deref()) {
        // A fresh delay each evening prevents a cat event that expired during
        // the day from firing on the exact Lamplight boundary.
        state.next_cat_at = None;
        return;
    }

    let day = clock
        .as_deref()
        .filter(|clock| clock.present)
        .map_or(0, |clock| clock.day);
    let due = *state
        .next_cat_at
        .get_or_insert_with(|| now + 18.0 + unit(stable_hash(&format!("cat-first:{day}"))) * 42.0);
    if now < due {
        return;
    }
    let nearest_roof = CAT_ROOF_ANCHORS
        .into_iter()
        .filter(|source| listener.distance_squared(*source) <= 40.0_f32.powi(2))
        .min_by(|a, b| {
            listener
                .distance_squared(*a)
                .total_cmp(&listener.distance_squared(*b))
        });
    let Some(source) = nearest_roof else { return };
    let sequence = state.next_sequence();
    let gain = 0.94 + unit(stable_hash(&format!("cat-gain:{day}:{sequence}"))) as f32 * 0.12;
    let speed = 0.97 + unit(stable_hash(&format!("cat-speed:{day}:{sequence}"))) as f32 * 0.06;
    scheduled.push_shaped(now, SoundscapeSound::AlleyCat, source, gain, speed);
    state.next_cat_at = Some(
        now + CAT_MIN_INTERVAL_SECONDS
            + unit(stable_hash(&format!("cat-next:{day}:{sequence}")))
                * CAT_INTERVAL_JITTER_SECONDS,
    );
}

// Each point sits on a road ribbon of the shrunk plan: the west approach by
// the Gradine forecourt, the dry Cut through the Tallage, the wall-road /
// fabric-way crossing inside Stone Gate, the river cartway inside River Gate,
// and the loft-lane bend at the Seven Lofts.
const AUTHORED_RUTS: [Vec2; 5] = [
    Vec2::new(0.0, 171.0),
    Vec2::new(-212.8, 63.0),
    Vec2::new(332.5, 95.5),
    Vec2::new(-335.3, -93.5),
    Vec2::new(252.0, 234.5),
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
    let _span = crate::perf::span(crate::perf::Probe::Soundscape);
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
    let _span = crate::perf::span(crate::perf::Probe::Soundscape);
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
    DaylightAnimals,
    WarmDayWaste,
    /// Bees only work the comb while the honey pitch is open and the day warm.
    HoneyStallDay,
}

#[derive(Debug, Clone, Copy)]
struct StaticEmitter {
    key: u64,
    sound: SoundscapeSound,
    position: Vec3,
    schedule: EmitterSchedule,
    priority: u8,
}

const STATIC_EMITTERS: [StaticEmitter; 31] = [
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
        position: Vec3::new(-163.394, 0.7, -248.332),
        schedule: EmitterSchedule::MarenLowmarket,
        priority: 55,
    },
    StaticEmitter {
        key: 4,
        sound: SoundscapeSound::EelSmokeFire,
        position: Vec3::new(-255.148, 0.7, -240.314),
        schedule: EmitterSchedule::MarenLowmarket,
        priority: 55,
    },
    StaticEmitter {
        key: 5,
        sound: SoundscapeSound::EelSmokeFire,
        position: Vec3::new(-175.944, 0.7, -237.339),
        schedule: EmitterSchedule::MarenLowmarket,
        priority: 55,
    },
    StaticEmitter {
        key: 6,
        sound: SoundscapeSound::EelSmokeFire,
        position: Vec3::new(-228.971, 0.7, -220.211),
        schedule: EmitterSchedule::MarenLowmarket,
        priority: 55,
    },
    StaticEmitter {
        key: 7,
        sound: SoundscapeSound::EelSmokeFire,
        position: Vec3::new(-167.872, 0.7, -248.900),
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
        position: Vec3::new(37.0, 1.5, 256.0),
        schedule: EmitterSchedule::LoomWork,
        priority: 62,
    },
    StaticEmitter {
        key: 12,
        sound: SoundscapeSound::Loom,
        position: Vec3::new(96.0, 1.5, 234.0),
        schedule: EmitterSchedule::LoomWork,
        priority: 62,
    },
    StaticEmitter {
        key: 13,
        sound: SoundscapeSound::Loom,
        position: Vec3::new(154.0, 1.5, 204.0),
        schedule: EmitterSchedule::LoomWork,
        priority: 62,
    },
    StaticEmitter {
        key: 20,
        sound: SoundscapeSound::NorthTowerRavens,
        position: NORTH_TOWER_NESTS,
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 68,
    },
    // House-door coordinates in quiet residential lanes, lifted to their
    // eaves. None is within the playback radius of a market or furnace bed.
    // Re-picked from real door points of the post-shrink homes.json bake.
    StaticEmitter {
        key: 21,
        sound: SoundscapeSound::SparrowsUnderEaves,
        position: Vec3::new(125.125, 8.5, 5.375),
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 28,
    },
    StaticEmitter {
        key: 22,
        sound: SoundscapeSound::SparrowsUnderEaves,
        position: Vec3::new(-38.125, 8.5, 315.125),
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 28,
    },
    StaticEmitter {
        key: 23,
        sound: SoundscapeSound::SparrowsUnderEaves,
        position: Vec3::new(192.875, 9.5, -217.875),
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 28,
    },
    StaticEmitter {
        key: 24,
        sound: SoundscapeSound::SparrowsUnderEaves,
        position: Vec3::new(-181.875, 8.0, -39.625),
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 28,
    },
    StaticEmitter {
        key: 25,
        sound: SoundscapeSound::SparrowsUnderEaves,
        position: Vec3::new(-274.375, 7.5, -177.875),
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 28,
    },
    // The fixed world is a temperate-summer city (see smart_actors/clock.rs),
    // so these are the seasonal court sources rather than year-round birds.
    StaticEmitter {
        key: 26,
        sound: SoundscapeSound::SwallowsOverCourt,
        position: Vec3::new(-123.4, 17.2, 166.4),
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 36,
    },
    StaticEmitter {
        key: 27,
        sound: SoundscapeSound::SwallowsOverCourt,
        position: Vec3::new(135.0, 17.0, 188.0),
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 36,
    },
    StaticEmitter {
        key: 28,
        sound: SoundscapeSound::SwallowsOverCourt,
        position: Vec3::new(35.8, 17.0, -219.0),
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 36,
    },
    StaticEmitter {
        key: 29,
        sound: SoundscapeSound::RiverWharfGulls,
        position: OUTER_FISH_WHARF,
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 48,
    },
    // The city's fixed season is warm. These intentionally tiny sources mark
    // material waste piles, not three broad district ambience beds.
    StaticEmitter {
        key: 30,
        sound: SoundscapeSound::FliesAtWaste,
        position: Vec3::new(-214.2, 0.7, -255.5),
        schedule: EmitterSchedule::WarmDayWaste,
        priority: 18,
    },
    StaticEmitter {
        key: 31,
        sound: SoundscapeSound::FliesAtWaste,
        position: Vec3::new(-294.2, 0.7, 219.6),
        schedule: EmitterSchedule::WarmDayWaste,
        priority: 18,
    },
    StaticEmitter {
        key: 32,
        sound: SoundscapeSound::FliesAtWaste,
        position: Vec3::new(-296.5, 0.7, -228.6),
        schedule: EmitterSchedule::WarmDayWaste,
        priority: 18,
    },
    // One honey pitch, not a beekeeping quarter: a 12 m source that only the
    // people at that stall ever hear.
    StaticEmitter {
        key: 33,
        sound: SoundscapeSound::BeesAtHoneyStall,
        position: HONEY_STALL,
        schedule: EmitterSchedule::HoneyStallDay,
        priority: 20,
    },
    StaticEmitter {
        key: 34,
        sound: SoundscapeSound::HensInYard,
        position: HEN_YARD_ANCHORS[0],
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 30,
    },
    StaticEmitter {
        key: 35,
        sound: SoundscapeSound::HensInYard,
        position: HEN_YARD_ANCHORS[1],
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 30,
    },
    StaticEmitter {
        key: 36,
        sound: SoundscapeSound::HensInYard,
        position: HEN_YARD_ANCHORS[2],
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 30,
    },
    StaticEmitter {
        key: 37,
        sound: SoundscapeSound::HensInYard,
        position: HEN_YARD_ANCHORS[3],
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 30,
    },
    StaticEmitter {
        key: 38,
        sound: SoundscapeSound::HensInYard,
        position: HEN_YARD_ANCHORS[4],
        schedule: EmitterSchedule::DaylightAnimals,
        priority: 30,
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
        EmitterSchedule::DaylightAnimals => daylight_animals_active(Some(clock)),
        EmitterSchedule::WarmDayWaste => clock.brightness > 0.30,
        EmitterSchedule::HoneyStallDay => {
            clock.weekday != Weekday::Bellday
                && clock.brightness > 0.30
                && matches!(
                    clock.office,
                    Office::Dayspring | Office::HighWick | Office::Waning
                )
        }
    }
}

fn static_emitter_speed(emitter: StaticEmitter) -> f32 {
    match emitter.sound {
        // Closely spaced copies of one reviewed loop must not phase-lock into a
        // single conspicuous recording. The variance is fixed per nest/court.
        SoundscapeSound::SparrowsUnderEaves
        | SoundscapeSound::SwallowsOverCourt
        | SoundscapeSound::FliesAtWaste
        | SoundscapeSound::HensInYard => {
            let seed =
                stable_hash("animal-loop-speed") ^ emitter.key.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            0.985 + unit(seed) as f32 * 0.03
        }
        _ => 1.0,
    }
}

/// Weather shaping applied to a static emitter's authored gain.
///
/// Rain is not a switch for every source: covering the honey pots quiets the
/// bees long before the shower is heavy enough for [`wildlife_suppressed`] to
/// send them in altogether.
fn static_emitter_weather_gain(emitter: StaticEmitter, weather: Option<&WorldWeatherState>) -> f32 {
    let rain = weather.map_or(0.0, |weather| weather.current.precipitation as f32);
    match emitter.sound {
        SoundscapeSound::BeesAtHoneyStall => (1.0 - rain * 1.6).clamp(0.0, 1.0),
        _ => 1.0,
    }
}

/// Rain at or above this fraction is what the Draper's Reach bed is *about*:
/// below it the gallery simply sounds like a working cloth market.
const AREA_BED_RAIN_THRESHOLD: f64 = 0.14;
/// What a static point source drops to while the listener stands inside a bed
/// that explicitly describes it as heard through a screen or shutter.
const MUFFLED_BEHIND_BED_GAIN: f32 = 0.45;
/// How far outside a place somebody may stand and still be *at* it. The round's
/// widest leash is 24 m, but a place bed only cares about the people working
/// the place itself, so this is one ordinary tavern leash.
const AREA_BED_OCCUPANCY_MARGIN_M: f32 = 9.0;

/// When a named place sounds like itself.
///
/// Each variant is one authored place's working hours, not a generic clock
/// window: the Wickmarket closes down while the Tallage is still weighing, and
/// Maren's Green is loudest before the office that opens everyone else's day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AreaBedSchedule {
    /// Petitioners and vergers on the ceremonial steps — every day but the
    /// feast day, which the steps belong to for other reasons.
    GradineOrdinaryDay,
    /// The close-down bed. It takes over from the Highmarket crowd at
    /// Lamplight, and owns the Waning on the days that crowd never gathers.
    WickmarketCloseDown,
    /// Apprentices uncovering benches before the hammers start.
    CoswaldsFirstLight,
    /// Freight and the weigh-beams.
    TallageWeighing,
    /// Fish handbarrows arriving in the dark, before Dayspring opens the market.
    MarensGreenFishArrival,
    /// Weather-shaped, not clock-shaped: the gallery in rain, at any hour.
    DrapersReachRain,
    /// Fulling, stretching and cistern work along the lane.
    TenterhookClothWork,
    /// The glass street with its furnaces behind screened bays.
    CinderRowDay,
    /// Carts and porters along the old river bed.
    CutFreight,
    /// The passage while it is still a useful shortcut — folklore empties it
    /// after dark, and this bed leaves with the light.
    GauntPassageWork,
    /// Spill from an occupied tavern doorway, from Lamplight onward.
    HungryOxEvening,
    /// Dry arches and ordinary foot traffic, in daylight.
    OldSluiceDay,
    /// A court that is a workshop and a home at once, by day.
    SkinnersCourtDay,
    /// Stock moving through the grain lofts on a working day.
    SevenLoftsStock,
}

impl AreaBedSchedule {
    /// Whether this bed's activity depends on how many people are in the place.
    /// Only the tavern does: an empty room spills nothing.
    const fn needs_occupancy(self) -> bool {
        matches!(self, Self::HungryOxEvening)
    }
}

/// One reviewed loop bound to a named area in `assets/world/areas.json`.
///
/// Binding to the shipped area rather than a hand-copied coordinate is the
/// point: the emitter follows the nearest part of the real place, so a bed can
/// belong to a 1 km corridor (`the_cut`) or a 12 m passage (`gaunt_passage`)
/// without either becoming a point source shouting from its centroid.
#[derive(Debug, Clone, Copy)]
struct AreaBed {
    key: u64,
    area_id: &'static str,
    sound: SoundscapeSound,
    schedule: AreaBedSchedule,
    priority: u8,
    /// How far outside the area's own boxes the bed may still be demanded.
    /// Always below the sound's descriptor radius, so the radius governs
    /// attenuation and this governs belonging.
    spill_m: f32,
    /// Open to the sky, so a downpour genuinely thins the people in it.
    open_air: bool,
    /// Point sources this bed puts behind a screen while the listener stands
    /// inside its area.
    muffles: &'static [SoundscapeSound],
}

const AREA_BEDS: [AreaBed; 14] = [
    AreaBed {
        key: 5_001,
        area_id: "gradine",
        sound: SoundscapeSound::GradineOrdinaryDay,
        schedule: AreaBedSchedule::GradineOrdinaryDay,
        priority: 82,
        spill_m: 26.0,
        open_air: true,
        muffles: &[],
    },
    AreaBed {
        key: 5_002,
        area_id: "wickmarket",
        sound: SoundscapeSound::WickmarketAtLamplight,
        schedule: AreaBedSchedule::WickmarketCloseDown,
        priority: 95,
        spill_m: 30.0,
        open_air: true,
        muffles: &[],
    },
    AreaBed {
        key: 5_003,
        area_id: "coswalds_yard",
        sound: SoundscapeSound::CoswaldsYardEarly,
        schedule: AreaBedSchedule::CoswaldsFirstLight,
        priority: 80,
        spill_m: 30.0,
        open_air: true,
        muffles: &[],
    },
    AreaBed {
        key: 5_004,
        area_id: "tallage",
        sound: SoundscapeSound::TallageWeighingHour,
        schedule: AreaBedSchedule::TallageWeighing,
        priority: 82,
        spill_m: 40.0,
        open_air: true,
        muffles: &[],
    },
    AreaBed {
        key: 5_005,
        area_id: "marens_green",
        sound: SoundscapeSound::MarensGreenBeforeDayspring,
        schedule: AreaBedSchedule::MarensGreenFishArrival,
        priority: 82,
        spill_m: 40.0,
        open_air: true,
        muffles: &[],
    },
    AreaBed {
        key: 5_006,
        area_id: "drapers_reach",
        sound: SoundscapeSound::DrapersReachInRain,
        schedule: AreaBedSchedule::DrapersReachRain,
        priority: 84,
        spill_m: 20.0,
        // The Reach alternates covered bays with two open courts; the bed is
        // the rain on that gallery, so rain must not also thin it.
        open_air: false,
        muffles: &[],
    },
    AreaBed {
        key: 5_007,
        area_id: "tenterhook_lane",
        sound: SoundscapeSound::TenterhookLaneWorkday,
        schedule: AreaBedSchedule::TenterhookClothWork,
        priority: 80,
        spill_m: 22.0,
        open_air: true,
        muffles: &[],
    },
    AreaBed {
        key: 5_008,
        area_id: "cinder_row",
        sound: SoundscapeSound::CinderRowBehindShutters,
        schedule: AreaBedSchedule::CinderRowDay,
        priority: 82,
        spill_m: 22.0,
        open_air: true,
        // "each numbered furnace a muffled source": standing in the street, the
        // glass-house draw is behind a screened bay, not in the open with you.
        muffles: &[SoundscapeSound::CinderFurnace],
    },
    AreaBed {
        key: 5_009,
        area_id: "the_cut",
        sound: SoundscapeSound::CutFreightCorridor,
        schedule: AreaBedSchedule::CutFreight,
        priority: 78,
        spill_m: 26.0,
        open_air: true,
        muffles: &[],
    },
    AreaBed {
        key: 5_010,
        area_id: "gaunt_passage",
        sound: SoundscapeSound::GauntPassageByDay,
        schedule: AreaBedSchedule::GauntPassageWork,
        priority: 84,
        // The canonical 20 m of leakage at the grate and both public bends.
        spill_m: 20.0,
        open_air: false,
        muffles: &[],
    },
    AreaBed {
        key: 5_011,
        area_id: "hungry_ox",
        sound: SoundscapeSound::HungryOxDoorway,
        schedule: AreaBedSchedule::HungryOxEvening,
        priority: 84,
        // Canon gives the Ox 30 m of spill; the door is the source, not the room.
        spill_m: 22.0,
        open_air: false,
        muffles: &[],
    },
    AreaBed {
        key: 5_012,
        area_id: "old_sluice",
        sound: SoundscapeSound::OldSluiceInDaylight,
        schedule: AreaBedSchedule::OldSluiceDay,
        priority: 78,
        spill_m: 26.0,
        open_air: true,
        muffles: &[],
    },
    AreaBed {
        key: 5_013,
        area_id: "skinners_court",
        sound: SoundscapeSound::SkinnersCourtLife,
        schedule: AreaBedSchedule::SkinnersCourtDay,
        priority: 80,
        spill_m: 24.0,
        open_air: true,
        muffles: &[],
    },
    AreaBed {
        key: 5_014,
        area_id: "seven_lofts",
        sound: SoundscapeSound::SevenLoftsGrain,
        schedule: AreaBedSchedule::SevenLoftsStock,
        priority: 78,
        spill_m: 30.0,
        // Granary floors under a roof: the store does not empty because it rains.
        open_air: false,
        muffles: &[],
    },
];

fn area_bed_for_key(key: u64) -> Option<AreaBed> {
    AREA_BEDS.into_iter().find(|bed| bed.key == key)
}

/// The shipped place map, embedded so audio geography cannot drift from the
/// simulation's. The same file the sim parses at startup.
const AREA_MAP_SOURCE: &str = include_str!("../assets/world/areas.json");

/// One area box in render-side f32, ready for per-frame distance work.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BedBox {
    min: Vec3,
    max: Vec3,
}

impl BedBox {
    /// The point of this box nearest `position` — `position` itself when it is
    /// inside, which is exactly the behaviour a room tone wants: the bed stops
    /// having a direction the moment you are in the place.
    fn nearest_point(self, position: Vec3) -> Vec3 {
        position.clamp(self.min, self.max)
    }

    fn contains(self, position: Vec3) -> bool {
        inside_box(position, self.min, self.max)
    }
}

/// The box unions of every area an [`AreaBed`] names, resolved once at startup.
#[derive(Resource, Debug, Default)]
struct AreaBedGeometry(HashMap<&'static str, Vec<BedBox>>);

impl AreaBedGeometry {
    /// Resolve the beds' areas out of the shipped map. An area that has been
    /// renamed or removed loses its bed and logs it — the tests are the guard
    /// against that happening silently, not a startup panic.
    fn from_shipped_map() -> Self {
        let map = match cathedral_sim::AreaMap::from_json_str(AREA_MAP_SOURCE) {
            Ok(map) => map,
            Err(error) => {
                error!("soundscape: shipped area map did not parse ({error}); place beds are off");
                return Self::default();
            }
        };
        let mut boxes = HashMap::new();
        for bed in AREA_BEDS {
            if boxes.contains_key(bed.area_id) {
                continue;
            }
            let Some(area) = map.areas.iter().find(|area| area.id == bed.area_id) else {
                warn!(
                    "soundscape: no area `{}` in the shipped map; its bed will not play",
                    bed.area_id
                );
                continue;
            };
            boxes.insert(
                bed.area_id,
                area.boxes
                    .iter()
                    .map(|bounds| BedBox {
                        min: Vec3::new(
                            bounds.min_m.x as f32,
                            bounds.min_m.y as f32,
                            bounds.min_m.z as f32,
                        ),
                        max: Vec3::new(
                            bounds.max_m.x as f32,
                            bounds.max_m.y as f32,
                            bounds.max_m.z as f32,
                        ),
                    })
                    .collect::<Vec<_>>(),
            );
        }
        Self(boxes)
    }

    /// The point of the named area nearest `position`, with its distance.
    fn anchor(&self, area_id: &str, position: Vec3) -> Option<(Vec3, f32)> {
        self.0
            .get(area_id)?
            .iter()
            .map(|bounds| {
                let point = bounds.nearest_point(position);
                (point, point.distance(position))
            })
            .min_by(|(_, left), (_, right)| left.total_cmp(right))
    }

    fn contains(&self, area_id: &str, position: Vec3) -> bool {
        self.0
            .get(area_id)
            .is_some_and(|boxes| boxes.iter().any(|bounds| bounds.contains(position)))
    }

    /// Whether somebody standing here counts as being *at* the place.
    ///
    /// Deliberately looser than [`Self::contains`]: the round parks a worker on
    /// their workplace's nav node and lets them drift within their leash, and
    /// the Hungry Ox's node sits about a metre inside its own box. A strict
    /// containment test would report the tavern empty all evening while five
    /// people worked it.
    fn occupied_by(&self, area_id: &str, position: Vec3) -> bool {
        self.anchor(area_id, position)
            .is_some_and(|(_, distance_m)| distance_m <= AREA_BED_OCCUPANCY_MARGIN_M)
    }
}

fn area_bed_is_active(
    schedule: AreaBedSchedule,
    clock: Option<&WorldClockState>,
    weather: Option<&WorldWeatherState>,
    occupants: usize,
) -> bool {
    let raining =
        weather.is_some_and(|weather| weather.current.precipitation >= AREA_BED_RAIN_THRESHOLD);
    if schedule == AreaBedSchedule::DrapersReachRain {
        // The one bed the clock has no opinion about.
        return raining;
    }
    let Some(clock) = clock.filter(|clock| clock.present) else {
        // Without a clock, keep the beds that are simply "this place, working"
        // and drop the ones that mean a specific hour of a specific day.
        return matches!(
            schedule,
            AreaBedSchedule::GradineOrdinaryDay
                | AreaBedSchedule::TallageWeighing
                | AreaBedSchedule::TenterhookClothWork
                | AreaBedSchedule::CinderRowDay
                | AreaBedSchedule::CutFreight
                | AreaBedSchedule::GauntPassageWork
                | AreaBedSchedule::OldSluiceDay
                | AreaBedSchedule::SkinnersCourtDay
                | AreaBedSchedule::SevenLoftsStock
        );
    };
    let workday = clock.weekday != Weekday::Bellday;
    let open_hours = matches!(
        clock.office,
        Office::Dayspring | Office::HighWick | Office::Waning
    );
    match schedule {
        // The feast day belongs to the Gradine for other reasons; this is the
        // texture of an *ordinary* day on the steps.
        AreaBedSchedule::GradineOrdinaryDay => workday && open_hours,
        AreaBedSchedule::WickmarketCloseDown => {
            clock.office == Office::Lamplight
                || (clock.office == Office::Waning && clock.weekday != Weekday::Highmarket)
        }
        AreaBedSchedule::CoswaldsFirstLight => {
            workday && matches!(clock.office, Office::Kindling | Office::Dayspring)
        }
        AreaBedSchedule::TallageWeighing => workday && open_hours,
        AreaBedSchedule::MarensGreenFishArrival => {
            (workday && clock.office == Office::Kindling)
                || (clock.weekday == Weekday::Lowmarket && clock.office == Office::Watch)
        }
        // Handled above; the clock never gates it.
        AreaBedSchedule::DrapersReachRain => raining,
        AreaBedSchedule::TenterhookClothWork => workday && open_hours,
        AreaBedSchedule::CinderRowDay | AreaBedSchedule::CutFreight => {
            workday
                && matches!(
                    clock.office,
                    Office::Kindling | Office::Dayspring | Office::HighWick | Office::Waning
                )
        }
        AreaBedSchedule::GauntPassageWork => workday && open_hours,
        AreaBedSchedule::HungryOxEvening => {
            occupants > 0
                && matches!(
                    clock.office,
                    Office::Lamplight | Office::Snuffing | Office::Watch
                )
        }
        AreaBedSchedule::OldSluiceDay | AreaBedSchedule::SkinnersCourtDay => {
            clock.brightness > 0.22
        }
        AreaBedSchedule::SevenLoftsStock => {
            workday
                && matches!(
                    clock.office,
                    Office::Kindling | Office::Dayspring | Office::HighWick | Office::Waning
                )
        }
    }
}

/// The multiplier on a bed's authored gain: which day it is, how full the room
/// is, and whether the weather has emptied an open pitch.
fn area_bed_gain_scale(
    bed: AreaBed,
    clock: Option<&WorldClockState>,
    weather: Option<&WorldWeatherState>,
    occupants: usize,
) -> f32 {
    let weekday = clock
        .filter(|clock| clock.present)
        .map(|clock| clock.weekday);
    let rain = weather.map_or(0.0, |weather| weather.current.precipitation as f32);
    let mut scale = match bed.schedule {
        // The Tallage and Maren's Green are the Lowmarket's two squares.
        AreaBedSchedule::TallageWeighing | AreaBedSchedule::MarensGreenFishArrival => {
            if weekday == Some(Weekday::Lowmarket) {
                1.18
            } else {
                1.0
            }
        }
        // The lofts fill before a market day and empty after it.
        AreaBedSchedule::SevenLoftsStock => {
            if matches!(weekday, Some(Weekday::Highmarket | Weekday::Lowmarket)) {
                1.15
            } else {
                1.0
            }
        }
        // Rain is the subject of this bed, not an attenuator of it.
        AreaBedSchedule::DrapersReachRain => (0.42 + rain * 0.86).clamp(0.0, 1.25),
        // A couple of boatmen at their pots is not a full room.
        AreaBedSchedule::HungryOxEvening => (0.52 + occupants as f32 * 0.16).min(1.15),
        _ => 1.0,
    };
    if bed.open_air {
        // A downpour thins the people in an open pitch; it does not silence the
        // place, and the weather stems carry the rain itself.
        scale *= 1.0 - ((rain - 0.28) / 0.72).clamp(0.0, 1.0) * 0.45;
    }
    scale
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

fn inside_box(position: Vec3, min: Vec3, max: Vec3) -> bool {
    position.cmpge(min).all() && position.cmple(max).all()
}

/// Mirrors the three canonical `lanthorn_interior` boxes in
/// `assets/world/areas.json`. Keeping the loop at the listener while it is in
/// one of these volumes makes the room tone fill the building without leaking
/// from a point source through the west doors or transept walls.
fn inside_lanthorn_interior(position: Vec3) -> bool {
    inside_box(
        position,
        Vec3::new(-44.0, 0.0, -104.0),
        Vec3::new(44.0, 83.0, 81.0),
    ) || inside_box(
        position,
        Vec3::new(-67.0, 0.0, -39.0),
        Vec3::new(-44.0, 83.0, -7.0),
    ) || inside_box(
        position,
        Vec3::new(44.0, 0.0, -39.0),
        Vec3::new(67.0, 83.0, -7.0),
    )
}

fn inside_saint_maren_congregation_area(position: Vec3) -> bool {
    // The parish church itself is not an explorable interior yet. A narrow
    // eight-metre apron admits people gathered at its doors and churchyard
    // edge without counting the whole fish quarter as a congregation.
    // The box's centre rides the Maren cluster; the half-extents are kept
    // because the church itself did not shrink.
    inside_box(
        position,
        Vec3::new(-163.5, -1.0, -301.6),
        Vec3::new(-117.5, 18.0, -249.6),
    )
}

fn congregation_murmur_active(occupants: usize, clock: Option<&WorldClockState>) -> bool {
    // A large group is authoritative even after dark (for example a future
    // crisis shelter). Smaller groups need an ordinary service/pilgrim hour.
    occupants >= 8
        || (occupants >= 4
            && clock.filter(|clock| clock.present).is_some_and(|clock| {
                matches!(
                    clock.office,
                    Office::Dayspring | Office::HighWick | Office::Waning
                )
            }))
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

/// Slow-moving crowd counts, refreshed at 2 Hz: which places hold how many
/// people changes over seconds of walking, and each count is a full pass over
/// the cast. Bed occupancy is computed lazily, at most once per bed per
/// refresh window.
#[derive(Default)]
struct OccupancyCache {
    next_refresh_at: f64,
    wickmarket_population: usize,
    lanthorn_occupants: usize,
    saint_maren_occupants: usize,
    bed_occupants: HashMap<u64, usize>,
}

#[allow(clippy::too_many_arguments)]
fn update_virtualized_loops(
    mut commands: Commands,
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    activity: Option<Res<AudioActivity>>,
    assets: Res<SoundscapeAssets>,
    beds: Option<Res<AreaBedGeometry>>,
    player: Query<&Transform, With<PlayerController>>,
    actors: Query<&GlobalTransform, With<ActorView>>,
    mut occupancy: Local<OccupancyCache>,
    cart_views: Query<(&RoadCartView, &GlobalTransform)>,
    carts: Res<CartSoundState>,
    wells: Res<WellSoundState>,
    work: Res<WorkSoundState>,
    nature: Res<UrbanNatureState>,
    weather: Option<Res<WorldWeatherState>>,
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
    let _span = crate::perf::span(crate::perf::Probe::Soundscape);
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    let busy = activity.as_deref().is_some_and(|activity| activity.busy);
    let player_position = player.single().ok().map(|transform| transform.translation);
    let existing_keys: HashSet<u64> = playing.iter().map(|(_, state, _, _)| state.key).collect();
    let mut demands = Vec::new();
    if now >= occupancy.next_refresh_at {
        occupancy.next_refresh_at = now + 0.5;
        occupancy.bed_occupants.clear();
        occupancy.wickmarket_population = actors
            .iter()
            .filter(|transform| {
                transform
                    .translation()
                    .xz()
                    .distance_squared(WICKMARKET.xz())
                    < 41.0_f32.powi(2)
            })
            .take(3)
            .count();
        (occupancy.lanthorn_occupants, occupancy.saint_maren_occupants) =
            actors
                .iter()
                .fold((0_usize, 0_usize), |(lanthorn, maren), transform| {
                    let position = transform.translation();
                    (
                        lanthorn + usize::from(inside_lanthorn_interior(position)),
                        maren + usize::from(inside_saint_maren_congregation_area(position)),
                    )
                });
    }
    let wickmarket_population = occupancy.wickmarket_population;

    // The named-place beds come first: they decide which point sources the
    // listener is currently hearing through a screen rather than in the open.
    let mut muffled_by_bed: HashSet<SoundscapeSound> = HashSet::new();
    if let (Some(beds), Some(listener)) = (beds.as_deref(), player_position) {
        for bed in AREA_BEDS {
            let Some((anchor, distance_m)) = beds.anchor(bed.area_id, listener) else {
                continue;
            };
            let reach = bed.spill_m
                * if existing_keys.contains(&bed.key) {
                    LOOP_ACTIVATION_HYSTERESIS
                } else {
                    1.0
                };
            if distance_m > reach {
                continue;
            }
            let occupants = if bed.schedule.needs_occupancy() {
                *occupancy.bed_occupants.entry(bed.key).or_insert_with(|| {
                    actors
                        .iter()
                        .filter(|transform| beds.occupied_by(bed.area_id, transform.translation()))
                        .take(8)
                        .count()
                })
            } else {
                0
            };
            if !area_bed_is_active(
                bed.schedule,
                clock.as_deref(),
                weather.as_deref(),
                occupants,
            ) {
                continue;
            }
            if beds.contains(bed.area_id, listener) {
                muffled_by_bed.extend(bed.muffles.iter().copied());
            }
            let descriptor = bed.sound.descriptor();
            demands.push(LoopDemand {
                key: bed.key,
                sound: bed.sound,
                position: anchor,
                gain: descriptor.gain
                    * area_bed_gain_scale(bed, clock.as_deref(), weather.as_deref(), occupants),
                radius_m: descriptor.radius_m,
                speed: 1.0,
                priority: bed.priority,
            });
        }
    }

    for emitter in STATIC_EMITTERS {
        let overridden =
            work_kind_for_sound(emitter.sound).is_some_and(|kind| work.0.contains_key(&kind));
        let ravens_scattered =
            emitter.sound == SoundscapeSound::NorthTowerRavens && now < nature.ravens_silent_until;
        let storm_shy_animal = wildlife_suppressed(weather.as_deref())
            && matches!(
                emitter.sound,
                SoundscapeSound::NorthTowerRavens
                    | SoundscapeSound::SparrowsUnderEaves
                    | SoundscapeSound::SwallowsOverCourt
                    | SoundscapeSound::RiverWharfGulls
                    | SoundscapeSound::FliesAtWaste
                    | SoundscapeSound::HensInYard
                    | SoundscapeSound::BeesAtHoneyStall
            );
        // Severe weather removes the broad exposed-market bed only when the
        // projected crowd has actually dispersed. Covered pitches can remain
        // staffed, so weather alone is not a second authority for attendance.
        let weather_closed_market = wildlife_suppressed(weather.as_deref())
            && emitter.sound == SoundscapeSound::WickmarketCrowd
            && wickmarket_population < 3;
        if !overridden
            && !ravens_scattered
            && !storm_shy_animal
            && !weather_closed_market
            && schedule_is_active(emitter.schedule, clock.as_deref())
        {
            let descriptor = emitter.sound.descriptor();
            let muffled = if muffled_by_bed.contains(&emitter.sound) {
                MUFFLED_BEHIND_BED_GAIN
            } else {
                1.0
            };
            demands.push(LoopDemand {
                key: emitter.key,
                sound: emitter.sound,
                position: emitter.position,
                gain: descriptor.gain
                    * muffled
                    * static_emitter_weather_gain(emitter, weather.as_deref()),
                radius_m: descriptor.radius_m,
                speed: static_emitter_speed(emitter),
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

    let (lanthorn_occupants, saint_maren_occupants) = (
        occupancy.lanthorn_occupants,
        occupancy.saint_maren_occupants,
    );
    if let Some(listener) = player_position {
        if inside_lanthorn_interior(listener) {
            let descriptor = SoundscapeSound::LanthornNaveAir.descriptor();
            demands.push(LoopDemand {
                key: 4_000,
                sound: SoundscapeSound::LanthornNaveAir,
                position: listener,
                gain: descriptor.gain,
                radius_m: descriptor.radius_m,
                speed: 1.0,
                priority: 97,
            });
            if congregation_murmur_active(lanthorn_occupants, clock.as_deref()) {
                let descriptor = SoundscapeSound::CongregationPrayer.descriptor();
                demands.push(LoopDemand {
                    key: 4_001,
                    sound: SoundscapeSound::CongregationPrayer,
                    position: listener,
                    gain: descriptor.gain,
                    radius_m: descriptor.radius_m,
                    speed: 1.0,
                    priority: 99,
                });
            }
        }
        if congregation_murmur_active(saint_maren_occupants, clock.as_deref()) {
            let descriptor = SoundscapeSound::CongregationPrayer.descriptor();
            demands.push(LoopDemand {
                key: 4_002,
                sound: SoundscapeSound::CongregationPrayer,
                position: SAINT_MARENS_CHURCH,
                gain: descriptor.gain * 0.82,
                radius_m: descriptor.radius_m,
                speed: 0.992,
                priority: 94,
            });
        }
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
            if let Some(bed) = area_bed_for_key(state.key) {
                info!("[soundscape] bed out: {}", bed.area_id);
            }
            commands.entity(entity).despawn();
        }
    }

    for demand in selected.drain(..) {
        if represented.contains(&demand.key) {
            continue;
        }
        // A named-place bed is the identity of somewhere the player can stand,
        // so its comings and goings are worth a line: it is the only way a
        // drive script (or a bug report) can tell which place it was hearing.
        if let Some(bed) = area_bed_for_key(demand.key) {
            info!(
                "[soundscape] bed in: {} ({:.2} gain)",
                bed.area_id, demand.gain
            );
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
        SoundscapeSound::EelSmokeFire
        | SoundscapeSound::Loom
        | SoundscapeSound::GrainCart
        | SoundscapeSound::SparrowsUnderEaves => 2,
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
    let _span = crate::perf::span(crate::perf::Probe::Soundscape);
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
    fn every_route_has_a_unique_asset_and_the_right_container() {
        assert_eq!(ALL_SOUNDS.len(), 55);
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
        let assets: [&[u8]; 55] = [
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
            include_bytes!("../assets/sounds/soundscape/snd_241_north_tower_ravens.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_243_sparrows_under_the_eaves.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_244_swallows_over_a_court.wav"),
            include_bytes!(
                "../assets/sounds/soundscape/snd_245_river_gulls_at_the_outer_wharf.wav"
            ),
            include_bytes!("../assets/sounds/soundscape/snd_246_market_dog_warning_bark.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_247_sparr_furnace_yard_dogs.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_248_alley_cat_on_slate.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_259_flies_at_eel_smoke_and_offal.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_257_geese_at_a_gate_pond_rut.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_268_lightning_over_the_lanthorn.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_281_lanthorn_nave_air.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_292_congregation_prayer_murmur.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_256_hens_in_a_domestic_yard.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_258_bees_at_the_honey_stall.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_307_smallvoice_single_stroke.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_308_scold_single_stroke.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_321_gradine_ordinary_day_texture.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_322_wickmarket_at_lamplight.wav"),
            include_bytes!(
                "../assets/sounds/soundscape/snd_323_coswalds_yard_before_the_hammers.wav"
            ),
            include_bytes!("../assets/sounds/soundscape/snd_324_tallage_weighing_hour.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_325_marens_green_before_dayspring.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_328_drapers_reach_in_rain.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_329_tenterhook_lane_workday.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_330_cinder_row_behind_shutters.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_332_dry_cut_freight_corridor.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_333_gaunt_passage_by_day.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_337_hungry_ox_doorway_spill.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_338_old_sluice_in_daylight.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_339_skinners_court_work_and_home.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_340_seven_lofts_grain_interior.wav"),
            include_bytes!("../assets/sounds/soundscape/snd_065_gatekeepers_key_ring.mp3"),
            include_bytes!("../assets/sounds/soundscape/snd_080_stone_house_cell_door.mp3"),
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
    fn implementation_manifest_contains_every_runtime_route() {
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
        assert_eq!(manifest["implemented_count"], 55);
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
    fn urban_nature_schedules_and_locations_follow_the_city_clock_and_map() {
        let daylight = clock(Office::HighWick, Weekday::Second);
        let mut night = clock(Office::Watch, Weekday::Second);
        night.brightness = 0.05;
        assert!(schedule_is_active(
            EmitterSchedule::DaylightAnimals,
            Some(&daylight)
        ));
        assert!(!schedule_is_active(
            EmitterSchedule::DaylightAnimals,
            Some(&night)
        ));
        assert!(schedule_is_active(
            EmitterSchedule::WarmDayWaste,
            Some(&daylight)
        ));
        assert!(!schedule_is_active(
            EmitterSchedule::WarmDayWaste,
            Some(&night)
        ));
        assert!(dusk_or_night(Some(&night)));
        assert!(!dusk_or_night(Some(&daylight)));
        assert!(!dusk_or_night(None));

        assert!(NORTH_TOWER_NESTS.y > 40.0);
        assert!(OUTER_FISH_WHARF.x < -400.0);
        assert!(OUTER_FISH_WHARF.xz().distance(TALLAGE_SQUARE) > 250.0);
        assert_eq!(
            STATIC_EMITTERS
                .iter()
                .filter(|emitter| emitter.sound == SoundscapeSound::SparrowsUnderEaves)
                .count(),
            5
        );
        assert_eq!(
            STATIC_EMITTERS
                .iter()
                .filter(|emitter| emitter.sound == SoundscapeSound::SwallowsOverCourt)
                .count(),
            3
        );
        let fly_sources: Vec<_> = STATIC_EMITTERS
            .iter()
            .filter(|emitter| emitter.sound == SoundscapeSound::FliesAtWaste)
            .collect();
        assert_eq!(fly_sources.len(), 3);
        assert!(fly_sources.iter().all(|emitter| {
            emitter.sound.descriptor().radius_m <= 14.0
                && emitter.schedule == EmitterSchedule::WarmDayWaste
        }));
        assert_eq!(GATE_GEESE_ANCHORS.len(), 4);
        assert!(
            GATE_GEESE_ANCHORS
                .iter()
                .all(|source| source.xz().length() > 350.0)
        );
    }

    #[test]
    fn hens_and_bees_are_small_sources_in_the_places_that_keep_them() {
        let geometry = AreaBedGeometry::from_shipped_map();
        let map = cathedral_sim::AreaMap::from_json_str(AREA_MAP_SOURCE).expect("area map");

        // One honey pitch on the Wickmarket, audible only at the stall.
        assert!(geometry.contains("wickmarket", HONEY_STALL));
        assert!(SoundscapeSound::BeesAtHoneyStall.descriptor().radius_m <= 14.0);

        let hens: Vec<_> = STATIC_EMITTERS
            .iter()
            .filter(|emitter| emitter.sound == SoundscapeSound::HensInYard)
            .collect();
        assert_eq!(hens.len(), HEN_YARD_ANCHORS.len());
        for anchor in HEN_YARD_ANCHORS {
            // Not on the swept ceremonial steps, and not stacked on a market bed.
            assert!(!geometry.contains("gradine", anchor), "{anchor} is Gradine");
            let position = cathedral_sim::math::Vec3::new(
                f64::from(anchor.x),
                f64::from(anchor.y),
                f64::from(anchor.z),
            );
            let market = map
                .containing_area(position)
                .is_some_and(|area| matches!(area.id.as_str(), "wickmarket" | "tallage"));
            assert!(!market, "{anchor} is inside a market square");
            assert!(
                anchor.xz().distance(WICKMARKET.xz()) > 60.0,
                "{anchor} is inside the Wickmarket crowd bed"
            );
        }
        // Five separate wards, so no two yards can ever be heard at once.
        for (index, left) in HEN_YARD_ANCHORS.iter().enumerate() {
            for right in &HEN_YARD_ANCHORS[index + 1..] {
                assert!(left.distance(*right) > 100.0, "{left} and {right} overlap");
            }
        }

        // Covering the pots quiets the bees long before a storm sends them in.
        let mut drizzle = WorldWeatherState::default();
        drizzle.current.precipitation = 0.25;
        let bees = *STATIC_EMITTERS
            .iter()
            .find(|emitter| emitter.sound == SoundscapeSound::BeesAtHoneyStall)
            .expect("the honey pitch is emitted");
        assert_eq!(static_emitter_weather_gain(bees, None), 1.0);
        assert!(static_emitter_weather_gain(bees, Some(&drizzle)) < 0.65);
        assert!(
            !wildlife_suppressed(Some(&drizzle)),
            "drizzle is not a storm"
        );
        assert!(schedule_is_active(
            EmitterSchedule::HoneyStallDay,
            Some(&clock(Office::HighWick, Weekday::Second))
        ));
        assert!(!schedule_is_active(
            EmitterSchedule::HoneyStallDay,
            Some(&clock(Office::HighWick, Weekday::Bellday))
        ));
        assert!(!schedule_is_active(
            EmitterSchedule::HoneyStallDay,
            Some(&clock(Office::Lamplight, Weekday::Second))
        ));
    }

    #[test]
    fn every_named_place_bed_binds_to_a_shipped_area_within_its_own_carry() {
        let geometry = AreaBedGeometry::from_shipped_map();
        let mut keys = HashSet::new();
        let mut sounds = HashSet::new();
        let static_keys: HashSet<u64> = STATIC_EMITTERS.iter().map(|emitter| emitter.key).collect();
        for bed in AREA_BEDS {
            assert!(
                geometry.0.contains_key(bed.area_id),
                "`{}` is not an area in the shipped map",
                bed.area_id
            );
            assert!(keys.insert(bed.key), "duplicate bed key {}", bed.key);
            assert!(
                !static_keys.contains(&bed.key),
                "bed {} collides with a static emitter",
                bed.key
            );
            assert!(
                sounds.insert(bed.sound),
                "two beds share {:?}",
                bed.sound.descriptor().file
            );
            let descriptor = bed.sound.descriptor();
            assert_eq!(descriptor.mode, ClipMode::Loop);
            // Belonging is always tighter than carry, so the loop selector's own
            // radius check can never reject a bed the spill test admitted.
            assert!(
                bed.spill_m < descriptor.radius_m,
                "{} spills further than it carries",
                bed.area_id
            );
            assert!(bed.spill_m >= 16.0);
        }
        // The muffle relation may only name sounds that actually have emitters.
        for bed in AREA_BEDS {
            for muffled in bed.muffles {
                assert!(
                    STATIC_EMITTERS
                        .iter()
                        .any(|emitter| emitter.sound == *muffled),
                    "{} muffles a sound nothing emits",
                    bed.area_id
                );
            }
        }
    }

    #[test]
    fn a_bed_follows_the_nearest_part_of_its_place_and_stops_having_a_direction_inside() {
        let geometry = AreaBedGeometry::from_shipped_map();

        // The Cut is five boxes over three quarters of a kilometre: a listener
        // at its southern end must hear the corridor beside them, not a point
        // source at its middle.
        let south = Vec3::new(-213.5, 1.0, -350.0);
        let (anchor, distance) = geometry.anchor("the_cut", south).expect("the Cut resolves");
        assert_eq!(distance, 0.0, "inside the corridor");
        assert_eq!(anchor, south, "the bed has no direction from inside");

        let north = Vec3::new(-213.5, 1.0, 280.0);
        let (_, inside_north) = geometry.anchor("the_cut", north).expect("the Cut resolves");
        assert_eq!(inside_north, 0.0, "the same bed, 630 m up the corridor");

        // Approaching from the side lands on the nearest wall of the nearest box.
        // The Cut's post-shrink area boxes are seam-derived from the translated
        // bridges, so its east face sits at x = -207.9, not centreline - 8.
        let beside = Vec3::new(-193.5, 1.0, 140.0);
        let (anchor, distance) = geometry
            .anchor("the_cut", beside)
            .expect("the Cut resolves");
        assert!((distance - 14.4).abs() < 0.01, "{distance}");
        assert_eq!(anchor.x, -207.9);
        assert_eq!(anchor.z, beside.z, "the anchor tracks along the corridor");

        // Height counts: flying over Gaunt Passage is not standing in it.
        let (_, overhead) = geometry
            .anchor("gaunt_passage", Vec3::new(-154.8, 60.0, 16.1))
            .expect("the passage resolves");
        assert!(overhead > 40.0, "{overhead}");

        assert!(geometry.contains("hungry_ox", Vec3::new(-237.5, 1.0, -338.6)));
        assert!(!geometry.contains("hungry_ox", Vec3::new(-237.5, 1.0, -283.6)));
        assert!(geometry.anchor("no_such_place", Vec3::ZERO).is_none());

        // The round parks a tavern worker on the Ox's own nav node — which sits
        // about a metre inside the box — and lets them drift up to their eight
        // metre leash. Strict containment reported the tavern empty all evening.
        let ox_workplace_node = Vec3::new(-231.875, 1.0, -324.725);
        assert!(geometry.contains("hungry_ox", ox_workplace_node));
        let leash_north = ox_workplace_node + Vec3::Z * 8.0;
        assert!(
            !geometry.contains("hungry_ox", leash_north),
            "over the edge"
        );
        assert!(
            geometry.occupied_by("hungry_ox", leash_north),
            "a worker at the end of their leash is still working the Ox"
        );
        assert!(!geometry.occupied_by("hungry_ox", Vec3::new(-237.5, 1.0, -303.6)));
    }

    #[test]
    fn place_beds_hand_over_to_each_other_instead_of_playing_at_once() {
        for weekday in Weekday::ALL {
            for office in Office::ALL {
                let clock = clock(office, weekday);
                // The Wickmarket is one square: the crowd or the close-down bed,
                // never both stacked on the same stalls.
                let crowd = schedule_is_active(EmitterSchedule::WickmarketHighmarket, Some(&clock));
                let closing =
                    area_bed_is_active(AreaBedSchedule::WickmarketCloseDown, Some(&clock), None, 0);
                assert!(!(crowd && closing), "{weekday:?} {office:?}");

                // Maren's Green wakes before Dayspring and hands over to the
                // Lowmarket fish market that owns the same ground after it.
                let arrival = area_bed_is_active(
                    AreaBedSchedule::MarensGreenFishArrival,
                    Some(&clock),
                    None,
                    0,
                );
                let market = schedule_is_active(EmitterSchedule::MarenLowmarket, Some(&clock));
                assert!(!(arrival && market), "{weekday:?} {office:?}");
            }
        }

        let dawn = clock(Office::Kindling, Weekday::Second);
        assert!(area_bed_is_active(
            AreaBedSchedule::MarensGreenFishArrival,
            Some(&dawn),
            None,
            0
        ));
        assert!(area_bed_is_active(
            AreaBedSchedule::CoswaldsFirstLight,
            Some(&dawn),
            None,
            0
        ));
        assert!(!area_bed_is_active(
            AreaBedSchedule::GradineOrdinaryDay,
            Some(&clock(Office::HighWick, Weekday::Bellday)),
            None,
            0
        ));
        assert!(area_bed_is_active(
            AreaBedSchedule::GradineOrdinaryDay,
            Some(&clock(Office::HighWick, Weekday::Second)),
            None,
            0
        ));

        // Folklore empties Gaunt Passage after dark; so does its bed.
        assert!(!area_bed_is_active(
            AreaBedSchedule::GauntPassageWork,
            Some(&clock(Office::Watch, Weekday::Second)),
            None,
            0
        ));

        // An empty tavern spills nothing, however late it is.
        let evening = clock(Office::Lamplight, Weekday::Second);
        assert!(!area_bed_is_active(
            AreaBedSchedule::HungryOxEvening,
            Some(&evening),
            None,
            0
        ));
        assert!(area_bed_is_active(
            AreaBedSchedule::HungryOxEvening,
            Some(&evening),
            None,
            2
        ));
        assert!(!area_bed_is_active(
            AreaBedSchedule::HungryOxEvening,
            Some(&clock(Office::HighWick, Weekday::Second)),
            None,
            6
        ));

        // Only the tavern asks about people at all.
        for schedule in [
            AreaBedSchedule::GradineOrdinaryDay,
            AreaBedSchedule::CutFreight,
            AreaBedSchedule::SevenLoftsStock,
        ] {
            assert!(!schedule.needs_occupancy());
        }
        assert!(AreaBedSchedule::HungryOxEvening.needs_occupancy());
    }

    #[test]
    fn the_drapers_reach_bed_is_weather_shaped_and_the_open_pitches_thin_in_rain() {
        let mut rain = WorldWeatherState::default();
        rain.current.kind = WeatherKind::Rain;
        rain.current.precipitation = 0.55;
        let noon = clock(Office::HighWick, Weekday::Second);
        let night = clock(Office::Watch, Weekday::Second);

        // The gallery in rain is the subject, so the clock has no vote at all.
        assert!(area_bed_is_active(
            AreaBedSchedule::DrapersReachRain,
            Some(&noon),
            Some(&rain),
            0
        ));
        assert!(area_bed_is_active(
            AreaBedSchedule::DrapersReachRain,
            Some(&night),
            Some(&rain),
            0
        ));
        assert!(!area_bed_is_active(
            AreaBedSchedule::DrapersReachRain,
            Some(&noon),
            None,
            0
        ));

        let reach = AREA_BEDS
            .into_iter()
            .find(|bed| bed.schedule == AreaBedSchedule::DrapersReachRain)
            .expect("the Reach has a bed");
        let cut = AREA_BEDS
            .into_iter()
            .find(|bed| bed.schedule == AreaBedSchedule::CutFreight)
            .expect("the Cut has a bed");
        assert!(
            area_bed_gain_scale(reach, Some(&noon), Some(&rain), 0)
                > area_bed_gain_scale(reach, Some(&noon), None, 0),
            "harder rain must make the gallery louder, not quieter"
        );
        assert!(
            area_bed_gain_scale(cut, Some(&noon), Some(&rain), 0) < 0.9,
            "an open corridor loses its people in a downpour"
        );

        // Market days fill the squares that trade on them.
        let tallage = AREA_BEDS
            .into_iter()
            .find(|bed| bed.schedule == AreaBedSchedule::TallageWeighing)
            .expect("the Tallage has a bed");
        assert!(
            area_bed_gain_scale(
                tallage,
                Some(&clock(Office::HighWick, Weekday::Lowmarket)),
                None,
                0
            ) > area_bed_gain_scale(tallage, Some(&noon), None, 0)
        );

        // A full room spills more than two boatmen, and never without bound.
        let ox = AREA_BEDS
            .into_iter()
            .find(|bed| bed.schedule == AreaBedSchedule::HungryOxEvening)
            .expect("the Ox has a bed");
        let quiet = area_bed_gain_scale(ox, Some(&noon), None, 1);
        let full = area_bed_gain_scale(ox, Some(&noon), None, 8);
        assert!(quiet < full && full <= 1.15, "{quiet} {full}");
    }

    #[test]
    fn church_beds_are_zone_shaped_and_require_a_congregation() {
        assert!(inside_lanthorn_interior(Vec3::new(0.0, 1.0, 20.0)));
        assert!(inside_lanthorn_interior(Vec3::new(-60.0, 1.0, -23.0)));
        assert!(!inside_lanthorn_interior(Vec3::new(60.0, 1.0, 20.0)));
        assert!(!inside_lanthorn_interior(Vec3::new(0.0, 84.0, -23.0)));
        assert!(inside_saint_maren_congregation_area(SAINT_MARENS_CHURCH));
        assert!(!inside_saint_maren_congregation_area(Vec3::ZERO));

        let pilgrim_hour = clock(Office::HighWick, Weekday::Second);
        let night = clock(Office::Watch, Weekday::Second);
        assert!(!congregation_murmur_active(3, Some(&pilgrim_hour)));
        assert!(congregation_murmur_active(4, Some(&pilgrim_hour)));
        assert!(!congregation_murmur_active(4, Some(&night)));
        assert!(congregation_murmur_active(8, Some(&night)));
    }

    #[test]
    fn the_two_civic_bells_stay_countable_and_hang_in_their_own_towers() {
        let map = cathedral_sim::AreaMap::from_json_str(AREA_MAP_SOURCE).expect("area map");
        let area_of = |position: Vec3| {
            map.containing_area(cathedral_sim::math::Vec3::new(
                f64::from(position.x),
                f64::from(position.y),
                f64::from(position.z),
            ))
            .map(|area| area.id.clone())
        };
        assert_eq!(area_of(SCOLD_TOWER).as_deref(), Some("bellstand_tower"));
        assert_eq!(
            area_of(SMALLVOICE_TOWER).as_deref(),
            Some("saint_marens_church")
        );
        // Both hang above the street rather than sounding out of a doorway.
        const { assert!(SCOLD_TOWER.y > 18.0 && SMALLVOICE_TOWER.y > 10.0) };

        let curfew = BellPattern::ScoldCurfew.plan();
        let summons = BellPattern::ScoldSummons.plan();
        let knell = BellPattern::NameKnell { years: 17 }.plan();

        // A counted curfew can never be misread as a counted hour: the greatest
        // office rings seven, and the Scold's law rings more than any of them.
        let longest_office = Office::ALL
            .into_iter()
            .map(Office::ordinal)
            .max()
            .expect("seven offices");
        assert!(curfew.strokes > u16::from(longest_office));
        assert_eq!(curfew.interval_seconds, BELL_STROKE_INTERVAL_SECONDS);
        // The summons is too quick to be counted as an hour at all.
        assert!(summons.interval_seconds < BELL_STROKE_INTERVAL_SECONDS * 0.5);
        assert_eq!(knell.strokes, 17, "the count is the age, unrounded");
        assert_eq!(knell.interval_seconds, BELL_STROKE_INTERVAL_SECONDS);

        // The Scold is the eastern city's bell; the knell is the Reed Ward's.
        assert!(summons.sound.descriptor().radius_m > knell.sound.descriptor().radius_m);
        // Both stay below the Lanthorn's own storm voice in scale.
        assert!(
            summons.sound.descriptor().radius_m
                < SoundscapeSound::LightningOverLanthorn.descriptor().radius_m
        );

        // A count can be absurd in a caller; it is never absurd at the rope.
        assert_eq!(BellPattern::NameKnell { years: 0 }.plan().strokes, 1);
        assert_eq!(
            BellPattern::NameKnell { years: 9_000 }.plan().strokes,
            MAX_BELL_STROKES
        );
        assert_eq!(BellPattern::MAX_KNELL_YEARS, MAX_BELL_STROKES);
    }

    #[test]
    fn an_assembled_peal_is_jittered_by_hand_but_never_retuned() {
        let mut scheduled = ScheduledSounds::default();
        let plan = schedule_bell_pattern(
            BellPattern::NameKnell { years: 17 },
            100.0,
            "test",
            &mut scheduled,
        );
        assert_eq!(plan.strokes, 17);
        assert_eq!(scheduled.0.len(), 17);

        let mut times: Vec<f64> = scheduled.0.iter().map(|sound| sound.at).collect();
        times.sort_by(f64::total_cmp);
        for (stroke, at) in times.iter().enumerate() {
            let ideal = 100.0 + stroke as f64 * BELL_STROKE_INTERVAL_SECONDS;
            let offset = (at - ideal).abs();
            assert!(
                (0.020..=0.040).contains(&offset),
                "stroke {stroke} moved {offset}s — hands, not a sequencer, and not a shuffle"
            );
        }
        for sound in &scheduled.0 {
            assert_eq!(
                sound.speed, 1.0,
                "a bell the player must count may never be retuned"
            );
            assert!((0.94..=1.06).contains(&sound.gain_scale));
            assert_eq!(sound.sound, SoundscapeSound::SmallvoiceStroke);
            assert_eq!(sound.position, SMALLVOICE_TOWER);
            assert!(
                sound.at >= 100.0,
                "no stroke lands before the rope is pulled"
            );
        }

        // The same peal is the same peal: `fake_backend` runs must not drift.
        let mut again = ScheduledSounds::default();
        schedule_bell_pattern(
            BellPattern::NameKnell { years: 17 },
            100.0,
            "test",
            &mut again,
        );
        let replay: Vec<f64> = again.0.iter().map(|sound| sound.at).collect();
        let first: Vec<f64> = scheduled.0.iter().map(|sound| sound.at).collect();
        assert_eq!(first, replay);
    }

    #[test]
    fn the_scold_rings_curfew_once_a_day_and_only_after_the_seventh_office() {
        fn curfew_app() -> App {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .init_resource::<ScheduledSounds>()
                .init_resource::<CueCooldowns>()
                .init_resource::<CivicBellState>()
                .insert_resource(clock(Office::Waning, Weekday::Second))
                .add_systems(Update, schedule_curfew_bell);
            app
        }
        fn strokes(app: &App) -> usize {
            app.world()
                .resource::<ScheduledSounds>()
                .0
                .iter()
                .filter(|sound| sound.sound == SoundscapeSound::ScoldStroke)
                .count()
        }

        let mut app = curfew_app();
        // The first projection after startup is a reading, not a ringing.
        app.update();
        *app.world_mut().resource_mut::<WorldClockState>() =
            clock(Office::Snuffing, Weekday::Second);
        app.update();
        let expected = usize::from(BellPattern::ScoldCurfew.plan().strokes);
        assert_eq!(strokes(&app), expected, "the law follows the office");
        app.update();
        assert_eq!(strokes(&app), expected, "and only once that evening");

        // Midnight arrives *inside* the Snuffing (21:00–02:00), so the day
        // number changes while the office does not. That is not a second
        // curfew, and keying the edge on the day would have made it one.
        let mut past_midnight = clock(Office::Snuffing, Weekday::Highmarket);
        past_midnight.day = 3;
        *app.world_mut().resource_mut::<WorldClockState>() = past_midnight;
        app.update();
        assert_eq!(strokes(&app), expected, "midnight is not a bell");

        // The next day's Snuffing is a new curfew.
        let mut tomorrow = clock(Office::Watch, Weekday::Highmarket);
        tomorrow.day = 1;
        *app.world_mut().resource_mut::<WorldClockState>() = tomorrow;
        app.update();
        let mut tomorrow_night = clock(Office::Snuffing, Weekday::Highmarket);
        tomorrow_night.day = 1;
        *app.world_mut().resource_mut::<WorldClockState>() = tomorrow_night;
        app.update();
        assert_eq!(strokes(&app), expected * 2);

        // Starting the game already inside the Snuffing rings nothing: the
        // player did not miss a bell, there was no bell.
        let mut fresh = curfew_app();
        *fresh.world_mut().resource_mut::<WorldClockState>() =
            clock(Office::Snuffing, Weekday::Second);
        fresh.update();
        fresh.update();
        assert_eq!(strokes(&fresh), 0);
    }

    /// The same midnight the curfew's edge detector was built for, one system
    /// along. The yawning hours run Lamplight to the end of the Snuffing —
    /// 18:00 to 02:00 — so the day number rolls over *inside* the window, and a
    /// yawn keyed on the day re-armed at midnight and was yawned a second time
    /// before the evening was out.
    #[test]
    fn a_body_yawns_once_an_evening_though_the_day_rolls_over_inside_it() {
        use std::time::Duration;

        /// Everything a yawn reads and nothing else: one body standing at arm's
        /// length, so the only thing left deciding is the hour. An absent
        /// `MovementInbox` reads as standing still and an absent `WorldMirror`
        /// as unwearied, which is the slowest yawn there is — 20 to 150 s.
        fn yawn_app() -> App {
            let mut app = App::new();
            app.init_resource::<Time>()
                .init_resource::<ScheduledSounds>()
                .init_resource::<NpcSoundState>()
                .insert_resource(clock(Office::Waning, Weekday::Second))
                .add_systems(Update, schedule_npc_body_sounds);
            app.world_mut().spawn((
                PlayerController::moving_at(Vec3::ZERO),
                Transform::from_xyz(0.0, 1.7, 0.0),
            ));
            app.world_mut().spawn((
                ActorId("p001v".to_string()),
                ActorView,
                GlobalTransform::from_xyz(2.0, 0.0, 0.0),
            ));
            app
        }
        fn yawns(app: &App) -> usize {
            app.world()
                .resource::<ScheduledSounds>()
                .0
                .iter()
                .filter(|sound| sound.sound == SoundscapeSound::EveningYawn)
                .count()
        }
        /// Five minutes of one office, scanned at the system's own rate — long
        /// enough that a body owed a yawn has certainly yawned it.
        fn spend_an_hour_in(app: &mut App, office: Office, day: i64) {
            let mut hour = clock(office, Weekday::Second);
            hour.day = day;
            *app.world_mut().resource_mut::<WorldClockState>() = hour;
            for _ in 0..30 {
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(Duration::from_secs(10));
                app.update();
            }
        }

        let mut app = yawn_app();
        spend_an_hour_in(&mut app, Office::Lamplight, 3);
        assert_eq!(yawns(&app), 1, "the lamps are lit and the body is tired");
        spend_an_hour_in(&mut app, Office::Snuffing, 3);
        assert_eq!(yawns(&app), 1, "and it is one evening, not two offices");

        // Midnight, hours into the Snuffing: a new day over the same evening.
        spend_an_hour_in(&mut app, Office::Snuffing, 4);
        assert_eq!(yawns(&app), 1, "midnight is not a second evening");

        spend_an_hour_in(&mut app, Office::Watch, 4);
        spend_an_hour_in(&mut app, Office::Dayspring, 4);
        assert_eq!(yawns(&app), 1, "nobody yawns their way through the day");

        // Sunset again, and the evening that opens with it is owed one.
        spend_an_hour_in(&mut app, Office::Lamplight, 4);
        assert_eq!(yawns(&app), 2);
    }

    #[test]
    fn a_summons_cried_during_the_curfew_peal_never_falls_between_its_strokes() {
        fn strokes(app: &App, sound: SoundscapeSound) -> usize {
            app.world()
                .resource::<ScheduledSounds>()
                .0
                .iter()
                .filter(|queued| queued.sound == sound)
                .count()
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SoundscapeCue>()
            .init_resource::<ScheduledSounds>()
            .init_resource::<CueCooldowns>()
            .init_resource::<CivicBellState>()
            .init_resource::<WellSoundState>()
            .init_resource::<WorkSoundState>()
            .insert_resource(clock(Office::Waning, Weekday::Second))
            .add_systems(
                Update,
                (ingest_soundscape_cues, schedule_curfew_bell).chain(),
            );

        // The office edges into the Snuffing and the whole curfew is queued at
        // once, a dusk grace ahead of its first stroke.
        app.update();
        *app.world_mut().resource_mut::<WorldClockState>() =
            clock(Office::Snuffing, Weekday::Second);
        app.update();
        let curfew = usize::from(BellPattern::ScoldCurfew.plan().strokes);
        assert_eq!(strokes(&app, SoundscapeSound::ScoldStroke), curfew);

        // An officer cries a summons while that peal is still to come.
        app.world_mut()
            .write_message(SoundscapeCue::CivicBell(BellPattern::ScoldSummons));
        app.update();
        assert_eq!(
            strokes(&app, SoundscapeSound::ScoldStroke),
            curfew,
            "five quick strokes among the nine would leave the curfew uncountable"
        );

        // Maren Smallvoice hangs in her own tower and is not held by the Scold.
        app.world_mut()
            .write_message(SoundscapeCue::CivicBell(BellPattern::NameKnell {
                years: 3,
            }));
        app.update();
        assert_eq!(strokes(&app, SoundscapeSound::SmallvoiceStroke), 3);
    }

    #[test]
    fn a_bell_cue_queues_one_peal_and_refuses_to_overlap_itself() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SoundscapeCue>()
            .init_resource::<ScheduledSounds>()
            .init_resource::<CueCooldowns>()
            .init_resource::<WellSoundState>()
            .init_resource::<WorkSoundState>()
            .add_systems(Update, ingest_soundscape_cues);
        let expected = usize::from(BellPattern::ScoldSummons.plan().strokes);

        app.world_mut()
            .write_message(SoundscapeCue::CivicBell(BellPattern::ScoldSummons));
        app.world_mut()
            .write_message(SoundscapeCue::CivicBell(BellPattern::ScoldSummons));
        app.update();

        let queued = &app.world().resource::<ScheduledSounds>().0;
        assert_eq!(
            queued.len(),
            expected,
            "a second summons on a ringing one would make the strokes uncountable"
        );

        // The other bell is a different rope and is not blocked by the Scold.
        app.world_mut()
            .write_message(SoundscapeCue::CivicBell(BellPattern::NameKnell {
                years: 3,
            }));
        app.update();
        let queued = &app.world().resource::<ScheduledSounds>().0;
        assert_eq!(
            queued
                .iter()
                .filter(|sound| sound.sound == SoundscapeSound::SmallvoiceStroke)
                .count(),
            3
        );
    }

    #[test]
    fn lightning_delay_is_physical_real_time() {
        let origin = Vec3::new(0.0, 140.0, -23.0);
        assert_eq!(lightning_sound_delay(origin, origin), 0.0);
        assert!(lightning_sound_delay(Vec3::ZERO, origin) > 0.4);
    }

    #[test]
    fn crossed_lightning_message_queues_one_delayed_thunder() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<WeatherLightning>()
            .insert_resource(ScheduledSounds::default())
            .add_systems(Update, schedule_weather_thunder);
        app.world_mut().spawn((
            PlayerController::default(),
            Transform::from_translation(Vec3::ZERO),
        ));
        app.world_mut()
            .write_message(WeatherLightning(cathedral_sim::LightningStrike {
                id: 7,
                game_instant_days: 1.5,
                origin_m: [0.0, 140.0, -23.0],
                strength: 0.9,
            }));

        app.update();

        let queued = &app.world().resource::<ScheduledSounds>().0;
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].sound, SoundscapeSound::LightningOverLanthorn);
        assert!(queued[0].at > 0.0, "sound waits for propagation");
        app.update();
        assert_eq!(app.world().resource::<ScheduledSounds>().0.len(), 1);
    }

    #[test]
    fn shelter_crossfades_exterior_rain_to_the_matching_roof() {
        let mut sample = cathedral_sim::WeatherSample::CLEAR;
        sample.kind = WeatherKind::Rain;
        sample.precipitation = 0.62;
        sample.surface_wetness = 0.7;
        let outside = weather_mix(sample, CoverMaterial::Open, false, false);
        let slate = weather_mix(sample, CoverMaterial::Slate, true, false);
        let canvas = weather_mix(sample, CoverMaterial::Canvas, true, false);
        let nave = weather_mix(sample, CoverMaterial::Slate, true, true);
        assert!(outside.gains[WeatherStem::HeavyExterior as usize] > 0.0);
        assert_eq!(outside.gains[WeatherStem::HardRoof as usize], 0.0);
        assert!(slate.gains[WeatherStem::HardRoof as usize] > 0.0);
        assert_eq!(slate.gains[WeatherStem::HeavyExterior as usize], 0.0);
        assert!(canvas.gains[WeatherStem::SoftRoof as usize] > 0.0);
        assert!(
            nave.gains[WeatherStem::MuffledExterior as usize]
                < slate.gains[WeatherStem::MuffledExterior as usize]
        );
    }

    #[test]
    fn generated_weather_clips_are_valid_bounded_pcm_wavs() {
        for clip in WeatherStem::ALL
            .map(WeatherClip::Stem)
            .into_iter()
            .chain([WeatherClip::WetStep, WeatherClip::PuddleSplash])
        {
            let bytes = weather_wav(clip);
            assert_eq!(&bytes[0..4], b"RIFF");
            assert_eq!(&bytes[8..12], b"WAVE");
            assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1);
            assert_eq!(
                u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
                WEATHER_AUDIO_SAMPLE_RATE
            );
            assert!(bytes.len() > 20_000);
        }
    }

    #[test]
    fn animal_approaches_rearm_with_hysteresis_and_ravens_clear_the_bells() {
        let mut approaches = HashMap::new();
        assert!(proximity_entered(
            &mut approaches,
            1,
            Vec3::X * 26.0,
            Vec3::ZERO,
            27.0,
            42.0
        ));
        assert!(!proximity_entered(
            &mut approaches,
            1,
            Vec3::X * 35.0,
            Vec3::ZERO,
            27.0,
            42.0
        ));
        assert!(!proximity_entered(
            &mut approaches,
            1,
            Vec3::X * 43.0,
            Vec3::ZERO,
            27.0,
            42.0
        ));
        assert!(proximity_entered(
            &mut approaches,
            1,
            Vec3::X * 26.0,
            Vec3::ZERO,
            27.0,
            42.0
        ));

        let expected = 6.0 * BELL_STROKE_INTERVAL_SECONDS + TOWN_BELL_CLIP_SECONDS;
        assert_eq!(office_bell_span_seconds(Office::Snuffing), expected);
        let mut nature = UrbanNatureState::default();
        let waning = clock(Office::Waning, Weekday::Second);
        let snuffing = clock(Office::Snuffing, Weekday::Second);
        nature.observe_office_bell(10.0, Some(&waning));
        assert_eq!(
            nature.ravens_silent_until, 0.0,
            "initial projection is not a bell"
        );
        nature.observe_office_bell(20.0, Some(&snuffing));
        assert_eq!(nature.ravens_silent_until, 20.0 + expected);
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
        assert_eq!(cooldowns.free_at.len(), 1);

        // A hold claims a window that has not begun yet, and a prune landing
        // inside it must not clear it: an entry stamped ahead of now is a sound
        // still to be heard, not a stale one.
        cooldowns.hold(11, 300.0);
        assert!(!cooldowns.allow(11, 299.9, 1.0));
        assert!(cooldowns.allow(11, 300.0, 1.0));
        // The longer sound keeps the floor.
        cooldowns.hold(11, 310.0);
        cooldowns.hold(11, 305.0);
        assert!(!cooldowns.allow(11, 309.9, 1.0));
    }

    #[test]
    fn headless_plugin_registers_contract_without_audio_output() {
        let mut app = App::new();
        app.add_plugins(SoundscapePlugin);
        assert!(app.world().contains_resource::<ScheduledSounds>());
        assert!(app.world().contains_resource::<WellSoundState>());
        assert!(app.world().contains_resource::<WellMechanismActivity>());
        assert!(app.world().contains_resource::<UrbanNatureState>());
        assert!(app.world().contains_resource::<WeatherAudioState>());
        assert!(app.world().contains_resource::<WorldWeatherState>());
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
            tallage_measurement_anchor(Vec3::new(-213.5, 0.0, 63.0)),
            Some(Vec3::new(-214.2, 1.5, 45.5))
        );
        // A representative stall pitch ~30 m from the beam remains in the
        // market catchment, while the neighbouring Chain Well does not.
        assert_eq!(
            tallage_measurement_anchor(Vec3::new(-239.396, 0.0, 61.217)),
            Some(TALLAGE_WEIGHBEAM)
        );
        assert_eq!(tallage_measurement_anchor(CHAIN_WELL), None);
        assert_eq!(tallage_measurement_anchor(Vec3::splat(f32::NAN)), None);
    }
}
